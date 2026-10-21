//! How long a periodic `SyncExpanseStrMap::mem_used()` reading stalls the
//! map's writers (`docs/benchmarks/concurrency/METHODOLOGY.md` §44, #1586).
//!
//! One process measures one (cell, arm, round) and prints one JSON row;
//! `docs/benchmarks/concurrency/scripts/strmap_poll_stall.py` runs the cells,
//! rotates their order per round, and writes the artifact.
//!
//! `--mode run` prefills a `SyncExpanseStrMap` with `--keys` keys of
//! `--shape` (single-threaded, outside every measured window), then runs
//! `--writers` threads inserting fresh keys for `--seconds`. `--arm` adds the
//! poller: `none` runs no poller, `len` calls the lock-free `len()` every
//! `--period-ms`, and `mem_used` calls `mem_used()`, which closes the writer
//! gate and walks every node shell and suffix leaf, at the same cadence. The
//! poller keeps a fixed schedule (`t0 + k × period`); a reading that outlasts
//! the period leaves the next one due at its return, so the readings run back
//! to back rather than overlapping or catching up.
//!
//! `--cpus a,b,c,...` (Linux) places the threads: the poller on the first CPU
//! (left unused by the `none` arm, so every arm puts its writers on the same
//! CPUs), then writer `w` on CPU `w + 1` of the list. The driver orders the
//! list one CPU per physical core first, then SMT siblings, so the poller has
//! a core of its own and writers reach a sibling only once the cores run out.
//! Each thread sets its own affinity and the row records the list.
//!
//! `--mode census` prefills and reports `len()`, `mem_used()` and the peak
//! resident set, timing nothing; the driver uses it to size the run.
//! `--mode identify` makes no measurement: one row naming the build (whether
//! `occ-stats` is compiled in) and the CPUs the process may run on.
//!
//! Built with `occ-stats`, a `run` row also carries the window's `counters`:
//! `GateBlockedEntries`, `GateWaitCycles`, `QuiesceCalls`,
//! `QuiesceDrainCycles`, `LockedReads` and `ContentionGateClosed`, with
//! `cycles_hz()` and the writers' CPU time (`CLOCK_THREAD_CPUTIME_ID`). A
//! counters build times nothing the driver reads.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `strmap_poll_stall` |
//! | `group` | 4 |
//! | `population` | `--keys` prefilled keys, 2,000,000 or 20,000,000 (METHODOLOGY §44.3), plus the fresh keys the writers insert during the window; `len()` is recorded before and after the window |
//! | `insertion_order` | generator — the prefill inserts keys in draw order: `routes` key `i` for `i` in `0..N`, `short` the file's XorShift stream with repeats skipped; writers insert fresh keys in their own draw order |
//! | `probes_and_reuse` | writers: fresh keys for `--seconds`, each inserted once (reuse 1.0); `routes` writer `w` takes indices `N + w + k × W`, `short` writer `w` draws from its own XorShift stream; the poller makes one reading per `--period-ms` |
//! | `hit_rate` | 0 % — every insert is of a key absent at draw time; a draw already present (`contains_key`) is rejected before the timed insert and counted, and an insert that finds its key present anyway (two writers drew it) is counted as an overwrite |
//! | `miss_gen_method` | fresh keys drawn from the population's generator and rejected on membership (`contains_key`), outside the timed call (AGENTS.md §8.6) |
//! | `value_dereference` | the insert's previous value is consumed (overwrite count); each reading's returned value is recorded |
//! | `measured_region` | `Instant::now()` around each insert and around each reading, uncalibrated; the prefill, key generation and membership check are outside it |
//! | `arm_symmetry` | the three arms run the same prefill, writers, placement and window; they differ only in the poller, and `len` is the poller-presence control for `mem_used` |
//! | `statistics` | HdrHistogram (3 significant figures) of insert latencies, the 8 largest insert latencies, every reading's offset, duration and value; the driver runs 5 rounds per (cell, arm) as separate processes and computes BCa intervals over rounds |
//! | `verdict` | **PRE-REGISTERED** — METHODOLOGY §44; read by the driver (S1, M1) |

use expanse_trie::strmap::NulFreeStr;
use expanse_trie::sync::SyncExpanseStrMap;
use hdrhistogram::Histogram;
use std::collections::HashMap;
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

const TOP: usize = 8;
const HIST_MAX_NS: u64 = 60_000_000_000;
/// The `short` prefill's seed, the one `short_keys` in `instructions.rs`
/// uses, so the first 50,000 keys are the #1587 arm's population.
const SHORT_SEED: u64 = 0x0DDB_1A5E_5EED_0730;
const ALNUM: &[u8; 62] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

struct XorShift(u64);
impl XorShift {
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

/// Writer `w`'s stream seed: `seed` and `w` through SplitMix64's finaliser
/// (Steele, Lea & Flood, "Fast Splittable Pseudorandom Number Generators",
/// OOPSLA 2014), so no two writers start one stream. A plain `seed ^ (k + w)`
/// differs only in the low bit between neighbouring writers, which
/// `XorShift::new`'s `| 1` then erases.
fn writer_seed(seed: u64, w: u64) -> u64 {
    let mut z = seed.wrapping_add((w + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn nul_free(bytes: &[u8]) -> &NulFreeStr {
    // SAFETY: both generators emit ASCII digits, letters and `/` only, never NUL.
    unsafe { NulFreeStr::new_unchecked(bytes) }
}

/// `routes` key `i`: the `str_keys` shape of `instructions.rs`, 37 bytes
/// below `i = 16,000,000` and 38 from there, where `i / 16` gains a digit.
fn route_key(i: u64, buf: &mut Vec<u8>) {
    use std::io::Write;
    buf.clear();
    write!(buf, "/api/v2/tenants/{:06}/resources/{:04}", i / 16, i % 16).expect("write to Vec");
}

/// One `short` draw: 8 to 16 random alphanumerics, as `short_keys` draws them.
fn short_key(rng: &mut XorShift, buf: &mut Vec<u8>) {
    buf.clear();
    let n = 8 + (rng.next() % 9) as usize;
    buf.extend((0..n).map(|_| ALNUM[(rng.next() % 62) as usize]));
}

fn hist() -> Histogram<u64> {
    Histogram::<u64>::new_with_max(HIST_MAX_NS, 3).expect("histogram bounds")
}

fn push_top(top: &mut Vec<u64>, ns: u64) {
    if top.len() < TOP || ns > *top.last().expect("non-empty") {
        let at = top.partition_point(|&t| t >= ns);
        top.insert(at, ns);
        top.truncate(TOP);
    }
}

/// One writer's record.
struct Ops {
    hist: Histogram<u64>,
    top: Vec<u64>,
    inserted: u64,
    overwrites: u64,
    rejected: u64,
    cpu_ns: Option<u64>,
}

impl Ops {
    fn new() -> Self {
        Self {
            hist: hist(),
            top: Vec::with_capacity(TOP + 1),
            inserted: 0,
            overwrites: 0,
            rejected: 0,
            cpu_ns: None,
        }
    }
    fn merge(&mut self, other: Ops) {
        self.hist.add(other.hist).expect("same bounds");
        for t in other.top {
            push_top(&mut self.top, t);
        }
        self.inserted += other.inserted;
        self.overwrites += other.overwrites;
        self.rejected += other.rejected;
        self.cpu_ns = match (self.cpu_ns, other.cpu_ns) {
            (Some(a), Some(b)) => Some(a + b),
            _ => None,
        };
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

/// The process's peak resident set in bytes (`getrusage`), or `None`.
#[cfg(unix)]
fn max_rss_bytes() -> Option<u64> {
    // SAFETY: `rusage` is plain data; zeroed is a valid initial value.
    let mut ru: libc::rusage = unsafe { core::mem::zeroed() };
    // SAFETY: `ru` is a valid, writable `rusage` for the duration of the call.
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
    if rc != 0 {
        return None;
    }
    let raw = ru.ru_maxrss as u64;
    // Linux reports KiB, macOS bytes.
    Some(if cfg!(target_os = "macos") {
        raw
    } else {
        raw * 1024
    })
}

#[cfg(not(unix))]
fn max_rss_bytes() -> Option<u64> {
    None
}

/// The CPUs this process may run on (`Cpus_allowed_list`), or `None`.
fn cpu_affinity() -> Option<String> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find_map(|l| l.strip_prefix("Cpus_allowed_list:"))
        .map(|v| v.trim().to_string())
}

/// Confines the calling thread to `cpu`, or panics (§8.1: a placement the
/// row records must be the one the thread ran under).
#[cfg(target_os = "linux")]
fn place_thread(cpu: usize) {
    assert!(
        cpu < 8 * core::mem::size_of::<libc::cpu_set_t>(),
        "CPU {cpu} out of range"
    );
    // SAFETY: `cpu_set_t` is plain data; zeroed is the empty set.
    let mut set: libc::cpu_set_t = unsafe { core::mem::zeroed() };
    // SAFETY: `set` is a valid, writable `cpu_set_t` and `cpu` is within it (checked above).
    unsafe { libc::CPU_SET(cpu, &mut set) };
    // SAFETY: pid 0 is the calling thread; `set` outlives the call.
    let rc = unsafe { libc::sched_setaffinity(0, core::mem::size_of::<libc::cpu_set_t>(), &set) };
    assert_eq!(rc, 0, "sched_setaffinity to CPU {cpu} failed");
}

#[cfg(not(target_os = "linux"))]
fn place_thread(_cpu: usize) {
    panic!("--cpus is supported on Linux only");
}

fn parse_cpus(s: &str) -> Vec<usize> {
    s.split(',')
        .map(|c| {
            c.trim()
                .parse()
                .unwrap_or_else(|_| panic!("--cpus: {c:?} is not a CPU number"))
        })
        .collect()
}

#[cfg(feature = "occ-stats")]
fn window_counters(writer_cpu_ns: Option<u64>) -> serde_json::Value {
    use expanse_trie::occ_stats::{self, Stat};
    let snap = occ_stats::snapshot();
    let tsc_hz = occ_stats::cycles_hz(Duration::from_millis(200));
    serde_json::json!({
        "gate_blocked_entries": snap[Stat::GateBlockedEntries as usize],
        "gate_wait_cycles": snap[Stat::GateWaitCycles as usize],
        "quiesce_calls": snap[Stat::QuiesceCalls as usize],
        "quiesce_drain_cycles": snap[Stat::QuiesceDrainCycles as usize],
        "locked_reads": snap[Stat::LockedReads as usize],
        "contention_gate_closed": snap[Stat::ContentionGateClosed as usize],
        "tsc_hz": tsc_hz,
        "writer_cpu_ns": writer_cpu_ns,
    })
}

fn identity(row: &mut serde_json::Value) {
    row["occ_stats"] = cfg!(feature = "occ-stats").into();
    row["cpu_affinity"] = cpu_affinity().into();
}

/// Prefills `m` with `keys` keys of `shape` in draw order.
fn prefill(m: &SyncExpanseStrMap, shape: &str, keys: u64) {
    let mut buf = Vec::with_capacity(40);
    match shape {
        "routes" => {
            for i in 0..keys {
                route_key(i, &mut buf);
                m.insert(nul_free(&buf), i);
            }
        }
        "short" => {
            let mut rng = XorShift(SHORT_SEED);
            let mut i = 0u64;
            while i < keys {
                short_key(&mut rng, &mut buf);
                let key = nul_free(&buf);
                if !m.contains_key(key) {
                    m.insert(key, i);
                    i += 1;
                }
            }
        }
        _ => panic!("unknown --shape {shape}"),
    }
    assert_eq!(
        m.len(),
        keys,
        "the prefill holds exactly --keys distinct keys"
    );
}

#[derive(Clone, Copy, PartialEq)]
enum Arm {
    None,
    Len,
    MemUsed,
}

fn writer_loop(
    m: &SyncExpanseStrMap,
    shape: &str,
    keys: u64,
    w: u64,
    writers: u64,
    seed: u64,
    stop: &AtomicBool,
) -> Ops {
    let mut ops = Ops::new();
    let mut buf = Vec::with_capacity(40);
    let mut rng = XorShift::new(writer_seed(seed, w));
    let mut k = 0u64;
    let c0 = thread_cpu_ns();
    while !stop.load(Ordering::Relaxed) {
        if shape == "routes" {
            route_key(keys + w + k * writers, &mut buf);
            k += 1;
        } else {
            short_key(&mut rng, &mut buf);
        }
        let key = nul_free(&buf);
        if m.contains_key(key) {
            ops.rejected += 1;
            continue;
        }
        let t = Instant::now();
        let prev = m.insert(key, w);
        let ns = t.elapsed().as_nanos() as u64;
        ops.hist
            .record(ns.clamp(1, HIST_MAX_NS - 1))
            .expect("in range");
        push_top(&mut ops.top, ns);
        ops.inserted += 1;
        if prev.is_some() {
            ops.overwrites += 1;
        }
    }
    ops.cpu_ns = match (c0, thread_cpu_ns()) {
        (Some(a), Some(b)) => Some(b.saturating_sub(a)),
        _ => None,
    };
    ops
}

/// The poller's readings: (offset from the window start, duration, value), in ns.
fn poller_loop(
    m: &SyncExpanseStrMap,
    arm: Arm,
    period: Duration,
    t0: Instant,
    window: Duration,
) -> Vec<(u64, u64, u64)> {
    let mut polls = Vec::new();
    let mut due = t0 + period;
    while due < t0 + window {
        let now = Instant::now();
        if due > now {
            std::thread::sleep(due - now);
        }
        let t = Instant::now();
        let v = match arm {
            Arm::Len => black_box(m).len(),
            Arm::MemUsed => black_box(m).mem_used() as u64,
            Arm::None => unreachable!("the none arm runs no poller"),
        };
        let ns = t.elapsed().as_nanos() as u64;
        polls.push(((t - t0).as_nanos() as u64, ns, black_box(v)));
        // Fixed schedule; an overrun leaves the next reading due on return.
        due = (due + period).max(Instant::now());
    }
    polls
}

fn main() {
    let a = Args::parse();
    let mode = a.get("mode").to_string();
    let mut row = serde_json::json!({"workload_id": "strmap_poll_stall", "mode": mode});
    identity(&mut row);
    if mode == "identify" {
        println!("{row}");
        return;
    }
    let shape = a.get("shape").to_string();
    let keys = a.num("keys");
    row["shape"] = shape.clone().into();
    row["keys"] = keys.into();
    let m = Arc::new(SyncExpanseStrMap::new());
    let p0 = Instant::now();
    prefill(&m, &shape, keys);
    row["prefill_s"] = p0.elapsed().as_secs_f64().into();
    row["len_before"] = m.len().into();
    row["mem_used_before"] = (m.mem_used() as u64).into();
    if mode == "census" {
        row["max_rss_bytes"] = max_rss_bytes().into();
        println!("{row}");
        return;
    }
    assert_eq!(mode, "run", "unknown --mode {mode}");
    let writers = a.num("writers");
    assert!(writers >= 1, "--writers must be at least 1");
    let arm = match a.get("arm") {
        "none" => Arm::None,
        "len" => Arm::Len,
        "mem_used" => Arm::MemUsed,
        other => panic!("unknown --arm {other}"),
    };
    let period_ms = a.num("period-ms");
    let seconds = a.num("seconds");
    let round = a.num("round");
    let seed = a.num("seed");
    assert!(
        period_ms >= 1 && seconds >= 1,
        "--period-ms and --seconds must be positive"
    );
    let cpus = a.0.get("cpus").map(|s| parse_cpus(s));
    if let Some(c) = &cpus {
        assert!(
            c.len() > writers as usize,
            "--cpus needs the poller's CPU and one per writer"
        );
    }
    row["writers"] = writers.into();
    row["arm"] = a.get("arm").into();
    row["period_ms"] = period_ms.into();
    row["seconds"] = seconds.into();
    row["round"] = round.into();
    row["seed"] = seed.into();
    row["placement"] = match &cpus {
        Some(c) => serde_json::json!({"poller": c[0], "writers": &c[1..=writers as usize]}),
        None => serde_json::Value::Null,
    };

    #[cfg(feature = "occ-stats")]
    expanse_trie::occ_stats::reset();
    let stop = Arc::new(AtomicBool::new(false));
    let has_poller = arm != Arm::None;
    let barrier = Arc::new(Barrier::new(writers as usize + 1 + usize::from(has_poller)));
    let start = Arc::new(std::sync::OnceLock::<Instant>::new());
    let window = Duration::from_secs(seconds);
    let whs: Vec<_> = (0..writers)
        .map(|w| {
            let (m, stop, b) = (Arc::clone(&m), Arc::clone(&stop), Arc::clone(&barrier));
            let shape = shape.clone();
            let cpu = cpus.as_ref().map(|c| c[1 + w as usize]);
            std::thread::spawn(move || {
                if let Some(cpu) = cpu {
                    place_thread(cpu);
                }
                b.wait();
                writer_loop(&m, &shape, keys, w, writers, seed, &stop)
            })
        })
        .collect();
    let ph = has_poller.then(|| {
        let (m, b, start) = (Arc::clone(&m), Arc::clone(&barrier), Arc::clone(&start));
        let cpu = cpus.as_ref().map(|c| c[0]);
        let period = Duration::from_millis(period_ms);
        std::thread::spawn(move || {
            if let Some(cpu) = cpu {
                place_thread(cpu);
            }
            b.wait();
            let t0 = *start.wait();
            poller_loop(&m, arm, period, t0, window)
        })
    });
    barrier.wait();
    let t0 = Instant::now();
    start.set(t0).expect("set once");
    std::thread::sleep(window);
    stop.store(true, Ordering::Relaxed);
    let mut ops = Ops::new();
    ops.cpu_ns = Some(0);
    for h in whs {
        ops.merge(h.join().expect("writer panicked"));
    }
    let elapsed = t0.elapsed().as_secs_f64();
    let polls = ph.map(|h| h.join().expect("poller panicked"));
    #[cfg(feature = "occ-stats")]
    {
        row["counters"] = window_counters(ops.cpu_ns);
    }
    row["elapsed_s"] = elapsed.into();
    row["insert"] = percentiles(&ops.hist);
    row["insert_top_ns"] = ops.top.clone().into();
    row["inserts"] = ops.inserted.into();
    row["overwrites"] = ops.overwrites.into();
    row["rejected_present"] = ops.rejected.into();
    row["writer_cpu_ns"] = ops.cpu_ns.into();
    row["polls"] = match &polls {
        Some(p) => p
            .iter()
            .map(|&(at, ns, v)| serde_json::json!([at, ns, v]))
            .collect::<Vec<_>>()
            .into(),
        None => serde_json::Value::Array(Vec::new()),
    };
    row["polls_fields"] = serde_json::json!(["at_ns", "duration_ns", "value"]);
    row["len_after"] = m.len().into();
    row["mem_used_after"] = (m.mem_used() as u64).into();
    row["max_rss_bytes"] = max_rss_bytes().into();
    println!("{row}");
}
