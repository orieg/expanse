//! Shared core of the concurrent YCSB harness (Refs #1006,
//! `docs/benchmarks/concurrency/METHODOLOGY.md` §20).
//!
//! `examples/ycsb_concurrent.rs` (the harness) and
//! `tests/test_ycsb_concurrent.rs` (its tests) both `#[path]`-include this
//! file, so the tests exercise the stream generator, the timed operation loop
//! and the correctness oracle the harness runs, not copies of them. It has no
//! entry point and no `# Workload shape` table of its own; it is a helper
//! module in the sense `ycsb_common` is, and the shape table lives in the
//! harness (AGENTS.md §8.15).
//!
//! The including crate root must declare `mod ycsb_common;` beside this module.

#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Mutex, RwLock};
use std::time::{Duration, Instant};

use crossbeam_skiplist::SkipMap;
use dashmap::DashMap;

use expanse_trie::map::ExpanseMap;
use expanse_trie::occ_stats::{self, NUM_STATS};
use expanse_trie::sync::{OwnedMapReader, SyncExpanseMap};
use expanse_trie::validate::ExpanseStats;

use crate::ycsb_common::{XorShift64, ZIPFIAN_THETA, ZipfianGenerator};

/// Gated population: 2^20 keys (§20.4, D11).
pub const STANDARD_POPULATION_N: usize = 1_048_576;
/// Anchor population of the `-dram` cells: 2^24 keys (§20.4, D11).
pub const DRAM_POPULATION_N: usize = 16_777_216;
/// `--quick` population. Never a registered cell.
pub const QUICK_POPULATION_N: usize = 4_096;
/// Operations per thread: 2^20 (§20.4).
pub const STANDARD_OPS_PER_THREAD: usize = 1_048_576;
/// `--quick` operations per thread.
pub const QUICK_OPS_PER_THREAD: usize = 4_096;

/// Population key PRNG seed.
pub const SEED_KEY_BASE: u64 = 0x0DDB_1A5E_5EED_0001;
/// Default suite seed.
pub const DEFAULT_SUITE_SEED: u64 = 0x1006_2026_0917_0001;
/// Thread-seed decorrelation constant: suite seed XOR (t + 1)·φ64 (§20.4).
pub const PHI64: u64 = 0x9E37_79B9_7F4A_7C15;

/// Stripes of the external lock that makes `olc`'s read-modify-write atomic (D1).
pub const NUM_STRIPES: usize = 1_024;
/// `DashMap` shard count, fixed so it does not follow the affinity mask (§20.5).
pub const DASH_SHARDS: usize = 64;

/// The lowest-rank counts whose observed share of one stream is published
/// (§20.6, `provenance.rank_histogram`).
pub const RANK_K: [u64; 5] = [1, 2, 16, 256, 4_096];

/// Cache-line padded wrapper, so lock stripes do not share a line.
#[repr(align(64))]
pub struct CachePadded<T>(pub T);

/// Competitor arms (§20.5).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Arm {
    Olc,
    Mutex,
    Skip,
    Dash,
    RwBTree,
}

impl Arm {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "olc" => Ok(Self::Olc),
            "mutex" => Ok(Self::Mutex),
            "skip" => Ok(Self::Skip),
            "dash" => Ok(Self::Dash),
            "rwbtree" => Ok(Self::RwBTree),
            _ => Err(format!(
                "unknown arm '{s}'; expected olc, mutex, skip, dash, or rwbtree"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Olc => "olc",
            Self::Mutex => "mutex",
            Self::Skip => "skip",
            Self::Dash => "dash",
            Self::RwBTree => "rwbtree",
        }
    }

    /// §20.5's update idiom for the arm.
    pub fn update_idiom(self) -> &'static str {
        match self {
            Self::Olc | Self::Mutex => "map_insert_in_place",
            Self::Skip | Self::Dash | Self::RwBTree => "value_cell_store",
        }
    }

    /// §20.4's value type for the arm.
    pub fn value_type(self) -> &'static str {
        match self {
            Self::Olc | Self::Mutex => "u64",
            Self::Skip | Self::Dash | Self::RwBTree => "atomic_u64",
        }
    }

    /// §20.5's read-modify-write provider for the arm.
    pub fn rmw_provider(self) -> &'static str {
        match self {
            Self::Olc => "striped_lock",
            Self::Mutex => "global_mutex",
            Self::Skip | Self::Dash | Self::RwBTree => "atomic_fetch_add",
        }
    }
}

/// Cell families (§20.4's table).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Family {
    A,
    B,
    D,
    F,
    A0,
    B0,
    F0,
    C,
    C0,
    Ac,
    Fc,
    ADram,
    CDram,
}

impl Family {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_ascii_uppercase().as_str() {
            "A" => Ok(Self::A),
            "B" => Ok(Self::B),
            "D" => Ok(Self::D),
            "F" => Ok(Self::F),
            "A0" => Ok(Self::A0),
            "B0" => Ok(Self::B0),
            "F0" => Ok(Self::F0),
            "C" => Ok(Self::C),
            "C0" => Ok(Self::C0),
            "AC" => Ok(Self::Ac),
            "FC" => Ok(Self::Fc),
            "A-DRAM" | "ADRAM" => Ok(Self::ADram),
            "C-DRAM" | "CDRAM" => Ok(Self::CDram),
            _ => Err(format!("unknown family '{s}'")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::D => "D",
            Self::F => "F",
            Self::A0 => "A0",
            Self::B0 => "B0",
            Self::F0 => "F0",
            Self::C => "C",
            Self::C0 => "C0",
            Self::Ac => "Ac",
            Self::Fc => "Fc",
            Self::ADram => "A-dram",
            Self::CDram => "C-dram",
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Self::A => "concurrency_ycsb_a",
            Self::B => "concurrency_ycsb_b",
            Self::D => "concurrency_ycsb_d",
            Self::F => "concurrency_ycsb_f",
            Self::A0 => "concurrency_ycsb_a0",
            Self::B0 => "concurrency_ycsb_b0",
            Self::F0 => "concurrency_ycsb_f0",
            Self::C => "concurrency_ycsb_c",
            Self::C0 => "concurrency_ycsb_c0",
            Self::Ac => "concurrency_ycsb_ac",
            Self::Fc => "concurrency_ycsb_fc",
            Self::ADram => "concurrency_ycsb_a_dram",
            Self::CDram => "concurrency_ycsb_c_dram",
        }
    }

    pub fn is_uniform(self) -> bool {
        matches!(self, Self::A0 | Self::B0 | Self::F0 | Self::C0)
    }

    pub fn is_contiguous_ranks(self) -> bool {
        matches!(self, Self::Ac | Self::Fc)
    }

    pub fn is_dram(self) -> bool {
        matches!(self, Self::ADram | Self::CDram)
    }

    /// Families whose write is value ← value + 1 (G5's invariant applies).
    pub fn is_rmw(self) -> bool {
        matches!(self, Self::F | Self::F0 | Self::Fc)
    }

    /// θ of the family's key choice: 0 in a uniform twin (§20.8's void rule).
    pub fn theta(self) -> f64 {
        if self.is_uniform() {
            0.0
        } else {
            ZIPFIAN_THETA
        }
    }
}

/// One pre-generated operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Read { key: u64 },
    Update { key: u64, val: u64 },
    Insert { key: u64, val: u64 },
    ReadModifyWrite { key: u64 },
}

/// Draws of one stream that landed on the `RANK_K[i]` lowest ranks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RankHistogram {
    pub draws: u64,
    pub at_or_below: [u64; RANK_K.len()],
}

impl RankHistogram {
    #[inline]
    fn record(&mut self, rank: usize) {
        self.draws += 1;
        for (slot, &k) in self.at_or_below.iter_mut().zip(RANK_K.iter()) {
            if (rank as u64) < k {
                *slot += 1;
            }
        }
    }

    /// Observed share of draws on the `RANK_K[i]` lowest ranks.
    pub fn shares(&self) -> [f64; RANK_K.len()] {
        let mut out = [0.0; RANK_K.len()];
        for (o, &c) in out.iter_mut().zip(self.at_or_below.iter()) {
            *o = c as f64 / self.draws.max(1) as f64;
        }
        out
    }
}

/// Power-of-two histogram with 16 linear sub-buckets each (§20.14 (a)).
///
/// Covering cycles from 0 up to u64::MAX:
/// - Values 0..16: 16 linear buckets.
/// - Values >= 16: for each power-of-two interval [2^k, 2^(k+1)) where k in 4..=63,
///   16 linear subdivisions of width 2^(k-4).
///
/// Total buckets: 16 + (64 - 4) * 16 = 976.
pub const LOG2X16_BUCKETS: usize = 976;

#[derive(Clone, Debug)]
pub struct Log2x16Histogram {
    pub buckets: [u64; LOG2X16_BUCKETS],
    pub count: u64,
    pub min_val: u64,
    pub max_val: u64,
}

impl Default for Log2x16Histogram {
    fn default() -> Self {
        Self {
            buckets: [0u64; LOG2X16_BUCKETS],
            count: 0,
            min_val: u64::MAX,
            max_val: 0,
        }
    }
}

impl Log2x16Histogram {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline(always)]
    pub fn bucket_index(v: u64) -> usize {
        if v < 16 {
            v as usize
        } else {
            let k = 63 - v.leading_zeros() as usize;
            16 + (k - 4) * 16 + (((v >> (k - 4)) & 0x0F) as usize)
        }
    }

    #[inline(always)]
    pub fn record(&mut self, val: u64) {
        let idx = Self::bucket_index(val);
        self.buckets[idx] += 1;
        self.count += 1;
        if val > self.max_val {
            self.max_val = val;
        }
        if val < self.min_val {
            self.min_val = val;
        }
    }

    pub fn merge(&mut self, other: &Self) {
        for (dst, src) in self.buckets.iter_mut().zip(other.buckets.iter()) {
            *dst += *src;
        }
        self.count += other.count;
        if other.max_val > self.max_val {
            self.max_val = other.max_val;
        }
        if other.min_val < self.min_val {
            self.min_val = other.min_val;
        }
    }

    pub fn bucket_range(idx: usize) -> (u64, u64, u64) {
        if idx < 16 {
            let v = idx as u64;
            (v, v, v + 1)
        } else {
            let offset = idx - 16;
            let k = 4 + offset / 16;
            let sub = (offset % 16) as u64;
            let width = 1u64 << (k - 4);
            let lower = (16 + sub) << (k - 4);
            let upper = lower.saturating_add(width);
            let mid = lower + (width / 2);
            (lower, mid, upper)
        }
    }

    pub fn percentile_cycles(&self, p: f64) -> u64 {
        if self.count == 0 {
            return 0;
        }
        let target = ((self.count as f64) * p.clamp(0.0, 1.0)).ceil() as u64;
        let target = target.max(1);
        let mut accum = 0u64;
        for (idx, &c) in self.buckets.iter().enumerate() {
            accum += c;
            if accum >= target {
                let (_, mid, _) = Self::bucket_range(idx);
                return mid;
            }
        }
        self.max_val
    }
}

/// Sized collection of 4 operation-type histograms per thread (§20.14 (a)).
#[derive(Clone, Debug, Default)]
pub struct ThreadHistograms {
    pub read: Log2x16Histogram,
    pub update: Log2x16Histogram,
    pub insert: Log2x16Histogram,
    pub rmw: Log2x16Histogram,
}

impl ThreadHistograms {
    pub fn merge(&mut self, other: &Self) {
        self.read.merge(&other.read);
        self.update.merge(&other.update);
        self.insert.merge(&other.insert);
        self.rmw.merge(&other.rmw);
    }
}

/// Voluntary context switches for the calling thread (§20.14 (b)).
#[inline]
pub fn thread_nvcsw() -> u64 {
    #[cfg(all(target_os = "linux", feature = "std"))]
    {
        let mut ru = std::mem::MaybeUninit::<libc::rusage>::zeroed();
        // SAFETY: `ru.as_mut_ptr()` is a valid aligned pointer to `libc::rusage`,
        // and RUSAGE_THREAD queries the calling thread on Linux.
        let ret = unsafe { libc::getrusage(libc::RUSAGE_THREAD, ru.as_mut_ptr()) };
        if ret == 0 {
            // SAFETY: `getrusage` returned 0, so the struct was fully initialized by the kernel.
            let ru = unsafe { ru.assume_init() };
            return ru.ru_nvcsw as u64;
        }
    }
    0
}

/// Calibrates the cycle cost of one timestamp pair (`rdtsc` read pair) and converts to ns (§20.14 (a)).
pub fn calibrate_bracket_overhead(tsc_hz: u64, iterations: usize) -> (u64, f64) {
    if tsc_hz == 0 {
        return (0, 0.0);
    }
    let iters = iterations.max(1_000);
    let mut total_cycles = 0u64;
    for _ in 0..iters {
        let t0 = occ_stats::cycles_now();
        let t1 = occ_stats::cycles_now();
        total_cycles += t1.saturating_sub(t0);
    }
    let mean_cycles = total_cycles as f64 / iters as f64;
    let overhead_ns = (mean_cycles / tsc_hz as f64) * 1e9;
    (total_cycles / iters as u64, overhead_ns)
}

/// Distinct population keys in generator draw order.
pub fn generate_initial_population(n: usize) -> Vec<u64> {
    let mut rng = XorShift64::new(SEED_KEY_BASE);
    let mut keys = Vec::with_capacity(n);
    let mut seen = HashSet::with_capacity(n);
    while keys.len() < n {
        let k = rng.next_u64();
        if seen.insert(k) {
            keys.push(k);
        }
    }
    keys
}

/// The value thread `thread_id` writes at stream index `op_idx`: (t + 1) in the
/// top 8 bits, the index in the low 56 (§20.4). The prefill value is 0.
#[inline]
pub fn encode_write(thread_id: usize, op_idx: usize) -> u64 {
    ((thread_id as u64 + 1) << 56) | ((op_idx as u64 + 1) & 0x00FF_FFFF_FFFF_FFFF)
}

/// Pre-generates thread `thread_id`'s stream and the rank histogram of the
/// generator draws that produced it.
#[allow(clippy::too_many_arguments)]
pub fn generate_thread_stream(
    family: Family,
    thread_id: usize,
    num_threads: usize,
    ops_count: usize,
    population_n: usize,
    keys: &[u64],
    sorted_keys: &[u64],
    suite_seed: u64,
) -> (Vec<Op>, RankHistogram) {
    let thread_seed = suite_seed ^ ((thread_id as u64 + 1).wrapping_mul(PHI64));
    let mut rng = XorShift64::new(thread_seed);

    let zipf = if family.is_uniform() {
        None
    } else {
        Some(ZipfianGenerator::new(population_n as u64, ZIPFIAN_THETA))
    };

    // Ac and Fc: rank r is the r-th smallest key (§20.4). Everywhere else rank
    // r is the r-th key in generator draw order.
    let key_map = if family.is_contiguous_ranks() {
        sorted_keys
    } else {
        keys
    };

    let mut stream = Vec::with_capacity(ops_count);
    let mut hist = RankHistogram::default();
    let mut insert_j = 0u64;

    for op_idx in 0..ops_count {
        let u = rng.next_f64();
        let draw_rank = match &zipf {
            Some(z) => z.next(u) as usize,
            None => (rng.next_u64() % (population_n as u64)) as usize,
        };
        hist.record(draw_rank);

        let write_val = encode_write(thread_id, op_idx);
        let roll = rng.next_u64() % 100;

        match family {
            Family::A | Family::A0 | Family::Ac | Family::ADram => {
                // 50% read, 50% update
                let k = key_map[draw_rank % population_n];
                if roll < 50 {
                    stream.push(Op::Read { key: k });
                } else {
                    stream.push(Op::Update {
                        key: k,
                        val: write_val,
                    });
                }
            }
            Family::B | Family::B0 => {
                // 95% read, 5% update
                let k = key_map[draw_rank % population_n];
                if roll < 95 {
                    stream.push(Op::Read { key: k });
                } else {
                    stream.push(Op::Update {
                        key: k,
                        val: write_val,
                    });
                }
            }
            Family::C | Family::C0 | Family::CDram => {
                // 100% read
                let k = key_map[draw_rank % population_n];
                stream.push(Op::Read { key: k });
            }
            Family::D => {
                // 95% read over recency, 5% insert (§20.4, "D, in full").
                if roll < 95 {
                    let rho = draw_rank as u64;
                    let k = if rho < insert_j {
                        // Thread t's (rho + 1)-th most recent insert.
                        let prev_idx = insert_j - 1 - rho;
                        (1u64 << 63) + 1 + prev_idx * (num_threads as u64) + (thread_id as u64)
                    } else {
                        // rho - (t's insert count) places from the end of draw order.
                        let delta = rho - insert_j;
                        let pop_idx = (population_n as u64 - 1).saturating_sub(delta) as usize;
                        keys[pop_idx % population_n]
                    };
                    stream.push(Op::Read { key: k });
                } else {
                    // Thread t's j-th insert is key 2^63 + 1 + j·T + t.
                    let k = (1u64 << 63) + 1 + insert_j * (num_threads as u64) + (thread_id as u64);
                    insert_j += 1;
                    stream.push(Op::Insert {
                        key: k,
                        val: write_val,
                    });
                }
            }
            Family::F | Family::F0 | Family::Fc => {
                // 50% read, 50% read-modify-write
                let k = key_map[draw_rank % population_n];
                if roll < 50 {
                    stream.push(Op::Read { key: k });
                } else {
                    stream.push(Op::ReadModifyWrite { key: k });
                }
            }
        }
    }

    (stream, hist)
}

/// What the streams say the structure must hold after the window, tallied
/// before it (§20.5, §20.15).
#[derive(Debug, Default)]
pub struct Expected {
    /// Per key, each writing thread's **last** update value — at most T entries.
    pub last_writes: HashMap<u64, Vec<u64>>,
    /// Per key, the number of read-modify-writes the streams contain.
    pub rmw_counts: HashMap<u64, u64>,
    /// Every D insert with exactly its own value; one thread owns each key.
    pub d_inserts: HashMap<u64, u64>,
    pub read_ops: u64,
    pub update_ops: u64,
    pub insert_ops: u64,
    pub rmw_ops: u64,
}

impl Expected {
    /// `write_ops` as the artifact names it: updates, inserts and RMWs.
    pub fn write_ops(&self) -> u64 {
        self.update_ops + self.insert_ops + self.rmw_ops
    }
}

/// Tallies the per-(thread, key) last write, the per-key RMW count and the D
/// inserts from the streams.
pub fn tally_expected(streams: &[Vec<Op>]) -> Expected {
    let mut exp = Expected::default();
    for stream in streams {
        // This thread's last write per key: a later write replaces an earlier one.
        let mut last: HashMap<u64, u64> = HashMap::new();
        for op in stream {
            match *op {
                Op::Read { .. } => exp.read_ops += 1,
                Op::Update { key, val } => {
                    exp.update_ops += 1;
                    last.insert(key, val);
                }
                Op::Insert { key, val } => {
                    exp.insert_ops += 1;
                    exp.d_inserts.insert(key, val);
                }
                Op::ReadModifyWrite { key } => {
                    exp.rmw_ops += 1;
                    *exp.rmw_counts.entry(key).or_insert(0) += 1;
                }
            }
        }
        for (key, val) in last {
            exp.last_writes.entry(key).or_default().push(val);
        }
    }
    exp
}

/// One arm's structure behind the four stream operations.
///
/// Write methods return `Some(fold)` when the key was present — `fold` is the
/// operation's return value where it has one (the previous value for a map
/// `insert` and for `fetch_add`) and 0 for a value-cell `store`, which returns
/// nothing — and `None` when it was absent. For `update` and `rmw` an absent
/// key is a miss (§20.4: every operation names a present key); for `insert`
/// `None` is the success case, a key that was absent.
pub trait ArmStore: Send + Sync + 'static {
    /// Per-thread state registered before the barrier and dropped after the window.
    type Handle: Send + 'static;

    fn handle(this: &Arc<Self>) -> Self::Handle;
    fn read(&self, h: &Self::Handle, key: u64) -> Option<u64>;
    fn update(&self, h: &Self::Handle, key: u64, val: u64) -> Option<u64>;
    fn insert(&self, h: &Self::Handle, key: u64, val: u64) -> Option<u64>;
    fn rmw(&self, h: &Self::Handle, key: u64) -> Option<u64>;

    /// Post-window, untimed.
    fn final_value(&self, key: u64) -> Option<u64>;
    fn final_len(&self) -> u64;
    fn mem_used(&self) -> Option<usize> {
        None
    }
    /// Contended stripe-lock acquisitions (`occ-stats` build, `olc` only).
    fn contended_acquisitions(&self) -> u64 {
        0
    }
    /// Node census for trie stores (§20.15).
    fn node_census(&self) -> Option<ExpanseStats> {
        None
    }
}

/// `olc` — `SyncExpanseMap` with the D1 striped lock around its RMW.
pub struct OlcStore {
    map: Arc<SyncExpanseMap>,
    stripes: Vec<CachePadded<Mutex<()>>>,
    contended: AtomicU64,
    /// Test-only negative control (§20.5): skip the striped lock. Absent from
    /// the harness build, so the timed loop carries no branch for it.
    #[cfg(test)]
    skip_stripe_lock: bool,
}

impl OlcStore {
    pub fn prefilled(sorted_keys: &[u64]) -> Self {
        let map = Arc::new(SyncExpanseMap::new());
        for &k in sorted_keys {
            map.insert(k, 0);
        }
        let mut stripes = Vec::with_capacity(NUM_STRIPES);
        for _ in 0..NUM_STRIPES {
            stripes.push(CachePadded(Mutex::new(())));
        }
        Self {
            map,
            stripes,
            contended: AtomicU64::new(0),
            #[cfg(test)]
            skip_stripe_lock: false,
        }
    }

    /// The §20.5 negative control: the same store with the striped lock skipped.
    #[cfg(test)]
    pub fn prefilled_without_stripe_lock(sorted_keys: &[u64]) -> Self {
        let mut s = Self::prefilled(sorted_keys);
        s.skip_stripe_lock = true;
        s
    }

    #[inline(always)]
    fn stripe_index(&self, key: u64) -> usize {
        (key.wrapping_mul(PHI64) >> 54) as usize % self.stripes.len()
    }

    #[inline(always)]
    fn rmw_unlocked(&self, reader: &OwnedMapReader, key: u64) -> Option<u64> {
        // The thread's registered reader: `SyncExpanseMap::get` registers a
        // throwaway reader per call.
        let cur = reader.get(key)?;
        self.map.insert(key, cur + 1)
    }
}

impl ArmStore for OlcStore {
    type Handle = OwnedMapReader;

    fn handle(this: &Arc<Self>) -> Self::Handle {
        this.map.owned_reader()
    }

    #[inline(always)]
    fn read(&self, h: &Self::Handle, key: u64) -> Option<u64> {
        h.get(key)
    }

    #[inline(always)]
    fn update(&self, _h: &Self::Handle, key: u64, val: u64) -> Option<u64> {
        self.map.insert(key, val)
    }

    #[inline(always)]
    fn insert(&self, _h: &Self::Handle, key: u64, val: u64) -> Option<u64> {
        self.map.insert(key, val)
    }

    #[inline(always)]
    fn rmw(&self, h: &Self::Handle, key: u64) -> Option<u64> {
        #[cfg(test)]
        if self.skip_stripe_lock {
            return self.rmw_unlocked(h, key);
        }
        let stripe = &self.stripes[self.stripe_index(key)].0;
        // §20.14 (b): `try_lock` first and count the failures, in the
        // `occ-stats` build only. The throughput build has no such branch.
        #[cfg(feature = "occ-stats")]
        let _guard = match stripe.try_lock() {
            Ok(g) => g,
            Err(std::sync::TryLockError::WouldBlock) => {
                self.contended.fetch_add(1, Ordering::Relaxed);
                stripe.lock().unwrap()
            }
            Err(std::sync::TryLockError::Poisoned(e)) => panic!("stripe lock poisoned: {e}"),
        };
        #[cfg(not(feature = "occ-stats"))]
        let _guard = stripe.lock().unwrap();
        self.rmw_unlocked(h, key)
    }

    fn final_value(&self, key: u64) -> Option<u64> {
        self.map.get(key)
    }

    fn final_len(&self) -> u64 {
        self.map.len()
    }

    fn mem_used(&self) -> Option<usize> {
        Some(self.map.mem_used())
    }

    fn contended_acquisitions(&self) -> u64 {
        self.contended.load(Ordering::Relaxed)
    }

    fn node_census(&self) -> Option<ExpanseStats> {
        Some(self.map.with_locked(|m| m.stats()))
    }
}

/// `mutex` — `Mutex<ExpanseMap>`, the α = 1 control.
pub struct MutexStore {
    map: Mutex<ExpanseMap>,
}

impl MutexStore {
    pub fn prefilled(sorted_keys: &[u64]) -> Self {
        let mut map = ExpanseMap::new();
        for &k in sorted_keys {
            map.insert(k, 0);
        }
        Self {
            map: Mutex::new(map),
        }
    }
}

impl ArmStore for MutexStore {
    type Handle = ();

    fn handle(_this: &Arc<Self>) -> Self::Handle {}

    #[inline(always)]
    fn read(&self, _h: &(), key: u64) -> Option<u64> {
        self.map.lock().unwrap().get(key)
    }

    #[inline(always)]
    fn update(&self, _h: &(), key: u64, val: u64) -> Option<u64> {
        self.map.lock().unwrap().insert(key, val)
    }

    #[inline(always)]
    fn insert(&self, _h: &(), key: u64, val: u64) -> Option<u64> {
        self.map.lock().unwrap().insert(key, val)
    }

    #[inline(always)]
    fn rmw(&self, _h: &(), key: u64) -> Option<u64> {
        let mut guard = self.map.lock().unwrap();
        let cur = guard.get(key)?;
        guard.insert(key, cur + 1)
    }

    fn final_value(&self, key: u64) -> Option<u64> {
        self.map.lock().unwrap().get(key)
    }

    fn final_len(&self) -> u64 {
        self.map.lock().unwrap().len()
    }

    fn mem_used(&self) -> Option<usize> {
        Some(self.map.lock().unwrap().mem_used())
    }

    fn node_census(&self) -> Option<ExpanseStats> {
        Some(self.map.lock().unwrap().stats())
    }
}

/// `skip` — `SkipMap<u64, AtomicU64>`, the value-cell idiom.
pub struct SkipStore {
    map: SkipMap<u64, AtomicU64>,
}

impl SkipStore {
    pub fn prefilled(sorted_keys: &[u64]) -> Self {
        let map = SkipMap::new();
        for &k in sorted_keys {
            map.insert(k, AtomicU64::new(0));
        }
        Self { map }
    }
}

impl ArmStore for SkipStore {
    type Handle = ();

    fn handle(_this: &Arc<Self>) -> Self::Handle {}

    #[inline(always)]
    fn read(&self, _h: &(), key: u64) -> Option<u64> {
        self.map
            .get(&key)
            .map(|e| e.value().load(Ordering::Relaxed))
    }

    #[inline(always)]
    fn update(&self, _h: &(), key: u64, val: u64) -> Option<u64> {
        let e = self.map.get(&key)?;
        e.value().store(val, Ordering::Relaxed);
        Some(0)
    }

    #[inline(always)]
    fn insert(&self, _h: &(), key: u64, val: u64) -> Option<u64> {
        // `SkipMap::insert` returns the new entry and no "was present" flag;
        // the post-window count check is what shows an insert met a present key.
        let e = self.map.insert(key, AtomicU64::new(val));
        std::hint::black_box(e.value().load(Ordering::Relaxed));
        None
    }

    #[inline(always)]
    fn rmw(&self, _h: &(), key: u64) -> Option<u64> {
        let e = self.map.get(&key)?;
        Some(e.value().fetch_add(1, Ordering::Relaxed))
    }

    fn final_value(&self, key: u64) -> Option<u64> {
        self.map
            .get(&key)
            .map(|e| e.value().load(Ordering::Relaxed))
    }

    fn final_len(&self) -> u64 {
        self.map.len() as u64
    }
}

/// `dash` — `DashMap<u64, AtomicU64>` at `DASH_SHARDS` shards.
pub struct DashStore {
    map: DashMap<u64, AtomicU64>,
}

impl DashStore {
    pub fn prefilled(sorted_keys: &[u64]) -> Self {
        let map = DashMap::with_shard_amount(DASH_SHARDS);
        for &k in sorted_keys {
            map.insert(k, AtomicU64::new(0));
        }
        Self { map }
    }
}

impl ArmStore for DashStore {
    type Handle = ();

    fn handle(_this: &Arc<Self>) -> Self::Handle {}

    #[inline(always)]
    fn read(&self, _h: &(), key: u64) -> Option<u64> {
        self.map
            .get(&key)
            .map(|e| e.value().load(Ordering::Relaxed))
    }

    #[inline(always)]
    fn update(&self, _h: &(), key: u64, val: u64) -> Option<u64> {
        let e = self.map.get(&key)?;
        e.value().store(val, Ordering::Relaxed);
        Some(0)
    }

    #[inline(always)]
    fn insert(&self, _h: &(), key: u64, val: u64) -> Option<u64> {
        self.map
            .insert(key, AtomicU64::new(val))
            .map(AtomicU64::into_inner)
    }

    #[inline(always)]
    fn rmw(&self, _h: &(), key: u64) -> Option<u64> {
        let e = self.map.get(&key)?;
        Some(e.value().fetch_add(1, Ordering::Relaxed))
    }

    fn final_value(&self, key: u64) -> Option<u64> {
        self.map
            .get(&key)
            .map(|e| e.value().load(Ordering::Relaxed))
    }

    fn final_len(&self) -> u64 {
        self.map.len() as u64
    }
}

/// `rwbtree` — `RwLock<BTreeMap<u64, AtomicU64>>`.
pub struct RwBTreeStore {
    map: RwLock<BTreeMap<u64, AtomicU64>>,
}

impl RwBTreeStore {
    pub fn prefilled(sorted_keys: &[u64]) -> Self {
        let mut map = BTreeMap::new();
        for &k in sorted_keys {
            map.insert(k, AtomicU64::new(0));
        }
        Self {
            map: RwLock::new(map),
        }
    }
}

impl ArmStore for RwBTreeStore {
    type Handle = ();

    fn handle(_this: &Arc<Self>) -> Self::Handle {}

    #[inline(always)]
    fn read(&self, _h: &(), key: u64) -> Option<u64> {
        self.map
            .read()
            .unwrap()
            .get(&key)
            .map(|v| v.load(Ordering::Relaxed))
    }

    #[inline(always)]
    fn update(&self, _h: &(), key: u64, val: u64) -> Option<u64> {
        let guard = self.map.read().unwrap();
        guard.get(&key)?.store(val, Ordering::Relaxed);
        Some(0)
    }

    #[inline(always)]
    fn insert(&self, _h: &(), key: u64, val: u64) -> Option<u64> {
        self.map
            .write()
            .unwrap()
            .insert(key, AtomicU64::new(val))
            .map(AtomicU64::into_inner)
    }

    #[inline(always)]
    fn rmw(&self, _h: &(), key: u64) -> Option<u64> {
        let guard = self.map.read().unwrap();
        Some(guard.get(&key)?.fetch_add(1, Ordering::Relaxed))
    }

    fn final_value(&self, key: u64) -> Option<u64> {
        self.map
            .read()
            .unwrap()
            .get(&key)
            .map(|v| v.load(Ordering::Relaxed))
    }

    fn final_len(&self) -> u64 {
        self.map.read().unwrap().len() as u64
    }
}

/// What one thread's timed loop observed.
#[derive(Clone, Copy, Debug, Default)]
pub struct LoopTally {
    /// Every read's value and every write's return value, folded (§8.6).
    pub sink: u64,
    pub read_misses: u64,
    pub write_misses: u64,
    /// Inserts whose key was absent.
    pub successful_inserts: u64,
    /// Inserts whose key was already present (must be 0: one thread owns each key).
    pub insert_collisions: u64,
}

/// The timed operation loop — the same code in every arm.
///
/// It borrows the stream: consuming the `Vec` here would put its deallocation
/// inside the caller's timer (AGENTS.md §8.6).
#[inline(never)]
pub fn op_loop<S: ArmStore>(store: &S, handle: &S::Handle, stream: &[Op]) -> LoopTally {
    let mut t = LoopTally::default();
    for op in stream {
        match *op {
            Op::Read { key } => match store.read(handle, key) {
                Some(v) => t.sink = t.sink.wrapping_add(v),
                None => t.read_misses += 1,
            },
            Op::Update { key, val } => match store.update(handle, key, val) {
                Some(v) => t.sink = t.sink.wrapping_add(v),
                None => t.write_misses += 1,
            },
            Op::Insert { key, val } => match store.insert(handle, key, val) {
                None => t.successful_inserts += 1,
                Some(v) => {
                    t.sink = t.sink.wrapping_add(v);
                    t.insert_collisions += 1;
                }
            },
            Op::ReadModifyWrite { key } => match store.rmw(handle, key) {
                Some(v) => t.sink = t.sink.wrapping_add(v),
                None => t.write_misses += 1,
            },
        }
    }
    t
}

/// What a thread hands back through its join value, so that nothing it owns is
/// dropped before the window closes.
struct ThreadReturn<H> {
    elapsed_s: f64,
    tally: LoopTally,
    nvcsw: u64,
    _stream: Vec<Op>,
    _handle: H,
}

/// The window's measurements.
#[derive(Debug)]
pub struct WindowOutcome {
    /// Barrier release to the join of the last thread.
    pub elapsed_s: f64,
    pub thread_elapsed_s: Vec<f64>,
    pub tally: LoopTally,
    /// `occ_stats` deltas over the window; `None` without the feature.
    pub counters: Option<[u64; NUM_STATS]>,
    pub total_nvcsw: u64,
}

/// Runs the streams, one thread each, and times the window.
pub fn run_window<S: ArmStore>(store: &Arc<S>, streams: Vec<Vec<Op>>) -> WindowOutcome {
    let threads = streams.len();
    let barrier = Arc::new(Barrier::new(threads + 1));
    let mut handles = Vec::with_capacity(threads);

    for stream in streams {
        let store = Arc::clone(store);
        let barrier = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            // Reader-handle registration is outside the window (§20.4).
            let handle = S::handle(&store);
            let nvcsw_start = thread_nvcsw();
            barrier.wait();
            let start = Instant::now();
            let tally = op_loop(&*store, &handle, &stream);
            // The end instant is taken before anything this thread owns can
            // drop; the stream and the handle leave through the join value.
            let elapsed_s = start.elapsed().as_secs_f64();
            let nvcsw_end = thread_nvcsw();
            let nvcsw = nvcsw_end.saturating_sub(nvcsw_start);
            ThreadReturn {
                elapsed_s,
                tally,
                nvcsw,
                _stream: stream,
                _handle: handle,
            }
        }));
    }

    let snap0 = cfg!(feature = "occ-stats").then(occ_stats::snapshot);
    barrier.wait();
    let window_start = Instant::now();
    let returns: Vec<ThreadReturn<S::Handle>> =
        handles.into_iter().map(|h| h.join().unwrap()).collect();
    let elapsed_s = window_start.elapsed().as_secs_f64();
    let counters = snap0.map(|s0| {
        let s1 = occ_stats::snapshot();
        let mut d = [0u64; NUM_STATS];
        for i in 0..NUM_STATS {
            d[i] = s1[i].saturating_sub(s0[i]);
        }
        d
    });

    let mut tally = LoopTally::default();
    let mut thread_elapsed_s = Vec::with_capacity(threads);
    let mut total_nvcsw = 0u64;
    for r in &returns {
        thread_elapsed_s.push(r.elapsed_s);
        tally.sink = tally.sink.wrapping_add(r.tally.sink);
        tally.read_misses += r.tally.read_misses;
        tally.write_misses += r.tally.write_misses;
        tally.successful_inserts += r.tally.successful_inserts;
        tally.insert_collisions += r.tally.insert_collisions;
        total_nvcsw += r.nvcsw;
    }
    std::hint::black_box(tally.sink);
    // Streams and reader handles drop here, after the window.
    drop(returns);

    WindowOutcome {
        elapsed_s,
        thread_elapsed_s,
        tally,
        counters,
        total_nvcsw,
    }
}

/// The post-window oracle's findings (§20.5, §20.15).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OracleReport {
    /// Population keys whose final value is not one the streams allow, plus D
    /// inserts absent or holding another value, plus absent population keys.
    pub per_key_mismatches: u64,
    /// |Σ values − RMW operations| in an RMW family; 0 elsewhere.
    pub lost_updates: u64,
    /// Σ of the population's final values (wrapping outside the RMW families).
    pub value_sum: u64,
    pub missing_population_keys: u64,
    pub final_count: u64,
    pub expected_final_count: u64,
}

/// Checks every population key, every D insert and the final count.
///
/// A key's final value must be one of the per-thread **last** writes to it; 0
/// (the prefill) is accepted only for a key no stream wrote. In an RMW family
/// it must equal the key's RMW count exactly.
pub fn verify_oracle<S: ArmStore>(
    store: &S,
    family: Family,
    population: &[u64],
    expected: &Expected,
    successful_inserts: u64,
) -> OracleReport {
    let mut rep = OracleReport::default();

    for &k in population {
        let Some(v) = store.final_value(k) else {
            rep.missing_population_keys += 1;
            rep.per_key_mismatches += 1;
            continue;
        };
        rep.value_sum = rep.value_sum.wrapping_add(v);
        let ok = if family.is_rmw() {
            v == expected.rmw_counts.get(&k).copied().unwrap_or(0)
        } else {
            match expected.last_writes.get(&k) {
                Some(candidates) => candidates.contains(&v),
                None => v == 0,
            }
        };
        if !ok {
            rep.per_key_mismatches += 1;
        }
    }

    if family.is_rmw() {
        rep.lost_updates = expected.rmw_ops.abs_diff(rep.value_sum);
    }

    for (&k, &want) in &expected.d_inserts {
        if store.final_value(k) != Some(want) {
            rep.per_key_mismatches += 1;
        }
    }

    rep.final_count = store.final_len();
    rep.expected_final_count = population.len() as u64 + successful_inserts;
    rep
}

/// The oracle's verdict as the diagnostic string the driver and the tests
/// assert on (AGENTS.md §5: the string, never an exit code).
pub fn oracle_verdict(
    family: Family,
    rep: &OracleReport,
    tally: &LoopTally,
    expected: &Expected,
) -> String {
    if family.is_rmw() && (rep.lost_updates != 0 || rep.per_key_mismatches != 0) {
        return format!(
            "VOID_LOST_UPDATE: value_sum {} against {} read-modify-writes ({} lost), {} per-key mismatches",
            rep.value_sum, expected.rmw_ops, rep.lost_updates, rep.per_key_mismatches
        );
    }
    let mut problems = Vec::new();
    if rep.per_key_mismatches != 0 {
        problems.push(format!("{} per-key mismatches", rep.per_key_mismatches));
    }
    if rep.missing_population_keys != 0 {
        problems.push(format!(
            "{} population keys absent",
            rep.missing_population_keys
        ));
    }
    if rep.final_count != rep.expected_final_count {
        problems.push(format!(
            "final count {} against population plus successful inserts {}",
            rep.final_count, rep.expected_final_count
        ));
    }
    if tally.successful_inserts != expected.insert_ops || tally.insert_collisions != 0 {
        problems.push(format!(
            "{} successful inserts of {} in the streams, {} met a present key",
            tally.successful_inserts, expected.insert_ops, tally.insert_collisions
        ));
    }
    if tally.read_misses != 0 || tally.write_misses != 0 {
        problems.push(format!(
            "{} read misses and {} write misses against a registered 100% hit rate",
            tally.read_misses, tally.write_misses
        ));
    }
    if problems.is_empty() {
        "PASS".to_string()
    } else {
        format!("VOID_ORACLE: {}", problems.join("; "))
    }
}

/// One cell, run and checked.
#[derive(Debug)]
pub struct CellOutcome {
    pub window: WindowOutcome,
    pub oracle: OracleReport,
    pub verdict: String,
    pub mem_used: Option<usize>,
    pub contended_acquisitions: u64,
    pub total_nvcsw: u64,
}

/// Window, then oracle, over an already prefilled store.
pub fn run_store<S: ArmStore>(
    store: S,
    family: Family,
    population: &[u64],
    streams: Vec<Vec<Op>>,
    expected: &Expected,
) -> CellOutcome {
    let store = Arc::new(store);
    let window = run_window(&store, streams);
    let oracle = verify_oracle(
        &*store,
        family,
        population,
        expected,
        window.tally.successful_inserts,
    );
    let verdict = oracle_verdict(family, &oracle, &window.tally, expected);
    CellOutcome {
        mem_used: store.mem_used(),
        contended_acquisitions: store.contended_acquisitions(),
        total_nvcsw: window.total_nvcsw,
        window,
        oracle,
        verdict,
    }
}

/// Prefills the arm in ascending key order (§20.4, D10) and runs the cell.
pub fn run_cell(
    arm: Arm,
    family: Family,
    sorted_keys: &[u64],
    streams: Vec<Vec<Op>>,
    expected: &Expected,
) -> CellOutcome {
    match arm {
        Arm::Olc => run_store(
            OlcStore::prefilled(sorted_keys),
            family,
            sorted_keys,
            streams,
            expected,
        ),
        Arm::Mutex => run_store(
            MutexStore::prefilled(sorted_keys),
            family,
            sorted_keys,
            streams,
            expected,
        ),
        Arm::Skip => run_store(
            SkipStore::prefilled(sorted_keys),
            family,
            sorted_keys,
            streams,
            expected,
        ),
        Arm::Dash => run_store(
            DashStore::prefilled(sorted_keys),
            family,
            sorted_keys,
            streams,
            expected,
        ),
        Arm::RwBTree => run_store(
            RwBTreeStore::prefilled(sorted_keys),
            family,
            sorted_keys,
            streams,
            expected,
        ),
    }
}

// ---------------------------------------------------------------------------
// Latency Role (§20.14 (a))
// ---------------------------------------------------------------------------

/// Sized, non-allocating timed operation loop for the latency role (§20.14 (a)).
#[inline(never)]
pub fn op_loop_latency<S: ArmStore>(
    store: &S,
    handle: &S::Handle,
    stream: &[Op],
    hists: &mut ThreadHistograms,
) -> LoopTally {
    let mut t = LoopTally::default();
    for op in stream {
        match *op {
            Op::Read { key } => {
                let t0 = occ_stats::cycles_now();
                let res = store.read(handle, key);
                let t1 = occ_stats::cycles_now();
                hists.read.record(t1.saturating_sub(t0));
                match res {
                    Some(v) => t.sink = t.sink.wrapping_add(v),
                    None => t.read_misses += 1,
                }
            }
            Op::Update { key, val } => {
                let t0 = occ_stats::cycles_now();
                let res = store.update(handle, key, val);
                let t1 = occ_stats::cycles_now();
                hists.update.record(t1.saturating_sub(t0));
                match res {
                    Some(v) => t.sink = t.sink.wrapping_add(v),
                    None => t.write_misses += 1,
                }
            }
            Op::Insert { key, val } => {
                let t0 = occ_stats::cycles_now();
                let res = store.insert(handle, key, val);
                let t1 = occ_stats::cycles_now();
                hists.insert.record(t1.saturating_sub(t0));
                match res {
                    None => t.successful_inserts += 1,
                    Some(v) => {
                        t.sink = t.sink.wrapping_add(v);
                        t.insert_collisions += 1;
                    }
                }
            }
            Op::ReadModifyWrite { key } => {
                let t0 = occ_stats::cycles_now();
                let res = store.rmw(handle, key);
                let t1 = occ_stats::cycles_now();
                hists.rmw.record(t1.saturating_sub(t0));
                match res {
                    Some(v) => t.sink = t.sink.wrapping_add(v),
                    None => t.write_misses += 1,
                }
            }
        }
    }
    t
}

struct ThreadLatencyReturn<H> {
    tally: LoopTally,
    hists: ThreadHistograms,
    nvcsw: u64,
    _stream: Vec<Op>,
    _handle: H,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LatencyRecord {
    pub family: &'static str,
    pub arm: &'static str,
    pub threads: usize,
    pub op: &'static str,
    pub samples: u64,
    pub p50_ns: f64,
    pub p99_ns: f64,
    pub p999_ns: f64,
    pub max_ns: f64,
    pub tsc_hz: u64,
    pub bucket_scheme: &'static str,
    pub clock: &'static str,
    pub model: &'static str,
    pub bracket_overhead_ns: f64,
}

impl LatencyRecord {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"family\":\"{}\",\"arm\":\"{}\",\"threads\":{},\"op\":\"{}\",\"samples\":{},\"p50_ns\":{:.3},\"p99_ns\":{:.3},\"p999_ns\":{:.3},\"max_ns\":{:.3},\"tsc_hz\":{},\"bucket_scheme\":\"{}\",\"clock\":\"{}\",\"model\":\"{}\",\"bracket_overhead_ns\":{:.3}}}",
            self.family,
            self.arm,
            self.threads,
            self.op,
            self.samples,
            self.p50_ns,
            self.p99_ns,
            self.p999_ns,
            self.max_ns,
            self.tsc_hz,
            self.bucket_scheme,
            self.clock,
            self.model,
            self.bracket_overhead_ns,
        )
    }
}

#[derive(Debug)]
pub struct CellOutcomeLatency {
    pub latency: Vec<LatencyRecord>,
    pub tally: LoopTally,
    pub oracle: OracleReport,
    pub verdict: String,
    pub mem_used: Option<usize>,
    pub tsc_hz: u64,
    pub bracket_overhead_ns: f64,
    pub total_nvcsw: u64,
}

pub fn run_window_latency<S: ArmStore>(
    store: &Arc<S>,
    family: Family,
    arm: Arm,
    streams: Vec<Vec<Op>>,
    tsc_hz: u64,
    bracket_overhead_ns: f64,
) -> (Vec<LatencyRecord>, LoopTally, u64) {
    let threads = streams.len();
    let barrier = Arc::new(Barrier::new(threads + 1));
    let mut handles = Vec::with_capacity(threads);

    for stream in streams {
        let store = Arc::clone(store);
        let barrier = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            let handle = S::handle(&store);
            let mut hists = ThreadHistograms::default();
            let nvcsw_start = thread_nvcsw();
            barrier.wait();
            let tally = op_loop_latency(&*store, &handle, &stream, &mut hists);
            let nvcsw_end = thread_nvcsw();
            let nvcsw = nvcsw_end.saturating_sub(nvcsw_start);
            ThreadLatencyReturn {
                tally,
                hists,
                nvcsw,
                _stream: stream,
                _handle: handle,
            }
        }));
    }

    barrier.wait();
    let returns: Vec<ThreadLatencyReturn<S::Handle>> =
        handles.into_iter().map(|h| h.join().unwrap()).collect();

    let mut tally = LoopTally::default();
    let mut merged_hists = ThreadHistograms::default();
    let mut total_nvcsw = 0u64;

    for r in &returns {
        tally.sink = tally.sink.wrapping_add(r.tally.sink);
        tally.read_misses += r.tally.read_misses;
        tally.write_misses += r.tally.write_misses;
        tally.successful_inserts += r.tally.successful_inserts;
        tally.insert_collisions += r.tally.insert_collisions;
        total_nvcsw += r.nvcsw;
        merged_hists.merge(&r.hists);
    }
    std::hint::black_box(tally.sink);
    drop(returns);

    let tsc_hz_f64 = if tsc_hz > 0 { tsc_hz as f64 } else { 1.0 };
    let mut records = Vec::new();

    let ops_to_check: [(&str, &Log2x16Histogram); 4] = [
        ("read", &merged_hists.read),
        ("update", &merged_hists.update),
        ("insert", &merged_hists.insert),
        ("rmw", &merged_hists.rmw),
    ];

    for (op_name, hist) in ops_to_check {
        if hist.count > 0 {
            records.push(LatencyRecord {
                family: family.as_str(),
                arm: arm.as_str(),
                threads,
                op: op_name,
                samples: hist.count,
                p50_ns: (hist.percentile_cycles(0.50) as f64 / tsc_hz_f64) * 1e9,
                p99_ns: (hist.percentile_cycles(0.99) as f64 / tsc_hz_f64) * 1e9,
                p999_ns: (hist.percentile_cycles(0.999) as f64 / tsc_hz_f64) * 1e9,
                max_ns: (hist.max_val as f64 / tsc_hz_f64) * 1e9,
                tsc_hz,
                bucket_scheme: "log2x16",
                clock: "rdtsc_over_tsc_hz",
                model: "closed_loop_service_time",
                bracket_overhead_ns,
            });
        }
    }

    (records, tally, total_nvcsw)
}

#[allow(clippy::too_many_arguments)]
pub fn run_store_latency<S: ArmStore>(
    store: S,
    family: Family,
    arm: Arm,
    population: &[u64],
    streams: Vec<Vec<Op>>,
    expected: &Expected,
    tsc_hz: u64,
    bracket_overhead_ns: f64,
) -> CellOutcomeLatency {
    let store = Arc::new(store);
    let (latency, tally, total_nvcsw) =
        run_window_latency(&store, family, arm, streams, tsc_hz, bracket_overhead_ns);
    let oracle = verify_oracle(
        &*store,
        family,
        population,
        expected,
        tally.successful_inserts,
    );
    let verdict = oracle_verdict(family, &oracle, &tally, expected);
    CellOutcomeLatency {
        mem_used: store.mem_used(),
        latency,
        tally,
        oracle,
        verdict,
        tsc_hz,
        bracket_overhead_ns,
        total_nvcsw,
    }
}

pub fn run_cell_latency(
    arm: Arm,
    family: Family,
    sorted_keys: &[u64],
    streams: Vec<Vec<Op>>,
    expected: &Expected,
    quick: bool,
) -> CellOutcomeLatency {
    let tsc_window = if quick {
        Duration::from_millis(20)
    } else {
        Duration::from_millis(200)
    };
    let tsc_hz = occ_stats::cycles_hz(tsc_window);
    let (_, bracket_overhead_ns) = calibrate_bracket_overhead(tsc_hz, 100_000);

    match arm {
        Arm::Olc => run_store_latency(
            OlcStore::prefilled(sorted_keys),
            family,
            arm,
            sorted_keys,
            streams,
            expected,
            tsc_hz,
            bracket_overhead_ns,
        ),
        Arm::Mutex => run_store_latency(
            MutexStore::prefilled(sorted_keys),
            family,
            arm,
            sorted_keys,
            streams,
            expected,
            tsc_hz,
            bracket_overhead_ns,
        ),
        Arm::Skip => run_store_latency(
            SkipStore::prefilled(sorted_keys),
            family,
            arm,
            sorted_keys,
            streams,
            expected,
            tsc_hz,
            bracket_overhead_ns,
        ),
        Arm::Dash => run_store_latency(
            DashStore::prefilled(sorted_keys),
            family,
            arm,
            sorted_keys,
            streams,
            expected,
            tsc_hz,
            bracket_overhead_ns,
        ),
        Arm::RwBTree => run_store_latency(
            RwBTreeStore::prefilled(sorted_keys),
            family,
            arm,
            sorted_keys,
            streams,
            expected,
            tsc_hz,
            bracket_overhead_ns,
        ),
    }
}

// ---------------------------------------------------------------------------
// Untimed Monotonicity Pass & Node Census (§20.15)
// ---------------------------------------------------------------------------

/// Outcome of the untimed monotonicity pass (§20.15).
#[derive(Debug)]
pub struct MonotonicityOutcome {
    pub monotonicity_violations: u64,
    pub tracked_keys: usize,
    pub oracle: OracleReport,
    pub verdict: String,
    pub node_census: Option<ExpanseStats>,
    pub mem_used: Option<usize>,
}

pub fn run_monotonicity_pass<S: ArmStore>(
    store: &Arc<S>,
    family: Family,
    population: &[u64],
    tracked_keys: &[u64],
    streams: Vec<Vec<Op>>,
    expected: &Expected,
) -> MonotonicityOutcome {
    let threads = streams.len();
    let barrier = Arc::new(Barrier::new(threads + 1));
    let num_tracked = tracked_keys.len();

    // Map each tracked key to index in 0..num_tracked
    let key_to_idx: Arc<HashMap<u64, usize>> = Arc::new(
        tracked_keys
            .iter()
            .enumerate()
            .map(|(i, &k)| (k, i))
            .collect(),
    );

    let mut handles = Vec::with_capacity(threads);
    for stream in streams {
        let store = Arc::clone(store);
        let barrier = Arc::clone(&barrier);
        let key_to_idx = Arc::clone(&key_to_idx);
        handles.push(std::thread::spawn(move || {
            let handle = S::handle(&store);
            // Array sized before the start: [reader, key, writer]
            let mut last_seen_seq = vec![[0u64; 8]; num_tracked];
            let mut violations = 0u64;
            let mut tally = LoopTally::default();

            barrier.wait();
            for op in &stream {
                match *op {
                    Op::Read { key } => {
                        let res = store.read(&handle, key);
                        match res {
                            Some(v) => {
                                tally.sink = tally.sink.wrapping_add(v);
                                if let Some(&k_idx) = key_to_idx.get(&key).filter(|_| v > 0) {
                                    let writer_id = ((v >> 56) as usize).saturating_sub(1);
                                    let seq = v & 0x00FF_FFFF_FFFF_FFFF;
                                    if writer_id < 8 {
                                        let prev = &mut last_seen_seq[k_idx][writer_id];
                                        if seq < *prev {
                                            violations += 1;
                                        }
                                        *prev = (*prev).max(seq);
                                    }
                                }
                            }
                            None => tally.read_misses += 1,
                        }
                    }
                    Op::Update { key, val } => match store.update(&handle, key, val) {
                        Some(v) => tally.sink = tally.sink.wrapping_add(v),
                        None => tally.write_misses += 1,
                    },
                    Op::Insert { key, val } => match store.insert(&handle, key, val) {
                        None => tally.successful_inserts += 1,
                        Some(v) => {
                            tally.sink = tally.sink.wrapping_add(v);
                            tally.insert_collisions += 1;
                        }
                    },
                    Op::ReadModifyWrite { key } => match store.rmw(&handle, key) {
                        Some(v) => tally.sink = tally.sink.wrapping_add(v),
                        None => tally.write_misses += 1,
                    },
                }
            }
            (violations, tally, stream, handle)
        }));
    }

    barrier.wait();
    let returns: Vec<(u64, LoopTally, Vec<Op>, S::Handle)> =
        handles.into_iter().map(|h| h.join().unwrap()).collect();

    let mut total_violations = 0u64;
    let mut tally = LoopTally::default();
    for (v, t, _, _) in &returns {
        total_violations += v;
        tally.sink = tally.sink.wrapping_add(t.sink);
        tally.read_misses += t.read_misses;
        tally.write_misses += t.write_misses;
        tally.successful_inserts += t.successful_inserts;
        tally.insert_collisions += t.insert_collisions;
    }
    drop(returns);

    let oracle = verify_oracle(
        &**store,
        family,
        population,
        expected,
        tally.successful_inserts,
    );
    let mut verdict = oracle_verdict(family, &oracle, &tally, expected);
    if total_violations > 0 {
        if verdict == "PASS" {
            verdict = format!(
                "VOID_ORACLE: {} monotonicity violations observed (sequences went backwards)",
                total_violations
            );
        } else {
            verdict = format!(
                "{}; {} monotonicity violations observed",
                verdict, total_violations
            );
        }
    }

    MonotonicityOutcome {
        monotonicity_violations: total_violations,
        tracked_keys: num_tracked,
        oracle,
        verdict,
        node_census: store.node_census(),
        mem_used: store.mem_used(),
    }
}

pub fn run_cell_monotonicity(
    arm: Arm,
    family: Family,
    initial_keys: &[u64],
    sorted_keys: &[u64],
    streams: Vec<Vec<Op>>,
    expected: &Expected,
) -> MonotonicityOutcome {
    let key_map = if family.is_contiguous_ranks() {
        sorted_keys
    } else {
        initial_keys
    };
    let tracked_count = 4_096.min(key_map.len());
    let tracked_keys = &key_map[..tracked_count];

    macro_rules! run_with_store {
        ($store_ty:ident) => {{
            let store = Arc::new($store_ty::prefilled(sorted_keys));
            run_monotonicity_pass(&store, family, sorted_keys, tracked_keys, streams, expected)
        }};
    }

    match arm {
        Arm::Olc => run_with_store!(OlcStore),
        Arm::Mutex => run_with_store!(MutexStore),
        Arm::Skip => run_with_store!(SkipStore),
        Arm::Dash => run_with_store!(DashStore),
        Arm::RwBTree => run_with_store!(RwBTreeStore),
    }
}

/// Serializes ExpanseStats node census to compact JSON (§20.15).
pub fn census_json(s: &ExpanseStats) -> String {
    format!(
        "{{\"node_counts\":{{\"null\":{},\"immed\":{},\"leaf_linear\":{},\"leaf_bitmap\":{},\"branch_l3\":{},\"branch_l7\":{},\"branch_b\":{},\"branch_u\":{}}},\"node_bytes\":{{\"immed_values\":{},\"leaf_linear\":{},\"leaf_bitmap\":{},\"branch_l3\":{},\"branch_l7\":{},\"branch_b\":{},\"branch_u\":{},\"total\":{}}},\"depth_histogram\":[{}],\"branch_depth_histogram\":[{}],\"leaf_depth_histogram\":[{}]}}",
        s.node_counts.null,
        s.node_counts.immed,
        s.node_counts.leaf_linear,
        s.node_counts.leaf_bitmap,
        s.node_counts.branch_l3,
        s.node_counts.branch_l7,
        s.node_counts.branch_b,
        s.node_counts.branch_u,
        s.node_bytes.immed_values,
        s.node_bytes.leaf_linear,
        s.node_bytes.leaf_bitmap,
        s.node_bytes.branch_l3,
        s.node_bytes.branch_l7,
        s.node_bytes.branch_b,
        s.node_bytes.branch_u,
        s.node_bytes.total(),
        s.depth_histogram
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(","),
        s.branch_depth_histogram
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(","),
        s.leaf_depth_histogram
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(","),
    )
}
