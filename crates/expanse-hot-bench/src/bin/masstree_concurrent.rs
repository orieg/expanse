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
//! | `group` | 7 |
//! | `emits` | `masstree_conc_map_64bit`, `masstree_conc_str` — the id(s) this harness writes into its JSON artifact, which is what the suite README's `(workload: …)` tags cite; every row carries `role` = `throughput`, `health` or `counters` |
//! | `population` | prefill 2^20 keys (uniform random u64 at 64 bits, or `short` strings), plus 2^20 fresh keys inserted concurrently by W writers |
//! | `insertion_order` | sorted ascending — the shared generator sorts and dedups the population (`workload.rs`); this harness takes no `order` token |
//! | `probes_and_reuse` | R readers cycle a shuffled 2^20-probe stream against the prefill until the writers finish; at W = 0 each reader makes exactly one pass; hits strided across the whole sorted population, not its prefix (§8.6) |
//! | `hit_rate` | 50% against the prefill; some misses become hits as writers land, identically on both arms |
//! | `miss_gen_method` | same-generator rejection sampling (§8.6); fresh writer keys rejected on prefill membership |
//! | `value_dereference` | both sides fetch the stored value and check it against its key-derived expectation |
//! | `measured_region` | barrier release to last-writer join (writers) and to last-reader join (readers); prefill, teardown and walks outside; Masstree's per-64-op `quiesce` inside its threads (§3.2) |
//! | `arm_symmetry` | identical prefill, probe and fresh-key streams; both arms below any external lock through their native concurrent APIs (§8.16); one thread slot per Masstree thread; same ISA target; W + R ≤ 16 inside the P-core pin, which is 16 logical CPUs on 8 physical P-cores — a cell with W + R > 8 places two threads per physical core (SMT siblings), and no interval says so |
//! | `statistics` | per-round throughput emitted raw, arms interleaved per round (`--rounds N --round-offset K` runs rounds K..K+N of that interleaving in one process, so a two-commit runner can alternate two builds round by round with the arm order continuous across them — the only before/after form docs/BENCHMARKING.md rule 18 admits); BCa 95% CIs on the Expanse ÷ Masstree ratio computed by the runner (§8.4); every row carries the harness's own operation counts (`*_read_ops` = probes completed by the readers, `*_write_ops` = fresh keys inserted) as the divisor for any per-operation figure (§8.9 principle 5); `--health` emits event ratios from a diagnostic build and never a timing; `--arm <expanse or masstree>` runs one arm alone and emits `counters` rows (`read_ops`, `write_ops`, elapsed) for `scripts/bench_counters.py`'s per-thread `perf stat` attach, never a comparison |
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
//! feature and refuses to run without it; a throughput cell — and the
//! `--arm` counters cell — refuses to run with it (the counters are
//! diagnostic-only in the engine).
//!
//! ## Counters mode (`--arm`), and the per-thread attach handshake
//!
//! `--arm <expanse|masstree> [--rounds N] [--wait-stdin]` runs only that arm,
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

use expanse_hot_bench::masstree::{Masstree, MtThread, QUIESCE_EVERY, StrInsert, Table};
use expanse_hot_bench::strings::{self, KeyStr, StrDist};
use expanse_hot_bench::workload;
use expanse_trie::occ_stats;
use expanse_trie::sync::{SyncExpanseMap, SyncExpanseStrMap};

const N_PREFILL: usize = 1 << 20;
const M_NEW: usize = 1 << 20;
const ROUNDS: usize = 15;
const HEALTH_ROUNDS: usize = 5;
/// Rounds per `--arm` counters cell unless `--rounds` says otherwise.
const COUNTER_ROUNDS: usize = 5;
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

/// `ready` runs on the main thread after every writer and reader has been
/// spawned and before the barrier releases them — the attach point for a
/// per-thread `perf stat` (see the module doc).
fn run_round<K: KeyLike, A: ConcArm<K>>(
    arm: &A,
    s: &Stream<K>,
    writers: usize,
    readers: usize,
    prefill: bool,
    ready: Option<&dyn Fn()>,
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
            let name = format!("writer-{w}");
            let h = std::thread::Builder::new()
                .name(name.clone())
                .spawn_scoped(sc, move || {
                    let c = A::ctx(w as u32);
                    A::begin(c);
                    barrier.wait();
                    for (i, k) in slice.iter().enumerate() {
                        arm.insert(c, k);
                        A::tick(c, i as u64 + 1);
                    }
                    A::end(c);
                })
                .unwrap_or_else(|e| panic!("spawning {name} failed: {e}"));
            wh.push(h);
        }
        let mut rh = Vec::with_capacity(readers);
        for r in 0..readers {
            let (barrier, stop) = (&barrier, &stop);
            let (reads_total, errors_total) = (&reads_total, &errors_total);
            let name = format!("reader-{r}");
            let body = move || {
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
            };
            let h = std::thread::Builder::new()
                .name(name.clone())
                .spawn_scoped(sc, body)
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
        "usage: masstree_concurrent <map|str> <writers> <readers> \
         [--rounds N] [--round-offset K] [--health | --arm <expanse|masstree> [--rounds N] [--wait-stdin]]"
    );
    eprintln!("       masstree_concurrent --layout");
    eprintln!("  writers + readers <= {MAX_THREADS}; one cell per invocation (§3.6)");
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
    kind: String,
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
    let kind = a[1].clone();
    if kind != "map" && kind != "str" {
        usage();
    }
    let writers: usize = a[2].parse().unwrap_or_else(|_| usage());
    let readers: usize = a[3].parse().unwrap_or_else(|_| usage());
    let (mut health, mut arm, mut rounds, mut wait_stdin) = (false, None, None, false);
    let mut offset = 0usize;
    let mut i = 4;
    while i < a.len() {
        match a[i].as_str() {
            "--health" => health = true,
            "--wait-stdin" => wait_stdin = true,
            "--arm" => {
                i += 1;
                arm = Some(a.get(i).cloned().unwrap_or_else(|| usage()));
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
    let mode = match (health, arm) {
        (true, None) => {
            if rounds.is_some() || wait_stdin || offset != 0 {
                eprintln!("--rounds, --round-offset and --wait-stdin do not apply to --health");
                usage();
            }
            Mode::Health
        }
        (false, Some(arm)) => {
            let expanse = match arm.as_str() {
                "expanse" => true,
                "masstree" => false,
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
        kind,
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
    mode: Mode,
    id: &CellId<'_>,
) {
    let (workload_id, label, dist) = (id.workload_id, id.label, id.dist);
    let n0 = s.prefill.len();
    let m = s.new_keys.len() as u64;
    // Fresh keys inserted per round: the writer-side operation count.
    let write_ops = if writers > 0 { m } else { 0 };
    let expected = n0 + if writers > 0 { s.new_keys.len() } else { 0 };
    let cpus = cpus_allowed();
    let pin = env::var("EXPANSE_BENCH_PIN_APPLIED").unwrap_or_else(|_| "unset".to_string());

    if let Mode::Counters {
        expanse,
        rounds,
        wait_stdin,
    } = mode
    {
        // One arm alone, no interleaving: the rows exist to give a per-thread
        // counter attach a harness-owned divisor, not to compare anything.
        let arm_name = if expanse { "expanse" } else { "masstree" };
        let pid = std::process::id();
        for round in 0..rounds {
            let ready = move || threads_ready(round, wait_stdin);
            let r = if expanse {
                let e = make_exp();
                let r = run_round(&e, s, writers, readers, true, Some(&ready));
                void_cell(round, "expanse", &r, expected);
                r
            } else {
                let t = make_mt();
                let r = run_round(&t, s, writers, readers, true, Some(&ready));
                void_cell(round, "masstree", &r, expected);
                r
            };
            println!(
                "{{\"workload_id\":\"{workload_id}\",\"role\":\"counters\",\"arm\":\"{arm_name}\",\
                 \"cell\":\"{label}\",\"dist\":\"{dist}\",\"prefill\":{n0},\"fresh_keys\":{m},\
                 \"writers\":{writers},\"readers\":{readers},\"round\":{round},\"pid\":{pid},\
                 \"read_ops\":{},\"write_ops\":{write_ops},\
                 \"reader_elapsed_s\":{},\"writer_elapsed_s\":{},\
                 \"population_after\":{expected},\"cpus_allowed\":\"{cpus}\",\"pin_applied\":\"{pin}\"}}",
                r.reads,
                json_secs(r.reader_elapsed),
                json_secs(r.writer_elapsed),
            );
            std::io::stdout().flush().expect("stdout flush failed");
        }
        return;
    }

    if mode == Mode::Health {
        // Expanse side only: Masstree has no counterpart counter (§6.3).
        // The cycle counter behind `sample_spin_cycles` is calibrated once per
        // process and published beside every row that carries it.
        let cycles_hz = occ_stats::cycles_hz(Duration::from_millis(200));
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
            let r = run_round(&e, s, writers, readers, false, None);
            let snap = occ_stats::snapshot();
            void_cell(round, "expanse", &r, expected);
            let st = |name: &str| stat(&snap, name);
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
                 \"fallback_share\":{fallback_share:.6},\
                 \"handoffs\":{},\"retired\":{},\"freed_raw\":{},\"sample_spin_cycles\":{},\
                 \"branch_replacements\":{},\"deep_cascades\":{},\"root_rewrites\":{},\
                 \"cycles_hz\":{cycles_hz},\"reader_elapsed_s\":{},\"writer_elapsed_s\":{},\
                 \"cpus_allowed\":\"{cpus}\",\"pin_applied\":\"{pin}\"}}",
                st("sample_spins"),
                st("write_ops"),
                st("locked_reads"),
                st("handoffs"),
                st("retired"),
                st("freed_raw"),
                st("sample_spin_cycles"),
                st("branch_replacements"),
                st("deep_cascades"),
                st("root_rewrites"),
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
        // Interleave the arms round by round (docs/BENCHMARKING.md rule 1).
        let mt_first = round % 2 == 0;
        let run_mt = || {
            let t = make_mt();
            let r = run_round(&t, s, writers, readers, true, None);
            void_cell(round, "masstree", &r, expected);
            r
        };
        let run_exp = || {
            let e = make_exp();
            let r = run_round(&e, s, writers, readers, true, None);
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
             \"masstree_reads\":{},\"expanse_reads\":{},\
             \"masstree_read_ops\":{},\"expanse_read_ops\":{},\
             \"masstree_write_ops\":{write_ops},\"expanse_write_ops\":{write_ops},\
             \"population_after\":{expected},\
             \"cpus_allowed\":\"{cpus}\",\"pin_applied\":\"{pin}\"}}",
            if mt_first { "masstree" } else { "expanse" },
            json_opt(mops(m, mt.writer_elapsed)),
            json_opt(mops(m, ex.writer_elapsed)),
            json_opt(mops(mt.reads, mt.reader_elapsed)),
            json_opt(mops(ex.reads, ex.reader_elapsed)),
            mt.reads,
            ex.reads,
            // `*_read_ops` repeats `*_reads` under the name every role's rows
            // share, so one divisor name serves throughput and counters rows.
            mt.reads,
            ex.reads,
        );
    }
}

fn main() {
    let a: Vec<String> = env::args().collect();
    let Cli {
        kind,
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
             figure from a diagnostic build, or a health ratio from a default build, is void (§9)",
            if occ_stats::enabled() { "on" } else { "off" },
            if health { "given" } else { "absent" }
        );
        std::process::exit(1);
    }

    let main_ti = MtThread::slot(MAIN_SLOT);
    match kind.as_str() {
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
                mode,
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
                mode,
                &CellId {
                    workload_id: "masstree_conc_str",
                    label: "str",
                    dist: "short",
                },
            );
        }
    }
}
