//! Expanse-native multi-writer OLC scaling instrument (Refs #568, Phase 1.5D).
//!
//! Measures writer throughput of [`SyncExpanseMap`], [`SyncExpanseSet`], and
//! [`SyncExpanseStrMap`] as writer count scales across physical P-cores (W in {1, 2, 4, 8}, R = 0).
//!
//! Decoupled from `crates/expanse-hot-bench`: links no third-party competitor
//! trees, requiring zero C++ submodules. Uses the shared XorShift64 seeds and
//! population parameters (2^20 prefill, 2^20 fresh inserts) to ensure 1:1
//! comparability with historical baselines.
//!
//! ## Two builds, never one (AGENTS.md §6)
//!
//! Throughput comes from the default build (`--role throughput`, refuses to run if
//! `occ-stats` is enabled). Fallback counters come from the diagnostic build
//! (`--role counters`, requires `--features occ-stats` and refuses to run without it).
//! The roles cannot share a binary because counter instrumentation perturbs timing.
//!
//! Run (throughput — default build, no occ-stats):
//! ```text
//! cargo run --release -p expanse-trie --example writer_scaling -- [--role throughput] [--arm <map|set|str|all>] [--writers <1,2,4,8>] [--rounds <N>]
//! ```
//!
//! Run (counters — occ-stats build only):
//! ```text
//! cargo run --release -p expanse-trie --features occ-stats --example writer_scaling -- --role counters [--arm <map|set|str|all>] [--writers <1,2,4,8>] [--rounds <N>]
//! ```
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `concurrency_writer_scaling` |
//! | `group` | 5 |
//! | `emits` | `concurrency_writer_map_64bit`, `concurrency_writer_set_63bit`, `concurrency_writer_str` |
//! | `population` | prefill 2^20 keys (1M), plus 2^20 fresh keys inserted concurrently by W writers |
//! | `insertion_order` | sorted — prefill ascending, matching expanse-hot-bench; fresh stream in generator draw order |
//! | `probes_and_reuse` | none — pure writer scaling (R = 0), insert-only |
//! | `hit_rate` | n/a — no read probes |
//! | `miss_gen_method` | same-generator rejection sampling against prefill |
//! | `value_dereference` | map arms check stored values against key-derived expectation |
//! | `measured_region` | barrier release to last-writer join; prefill and teardown outside |
//! | `arm_symmetry` | symmetric across thread counts; W in {1, 2, 4, 8} on physical P-cores |
//! | `statistics` | throughput ops/sec emitted raw, paired bootstrap BCa 95% CI for C(N); lock fallbacks and their six causes (partition-checked per row) from the occ-stats counters pass |
//! | `verdict` | pending measurement |

use std::collections::HashSet;
use std::sync::Barrier;
use std::time::Instant;

use expanse_trie::occ_stats::{self, Stat};
use expanse_trie::strmap::NulFreeStr;
use expanse_trie::sync::{SyncExpanseMap, SyncExpanseSet, SyncExpanseStrMap};

/// The six fallback causes. They partition `Stat::LockFallbacks` exactly —
/// every fallback carries one cause (`tests/test_fallback_attribution.rs`) —
/// which is what lets a cell say *which* Phase 4 rung its fallbacks need
/// rather than only how many there were (#568).
const CAUSES: [(Stat, &str); 6] = [
    (Stat::FallbackCapExpansion, "cap_expansion"),
    (Stat::FallbackImmediateConversion, "immediate_conversion"),
    (Stat::FallbackBranchSplit, "branch_split"),
    (Stat::FallbackRootGrowth, "root_growth"),
    (Stat::FallbackContention, "contention"),
    (Stat::FallbackUnknownTag, "unknown_tag"),
];

/// One round's diagnostic counters. All zero on the throughput build, which
/// never reads them.
#[derive(Clone, Copy, Default)]
struct Counters {
    lock_fallbacks: u64,
    inserts: u64,
    causes: [u64; 6],
    lock_restarts: u64,
    contention_gate_closed: u64,
    contention_retry_exhausted: u64,
    gate_blocked_entries: u64,
    gate_wait_cycles: u64,
    quiesce_calls: u64,
    quiesce_drain_cycles: u64,
    branch_split_subarray: u64,
    branch_split_linear: u64,
    branch_split_prefix: u64,
    branch_split_remove: u64,
    branch_split_upgrade: u64,
    cap_expansion_class: u64,
    cap_expansion_leaf_full: u64,
    cap_expansion_bitmap_near_full: u64,
    cap_expansion_map_bitmap_sub: u64,
    cap_expansion_remove: u64,
    retired: u64,
    total_allocs: Option<u64>,
}

struct PerfControl {
    ctl_file: Option<std::fs::File>,
    ack_path: Option<std::path::PathBuf>,
}

impl PerfControl {
    fn new(ctl_arg: Option<&str>, ack_arg: Option<&str>) -> Self {
        let ctl_path = ctl_arg.map(std::path::PathBuf::from).or_else(|| {
            std::env::var("PERF_CTL_FIFO")
                .ok()
                .map(std::path::PathBuf::from)
        });
        let ack_path = ack_arg.map(std::path::PathBuf::from).or_else(|| {
            std::env::var("PERF_ACK_FIFO")
                .ok()
                .map(std::path::PathBuf::from)
        });

        let ctl_file = ctl_path.map(|p| {
            std::fs::OpenOptions::new()
                .write(true)
                .open(&p)
                .unwrap_or_else(|e| panic!("failed to open perf ctl fifo {}: {e}", p.display()))
        });

        Self { ctl_file, ack_path }
    }

    fn enable(&mut self) {
        if let Some(ref mut file) = self.ctl_file {
            use std::io::{Read, Write};
            file.write_all(b"enable\n")
                .expect("failed to write enable to perf ctl fifo");
            file.flush().expect("failed to flush perf ctl fifo");
            if let Some(ref ack_path) = self.ack_path {
                let mut ack_file = std::fs::File::open(ack_path).unwrap_or_else(|e| {
                    panic!("failed to open perf ack fifo {}: {e}", ack_path.display())
                });
                let mut buf = [0u8; 16];
                let _ = ack_file.read(&mut buf);
            }
        }
    }

    fn disable(&mut self) {
        if let Some(ref mut file) = self.ctl_file {
            use std::io::{Read, Write};
            file.write_all(b"disable\n")
                .expect("failed to write disable to perf ctl fifo");
            file.flush().expect("failed to flush perf ctl fifo");
            if let Some(ref ack_path) = self.ack_path {
                let mut ack_file = std::fs::File::open(ack_path).unwrap_or_else(|e| {
                    panic!("failed to open perf ack fifo {}: {e}", ack_path.display())
                });
                let mut buf = [0u8; 16];
                let _ = ack_file.read(&mut buf);
            }
        }
    }
}

impl Counters {
    fn read(is_counters: bool) -> Self {
        if !is_counters {
            return Self::default();
        }
        let snap = occ_stats::snapshot();
        let mut causes = [0u64; 6];
        for (slot, &(stat, _)) in causes.iter_mut().zip(CAUSES.iter()) {
            *slot = snap[stat as usize];
        }
        Self {
            lock_fallbacks: snap[Stat::LockFallbacks as usize],
            inserts: snap[Stat::Inserts as usize],
            causes,
            lock_restarts: snap[Stat::LockRestarts as usize],
            contention_gate_closed: snap[Stat::ContentionGateClosed as usize],
            contention_retry_exhausted: snap[Stat::ContentionRetryExhausted as usize],
            gate_blocked_entries: snap[Stat::GateBlockedEntries as usize],
            gate_wait_cycles: snap[Stat::GateWaitCycles as usize],
            quiesce_calls: snap[Stat::QuiesceCalls as usize],
            quiesce_drain_cycles: snap[Stat::QuiesceDrainCycles as usize],
            branch_split_subarray: snap[Stat::BranchSplitSubarray as usize],
            branch_split_linear: snap[Stat::BranchSplitLinear as usize],
            branch_split_prefix: snap[Stat::BranchSplitPrefix as usize],
            branch_split_remove: snap[Stat::BranchSplitRemove as usize],
            branch_split_upgrade: snap[Stat::BranchSplitUpgrade as usize],
            cap_expansion_class: snap[Stat::CapExpansionClass as usize],
            cap_expansion_leaf_full: snap[Stat::CapExpansionLeafFull as usize],
            cap_expansion_bitmap_near_full: snap[Stat::CapExpansionBitmapNearFull as usize],
            cap_expansion_map_bitmap_sub: snap[Stat::CapExpansionMapBitmapSub as usize],
            cap_expansion_remove: snap[Stat::CapExpansionRemove as usize],
            retired: snap[Stat::Retired as usize],
            total_allocs: None,
        }
    }

    fn causes_json(&self) -> String {
        let fields: Vec<String> = CAUSES
            .iter()
            .zip(self.causes)
            .map(|(&(_, name), v)| format!("\"{name}\":{v}"))
            .collect();
        format!("{{{}}}", fields.join(","))
    }

    fn extra_counters_json(&self) -> String {
        let total_allocs_str = match self.total_allocs {
            Some(v) => v.to_string(),
            None => "null".to_string(),
        };
        format!(
            "\"lock_restarts\":{restarts},\
             \"contention_gate_closed\":{c_closed},\"contention_retry_exhausted\":{c_exhausted},\
             \"gate_blocked_entries\":{g_blocked},\"gate_wait_cycles\":{g_wait},\
             \"quiesce_calls\":{q_calls},\"quiesce_drain_cycles\":{q_drain},\
             \"branch_split_subarray\":{bs_sub},\"branch_split_linear\":{bs_lin},\
             \"branch_split_prefix\":{bs_pfx},\"branch_split_remove\":{bs_rem},\
             \"branch_split_upgrade\":{bs_upg},\
             \"cap_expansion_class\":{ce_cls},\"cap_expansion_leaf_full\":{ce_full},\
             \"cap_expansion_bitmap_near_full\":{ce_bm},\"cap_expansion_map_bitmap_sub\":{ce_sub},\
             \"cap_expansion_remove\":{ce_rem},\
             \"retired\":{retired},\"total_allocs\":{total_allocs}",
            restarts = self.lock_restarts,
            c_closed = self.contention_gate_closed,
            c_exhausted = self.contention_retry_exhausted,
            g_blocked = self.gate_blocked_entries,
            g_wait = self.gate_wait_cycles,
            q_calls = self.quiesce_calls,
            q_drain = self.quiesce_drain_cycles,
            bs_sub = self.branch_split_subarray,
            bs_lin = self.branch_split_linear,
            bs_pfx = self.branch_split_prefix,
            bs_rem = self.branch_split_remove,
            bs_upg = self.branch_split_upgrade,
            ce_cls = self.cap_expansion_class,
            ce_full = self.cap_expansion_leaf_full,
            ce_bm = self.cap_expansion_bitmap_near_full,
            ce_sub = self.cap_expansion_map_bitmap_sub,
            ce_rem = self.cap_expansion_remove,
            retired = self.retired,
            total_allocs = total_allocs_str,
        )
    }

    /// The exact identities a counters row must satisfy: one `Inserts`
    /// bump per public insert, causes summing to fallbacks, exact contention
    /// partition, exact branch split partition, and quiesce calls equal to
    /// fallbacks in an insert-only workload.
    fn check(&self, expected_inserts: u64, cell: &str) -> Result<(), String> {
        if self.inserts != expected_inserts {
            return Err(format!(
                "{cell}: Stat::Inserts = {}, expected {expected_inserts} (one per public insert)",
                self.inserts
            ));
        }
        let summed: u64 = self.causes.iter().sum();
        if summed != self.lock_fallbacks {
            return Err(format!(
                "{cell}: fallback causes sum to {summed}, lock_fallbacks = {}; unattributed = {}",
                self.lock_fallbacks,
                self.lock_fallbacks as i128 - summed as i128
            ));
        }
        if self.quiesce_calls != self.lock_fallbacks {
            return Err(format!(
                "{cell}: quiesce_calls = {}, lock_fallbacks = {}",
                self.quiesce_calls, self.lock_fallbacks
            ));
        }
        let contention = self.causes[4]; // Stat::FallbackContention
        let contention_sub = self.contention_gate_closed + self.contention_retry_exhausted;
        if contention_sub != contention {
            return Err(format!(
                "{cell}: contention subsets sum to {contention_sub} (closed={}, exhausted={}), but contention = {}",
                self.contention_gate_closed, self.contention_retry_exhausted, contention
            ));
        }
        let branch_split = self.causes[2]; // Stat::FallbackBranchSplit
        let branch_sub = self.branch_split_subarray
            + self.branch_split_linear
            + self.branch_split_prefix
            + self.branch_split_remove
            + self.branch_split_upgrade;
        if branch_sub != branch_split {
            return Err(format!(
                "{cell}: branch split subsets sum to {branch_sub} (subarray={}, linear={}, prefix={}, remove={}, upgrade={}), but branch_split = {}",
                self.branch_split_subarray,
                self.branch_split_linear,
                self.branch_split_prefix,
                self.branch_split_remove,
                self.branch_split_upgrade,
                branch_split
            ));
        }
        let cap_expansion = self.causes[0]; // Stat::FallbackCapExpansion
        let cap_sub = self.cap_expansion_class
            + self.cap_expansion_leaf_full
            + self.cap_expansion_bitmap_near_full
            + self.cap_expansion_map_bitmap_sub
            + self.cap_expansion_remove;
        if cap_sub != cap_expansion {
            return Err(format!(
                "{cell}: cap expansion subsets sum to {cap_sub} (class={}, leaf_full={}, bitmap_near_full={}, map_bitmap_sub={}, remove={}), but cap_expansion = {}",
                self.cap_expansion_class,
                self.cap_expansion_leaf_full,
                self.cap_expansion_bitmap_near_full,
                self.cap_expansion_map_bitmap_sub,
                self.cap_expansion_remove,
                cap_expansion
            ));
        }
        Ok(())
    }
}

/// Prefill population (2^20 keys = 1,048,576).
pub const N_PREFILL: usize = 1 << 20;
/// Fresh keys the writers insert (2^20 keys = 1,048,576).
pub const M_FRESH: usize = 1 << 20;

/// The shared suite seed (`expanse_hot_bench::workload::XorShift::SEED`).
pub const SEED_PREFILL: u64 = 0x0DDB_1A5E_5EED_0001;
/// The continuation seed for fresh integer keys, derived identically to `hot_concurrent`.
pub const SEED_FRESH: u64 = SEED_PREFILL ^ 0x5EED_C0DE_0000_0692;
/// The continuation seed for fresh string keys, derived identically to `masstree_concurrent`.
pub const SEED_STR_FRESH: u64 = SEED_PREFILL ^ 0x5EED_C0DE_0000_0661;

/// The 62 ASCII alphanumerics — NUL-free by construction.
const ALNUM: &[u8; 62] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// Deterministic XorShift64 PRNG matching repo standard.
#[derive(Clone)]
struct XorShift(u64);

impl XorShift {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    #[allow(clippy::should_implement_trait)]
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// The stored value for a key on the map arm, matching `hot_concurrent`.
#[inline]
fn value_of(k: u64) -> u64 {
    k.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// Deterministic key-derived value for string map arm.
#[inline]
fn str_value_of(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

fn fill_alnum(rng: &mut XorShift, out: &mut Vec<u8>, n: usize) {
    for _ in 0..n {
        let idx = (rng.next() % 62) as usize;
        out.push(ALNUM[idx]);
    }
}

/// Workload containing prefill keys and fresh disjoint keys for integer writers.
struct WriterWorkload {
    prefill: Vec<u64>,
    fresh_keys: Vec<u64>,
    keyspace_bits: u32,
}

impl WriterWorkload {
    fn generate(n_prefill: usize, m_fresh: usize, keyspace_bits: u32) -> Self {
        let mask = if keyspace_bits >= 64 {
            u64::MAX
        } else {
            (1u64 << keyspace_bits) - 1
        };

        // 1. Generate prefill keys.
        let mut rng = XorShift::new(SEED_PREFILL);
        let mut prefill = Vec::with_capacity(n_prefill);
        for _ in 0..n_prefill {
            prefill.push(rng.next() & mask);
        }
        prefill.sort_unstable();
        prefill.dedup();
        // If duplicates occurred, top up to exactly n_prefill.
        while prefill.len() < n_prefill {
            let k = rng.next() & mask;
            if let Err(pos) = prefill.binary_search(&k) {
                prefill.insert(pos, k);
            }
        }

        // 2. Generate fresh keys rejection-sampled against prefill.
        let mut fresh_rng = XorShift::new(SEED_FRESH);
        let mut fresh_keys = Vec::with_capacity(m_fresh);
        while fresh_keys.len() < m_fresh {
            let c = fresh_rng.next() & mask;
            if prefill.binary_search(&c).is_err() {
                fresh_keys.push(c);
            }
        }

        Self {
            prefill,
            fresh_keys,
            keyspace_bits,
        }
    }
}

/// Workload containing prefill keys and fresh disjoint keys for string writers.
struct WriterStrWorkload {
    prefill: Vec<Vec<u8>>,
    fresh_keys: Vec<Vec<u8>>,
}

impl WriterStrWorkload {
    fn generate(n_prefill: usize, m_fresh: usize) -> Self {
        let mut rng = XorShift::new(SEED_PREFILL);
        let mut prefill = Vec::with_capacity(n_prefill);
        let mut buf = Vec::with_capacity(16);
        for _ in 0..n_prefill {
            buf.clear();
            let n = 8 + (rng.next() % 9) as usize;
            fill_alnum(&mut rng, &mut buf, n);
            prefill.push(buf.clone());
        }
        prefill.sort_unstable();
        prefill.dedup();
        while prefill.len() < n_prefill {
            buf.clear();
            let n = 8 + (rng.next() % 9) as usize;
            fill_alnum(&mut rng, &mut buf, n);
            if let Err(pos) = prefill.binary_search(&buf) {
                prefill.insert(pos, buf.clone());
            }
        }

        let mut fresh_rng = XorShift::new(SEED_STR_FRESH);
        let mut fresh_keys = Vec::with_capacity(m_fresh);
        let mut seen = HashSet::with_capacity(m_fresh);
        while fresh_keys.len() < m_fresh {
            buf.clear();
            let n = 8 + (fresh_rng.next() % 9) as usize;
            fill_alnum(&mut fresh_rng, &mut buf, n);
            if prefill.binary_search(&buf).is_err() && seen.insert(buf.clone()) {
                fresh_keys.push(buf.clone());
            }
        }

        Self {
            prefill,
            fresh_keys,
        }
    }
}

fn run_map_cell(
    workload: &WriterWorkload,
    writers: usize,
    round: usize,
    is_counters: bool,
    perf_ctl: &mut PerfControl,
) -> (f64, u64, Counters) {
    let map = SyncExpanseMap::new();
    for &k in &workload.prefill {
        map.insert(k, value_of(k));
    }

    let per = workload.fresh_keys.len() / writers.max(1);
    let barrier = Barrier::new(writers + 1);

    let allocs_before = if is_counters {
        map.with_locked(|inner| inner.total_node_allocs() as u64)
    } else {
        0
    };

    if is_counters {
        occ_stats::reset();
    }

    let start = std::thread::scope(|s| {
        for w in 0..writers {
            let lo = w * per;
            let hi = if w + 1 == writers {
                workload.fresh_keys.len()
            } else {
                lo + per
            };
            let slice = &workload.fresh_keys[lo..hi];
            let b = &barrier;
            let m = &map;
            s.spawn(move || {
                b.wait();
                for &k in slice {
                    m.insert(k, value_of(k));
                }
            });
        }

        perf_ctl.enable();
        barrier.wait();
        Instant::now()
    });
    perf_ctl.disable();

    let elapsed = if is_counters {
        0.0
    } else {
        start.elapsed().as_secs_f64()
    };
    let mut counters = Counters::read(is_counters);
    if is_counters {
        counters.total_allocs = Some(
            map.with_locked(|inner| inner.total_node_allocs() as u64)
                .saturating_sub(allocs_before),
        );
    }
    let final_pop = map.len();

    // Verify samples
    let reader = map.reader();
    for &k in workload.fresh_keys.iter().step_by(10_000) {
        let val = reader.get(k);
        assert_eq!(
            val,
            Some(value_of(k)),
            "map missing fresh key {k} at round {round}"
        );
    }

    (elapsed, final_pop, counters)
}

fn run_set_cell(
    workload: &WriterWorkload,
    writers: usize,
    round: usize,
    is_counters: bool,
    perf_ctl: &mut PerfControl,
) -> (f64, u64, Counters) {
    let set = SyncExpanseSet::new();
    for &k in &workload.prefill {
        set.insert(k);
    }

    let per = workload.fresh_keys.len() / writers.max(1);
    let barrier = Barrier::new(writers + 1);

    let allocs_before = if is_counters {
        set.with_locked(|inner| inner.total_node_allocs() as u64)
    } else {
        0
    };

    if is_counters {
        occ_stats::reset();
    }

    let start = std::thread::scope(|s| {
        for w in 0..writers {
            let lo = w * per;
            let hi = if w + 1 == writers {
                workload.fresh_keys.len()
            } else {
                lo + per
            };
            let slice = &workload.fresh_keys[lo..hi];
            let b = &barrier;
            let s_ref = &set;
            s.spawn(move || {
                b.wait();
                for &k in slice {
                    s_ref.insert(k);
                }
            });
        }

        perf_ctl.enable();
        barrier.wait();
        Instant::now()
    });
    perf_ctl.disable();

    let elapsed = if is_counters {
        0.0
    } else {
        start.elapsed().as_secs_f64()
    };
    let mut counters = Counters::read(is_counters);
    if is_counters {
        counters.total_allocs = Some(
            set.with_locked(|inner| inner.total_node_allocs() as u64)
                .saturating_sub(allocs_before),
        );
    }
    let final_pop = set.len();

    // Verify samples
    let reader = set.reader();
    for &k in workload.fresh_keys.iter().step_by(10_000) {
        assert!(
            reader.contains(k),
            "set missing fresh key {k} at round {round}"
        );
    }

    (elapsed, final_pop, counters)
}

fn run_str_cell(
    workload: &WriterStrWorkload,
    writers: usize,
    round: usize,
    is_counters: bool,
    perf_ctl: &mut PerfControl,
) -> (f64, u64, Counters) {
    let map = SyncExpanseStrMap::new();
    for k in &workload.prefill {
        let nk = NulFreeStr::new(k).expect("alnum bytes are NUL-free");
        map.insert(nk, str_value_of(k));
    }

    let per = workload.fresh_keys.len() / writers.max(1);
    let barrier = Barrier::new(writers + 1);

    if is_counters {
        occ_stats::reset();
    }

    let start = std::thread::scope(|s| {
        for w in 0..writers {
            let lo = w * per;
            let hi = if w + 1 == writers {
                workload.fresh_keys.len()
            } else {
                lo + per
            };
            let slice = &workload.fresh_keys[lo..hi];
            let b = &barrier;
            let m = &map;
            s.spawn(move || {
                b.wait();
                for k in slice {
                    let nk = NulFreeStr::new(k).expect("alnum bytes are NUL-free");
                    m.insert(nk, str_value_of(k));
                }
            });
        }

        perf_ctl.enable();
        barrier.wait();
        Instant::now()
    });
    perf_ctl.disable();

    let elapsed = if is_counters {
        0.0
    } else {
        start.elapsed().as_secs_f64()
    };
    let counters = Counters::read(is_counters);
    let final_pop = map.len();

    // Verify samples
    let reader = map.reader();
    for k in workload.fresh_keys.iter().step_by(10_000) {
        let nk = NulFreeStr::new(k).expect("alnum bytes are NUL-free");
        let val = reader.get(nk);
        assert_eq!(
            val,
            Some(str_value_of(k)),
            "str missing fresh key at round {round}"
        );
    }

    (elapsed, final_pop, counters)
}

fn self_test(role_opt: Option<&str>) -> Result<(), String> {
    eprintln!("running writer_scaling self-test...");
    let n0 = 1024;
    let m = 1024;

    let is_counters = match role_opt {
        Some("counters") => {
            if !occ_stats::enabled() {
                return Err(
                    "build/role mismatch: occ-stats is OFF but role is 'counters' (AGENTS.md §6 / two builds, never one)".into()
                );
            }
            true
        }
        Some("throughput") => {
            if occ_stats::enabled() {
                return Err(
                    "build/role mismatch: occ-stats is ON but role is 'throughput' (AGENTS.md §6 / two builds, never one)".into()
                );
            }
            false
        }
        _ => occ_stats::enabled(),
    };

    // Every cause the engine defines must be read here, or a fallback it
    // attributes to that cause would go unreported and the partition check
    // below could only catch it at a scale where that cause happens to fire.
    // Checked against the engine's own counter names, so a cause added later
    // fails this test at any population.
    for (idx, name) in occ_stats::NAMES.iter().enumerate() {
        if let Some(cause) = name.strip_prefix("fallback_") {
            let covered = CAUSES
                .iter()
                .any(|&(stat, label)| stat as usize == idx && label == cause);
            if !covered {
                return Err(format!(
                    "occ_stats defines fallback cause `{name}` (index {idx}) that CAUSES does not read"
                ));
            }
        }
    }
    let engine_causes = occ_stats::NAMES
        .iter()
        .filter(|n| n.starts_with("fallback_"))
        .count();
    if engine_causes != CAUSES.len() {
        return Err(format!(
            "CAUSES reads {} causes, occ_stats defines {engine_causes}",
            CAUSES.len()
        ));
    }

    let mut dummy_ctl = PerfControl::new(None, None);
    let wl_map = WriterWorkload::generate(n0, m, 64);
    assert_eq!(wl_map.prefill.len(), n0);
    assert_eq!(wl_map.fresh_keys.len(), m);

    let (el_map, pop_map, fb_map) = run_map_cell(&wl_map, 2, 0, is_counters, &mut dummy_ctl);
    if pop_map != (n0 + m) as u64 {
        return Err(format!("map expected pop {}, got {pop_map}", n0 + m));
    }
    if is_counters {
        if fb_map.total_allocs.unwrap_or(0) == 0 {
            return Err("counters test: expected fb_map.total_allocs > 0".into());
        }
        fb_map.check(m as u64, "self-test map")?;
    } else if el_map <= 0.0 {
        return Err(format!("throughput test: invalid map elapsed {el_map}"));
    }

    let wl_set = WriterWorkload::generate(n0, m, 63);
    let (el_set, pop_set, fb_set) = run_set_cell(&wl_set, 2, 0, is_counters, &mut dummy_ctl);
    if pop_set != (n0 + m) as u64 {
        return Err(format!("set expected pop {}, got {pop_set}", n0 + m));
    }
    if is_counters {
        if fb_set.total_allocs.unwrap_or(0) == 0 {
            return Err("counters test: expected fb_set.total_allocs > 0".into());
        }
        fb_set.check(m as u64, "self-test set")?;
    } else if el_set <= 0.0 {
        return Err(format!("throughput test: invalid set elapsed {el_set}"));
    }

    // No assertion here that any cell produced a fallback. There was one, on
    // both integer arms, and it was wrong in two ways.
    //
    // At this scale the 64- and 63-bit keyspaces are so sparse that almost
    // every key lands in its own expanse: no leaf fills, so the only fallbacks
    // those cells can produce come from two writers colliding. That count is a
    // property of the interleaving, not of the engine — it failed four runs in
    // five on one developer host and failed CI outright.
    //
    // Densifying the workload does not rescue it either, and that is the more
    // important half: every phase of the multi-writer work (Refs #568) moves
    // another structural transition onto the optimistic path, so the fallback
    // count legitimately falls toward zero. With concurrent capacity growth
    // and immediate conversion merged, one writer over a 12-bit keyspace
    // produces no fallbacks at all. A test that requires fallbacks to exist is
    // a test that fails when the engine improves.
    //
    // What the counters build actually owes is checked instead, by `check()`
    // on every cell above: the six causes sum to `lock_fallbacks`, and
    // `write_ops` equals the inserts the cell performed. Those hold at any
    // count, zero included (AGENTS.md section 8.4: hard assertions belong on
    // deterministic invariants).

    let wl_str = WriterStrWorkload::generate(n0, m);
    let (el_str, pop_str, fb_str) = run_str_cell(&wl_str, 2, 0, is_counters, &mut dummy_ctl);
    if pop_str != (n0 + m) as u64 {
        return Err(format!("str expected pop {}, got {pop_str}", n0 + m));
    }
    if is_counters {
        // str arm is the alpha=1 coarse-mutex reference curve: 0 lock fallbacks by construction
        if fb_str.lock_fallbacks != 0 {
            return Err(format!(
                "counters test: expected fb_str == 0, got {}",
                fb_str.lock_fallbacks
            ));
        }
        if fb_str.total_allocs.is_some() {
            return Err("counters test: expected fb_str.total_allocs to be None (null)".into());
        }
        fb_str.check(m as u64, "self-test str")?;
    } else if el_str <= 0.0 {
        return Err(format!("throughput test: invalid str elapsed {el_str}"));
    }

    let mode_str = if is_counters {
        "counters"
    } else {
        "throughput"
    };

    // Verify Williams square design balance properties directly:
    let test_writers = [1, 2, 4, 8];
    let n = test_writers.len();
    let mut pos_counts = vec![vec![0usize; n]; n];
    let mut pair_counts = vec![vec![0usize; n]; n];
    for r in 0..n {
        let order = williams_order(&test_writers, r);
        if order.len() != n {
            return Err(format!(
                "williams_order returned len {}, expected {n}",
                order.len()
            ));
        }
        for (pos, &w) in order.iter().enumerate() {
            let widx = test_writers.iter().position(|&x| x == w).unwrap();
            pos_counts[widx][pos] += 1;
            if pos > 0 {
                let prev_w = order[pos - 1];
                let prev_idx = test_writers.iter().position(|&x| x == prev_w).unwrap();
                pair_counts[prev_idx][widx] += 1;
            }
        }
    }
    for (widx, &w) in test_writers.iter().enumerate() {
        for (pos, &cnt) in pos_counts[widx].iter().enumerate() {
            if cnt != 1 {
                return Err(format!(
                    "Williams square imbalance: W={w} appeared in pos {pos} {cnt} times (expected 1)"
                ));
            }
        }
    }
    for i in 0..n {
        for j in 0..n {
            if i != j && pair_counts[i][j] != 1 {
                return Err(format!(
                    "Williams square carryover imbalance: pair ({}, {}) appeared {} times (expected 1)",
                    test_writers[i], test_writers[j], pair_counts[i][j]
                ));
            }
        }
    }

    eprintln!("writer_scaling {mode_str} self-test PASSED");
    Ok(())
}

fn williams_order(writers: &[usize], round: usize) -> Vec<usize> {
    let n = writers.len();
    if n <= 1 {
        return writers.to_vec();
    }
    // First row 0, 1, n-1, 2, n-2, ...; row r adds r mod n.
    let mut first = Vec::with_capacity(n);
    first.push(0);
    let (mut lo, mut hi) = (1, n - 1);
    while first.len() < n {
        first.push(lo);
        lo += 1;
        if first.len() < n {
            first.push(hi);
            hi -= 1;
        }
    }
    first.iter().map(|&i| writers[(i + round) % n]).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let role_opt = args
        .iter()
        .position(|a| a == "--role")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str());

    if args.iter().any(|a| a == "--self-test") {
        if let Err(e) = self_test(role_opt) {
            eprintln!("self-test failed: {e}");
            std::process::exit(1);
        }
        return;
    }

    let role_arg = role_opt.unwrap_or("throughput");
    let is_counters = match role_arg {
        "counters" => {
            if !occ_stats::enabled() {
                eprintln!(
                    "build/role mismatch: occ-stats is OFF but role is 'counters' — \
                     counter extraction requires --features occ-stats (AGENTS.md §6 / two builds, never one)"
                );
                std::process::exit(1);
            }
            true
        }
        "throughput" => {
            if occ_stats::enabled() {
                eprintln!(
                    "build/role mismatch: occ-stats is ON but role is 'throughput' — \
                     throughput must come from the default build only (AGENTS.md §6 / two builds, never one)"
                );
                std::process::exit(1);
            }
            false
        }
        other => {
            eprintln!("unknown role: {other} (expected 'throughput' or 'counters')");
            std::process::exit(1);
        }
    };

    let arm_arg = args
        .iter()
        .position(|a| a == "--arm")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("all");

    let writers_arg = args
        .iter()
        .position(|a| a == "--writers")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("1,2,4,8");

    let rounds: usize = args
        .iter()
        .position(|a| a == "--rounds")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);

    let round_opt: Option<usize> = args
        .iter()
        .position(|a| a == "--round")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok());

    let (round_start, round_end) = if let Some(r) = round_opt {
        (r, r + 1)
    } else {
        (0, rounds)
    };

    let is_quick = args.iter().any(|a| a == "--quick");
    let (n0, m) = if is_quick {
        (4096, 4096)
    } else {
        (N_PREFILL, M_FRESH)
    };

    let writers_list: Vec<usize> = writers_arg
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let n_w = writers_list.len();
    if !n_w.is_multiple_of(2) || !rounds.is_multiple_of(n_w) {
        eprintln!(
            "notice: Williams square balance requires even writer count and rounds multiple of len(writers); \
             got len(writers)={n_w}, rounds={rounds} — position/carryover balance will be incomplete"
        );
    }

    let run_map = arm_arg == "map" || arm_arg == "all" || arm_arg == "both";
    let run_set = arm_arg == "set" || arm_arg == "all" || arm_arg == "both";
    let run_str = arm_arg == "str" || arm_arg == "all";

    let tsc_hz = occ_stats::cycles_hz(std::time::Duration::from_millis(200));

    let perf_ctl_fifo = args
        .iter()
        .position(|a| a == "--perf-ctl-fifo")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str());
    let perf_ack_fifo = args
        .iter()
        .position(|a| a == "--perf-ack-fifo")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str());
    let mut perf_ctl = PerfControl::new(perf_ctl_fifo, perf_ack_fifo);

    // Interleaved execution across writer counts within each round, balancing
    // position and first-order carryover across rounds (Williams design):
    if run_map {
        eprintln!("generating map 64-bit workload (prefill={n0}, fresh={m})...");
        let wl = WriterWorkload::generate(n0, m, 64);
        let bits = wl.keyspace_bits;
        for round in round_start..round_end {
            let round_writers = williams_order(&writers_list, round);
            for (pos, &w) in round_writers.iter().enumerate() {
                let (elapsed_s, final_pop, counters) =
                    run_map_cell(&wl, w, round, is_counters, &mut perf_ctl);
                let write_ops = m;
                if is_counters {
                    if let Err(e) = counters.check(m as u64, &format!("map_w{w}_r0 round {round}"))
                    {
                        eprintln!("counter identity violated: {e}");
                        std::process::exit(1);
                    }
                    println!(
                        "{{\"workload_id\":\"concurrency_writer_map_64bit\",\"role\":\"counters\",\
                         \"arm\":\"expanse\",\"cell\":\"map_w{w}_r0\",\"keyspace_bits\":{bits},\
                         \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{w},\"readers\":0,\
                         \"round\":{round},\"position\":{pos},\"write_ops\":{write_ops},\"tsc_hz\":{tsc_hz},\
                         \"lock_fallbacks\":{fb},\"inserts\":{ins},{extra},\"fallback_causes\":{causes},\"population_after\":{final_pop}}}",
                        fb = counters.lock_fallbacks,
                        ins = counters.inserts,
                        extra = counters.extra_counters_json(),
                        causes = counters.causes_json(),
                    );
                } else {
                    let writer_mops = (write_ops as f64) / elapsed_s / 1e6;
                    println!(
                        "{{\"workload_id\":\"concurrency_writer_map_64bit\",\"role\":\"throughput\",\
                         \"arm\":\"expanse\",\"cell\":\"map_w{w}_r0\",\"keyspace_bits\":{bits},\
                         \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{w},\"readers\":0,\
                         \"round\":{round},\"position\":{pos},\"write_ops\":{write_ops},\"writer_elapsed_s\":{elapsed_s:.6},\
                         \"writer_mops\":{writer_mops:.4},\"tsc_hz\":{tsc_hz},\"population_after\":{final_pop}}}"
                    );
                }
            }
        }
    }

    if run_set {
        eprintln!("generating set 63-bit workload (prefill={n0}, fresh={m})...");
        let wl = WriterWorkload::generate(n0, m, 63);
        let bits = wl.keyspace_bits;
        for round in round_start..round_end {
            let round_writers = williams_order(&writers_list, round);
            for (pos, &w) in round_writers.iter().enumerate() {
                let (elapsed_s, final_pop, counters) =
                    run_set_cell(&wl, w, round, is_counters, &mut perf_ctl);
                let write_ops = m;
                if is_counters {
                    if let Err(e) = counters.check(m as u64, &format!("set_w{w}_r0 round {round}"))
                    {
                        eprintln!("counter identity violated: {e}");
                        std::process::exit(1);
                    }
                    println!(
                        "{{\"workload_id\":\"concurrency_writer_set_63bit\",\"role\":\"counters\",\
                         \"arm\":\"expanse\",\"cell\":\"set_w{w}_r0\",\"keyspace_bits\":{bits},\
                         \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{w},\"readers\":0,\
                         \"round\":{round},\"position\":{pos},\"write_ops\":{write_ops},\"tsc_hz\":{tsc_hz},\
                         \"lock_fallbacks\":{fb},\"inserts\":{ins},{extra},\"fallback_causes\":{causes},\"population_after\":{final_pop}}}",
                        fb = counters.lock_fallbacks,
                        ins = counters.inserts,
                        extra = counters.extra_counters_json(),
                        causes = counters.causes_json(),
                    );
                } else {
                    let writer_mops = (write_ops as f64) / elapsed_s / 1e6;
                    println!(
                        "{{\"workload_id\":\"concurrency_writer_set_63bit\",\"role\":\"throughput\",\
                         \"arm\":\"expanse\",\"cell\":\"set_w{w}_r0\",\"keyspace_bits\":{bits},\
                         \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{w},\"readers\":0,\
                         \"round\":{round},\"position\":{pos},\"write_ops\":{write_ops},\"writer_elapsed_s\":{elapsed_s:.6},\
                         \"writer_mops\":{writer_mops:.4},\"tsc_hz\":{tsc_hz},\"population_after\":{final_pop}}}"
                    );
                }
            }
        }
    }

    if run_str {
        eprintln!("generating str workload (prefill={n0}, fresh={m})...");
        let wl = WriterStrWorkload::generate(n0, m);
        for round in round_start..round_end {
            let round_writers = williams_order(&writers_list, round);
            for (pos, &w) in round_writers.iter().enumerate() {
                let (elapsed_s, final_pop, counters) =
                    run_str_cell(&wl, w, round, is_counters, &mut perf_ctl);
                let write_ops = m;
                if is_counters {
                    if let Err(e) = counters.check(m as u64, &format!("str_w{w}_r0 round {round}"))
                    {
                        eprintln!("counter identity violated: {e}");
                        std::process::exit(1);
                    }
                    println!(
                        "{{\"workload_id\":\"concurrency_writer_str\",\"role\":\"counters\",\
                         \"arm\":\"expanse\",\"cell\":\"str_w{w}_r0\",\"dist\":\"short\",\
                         \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{w},\"readers\":0,\
                         \"round\":{round},\"position\":{pos},\"write_ops\":{write_ops},\"tsc_hz\":{tsc_hz},\
                         \"lock_fallbacks\":{fb},\"inserts\":{ins},{extra},\"fallback_causes\":{causes},\"population_after\":{final_pop}}}",
                        fb = counters.lock_fallbacks,
                        ins = counters.inserts,
                        extra = counters.extra_counters_json(),
                        causes = counters.causes_json(),
                    );
                } else {
                    let writer_mops = (write_ops as f64) / elapsed_s / 1e6;
                    println!(
                        "{{\"workload_id\":\"concurrency_writer_str\",\"role\":\"throughput\",\
                         \"arm\":\"expanse\",\"cell\":\"str_w{w}_r0\",\"dist\":\"short\",\
                         \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{w},\"readers\":0,\
                         \"round\":{round},\"position\":{pos},\"write_ops\":{write_ops},\"writer_elapsed_s\":{elapsed_s:.6},\
                         \"writer_mops\":{writer_mops:.4},\"tsc_hz\":{tsc_hz},\"population_after\":{final_pop}}}"
                    );
                }
            }
        }
    }
}
