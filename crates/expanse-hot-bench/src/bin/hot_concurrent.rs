//! Concurrent arm: writer throughput as writer count scales, reader throughput
//! alongside, and the Expanse protocol's health under that write load —
//! HOT-ROWEX against `SyncExpanseSet` / `SyncExpanseMap` (#692, METHODOLOGY.md §10).
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `hot_concurrent` |
//! | `group` | 5 |
//! | `population` | prefill 2^20 uniform random keys (λ = 16 at 64 bits; 63-bit domain on the set arm), plus 2^20 fresh keys inserted concurrently by W writers |
//! | `probes_and_reuse` | R readers cycle a shuffled 2^20-probe stream against the prefill until the writers finish; at W = 0 each reader makes exactly one pass |
//! | `hit_rate` | 50% against the prefill; some misses become hits as writers land, identically on both arms |
//! | `miss_gen_method` | same-generator rejection sampling (§8.6); fresh writer keys rejected on prefill membership |
//! | `value_dereference` | map arm fetches the stored value on both sides and checks it against its key-derived expectation; set arm checks presence of every prefill probe |
//! | `measured_region` | barrier release to last-writer join (writers) and to last-reader join (readers); prefill, teardown and population walks outside |
//! | `arm_symmetry` | identical prefill, probe and fresh-key streams; both arms below any external lock through their native concurrent APIs (§8.16, §10.3 decision 4); same ISA target; W + R ≤ 16 inside the P-core pin |
//! | `statistics` | per-round throughput emitted raw, arms interleaved per round; BCa 95% CIs on the Expanse ÷ ROWEX ratio computed by the runner (§8.4); `--health` emits event ratios from a diagnostic build and never a timing |
//! | `verdict` | pending measurement |
//!
//! ## Fixed work, not a fixed window
//!
//! Each writer inserts its whole slice of the fresh-key stream; the timed
//! region ends when the last writer joins. Both arms therefore do identical
//! work per round and grow by exactly the same population. A fixed-duration
//! window would let the faster arm grow more and face a larger trie (§10.4).
//!
//! ## One cell per invocation
//!
//! ROWEX's reclamation strategy is a process-global singleton with thread-local
//! free lists that outlive every trie (§10.3, decision 6). The runner drives
//! the sweep; this binary runs one `(arm, writers, readers)` cell.
//!
//! ## Two builds, never one
//!
//! Throughput comes from the default build. `--health` requires the
//! `occ-stats` feature and refuses to run without it; a throughput cell
//! refuses to run *with* it. The counters are diagnostic-only (the engine
//! documents them as never enabled for a published benchmark), so the two
//! roles cannot share a binary.

use std::env;
use std::hint::black_box;
use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use expanse_hot_bench::rowex::{RowexMap, RowexSet};
use expanse_hot_bench::workload::{self, ConcurrentWorkload};
use expanse_hot_bench::{InlineInsert, hot_can_inline};
use expanse_trie::occ_stats;
use expanse_trie::sync::{SyncExpanseMap, SyncExpanseSet};

/// Prefill population, identical on both arms (λ = 16 at 64 bits).
const N_PREFILL: usize = 1 << 20;
/// Fresh keys the writers insert, split into W contiguous slices.
const M_NEW: usize = 1 << 20;
/// Rounds per throughput cell; the runner bootstraps over these.
const ROUNDS: usize = 15;
/// Rounds per health cell (event ratios, reported as median with range).
const HEALTH_ROUNDS: usize = 5;
/// The P-core pin on the reference host is 16 logical CPUs (§10.3, decision 3).
const MAX_THREADS: usize = 16;

/// The stored value for a key, so readers can verify every hit on both arms.
#[inline]
fn value_of(k: u64) -> u64 {
    k.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Arm {
    /// ROWEX `IdentityKeyExtractor` vs `SyncExpanseSet`, 63-bit domain (§9.6).
    SetA,
    /// ROWEX `PairPointerKeyExtractor` vs `SyncExpanseMap`, full 64-bit domain.
    MapB,
}

impl Arm {
    fn width(self) -> u32 {
        match self {
            Arm::SetA => 63,
            Arm::MapB => 64,
        }
    }
    fn workload_id(self) -> &'static str {
        match self {
            Arm::SetA => "hot_rowex_set_63bit",
            Arm::MapB => "hot_rowex_map_64bit",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Arm::SetA => "set",
            Arm::MapB => "map",
        }
    }
}

/// One side of a pairing, driven identically by [`run_round`].
trait ConcArm: Sync {
    /// A per-thread reader handle (Expanse registers one epoch slot per reader).
    type Reader<'a>
    where
        Self: 'a;
    fn reader(&self) -> Self::Reader<'_>;
    /// Concurrent insert of `k` with `value_of(k)`.
    fn insert(&self, k: u64);
    /// `Some(value_ok)` when found, `None` when absent.
    fn probe(r: &Self::Reader<'_>, k: u64) -> Option<bool>;
    /// Population, quiescent-only.
    fn len(&self) -> usize;
}

impl ConcArm for RowexSet {
    type Reader<'a> = &'a RowexSet;
    fn reader(&self) -> Self::Reader<'_> {
        self
    }
    fn insert(&self, k: u64) {
        if RowexSet::insert(self, k) == InlineInsert::NotRepresentable {
            eprintln!("set arm: key not representable during a concurrent insert; cell is void");
            std::process::exit(1);
        }
    }
    fn probe(r: &Self::Reader<'_>, k: u64) -> Option<bool> {
        r.contains(k).then_some(true)
    }
    fn len(&self) -> usize {
        RowexSet::len(self)
    }
}

impl ConcArm for RowexMap {
    type Reader<'a> = &'a RowexMap;
    fn reader(&self) -> Self::Reader<'_> {
        self
    }
    fn insert(&self, k: u64) {
        RowexMap::insert(self, k, value_of(k));
    }
    fn probe(r: &Self::Reader<'_>, k: u64) -> Option<bool> {
        r.get(k).map(|v| v == value_of(k))
    }
    fn len(&self) -> usize {
        RowexMap::len(self)
    }
}

impl ConcArm for SyncExpanseSet {
    type Reader<'a> = expanse_trie::sync::SetReader<'a>;
    fn reader(&self) -> Self::Reader<'_> {
        SyncExpanseSet::reader(self)
    }
    fn insert(&self, k: u64) {
        SyncExpanseSet::insert(self, k);
    }
    fn probe(r: &Self::Reader<'_>, k: u64) -> Option<bool> {
        r.contains(k).then_some(true)
    }
    fn len(&self) -> usize {
        SyncExpanseSet::len(self) as usize
    }
}

impl ConcArm for SyncExpanseMap {
    type Reader<'a> = expanse_trie::sync::MapReader<'a>;
    fn reader(&self) -> Self::Reader<'_> {
        SyncExpanseMap::reader(self)
    }
    fn insert(&self, k: u64) {
        SyncExpanseMap::insert(self, k, value_of(k));
    }
    fn probe(r: &Self::Reader<'_>, k: u64) -> Option<bool> {
        r.get(k).map(|v| v == value_of(k))
    }
    fn len(&self) -> usize {
        SyncExpanseMap::len(self) as usize
    }
}

/// What one round on one arm produced.
struct RoundResult {
    /// Barrier release to last-writer join. `None` when W = 0.
    writer_elapsed: Option<Duration>,
    /// Barrier release to last-reader join. `None` when R = 0.
    reader_elapsed: Option<Duration>,
    /// Reads completed across all readers.
    reads: u64,
    /// Prefill probes not found, or hits with a wrong value. Must be zero.
    errors: u64,
    /// Population after the writers joined, by walk (ROWEX) or `len` (Expanse).
    population: usize,
}

/// Prefills `arm` single-threaded when `prefill` is set (outside every timed
/// window), then runs the cell: W writers each insert one slice of the fresh
/// keys, R readers probe from the same barrier until the writers finish. The
/// health cells prefill themselves so the counters can be zeroed *after* the
/// prefill's writes and before the measured section.
fn run_round<A: ConcArm>(
    arm: &A,
    cw: &ConcurrentWorkload,
    writers: usize,
    readers: usize,
    prefill: bool,
) -> RoundResult {
    if prefill {
        for k in &cw.base.population {
            arm.insert(*k);
        }
    }

    let stop = AtomicBool::new(false);
    let barrier = Barrier::new(writers + readers + 1);
    let reads_total = AtomicU64::new(0);
    let errors_total = AtomicU64::new(0);
    let probes = &cw.base.probes;
    let is_prefill = &cw.probe_is_prefill;

    let (writer_elapsed, reader_elapsed) = std::thread::scope(|s| {
        let mut wh = Vec::with_capacity(writers);
        let per = cw.new_keys.len() / writers.max(1);
        if writers > 0 {
            for w in 0..writers {
                let lo = w * per;
                let hi = if w + 1 == writers {
                    cw.new_keys.len()
                } else {
                    lo + per
                };
                let slice = &cw.new_keys[lo..hi];
                let barrier = &barrier;
                wh.push(s.spawn(move || {
                    barrier.wait();
                    for k in slice {
                        arm.insert(*k);
                    }
                }));
            }
        }
        let mut rh = Vec::with_capacity(readers);
        for r in 0..readers {
            let (barrier, stop) = (&barrier, &stop);
            let (reads_total, errors_total) = (&reads_total, &errors_total);
            rh.push(s.spawn(move || {
                let rd = arm.reader();
                let n = probes.len();
                // Readers start at staggered offsets so they do not walk the
                // stream in lockstep; each still sees the whole stream.
                let mut i = (r * n) / readers.max(1);
                let (mut reads, mut errors, mut sink) = (0u64, 0u64, 0u64);
                barrier.wait();
                if writers == 0 {
                    for _ in 0..n {
                        match A::probe(&rd, probes[i]) {
                            Some(ok) => {
                                sink ^= 1;
                                errors += u64::from(!ok);
                            }
                            None => errors += u64::from(is_prefill[i]),
                        }
                        reads += 1;
                        i += 1;
                        if i == n {
                            i = 0;
                        }
                    }
                } else {
                    while !stop.load(Ordering::Relaxed) {
                        match A::probe(&rd, probes[i]) {
                            Some(ok) => {
                                sink ^= 1;
                                errors += u64::from(!ok);
                            }
                            None => errors += u64::from(is_prefill[i]),
                        }
                        reads += 1;
                        i += 1;
                        if i == n {
                            i = 0;
                        }
                    }
                }
                black_box(sink);
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

    RoundResult {
        writer_elapsed,
        reader_elapsed,
        reads: reads_total.load(Ordering::Relaxed),
        errors: errors_total.load(Ordering::Relaxed),
        population: arm.len(),
    }
}

fn mops(ops: u64, d: Option<Duration>) -> Option<f64> {
    d.map(|d| ops as f64 / d.as_secs_f64() / 1e6)
}

fn json_opt(v: Option<f64>) -> String {
    v.map_or_else(|| "null".to_string(), |x| format!("{x:.4}"))
}

/// The process's CPU affinity list, recorded in every row so thread placement
/// is part of the artifact (§10.3, decision 3).
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
    eprintln!("usage: hot_concurrent <set|map> <writers> <readers> [--health]");
    eprintln!("  writers + readers <= {MAX_THREADS}; one cell per invocation (§10.3 decision 6)");
    eprintln!("  --health needs the `occ-stats` feature and emits event ratios only");
    std::process::exit(2);
}

fn void_cell(round: usize, side: &str, r: &RoundResult, expected: usize) {
    if r.errors != 0 || r.population != expected {
        eprintln!(
            "round {round} {side}: {} reader error(s), population {} (intended {expected}); cell is void (§10.7)",
            r.errors, r.population
        );
        std::process::exit(1);
    }
}

fn main() {
    let a: Vec<String> = env::args().collect();
    if a.len() < 4 || a.len() > 5 {
        usage();
    }
    let arm = match a[1].as_str() {
        "set" => Arm::SetA,
        "map" => Arm::MapB,
        _ => usage(),
    };
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
             diagnostic build, or a health ratio from a default build, is void (§10.7)",
            if occ_stats::enabled() { "on" } else { "off" },
            if health { "given" } else { "absent" }
        );
        std::process::exit(1);
    }

    let cw = workload::build_concurrent(N_PREFILL, M_NEW, arm.width(), 0.5);
    let n0 = cw.base.population.len();
    // The reader-only reference cell (W = 0) inserts nothing past the prefill.
    let expected = n0 + if writers > 0 { cw.new_keys.len() } else { 0 };
    if arm == Arm::SetA
        && !(cw.base.population.iter().all(|k| hot_can_inline(*k))
            && cw.new_keys.iter().all(|k| hot_can_inline(*k)))
    {
        eprintln!("set arm generator produced a key outside HOT's inline payload");
        std::process::exit(1);
    }
    let cpus = cpus_allowed();
    let pin = env::var("EXPANSE_BENCH_PIN_APPLIED").unwrap_or_else(|_| "unset".to_string());

    if health {
        // Expanse side only: ROWEX has no counterpart counter. Event ratios
        // from a diagnostic build; nothing here is a timing (§10.3, decision 5).
        for round in 0..HEALTH_ROUNDS {
            let snap = match arm {
                Arm::SetA => {
                    let e = SyncExpanseSet::new();
                    for k in &cw.base.population {
                        e.insert(*k);
                    }
                    occ_stats::reset();
                    let r = run_round(&e, &cw, writers, readers, false);
                    let snap = occ_stats::snapshot();
                    void_cell(round, "expanse", &r, expected);
                    snap
                }
                Arm::MapB => {
                    let e = SyncExpanseMap::new();
                    for k in &cw.base.population {
                        e.insert(*k, value_of(*k));
                    }
                    occ_stats::reset();
                    let r = run_round(&e, &cw, writers, readers, false);
                    let snap = occ_stats::snapshot();
                    void_cell(round, "expanse", &r, expected);
                    snap
                }
            };
            let s = |name: &str| snap[occ_stats::NAMES.iter().position(|n| *n == name).unwrap()];
            let (ops, attempts, fallbacks) =
                (s("read_ops"), s("read_attempts"), s("read_fallbacks"));
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
                "{{\"workload_id\":\"{}\",\"role\":\"health\",\"arm\":\"{}\",\"writers\":{writers},\
                 \"readers\":{readers},\"round\":{round},\"read_ops\":{ops},\"read_attempts\":{attempts},\
                 \"read_fallbacks\":{fallbacks},\"sample_spins\":{},\"write_ops\":{},\"locked_reads\":{},\
                 \"restart_share\":{restart_share:.6},\"fallback_share\":{fallback_share:.6},\
                 \"cpus_allowed\":\"{cpus}\",\"pin_applied\":\"{pin}\"}}",
                arm.workload_id(),
                arm.label(),
                s("sample_spins"),
                s("write_ops"),
                s("locked_reads"),
            );
        }
        return;
    }

    for round in 0..ROUNDS {
        // Interleave the arms round by round (docs/BENCHMARKING.md rule 1):
        // drift hits both and cancels in the paired ratio.
        let rowex_first = round % 2 == 0;
        let (hot, exp) = match arm {
            Arm::SetA => {
                let run_hot = || {
                    let h = RowexSet::new();
                    let r = run_round(&h, &cw, writers, readers, true);
                    void_cell(round, "rowex", &r, expected);
                    r
                };
                let run_exp = || {
                    let e = SyncExpanseSet::new();
                    let r = run_round(&e, &cw, writers, readers, true);
                    void_cell(round, "expanse", &r, expected);
                    r
                };
                if rowex_first {
                    let h = run_hot();
                    (h, run_exp())
                } else {
                    let e = run_exp();
                    (run_hot(), e)
                }
            }
            Arm::MapB => {
                let run_hot = || {
                    let h = RowexMap::new();
                    let r = run_round(&h, &cw, writers, readers, true);
                    void_cell(round, "rowex", &r, expected);
                    r
                };
                let run_exp = || {
                    let e = SyncExpanseMap::new();
                    let r = run_round(&e, &cw, writers, readers, true);
                    void_cell(round, "expanse", &r, expected);
                    r
                };
                if rowex_first {
                    let h = run_hot();
                    (h, run_exp())
                } else {
                    let e = run_exp();
                    (run_hot(), e)
                }
            }
        };

        let m = cw.new_keys.len() as u64;
        println!(
            "{{\"workload_id\":\"{}\",\"role\":\"throughput\",\"arm\":\"{}\",\"keyspace_bits\":{},\
             \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{writers},\"readers\":{readers},\
             \"round\":{round},\"first\":\"{}\",\
             \"rowex_writer_mops\":{},\"expanse_writer_mops\":{},\
             \"rowex_reader_mops\":{},\"expanse_reader_mops\":{},\
             \"rowex_reads\":{},\"expanse_reads\":{},\"population_after\":{expected},\
             \"cpus_allowed\":\"{cpus}\",\"pin_applied\":\"{pin}\"}}",
            arm.workload_id(),
            arm.label(),
            arm.width(),
            if rowex_first { "rowex" } else { "expanse" },
            json_opt(mops(m, hot.writer_elapsed)),
            json_opt(mops(m, exp.writer_elapsed)),
            json_opt(mops(hot.reads, hot.reader_elapsed)),
            json_opt(mops(exp.reads, exp.reader_elapsed)),
            hot.reads,
            exp.reads,
        );
    }
}
