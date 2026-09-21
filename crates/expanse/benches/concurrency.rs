//! Concurrency scalability benchmark for SyncExpanseSet, SyncExpanseMap,
//! SyncExpanseBlobMap, SyncExpanseStrMap and SyncExpanseBytesMap (issue
//! #219: the blob arm compares the OCC wrapper against a
//! `Mutex<ExpanseBlobMap>` baseline, an `RwLock<BTreeMap>` and
//! `crossbeam_skiplist`; the string arm against `Mutex<ExpanseStrMap>` and
//! `DashMap` — `ExpanseStrMap`, like the other single-threaded structures,
//! is deliberately `!Sync`, so an `RwLock<ExpanseStrMap>` cannot legally be
//! shared). The bytes arm (issue #362) runs the *identical* workload as the
//! string and `DashMap` arms — same keys, same distribution — so the
//! unordered hash-keyed wrapper is directly comparable to `DashMap` and to
//! the ordered cascade it sidesteps.
//!
//! The `SyncExpanseMap32` arms (issue #573) are the Tier-1 host instrument
//! for the 32-bit single-writer/many-reader protocol: one writer thread
//! churns a disjoint key range while `threads` readers probe the stable
//! range with `try_get`, and each table reports reader throughput plus
//! the **Busy rate** — the protocol-health number, the share of read
//! attempts that observed an open write bracket and gave up — and the
//! writer's refusal count (`ArenaFull`/`ReclaimBacklog`). One table per
//! writer duty: full duty (every read races a bracket; the ceiling of
//! the Busy rate) and paced write rates of 1M, 100k and 10k mutations/s
//! bracketing embedded ingestion rates (a saturated CAN bus is ~10k
//! frames/s). Report-only like every arm here; a read path degrading to
//! permanent `Busy` shows up as a collapsed `busy` line, not a failed
//! gate.
//!
//! # Rounds and the measured window
//!
//! Each engine and workload builds and prefills one structure, then runs
//! `EXPANSE_BENCH_ROUNDS` rounds over it. A round is one window per thread
//! count, in a Williams order that rotates across rounds, so over a full cycle
//! every thread count takes every position and follows every other one equally
//! often (`n` rounds for an even number of thread counts, `2n` for an odd
//! one). A window starts at a barrier that every worker reaches after its own
//! setup, and its rates are its counts over its own elapsed time to the stop
//! signal, so thread creation and setup stay outside it. The tables print the
//! mean over a thread count's windows. `EXPANSE_BENCH_SAMPLES` names a file to
//! append every window to as one JSON line, which
//! `docs/benchmarks/concurrency/scripts/mixed_concurrency.py` reads to put BCa
//! 95% intervals on the cells. `EXPANSE_BENCH_ENGINES` selects arms by key:
//! `map`, `set`, `blob`, `blob_mutex`, `blob_rwlock_btree`, `blob_skiplist`,
//! `str`, `str_mutex`, `bytes`, `bytes_mutex`, `str_dashmap`, `sync32`.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `core_concurrency` |
//! | `group` | 2 |
//! | `population` | 1M draws (keyspace 2M); `SyncExpanseMap32` arm: 4,096 draws over an 8k keyspace whose upper 4,096 keys the writer churns (32-bit) |
//! | `insertion_order` | generator draw order — the population is inserted as drawn, neither sorted nor shuffled; prefill and the concurrent stream both draw from the bounded-keyspace PRNG |
//! | `probes_and_reuse` | Continuous stream; `EXPANSE_BENCH_ROUNDS` interleaved 500 ms windows per thread count (Williams order), all on one prefilled structure per engine and workload |
//! | `hit_rate` | ~39% at 100% read (derived: 1M uniform draws over 2M keys occupy 1 − e^(−1/2) of the keyspace); under a mix, writes insert even keys and remove odd ones, which moves occupancy toward half |
//! | `miss_gen_method` | Bounded keyspace random stream |
//! | `value_dereference` | `black_box(sink)` |
//! | `measured_region` | Clean (`run_rounds`): a window starts at a barrier after thread creation and per-thread setup, and its rates divide by its own elapsed time |
//! | `arm_symmetry` | Symmetric within three key types, not across them: u64 → u64 over 1M draws (`SyncExpanseMap`, `SyncExpanseSet`, with no third-party arm), u64 → 128-byte payload and u32 over 200k (`SyncExpanseBlobMap`, `Mutex<ExpanseBlobMap>`, `RwLock<BTreeMap>`, and `SkipMap`, whose values are `(Vec<u8>, u32)` although its label reads `Vec<u8>`), and 37-byte string keys → u64 over 100k (`SyncExpanseStrMap`, `SyncExpanseBytesMap`, their `Mutex` twins, `DashMap`). Compare arms only within a key type |
//! | `statistics` | Tables: mean ops/sec over a thread count's windows; `EXPANSE_BENCH_SAMPLES` carries every window for BCa 95% intervals (`docs/benchmarks/concurrency/scripts/mixed_concurrency.py`) |
//! | `verdict` | **MEASURED** `[verified: RUN (reference host, runs 34881026495 and 34882381735)]`: every cell of both runs, with its BCa interval, is in `docs/benchmarks/concurrency/README.md` §12; report-only, no gate is pre-registered on it. The #375 bounded-keyspace correction still holds. |

use crossbeam_skiplist::SkipMap;
use dashmap::DashMap;
use expanse_trie::ExpanseBlobMap;
use expanse_trie::bytesmap::ExpanseBytesMap;
use expanse_trie::strmap::ExpanseStrMap;
use expanse_trie::sync::{
    SyncExpanseBlobMap, SyncExpanseBytesMap, SyncExpanseMap, SyncExpanseSet, SyncExpanseStrMap,
};
use expanse_trie::sync32::{Busy, SyncExpanseMap32, WriteError};
use serde_json::json;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Barrier, RwLock};
use std::time::{Duration, Instant};

/// Wraps a key for `ExpanseStrMap`.
///
/// `new_unchecked`: the validating constructor would put a whole-key scan
/// inside the measured region, and the arm would measure the check instead of
/// the descent. Every generator in this file emits route-shaped ASCII.
#[inline(always)]
fn tk<B: AsRef<[u8]> + ?Sized>(bytes: &B) -> &expanse_trie::strmap::NulFreeStr {
    // SAFETY: generators in this file emit no NUL bytes.
    unsafe { expanse_trie::strmap::NulFreeStr::new_unchecked(bytes.as_ref()) }
}

struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

const POP: u64 = 1_000_000;
/// Map/set arms: prefill and probes draw from the same bounded keyspace
/// (the `BLOB_KEYSPACE` construction the blob arm below uses), so reads
/// actually hit ~50% of the time and exercise the full descent-to-value
/// path. Until #375 these arms probed unbounded `u64` keys against a
/// 1M-key prefill (~100% miss), so the read numbers measured early-exit
/// descent on absent keys only.
const KEYSPACE: u64 = 2 * POP;
const WINDOW: Duration = Duration::from_millis(500);

fn bench_map(ratio_read: u32, plan: &Plan) -> Vec<Sample> {
    let m = Arc::new(SyncExpanseMap::new());
    let mut rng = XorShift(0x5CA1_AB1E);
    for _ in 0..POP {
        let k = rng.next() % KEYSPACE;
        m.insert(k, !k);
    }
    run_rounds(plan, move |i, win| {
        let rd = m.reader();
        let mut rng = XorShift(0x1000 + i as u64);
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        win.begin();
        while win.running() {
            let r = (rng.next() % 100) as u32;
            let k = rng.next() % KEYSPACE;
            if r < ratio_read {
                sink ^= rd.get(k).unwrap_or(0);
                read_ops += 1;
            } else {
                if k & 1 == 0 {
                    m.insert(k, !k);
                } else {
                    m.remove(k);
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
}

fn bench_set(ratio_read: u32, plan: &Plan) -> Vec<Sample> {
    let s = Arc::new(SyncExpanseSet::new());
    let mut rng = XorShift(0x5CA1_AB1E);
    for _ in 0..POP {
        s.insert(rng.next() % KEYSPACE);
    }
    run_rounds(plan, move |i, win| {
        let rd = s.reader();
        let mut rng = XorShift(0x1000 + i as u64);
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = false;
        win.begin();
        while win.running() {
            let r = (rng.next() % 100) as u32;
            let k = rng.next() % KEYSPACE;
            if r < ratio_read {
                sink ^= rd.contains(k);
                read_ops += 1;
            } else {
                if k & 1 == 0 {
                    s.insert(k);
                } else {
                    s.remove(k);
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
}

/// Blob arm: bounded keyspace so lookups actually hit (~50%) and dereference
/// payload bytes; 128-byte payloads land squarely in the arena regime.
const BLOB_POP: u64 = 200_000;
const BLOB_KEYSPACE: u64 = 2 * BLOB_POP;
const BLOB_LEN: usize = 128;

fn blob_payload(k: u64, buf: &mut [u8; BLOB_LEN]) {
    let mut x = k | 1;
    for chunk in buf.chunks_mut(8) {
        x = x.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        chunk.copy_from_slice(&x.to_le_bytes()[..chunk.len()]);
    }
}

/// One measurement window, shared by its worker threads. Each worker does its
/// own setup (reader handle, RNG, buffers), then calls [`Window::begin`]; the
/// window's clock starts once every worker and the driver are through that
/// barrier, so thread creation and per-thread setup stay outside it.
struct Window {
    stop: AtomicBool,
    start: Barrier,
}

impl Window {
    fn new(parties: usize) -> Self {
        Self {
            stop: AtomicBool::new(false),
            start: Barrier::new(parties),
        }
    }

    /// Blocks until every worker and the driver are ready.
    fn begin(&self) {
        self.start.wait();
    }

    /// False once the driver has closed the window.
    fn running(&self) -> bool {
        !self.stop.load(Ordering::Relaxed)
    }
}

/// The thread counts a cell runs, and how many interleaved rounds.
struct Plan {
    threads: Vec<usize>,
    rounds: usize,
}

/// One window's raw counts over its own elapsed time. `busy`, `ok` and
/// `refused` are the `sync32` arm's protocol telemetry and stay zero elsewhere.
///
/// Under `occ-stats` each window also carries the protocol counters it
/// accumulated, zeroed at the window's barrier and read after its workers
/// join ([`run_rounds`]). The field does not exist in the default build, so
/// the timed windows this instrument publishes are unaffected; the census is
/// a diagnostic for attributing a cell's loss to restarts, fallbacks or
/// quiesce waits (Refs #1047).
#[derive(Clone, Copy)]
struct Sample {
    threads: usize,
    round: usize,
    position: usize,
    elapsed_s: f64,
    read_ops: u64,
    write_ops: u64,
    busy: u64,
    ok: u64,
    refused: u64,
    #[cfg(feature = "occ-stats")]
    stats: [u64; expanse_trie::occ_stats::NUM_STATS],
}

impl Default for Sample {
    fn default() -> Self {
        Self {
            threads: 0,
            round: 0,
            position: 0,
            elapsed_s: 0.0,
            read_ops: 0,
            write_ops: 0,
            busy: 0,
            ok: 0,
            refused: 0,
            #[cfg(feature = "occ-stats")]
            stats: [0; expanse_trie::occ_stats::NUM_STATS],
        }
    }
}

/// The order of `n` thread counts in `round`: row `round` of a Williams
/// design, so over a full cycle every level takes every position and follows
/// every other level equally often. The cycle is `n` rounds for even `n`; for
/// odd `n` it is `2n`, the second half being the first half's rows reversed.
fn williams_order(n: usize, round: usize) -> Vec<usize> {
    if n == 0 {
        return Vec::new();
    }
    let base: Vec<usize> = (0..n)
        .map(|k| match k {
            0 => 0,
            k if k % 2 == 1 => k.div_ceil(2),
            k => n - k / 2,
        })
        .collect();
    let period = if n.is_multiple_of(2) { n } else { 2 * n };
    let row = round % period;
    let mut order: Vec<usize> = base.iter().map(|&b| (b + row % n) % n).collect();
    if row >= n {
        order.reverse();
    }
    order
}

/// Runs every round of `plan` over one shared structure. Each round is one
/// window per thread count, in [`williams_order`]; a window is `threads`
/// copies of `work(thread_idx, window) -> (read_ops, write_ops)`, and its
/// elapsed time runs from the barrier to the stop signal.
fn run_rounds<F>(plan: &Plan, work: F) -> Vec<Sample>
where
    F: Fn(usize, &Window) -> (u64, u64) + Send + Sync + 'static,
{
    let work = Arc::new(work);
    let mut samples = Vec::with_capacity(plan.rounds * plan.threads.len());
    for round in 0..plan.rounds {
        for (position, idx) in williams_order(plan.threads.len(), round)
            .into_iter()
            .enumerate()
        {
            let threads = plan.threads[idx];
            let window = Arc::new(Window::new(threads + 1));
            let handles: Vec<_> = (0..threads)
                .map(|i| {
                    let window = Arc::clone(&window);
                    let work = Arc::clone(&work);
                    std::thread::spawn(move || work(i, &window))
                })
                .collect();
            // Zeroed inside the barrier: every worker is parked in
            // `Window::begin` and the prefill is long done, so the census
            // below covers this window and nothing else.
            #[cfg(feature = "occ-stats")]
            expanse_trie::occ_stats::reset();
            window.begin();
            let t0 = Instant::now();
            std::thread::sleep(WINDOW);
            window.stop.store(true, Ordering::Relaxed);
            let elapsed_s = t0.elapsed().as_secs_f64();
            let (mut read_ops, mut write_ops) = (0u64, 0u64);
            for h in handles {
                let (r, w) = h.join().expect("thread join");
                read_ops += r;
                write_ops += w;
            }
            samples.push(Sample {
                threads,
                round,
                position,
                elapsed_s,
                read_ops,
                write_ops,
                #[cfg(feature = "occ-stats")]
                stats: expanse_trie::occ_stats::snapshot(),
                ..Sample::default()
            });
        }
    }
    samples
}

fn bench_blob_sync(ratio_read: u32, plan: &Plan) -> Vec<Sample> {
    let m = Arc::new(SyncExpanseBlobMap::new());
    let mut rng = XorShift(0x5CA1_AB1E);
    let mut buf = [0u8; BLOB_LEN];
    for _ in 0..BLOB_POP {
        let k = rng.next() % BLOB_KEYSPACE;
        blob_payload(k, &mut buf);
        m.insert(k, &buf, k as u32 & 0xFF_FFFF).expect("prefill");
    }
    run_rounds(plan, move |i, win| {
        let mut rd = m.reader();
        let mut rng = XorShift(0x1000 + i as u64);
        let mut buf = [0u8; BLOB_LEN];
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        win.begin();
        while win.running() {
            let r = (rng.next() % 100) as u32;
            let k = rng.next() % BLOB_KEYSPACE;
            if r < ratio_read {
                let guard = rd.pin();
                if let Some((view, meta)) = guard.get(k) {
                    sink ^= u64::from(view.as_bytes()[0]) ^ u64::from(meta);
                }
                read_ops += 1;
            } else {
                if k & 1 == 0 {
                    blob_payload(k, &mut buf);
                    let _ = m.insert(k, &buf, k as u32 & 0xFF_FFFF);
                } else {
                    m.remove(k);
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
}

/// Locked baseline. `ExpanseBlobMap` is deliberately `!Sync` (its map's
/// insert-path cache is mutated through `&self` on some read APIs; shared
/// access is the `SyncExpanseBlobMap` wrapper's job), so an
/// `RwLock<ExpanseBlobMap>` cannot legally be shared — the honest std
/// baseline is a `Mutex`, with `RwLock<BTreeMap>` below capturing the
/// reader-counter scaling behaviour of an `RwLock` on a `Sync` structure.
fn bench_blob_mutex(ratio_read: u32, plan: &Plan) -> Vec<Sample> {
    let m = Arc::new(std::sync::Mutex::new(ExpanseBlobMap::new()));
    let mut rng = XorShift(0x5CA1_AB1E);
    let mut buf = [0u8; BLOB_LEN];
    {
        let mut g = m.lock().expect("lock");
        for _ in 0..BLOB_POP {
            let k = rng.next() % BLOB_KEYSPACE;
            blob_payload(k, &mut buf);
            g.insert(k, &buf, k as u32 & 0xFF_FFFF).expect("prefill");
        }
    }
    run_rounds(plan, move |i, win| {
        let mut rng = XorShift(0x1000 + i as u64);
        let mut buf = [0u8; BLOB_LEN];
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        win.begin();
        while win.running() {
            let r = (rng.next() % 100) as u32;
            let k = rng.next() % BLOB_KEYSPACE;
            // Payload generation stays outside the critical section so the
            // lock covers map work only (comparable to the OCC arm, whose
            // internal writer mutex sees pre-built payloads).
            if r < ratio_read {
                let g = m.lock().expect("lock");
                if let Some((view, meta)) = g.get(k) {
                    sink ^= u64::from(view.as_bytes()[0]) ^ u64::from(meta);
                }
                read_ops += 1;
            } else {
                if k & 1 == 0 {
                    blob_payload(k, &mut buf);
                    let mut g = m.lock().expect("lock");
                    let _ = g.insert(k, &buf, k as u32 & 0xFF_FFFF);
                } else {
                    let mut g = m.lock().expect("lock");
                    g.remove(k);
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
}

type BlobBTree = std::collections::BTreeMap<u64, (Vec<u8>, u32)>;

fn bench_blob_rwlock_btree(ratio_read: u32, plan: &Plan) -> Vec<Sample> {
    let m: Arc<RwLock<BlobBTree>> = Arc::new(RwLock::new(BlobBTree::new()));
    let mut rng = XorShift(0x5CA1_AB1E);
    let mut buf = [0u8; BLOB_LEN];
    {
        let mut g = m.write().expect("lock");
        for _ in 0..BLOB_POP {
            let k = rng.next() % BLOB_KEYSPACE;
            blob_payload(k, &mut buf);
            g.insert(k, (buf.to_vec(), k as u32 & 0xFF_FFFF));
        }
    }
    run_rounds(plan, move |i, win| {
        let mut rng = XorShift(0x1000 + i as u64);
        let mut buf = [0u8; BLOB_LEN];
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        win.begin();
        while win.running() {
            let r = (rng.next() % 100) as u32;
            let k = rng.next() % BLOB_KEYSPACE;
            if r < ratio_read {
                let g = m.read().expect("lock");
                if let Some((bytes, meta)) = g.get(&k) {
                    sink ^= u64::from(bytes[0]) ^ u64::from(*meta);
                }
                read_ops += 1;
            } else {
                // Payload generation and Vec construction stay outside the
                // write lock (see the Mutex arm).
                if k & 1 == 0 {
                    blob_payload(k, &mut buf);
                    let v = buf.to_vec();
                    let mut g = m.write().expect("lock");
                    g.insert(k, (v, k as u32 & 0xFF_FFFF));
                } else {
                    let mut g = m.write().expect("lock");
                    g.remove(&k);
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
}

fn bench_blob_skiplist(ratio_read: u32, plan: &Plan) -> Vec<Sample> {
    let m: Arc<SkipMap<u64, (Vec<u8>, u32)>> = Arc::new(SkipMap::new());
    let mut rng = XorShift(0x5CA1_AB1E);
    let mut buf = [0u8; BLOB_LEN];
    for _ in 0..BLOB_POP {
        let k = rng.next() % BLOB_KEYSPACE;
        blob_payload(k, &mut buf);
        m.insert(k, (buf.to_vec(), k as u32 & 0xFF_FFFF));
    }
    run_rounds(plan, move |i, win| {
        let mut rng = XorShift(0x1000 + i as u64);
        let mut buf = [0u8; BLOB_LEN];
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        win.begin();
        while win.running() {
            let r = (rng.next() % 100) as u32;
            let k = rng.next() % BLOB_KEYSPACE;
            if r < ratio_read {
                if let Some(e) = m.get(&k) {
                    let (bytes, meta) = e.value();
                    sink ^= u64::from(bytes[0]) ^ u64::from(*meta);
                }
                read_ops += 1;
            } else {
                if k & 1 == 0 {
                    blob_payload(k, &mut buf);
                    m.insert(k, (buf.to_vec(), k as u32 & 0xFF_FFFF));
                } else {
                    m.remove(&k);
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
}

/// String arm (issue #219 Phase 2): URL-route-shaped keys (~40 bytes, 5
/// sub-trie hops) matching the prefix-routing workload; the key universe is
/// pre-generated so per-op costs are map work, not string formatting.
const STR_POP: usize = 100_000;
const STR_KEYSPACE: usize = 2 * STR_POP;

fn str_keys() -> Arc<Vec<Vec<u8>>> {
    Arc::new(
        (0..STR_KEYSPACE)
            .map(|i| format!("/api/v2/tenants/{:06}/resources/{:04}", i / 16, i % 16).into_bytes())
            .collect(),
    )
}

fn bench_str_sync(ratio_read: u32, plan: &Plan) -> Vec<Sample> {
    let keys = str_keys();
    let m = Arc::new(SyncExpanseStrMap::new());
    let mut rng = XorShift(0x5CA1_AB1E);
    for _ in 0..STR_POP {
        let k = &keys[(rng.next() as usize) % STR_KEYSPACE];
        m.insert(tk(k), rng.next());
    }
    run_rounds(plan, move |i, win| {
        let rd = m.reader();
        let mut rng = XorShift(0x1000 + i as u64);
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        win.begin();
        while win.running() {
            let r = (rng.next() % 100) as u32;
            let k = &keys[(rng.next() as usize) % STR_KEYSPACE];
            if r < ratio_read {
                sink ^= rd.get(tk(k)).unwrap_or(0);
                read_ops += 1;
            } else {
                if rng.next() & 1 == 0 {
                    m.insert(tk(k), rng.next());
                } else {
                    m.remove(tk(k));
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
}

fn bench_str_mutex(ratio_read: u32, plan: &Plan) -> Vec<Sample> {
    let keys = str_keys();
    let m = Arc::new(std::sync::Mutex::new(ExpanseStrMap::new()));
    let mut rng = XorShift(0x5CA1_AB1E);
    {
        let mut g = m.lock().expect("lock");
        for _ in 0..STR_POP {
            let k = &keys[(rng.next() as usize) % STR_KEYSPACE];
            g.insert(tk(k), rng.next());
        }
    }
    run_rounds(plan, move |i, win| {
        let mut rng = XorShift(0x1000 + i as u64);
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        win.begin();
        while win.running() {
            let r = (rng.next() % 100) as u32;
            let k = &keys[(rng.next() as usize) % STR_KEYSPACE];
            let mut g = m.lock().expect("lock");
            if r < ratio_read {
                sink ^= g.get(tk(k)).unwrap_or(0);
                read_ops += 1;
            } else {
                if rng.next() & 1 == 0 {
                    g.insert(tk(k), rng.next());
                } else {
                    g.remove(tk(k));
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
}

/// Bytes arm (issue #362): the same workload as the string and DashMap
/// arms — identical keys, distribution, and op mix — over the unordered
/// hash-keyed wrapper.
fn bench_bytes_sync(ratio_read: u32, plan: &Plan) -> Vec<Sample> {
    let keys = str_keys();
    let m = Arc::new(SyncExpanseBytesMap::new());
    let mut rng = XorShift(0x5CA1_AB1E);
    for _ in 0..STR_POP {
        let k = &keys[(rng.next() as usize) % STR_KEYSPACE];
        m.insert(k, rng.next());
    }
    run_rounds(plan, move |i, win| {
        let rd = m.reader();
        let mut rng = XorShift(0x1000 + i as u64);
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        win.begin();
        while win.running() {
            let r = (rng.next() % 100) as u32;
            let k = &keys[(rng.next() as usize) % STR_KEYSPACE];
            if r < ratio_read {
                sink ^= rd.get(k).unwrap_or(0);
                read_ops += 1;
            } else {
                if rng.next() & 1 == 0 {
                    m.insert(k, rng.next());
                } else {
                    m.remove(k);
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
}

fn bench_bytes_mutex(ratio_read: u32, plan: &Plan) -> Vec<Sample> {
    let keys = str_keys();
    let m = Arc::new(std::sync::Mutex::new(ExpanseBytesMap::new()));
    let mut rng = XorShift(0x5CA1_AB1E);
    {
        let mut g = m.lock().expect("lock");
        for _ in 0..STR_POP {
            let k = &keys[(rng.next() as usize) % STR_KEYSPACE];
            g.insert(k, rng.next());
        }
    }
    run_rounds(plan, move |i, win| {
        let mut rng = XorShift(0x1000 + i as u64);
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        win.begin();
        while win.running() {
            let r = (rng.next() % 100) as u32;
            let k = &keys[(rng.next() as usize) % STR_KEYSPACE];
            let mut g = m.lock().expect("lock");
            if r < ratio_read {
                sink ^= g.get(k).unwrap_or(0);
                read_ops += 1;
            } else {
                if rng.next() & 1 == 0 {
                    g.insert(k, rng.next());
                } else {
                    g.remove(k);
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
}

fn bench_str_dashmap(ratio_read: u32, plan: &Plan) -> Vec<Sample> {
    let keys = str_keys();
    let m: Arc<DashMap<Vec<u8>, u64>> = Arc::new(DashMap::new());
    let mut rng = XorShift(0x5CA1_AB1E);
    for _ in 0..STR_POP {
        let k = &keys[(rng.next() as usize) % STR_KEYSPACE];
        m.insert(k.clone(), rng.next());
    }
    run_rounds(plan, move |i, win| {
        let mut rng = XorShift(0x1000 + i as u64);
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        win.begin();
        while win.running() {
            let r = (rng.next() % 100) as u32;
            let k = &keys[(rng.next() as usize) % STR_KEYSPACE];
            if r < ratio_read {
                sink ^= m.get(k.as_slice()).map_or(0, |e| *e.value());
                read_ops += 1;
            } else {
                if rng.next() & 1 == 0 {
                    m.insert(k.clone(), rng.next());
                } else {
                    m.remove(k.as_slice());
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
}

/// Parses a comma-separated list from the environment variable `name`,
/// falling back to `default` when unset. CI uses these knobs
/// (`EXPANSE_BENCH_THREADS`, `EXPANSE_BENCH_WORKLOADS`) to bound the sweep;
/// local runs with the variables unset keep the full default sweep.
/// Prefill of the `sync32` arm: this many draws over a 2× keyspace, the
/// keyspace every reader probes.
const S32_STABLE: u32 = 4_096;
/// Churn keys are the upper half of that keyspace, where prefill draws also
/// land; the writer inserts and removes them continuously, so reader probes
/// race a live write bracket.
const S32_CHURN: u32 = 4_096;
/// Fixed arena for the `sync32` wrapper: comfortably above the node count
/// an 8k-key `ExpanseMap32` reaches, so `ArenaFull` only ever signals a
/// reclamation stall (which the `refused` column then reports).
const S32_NODE_CAP: usize = 16_384;
const S32_MAX_READERS: usize = 16;

/// One writer on the churn range — at full duty when `write_rate` is
/// `None`, otherwise paced to that many mutations per second by spinning
/// on a deadline — and `threads` readers over the whole keyspace, over the rounds
/// of `plan`. `threads` is the table's `threads` column; the writer is an
/// extra thread. Reader throughput counts only reads that validated; `busy`
/// counts the attempts abandoned to an open write bracket. The readers are
/// taken from the pool once and lent to each window.
fn bench_sync32_map(plan: &Plan, write_rate: Option<u64>) -> Vec<Sample> {
    let max_readers = plan.threads.iter().copied().max().unwrap_or(1);
    assert!(
        max_readers <= S32_MAX_READERS,
        "sync32 arm supports at most {S32_MAX_READERS} readers"
    );
    let mut m = SyncExpanseMap32::with_capacity(S32_NODE_CAP, S32_MAX_READERS);
    let (mut w, mut pool) = m.split();
    let mut rng = XorShift(0x5CA1_AB1E);
    for _ in 0..S32_STABLE {
        let k = (rng.next() % (2 * u64::from(S32_STABLE))) as u32;
        w.try_insert(k, !k).expect("prefill");
    }
    let mut readers: Vec<_> = (0..max_readers)
        .map(|_| pool.take().expect("reader slot"))
        .collect();

    let mut samples = Vec::with_capacity(plan.rounds * plan.threads.len());
    for round in 0..plan.rounds {
        for (position, idx) in williams_order(plan.threads.len(), round)
            .into_iter()
            .enumerate()
        {
            let threads = plan.threads[idx];
            // Readers, the writer and the driver.
            let window = Window::new(threads + 2);
            let (busy_total, ok_total, refused_total, write_total) = (
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            );
            let mut elapsed_s = 0.0;
            std::thread::scope(|s| {
                for (i, r) in readers[..threads].iter_mut().enumerate() {
                    let window = &window;
                    let (busy_total, ok_total) = (&busy_total, &ok_total);
                    s.spawn(move || {
                        let mut rng = XorShift(0x1000 + i as u64);
                        let (mut ok, mut busy) = (0u64, 0u64);
                        let mut sink = 0u32;
                        window.begin();
                        while window.running() {
                            let k = (rng.next() % (2 * u64::from(S32_STABLE))) as u32;
                            match r.try_get(k) {
                                Ok(v) => {
                                    sink ^= v.unwrap_or(0);
                                    ok += 1;
                                }
                                Err(Busy) => busy += 1,
                            }
                        }
                        std::hint::black_box(sink);
                        ok_total.fetch_add(ok, Ordering::Relaxed);
                        busy_total.fetch_add(busy, Ordering::Relaxed);
                    });
                }
                {
                    let window = &window;
                    let w = &mut w;
                    let (refused_total, write_total) = (&refused_total, &write_total);
                    s.spawn(move || {
                        let mut rng = XorShift(0x5EED_5EED);
                        let (mut writes, mut refused) = (0u64, 0u64);
                        let period = write_rate.map(|r| Duration::from_secs_f64(1.0 / r as f64));
                        window.begin();
                        let start = Instant::now();
                        while window.running() {
                            if let Some(period) = period {
                                // Deadline pacing: mutation `n` may not start before
                                // `start + n * period`, so the rate holds without
                                // sleeping (whose granularity is coarser than the
                                // 1 µs period of the 1M/s row).
                                let deadline = start + period * (writes + refused) as u32;
                                while Instant::now() < deadline {
                                    if !window.running() {
                                        break;
                                    }
                                    std::hint::spin_loop();
                                }
                            }
                            let k = S32_STABLE + (rng.next() % u64::from(S32_CHURN)) as u32;
                            let res = if writes % 3 == 2 {
                                w.try_remove(k).map(|_| ())
                            } else {
                                w.try_insert(k, k).map(|_| ())
                            };
                            match res {
                                Ok(()) => writes += 1,
                                Err(WriteError::ArenaFull | WriteError::ReclaimBacklog) => {
                                    refused += 1;
                                    // A stalled reader blocks reclamation; retrying
                                    // is the documented response, not spinning.
                                    w.try_reclaim();
                                    std::thread::yield_now();
                                }
                            }
                        }
                        write_total.fetch_add(writes, Ordering::Relaxed);
                        refused_total.fetch_add(refused, Ordering::Relaxed);
                    });
                }
                window.begin();
                let t0 = Instant::now();
                std::thread::sleep(WINDOW);
                window.stop.store(true, Ordering::Relaxed);
                elapsed_s = t0.elapsed().as_secs_f64();
            });
            let ok = ok_total.load(Ordering::Relaxed);
            samples.push(Sample {
                threads,
                round,
                position,
                elapsed_s,
                read_ops: ok,
                write_ops: write_total.load(Ordering::Relaxed),
                busy: busy_total.load(Ordering::Relaxed),
                ok,
                refused: refused_total.load(Ordering::Relaxed),
                #[cfg(feature = "occ-stats")]
                stats: expanse_trie::occ_stats::snapshot(),
            });
        }
    }
    samples
}

fn env_csv<T>(name: &str, default: &[T]) -> Vec<T>
where
    T: Copy + std::str::FromStr,
{
    match std::env::var(name) {
        Ok(s) => {
            let parsed: Vec<T> = s
                .split(',')
                .map(|p| {
                    p.trim()
                        .parse::<T>()
                        .unwrap_or_else(|_| panic!("invalid {name} entry: {p:?}"))
                })
                .collect();
            assert!(!parsed.is_empty(), "{name} must not be empty");
            parsed
        }
        Err(_) => default.to_vec(),
    }
}

/// A `run_rounds` arm: `(read_percentage, plan)`.
type EngineBench = fn(u32, &Plan) -> Vec<Sample>;

/// The `run_rounds` arms, by the key `EXPANSE_BENCH_ENGINES` selects them with.
const ENGINES: [(&str, &str, EngineBench); 11] = [
    ("map", "SyncExpanseMap", bench_map),
    ("set", "SyncExpanseSet", bench_set),
    ("blob", "SyncExpanseBlobMap", bench_blob_sync),
    ("blob_mutex", "Mutex<ExpanseBlobMap>", bench_blob_mutex),
    (
        "blob_rwlock_btree",
        "RwLock<BTreeMap<u64, (Vec<u8>, u32)>>",
        bench_blob_rwlock_btree,
    ),
    (
        "blob_skiplist",
        "SkipMap<u64, Vec<u8>>",
        bench_blob_skiplist,
    ),
    ("str", "SyncExpanseStrMap", bench_str_sync),
    ("str_mutex", "Mutex<ExpanseStrMap>", bench_str_mutex),
    ("bytes", "SyncExpanseBytesMap", bench_bytes_sync),
    ("bytes_mutex", "Mutex<ExpanseBytesMap>", bench_bytes_mutex),
    ("str_dashmap", "DashMap<Vec<u8>, u64>", bench_str_dashmap),
];

/// The `sync32` arm's key; it runs a writer-duty sweep, not the workloads.
const SYNC32_KEY: &str = "sync32";

/// The arm keys `EXPANSE_BENCH_ENGINES` selects, or every arm when unset. An
/// unknown key fails loudly rather than silently measuring nothing.
fn selected_engines() -> Vec<String> {
    let known: Vec<&str> = ENGINES
        .iter()
        .map(|e| e.0)
        .chain(std::iter::once(SYNC32_KEY))
        .collect();
    let Ok(list) = std::env::var("EXPANSE_BENCH_ENGINES") else {
        return known.iter().map(|k| k.to_string()).collect();
    };
    let picked: Vec<String> = list
        .split(',')
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .collect();
    assert!(
        !picked.is_empty(),
        "EXPANSE_BENCH_ENGINES must not be empty"
    );
    for k in &picked {
        assert!(
            known.contains(&k.as_str()),
            "unknown EXPANSE_BENCH_ENGINES entry {k:?}; known: {}",
            known.join(",")
        );
    }
    picked
}

/// The file `EXPANSE_BENCH_SAMPLES` names, opened for append, if set.
fn samples_writer() -> Option<std::io::BufWriter<std::fs::File>> {
    let path = std::env::var_os("EXPANSE_BENCH_SAMPLES")?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap_or_else(|e| {
            panic!(
                "EXPANSE_BENCH_SAMPLES {}: {e}",
                std::path::Path::new(&path).display()
            )
        });
    Some(std::io::BufWriter::new(file))
}

/// The labels one table and its samples carry.
struct Cell<'a> {
    engine_key: &'a str,
    engine: &'a str,
    workload: &'a str,
    read_pct: Option<u32>,
    write_rate: Option<u64>,
}

/// Appends every window of `cell` to the samples file as one JSON line.
fn write_samples(
    out: Option<&mut std::io::BufWriter<std::fs::File>>,
    cell: &Cell<'_>,
    samples: &[Sample],
) {
    let Some(out) = out else { return };
    for s in samples {
        let row = json!({
            "workload_id": "core_concurrency",
            "engine_key": cell.engine_key,
            "engine": cell.engine,
            "workload": cell.workload,
            "read_pct": cell.read_pct,
            "write_rate": cell.write_rate,
            "threads": s.threads,
            "round": s.round,
            "position": s.position,
            "elapsed_s": s.elapsed_s,
            "read_ops": s.read_ops,
            "write_ops": s.write_ops,
            "busy": s.busy,
            "ok": s.ok,
            "refused": s.refused,
        });
        writeln!(out, "{row}").expect("write EXPANSE_BENCH_SAMPLES");
    }
    out.flush().expect("flush EXPANSE_BENCH_SAMPLES");
}

/// The counters this census prints, as (label, `Stat`) pairs. Restarts,
/// fallbacks and the two waits are three different diseases (Refs #1047):
/// a restart storm is optimistic writers colliding, a fallback storm is
/// optimistic writers giving up, and gate/quiesce waits are the serialised
/// sections excluding each other and the optimistic writers alike.
#[cfg(feature = "occ-stats")]
const CENSUS: &[(&str, expanse_trie::occ_stats::Stat)] = {
    use expanse_trie::occ_stats::Stat;
    &[
        ("inserts", Stat::Inserts),
        ("write_ops", Stat::WriteOps),
        ("lock_restarts", Stat::LockRestarts),
        ("lock_fallbacks", Stat::LockFallbacks),
        ("fb_contention", Stat::FallbackContention),
        ("fb_root_growth", Stat::FallbackRootGrowth),
        ("fb_cap_expansion", Stat::FallbackCapExpansion),
        ("fb_branch_split", Stat::FallbackBranchSplit),
        ("fb_immediate", Stat::FallbackImmediateConversion),
        ("fb_unknown_tag", Stat::FallbackUnknownTag),
        ("gate_closed", Stat::ContentionGateClosed),
        ("retry_exhausted", Stat::ContentionRetryExhausted),
        ("gate_blocked", Stat::GateBlockedEntries),
        ("quiesce_calls", Stat::QuiesceCalls),
        ("read_ops", Stat::ReadOps),
        ("read_attempts", Stat::ReadAttempts),
        ("read_fallbacks", Stat::ReadFallbacks),
        ("locked_reads", Stat::LockedReads),
        ("retired", Stat::Retired),
        ("freed_raw", Stat::FreedRaw),
        ("sample_spins", Stat::SampleSpins),
    ]
};

/// The cycle-counter totals, printed as seconds per op: a spin count says
/// how often, only the ticks say how long (AGENTS.md §8.20.1 — these are
/// constant-rate ticks, divided by `cycles_hz`, never by a core clock).
#[cfg(feature = "occ-stats")]
const CENSUS_CYCLES: &[(&str, expanse_trie::occ_stats::Stat)] = {
    use expanse_trie::occ_stats::Stat;
    &[
        ("gate_wait_s/op", Stat::GateWaitCycles),
        ("quiesce_drain_s/op", Stat::QuiesceDrainCycles),
        ("lock_hold_s/op", Stat::LockHoldCycles),
        ("sample_spin_s/op", Stat::SampleSpinCycles),
    ]
};

#[cfg(feature = "occ-stats")]
fn read_ops_sum(windows: &[&Sample]) -> u64 {
    windows.iter().map(|s| s.read_ops).sum()
}

#[cfg(feature = "occ-stats")]
fn write_ops_sum(windows: &[&Sample]) -> u64 {
    windows.iter().map(|s| s.write_ops).sum()
}

/// Prints the per-op protocol census of one thread count's windows. Rates
/// divide by the cell's own op count, so a row is comparable across thread
/// counts; the raw totals are printed beside them because a rate of 0.000
/// and a count of 0 are different findings.
#[cfg(feature = "occ-stats")]
fn print_counter_census(windows: &[&Sample], ops: u64) {
    assert!(
        expanse_trie::occ_stats::enabled(),
        "occ-stats census reached with the counters compiled out"
    );
    if ops == 0 {
        println!("         census: 0 ops in this cell — nothing to divide by");
        return;
    }
    let total = |st: expanse_trie::occ_stats::Stat| -> u64 {
        windows
            .iter()
            .map(|s| s.stats[st as usize])
            .fold(0u64, u64::wrapping_add)
    };
    let d = ops as f64;
    let mut line = String::new();
    for (label, st) in CENSUS {
        let v = total(*st);
        line.push_str(&format!(" {label}={:.4}({v})", v as f64 / d));
    }
    println!("        census/op:{line}");
    let hz = expanse_trie::occ_stats::cycles_hz(Duration::from_millis(50)) as f64;
    let mut cyc = String::new();
    for (label, st) in CENSUS_CYCLES {
        let v = total(*st);
        let secs = if hz > 0.0 {
            v as f64 / hz / d
        } else {
            f64::NAN
        };
        cyc.push_str(&format!(" {label}={secs:.3e}"));
    }
    println!("        census/op:{cyc}  (cycles_hz={hz:.0})");
}

/// Prints one table in the four-column shape `scripts/bench_concurrency_check.py`
/// parses: per thread count, the mean over its windows of read and write
/// ops/sec, and total ops/sec relative to the first thread count. The `sync32`
/// arm adds its protocol-health line, which the parser skips.
fn print_table(cell: &Cell<'_>, plan: &Plan, samples: &[Sample]) {
    println!("\n=== {} ({}) ===", cell.engine, cell.workload);
    println!(
        "{:>8} {:>16} {:>16} {:>10}",
        "threads", "read ops/sec", "write ops/sec", "scale"
    );
    let mut base: Option<f64> = None;
    for &t in &plan.threads {
        let windows: Vec<&Sample> = samples.iter().filter(|s| s.threads == t).collect();
        if windows.is_empty() {
            continue;
        }
        let n = windows.len() as f64;
        let rops = windows
            .iter()
            .map(|s| s.read_ops as f64 / s.elapsed_s)
            .sum::<f64>()
            / n;
        let wops = windows
            .iter()
            .map(|s| s.write_ops as f64 / s.elapsed_s)
            .sum::<f64>()
            / n;
        let total = rops + wops;
        let base = *base.get_or_insert(total);
        println!(
            "{:>8} {:>16.0} {:>16.0} {:>9.2}x",
            t,
            rops,
            wops,
            total / base
        );
        #[cfg(feature = "occ-stats")]
        print_counter_census(&windows, read_ops_sum(&windows) + write_ops_sum(&windows));
        if cell.engine_key == SYNC32_KEY {
            let busy: u64 = windows.iter().map(|s| s.busy).sum();
            let ok: u64 = windows.iter().map(|s| s.ok).sum();
            let refused: u64 = windows.iter().map(|s| s.refused).sum();
            let attempts = ok + busy;
            let busy_pct = if attempts == 0 {
                0.0
            } else {
                busy as f64 * 100.0 / attempts as f64
            };
            println!(
                "         busy {:>12} of {:>12} attempts ({:>6.3}%)   refused writes {}",
                busy, attempts, busy_pct, refused
            );
        }
    }
}

fn main() {
    let max_threads = std::thread::available_parallelism()
        .map_or(16, usize::from)
        .min(16);
    // Thread counts, workload read-percentages and rounds; overridable via
    // EXPANSE_BENCH_THREADS="1,4,16" / EXPANSE_BENCH_WORKLOADS="100,50" /
    // EXPANSE_BENCH_ROUNDS=18.
    let threads: Vec<usize> = env_csv("EXPANSE_BENCH_THREADS", &[1usize, 2, 4, 8, 16])
        .into_iter()
        .filter(|&t| t == 1 || t <= max_threads)
        .collect();
    let rounds = env_csv("EXPANSE_BENCH_ROUNDS", &[3usize]);
    assert!(
        rounds.len() == 1 && rounds[0] >= 1,
        "EXPANSE_BENCH_ROUNDS is one positive round count"
    );
    let plan = Plan {
        threads,
        rounds: rounds[0],
    };
    let read_pcts = env_csv("EXPANSE_BENCH_WORKLOADS", &[100u32, 95, 50]);
    for &rr in &read_pcts {
        assert!(
            rr <= 100,
            "EXPANSE_BENCH_WORKLOADS entries are read percentages (0-100)"
        );
    }
    let selected = selected_engines();
    let mut samples_out = samples_writer();

    for (key, name, bench) in ENGINES {
        if !selected.iter().any(|k| k == key) {
            continue;
        }
        for &rr in &read_pcts {
            let workload = format!("{rr}% Read / {}% Write", 100 - rr);
            let cell = Cell {
                engine_key: key,
                engine: name,
                workload: &workload,
                read_pct: Some(rr),
                write_rate: None,
            };
            let samples = bench(rr, &plan);
            print_table(&cell, &plan, &samples);
            write_samples(samples_out.as_mut(), &cell, &samples);
        }
    }

    // sync32 arms (issue #573): the roles are fixed by the protocol — one
    // writer, `threads` readers — so the read-percentage sweep does not
    // apply; the sweep here is over the writer's duty instead. Rows keep
    // the parser-compatible four-column shape (`bench_concurrency_check.py`,
    // one table per duty); the `busy`/`refused` line that follows each row
    // is the protocol-health telemetry and is not parsed.
    if selected.iter().any(|k| k == SYNC32_KEY) {
        let s32_plan = Plan {
            threads: plan
                .threads
                .iter()
                .copied()
                .filter(|&t| t <= S32_MAX_READERS)
                .collect(),
            rounds: plan.rounds,
        };
        let duties: [(Option<u64>, &str); 4] = [
            (None, "writer full duty / N readers try_get"),
            (Some(1_000_000), "writer 1M/s / N readers try_get"),
            (Some(100_000), "writer 100k/s / N readers try_get"),
            (Some(10_000), "writer 10k/s / N readers try_get"),
        ];
        for (rate, label) in duties {
            let cell = Cell {
                engine_key: SYNC32_KEY,
                engine: "SyncExpanseMap32",
                workload: label,
                read_pct: None,
                write_rate: rate,
            };
            let samples = bench_sync32_map(&s32_plan, rate);
            print_table(&cell, &s32_plan, &samples);
            write_samples(samples_out.as_mut(), &cell, &samples);
        }
    }
}
