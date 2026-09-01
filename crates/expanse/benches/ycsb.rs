//! Standardized YCSB (Yahoo! Cloud Serving Benchmark) benchmark suite.
//!
//! Evaluates Workloads A through F against:
//! - [`ExpanseMap`] (pure 64-bit digital trie)
//! - [`ExpanseBlobMap`] (inlined value slots + chunked slab arena)
//! - [`SyncExpanseMap`] (optimistic concurrent trie)
//! - [`std::collections::BTreeMap`] (standard comparison baseline)
//! - [`crossbeam_skiplist::SkipMap`] (RocksDB In-Memory MemTable model)
//!
//! # Workload Specifications
//! - **Workload A (Update Heavy)**: 50% Read, 50% Update (Zipfian key distribution)
//! - **Workload B (Read Mostly)**: 95% Read, 5% Update (Zipfian key distribution)
//! - **Workload C (Read Only)**: 100% Read (Zipfian key distribution)
//! - **Workload D (Read Latest)**: 95% Read, 5% Insert (Latest-skewed append)
//! - **Workload E (Short Range Scans)**: 95% Scan (10..100 items with predicate filtering), 5% Insert
//! - **Workload F (Read-Modify-Write)**: 50% Read, 50% RMW (Zipfian key distribution)
//!
//! # Distribution Parameters
//! - Key count: $N = 100,000$
//! - Zipfian skew parameter: $\theta = 0.99$
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `workload_ycsb` |
//! | `group` | 4 |
//! | `population` | 100k |
//! | `probes_and_reuse` | 100k ops |
//! | `hit_rate` | Zipfian |
//! | `miss_gen_method` | Zipfian draw from dataset |
//! | `value_dereference` | `black_box(res)` does **NOT** deref payload buffer in `BlobMap`/`BTree` (L479, L540) |
//! | `measured_region` | Clean |
//! | `arm_symmetry` | Symmetric predicates (50% key-parity) |
//! | `statistics` | Criterion / LatencyStats |
//! | `verdict` | ✅ **RESOLVED in #470 — was DEFECT (Class 6)** `[verified: CODE READ]`: `Read` op on `ExpanseBlobMap` and `BTreeMap` omits payload cache-line fetch. (See Policy Decision below regarding ratio soundness vs absolute numbers). |

use crossbeam_skiplist::SkipMap;
use expanse_trie::blobmap::ExpanseBlobMap;
use expanse_trie::map::ExpanseMap;
use expanse_trie::sync::SyncExpanseMap;
use std::collections::BTreeMap;
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group};

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

/// Standard benchmark population size: $N = 100,000$.
pub const POPULATION_N: usize = 100_000;
/// Default Zipfian skew $\theta = 0.99$.
pub const ZIPFIAN_THETA: f64 = 0.99;
/// Default operation count per benchmark round.
pub const OP_COUNT: usize = 50_000;
/// Standard 128-byte payload size for database blob modeling.
pub const BLOB_PAYLOAD_SIZE: usize = 128;

/// Generates initial dataset of $N$ distinct 64-bit keys.
pub fn generate_initial_keys(n: usize) -> Vec<u64> {
    let mut rng = XorShift64::new(0x0DDB_1A5E_5EED_0001);
    let mut keys = Vec::with_capacity(n);
    for _ in 0..n {
        keys.push(rng.next_u64());
    }
    keys
}

/// Generates a standardized 128-byte payload for a given key.
pub fn generate_payload(key: u64) -> [u8; BLOB_PAYLOAD_SIZE] {
    let mut buf = [0u8; BLOB_PAYLOAD_SIZE];
    let kb = key.to_be_bytes();
    for i in 0..BLOB_PAYLOAD_SIZE {
        buf[i] = kb[i % 8] ^ (i as u8);
    }
    buf
}

/// Generates a sequence of pre-computed YCSB operations for a given workload.
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
    let mut next_insert_seq = 0x8000_0000_0000_0000u64;

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
                    // Skewed toward most recently inserted keys
                    let latest_rank = zipf.next(rng.next_f64()) as usize;
                    let target_key = initial_keys[n - 1 - (latest_rank % n)];
                    YcsbOp::Read(target_key)
                } else {
                    next_insert_seq = next_insert_seq.wrapping_add(1);
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

/// Per-operation latency statistics collector.
#[derive(Clone, Debug, Default)]
pub struct LatencyStats {
    /// Total operations measured.
    pub count: usize,
    /// Total elapsed duration in nanoseconds.
    pub total_ns: u64,
    /// Minimum latency in nanoseconds.
    pub min_ns: u64,
    /// Maximum latency in nanoseconds.
    pub max_ns: u64,
    /// Median (p50) latency in nanoseconds.
    pub p50_ns: u64,
    /// 90th percentile latency in nanoseconds.
    pub p90_ns: u64,
    /// 95th percentile latency in nanoseconds.
    pub p95_ns: u64,
    /// 99th percentile latency in nanoseconds.
    pub p99_ns: u64,
    /// 99.9th percentile latency in nanoseconds.
    pub p999_ns: u64,
    /// Mean latency in nanoseconds.
    pub mean_ns: f64,
    /// Operations per second throughput.
    pub ops_per_sec: f64,
}

impl LatencyStats {
    /// Computes summary statistics from a slice of raw per-operation nanosecond latencies.
    pub fn compute(mut latencies: Vec<u64>, total_duration: Duration) -> Self {
        if latencies.is_empty() {
            return Self::default();
        }
        latencies.sort_unstable();
        let count = latencies.len();
        let min_ns = latencies[0];
        let max_ns = latencies[count - 1];
        let p50_ns = latencies[(count * 50) / 100];
        let p90_ns = latencies[(count * 90) / 100];
        let p95_ns = latencies[(count * 95) / 100];
        let p99_ns = latencies[(count * 99) / 100];
        let p999_ns = latencies[((count * 999) / 1000).min(count - 1)];
        let total_ns: u64 = latencies.iter().sum();
        let mean_ns = total_ns as f64 / count as f64;
        let secs = total_duration.as_secs_f64();
        let ops_per_sec = if secs > 0.0 { count as f64 / secs } else { 0.0 };

        Self {
            count,
            total_ns,
            min_ns,
            max_ns,
            p50_ns,
            p90_ns,
            p95_ns,
            p99_ns,
            p999_ns,
            mean_ns,
            ops_per_sec,
        }
    }
}

// ---------------------------------------------------------------------------
// Workload E scan bound — record-count terminated, not key-width terminated
// ---------------------------------------------------------------------------
//
// YCSB Workload E is specified as "scan up to N records from a starting key".
// The bound is a RECORD COUNT, so every arm iterates from `start_k` forward
// and stops when `len` predicate-matching records have been collected.
//
// This replaces a `start_k ..= start_k + len * 1000` key-width window, which
// silently made Workload E a seek benchmark instead of a scan benchmark. The
// window constant assumed a dense keyspace, but `generate_initial_keys` draws
// uniform-random u64s: at N = 100 000 the key density is ~5.4e-15 per key
// unit, so a <= 99 000-wide window is expected to contain 3e-10 keys beyond
// the start key. Measured on the shipped stream (`generate_operations`,
// Workload::E, seed 0x1234_5678_9ABC, 20 000 ops, 19 054 scans) via
// structural counters inside `RawIter`: the iterator yielded 1.0000 records
// per scan against a mean REQUESTED length of 54.9, and `count >= len` was
// reached 0 times out of 19 054. Every arm used the same window, so the
// cross-engine ratio was not fabricated — but what it ranked was seek plus
// one terminating iterator advance, not short range scanning.
//
// Scan-length semantics unchanged: `rng.gen_range(10, 100)` records.
//
// ---------------------------------------------------------------------------
// Workload E scan predicate — identical selectivity by construction (#375)
// ---------------------------------------------------------------------------
//
// Every engine's scan arm filters entries by KEY parity: `k & 1 == 0`.
// Keys are uniform-random u64s, so the predicate passes ~50% of scanned
// entries, and because it depends only on the key — never on an engine's
// value representation — the pass/fail decision is identical for the same
// key across the ExpanseMap, ExpanseBlobMap, BTreeMap and SkipMap arms.
//
// (Before #375 the ExpanseMap arm used `(v ^ k) & 1 == 0`. Values are
// `k ^ 0x5CA1_AB1E` in the map arms, so `v ^ k` folds to the even constant
// `0x5CA1_AB1E` and that predicate was unconditionally true — asymmetric
// against the ~50%-selective baseline arms, and dead code besides. The #375
// note attributed "~2x the entries per scan" to it; that magnitude does not
// hold, because under the key-width window every arm traversed exactly one
// record per scan regardless of predicate. The #375 change is still correct;
// only its stated effect size was wrong. Any throughput delta measured
// across #375 therefore needs re-attribution to some other cause.)

/// Execution runner for `ExpanseMap`.
pub fn run_workload_expanse_map(
    map: &mut ExpanseMap,
    ops: &[YcsbOp],
    record_latencies: bool,
) -> (LatencyStats, usize) {
    let mut latencies = if record_latencies {
        Vec::with_capacity(ops.len())
    } else {
        Vec::new()
    };
    let start_all = Instant::now();

    for &op in ops {
        let t0 = if record_latencies {
            Some(Instant::now())
        } else {
            None
        };
        match op {
            YcsbOp::Read(k) => {
                let v = map.get(k);
                black_box(v);
            }
            YcsbOp::Update(k, v) => {
                map.insert(k, v);
            }
            YcsbOp::Insert(k, v) => {
                map.insert(k, v);
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
                black_box(count);
            }
            YcsbOp::ReadModifyWrite(k) => {
                let old_v = map.get(k).unwrap_or(0);
                map.insert(k, old_v.wrapping_add(1));
            }
        }
        if let Some(t) = t0 {
            latencies.push(t.elapsed().as_nanos() as u64);
        }
    }

    let elapsed = start_all.elapsed();
    let mem = map.mem_used();
    (LatencyStats::compute(latencies, elapsed), mem)
}

/// Execution runner for `ExpanseBlobMap`.
pub fn run_workload_expanse_blobmap(
    map: &mut ExpanseBlobMap,
    ops: &[YcsbOp],
    payload: &[u8],
    record_latencies: bool,
) -> (LatencyStats, usize) {
    let mut latencies = if record_latencies {
        Vec::with_capacity(ops.len())
    } else {
        Vec::new()
    };
    let start_all = Instant::now();

    for &op in ops {
        let t0 = if record_latencies {
            Some(Instant::now())
        } else {
            None
        };
        match op {
            YcsbOp::Read(k) => {
                let res = map.get(k);
                if let Some((view, _)) = res {
                    black_box(if view.is_empty() { 0 } else { view[0] });
                } else {
                    black_box(0);
                }
            }
            YcsbOp::Update(k, meta) => {
                let _ = map.insert(k, payload, meta as u32);
            }
            YcsbOp::Insert(k, meta) => {
                let _ = map.insert(k, payload, meta as u32);
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
                black_box(count);
            }
            YcsbOp::ReadModifyWrite(k) => {
                let current_meta = map.get(k).map(|(_, m)| m).unwrap_or(0);
                let _ = map.insert(k, payload, current_meta.wrapping_add(1));
            }
        }
        if let Some(t) = t0 {
            latencies.push(t.elapsed().as_nanos() as u64);
        }
    }

    let elapsed = start_all.elapsed();
    let mem = map.mem_used();
    (LatencyStats::compute(latencies, elapsed), mem)
}

/// Execution runner for `BTreeMap`.
pub fn run_workload_btreemap(
    map: &mut BTreeMap<u64, Box<[u8]>>,
    ops: &[YcsbOp],
    payload: &[u8],
    record_latencies: bool,
) -> (LatencyStats, usize) {
    let mut latencies = if record_latencies {
        Vec::with_capacity(ops.len())
    } else {
        Vec::new()
    };
    let start_all = Instant::now();

    for &op in ops {
        let t0 = if record_latencies {
            Some(Instant::now())
        } else {
            None
        };
        match op {
            YcsbOp::Read(k) => {
                let res = map.get(&k);
                if let Some(v) = res {
                    black_box(if v.is_empty() { 0 } else { v[0] });
                } else {
                    black_box(0);
                }
            }
            YcsbOp::Update(k, _) => {
                map.insert(k, payload.to_vec().into_boxed_slice());
            }
            YcsbOp::Insert(k, _) => {
                map.insert(k, payload.to_vec().into_boxed_slice());
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
                black_box(count);
            }
            YcsbOp::ReadModifyWrite(k) => {
                if let Some(val) = map.get_mut(&k) {
                    if !val.is_empty() {
                        val[0] = val[0].wrapping_add(1);
                    }
                } else {
                    map.insert(k, payload.to_vec().into_boxed_slice());
                }
            }
        }
        if let Some(t) = t0 {
            latencies.push(t.elapsed().as_nanos() as u64);
        }
    }

    let elapsed = start_all.elapsed();
    // ESTIMATED, not measured (#375): std `BTreeMap` exposes no allocator
    // accounting and this harness has no allocator hook on external crates,
    // so this is a hand model — ~48 B amortized per-entry node overhead
    // (B-tree node headers, key + value slots, typical fill factor) plus
    // the boxed payload allocation plus ~16 B `Box`/allocator rounding.
    // The latency report marks these figures with a `*` (est.); the
    // Expanse arms report measured `mem_used()` figures.
    let entry_overhead = 48 + payload.len() + 16;
    let mem = map.len() * entry_overhead;
    (LatencyStats::compute(latencies, elapsed), mem)
}

/// Execution runner for `SkipMap` (RocksDB MemTable model).
pub fn run_workload_skipmap(
    map: &SkipMap<u64, Box<[u8]>>,
    ops: &[YcsbOp],
    payload: &[u8],
    record_latencies: bool,
) -> (LatencyStats, usize) {
    let mut latencies = if record_latencies {
        Vec::with_capacity(ops.len())
    } else {
        Vec::new()
    };
    let start_all = Instant::now();

    for &op in ops {
        let t0 = if record_latencies {
            Some(Instant::now())
        } else {
            None
        };
        match op {
            YcsbOp::Read(k) => {
                let res = map.get(&k);
                if let Some(entry) = res {
                    let v = entry.value();
                    black_box(if v.is_empty() { 0 } else { v[0] });
                } else {
                    black_box(0);
                }
            }
            YcsbOp::Update(k, _) => {
                map.insert(k, payload.to_vec().into_boxed_slice());
            }
            YcsbOp::Insert(k, _) => {
                map.insert(k, payload.to_vec().into_boxed_slice());
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
                black_box(count);
            }
            YcsbOp::ReadModifyWrite(k) => {
                if let Some(entry) = map.get(&k) {
                    let mut cloned: Box<[u8]> = entry.value().clone();
                    if !cloned.is_empty() {
                        cloned[0] = cloned[0].wrapping_add(1);
                    }
                    map.insert(k, cloned);
                } else {
                    map.insert(k, payload.to_vec().into_boxed_slice());
                }
            }
        }
        if let Some(t) = t0 {
            latencies.push(t.elapsed().as_nanos() as u64);
        }
    }

    let elapsed = start_all.elapsed();
    // ESTIMATED, not measured (#375): `crossbeam_skiplist` exposes no
    // allocator accounting and this harness has no allocator hook on
    // external crates, so this is a hand model — ~64 B amortized per
    // tower node (entry header + expected level pointers) plus the boxed
    // payload allocation plus ~16 B `Box`/allocator rounding. The latency
    // report marks these figures with a `*` (est.); the Expanse arms
    // report measured `mem_used()` figures.
    let entry_overhead = 64 + payload.len() + 16;
    let mem = map.len() * entry_overhead;
    (LatencyStats::compute(latencies, elapsed), mem)
}

/// Concurrent benchmark execution for `SyncExpanseMap`.
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
// Criterion Benchmark Groups
// ---------------------------------------------------------------------------

fn bench_ycsb_workloads(c: &mut Criterion) {
    let initial_keys = generate_initial_keys(POPULATION_N);
    let payload = generate_payload(0xFEED_FACE_CAFE_BEEF);
    let workloads = [
        Workload::A,
        Workload::B,
        Workload::C,
        Workload::D,
        Workload::E,
        Workload::F,
    ];

    for &wl in &workloads {
        let ops = generate_operations(wl, &initial_keys, 20_000, 0x1234_5678_9ABC);
        let mut group = c.benchmark_group(format!("ycsb/{}", wl.tag()));
        group.throughput(Throughput::Elements(ops.len() as u64));

        // 1. ExpanseMap
        group.bench_function(BenchmarkId::new("ExpanseMap", "u64"), |b| {
            b.iter_batched(
                || {
                    let mut map = ExpanseMap::new();
                    for &k in &initial_keys {
                        map.insert(k, k ^ 0x5CA1_AB1E);
                    }
                    map
                },
                |mut map| {
                    let (stats, _) = run_workload_expanse_map(&mut map, &ops, false);
                    black_box(stats);
                },
                criterion::BatchSize::SmallInput,
            );
        });

        // 2. ExpanseBlobMap
        group.bench_function(BenchmarkId::new("ExpanseBlobMap", "128B_blob"), |b| {
            b.iter_batched(
                || {
                    let mut map = ExpanseBlobMap::new();
                    for &k in &initial_keys {
                        let _ = map.insert(k, &payload, (k & 0xFF) as u32);
                    }
                    map
                },
                |mut map| {
                    let (stats, _) = run_workload_expanse_blobmap(&mut map, &ops, &payload, false);
                    black_box(stats);
                },
                criterion::BatchSize::SmallInput,
            );
        });

        // 3. std::collections::BTreeMap
        group.bench_function(BenchmarkId::new("BTreeMap", "128B_blob"), |b| {
            b.iter_batched(
                || {
                    let mut map = BTreeMap::new();
                    for &k in &initial_keys {
                        map.insert(k, payload.to_vec().into_boxed_slice());
                    }
                    map
                },
                |mut map| {
                    let (stats, _) = run_workload_btreemap(&mut map, &ops, &payload, false);
                    black_box(stats);
                },
                criterion::BatchSize::SmallInput,
            );
        });

        // 4. crossbeam_skiplist::SkipMap (RocksDB MemTable model)
        group.bench_function(BenchmarkId::new("SkipMap", "128B_blob"), |b| {
            b.iter_batched(
                || {
                    let map = SkipMap::new();
                    for &k in &initial_keys {
                        map.insert(k, payload.to_vec().into_boxed_slice());
                    }
                    map
                },
                |map| {
                    let (stats, _) = run_workload_skipmap(&map, &ops, &payload, false);
                    black_box(stats);
                },
                criterion::BatchSize::SmallInput,
            );
        });

        group.finish();
    }
}

// The former `bench_ycsb_concurrency` criterion group was removed (#375):
// it timed a closure containing the 100k-key build, the thread spawns and a
// fixed 50 ms sleep, so the reported time was ~constant regardless of thread
// count and the measured throughput was discarded. Thread-scaling throughput
// for the sync structures is owned by the `/benchmark concurrency` suite
// (`benches/concurrency.rs` + `scripts/bench_concurrency_check.py`, #368).
// `run_concurrent_ycsb` itself stays: `tests/test_ycsb.rs` exercises it.
criterion_group!(benches, bench_ycsb_workloads);

// ---------------------------------------------------------------------------
// Latency-report mode (opt-in via `YCSB_LATENCY_REPORT=1`)
// ---------------------------------------------------------------------------
//
// The criterion groups above call every `run_workload_*` with
// `record_latencies = false` (criterion times whole batches, not individual
// ops). Percentile latency columns therefore need a separate single-pass run
// with per-op recording enabled. Setting `YCSB_LATENCY_REPORT` in the
// environment runs each workload once per engine with `record_latencies =
// true` and prints a tab-friendly table of p50/p90/p95/p99/p99.9 plus a clean
// (untimed) throughput cross-check and the resident-memory footprint.
//
// Caveat printed with the report: every recorded latency includes the cost of
// the `Instant::now()`/`elapsed()` bracket itself, which the report calibrates
// and prints so the tail figures can be read honestly on any host.

/// Operations in the latency-recording stream (large enough that p99.9 has
/// several hundred tail samples).
const LATENCY_OP_COUNT: usize = 200_000;
/// Clean (untimed) passes whose median gives the throughput cross-check column.
const THROUGHPUT_PASSES: usize = 7;
/// Operations per clean throughput pass (matches the criterion group's stream).
const THROUGHPUT_OP_COUNT: usize = 20_000;
/// Deterministic operation-stream seed (identical to the criterion groups).
const REPORT_SEED: u64 = 0x1234_5678_9ABC;

/// Median of a slice of `f64` samples (sorts in place).
fn median_f64(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Calibrates the per-op cost of the `Instant::now()`/`elapsed()` bracket so
/// the report can state how much of each recorded latency is measurement.
fn timer_bracket_overhead_ns() -> f64 {
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

/// Prints one formatted engine row of the latency report. `mem_estimated`
/// marks the memory columns with `*`: baseline engines (BTreeMap, SkipMap)
/// have no allocator accounting, so their figures are hand-modeled per-node
/// constants (see `run_workload_btreemap` / `run_workload_skipmap`), not
/// measured like the Expanse `mem_used()` columns.
fn emit_latency_row(
    wl: Workload,
    engine: &str,
    thr_mops: f64,
    s: &LatencyStats,
    mem: usize,
    mem_estimated: bool,
) {
    let mem_mb = mem as f64 / (1024.0 * 1024.0);
    let bytes_per_key = mem as f64 / POPULATION_N as f64;
    let marker = if mem_estimated { "*" } else { "" };
    println!(
        "{:<11} {:<24} {:>9.2} {:>7} {:>7} {:>7} {:>7} {:>8} {:>9} {:>9}",
        wl.tag(),
        engine,
        thr_mops,
        s.p50_ns,
        s.p90_ns,
        s.p95_ns,
        s.p99_ns,
        s.p999_ns,
        format!("{mem_mb:.2}{marker}"),
        format!("{bytes_per_key:.1}{marker}"),
    );
}

/// Runs the opt-in per-operation latency report across all workloads/engines.
fn run_latency_report() {
    let initial_keys = generate_initial_keys(POPULATION_N);
    let payload = generate_payload(0xFEED_FACE_CAFE_BEEF);
    let workloads = [
        Workload::A,
        Workload::B,
        Workload::C,
        Workload::D,
        Workload::E,
        Workload::F,
    ];

    let overhead = timer_bracket_overhead_ns();
    println!("# YCSB per-operation latency report (record_latencies = true)");
    println!(
        "# population_n={POPULATION_N} theta={ZIPFIAN_THETA} payload={BLOB_PAYLOAD_SIZE}B seed={REPORT_SEED:#016x}"
    );
    println!(
        "# latency_ops={LATENCY_OP_COUNT} throughput_ops={THROUGHPUT_OP_COUNT} throughput_passes={THROUGHPUT_PASSES} (median)"
    );
    println!(
        "# Instant bracket overhead ~{overhead:.1} ns/op (included in every recorded latency below)"
    );
    println!("# latency columns are nanoseconds; thrpt is the untimed-pass cross-check (Mops/s)");
    println!(
        "# mem_MB/B-key marked * are ESTIMATES (hand-modeled ~48 B/node BTreeMap, ~64 B/node SkipMap + payload; no allocator hook on external crates) — unmarked Expanse rows are measured via mem_used()"
    );
    println!(
        "{:<11} {:<24} {:>9} {:>7} {:>7} {:>7} {:>7} {:>8} {:>9} {:>9}",
        "workload", "engine", "thrpt", "p50", "p90", "p95", "p99", "p99.9", "mem_MB", "B/key"
    );

    for &wl in &workloads {
        let lat_ops = generate_operations(wl, &initial_keys, LATENCY_OP_COUNT, REPORT_SEED);
        let thr_ops = generate_operations(wl, &initial_keys, THROUGHPUT_OP_COUNT, REPORT_SEED);

        // 1. ExpanseMap (u64)
        {
            let build = || {
                let mut m = ExpanseMap::new();
                for &k in &initial_keys {
                    m.insert(k, k ^ 0x5CA1_AB1E);
                }
                m
            };
            let mut mops = Vec::with_capacity(THROUGHPUT_PASSES);
            for _ in 0..THROUGHPUT_PASSES {
                let mut m = build();
                let t = Instant::now();
                let _ = run_workload_expanse_map(&mut m, &thr_ops, false);
                mops.push(thr_ops.len() as f64 / t.elapsed().as_secs_f64() / 1e6);
            }
            let mut m = build();
            let (s, mem) = run_workload_expanse_map(&mut m, &lat_ops, true);
            emit_latency_row(
                wl,
                "ExpanseMap (u64)",
                median_f64(&mut mops),
                &s,
                mem,
                false,
            );
        }

        // 2. ExpanseBlobMap (128B)
        {
            let build = || {
                let mut m = ExpanseBlobMap::new();
                for &k in &initial_keys {
                    let _ = m.insert(k, &payload, (k & 0xFF) as u32);
                }
                m
            };
            let mut mops = Vec::with_capacity(THROUGHPUT_PASSES);
            for _ in 0..THROUGHPUT_PASSES {
                let mut m = build();
                let t = Instant::now();
                let _ = run_workload_expanse_blobmap(&mut m, &thr_ops, &payload, false);
                mops.push(thr_ops.len() as f64 / t.elapsed().as_secs_f64() / 1e6);
            }
            let mut m = build();
            let (s, mem) = run_workload_expanse_blobmap(&mut m, &lat_ops, &payload, true);
            emit_latency_row(
                wl,
                "ExpanseBlobMap (128B)",
                median_f64(&mut mops),
                &s,
                mem,
                false,
            );
        }

        // 3. std::collections::BTreeMap (128B)
        {
            let build = || {
                let mut m = BTreeMap::new();
                for &k in &initial_keys {
                    m.insert(k, payload.to_vec().into_boxed_slice());
                }
                m
            };
            let mut mops = Vec::with_capacity(THROUGHPUT_PASSES);
            for _ in 0..THROUGHPUT_PASSES {
                let mut m = build();
                let t = Instant::now();
                let _ = run_workload_btreemap(&mut m, &thr_ops, &payload, false);
                mops.push(thr_ops.len() as f64 / t.elapsed().as_secs_f64() / 1e6);
            }
            let mut m = build();
            let (s, mem) = run_workload_btreemap(&mut m, &lat_ops, &payload, true);
            emit_latency_row(wl, "BTreeMap (128B)", median_f64(&mut mops), &s, mem, true);
        }

        // 4. crossbeam_skiplist::SkipMap (RocksDB MemTable model, 128B)
        {
            let build = || {
                let m = SkipMap::new();
                for &k in &initial_keys {
                    m.insert(k, payload.to_vec().into_boxed_slice());
                }
                m
            };
            let mut mops = Vec::with_capacity(THROUGHPUT_PASSES);
            for _ in 0..THROUGHPUT_PASSES {
                let m = build();
                let t = Instant::now();
                let _ = run_workload_skipmap(&m, &thr_ops, &payload, false);
                mops.push(thr_ops.len() as f64 / t.elapsed().as_secs_f64() / 1e6);
            }
            let m = build();
            let (s, mem) = run_workload_skipmap(&m, &lat_ops, &payload, true);
            emit_latency_row(wl, "SkipMap (128B)", median_f64(&mut mops), &s, mem, true);
        }
    }
}

fn main() {
    if std::env::var_os("YCSB_LATENCY_REPORT").is_some() {
        run_latency_report();
        return;
    }
    benches();
    Criterion::default().configure_from_args().final_summary();
}
