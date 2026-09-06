//! Masstree arm, concurrent cells (#661, METHODOLOGY §5): writer throughput as
//! writer count scales, reader throughput alongside, and the Expanse
//! protocol's health under that write load — Masstree against `SyncExpanseMap`
//! (MC1, `u64` keys) and `SyncExpanseStrMap` (MC2, `short` string keys). The
//! cells are those of `hot_comparison` §11.4, so the two routes to the
//! write-concurrency loss read side by side.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `masstree_concurrent` |
//! | `group` | 5 |
//! | `population` | prefill 2^20 keys (uniform random u64 at 64 bits, or `short` strings), plus 2^20 fresh keys inserted concurrently by W writers |
//! | `probes_and_reuse` | R readers cycle a shuffled 2^20-probe stream against the prefill until the writers finish; at W = 0 each reader makes exactly one pass |
//! | `hit_rate` | 50% against the prefill; some misses become hits as writers land, identically on both arms |
//! | `miss_gen_method` | same-generator rejection sampling (§8.6); fresh writer keys rejected on prefill membership |
//! | `value_dereference` | both sides fetch the stored value and check it against its key-derived expectation |
//! | `measured_region` | barrier release to last-writer join (writers) and to last-reader join (readers); prefill, teardown and walks outside; Masstree's per-64-op `quiesce` inside its threads (§3.2) |
//! | `arm_symmetry` | identical prefill, probe and fresh-key streams; both arms below any external lock through their native concurrent APIs (§8.16); one thread slot per Masstree thread; same ISA target; W + R ≤ 16 inside the P-core pin |
//! | `statistics` | per-round throughput emitted raw, arms interleaved per round; BCa 95% CIs on the Expanse ÷ Masstree ratio computed by the runner (§8.4); `--health` emits event ratios from a diagnostic build and never a timing |
//! | `verdict` | pending measurement |
//!
//! ## Fixed work, not a fixed window
//!
//! Each writer inserts its whole slice of the fresh-key stream; the timed
//! region ends when the last writer joins, so both arms do identical work per
//! round and grow by exactly the same population.
//!
//! ## One cell per invocation; two builds, never one
//!
//! Masstree's pools and limbo lists are per slot and outlive every table
//! (§3.6); the runner drives the sweep. `--health` requires the `occ-stats`
//! feature and refuses to run without it; a throughput cell refuses to run
//! with it (the counters are diagnostic-only in the engine).

use std::env;
use std::hint::black_box;
use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use expanse_hot_bench::masstree::{Masstree, MtThread, QUIESCE_EVERY, StrInsert, Table};
use expanse_hot_bench::strings::{self, KeyStr, StrDist};
use expanse_hot_bench::workload;
use expanse_trie::occ_stats;
use expanse_trie::sync::{SyncExpanseMap, SyncExpanseStrMap};

const N_PREFILL: usize = 1 << 20;
const M_NEW: usize = 1 << 20;
const ROUNDS: usize = 15;
const HEALTH_ROUNDS: usize = 5;
/// The P-core pin on the reference host is 16 logical CPUs.
const MAX_THREADS: usize = 16;
const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;
/// Writer `w` takes slot `w`; reader `r` takes slot `16 + r`; the prefilling
/// and verifying thread takes slot 32 (§3.2).
const READER_SLOT_BASE: u32 = 16;
const MAIN_SLOT: u32 = 32;

/// A key with a deterministic stored value, so readers verify every hit.
trait KeyLike: Sync + Send {
    fn value(&self) -> u64;
}
impl KeyLike for u64 {
    #[inline]
    fn value(&self) -> u64 {
        self.wrapping_mul(GOLDEN)
    }
}
impl KeyLike for KeyStr {
    #[inline]
    fn value(&self) -> u64 {
        // FNV-1a over the bytes, then the golden multiply — a function of the
        // key's content, not its allocation, since probes are fresh copies.
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for b in self.bytes() {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h.wrapping_mul(GOLDEN)
    }
}

/// Prefill, probes and fresh keys, from either shared generator.
struct Stream<K> {
    prefill: Vec<K>,
    probes: Vec<K>,
    probe_is_prefill: Vec<bool>,
    new_keys: Vec<K>,
}

/// One side of a pairing, driven identically by [`run_round`].
trait ConcArm<K: KeyLike>: Sync {
    /// Per-thread handle: a Masstree slot, or nothing.
    type Ctx: Copy + Send;
    /// Per-thread reader handle (Expanse registers one epoch slot per reader).
    type Reader<'a>
    where
        Self: 'a;
    fn ctx(slot: u32) -> Self::Ctx;
    fn begin(c: Self::Ctx);
    fn end(c: Self::Ctx);
    /// Housekeeping after the `n`-th operation on this thread.
    fn tick(c: Self::Ctx, n: u64);
    fn reader(&self) -> Self::Reader<'_>;
    fn insert(&self, c: Self::Ctx, k: &K);
    /// `Some(value_ok)` when found, `None` when absent.
    fn probe(r: &Self::Reader<'_>, c: Self::Ctx, k: &K) -> Option<bool>;
    /// Population, quiescent-only.
    fn len(&self, c: Self::Ctx) -> usize;
}

impl ConcArm<u64> for Masstree {
    type Ctx = MtThread;
    type Reader<'a> = &'a Masstree;
    fn ctx(slot: u32) -> MtThread {
        MtThread::slot(slot)
    }
    fn begin(c: MtThread) {
        c.enter();
    }
    fn end(c: MtThread) {
        c.exit();
    }
    #[inline]
    fn tick(c: MtThread, n: u64) {
        if n.is_multiple_of(QUIESCE_EVERY) {
            c.quiesce();
        }
    }
    fn reader(&self) -> &Masstree {
        self
    }
    #[inline]
    fn insert(&self, c: MtThread, k: &u64) {
        Masstree::insert(self, c, *k, k.value());
    }
    #[inline]
    fn probe(r: &&Masstree, c: MtThread, k: &u64) -> Option<bool> {
        r.get(c, *k).map(|v| v == k.value())
    }
    fn len(&self, c: MtThread) -> usize {
        Masstree::len(self, c)
    }
}

impl ConcArm<KeyStr> for Masstree {
    type Ctx = MtThread;
    type Reader<'a> = &'a Masstree;
    fn ctx(slot: u32) -> MtThread {
        MtThread::slot(slot)
    }
    fn begin(c: MtThread) {
        c.enter();
    }
    fn end(c: MtThread) {
        c.exit();
    }
    #[inline]
    fn tick(c: MtThread, n: u64) {
        if n.is_multiple_of(QUIESCE_EVERY) {
            c.quiesce();
        }
    }
    fn reader(&self) -> &Masstree {
        self
    }
    #[inline]
    fn insert(&self, c: MtThread, k: &KeyStr) {
        if Masstree::str_insert(self, c, k.bytes(), k.value()) == StrInsert::NotRepresentable {
            eprintln!("a key beyond the predicate reached the Masstree side; cell is void (§9)");
            std::process::exit(1);
        }
    }
    #[inline]
    fn probe(r: &&Masstree, c: MtThread, k: &KeyStr) -> Option<bool> {
        r.str_get(c, k.bytes()).map(|v| v == k.value())
    }
    fn len(&self, c: MtThread) -> usize {
        Masstree::len(self, c)
    }
}

impl ConcArm<u64> for SyncExpanseMap {
    type Ctx = ();
    type Reader<'a> = expanse_trie::sync::MapReader<'a>;
    fn ctx(_: u32) {}
    fn begin(_: ()) {}
    fn end(_: ()) {}
    #[inline]
    fn tick(_: (), _: u64) {}
    fn reader(&self) -> Self::Reader<'_> {
        SyncExpanseMap::reader(self)
    }
    #[inline]
    fn insert(&self, _: (), k: &u64) {
        SyncExpanseMap::insert(self, *k, k.value());
    }
    #[inline]
    fn probe(r: &Self::Reader<'_>, _: (), k: &u64) -> Option<bool> {
        r.get(*k).map(|v| v == k.value())
    }
    fn len(&self, _: ()) -> usize {
        SyncExpanseMap::len(self) as usize
    }
}

impl ConcArm<KeyStr> for SyncExpanseStrMap {
    type Ctx = ();
    type Reader<'a> = expanse_trie::sync::StrReader<'a>;
    fn ctx(_: u32) {}
    fn begin(_: ()) {}
    fn end(_: ()) {}
    #[inline]
    fn tick(_: (), _: u64) {}
    fn reader(&self) -> Self::Reader<'_> {
        SyncExpanseStrMap::reader(self)
    }
    #[inline]
    fn insert(&self, _: (), k: &KeyStr) {
        SyncExpanseStrMap::insert(self, k.bytes(), k.value());
    }
    #[inline]
    fn probe(r: &Self::Reader<'_>, _: (), k: &KeyStr) -> Option<bool> {
        r.get(k.bytes()).map(|v| v == k.value())
    }
    fn len(&self, _: ()) -> usize {
        SyncExpanseStrMap::len(self) as usize
    }
}

struct RoundResult {
    writer_elapsed: Option<Duration>,
    reader_elapsed: Option<Duration>,
    reads: u64,
    errors: u64,
    population: usize,
}

fn run_round<K: KeyLike, A: ConcArm<K>>(
    arm: &A,
    s: &Stream<K>,
    writers: usize,
    readers: usize,
    prefill: bool,
) -> RoundResult {
    if prefill {
        let c = A::ctx(MAIN_SLOT);
        A::begin(c);
        for (i, k) in s.prefill.iter().enumerate() {
            arm.insert(c, k);
            A::tick(c, i as u64 + 1);
        }
        A::end(c);
    }

    let stop = AtomicBool::new(false);
    let barrier = Barrier::new(writers + readers + 1);
    let reads_total = AtomicU64::new(0);
    let errors_total = AtomicU64::new(0);

    let (writer_elapsed, reader_elapsed) = std::thread::scope(|sc| {
        let mut wh = Vec::with_capacity(writers);
        let per = s.new_keys.len() / writers.max(1);
        for w in 0..writers {
            let lo = w * per;
            let hi = if w + 1 == writers {
                s.new_keys.len()
            } else {
                lo + per
            };
            let slice = &s.new_keys[lo..hi];
            let barrier = &barrier;
            wh.push(sc.spawn(move || {
                let c = A::ctx(w as u32);
                A::begin(c);
                barrier.wait();
                for (i, k) in slice.iter().enumerate() {
                    arm.insert(c, k);
                    A::tick(c, i as u64 + 1);
                }
                A::end(c);
            }));
        }
        let mut rh = Vec::with_capacity(readers);
        for r in 0..readers {
            let (barrier, stop) = (&barrier, &stop);
            let (reads_total, errors_total) = (&reads_total, &errors_total);
            rh.push(sc.spawn(move || {
                let c = A::ctx(READER_SLOT_BASE + r as u32);
                A::begin(c);
                let rd = arm.reader();
                let n = s.probes.len();
                let mut i = (r * n) / readers.max(1);
                let (mut reads, mut errors, mut sink) = (0u64, 0u64, 0u64);
                barrier.wait();
                if writers == 0 {
                    for _ in 0..n {
                        match A::probe(&rd, c, &s.probes[i]) {
                            Some(ok) => {
                                sink ^= 1;
                                errors += u64::from(!ok);
                            }
                            None => errors += u64::from(s.probe_is_prefill[i]),
                        }
                        reads += 1;
                        A::tick(c, reads);
                        i += 1;
                        if i == n {
                            i = 0;
                        }
                    }
                } else {
                    while !stop.load(Ordering::Relaxed) {
                        match A::probe(&rd, c, &s.probes[i]) {
                            Some(ok) => {
                                sink ^= 1;
                                errors += u64::from(!ok);
                            }
                            None => errors += u64::from(s.probe_is_prefill[i]),
                        }
                        reads += 1;
                        A::tick(c, reads);
                        i += 1;
                        if i == n {
                            i = 0;
                        }
                    }
                }
                black_box(sink);
                drop(rd);
                A::end(c);
                reads_total.fetch_add(reads, Ordering::Relaxed);
                errors_total.fetch_add(errors, Ordering::Relaxed);
            }));
        }

        barrier.wait();
        let t0 = Instant::now();
        for h in wh {
            h.join().expect("writer thread panicked");
        }
        let writer_elapsed = (writers > 0).then(|| t0.elapsed());
        stop.store(true, Ordering::Relaxed);
        for h in rh {
            h.join().expect("reader thread panicked");
        }
        let reader_elapsed = (readers > 0).then(|| t0.elapsed());
        (writer_elapsed, reader_elapsed)
    });

    let c = A::ctx(MAIN_SLOT);
    A::begin(c);
    let population = arm.len(c);
    A::end(c);
    RoundResult {
        writer_elapsed,
        reader_elapsed,
        reads: reads_total.load(Ordering::Relaxed),
        errors: errors_total.load(Ordering::Relaxed),
        population,
    }
}

fn mops(ops: u64, d: Option<Duration>) -> Option<f64> {
    d.map(|d| ops as f64 / d.as_secs_f64() / 1e6)
}

fn json_opt(v: Option<f64>) -> String {
    v.map_or_else(|| "null".to_string(), |x| format!("{x:.4}"))
}

fn cpus_allowed() -> String {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("Cpus_allowed_list:"))
                .map(|v| v.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn usage() -> ! {
    eprintln!("usage: masstree_concurrent <map|str> <writers> <readers> [--health]");
    eprintln!("  writers + readers <= {MAX_THREADS}; one cell per invocation (§3.6)");
    eprintln!("  --health needs the `occ-stats` feature and emits event ratios only");
    std::process::exit(2);
}

fn void_cell(round: usize, side: &str, r: &RoundResult, expected: usize) {
    if r.errors != 0 || r.population != expected {
        eprintln!(
            "round {round} {side}: {} reader error(s), population {} (intended {expected}); cell is void (§9)",
            r.errors, r.population
        );
        std::process::exit(1);
    }
}

/// Names a cell in every emitted row.
struct CellId<'a> {
    workload_id: &'a str,
    label: &'a str,
    dist: &'a str,
}

/// Runs the cell for one key type, generic over the two arms.
fn drive<K: KeyLike, M: ConcArm<K>, E: ConcArm<K>>(
    s: &Stream<K>,
    make_mt: impl Fn() -> M,
    make_exp: impl Fn() -> E,
    writers: usize,
    readers: usize,
    health: bool,
    id: &CellId<'_>,
) {
    let (workload_id, label, dist) = (id.workload_id, id.label, id.dist);
    let n0 = s.prefill.len();
    let m = s.new_keys.len() as u64;
    let expected = n0 + if writers > 0 { s.new_keys.len() } else { 0 };
    let cpus = cpus_allowed();
    let pin = env::var("EXPANSE_BENCH_PIN_APPLIED").unwrap_or_else(|_| "unset".to_string());

    if health {
        // Expanse side only: Masstree has no counterpart counter (§6.3).
        for round in 0..HEALTH_ROUNDS {
            let e = make_exp();
            {
                let c = E::ctx(MAIN_SLOT);
                for (i, k) in s.prefill.iter().enumerate() {
                    e.insert(c, k);
                    E::tick(c, i as u64 + 1);
                }
            }
            occ_stats::reset();
            let r = run_round(&e, s, writers, readers, false);
            let snap = occ_stats::snapshot();
            void_cell(round, "expanse", &r, expected);
            let st = |name: &str| snap[occ_stats::NAMES.iter().position(|n| *n == name).unwrap()];
            let (ops, attempts, fallbacks) =
                (st("read_ops"), st("read_attempts"), st("read_fallbacks"));
            let restart_share = if attempts == 0 {
                0.0
            } else {
                (attempts - ops) as f64 / attempts as f64
            };
            let fallback_share = if ops == 0 {
                0.0
            } else {
                fallbacks as f64 / ops as f64
            };
            println!(
                "{{\"workload_id\":\"{workload_id}\",\"role\":\"health\",\"arm\":\"{label}\",\"dist\":\"{dist}\",\
                 \"writers\":{writers},\"readers\":{readers},\"round\":{round},\"read_ops\":{ops},\
                 \"read_attempts\":{attempts},\"read_fallbacks\":{fallbacks},\"sample_spins\":{},\
                 \"write_ops\":{},\"locked_reads\":{},\"restart_share\":{restart_share:.6},\
                 \"fallback_share\":{fallback_share:.6},\"cpus_allowed\":\"{cpus}\",\"pin_applied\":\"{pin}\"}}",
                st("sample_spins"),
                st("write_ops"),
                st("locked_reads"),
            );
        }
        return;
    }

    for round in 0..ROUNDS {
        // Interleave the arms round by round (docs/BENCHMARKING.md rule 1).
        let mt_first = round % 2 == 0;
        let run_mt = || {
            let t = make_mt();
            let r = run_round(&t, s, writers, readers, true);
            void_cell(round, "masstree", &r, expected);
            r
        };
        let run_exp = || {
            let e = make_exp();
            let r = run_round(&e, s, writers, readers, true);
            void_cell(round, "expanse", &r, expected);
            r
        };
        let (mt, ex) = if mt_first {
            let a = run_mt();
            (a, run_exp())
        } else {
            let b = run_exp();
            (run_mt(), b)
        };
        println!(
            "{{\"workload_id\":\"{workload_id}\",\"role\":\"throughput\",\"arm\":\"{label}\",\"dist\":\"{dist}\",\
             \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{writers},\"readers\":{readers},\
             \"round\":{round},\"first\":\"{}\",\
             \"masstree_writer_mops\":{},\"expanse_writer_mops\":{},\
             \"masstree_reader_mops\":{},\"expanse_reader_mops\":{},\
             \"masstree_reads\":{},\"expanse_reads\":{},\"population_after\":{expected},\
             \"cpus_allowed\":\"{cpus}\",\"pin_applied\":\"{pin}\"}}",
            if mt_first { "masstree" } else { "expanse" },
            json_opt(mops(m, mt.writer_elapsed)),
            json_opt(mops(m, ex.writer_elapsed)),
            json_opt(mops(mt.reads, mt.reader_elapsed)),
            json_opt(mops(ex.reads, ex.reader_elapsed)),
            mt.reads,
            ex.reads,
        );
    }
}

fn main() {
    let a: Vec<String> = env::args().collect();
    if a.len() < 4 || a.len() > 5 {
        usage();
    }
    let arm = a[1].as_str();
    if arm != "map" && arm != "str" {
        usage();
    }
    let writers: usize = a[2].parse().unwrap_or_else(|_| usage());
    let readers: usize = a[3].parse().unwrap_or_else(|_| usage());
    let health = match a.get(4).map(String::as_str) {
        None => false,
        Some("--health") => true,
        Some(_) => usage(),
    };
    if writers + readers == 0 || writers + readers > MAX_THREADS {
        eprintln!("writers + readers must be in 1..={MAX_THREADS} (the P-core pin); refusing");
        std::process::exit(1);
    }
    if health != occ_stats::enabled() {
        eprintln!(
            "build/role mismatch: occ-stats {} but --health {} — a throughput figure from a \
             diagnostic build, or a health ratio from a default build, is void (§9)",
            if occ_stats::enabled() { "on" } else { "off" },
            if health { "given" } else { "absent" }
        );
        std::process::exit(1);
    }

    let main_ti = MtThread::slot(MAIN_SLOT);
    match arm {
        "map" => {
            let cw = workload::build_concurrent(N_PREFILL, M_NEW, 64, 0.5);
            let s = Stream {
                prefill: cw.base.population,
                probes: cw.base.probes,
                probe_is_prefill: cw.probe_is_prefill,
                new_keys: cw.new_keys,
            };
            drive(
                &s,
                || Masstree::new(main_ti, Table::Concurrent),
                SyncExpanseMap::new,
                writers,
                readers,
                health,
                &CellId {
                    workload_id: "masstree_conc_map_64bit",
                    label: "map",
                    dist: "random",
                },
            );
        }
        _ => {
            let cw = strings::build_concurrent(StrDist::Short, N_PREFILL, M_NEW, 0.5);
            let s = Stream {
                prefill: cw.base.population,
                probes: cw.base.probes,
                probe_is_prefill: cw.probe_is_prefill,
                new_keys: cw.new_keys,
            };
            drive(
                &s,
                || Masstree::new(main_ti, Table::Concurrent),
                SyncExpanseStrMap::new,
                writers,
                readers,
                health,
                &CellId {
                    workload_id: "masstree_conc_str",
                    label: "str",
                    dist: "short",
                },
            );
        }
    }
}
