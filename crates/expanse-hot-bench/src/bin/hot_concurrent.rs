//! Concurrent arm: writer throughput as writer count scales, reader throughput
//! alongside, and the Expanse protocol's health under that write load —
//! HOT-ROWEX against `SyncExpanseSet` / `SyncExpanseMap` (#692, METHODOLOGY.md §11).
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `hot_concurrent` |
//! | `group` | 7 |
//! | `emits` | `hot_rowex_set_63bit`, `hot_rowex_map_64bit` — the id(s) this harness writes into its JSON artifact; every row carries `role` = `throughput`, `health` or `counters` |
//! | `population` | prefill 2^20 uniform random keys (λ = 16 at 64 bits; 63-bit domain on the set arm), plus 2^20 fresh keys inserted concurrently by W writers |
//! | `insertion_order` | sorted ascending — the shared generator sorts and dedups the population (`workload.rs`); this harness takes no `order` token |
//! | `probes_and_reuse` | R readers cycle a shuffled 2^20-probe stream against the prefill until the writers finish; at W = 0 each reader makes exactly one pass; hits strided across the whole sorted population, not its prefix (§8.6) |
//! | `hit_rate` | 50% against the prefill; some misses become hits as writers land, identically on both arms |
//! | `miss_gen_method` | same-generator rejection sampling (§8.6); fresh writer keys rejected on prefill membership |
//! | `value_dereference` | map arm fetches the stored value on both sides and checks it against its key-derived expectation; set arm checks presence of every prefill probe |
//! | `measured_region` | barrier release to last-writer join (writers) and to last-reader join (readers); prefill, teardown and population walks outside |
//! | `arm_symmetry` | identical prefill, probe and fresh-key streams; both arms below any external lock through their native concurrent APIs (§8.16, §11.3 decision 4); same ISA target; W + R ≤ 16 inside the P-core pin, which is 16 logical CPUs on 8 physical P-cores — a cell with W + R > 8 places two threads per physical core (SMT siblings), and no interval says so |
//! | `statistics` | per-round throughput emitted raw, arms interleaved per round (`--rounds N --round-offset K` runs rounds K..K+N of that interleaving in one process, so a two-commit runner can alternate two builds round by round with the arm order continuous across them — the only before/after form docs/BENCHMARKING.md rule 18 admits); BCa 95% CIs on the Expanse ÷ ROWEX ratio computed by the runner (§8.4); every row carries the harness's own operation counts (`*_read_ops` = probes completed by the readers, `*_write_ops` = fresh keys inserted) as the divisor for any per-operation figure (§8.9 principle 5); `--health` emits event ratios from a diagnostic build and never a timing; `--arm <expanse or rowex>` runs one arm alone and emits `counters` rows (`read_ops`, `write_ops`, elapsed) for `scripts/bench_counters.py`'s per-thread `perf stat` attach, never a comparison |
//! | `verdict` | pending measurement |
//!
//! ## Fixed work, not a fixed window
//!
//! Each writer inserts its whole slice of the fresh-key stream; the timed
//! region ends when the last writer joins. Both arms therefore do identical
//! work per round and grow by exactly the same population. A fixed-duration
//! window would let the faster arm grow more and face a larger trie (§11.4).
//!
//! ## One cell per invocation
//!
//! ROWEX's reclamation strategy is a process-global singleton with thread-local
//! free lists that outlive every trie (§11.3, decision 6). The runner drives
//! the sweep; this binary runs one `(arm, writers, readers)` cell.
//!
//! ## Two builds, never one
//!
//! Throughput comes from the default build. `--health` requires the
//! `occ-stats` feature and refuses to run without it; a throughput cell —
//! and the `--arm` counters cell — refuses to run *with* it. The counters
//! are diagnostic-only (the engine documents them as never enabled for a
//! published benchmark), so the roles cannot share a binary.
//!
//! ## Counters mode (`--arm`), and the per-thread attach handshake
//!
//! `--arm <expanse|rowex> [--rounds N] [--wait-stdin]` runs only that arm,
//! with no interleaving, over the same prefill, probe and fresh-key streams
//! as the throughput cell, and prints one `{"role":"counters",...}` row per
//! round carrying `read_ops` and `write_ops` — the harness's own counts, so a
//! driver dividing a hardware-counter row by them publishes a per-operation
//! figure the harness agrees with (§8.9 principle 5). It is not a comparison
//! and its elapsed fields are not a published timing.
//!
//! Every thread is named: writer `w` is `writer-{w}`, reader `r` is
//! `reader-{r}` (the kernel's `comm`, which `perf stat --per-thread` keys its
//! rows on). After a round's threads are spawned and *before* the barrier
//! releases them, the harness prints `{"event":"threads_ready","round":N,"pid":P}`
//! and, with `--wait-stdin`, blocks on one line of stdin. The handshake is
//! what puts a `perf stat -p <pid>` attach between the spawn and the barrier;
//! what that attach does and does not count is recorded on [`threads_ready`].
//!
//! `--layout` (health build only) prints `expanse_trie::sync::layout_report()`
//! as JSON lines and exits — the byte offsets a `perf c2c` reader needs to
//! name which fields share a line.

use std::env;
use std::hint::black_box;
use std::io::{BufRead, Write};
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
/// Rounds per `--arm` counters cell unless `--rounds` says otherwise.
const COUNTER_ROUNDS: usize = 5;
/// The P-core pin on the reference host is 16 logical CPUs (§11.3, decision 3).
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
/// prefill's writes and before the measured section. `ready` runs on the
/// main thread after every writer and reader has been spawned and before the
/// barrier releases them — the attach point for a per-thread `perf stat`.
fn run_round<A: ConcArm>(
    arm: &A,
    cw: &ConcurrentWorkload,
    writers: usize,
    readers: usize,
    prefill: bool,
    ready: Option<&dyn Fn()>,
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
                let name = format!("writer-{w}");
                let h = std::thread::Builder::new()
                    .name(name.clone())
                    .spawn_scoped(s, move || {
                        barrier.wait();
                        for k in slice {
                            arm.insert(*k);
                        }
                    })
                    .unwrap_or_else(|e| panic!("spawning {name} failed: {e}"));
                wh.push(h);
            }
        }
        let mut rh = Vec::with_capacity(readers);
        for r in 0..readers {
            let (barrier, stop) = (&barrier, &stop);
            let (reads_total, errors_total) = (&reads_total, &errors_total);
            let name = format!("reader-{r}");
            let body = move || {
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
            };
            let h = std::thread::Builder::new()
                .name(name.clone())
                .spawn_scoped(s, body)
                .unwrap_or_else(|e| panic!("spawning {name} failed: {e}"));
            rh.push(h);
        }

        if let Some(f) = ready {
            f();
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

/// Elapsed seconds as a JSON number, `null` when the role had no thread.
fn json_secs(d: Option<Duration>) -> String {
    json_opt(d.map(|d| d.as_secs_f64()))
}

/// The `occ_stats` counter called `name`. A name the engine does not publish
/// is a harness bug, never a zero: a zero would read as "measured, and there
/// were none".
fn stat(snap: &[u64; occ_stats::NUM_STATS], name: &str) -> u64 {
    let i = occ_stats::NAMES
        .iter()
        .position(|n| *n == name)
        .unwrap_or_else(|| panic!("occ_stats::NAMES publishes no counter named `{name}`"));
    snap[i]
}

/// The attach handshake: announce the round's threads, then (with
/// `--wait-stdin`) block until the driver has attached and says so.
///
/// What `perf stat --per-thread -p <pid>` does, as observed on the reference
/// host (perf 6.8.12, Linux 6.8) before this was relied on: the attach reads
/// `/proc/<pid>/task` once and opens one counter set per thread it finds; the
/// `-x,` CSV then carries one row per (thread, event) keyed `<comm>-<tid>`; a
/// thread created after the attach has no row and is never counted; a thread
/// that exits before perf stops keeps its row; `SIGINT` to perf ends the
/// count and writes the rows. That is why this runs after every spawn and
/// before the barrier: each writer and reader is counted from its first
/// instruction of the round, and the main thread's join and population walk
/// land on the main thread's own row, which the driver keeps but does not
/// attribute to either role.
fn threads_ready(round: usize, wait_stdin: bool) {
    println!(
        "{{\"event\":\"threads_ready\",\"round\":{round},\"pid\":{}}}",
        std::process::id()
    );
    std::io::stdout().flush().expect("stdout flush failed");
    if wait_stdin {
        let mut line = String::new();
        let n = std::io::stdin()
            .lock()
            .read_line(&mut line)
            .expect("reading the release line from stdin failed");
        if n == 0 {
            eprintln!("stdin closed before round {round} was released; the driver detached");
            std::process::exit(1);
        }
    }
}

/// The process's CPU affinity list, recorded in every row so thread placement
/// is part of the artifact (§11.3, decision 3).
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
    eprintln!(
        "usage: hot_concurrent <set|map> <writers> <readers> \
         [--rounds N] [--round-offset K] [--health | --arm <expanse|rowex> [--rounds N] [--wait-stdin]]"
    );
    eprintln!("       hot_concurrent --layout");
    eprintln!("  writers + readers <= {MAX_THREADS}; one cell per invocation (§11.3 decision 6)");
    eprintln!("  --health needs the `occ-stats` feature and emits event ratios only");
    eprintln!("  --rounds / --round-offset run rounds K..K+N of the interleaved comparison");
    eprintln!("  --arm runs one arm alone and emits `counters` rows with its own op counts");
    eprintln!("  --wait-stdin blocks each round on one stdin line after `threads_ready`");
    eprintln!("  --layout prints sync::layout_report() as JSON lines (occ-stats build only)");
    std::process::exit(2);
}

/// What one invocation does; the three are mutually exclusive.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// The interleaved comparison for rounds `offset..offset + rounds`
    /// (`--rounds` / `--round-offset`; the whole cell by default). A
    /// two-commit runner splits one cell across processes of two builds,
    /// one round each, and the offset keeps the round numbers — and the
    /// arm order, which alternates by round — continuous across them.
    Throughput { rounds: usize, offset: usize },
    Health,
    /// One arm alone for `rounds` rounds, with the optional stdin handshake.
    Counters {
        expanse: bool,
        rounds: usize,
        wait_stdin: bool,
    },
}

struct Cli {
    arm: Arm,
    writers: usize,
    readers: usize,
    mode: Mode,
}

fn parse_cli(a: &[String]) -> Cli {
    if a.len() == 2 && a[1] == "--layout" {
        print_layout();
    }
    if a.len() < 4 {
        usage();
    }
    let arm = match a[1].as_str() {
        "set" => Arm::SetA,
        "map" => Arm::MapB,
        _ => usage(),
    };
    let writers: usize = a[2].parse().unwrap_or_else(|_| usage());
    let readers: usize = a[3].parse().unwrap_or_else(|_| usage());
    let (mut health, mut side, mut rounds, mut wait_stdin) = (false, None, None, false);
    let mut offset = 0usize;
    let mut i = 4;
    while i < a.len() {
        match a[i].as_str() {
            "--health" => health = true,
            "--wait-stdin" => wait_stdin = true,
            "--arm" => {
                i += 1;
                side = Some(a.get(i).cloned().unwrap_or_else(|| usage()));
            }
            "--rounds" => {
                i += 1;
                rounds = Some(
                    a.get(i)
                        .and_then(|v| v.parse::<usize>().ok())
                        .filter(|n| *n > 0)
                        .unwrap_or_else(|| usage()),
                );
            }
            "--round-offset" => {
                i += 1;
                offset = a
                    .get(i)
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or_else(|| usage());
            }
            _ => usage(),
        }
        i += 1;
    }
    let mode = match (health, side) {
        (true, None) => {
            if rounds.is_some() || wait_stdin || offset != 0 {
                eprintln!("--rounds, --round-offset and --wait-stdin do not apply to --health");
                usage();
            }
            Mode::Health
        }
        (false, Some(side)) => {
            let expanse = match side.as_str() {
                "expanse" => true,
                "rowex" => false,
                _ => usage(),
            };
            if offset != 0 {
                eprintln!("--round-offset applies to the interleaved comparison, not --arm");
                usage();
            }
            Mode::Counters {
                expanse,
                rounds: rounds.unwrap_or(COUNTER_ROUNDS),
                wait_stdin,
            }
        }
        (false, None) => {
            if wait_stdin {
                eprintln!("--wait-stdin needs --arm");
                usage();
            }
            Mode::Throughput {
                rounds: rounds.unwrap_or(ROUNDS),
                offset,
            }
        }
        (true, Some(_)) => {
            eprintln!("--health and --arm are two different builds; pick one");
            usage();
        }
    };
    Cli {
        arm,
        writers,
        readers,
        mode,
    }
}

/// `--layout`: the sync wrappers' field offsets as JSON lines, from the
/// health build only (the engine gates `layout_report` on `occ-stats`).
fn print_layout() -> ! {
    #[cfg(feature = "occ-stats")]
    {
        for (wrapper, field, offset) in expanse_trie::sync::layout_report() {
            println!(
                "{{\"role\":\"layout\",\"wrapper\":\"{wrapper}\",\"field\":\"{field}\",\"offset\":{offset}}}"
            );
        }
        std::process::exit(0);
    }
    #[cfg(not(feature = "occ-stats"))]
    {
        eprintln!("--layout needs the `occ-stats` build; this binary was built without it");
        std::process::exit(1);
    }
}

/// Everything a row needs to name its cell.
struct CellCtx<'a> {
    arm: Arm,
    writers: usize,
    readers: usize,
    n0: usize,
    m: u64,
    /// Fresh keys inserted per round: the writer-side operation count.
    write_ops: u64,
    expected: usize,
    cpus: &'a str,
    pin: &'a str,
}

/// The `--arm` counters cell for one side, generic over the arm type: `rounds`
/// rounds of one arm alone, each with the attach handshake, one `counters`
/// row per round carrying the harness's own operation counts.
fn run_counters<A: ConcArm>(
    make: impl Fn() -> A,
    side: &str,
    cw: &ConcurrentWorkload,
    cx: &CellCtx<'_>,
    rounds: usize,
    wait_stdin: bool,
) {
    let pid = std::process::id();
    for round in 0..rounds {
        let ready = move || threads_ready(round, wait_stdin);
        let a = make();
        let r = run_round(&a, cw, cx.writers, cx.readers, true, Some(&ready));
        void_cell(round, side, &r, cx.expected);
        println!(
            "{{\"workload_id\":\"{}\",\"role\":\"counters\",\"arm\":\"{side}\",\"cell\":\"{}\",\
             \"keyspace_bits\":{},\"prefill\":{},\"fresh_keys\":{},\"writers\":{},\"readers\":{},\
             \"round\":{round},\"pid\":{pid},\"read_ops\":{},\"write_ops\":{},\
             \"reader_elapsed_s\":{},\"writer_elapsed_s\":{},\"population_after\":{},\
             \"cpus_allowed\":\"{}\",\"pin_applied\":\"{}\"}}",
            cx.arm.workload_id(),
            cx.arm.label(),
            cx.arm.width(),
            cx.n0,
            cx.m,
            cx.writers,
            cx.readers,
            r.reads,
            cx.write_ops,
            json_secs(r.reader_elapsed),
            json_secs(r.writer_elapsed),
            cx.expected,
            cx.cpus,
            cx.pin,
        );
        std::io::stdout().flush().expect("stdout flush failed");
    }
}

fn void_cell(round: usize, side: &str, r: &RoundResult, expected: usize) {
    if r.errors != 0 || r.population != expected {
        eprintln!(
            "round {round} {side}: {} reader error(s), population {} (intended {expected}); cell is void (§11.7)",
            r.errors, r.population
        );
        std::process::exit(1);
    }
}

fn main() {
    let a: Vec<String> = env::args().collect();
    let Cli {
        arm,
        writers,
        readers,
        mode,
    } = parse_cli(&a);
    let health = mode == Mode::Health;
    if writers + readers == 0 || writers + readers > MAX_THREADS {
        eprintln!("writers + readers must be in 1..={MAX_THREADS} (the P-core pin); refusing");
        std::process::exit(1);
    }
    // Two builds, never one: health rows come from the occ-stats build only;
    // throughput and counters rows come from the default build only.
    if health != occ_stats::enabled() {
        eprintln!(
            "build/role mismatch: occ-stats {} but --health {} — a throughput or counters \
             figure from a diagnostic build, or a health ratio from a default build, is void (§11.7)",
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
    let m = cw.new_keys.len() as u64;
    // Fresh keys inserted per round: the writer-side operation count.
    let write_ops = if writers > 0 { m } else { 0 };

    if let Mode::Counters {
        expanse,
        rounds,
        wait_stdin,
    } = mode
    {
        // One arm alone, no interleaving: the rows exist to give a per-thread
        // counter attach a harness-owned divisor, not to compare anything.
        let cx = CellCtx {
            arm,
            writers,
            readers,
            n0,
            m,
            write_ops,
            expected,
            cpus: &cpus,
            pin: &pin,
        };
        match (arm, expanse) {
            (Arm::SetA, true) => {
                run_counters(SyncExpanseSet::new, "expanse", &cw, &cx, rounds, wait_stdin);
            }
            (Arm::SetA, false) => {
                run_counters(RowexSet::new, "rowex", &cw, &cx, rounds, wait_stdin);
            }
            (Arm::MapB, true) => {
                run_counters(SyncExpanseMap::new, "expanse", &cw, &cx, rounds, wait_stdin);
            }
            (Arm::MapB, false) => {
                run_counters(RowexMap::new, "rowex", &cw, &cx, rounds, wait_stdin);
            }
        }
        return;
    }

    if health {
        // Expanse side only: ROWEX has no counterpart counter. Event ratios
        // from a diagnostic build; nothing here is a timing (§11.3, decision 5).
        // The cycle counter behind `sample_spin_cycles` is calibrated once per
        // process and published beside every row that carries it.
        let cycles_hz = occ_stats::cycles_hz(Duration::from_millis(200));
        for round in 0..HEALTH_ROUNDS {
            let (snap, r) = match arm {
                Arm::SetA => {
                    let e = SyncExpanseSet::new();
                    for k in &cw.base.population {
                        e.insert(*k);
                    }
                    occ_stats::reset();
                    let r = run_round(&e, &cw, writers, readers, false, None);
                    let snap = occ_stats::snapshot();
                    void_cell(round, "expanse", &r, expected);
                    (snap, r)
                }
                Arm::MapB => {
                    let e = SyncExpanseMap::new();
                    for k in &cw.base.population {
                        e.insert(*k, value_of(*k));
                    }
                    occ_stats::reset();
                    let r = run_round(&e, &cw, writers, readers, false, None);
                    let snap = occ_stats::snapshot();
                    void_cell(round, "expanse", &r, expected);
                    (snap, r)
                }
            };
            let s = |name: &str| stat(&snap, name);
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
                 \"handoffs\":{},\"retired\":{},\"freed_raw\":{},\"sample_spin_cycles\":{},\
                 \"branch_replacements\":{},\"deep_cascades\":{},\"root_rewrites\":{},\
                 \"lock_restarts\":{},\"lock_spins\":{},\"lock_hold_cycles\":{},\"lock_fallbacks\":{},\
                 \"cycles_hz\":{cycles_hz},\"reader_elapsed_s\":{},\"writer_elapsed_s\":{},\
                 \"cpus_allowed\":\"{cpus}\",\"pin_applied\":\"{pin}\"}}",
                arm.workload_id(),
                arm.label(),
                s("sample_spins"),
                s("write_ops"),
                s("locked_reads"),
                s("handoffs"),
                s("retired"),
                s("freed_raw"),
                s("sample_spin_cycles"),
                s("branch_replacements"),
                s("deep_cascades"),
                s("root_rewrites"),
                s("lock_restarts"),
                s("lock_spins"),
                s("lock_hold_cycles"),
                s("lock_fallbacks"),
                // Barrier release to last-reader join / last-writer join:
                // one duration per role, not a per-thread mean.
                json_secs(r.reader_elapsed),
                json_secs(r.writer_elapsed),
            );
        }
        return;
    }

    let Mode::Throughput { rounds, offset } = mode else {
        unreachable!("health and counters modes returned above");
    };
    for round in offset..offset + rounds {
        // Interleave the arms round by round (docs/BENCHMARKING.md rule 1):
        // drift hits both and cancels in the paired ratio.
        let rowex_first = round % 2 == 0;
        let (hot, exp) = match arm {
            Arm::SetA => {
                let run_hot = || {
                    let h = RowexSet::new();
                    let r = run_round(&h, &cw, writers, readers, true, None);
                    void_cell(round, "rowex", &r, expected);
                    r
                };
                let run_exp = || {
                    let e = SyncExpanseSet::new();
                    let r = run_round(&e, &cw, writers, readers, true, None);
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
                    let r = run_round(&h, &cw, writers, readers, true, None);
                    void_cell(round, "rowex", &r, expected);
                    r
                };
                let run_exp = || {
                    let e = SyncExpanseMap::new();
                    let r = run_round(&e, &cw, writers, readers, true, None);
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

        println!(
            "{{\"workload_id\":\"{}\",\"role\":\"throughput\",\"arm\":\"{}\",\"keyspace_bits\":{},\
             \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{writers},\"readers\":{readers},\
             \"round\":{round},\"first\":\"{}\",\
             \"rowex_writer_mops\":{},\"expanse_writer_mops\":{},\
             \"rowex_reader_mops\":{},\"expanse_reader_mops\":{},\
             \"rowex_reads\":{},\"expanse_reads\":{},\
             \"rowex_read_ops\":{},\"expanse_read_ops\":{},\
             \"rowex_write_ops\":{write_ops},\"expanse_write_ops\":{write_ops},\
             \"population_after\":{expected},\
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
            // `*_read_ops` repeats `*_reads` under the name every role's rows
            // share, so one divisor name serves throughput and counters rows.
            hot.reads,
            exp.reads,
        );
    }
}
