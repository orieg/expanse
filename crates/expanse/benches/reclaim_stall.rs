//! The stall and the memory peak of reclamation at the arena cap
//! (`docs/benchmarks/concurrency/METHODOLOGY.md` §29, #1300 item 1).
//!
//! One process measures one (cell, round) and prints one JSON row;
//! `docs/benchmarks/concurrency/scripts/reclaim_stall.py` runs the cells,
//! rotates their order per round, and writes the artifact.
//!
//! - `--mode overwrite`: prefill `--live` keys with 128 B payloads, then
//!   `--writers` threads overwrite uniformly drawn live keys until 2.2 append
//!   cycles have run, so the arena reaches the 1 GiB cap twice and the reclaim
//!   rule compacts it twice. On `--map sync`, `--readers` threads read
//!   uniformly drawn live keys for the whole window.
//! - `--mode full`: fill a `SyncExpanseBlobMap` with distinct keys until an
//!   insert is refused (the waste guard then declines every compaction), then
//!   `--writers` threads insert fresh keys for `--seconds` while `--readers`
//!   read.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `reclaim_stall` |
//! | `group` | 4 |
//! | `population` | 200,000 or 3,600,000 live keys (`--live`); in `full` mode, as many distinct keys as the 1 GiB arena admits |
//! | `insertion_order` | generator — the prefill inserts keys `0..L` in order; overwrites and reads draw keys uniformly from them with a per-thread XorShift64 |
//! | `probes_and_reuse` | `overwrite`: 2.2 append cycles of overwrites split across the writers; `full`: fresh keys for `--seconds`; readers read live keys throughout |
//! | `hit_rate` | reads 100 % (every read key is live) |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | reads take a pinned zero-copy view and consume its length |
//! | `measured_region` | `Instant::now()` around each insert and each read, uncalibrated; the prefill and fill are outside every histogram |
//! | `arm_symmetry` | N/A — one map per process; the two maps are compared only on their own predictions (§29.3) |
//! | `statistics` | HdrHistogram (3 significant figures) per operation kind per process, plus the 8 largest insert latencies and a counting allocator's peak heap; the driver runs 5 rounds per cell as separate processes and computes BCa intervals over rounds |
//! | `verdict` | **PRE-REGISTERED** — METHODOLOGY §29; read by the driver |

use expanse_trie::blobmap::{ArenaError, ExpanseBlobMap};
use expanse_trie::sync::SyncExpanseBlobMap;
use hdrhistogram::Histogram;
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashMap;
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

/// Counts the process's live heap and its peak.
struct Counting;

static HEAP: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every method forwards to `System` unchanged and only adds
// relaxed bookkeeping on the side.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded with the caller's layout.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let now = HEAP.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        p
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded with the caller's layout.
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            let now = HEAP.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` was allocated by this allocator with `layout`.
        unsafe { System.dealloc(ptr, layout) };
        HEAP.fetch_sub(layout.size(), Ordering::Relaxed);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: forwarded with the caller's arguments.
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            if new_size >= layout.size() {
                let now = HEAP.fetch_add(new_size - layout.size(), Ordering::Relaxed)
                    + (new_size - layout.size());
                PEAK.fetch_max(now, Ordering::Relaxed);
            } else {
                HEAP.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        p
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

const PAYLOAD_LEN: usize = 128;
const HOT_META: u32 = 1;
/// Records of 128 B one 2 MiB chunk holds, times the 512 chunks of a 1 GiB
/// arena (`records_per_chunk` and `max_chunks` in
/// `scripts/blob_reclaim_bounds.py`; the driver checks the cycle against it).
const ARENA_RECORDS: u64 = 14_563 * 512;
const TOP: usize = 8;

struct XorShift64(u64);
impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

fn payload(key: u64, round: u64) -> [u8; PAYLOAD_LEN] {
    let mut p = [0u8; PAYLOAD_LEN];
    let mix = key ^ round.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    for (i, b) in p.iter_mut().enumerate() {
        *b = (mix >> ((i % 8) * 8)) as u8 ^ i as u8;
    }
    p
}

fn hist() -> Histogram<u64> {
    Histogram::<u64>::new_with_max(60_000_000_000, 3).expect("histogram bounds")
}

/// One thread's insert record: its histogram and its largest latencies.
struct Ops {
    hist: Histogram<u64>,
    top: Vec<u64>,
    ok: u64,
    refused: u64,
}

impl Ops {
    fn new() -> Self {
        Self {
            hist: hist(),
            top: Vec::with_capacity(TOP + 1),
            ok: 0,
            refused: 0,
        }
    }
    fn record(&mut self, ns: u64) {
        self.hist
            .record(ns.clamp(1, 59_999_999_999))
            .expect("in range");
        if self.top.len() < TOP || ns > *self.top.last().expect("non-empty") {
            let at = self.top.partition_point(|&t| t >= ns);
            self.top.insert(at, ns);
            self.top.truncate(TOP);
        }
    }
    fn merge(&mut self, other: Ops) {
        self.hist.add(other.hist).expect("same bounds");
        for t in other.top {
            let at = self.top.partition_point(|&x| x >= t);
            self.top.insert(at, t);
        }
        self.top.truncate(TOP);
        self.ok += other.ok;
        self.refused += other.refused;
    }
}

fn timed_insert<F: FnOnce() -> Result<(), ArenaError>>(ops: &mut Ops, f: F) {
    let t = Instant::now();
    let r = f();
    let ns = t.elapsed().as_nanos() as u64;
    ops.record(ns);
    match r {
        Ok(()) => ops.ok += 1,
        // A refusal either way: the cap refused with no compaction
        // (`OffsetOverflow`) or after this insert compacted (`ArenaFull`).
        Err(ArenaError::OffsetOverflow | ArenaError::ArenaFull) => ops.refused += 1,
        Err(e) => panic!("insert failed with {e:?}"),
    }
}

fn percentiles(h: &Histogram<u64>) -> serde_json::Value {
    serde_json::json!({
        "count": h.len(),
        "p50_ns": h.value_at_quantile(0.50),
        "p99_ns": h.value_at_quantile(0.99),
        "p999_ns": h.value_at_quantile(0.999),
        "max_ns": h.max(),
    })
}

struct Args(HashMap<String, String>);
impl Args {
    fn parse() -> Self {
        let mut m = HashMap::new();
        let mut it = std::env::args().skip(1);
        while let Some(a) = it.next() {
            if a == "--bench" {
                continue;
            }
            let key = a
                .strip_prefix("--")
                .unwrap_or_else(|| panic!("unexpected argument {a}"));
            let val = it.next().unwrap_or_else(|| panic!("--{key} needs a value"));
            m.insert(key.to_string(), val);
        }
        Self(m)
    }
    fn get(&self, k: &str) -> &str {
        self.0
            .get(k)
            .map(String::as_str)
            .unwrap_or_else(|| panic!("missing --{k}"))
    }
    fn num(&self, k: &str) -> u64 {
        self.get(k)
            .parse()
            .unwrap_or_else(|_| panic!("--{k} must be an integer"))
    }
}

fn reader_loop(m: &SyncExpanseBlobMap, live: u64, seed: u64, stop: &AtomicBool) -> Histogram<u64> {
    let mut h = hist();
    let mut rng = XorShift64::new(seed);
    let mut rd = m.reader();
    let mut sink = 0usize;
    while !stop.load(Ordering::Relaxed) {
        let k = rng.next() % live;
        let t = Instant::now();
        let len = rd.pin().get(k).map_or(0, |(v, _)| v.as_bytes().len());
        let ns = t.elapsed().as_nanos() as u64;
        sink = sink.wrapping_add(len);
        h.record(ns.clamp(1, 59_999_999_999)).expect("in range");
    }
    black_box(sink);
    h
}

fn main() {
    let a = Args::parse();
    let map = a.get("map").to_string();
    let mode = a.get("mode").to_string();
    let writers = a.num("writers").max(1);
    let readers = a.num("readers");
    let round = a.num("round");
    let seed = a.num("seed");
    let mut row = serde_json::json!({
        "workload_id": "reclaim_stall", "map": map, "mode": mode,
        "writers": writers, "readers": readers, "round": round, "seed": seed,
        "payload_len": PAYLOAD_LEN,
    });

    match (map.as_str(), mode.as_str()) {
        ("plain", "overwrite") => {
            assert_eq!(
                (writers, readers),
                (1, 0),
                "the plain map runs one writer and no reader"
            );
            let live = a.num("live");
            let appends = (ARENA_RECORDS - live) * 22 / 10;
            let mut m = ExpanseBlobMap::new();
            for k in 0..live {
                m.insert(k, &payload(k, 0), HOT_META)
                    .expect("the live set fits");
            }
            let g0 = m.arena().generation();
            let heap_before = HEAP.load(Ordering::Relaxed);
            PEAK.store(heap_before, Ordering::Relaxed);
            let mut ops = Ops::new();
            let mut rng = XorShift64::new(seed);
            let t0 = Instant::now();
            for i in 0..appends {
                let k = rng.next() % live;
                let p = payload(k, i + 1);
                timed_insert(&mut ops, || m.insert(k, &p, HOT_META));
            }
            let elapsed = t0.elapsed().as_secs_f64();
            row["live"] = live.into();
            row["appends"] = appends.into();
            row["elapsed_s"] = elapsed.into();
            row["compactions"] = (m.arena().generation().wrapping_sub(g0)).into();
            row["index_mem_used"] = m.index().mem_used().into();
            row["arena_total_allocated"] = m.arena().mem_used().into();
            row["insert"] = percentiles(&ops.hist);
            row["insert_top_ns"] = ops.top.clone().into();
            row["inserts_ok"] = ops.ok.into();
            row["inserts_refused"] = ops.refused.into();
            row["heap_before_window"] = heap_before.into();
            row["heap_peak"] = PEAK.load(Ordering::Relaxed).into();
            row["heap_end"] = HEAP.load(Ordering::Relaxed).into();
            black_box(&m);
        }
        ("sync", "overwrite") | ("sync", "full") => {
            let m = Arc::new(SyncExpanseBlobMap::new());
            let full = mode == "full";
            let live = if full {
                let mut n = 0u64;
                loop {
                    match m.insert(n, &payload(n, 0), HOT_META) {
                        Ok(()) => n += 1,
                        Err(ArenaError::OffsetOverflow | ArenaError::ArenaFull) => break,
                        Err(e) => panic!("fill failed with {e:?}"),
                    }
                }
                n
            } else {
                let live = a.num("live");
                for k in 0..live {
                    m.insert(k, &payload(k, 0), HOT_META)
                        .expect("the live set fits");
                }
                live
            };
            let g0 = m.with_locked(|t| t.arena().generation());
            let heap_before = HEAP.load(Ordering::Relaxed);
            PEAK.store(heap_before, Ordering::Relaxed);
            let appends = if full {
                0
            } else {
                (ARENA_RECORDS - live) * 22 / 10
            };
            let seconds = if full { a.num("seconds") } else { 0 };
            let stop = Arc::new(AtomicBool::new(false));
            let barrier = Arc::new(Barrier::new((writers + readers + 1) as usize));
            let rhs: Vec<_> = (0..readers)
                .map(|r| {
                    let (m, stop, b) = (Arc::clone(&m), Arc::clone(&stop), Arc::clone(&barrier));
                    std::thread::spawn(move || {
                        b.wait();
                        reader_loop(&m, live, seed ^ (0xA5A5 + r), &stop)
                    })
                })
                .collect();
            let whs: Vec<_> = (0..writers)
                .map(|w| {
                    let (m, stop, b) = (Arc::clone(&m), Arc::clone(&stop), Arc::clone(&barrier));
                    let share = appends / writers + u64::from(w < appends % writers);
                    std::thread::spawn(move || {
                        let mut ops = Ops::new();
                        let mut rng = XorShift64::new(seed ^ (0x5A5A + w));
                        b.wait();
                        if full {
                            let mut i = 0u64;
                            while !stop.load(Ordering::Relaxed) {
                                let k = (1u64 << 40) + (w << 32) + i;
                                let p = payload(k, 1);
                                timed_insert(&mut ops, || m.insert(k, &p, HOT_META));
                                i += 1;
                            }
                        } else {
                            for i in 0..share {
                                let k = rng.next() % live;
                                let p = payload(k, i + 1);
                                timed_insert(&mut ops, || m.insert(k, &p, HOT_META));
                            }
                        }
                        ops
                    })
                })
                .collect();
            barrier.wait();
            let t0 = Instant::now();
            if full {
                std::thread::sleep(Duration::from_secs(seconds));
                stop.store(true, Ordering::Relaxed);
            }
            let mut ops = Ops::new();
            for h in whs {
                ops.merge(h.join().expect("writer panicked"));
            }
            let elapsed = t0.elapsed().as_secs_f64();
            stop.store(true, Ordering::Relaxed);
            let mut reads = hist();
            for h in rhs {
                reads
                    .add(h.join().expect("reader panicked"))
                    .expect("same bounds");
            }
            let (generation, index_mem, arena_total) = m.with_locked(|t| {
                (
                    t.arena().generation(),
                    t.index().mem_used(),
                    t.arena().mem_used(),
                )
            });
            row["live"] = live.into();
            row["appends"] = appends.into();
            row["seconds"] = seconds.into();
            row["elapsed_s"] = elapsed.into();
            row["compactions"] = generation.wrapping_sub(g0).into();
            row["index_mem_used"] = index_mem.into();
            row["arena_total_allocated"] = arena_total.into();
            row["insert"] = percentiles(&ops.hist);
            row["insert_top_ns"] = ops.top.clone().into();
            row["inserts_ok"] = ops.ok.into();
            row["inserts_refused"] = ops.refused.into();
            row["read"] = percentiles(&reads);
            row["heap_before_window"] = heap_before.into();
            row["heap_peak"] = PEAK.load(Ordering::Relaxed).into();
            row["heap_end"] = HEAP.load(Ordering::Relaxed).into();
        }
        _ => panic!("unknown --map {map} / --mode {mode}"),
    }
    println!("{row}");
}
