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
//! - `--mode identify`: no measurement; one row naming the build (whether
//!   `occ-stats` is compiled in) and the CPUs the process may run on. The
//!   driver probes the binary with it before any cell, and every measured row
//!   carries the same two fields.
//! - `--mode seed-check --seed S`: no measurement; asserts that 8 writer
//!   streams seeded from `S` draw pairwise distinct values over their first
//!   1,024 draws, and that the `seed ^ (0x5A5A + w)` seeding they replaced
//!   does not (it gave writers 2k and 2k + 1 one stream). Every `--map sync`
//!   run makes the first check on the seeds its threads take, before any
//!   thread starts.
//!
//! `--noncompact-top 1` (sync `overwrite` at `--writers 1` only; METHODOLOGY
//! §42.3, #1481) adds `insert_top_noncompact_ns`: the 8 largest latencies
//! of the inserts across which the arena generation did not change, so the
//! inserts that ran a compaction are left out. The generation is read under
//! `with_locked` after each insert, outside the timed call, and the reading
//! after one insert is the reading before the next. The existing
//! `insert_top_ns` is unchanged. `--cap-mib N` replaces the 1 GiB cap with
//! N MiB of 2 MiB chunks for the driver's `--quick` smoke run.
//!
//! Built with `occ-stats`, a `--map sync` row also carries the window's
//! `counters` (METHODOLOGY section 37.2): the gate and drain waits, the
//! `SeqVersion::sample` spins, and the CPU time the window's worker threads
//! consumed (`thread_cpu_ns`, and `thread_cpu_cycles`, the same time in TSC
//! ticks). The wait counters are TSC deltas, so they include any time a
//! waiting thread was descheduled, which `thread_cpu_ns` does not: a share
//! taken against it can exceed what the waits cost in CPU. The denominator is
//! measured per-thread CPU time (`CLOCK_THREAD_CPUTIME_ID`), never wall time
//! multiplied by the thread count, which overstates it whenever the threads
//! outnumber the CPUs they may run on. A counters build times nothing the
//! driver reads.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `reclaim_stall` |
//! | `group` | 4 |
//! | `population` | 200,000 or 3,600,000 live keys (`--live`); in `full` mode, as many distinct keys as the 1 GiB arena admits |
//! | `insertion_order` | generator — the prefill inserts keys `0..L` in order; overwrites and reads draw keys uniformly from them with a per-thread XorShift64, each thread seeded through SplitMix64's finaliser so no two threads share a stream (checked at startup) |
//! | `probes_and_reuse` | `overwrite`: 2.2 append cycles of overwrites split across the writers; `full`: fresh keys for `--seconds`; readers read live keys throughout |
//! | `hit_rate` | reads 100 % (every read key is live) |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | reads take a pinned zero-copy view and consume its length |
//! | `measured_region` | `Instant::now()` around each insert and each read, uncalibrated; the prefill and fill are outside every histogram |
//! | `arm_symmetry` | N/A — one map per process; the two maps are compared only on their own predictions (§29.3) |
//! | `statistics` | HdrHistogram (3 significant figures) per operation kind per process, plus the 8 largest insert latencies (with `--noncompact-top 1`, also the 8 largest of the inserts that ran no compaction) and a counting allocator's peak heap; the driver runs 5 rounds per cell as separate processes and computes BCa intervals over rounds |
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
/// Records of 128 B one 2 MiB chunk holds.
const RECORDS_PER_CHUNK: u64 = 14_563;
/// The default arena chunk, which `--cap-mib` divides.
const CHUNK_BYTES: u64 = 2 << 20;
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

/// Thread `t`'s stream seed: `seed` and `t` through SplitMix64's finaliser
/// (Steele, Lea & Flood, "Fast Splittable Pseudorandom Number Generators",
/// OOPSLA 2014), the function `writer_seed` in `strmap_poll_stall.rs`. The
/// golden-ratio step is odd and the finaliser is a bijection, so distinct `t`
/// give distinct seeds for every `seed`. Writer `w` is stream `w`; reader `r`
/// is stream `READER_STREAM_BASE + r`.
fn thread_seed(seed: u64, t: u64) -> u64 {
    let mut z = seed.wrapping_add((t + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The first reader's stream index, above any writer count the harness runs.
const READER_STREAM_BASE: u64 = 1 << 32;
/// Draws per stream that [`streams_overlap`] compares.
const SEED_CHECK_DRAWS: usize = 1024;
/// The writer count the seed check always covers, whatever `--writers` is.
const SEED_CHECK_WRITERS: u64 = 8;

/// The first pair of streams, by index into `seeds`, whose first `draws`
/// `XorShift64` outputs share a value, or `None` when every value is distinct.
/// Two streams started from one state share all of them.
fn streams_overlap(seeds: &[u64], draws: usize) -> Option<(usize, usize)> {
    let mut first: HashMap<u64, usize> = HashMap::with_capacity(seeds.len() * draws);
    for (i, &s) in seeds.iter().enumerate() {
        let mut rng = XorShift64::new(s);
        for _ in 0..draws {
            let v = rng.next();
            match first.get(&v) {
                Some(&j) if j != i => return Some((j, i)),
                Some(_) => {}
                None => {
                    first.insert(v, i);
                }
            }
        }
    }
    None
}

/// The seeds of `writers` writer and `readers` reader streams.
fn stream_seeds(seed: u64, writers: u64, readers: u64) -> Vec<u64> {
    (0..writers)
        .map(|w| thread_seed(seed, w))
        .chain((0..readers).map(|r| thread_seed(seed, READER_STREAM_BASE + r)))
        .collect()
}

/// The run's stream seeds, writers first and then readers, which the threads
/// take by index. Panics unless these, and the streams of
/// [`SEED_CHECK_WRITERS`] writers, draw pairwise distinct values over their
/// first [`SEED_CHECK_DRAWS`] draws.
fn checked_stream_seeds(seed: u64, writers: u64, readers: u64) -> Vec<u64> {
    let seeds = stream_seeds(seed, writers, readers);
    let wide = stream_seeds(seed, writers.max(SEED_CHECK_WRITERS), readers);
    for set in [&seeds, &wide] {
        if let Some((a, b)) = streams_overlap(set, SEED_CHECK_DRAWS) {
            panic!(
                "seed {seed:#x}: streams {a} and {b} share a draw in their first {SEED_CHECK_DRAWS}"
            );
        }
    }
    seeds
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
    /// The largest latencies of the inserts that ran no compaction
    /// (`--noncompact-top`); empty otherwise.
    top_noncompact: Vec<u64>,
    ok: u64,
    refused: u64,
}

/// Inserts `ns` into a descending top-`TOP` list.
fn push_top(top: &mut Vec<u64>, ns: u64) {
    if top.len() < TOP || ns > *top.last().expect("non-empty") {
        let at = top.partition_point(|&t| t >= ns);
        top.insert(at, ns);
        top.truncate(TOP);
    }
}

impl Ops {
    fn new() -> Self {
        Self {
            hist: hist(),
            top: Vec::with_capacity(TOP + 1),
            top_noncompact: Vec::with_capacity(TOP + 1),
            ok: 0,
            refused: 0,
        }
    }
    fn record(&mut self, ns: u64) {
        self.hist
            .record(ns.clamp(1, 59_999_999_999))
            .expect("in range");
        push_top(&mut self.top, ns);
    }
    fn merge(&mut self, other: Ops) {
        self.hist.add(other.hist).expect("same bounds");
        for t in other.top {
            let at = self.top.partition_point(|&x| x >= t);
            self.top.insert(at, t);
        }
        self.top.truncate(TOP);
        for t in other.top_noncompact {
            push_top(&mut self.top_noncompact, t);
        }
        self.ok += other.ok;
        self.refused += other.refused;
    }
}

fn timed_insert<F: FnOnce() -> Result<(), ArenaError>>(ops: &mut Ops, f: F) -> u64 {
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
    ns
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

/// CPU time consumed by the calling thread so far, in nanoseconds
/// (`CLOCK_THREAD_CPUTIME_ID`), or `None` where the platform has no such clock.
/// Time spent descheduled is not counted.
#[cfg(unix)]
fn thread_cpu_ns() -> Option<u64> {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid, writable `timespec` for the duration of the call.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    (rc == 0).then(|| ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64)
}

#[cfg(not(unix))]
fn thread_cpu_ns() -> Option<u64> {
    None
}

/// The CPU time the calling thread used since `start`, a `thread_cpu_ns()` reading.
fn cpu_since(start: Option<u64>) -> Option<u64> {
    Some(thread_cpu_ns()?.saturating_sub(start?))
}

/// Sum of the workers' CPU times; `None` if any worker had no reading.
fn sum_cpu_ns(parts: impl IntoIterator<Item = Option<u64>>) -> Option<u64> {
    parts.into_iter().try_fold(0u64, |acc, p| Some(acc + p?))
}

/// `ns` nanoseconds of CPU time in ticks of a `hz` clock.
#[cfg_attr(not(feature = "occ-stats"), allow(dead_code))]
fn ns_to_ticks(ns: u64, hz: u64) -> u64 {
    (u128::from(ns) * u128::from(hz) / 1_000_000_000) as u64
}

/// The CPUs this process may run on, as the kernel lists them
/// (`Cpus_allowed_list` in `/proc/self/status`), or `None` where there is no
/// such file. The driver confines each child and refuses a row whose list
/// differs, so a cell never runs under a pin other than the one it records.
fn cpu_affinity() -> Option<String> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find_map(|l| l.strip_prefix("Cpus_allowed_list:"))
        .map(|v| v.trim().to_string())
}

/// The occ-stats counters of the window just run: the waits of the two
/// writer-side loops and of `SeqVersion::sample`, and the CPU time its worker
/// threads consumed, in nanoseconds and in the same ticks as the waits.
#[cfg(feature = "occ-stats")]
fn window_counters(thread_cpu_ns: Option<u64>) -> serde_json::Value {
    use expanse_trie::occ_stats::{self, Stat};
    let snap = occ_stats::snapshot();
    let tsc_hz = occ_stats::cycles_hz(Duration::from_millis(200));
    serde_json::json!({
        "sample_spins": snap[Stat::SampleSpins as usize],
        "sample_spin_cycles": snap[Stat::SampleSpinCycles as usize],
        "gate_blocked_entries": snap[Stat::GateBlockedEntries as usize],
        "gate_wait_cycles": snap[Stat::GateWaitCycles as usize],
        "quiesce_calls": snap[Stat::QuiesceCalls as usize],
        "quiesce_drain_cycles": snap[Stat::QuiesceDrainCycles as usize],
        "tsc_hz": tsc_hz,
        "thread_cpu_ns": thread_cpu_ns,
        "thread_cpu_cycles": thread_cpu_ns.map(|ns| ns_to_ticks(ns, tsc_hz)),
    })
}

/// What this binary was built as, so the driver can refuse a build that is
/// not the one it expects.
fn identity(row: &mut serde_json::Value) {
    row["occ_stats"] = cfg!(feature = "occ-stats").into();
    row["cpu_affinity"] = cpu_affinity().into();
}

fn main() {
    let a = Args::parse();
    let map = a.get("map").to_string();
    let mode = a.get("mode").to_string();
    if mode == "identify" {
        // The driver's probe: one row naming the build, no measurement.
        let mut row = serde_json::json!({"workload_id": "reclaim_stall", "mode": "identify"});
        identity(&mut row);
        println!("{row}");
        return;
    }
    if mode == "seed-check" {
        // The seeding's fail-then-pass control: the seeds the run uses must
        // pass, and the per-writer `seed ^ (0x5A5A + w)` seeding this replaced
        // must fail, since `XorShift64::new`'s `| 1` gives writers 2k and
        // 2k + 1 one stream.
        let seed = a.num("seed");
        let new = streams_overlap(&stream_seeds(seed, SEED_CHECK_WRITERS, 0), SEED_CHECK_DRAWS);
        let old: Vec<u64> = (0..SEED_CHECK_WRITERS)
            .map(|w| seed ^ (0x5A5A + w))
            .collect();
        let old = streams_overlap(&old, SEED_CHECK_DRAWS);
        assert_eq!(new, None, "the writer seeding gives two writers one stream");
        assert!(
            old.is_some(),
            "the replaced seeding no longer collides: the control is void"
        );
        let row = serde_json::json!({
            "workload_id": "reclaim_stall", "mode": "seed-check", "seed": seed,
            "writers": SEED_CHECK_WRITERS, "draws": SEED_CHECK_DRAWS,
            "overlap": new, "replaced_seeding_overlap": old,
        });
        println!("{row}");
        return;
    }
    let writers = a.num("writers").max(1);
    let readers = a.num("readers");
    let round = a.num("round");
    let seed = a.num("seed");
    let mut row = serde_json::json!({
        "workload_id": "reclaim_stall", "map": map, "mode": mode,
        "writers": writers, "readers": readers, "round": round, "seed": seed,
        "payload_len": PAYLOAD_LEN,
    });
    identity(&mut row);

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
            let full = mode == "full";
            let noncompact = a.0.get("noncompact-top").is_some_and(|v| v == "1");
            assert!(
                !noncompact || (!full && writers == 1),
                "--noncompact-top is defined for one overwrite writer only"
            );
            let cap_mib = a.0.get("cap-mib").map(|v| {
                v.parse::<u64>()
                    .unwrap_or_else(|_| panic!("--cap-mib must be an integer"))
            });
            let (m, arena_records) = match cap_mib {
                None => (Arc::new(SyncExpanseBlobMap::new()), ARENA_RECORDS),
                Some(mib) => {
                    let cap = mib << 20;
                    assert!(
                        cap >= 4 * CHUNK_BYTES && cap % CHUNK_BYTES == 0,
                        "--cap-mib must be a multiple of 2 and at least 8"
                    );
                    (
                        Arc::new(SyncExpanseBlobMap::with_chunk_size_and_max_capacity(
                            CHUNK_BYTES as usize,
                            cap as usize,
                        )),
                        RECORDS_PER_CHUNK * (cap / CHUNK_BYTES),
                    )
                }
            };
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
            #[cfg(feature = "occ-stats")]
            expanse_trie::occ_stats::reset();
            let heap_before = HEAP.load(Ordering::Relaxed);
            PEAK.store(heap_before, Ordering::Relaxed);
            let appends = if full {
                0
            } else {
                (arena_records - live) * 22 / 10
            };
            let seconds = if full { a.num("seconds") } else { 0 };
            let seeds = checked_stream_seeds(seed, writers, readers);
            let stop = Arc::new(AtomicBool::new(false));
            let barrier = Arc::new(Barrier::new((writers + readers + 1) as usize));
            let rhs: Vec<_> = (0..readers)
                .map(|r| {
                    let (m, stop, b) = (Arc::clone(&m), Arc::clone(&stop), Arc::clone(&barrier));
                    let rseed = seeds[(writers + r) as usize];
                    std::thread::spawn(move || {
                        b.wait();
                        let c0 = thread_cpu_ns();
                        let h = reader_loop(&m, live, rseed, &stop);
                        (h, cpu_since(c0))
                    })
                })
                .collect();
            let whs: Vec<_> = (0..writers)
                .map(|w| {
                    let (m, stop, b) = (Arc::clone(&m), Arc::clone(&stop), Arc::clone(&barrier));
                    let share = appends / writers + u64::from(w < appends % writers);
                    let wseed = seeds[w as usize];
                    std::thread::spawn(move || {
                        let mut ops = Ops::new();
                        let mut rng = XorShift64::new(wseed);
                        b.wait();
                        let c0 = thread_cpu_ns();
                        if full {
                            let mut i = 0u64;
                            while !stop.load(Ordering::Relaxed) {
                                let k = (1u64 << 40) + (w << 32) + i;
                                let p = payload(k, 1);
                                timed_insert(&mut ops, || m.insert(k, &p, HOT_META));
                                i += 1;
                            }
                        } else if noncompact {
                            // The generation after one insert is the one
                            // before the next; both reads are outside the
                            // timed call.
                            let mut g = m.with_locked(|t| t.arena().generation());
                            for i in 0..share {
                                let k = rng.next() % live;
                                let p = payload(k, i + 1);
                                let ns = timed_insert(&mut ops, || m.insert(k, &p, HOT_META));
                                let after = m.with_locked(|t| t.arena().generation());
                                if after == g {
                                    push_top(&mut ops.top_noncompact, ns);
                                }
                                g = after;
                            }
                        } else {
                            for i in 0..share {
                                let k = rng.next() % live;
                                let p = payload(k, i + 1);
                                timed_insert(&mut ops, || m.insert(k, &p, HOT_META));
                            }
                        }
                        (ops, cpu_since(c0))
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
            let mut cpu_parts = Vec::new();
            for h in whs {
                let (o, cpu) = h.join().expect("writer panicked");
                ops.merge(o);
                cpu_parts.push(cpu);
            }
            let elapsed = t0.elapsed().as_secs_f64();
            stop.store(true, Ordering::Relaxed);
            let mut reads = hist();
            for h in rhs {
                let (rh, cpu) = h.join().expect("reader panicked");
                reads.add(rh).expect("same bounds");
                cpu_parts.push(cpu);
            }
            let thread_cpu_ns = sum_cpu_ns(cpu_parts);
            #[cfg(feature = "occ-stats")]
            {
                row["counters"] = window_counters(thread_cpu_ns);
            }
            #[cfg(not(feature = "occ-stats"))]
            let _ = thread_cpu_ns;
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
            if noncompact {
                row["insert_top_noncompact_ns"] = ops.top_noncompact.clone().into();
            }
            if let Some(mib) = cap_mib {
                row["cap_mib"] = mib.into();
            }
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
