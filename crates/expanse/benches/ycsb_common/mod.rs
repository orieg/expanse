//! Shared code for the single-threaded YCSB harnesses (#1005).
//!
//! Two harness files drive this module, one per key shape, because a harness
//! file declares exactly one `# Workload shape` table (AGENTS.md §8.15):
//!
//! * `benches/ycsb.rs` — uniform-random 64-bit keys (`workload_ycsb`).
//! * `benches/ycsb_dense.rs` — dense clustered keys (`workload_ycsb_dense`).
//!
//! This file has no entry point and no shape table of its own; it is a helper
//! module in the sense `search_common` and `zset_common` are. Everything a
//! harness measures lives here so the two shapes cannot drift apart: the
//! generators, the operation streams, the four engine runners, the criterion
//! groups and the rounds/latency report.
//!
//! Engines:
//! - [`ExpanseMap`] (pure 64-bit digital trie, `u64` values)
//! - [`ExpanseBlobMap`] (inlined value slots + chunked slab arena, 128 B blobs)
//! - [`std::collections::BTreeMap`] (boxed 128 B blobs)
//! - [`crossbeam_skiplist::SkipMap`] (RocksDB in-memory MemTable model)
//!
//! # Workload Specifications
//! - **Workload A (Update Heavy)**: 50% Read, 50% Update (Zipfian key distribution)
//! - **Workload B (Read Mostly)**: 95% Read, 5% Update (Zipfian key distribution)
//! - **Workload C (Read Only)**: 100% Read (Zipfian key distribution)
//! - **Workload D (Read Latest)**: 95% Read, 5% Insert. Reads are Zipfian over
//!   recency: rank 0 is the most recently inserted key *including the keys this
//!   stream has inserted so far*, then older in-run inserts, then the initial
//!   population from the end of its canonical draw order backwards.
//! - **Workload E (Short Range Scans)**: 95% Scan (10..100 items with predicate filtering), 5% Insert
//! - **Workload F (Read-Modify-Write)**: 50% Read, 50% RMW (Zipfian key distribution)
//!
//! # What varies per run, and is recorded per row
//! - **Population**: 100k by default; `YCSB_POPULATIONS=100k,1m,10m` selects
//!   others. The large populations are opt-in by environment so that the smoke
//!   path `cargo test --bench` takes stays at 100k.
//! - **Insertion order**: every cell is built twice, once with the population
//!   sorted ascending and once Fisher–Yates shuffled from the suite PRNG
//!   (AGENTS.md §8.12.4). `BTreeMap` is order-sensitive — ascending insertion
//!   is its rightmost-append path — so a single order would publish one regime
//!   as if it were the structure. The operation stream is generated from the
//!   canonical (generator-order) key vector and is byte-identical across the
//!   two build orders, so build order is the only variable between them.
//! - **Zipfian skew**: $\theta = 0.99$.
//!
//! # Latency sampling
//! Percentiles come from **batch-sampled windows**: the stream is cut into
//! windows of `YCSB_LATENCY_WINDOW` ops (default 64), one `Instant` pair times
//! each window, and the window's elapsed time divided by its op count is one
//! sample. No op is bracketed individually — a per-op `Instant::now()` /
//! `elapsed()` pair costs about as much as the point lookups it would time.
//! The price is stated rather than hidden: a percentile here is a percentile of
//! *window means*, not of single-op latencies, so one stalled op inside a
//! 64-op window moves that window's sample by 1/64 of the stall. The report
//! prints the calibrated bracket cost and the residual per-op overhead
//! (bracket / window) on every run. A TSC read converted by a measured
//! `tsc_hz` is the alternative the issue names; it is not implemented here
//! because it is x86-only and this harness also runs on aarch64.

#![allow(dead_code)]

use crossbeam_skiplist::SkipMap;
use expanse_trie::blobmap::ExpanseBlobMap;
use expanse_trie::map::ExpanseMap;
use expanse_trie::sync::SyncExpanseMap;
use std::collections::BTreeMap;
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use criterion::{BenchmarkId, Criterion, Throughput};

/// Seeded fast 64-bit XorShift pseudorandom number generator for deterministic benchmarks.
#[derive(Clone, Debug)]
pub struct XorShift64(u64);

impl XorShift64 {
    /// Creates a new PRNG instance with given non-zero seed.
    #[inline(always)]
    pub fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0x5CA1_AB1E_0001 } else { seed })
    }

    /// Generates the next pseudorandom 64-bit integer.
    #[inline(always)]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Generates a floating-point value uniformly distributed in `[0.0, 1.0)`.
    #[inline(always)]
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Generates an integer uniformly distributed in `[min, max]`.
    #[inline(always)]
    pub fn gen_range(&mut self, min: u64, max: u64) -> u64 {
        if min >= max {
            return min;
        }
        let span = max - min + 1;
        min + (self.next_u64() % span)
    }
}

/// Standard YCSB Zipfian generator ($\theta = 0.99$, $N = 100,000$).
///
/// Implements Gray et al.'s algorithm: generates skewed item ranks where
/// rank 0 is accessed with the highest probability, followed by rank 1, etc.
#[derive(Clone, Debug)]
pub struct ZipfianGenerator {
    n: u64,
    theta: f64,
    zeta_n: f64,
    alpha: f64,
    eta: f64,
}

impl ZipfianGenerator {
    /// Creates a new Zipfian distribution generator over `n` items with skew `theta`.
    pub fn new(n: u64, theta: f64) -> Self {
        assert!(n > 0, "n must be positive");
        let zeta_2 = Self::zeta(2, theta);
        let zeta_n = Self::zeta(n, theta);
        let alpha = 1.0 / (1.0 - theta);
        let eta = (1.0 - (2.0 / n as f64).powf(1.0 - theta)) / (1.0 - zeta_2 / zeta_n);
        Self {
            n,
            theta,
            zeta_n,
            alpha,
            eta,
        }
    }

    /// Evaluates Hurwitz zeta function $\zeta(n, \theta) = \sum_{i=1}^n i^{-\theta}$.
    fn zeta(n: u64, theta: f64) -> f64 {
        let mut sum = 0.0;
        for i in 1..=n {
            sum += 1.0 / (i as f64).powf(theta);
        }
        sum
    }

    /// Generates the next item rank in `[0, n - 1]` using the given uniform random float $u \in [0, 1)$.
    #[inline(always)]
    pub fn next(&self, u: f64) -> u64 {
        let uz = u * self.zeta_n;
        if uz < 1.0 {
            return 0;
        }
        if uz < 1.0 + 0.5f64.powf(self.theta) {
            return 1;
        }
        let k = (self.n as f64 * (self.eta * u - self.eta + 1.0).powf(self.alpha)) as u64;
        k.min(self.n - 1)
    }
}

/// YCSB Operation types.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum YcsbOp {
    /// Point read by key.
    Read(u64),
    /// Record update on existing key.
    Update(u64, u64),
    /// Record insert for new key.
    Insert(u64, u64),
    /// Range scan starting at key for given length.
    Scan(u64, usize),
    /// Atomic Read-Modify-Write.
    ReadModifyWrite(u64),
}

/// Workload identifier.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Workload {
    /// 50% Read, 50% Update
    A,
    /// 95% Read, 5% Update
    B,
    /// 100% Read
    C,
    /// 95% Read, 5% Insert (Latest record append)
    D,
    /// 95% Short Range Scan, 5% Insert
    E,
    /// 50% Read, 50% Read-Modify-Write
    F,
}

impl Workload {
    /// Returns the descriptive workload label.
    pub fn name(self) -> &'static str {
        match self {
            Self::A => "Workload A (50% Read, 50% Update)",
            Self::B => "Workload B (95% Read, 5% Update)",
            Self::C => "Workload C (100% Read)",
            Self::D => "Workload D (95% Read Latest, 5% Insert)",
            Self::E => "Workload E (95% Scan 10..100, 5% Insert)",
            Self::F => "Workload F (50% Read, 50% Read-Modify-Write)",
        }
    }

    /// Returns short tag identifier.
    pub fn tag(self) -> &'static str {
        match self {
            Self::A => "workload_a",
            Self::B => "workload_b",
            Self::C => "workload_c",
            Self::D => "workload_d",
            Self::E => "workload_e",
            Self::F => "workload_f",
        }
    }
}

/// Default population size: $N = 100,000$. Other populations are selected per
/// run through `YCSB_POPULATIONS` and recorded in every result row.
pub const POPULATION_N: usize = 100_000;
/// Default Zipfian skew $\theta = 0.99$.
pub const ZIPFIAN_THETA: f64 = 0.99;
/// Operations per criterion iteration.
pub const CRITERION_OP_COUNT: usize = 20_000;
/// Operations per cell in the rounds/latency report. One stream serves both the
/// untimed-window throughput pass and the window-sampled latency pass, so the
/// two columns of a row describe the same work.
pub const REPORT_OP_COUNT: usize = 200_000;
/// Default ops per latency window (see the module doc, "Latency sampling").
pub const LATENCY_WINDOW: usize = 64;
/// Standard 128-byte payload size for database blob modeling.
pub const BLOB_PAYLOAD_SIZE: usize = 128;
/// Deterministic operation-stream seed (shared by criterion and the report).
pub const REPORT_SEED: u64 = 0x1234_5678_9ABC;
/// Seed of the population generator, for both key shapes.
pub const KEY_SEED: u64 = 0x0DDB_1A5E_5EED_0001;
/// Seed of the Fisher–Yates build-order permutation.
pub const SHUFFLE_SEED: u64 = 0x5EED_0F0F_1005_0001;
/// First key of the in-run insert sequence (workloads D and E).
pub const INSERT_SEQ_BASE: u64 = 0x8000_0000_0000_0000;
/// `ExpanseBlobMap`'s hot-metadata field is 24 bits wide; a wider value is an
/// error, not a truncation, so the harness masks what it passes.
pub const BLOB_META_MASK: u32 = expanse_trie::slot::ValueSlot::ARENA_META_MAX;
/// Keys per run in the dense clustered shape — the `clustered` class of
/// `benches/compare.rs`: 256 consecutive keys at a random base.
pub const CLUSTER_RUN: u64 = 256;

/// The key distribution a harness file measures.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum KeyShape {
    /// Uniform-random 64-bit keys.
    UniformRandom,
    /// Runs of [`CLUSTER_RUN`] consecutive keys at uniform-random bases.
    DenseClustered,
}

impl KeyShape {
    /// Tag recorded in every result row.
    pub fn tag(self) -> &'static str {
        match self {
            Self::UniformRandom => "uniform_random",
            Self::DenseClustered => "dense_clustered",
        }
    }
}

/// The order the population is inserted in before the timed region.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InsertionOrder {
    /// Sorted ascending: a B-tree's rightmost-append path.
    Sorted,
    /// Fisher–Yates permutation from [`SHUFFLE_SEED`].
    Shuffled,
}

impl InsertionOrder {
    /// Both orders, in the sequence every harness measures them.
    pub const BOTH: [Self; 2] = [Self::Sorted, Self::Shuffled];

    /// Tag recorded in every result row.
    pub fn tag(self) -> &'static str {
        match self {
            Self::Sorted => "sorted",
            Self::Shuffled => "shuffled",
        }
    }
}

/// What a harness file hands this module: its declared workload id and shape.
#[derive(Copy, Clone, Debug)]
pub struct Suite {
    /// The `workload_id` of the harness file's `# Workload shape` table.
    pub id: &'static str,
    /// Key shape the harness measures.
    pub shape: KeyShape,
}

/// Generates the canonical population of `n` keys for `shape`, in generator
/// draw order. The operation stream is built from this vector; the build order
/// is a permutation of it ([`build_order`]).
pub fn generate_keys(shape: KeyShape, n: usize) -> Vec<u64> {
    let mut rng = XorShift64::new(KEY_SEED);
    let mut keys = Vec::with_capacity(n);
    match shape {
        KeyShape::UniformRandom => {
            for _ in 0..n {
                keys.push(rng.next_u64());
            }
        }
        KeyShape::DenseClustered => {
            let mut base = 0u64;
            for i in 0..n as u64 {
                if i.is_multiple_of(CLUSTER_RUN) {
                    // Clear the low byte for the run, and the top bit so a base
                    // can never land in the in-run insert sequence.
                    base = rng.next_u64() & !(CLUSTER_RUN - 1) & !INSERT_SEQ_BASE;
                }
                keys.push(base + (i % CLUSTER_RUN));
            }
        }
    }
    keys
}

/// Generates the uniform-random population (the shape `benches/ycsb.rs` measures).
pub fn generate_initial_keys(n: usize) -> Vec<u64> {
    generate_keys(KeyShape::UniformRandom, n)
}

/// The population in the order it is inserted.
pub fn build_order(keys: &[u64], order: InsertionOrder) -> Vec<u64> {
    let mut out = keys.to_vec();
    match order {
        InsertionOrder::Sorted => out.sort_unstable(),
        InsertionOrder::Shuffled => {
            let mut rng = XorShift64::new(SHUFFLE_SEED);
            for i in (1..out.len()).rev() {
                let j = rng.gen_range(0, i as u64) as usize;
                out.swap(i, j);
            }
        }
    }
    out
}

/// Generates a standardized 128-byte payload for a given key.
pub fn generate_payload(key: u64) -> [u8; BLOB_PAYLOAD_SIZE] {
    let mut buf = [0u8; BLOB_PAYLOAD_SIZE];
    let kb = key.to_be_bytes();
    for (i, b) in buf.iter_mut().enumerate() {
        *b = kb[i % 8] ^ (i as u8);
    }
    buf
}

/// Generates a sequence of pre-computed YCSB operations for a given workload.
///
/// `initial_keys` is the canonical (generator-order) population. Zipfian rank
/// `r` maps to `initial_keys[r]`, so which keys are hot does not depend on the
/// order the structure was built in.
pub fn generate_operations(
    workload: Workload,
    initial_keys: &[u64],
    op_count: usize,
    seed: u64,
) -> Vec<YcsbOp> {
    let n = initial_keys.len();
    let zipf = ZipfianGenerator::new(n as u64, ZIPFIAN_THETA);
    let mut rng = XorShift64::new(seed);
    let mut ops = Vec::with_capacity(op_count);
    let mut next_insert_seq = INSERT_SEQ_BASE;
    // Keys this stream has inserted so far, oldest first (workload D reads them).
    let mut inserted: Vec<u64> = Vec::new();

    for _ in 0..op_count {
        let prob = rng.gen_range(0, 99);
        let u = rng.next_f64();
        let rank = zipf.next(u) as usize;
        let key = initial_keys[rank.min(n - 1)];

        let op = match workload {
            Workload::A => {
                // 50% Read, 50% Update
                if prob < 50 {
                    YcsbOp::Read(key)
                } else {
                    YcsbOp::Update(key, rng.next_u64())
                }
            }
            Workload::B => {
                // 95% Read, 5% Update
                if prob < 95 {
                    YcsbOp::Read(key)
                } else {
                    YcsbOp::Update(key, rng.next_u64())
                }
            }
            Workload::C => {
                // 100% Read
                YcsbOp::Read(key)
            }
            Workload::D => {
                // 95% Read Latest, 5% Insert
                if prob < 95 {
                    // Zipfian over recency. Rank 0 is the newest key there is:
                    // the in-run inserts first, newest to oldest, then the
                    // initial population from the end of its draw order.
                    let latest_rank = zipf.next(rng.next_f64()) as usize;
                    let target_key = if latest_rank < inserted.len() {
                        inserted[inserted.len() - 1 - latest_rank]
                    } else {
                        initial_keys[n - 1 - ((latest_rank - inserted.len()) % n)]
                    };
                    YcsbOp::Read(target_key)
                } else {
                    next_insert_seq = next_insert_seq.wrapping_add(1);
                    inserted.push(next_insert_seq);
                    YcsbOp::Insert(next_insert_seq, rng.next_u64())
                }
            }
            Workload::E => {
                // 95% Range Scan (10..100 items), 5% Insert
                if prob < 95 {
                    let scan_len = rng.gen_range(10, 100) as usize;
                    YcsbOp::Scan(key, scan_len)
                } else {
                    next_insert_seq = next_insert_seq.wrapping_add(1);
                    YcsbOp::Insert(next_insert_seq, rng.next_u64())
                }
            }
            Workload::F => {
                // 50% Read, 50% Read-Modify-Write
                if prob < 50 {
                    YcsbOp::Read(key)
                } else {
                    YcsbOp::ReadModifyWrite(key)
                }
            }
        };
        ops.push(op);
    }
    ops
}

/// How a runner samples latency.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Sampling {
    /// One window spanning the whole stream: throughput only, no samples.
    Off,
    /// Windows of this many ops; each contributes one window-mean sample.
    Windows(usize),
}

/// A window counts as slow when its mean exceeds this multiple of the median.
/// A classification for locating a tail, not a published threshold.
pub const SLOW_WINDOW_FACTOR: f64 = 8.0;

/// Window-mean latency statistics (see the module doc, "Latency sampling").
#[derive(Clone, Debug, Default)]
pub struct LatencyStats {
    /// Total operations executed.
    pub count: usize,
    /// Number of window samples the percentiles were taken over (0 when off).
    pub windows: usize,
    /// Ops per full window (0 when sampling was off).
    pub window_ops: usize,
    /// Smallest window mean, ns/op.
    pub min_ns: f64,
    /// Largest window mean, ns/op.
    pub max_ns: f64,
    /// Median window mean, ns/op.
    pub p50_ns: f64,
    /// 90th percentile window mean, ns/op.
    pub p90_ns: f64,
    /// 95th percentile window mean, ns/op.
    pub p95_ns: f64,
    /// 99th percentile window mean, ns/op.
    pub p99_ns: f64,
    /// 99.9th percentile window mean, ns/op.
    pub p999_ns: f64,
    /// Windows whose mean exceeds [`SLOW_WINDOW_FACTOR`] × the median.
    pub slow_windows: usize,
    /// Mean latency over the whole stream, ns/op (elapsed / count).
    pub mean_ns: f64,
    /// Seconds the op loop took, window brackets included when sampling.
    pub elapsed_s: f64,
    /// Operations per second over the whole stream.
    pub ops_per_sec: f64,
}

impl LatencyStats {
    /// Summarises window-mean samples (ns/op) and the stream's total duration.
    pub fn compute(
        mut samples: Vec<f64>,
        count: usize,
        window_ops: usize,
        total_duration: Duration,
    ) -> Self {
        let secs = total_duration.as_secs_f64();
        let mut out = Self {
            count,
            elapsed_s: secs,
            mean_ns: if count > 0 {
                secs * 1e9 / count as f64
            } else {
                0.0
            },
            ops_per_sec: if secs > 0.0 { count as f64 / secs } else { 0.0 },
            ..Self::default()
        };
        if samples.is_empty() {
            return out;
        }
        samples.sort_by(f64::total_cmp);
        let w = samples.len();
        let at = |num: usize, den: usize| samples[((w * num) / den).min(w - 1)];
        out.windows = w;
        out.window_ops = window_ops;
        out.min_ns = samples[0];
        out.max_ns = samples[w - 1];
        out.p50_ns = at(50, 100);
        out.p90_ns = at(90, 100);
        out.p95_ns = at(95, 100);
        out.p99_ns = at(99, 100);
        out.p999_ns = at(999, 1000);
        let cut = out.p50_ns * SLOW_WINDOW_FACTOR;
        out.slow_windows = samples.iter().filter(|&&s| s > cut).count();
        out
    }
}

/// What one runner call produced.
#[derive(Clone, Debug, Default)]
pub struct RunOutcome {
    /// Throughput and, when sampled, window-mean percentiles.
    pub stats: LatencyStats,
    /// Resident bytes after the stream (measured for Expanse, modelled otherwise).
    pub mem_bytes: usize,
    /// Work checksum: reads and RMWs that found their key, plus records a scan
    /// consumed. Identical across engines for one stream over one population —
    /// a differing count means two arms did different work (AGENTS.md §8.3).
    /// Summing it is also the loop's dead-code-elimination sink.
    pub consumed: u64,
}

/// Drives `apply` over `ops` in windows. The only timers are one `Instant` pair
/// around the whole stream and, when sampling, one pair per window.
#[inline(always)]
fn drive<F: FnMut(YcsbOp) -> u64>(
    ops: &[YcsbOp],
    sampling: Sampling,
    mut apply: F,
) -> (LatencyStats, u64) {
    let (window, record) = match sampling {
        Sampling::Off => (ops.len().max(1), false),
        Sampling::Windows(w) => (w.max(1), true),
    };
    let mut samples: Vec<f64> = if record {
        Vec::with_capacity(ops.len() / window + 1)
    } else {
        Vec::new()
    };
    let mut consumed = 0u64;
    let start_all = Instant::now();
    if record {
        for chunk in ops.chunks(window) {
            let t0 = Instant::now();
            for &op in chunk {
                consumed = consumed.wrapping_add(apply(op));
            }
            samples.push(t0.elapsed().as_nanos() as f64 / chunk.len() as f64);
        }
    } else {
        for &op in ops {
            consumed = consumed.wrapping_add(apply(op));
        }
    }
    let elapsed = start_all.elapsed();
    black_box(consumed);
    (
        LatencyStats::compute(samples, ops.len(), if record { window } else { 0 }, elapsed),
        consumed,
    )
}

// ---------------------------------------------------------------------------
// Workload E scan bound — record-count terminated, not key-width terminated
// ---------------------------------------------------------------------------
//
// YCSB Workload E is specified as "scan up to N records from a starting key".
// The bound is a RECORD COUNT, so every arm iterates from `start_k` forward
// and stops when `len` predicate-matching records have been collected. A
// key-width window (`start_k ..= start_k + len * 1000`) would make E a seek
// benchmark on uniform-random keys: at N = 100 000 such a window is expected
// to hold 3e-10 keys beyond the start key.
//
// Scan-length semantics: `rng.gen_range(10, 100)` records.
//
// ---------------------------------------------------------------------------
// Workload E scan predicate — identical selectivity by construction
// ---------------------------------------------------------------------------
//
// Every engine's scan arm filters entries by KEY parity: `k & 1 == 0`. The
// predicate depends only on the key — never on an engine's value
// representation — so the pass/fail decision is identical for the same key
// across all four arms. It passes ~50% of entries on uniform-random keys and
// every other key inside a dense run.

/// Execution runner for `ExpanseMap`.
pub fn run_workload_expanse_map(
    map: &mut ExpanseMap,
    ops: &[YcsbOp],
    sampling: Sampling,
) -> RunOutcome {
    let (stats, consumed) = drive(ops, sampling, |op| match op {
        YcsbOp::Read(k) => {
            let v = map.get(k);
            black_box(v);
            v.is_some() as u64
        }
        YcsbOp::Update(k, v) | YcsbOp::Insert(k, v) => {
            map.insert(k, v);
            0
        }
        YcsbOp::Scan(start_k, len) => {
            let mut count = 0;
            // Record-count bound (see WORKLOAD E note above).
            for (k, v) in map.range(start_k..=u64::MAX) {
                // Key-parity predicate (see WORKLOAD E note above).
                if k & 1 == 0 {
                    black_box((k, v));
                    count += 1;
                    if count >= len {
                        break;
                    }
                }
            }
            count as u64
        }
        YcsbOp::ReadModifyWrite(k) => {
            let old = map.get(k);
            map.insert(k, old.unwrap_or(0).wrapping_add(1));
            old.is_some() as u64
        }
    });
    RunOutcome {
        stats,
        mem_bytes: map.mem_used(),
        consumed,
    }
}

/// Execution runner for `ExpanseBlobMap`.
pub fn run_workload_expanse_blobmap(
    map: &mut ExpanseBlobMap,
    ops: &[YcsbOp],
    payload: &[u8],
    sampling: Sampling,
) -> RunOutcome {
    let (stats, consumed) = drive(ops, sampling, |op| match op {
        YcsbOp::Read(k) => {
            if let Some((view, _)) = map.get(k) {
                black_box(if view.is_empty() { 0 } else { view[0] });
                1
            } else {
                black_box(0);
                0
            }
        }
        YcsbOp::Update(k, meta) | YcsbOp::Insert(k, meta) => {
            // The op carries 64 random bits and the arena's hot metadata is a
            // 24-bit field. An unmasked value is rejected with `MetaOverflow`
            // before anything is written, and discarding that `Result` turned
            // 255 of every 256 blob writes into a no-op (#1005).
            map.insert(k, payload, meta as u32 & BLOB_META_MASK)
                .expect("ExpanseBlobMap rejected a write");
            0
        }
        YcsbOp::Scan(start_k, len) => {
            let mut count = 0;
            map.scan_filtered(
                // Record-count bound (see WORKLOAD E note above).
                start_k..=u64::MAX,
                // Key-parity predicate (see WORKLOAD E note above).
                |k, _meta| k & 1 == 0,
                |k, view, meta| {
                    let b0 = if view.is_empty() { 0 } else { view[0] };
                    black_box((k, b0, meta));
                    count += 1;
                    count < len
                },
            );
            count as u64
        }
        YcsbOp::ReadModifyWrite(k) => {
            let current = map.get(k).map(|(_, m)| m);
            map.insert(
                k,
                payload,
                current.unwrap_or(0).wrapping_add(1) & BLOB_META_MASK,
            )
            .expect("ExpanseBlobMap rejected a write");
            current.is_some() as u64
        }
    });
    RunOutcome {
        stats,
        mem_bytes: map.mem_used(),
        consumed,
    }
}

/// Execution runner for `BTreeMap`.
pub fn run_workload_btreemap(
    map: &mut BTreeMap<u64, Box<[u8]>>,
    ops: &[YcsbOp],
    payload: &[u8],
    sampling: Sampling,
) -> RunOutcome {
    let (stats, consumed) = drive(ops, sampling, |op| match op {
        YcsbOp::Read(k) => {
            if let Some(v) = map.get(&k) {
                black_box(if v.is_empty() { 0 } else { v[0] });
                1
            } else {
                black_box(0);
                0
            }
        }
        YcsbOp::Update(k, _) | YcsbOp::Insert(k, _) => {
            map.insert(k, payload.to_vec().into_boxed_slice());
            0
        }
        YcsbOp::Scan(start_k, len) => {
            let mut count = 0;
            // Record-count bound (see WORKLOAD E note above).
            for (&k, v) in map.range(start_k..=u64::MAX) {
                // Key-parity predicate (see WORKLOAD E note above).
                if k & 1 == 0 {
                    let b0 = if v.is_empty() { 0 } else { v[0] };
                    black_box((k, b0));
                    count += 1;
                    if count >= len {
                        break;
                    }
                }
            }
            count as u64
        }
        YcsbOp::ReadModifyWrite(k) => {
            if let Some(val) = map.get_mut(&k) {
                if !val.is_empty() {
                    val[0] = val[0].wrapping_add(1);
                }
                1
            } else {
                map.insert(k, payload.to_vec().into_boxed_slice());
                0
            }
        }
    });
    // ESTIMATED, not measured (#375): std `BTreeMap` exposes no allocator
    // accounting and this harness has no allocator hook on external crates,
    // so this is a hand model — ~48 B amortized per-entry node overhead
    // (B-tree node headers, key + value slots, typical fill factor) plus
    // the boxed payload allocation plus ~16 B `Box`/allocator rounding.
    // Every report marks these figures as estimates; the Expanse arms report
    // measured `mem_used()` figures.
    let entry_overhead = 48 + payload.len() + 16;
    RunOutcome {
        stats,
        mem_bytes: map.len() * entry_overhead,
        consumed,
    }
}

/// Execution runner for `SkipMap` (RocksDB MemTable model).
pub fn run_workload_skipmap(
    map: &SkipMap<u64, Box<[u8]>>,
    ops: &[YcsbOp],
    payload: &[u8],
    sampling: Sampling,
) -> RunOutcome {
    let (stats, consumed) = drive(ops, sampling, |op| match op {
        YcsbOp::Read(k) => {
            if let Some(entry) = map.get(&k) {
                let v = entry.value();
                black_box(if v.is_empty() { 0 } else { v[0] });
                1
            } else {
                black_box(0);
                0
            }
        }
        YcsbOp::Update(k, _) | YcsbOp::Insert(k, _) => {
            map.insert(k, payload.to_vec().into_boxed_slice());
            0
        }
        YcsbOp::Scan(start_k, len) => {
            let mut count = 0;
            // Record-count bound (see WORKLOAD E note above).
            for entry in map.range(start_k..=u64::MAX) {
                let k: u64 = *entry.key();
                let v: &[u8] = entry.value();
                // Key-parity predicate (see WORKLOAD E note above).
                if k & 1 == 0 {
                    let b0 = if v.is_empty() { 0 } else { v[0] };
                    black_box((k, b0));
                    count += 1;
                    if count >= len {
                        break;
                    }
                }
            }
            count as u64
        }
        YcsbOp::ReadModifyWrite(k) => {
            if let Some(entry) = map.get(&k) {
                let mut cloned: Box<[u8]> = entry.value().clone();
                if !cloned.is_empty() {
                    cloned[0] = cloned[0].wrapping_add(1);
                }
                map.insert(k, cloned);
                1
            } else {
                map.insert(k, payload.to_vec().into_boxed_slice());
                0
            }
        }
    });
    // ESTIMATED, not measured (#375): `crossbeam_skiplist` exposes no
    // allocator accounting and this harness has no allocator hook on
    // external crates, so this is a hand model — ~64 B amortized per
    // tower node (entry header + expected level pointers) plus the boxed
    // payload allocation plus ~16 B `Box`/allocator rounding.
    let entry_overhead = 64 + payload.len() + 16;
    RunOutcome {
        stats,
        mem_bytes: map.len() * entry_overhead,
        consumed,
    }
}

// ---------------------------------------------------------------------------
// Builders — always outside a timed region
// ---------------------------------------------------------------------------

/// Builds the `ExpanseMap` arm from `keys` in the order given.
pub fn build_expanse_map(keys: &[u64]) -> ExpanseMap {
    let mut m = ExpanseMap::new();
    for &k in keys {
        m.insert(k, k ^ 0x5CA1_AB1E);
    }
    m
}

/// Builds the `ExpanseBlobMap` arm from `keys` in the order given.
pub fn build_blobmap(keys: &[u64], payload: &[u8]) -> ExpanseBlobMap {
    let mut m = ExpanseBlobMap::new();
    for &k in keys {
        m.insert(k, payload, (k & 0xFF) as u32)
            .expect("ExpanseBlobMap rejected a build insert");
    }
    m
}

/// Builds the `BTreeMap` arm from `keys` in the order given.
pub fn build_btreemap(keys: &[u64], payload: &[u8]) -> BTreeMap<u64, Box<[u8]>> {
    let mut m = BTreeMap::new();
    for &k in keys {
        m.insert(k, payload.to_vec().into_boxed_slice());
    }
    m
}

/// Builds the `SkipMap` arm from `keys` in the order given.
pub fn build_skipmap(keys: &[u64], payload: &[u8]) -> SkipMap<u64, Box<[u8]>> {
    let m = SkipMap::new();
    for &k in keys {
        m.insert(k, payload.to_vec().into_boxed_slice());
    }
    m
}

/// The four arms, in their canonical order.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Engine {
    /// `ExpanseMap`, `u64` values.
    Trie,
    /// `ExpanseBlobMap`, 128 B blobs.
    Blob,
    /// `BTreeMap<u64, Box<[u8]>>`, 128 B blobs.
    BTree,
    /// `SkipMap<u64, Box<[u8]>>`, 128 B blobs.
    Skip,
}

impl Engine {
    /// Every arm.
    pub const ALL: [Self; 4] = [Self::Trie, Self::Blob, Self::BTree, Self::Skip];

    /// Row label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Trie => "ExpanseMap (u64)",
            Self::Blob => "ExpanseBlobMap (128B)",
            Self::BTree => "BTreeMap (128B)",
            Self::Skip => "SkipMap (128B)",
        }
    }

    /// Whether `mem_bytes` is a hand model rather than a measurement.
    pub fn mem_is_estimate(self) -> bool {
        matches!(self, Self::BTree | Self::Skip)
    }
}

/// Builds one arm, runs one stream over it, and drops it — build and drop both
/// outside the timed region, which is the op loop inside the runner.
pub fn run_cell(
    engine: Engine,
    build_keys: &[u64],
    ops: &[YcsbOp],
    payload: &[u8],
    sampling: Sampling,
) -> RunOutcome {
    match engine {
        Engine::Trie => {
            let mut m = build_expanse_map(build_keys);
            // Deterministic invariant: a population with a duplicate key would
            // silently shrink every arm.
            assert_eq!(
                m.len() as usize,
                build_keys.len(),
                "population not distinct"
            );
            run_workload_expanse_map(&mut m, ops, sampling)
        }
        Engine::Blob => {
            let mut m = build_blobmap(build_keys, payload);
            run_workload_expanse_blobmap(&mut m, ops, payload, sampling)
        }
        Engine::BTree => {
            let mut m = build_btreemap(build_keys, payload);
            run_workload_btreemap(&mut m, ops, payload, sampling)
        }
        Engine::Skip => {
            let m = build_skipmap(build_keys, payload);
            run_workload_skipmap(&m, ops, payload, sampling)
        }
    }
}

/// Concurrent smoke test execution for `SyncExpanseMap` (Refs #1006).
///
/// **Smoke test only, NOT a measurement.** This routine is a coarse harness used
/// by integration tests (`tests/test_ycsb.rs`) to exercise concurrent readers and
/// writers under basic scheduling. Workloads D and E use the B mix on Zipfian keys,
/// F performs `reader.get` followed by `map.insert` without mutual exclusion, the
/// window is timed via `thread::sleep`, and rates divide by the nominal sleep duration.
/// It is NOT an instrumented benchmark and does NOT produce publishable measurements
/// (see `docs/benchmarks/concurrency/METHODOLOGY.md` §20.1, §20.12 item 6).
pub fn run_concurrent_ycsb(
    readers_count: usize,
    workload: Workload,
    duration: Duration,
) -> (f64, f64) {
    let initial_keys = generate_initial_keys(POPULATION_N);
    let map = Arc::new(SyncExpanseMap::new());
    for &k in &initial_keys {
        map.insert(k, k ^ 0x5CA1_AB1E);
    }

    let stop = Arc::new(AtomicBool::new(false));
    let total_reads = Arc::new(AtomicU64::new(0));
    let total_writes = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..readers_count)
        .map(|worker_id| {
            let map = Arc::clone(&map);
            let stop = Arc::clone(&stop);
            let total_reads = Arc::clone(&total_reads);
            let total_writes = Arc::clone(&total_writes);
            let keys = initial_keys.clone();

            std::thread::spawn(move || {
                let reader = map.reader();
                let zipf = ZipfianGenerator::new(keys.len() as u64, ZIPFIAN_THETA);
                let mut rng = XorShift64::new(0x1000 + worker_id as u64);
                let mut read_ops = 0u64;
                let mut write_ops = 0u64;
                let mut sink = 0u64;

                let (read_prob, is_rmw) = match workload {
                    Workload::A => (50, false),
                    Workload::B => (95, false),
                    Workload::C => (100, false),
                    Workload::D => (95, false),
                    Workload::E => (95, false),
                    Workload::F => (50, true),
                };

                while !stop.load(Ordering::Relaxed) {
                    let prob = rng.gen_range(0, 99);
                    let rank = zipf.next(rng.next_f64()) as usize;
                    let key = keys[rank.min(keys.len() - 1)];

                    if prob < read_prob {
                        sink ^= reader.get(key).unwrap_or(0);
                        read_ops += 1;
                    } else if is_rmw {
                        let old_v = reader.get(key).unwrap_or(0);
                        map.insert(key, old_v.wrapping_add(1));
                        write_ops += 1;
                    } else {
                        map.insert(key, rng.next_u64());
                        write_ops += 1;
                    }
                }
                black_box(sink);
                total_reads.fetch_add(read_ops, Ordering::Relaxed);
                total_writes.fetch_add(write_ops, Ordering::Relaxed);
            })
        })
        .collect();

    std::thread::sleep(duration);
    stop.store(true, Ordering::Relaxed);
    for h in handles {
        h.join().expect("thread join");
    }

    let secs = duration.as_secs_f64();
    let r_ops = total_reads.load(Ordering::Relaxed) as f64 / secs;
    let w_ops = total_writes.load(Ordering::Relaxed) as f64 / secs;
    (r_ops, w_ops)
}

// ---------------------------------------------------------------------------
// Run configuration (environment)
// ---------------------------------------------------------------------------

/// Parses one population token: `100k`, `1m`, `10m`, or a bare integer.
pub fn parse_population(token: &str) -> Result<usize, String> {
    let t = token.trim().to_ascii_lowercase();
    let (digits, mult) = if let Some(d) = t.strip_suffix('k') {
        (d, 1_000usize)
    } else if let Some(d) = t.strip_suffix('m') {
        (d, 1_000_000usize)
    } else {
        (t.as_str(), 1usize)
    };
    let base: usize = digits
        .parse()
        .map_err(|_| format!("YCSB_POPULATIONS: cannot parse population {token:?}"))?;
    let n = base
        .checked_mul(mult)
        .ok_or_else(|| format!("YCSB_POPULATIONS: population {token:?} overflows"))?;
    if n < 2 {
        return Err(format!("YCSB_POPULATIONS: population {token:?} is below 2"));
    }
    Ok(n)
}

/// Populations for this run: `YCSB_POPULATIONS` (comma-separated), else 100k.
/// A malformed value stops the run; it never falls back to the default.
pub fn populations_from_env() -> Vec<usize> {
    match std::env::var("YCSB_POPULATIONS") {
        Err(_) => vec![POPULATION_N],
        Ok(v) => {
            let pops: Vec<usize> = v
                .split(',')
                .filter(|s| !s.trim().is_empty())
                .map(|s| parse_population(s).unwrap_or_else(|e| panic!("{e}")))
                .collect();
            assert!(
                !pops.is_empty(),
                "YCSB_POPULATIONS is set but names no population"
            );
            pops
        }
    }
}

/// Reads a positive integer from the environment, or `default` when unset.
fn usize_from_env(name: &str, default: usize) -> usize {
    match std::env::var(name) {
        Err(_) => default,
        Ok(v) => match v.trim().parse::<usize>() {
            Ok(n) if n > 0 => n,
            _ => panic!("{name}: expected a positive integer, got {v:?}"),
        },
    }
}

/// Short population label for bench ids: `100k`, `1m`, `10m`, else the integer.
pub fn population_label(n: usize) -> String {
    if n >= 1_000_000 && n.is_multiple_of(1_000_000) {
        format!("{}m", n / 1_000_000)
    } else if n >= 1_000 && n.is_multiple_of(1_000) {
        format!("{}k", n / 1_000)
    } else {
        n.to_string()
    }
}

// ---------------------------------------------------------------------------
// Criterion groups
// ---------------------------------------------------------------------------

/// Criterion groups for one suite: every workload × engine × insertion order,
/// at every population `YCSB_POPULATIONS` selects (100k by default).
///
/// Every routine **returns** its structure. `iter_batched` drops a routine's
/// output outside the timed region, but an input the routine consumes and does
/// not return is dropped inside it, and tearing down 100k boxed blobs is not
/// the same cost for a `BTreeMap`, a `SkipMap` and a slab arena (AGENTS.md
/// §8.6, §8.10.3).
pub fn bench_ycsb_workloads(c: &mut Criterion, suite: &Suite) {
    let payload = generate_payload(0xFEED_FACE_CAFE_BEEF);
    let workloads = [
        Workload::A,
        Workload::B,
        Workload::C,
        Workload::D,
        Workload::E,
        Workload::F,
    ];

    for pop in populations_from_env() {
        let canonical = generate_keys(suite.shape, pop);
        // Above the default population one input is large enough that criterion
        // must not hold a batch of them, and every sample costs a full rebuild.
        let large = pop > POPULATION_N;
        let batch = if large {
            criterion::BatchSize::PerIteration
        } else {
            criterion::BatchSize::SmallInput
        };
        for order in InsertionOrder::BOTH {
            let keys = build_order(&canonical, order);
            let param = format!("n{}/{}", population_label(pop), order.tag());
            for &wl in &workloads {
                let ops = generate_operations(wl, &canonical, CRITERION_OP_COUNT, REPORT_SEED);
                let mut group = c.benchmark_group(format!("{}/{}", suite.id, wl.tag()));
                group.throughput(Throughput::Elements(ops.len() as u64));
                if large {
                    group.sample_size(10);
                }

                group.bench_function(BenchmarkId::new("ExpanseMap_u64", &param), |b| {
                    b.iter_batched(
                        || build_expanse_map(&keys),
                        |mut map| {
                            black_box(run_workload_expanse_map(&mut map, &ops, Sampling::Off));
                            map
                        },
                        batch,
                    );
                });

                group.bench_function(BenchmarkId::new("ExpanseBlobMap_128B", &param), |b| {
                    b.iter_batched(
                        || build_blobmap(&keys, &payload),
                        |mut map| {
                            black_box(run_workload_expanse_blobmap(
                                &mut map,
                                &ops,
                                &payload,
                                Sampling::Off,
                            ));
                            map
                        },
                        batch,
                    );
                });

                group.bench_function(BenchmarkId::new("BTreeMap_128B", &param), |b| {
                    b.iter_batched(
                        || build_btreemap(&keys, &payload),
                        |mut map| {
                            black_box(run_workload_btreemap(
                                &mut map,
                                &ops,
                                &payload,
                                Sampling::Off,
                            ));
                            map
                        },
                        batch,
                    );
                });

                group.bench_function(BenchmarkId::new("SkipMap_128B", &param), |b| {
                    b.iter_batched(
                        || build_skipmap(&keys, &payload),
                        |map| {
                            black_box(run_workload_skipmap(&map, &ops, &payload, Sampling::Off));
                            map
                        },
                        batch,
                    );
                });

                group.finish();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rounds / latency report (opt-in via `YCSB_ROUNDS_JSON` or `YCSB_LATENCY_REPORT`)
// ---------------------------------------------------------------------------
//
// Criterion times whole batches and keeps its samples in its own directory. A
// published interval needs per-round samples a driver can pair across arms, so
// the report mode runs ONE round per process and prints one JSON object per
// cell; `scripts/ycsb_bench.py` owns the rounds, the load snapshots and the
// intervals. Within the round the engines of a workload run back to back, and
// the engine that goes first rotates with the round and the workload so no arm
// always inherits the same position.
//
//   YCSB_ROUNDS_JSON=1        one JSON object per cell on stdout
//   YCSB_LATENCY_REPORT=1     the same cells as a human-readable table
//   YCSB_ROUND=<n>            the round index stamped on every row (default 0)
//   YCSB_POPULATIONS=...      populations (default 100k)
//   YCSB_OPS=<n>              ops per cell (default REPORT_OP_COUNT)
//   YCSB_LATENCY_WINDOW=<n>   ops per latency window (default LATENCY_WINDOW)

/// Calibrates what the window bracket costs: the mean of one `Instant::now()` /
/// `elapsed()` pair, in nanoseconds. Divided by the window length it is the
/// residual per-op overhead carried by every window-mean sample.
pub fn window_bracket_overhead_ns() -> f64 {
    let n = 2_000_000usize;
    let mut acc = 0u64;
    let t_all = Instant::now();
    for _ in 0..n {
        let t0 = Instant::now();
        acc = acc.wrapping_add(t0.elapsed().as_nanos() as u64);
    }
    black_box(acc);
    t_all.elapsed().as_nanos() as f64 / n as f64
}

/// One measured cell of the report.
#[derive(Clone, Debug)]
pub struct CellRow {
    /// Workload tag (`workload_a` …).
    pub workload: &'static str,
    /// Which arm.
    pub engine: Engine,
    /// Population the structure was built at.
    pub population: usize,
    /// Build order.
    pub order: InsertionOrder,
    /// Untimed-window pass: throughput and the work checksum.
    pub throughput: RunOutcome,
    /// Window-sampled pass over a fresh build of the same cell.
    pub latency: RunOutcome,
}

/// Measures every cell of one round for `suite`, calling `emit` as each lands.
pub fn run_round<F: FnMut(&CellRow)>(
    suite: &Suite,
    round: usize,
    ops_per_cell: usize,
    window: usize,
    mut emit: F,
) {
    let payload = generate_payload(0xFEED_FACE_CAFE_BEEF);
    let workloads = [
        Workload::A,
        Workload::B,
        Workload::C,
        Workload::D,
        Workload::E,
        Workload::F,
    ];
    for pop in populations_from_env() {
        let canonical = generate_keys(suite.shape, pop);
        for order in InsertionOrder::BOTH {
            let keys = build_order(&canonical, order);
            for (wi, &wl) in workloads.iter().enumerate() {
                let ops = generate_operations(wl, &canonical, ops_per_cell, REPORT_SEED);
                let mut checksum: Option<u64> = None;
                for slot in 0..Engine::ALL.len() {
                    let engine = Engine::ALL[(slot + round + wi) % Engine::ALL.len()];
                    let throughput = run_cell(engine, &keys, &ops, &payload, Sampling::Off);
                    let latency =
                        run_cell(engine, &keys, &ops, &payload, Sampling::Windows(window));
                    // Deterministic invariants, so hard assertions (AGENTS.md
                    // §8.4): one stream over one population does the same work
                    // on both passes and on every engine.
                    assert_eq!(
                        throughput.consumed,
                        latency.consumed,
                        "{}/{:?}: the two passes consumed different work",
                        wl.tag(),
                        engine
                    );
                    match checksum {
                        None => checksum = Some(throughput.consumed),
                        Some(c) => assert_eq!(
                            c,
                            throughput.consumed,
                            "{}/{:?}: arms did different work",
                            wl.tag(),
                            engine
                        ),
                    }
                    emit(&CellRow {
                        workload: wl.tag(),
                        engine,
                        population: pop,
                        order,
                        throughput,
                        latency,
                    });
                }
            }
        }
    }
}

/// Renders one cell as the JSON object `scripts/ycsb_bench.py` parses.
pub fn row_json(suite: &Suite, round: usize, bracket_ns: f64, row: &CellRow) -> String {
    let t = &row.throughput.stats;
    let l = &row.latency.stats;
    serde_json::json!({
        "suite_workload": suite.id,
        "key_shape": suite.shape.tag(),
        "workload": row.workload,
        "engine": row.engine.label(),
        "population": row.population,
        "insertion_order": row.order.tag(),
        "round": round,
        "ops": t.count,
        "elapsed_s": t.elapsed_s,
        "mops": t.ops_per_sec / 1e6,
        "consumed": row.throughput.consumed,
        "mem_bytes": row.throughput.mem_bytes,
        "mem_is_estimate": row.engine.mem_is_estimate(),
        "latency": {
            "method": "window_mean",
            "window_ops": l.window_ops,
            "windows": l.windows,
            "bracket_ns": bracket_ns,
            "residual_ns_per_op": bracket_ns / l.window_ops.max(1) as f64,
            "p50_ns": l.p50_ns,
            "p90_ns": l.p90_ns,
            "p95_ns": l.p95_ns,
            "p99_ns": l.p99_ns,
            "p999_ns": l.p999_ns,
            "max_ns": l.max_ns,
            "slow_windows": l.slow_windows,
            "slow_window_factor": SLOW_WINDOW_FACTOR,
        },
    })
    .to_string()
}

/// Runs the report mode the environment selected.
fn run_report(suite: &Suite, json: bool) {
    let round = match std::env::var("YCSB_ROUND") {
        Err(_) => 0,
        Ok(v) => v
            .trim()
            .parse::<usize>()
            .unwrap_or_else(|_| panic!("YCSB_ROUND: expected an integer, got {v:?}")),
    };
    let ops_per_cell = usize_from_env("YCSB_OPS", REPORT_OP_COUNT);
    let window = usize_from_env("YCSB_LATENCY_WINDOW", LATENCY_WINDOW);
    let bracket = window_bracket_overhead_ns();

    // Comment lines start with `#` in both modes; the driver skips them.
    println!(
        "# {} shape={} round={round} ops_per_cell={ops_per_cell} theta={ZIPFIAN_THETA} payload={BLOB_PAYLOAD_SIZE}B seed={REPORT_SEED:#x}",
        suite.id,
        suite.shape.tag()
    );
    println!(
        "# latency: window-mean sampling, {window} ops/window; one Instant bracket costs ~{bracket:.1} ns, so the residual is ~{:.2} ns/op in every window sample",
        bracket / window as f64
    );
    println!(
        "# percentiles are of WINDOW MEANS, not of single ops; mem figures marked * (BTreeMap, SkipMap) are hand-modelled estimates"
    );
    if !json {
        println!(
            "{:<11} {:<22} {:>6} {:>9} {:>9} {:>8} {:>8} {:>8} {:>8} {:>9} {:>5} {:>9}",
            "workload",
            "engine",
            "pop",
            "order",
            "Mops/s",
            "p50",
            "p95",
            "p99",
            "p99.9",
            "max",
            "slow",
            "mem_MB"
        );
    }
    run_round(suite, round, ops_per_cell, window, |row| {
        if json {
            println!("{}", row_json(suite, round, bracket, row));
        } else {
            let l = &row.latency.stats;
            println!(
                "{:<11} {:<22} {:>6} {:>9} {:>9.3} {:>8.1} {:>8.1} {:>8.1} {:>8.1} {:>9.1} {:>5} {:>9}",
                row.workload,
                row.engine.label(),
                population_label(row.population),
                row.order.tag(),
                row.throughput.stats.ops_per_sec / 1e6,
                l.p50_ns,
                l.p95_ns,
                l.p99_ns,
                l.p999_ns,
                l.max_ns,
                l.slow_windows,
                format!(
                    "{:.2}{}",
                    row.throughput.mem_bytes as f64 / (1024.0 * 1024.0),
                    if row.engine.mem_is_estimate() {
                        "*"
                    } else {
                        ""
                    }
                ),
            );
        }
    });
}

/// Entry point shared by the harness files: a report mode when the environment
/// asks for one, the criterion groups otherwise.
pub fn harness_entry(suite: &Suite, criterion_groups: fn()) {
    if std::env::var_os("YCSB_ROUNDS_JSON").is_some() {
        run_report(suite, true);
        return;
    }
    if std::env::var_os("YCSB_LATENCY_REPORT").is_some() {
        run_report(suite, false);
        return;
    }
    criterion_groups();
    Criterion::default().configure_from_args().final_summary();
}
