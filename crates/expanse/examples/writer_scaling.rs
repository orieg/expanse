//! Expanse-native multi-writer OLC scaling instrument (Refs #568, Phase 1.5D).
//!
//! Measures writer throughput of [`SyncExpanseMap`], [`SyncExpanseSet`],
//! [`SyncExpanseStrMap`], [`SyncExpanseBytesMap`] and [`SyncExpanseBlobMap`] as writer
//! count scales across physical P-cores (W in {1, 2, 4, 8}, R = 0). The last three
//! serialise every insert on the writer mutex (#929), so they are the coarse-mutex
//! reference curves beside the two optimistic-lock-coupling arms.
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
//! cargo run --release -p expanse-trie --example writer_scaling -- [--role throughput] [--arm <map|set|str|bytes|blob|all>] [--writers <1,2,4,8>] [--rounds <N>] [--blob-op <insert|overwrite>] [--key-dist <uniform|zipfian>]
//! ```
//!
//! Run (counters — occ-stats build only):
//! ```text
//! cargo run --release -p expanse-trie --features occ-stats --example writer_scaling -- --role counters [--arm <map|set|str|bytes|blob|all>] [--writers <1,2,4,8>] [--rounds <N>]
//! ```
//!
//! ## One timed cell per process (`docs/benchmarks/concurrency/METHODOLOGY.md` §15)
//!
//! A multi-W invocation runs every W of a round in one process, in Williams
//! order. Each cell builds and drops its own tree, but process-wide state —
//! the allocator's per-thread arenas among it — carries from one cell into the
//! next, so a cell's throughput depends on which cells ran before it in the
//! same process. The driver (`writer_scaling.py`) therefore runs every timed
//! writer cell in a process of its own:
//!
//! ```text
//! cargo run --release -p expanse-trie --example writer_scaling -- --arm map --writers <W> --round <r> --position <p> [--quick]
//! ```
//!
//! - `--position` records the cell's scheduled place in its round, which the
//!   driver owns. It needs `--round` and a single `--writers` count, and is
//!   refused otherwise.
//! - The multi-W form stays for manual use and for the counters pass, which
//!   times nothing; there `position` is the index in the harness's own
//!   Williams row.
//!
//! ## Reader mode (#900, `docs/benchmarks/concurrency/METHODOLOGY.md` §12.4)
//!
//! `--readers R` with R > 0 runs ordered-read cells on the map arm only, one
//! (W, R, `--read-op`, `--probe`) cell per invocation. The driver
//! (`writer_scaling.py --ordered-readers`) interleaves the cells within each
//! round and passes `--round` and `--position` so every row names its place.
//! Without `--readers` (or with `--readers 0`) the output is the writer sweep
//! above, row for row.
//!
//! ```text
//! cargo run --release -p expanse-trie --example writer_scaling -- --arm map --writers <W> --readers <R> --read-op <get|prev_locked|prev> --probe <uniform|hotspot> [--round <r>] [--position <p>] [--quick]
//! ```
//!
//! - `--writers 0` is accepted here and nowhere else.
//! - `prev` is `prev_before` on the reader's own [`SyncExpanseMap::reader`]
//!   handle (optimistic); `prev_locked` is the same call through
//!   [`SyncExpanseMap::with_locked`], which excludes every writer; `get` is the
//!   optimistic point read on the same handle.
//! - With W ≥ 1 readers probe from the barrier until main has joined every
//!   writer and raised a stop flag. With W = 0 each reader makes exactly as many
//!   probes as the prefill holds: 2^20, or 4,096 under `--quick`.
//! - `reader_elapsed_s` is measured by main, from the barrier release to the
//!   join of the last reader. Prefill, workload generation, reader registration,
//!   the answer check and teardown are outside it.
//!
//! **Probes.** Each reader walks its own Fisher–Yates permutation of the probe
//! keys (the suite XorShift, seed `SEED_READER ^ (reader + 1) * φ64`), cycled.
//! `prev` and `prev_locked` therefore see the identical stream.
//!
//! - `uniform`: the 2^20 present uniform prefill keys.
//! - `hotspot`: 255 keys in the 2^16-wide expanse `[H, H + 2^16)`, where `H` is
//!   the first draw of `SEED_HOTSPOT_BASE` with its low 16 bits cleared. Each
//!   terminal byte `j` of the expanse (block `j`, 256 keys wide) holds one
//!   prefill key, at offset 1: `H + 256·j + 1`. The probes are those keys for
//!   `j ≥ 1`.
//!   - **Why every probe backtracks.** `MapReader::prev_before(k)` searches
//!     `k − 1 = H + 256·j`, offset 0 of block `j`. At a branch, `nav::prev`
//!     (`crates/expanse/src/nav.rs`) first descends the child for the target's
//!     digit and moves to a lower sibling only when that child returns nothing.
//!     Block `j`'s only prefill key sits above the target, so its child returns
//!     nothing, and the search descends the sibling for block `j − 1`, which
//!     answers at its offset 1. Block 0 has no lower sibling inside the
//!     expanse, so its key is prefilled and never probed.
//!   - **Why the branch exists.** 256 prefill keys exceed `LEAF_CAP` (32,
//!     `crates/expanse/src/types.rs:99`), the linear-leaf cap at levels 2–7
//!     that map inserts apply to every multi-byte leaf
//!     (`crates/expanse/src/mutate_map.rs:791`, `:1438`), so no two-byte leaf
//!     can hold the expanse and a level-2 branch indexes its blocks. A
//!     compile-time assertion below pins the relation.
//!   - **Writes.** The hotspot writers insert the expanse's other 65,280 keys
//!     (offsets 0 and 2–255 of every block), shuffled with `SEED_HOTSPOT_FRESH`
//!     and split contiguously across W. Offsets 2–255 land in block `j`, where
//!     a search fails, and in block `j − 1`, the sibling it descends into.
//!   - **What erodes it.** An offset-0 insert gives its block a key at the
//!     search target, so that block's probe answers without a sibling descent.
//!     255 of the 65,280 fresh keys do this (block 0's offset 0 is never
//!     searched). Every probe backtracks when a W ≥ 1 window opens, and the
//!     share falls only as those keys arrive. At W = 0 every probe backtracks.
//!   - **Why not offset 0.** Prefilling offset 0 (or every even key) forces no
//!     sibling descent at all: the search for `k − 1` enters the block below
//!     directly and answers at a key that block holds. That is 0 of 256 probes
//!     at offset 0, and 0 of 128 per block for even keys.
//!
//! The 256 hotspot keys are prefilled in **every** reader-mode cell, uniform
//! ones included, so the tree is the same across probe modes. A uniform cell's
//! writers insert the 2^20 uniform fresh keys. Generation refuses to run if any
//! uniform prefill or fresh key falls inside the hotspot expanse.
//!
//! ## Readers-only mode on the set and string arms (#730)
//!
//! `--arm set` and `--arm str` with `--readers R` run the readers-only cell:
//! `--writers 0`, `--read-op get` (`contains` on the set), `--probe uniform`,
//! and nothing else is accepted. The prefill is the arm's writer-sweep
//! prefill (2^20 keys over 63 bits for `set`, 2^20 `short` alphanumeric keys
//! of 8–16 bytes for `str`), and each reader walks its own Fisher–Yates
//! permutation of it once, so every probe is a present key. The map arm's
//! readers-only cell is reader mode's W = 0 `get` cell. Every reader-mode
//! throughput row also carries `reader_thread_elapsed_s`, each reader's own
//! loop time, beside `reader_elapsed_s`, which runs to the last join.
//! `writer_scaling.py --readers-only` drives the three arms at R ∈ {1, 2, 4, 8}.
//!
//! ```text
//! cargo run --release -p expanse-trie --example writer_scaling -- --arm <map|set|str> --writers 0 --readers <R> --read-op get --probe uniform [--round <r>] [--position <p>] [--quick]
//! ```
//!
//! ## The blob overwrite cell (#929, `docs/benchmarks/concurrency/METHODOLOGY.md` §21.4 G4, §21.11)
//!
//! `--arm blob --blob-op overwrite` replaces the blob arm's fresh inserts with
//! overwrites of existing entries; `--key-dist <uniform|zipfian>` (default
//! `uniform`) chooses which. Both flags need `--arm blob` and writer mode, and
//! `--key-dist` needs `--blob-op overwrite`; anything else exits non-zero.
//!
//! ```text
//! cargo run --release -p expanse-trie --example writer_scaling -- --arm blob --blob-op overwrite --key-dist <uniform|zipfian> --writers <W> --round <r> --position <p> [--quick]
//! ```
//!
//! - **Prefill.** The fresh-insert cell's 2^20 keys with their key-derived
//!   32-byte payloads, outside the timed window. Nothing else is inserted, so
//!   the population is 2^20 before and after, and the cell refuses to report
//!   otherwise.
//! - **Rank → key.** `rank_keys` is a Fisher–Yates permutation of the ascending
//!   prefill (the suite XorShift, seed `SEED_BLOB_RANK`); rank `r` is
//!   `rank_keys[r]`. Both key distributions index this one table, so the hot
//!   ranks are spread over the keyspace instead of sharing the lowest leaves.
//! - **Key choice.** Writer `w` draws its share of the 2^20 overwrites (⌊M/W⌋,
//!   the last writer taking the remainder) from its own
//!   `ycsb_common::XorShift64`, seeded `SEED_BLOB_OVERWRITE ^ (w + 1)·φ64 ^
//!   (round + 1)·0xD1B5_4A32_D192_ED03`. `zipfian` is
//!   `ycsb_common::ZipfianGenerator::new(2^20, ZIPFIAN_THETA).next(next_f64())`,
//!   θ = 0.99, rank 0 the hottest; `uniform` is `next_u64() % 2^20`. The ranks
//!   are drawn before the barrier, so the timed loop is identical under both:
//!   a table lookup, the payload, the insert.
//! - **Payload.** Eight bytes of a per-op counter (`(w + 1) << 40 | (i + 1)`)
//!   and three words derived from the key and that counter; the metadata word
//!   is derived from the same pair and is never zero.
//! - **Check, outside the window.** The 64 hottest ranks and every 1,000th rank
//!   after them are read back. A key no stream targeted must hold its prefill
//!   payload and metadata. A targeted key must hold one overwrite's: its
//!   counter must name a writer and an op index whose stream entry is that
//!   key, and payload and metadata must both be that counter's.
//! - **Rows.** `workload_id` `concurrency_writer_blob_overwrite_64bit`, cell
//!   `blob_overwrite_<dist>_w<W>_r0`, the fresh-insert row's fields plus
//!   `blob_op`, `key_dist`, `theta` (0 under `uniform`), `overwrites`,
//!   `distinct_keys_overwritten`, `verified_keys` and `verified_overwritten`;
//!   `fresh_keys` is 0. Each overwrite allocates a new arena record and
//!   retires the old one, so the arena grows by one record per overwrite while
//!   the index population does not.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `concurrency_writer_scaling` |
//! | `group` | 5 |
//! | `emits` | `concurrency_writer_map_64bit`, `concurrency_writer_set_63bit`, `concurrency_writer_str`, `concurrency_writer_bytes`, `concurrency_writer_blob_64bit`, `concurrency_writer_blob_overwrite_64bit`, `concurrency_ordered_readers_map_64bit`, `concurrency_readers_set_63bit`, `concurrency_readers_str` |
//! | `population` | prefill 2^20 keys (1M), plus 2^20 fresh keys inserted concurrently by W writers; reader mode (map only) adds 256 hotspot keys, one at offset 1 of every terminal byte of a 2^16-wide expanse, to every cell's prefill, and a hotspot cell's writers insert that expanse's other 65,280 keys instead of the 2^20 fresh keys; readers-only mode (set, str) prefills the arm's 2^20-key writer-sweep prefill and inserts nothing. Writer mode's `bytes` arm uses the `str` arm's `short` keys under a fixed-key SipHash hasher; its `blob` arm uses the map arm's 64-bit keys with a 32-byte key-derived arena payload and non-zero 24-bit metadata; the blob overwrite cell prefills the same 2^20 keys and performs 2^20 overwrites of them, inserting no fresh key |
//! | `insertion_order` | sorted — prefill ascending (reader mode: the sorted union with the hotspot keys), matching expanse-hot-bench; fresh stream in generator draw order; hotspot fresh keys Fisher–Yates shuffled; blob overwrite targets in per-writer stream draw order over a Fisher–Yates rank table |
//! | `probes_and_reuse` | writer mode: none (R = 0), insert-only; the blob overwrite cell re-targets prefilled keys, uniformly or Zipfian θ = 0.99 over the rank table, each writer on its own stream. Reader mode: R readers, each cycling its own Fisher–Yates permutation of the present uniform prefill (`uniform`) or of the 255 hotspot keys above the expanse's first terminal byte (`hotspot`); `get` or `prev_before` on a reader handle, or `prev_before` under `with_locked`, over the identical stream. Readers-only mode (set, str): R readers, each walking its own Fisher–Yates permutation of the prefill once, `contains` or `get` on a reader handle |
//! | `hit_rate` | writer mode: n/a. Reader mode: 100% — every probe is a present key; a hotspot `prev_before` fails in its own terminal byte and answers from the sibling below, until a writer inserts that byte's offset-0 key. Readers-only mode: 100% |
//! | `miss_gen_method` | same-generator rejection sampling against prefill; reader probes draw no misses |
//! | `value_dereference` | map arms check stored values against key-derived expectation; the blob arm checks payload bytes and metadata, and its overwrite cell that each read-back key holds its prefill or exactly one overwrite that targeted it; reader results fold key and value into a `black_box` accumulator |
//! | `measured_region` | writer mode: barrier release to last-writer join; the blob overwrite cell's prefill, Zipfian table, rank draws and read-back check are outside. Reader mode: barrier release to the last writer join (writers) and to the last reader join (readers); with W ≥ 1 readers stop once the writers have joined, with W = 0 each makes 2^20 probes; prefill, workload generation, reader registration, answer checks and teardown outside; every reader-mode throughput row also records each reader's own loop time |
//! | `arm_symmetry` | symmetric across thread counts; W in {1, 2, 4, 8} on physical P-cores; reader mode: `prev` and `prev_locked` run the same per-reader probe stream over the same tree, interleaved within each round by the driver; readers-only mode: the map (reader mode W = 0 `get`), set and str arms at R in {1, 2, 4, 8}, the R cells of each arm interleaved within each round by the driver |
//! | `statistics` | throughput ops/sec emitted raw, paired bootstrap BCa 95% CI for C(N); lock fallbacks and their six causes (partition-checked per row) from the occ-stats counters pass; reader mode: reader Mops/s with a BCa 95% CI per cell, the P12.5 per-round paired `prev` / `prev_locked` ratio with a BCa 95% CI, and P12.4's summed `read_fallbacks ÷ read_ops` from the counters pass; readers-only mode: reader Mops/s with a BCa 95% CI per (arm, R), S(R) = T(R) / T(1) paired within each round with a BCa 95% CI, slowest-over-mean reader loop time per round, and summed read counters |
//! | `verdict` | pending measurement |

use std::collections::HashSet;
use std::hint::black_box;
use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use expanse_trie::occ_stats::{self, Stat};
use expanse_trie::strmap::NulFreeStr;
use expanse_trie::sync::{
    SyncExpanseBlobMap, SyncExpanseBytesMap, SyncExpanseMap, SyncExpanseSet, SyncExpanseStrMap,
};

// The repository's one Zipfian generator (Gray et al.), shared with the YCSB
// suites; METHODOLOGY §21.11 names it as the blob overwrite cell's.
#[path = "../benches/ycsb_common/mod.rs"]
mod ycsb_common;
use ycsb_common::{XorShift64, ZIPFIAN_THETA, ZipfianGenerator};

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
    /// Optimistic read calls (`Stat::ReadOps`). Reader rows only.
    read_ops: u64,
    /// Optimistic walk attempts (`Stat::ReadAttempts`). Reader rows only.
    read_attempts: u64,
    /// Retry-exhausted reads that fell back to `read_locked`. Reader rows only.
    read_fallbacks: u64,
    /// Reads under the writer mutex by any route: `read_locked` and
    /// `with_locked` both count here (`Stat::LockedReads`). Reader rows only.
    locked_reads: u64,
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
            read_ops: snap[Stat::ReadOps as usize],
            read_attempts: snap[Stat::ReadAttempts as usize],
            read_fallbacks: snap[Stat::ReadFallbacks as usize],
            locked_reads: snap[Stat::LockedReads as usize],
        }
    }

    /// The reader counters of a reader-mode counters row.
    fn reader_counters_json(&self) -> String {
        format!(
            "\"read_ops\":{},\"read_attempts\":{},\"read_fallbacks\":{},\"locked_reads\":{}",
            self.read_ops, self.read_attempts, self.read_fallbacks, self.locked_reads
        )
    }

    /// The identities a reader-mode counters row owes on top of [`Self::check`].
    ///
    /// An optimistic reader (`prev`, `get`) counts one `ReadOps` per call and
    /// reaches the writer mutex only through a retry-exhausted fallback, which
    /// bumps `ReadFallbacks` and then `LockedReads` inside `read_locked`. A
    /// `prev_locked` reader never enters the optimistic protocol and bumps
    /// `LockedReads` once per call inside `with_locked`.
    fn check_reader(&self, op: ReadOp, reader_ops: u64, cell: &str) -> Result<(), String> {
        match op {
            ReadOp::Prev | ReadOp::Get => {
                if self.read_ops != reader_ops {
                    return Err(format!(
                        "{cell}: read_ops = {}, readers made {reader_ops} calls",
                        self.read_ops
                    ));
                }
                if self.locked_reads != self.read_fallbacks {
                    return Err(format!(
                        "{cell}: locked_reads = {}, read_fallbacks = {} (an optimistic reader \
                         reaches the writer mutex only by falling back)",
                        self.locked_reads, self.read_fallbacks
                    ));
                }
            }
            ReadOp::PrevLocked => {
                if self.read_ops != 0 {
                    return Err(format!(
                        "{cell}: read_ops = {} on a with_locked reader cell, expected 0",
                        self.read_ops
                    ));
                }
                if self.locked_reads != reader_ops {
                    return Err(format!(
                        "{cell}: locked_reads = {}, readers made {reader_ops} with_locked calls",
                        self.locked_reads
                    ));
                }
            }
        }
        Ok(())
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
    /// fallbacks plus `locked_reads`.
    ///
    /// `Shared::read_locked` and `Shared::with_locked` both call
    /// `quiesce_writers`, so every locked read quiesces once. An insert-only
    /// row passes `locked_reads = 0`, which is the identity the writer sweep
    /// has always checked.
    fn check(&self, expected_inserts: u64, locked_reads: u64, cell: &str) -> Result<(), String> {
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
        if self.quiesce_calls != self.lock_fallbacks + locked_reads {
            return Err(format!(
                "{cell}: quiesce_calls = {}, lock_fallbacks = {}, locked_reads = {locked_reads}",
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

// ---------------------------------------------------------------------------
// The byte-string and blob wrappers (#929 step 1)
// ---------------------------------------------------------------------------

/// The bytes arm's hasher: SipHash-1-3 with fixed zero keys, so every process
/// lays out the same hash trie. `SyncExpanseBytesMap::new` seeds per process,
/// and a per-process layout would add a between-process variance the
/// one-process-per-cell design exists to expose, not to create.
type DetHasher = std::hash::BuildHasherDefault<std::collections::hash_map::DefaultHasher>;

/// Payload length of the blob arm. Above the 7-byte inline limit, so every
/// value is an arena payload (`ExpanseBlobMap::insert`).
pub const BLOB_PAYLOAD_LEN: usize = 32;

/// The blob arm's payload for `k`: four rotations of the map arm's value.
fn blob_payload(k: u64) -> [u8; BLOB_PAYLOAD_LEN] {
    let v = value_of(k);
    let mut out = [0u8; BLOB_PAYLOAD_LEN];
    for i in 0..BLOB_PAYLOAD_LEN / 8 {
        out[8 * i..8 * (i + 1)].copy_from_slice(&v.rotate_left(13 * i as u32).to_le_bytes());
    }
    out
}

/// The blob arm's 24-bit hot metadata for `k`. Never zero, so an insert never
/// takes the metadata-free compressed-inline path and always reaches the arena.
fn blob_meta(k: u64) -> u32 {
    ((value_of(k) >> 40) as u32 & 0x00FF_FFFF) | 1
}

fn run_bytes_cell(
    workload: &WriterStrWorkload,
    writers: usize,
    round: usize,
    is_counters: bool,
    perf_ctl: &mut PerfControl,
) -> (f64, u64, Counters) {
    let map = SyncExpanseBytesMap::with_hasher(DetHasher::default());
    for k in &workload.prefill {
        map.insert(k, str_value_of(k));
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
                    m.insert(k, str_value_of(k));
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
        assert_eq!(
            reader.get(k),
            Some(str_value_of(k)),
            "bytes missing fresh key at round {round}"
        );
    }

    (elapsed, final_pop, counters)
}

fn run_blob_cell(
    workload: &WriterWorkload,
    writers: usize,
    round: usize,
    is_counters: bool,
    perf_ctl: &mut PerfControl,
) -> (f64, u64, Counters) {
    let map = SyncExpanseBlobMap::new();
    for &k in &workload.prefill {
        map.insert(k, &blob_payload(k), blob_meta(k))
            .expect("blob prefill insert");
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
                for &k in slice {
                    m.insert(k, &blob_payload(k), blob_meta(k))
                        .expect("blob insert");
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
    let final_pop = map.with_locked(|m| m.len());

    // Verify samples: payload bytes and metadata both round-trip.
    let mut reader = map.reader();
    for &k in workload.fresh_keys.iter().step_by(10_000) {
        assert_eq!(
            reader.get(k),
            Some((blob_payload(k).to_vec(), blob_meta(k))),
            "blob missing or corrupt fresh key {k} at round {round}"
        );
    }

    (elapsed, final_pop, counters)
}

// ---------------------------------------------------------------------------
// The blob overwrite cell (#929, docs/benchmarks/concurrency/METHODOLOGY.md
// §21.4 G4 and §21.11)
// ---------------------------------------------------------------------------

/// Seed of the Fisher–Yates permutation that maps a rank to a prefill key.
pub const SEED_BLOB_RANK: u64 = SEED_PREFILL ^ 0x5EED_C0DE_0000_0B10;
/// Base seed of the per-writer key-choice streams.
pub const SEED_BLOB_OVERWRITE: u64 = SEED_PREFILL ^ 0x5EED_C0DE_0000_0B11;
/// Odd multiplier that separates the writer index in a stream seed (φ64).
const STREAM_WRITER_MIX: u64 = 0x9E37_79B9_7F4A_7C15;
/// Odd multiplier that separates the round in a stream seed.
const STREAM_ROUND_MIX: u64 = 0xD1B5_4A32_D192_ED03;
/// Bits of an overwrite counter that hold the op index; the writer sits above.
const OVERWRITE_OP_BITS: u32 = 40;

/// What the blob arm's writers do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BlobOp {
    /// Fresh keys, the §21.4 G1–G3 cell.
    Insert,
    /// Overwrites of prefilled keys, the §21.4 G4 cell.
    Overwrite,
}

impl BlobOp {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "insert" => Ok(Self::Insert),
            "overwrite" => Ok(Self::Overwrite),
            other => Err(format!(
                "--blob-op takes insert or overwrite, got {other:?}"
            )),
        }
    }
}

/// How an overwrite picks its key.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum KeyDist {
    Uniform,
    Zipfian,
}

impl KeyDist {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "uniform" => Ok(Self::Uniform),
            "zipfian" => Ok(Self::Zipfian),
            other => Err(format!(
                "--key-dist takes uniform or zipfian, got {other:?}"
            )),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Uniform => "uniform",
            Self::Zipfian => "zipfian",
        }
    }

    /// The skew the row reports: `ZIPFIAN_THETA`, or 0 for uniform choice.
    fn theta(self) -> f64 {
        match self {
            Self::Uniform => 0.0,
            Self::Zipfian => ZIPFIAN_THETA,
        }
    }
}

/// The overwrite cell's fixed inputs: the fresh-insert cell's prefill, and the
/// rank → key table both key distributions index.
struct BlobOverwriteWorkload {
    /// Ascending: the keys `WriterWorkload::generate(n0, _, 64)` prefills.
    prefill: Vec<u64>,
    /// `rank_keys[r]` is the key of rank `r`: a Fisher–Yates permutation of
    /// `prefill`, so the hot ranks are spread over the keyspace rather than
    /// packed into the lowest leaves of the sorted prefill.
    rank_keys: Vec<u64>,
    /// Built once per process, outside every timed window: its constructor
    /// sums an N0-term zeta.
    zipf: ZipfianGenerator,
    keyspace_bits: u32,
}

impl BlobOverwriteWorkload {
    fn generate(n_prefill: usize) -> Self {
        assert!(
            n_prefill > 0 && n_prefill <= u32::MAX as usize,
            "overwrite prefill must be in 1..=u32::MAX"
        );
        let base = WriterWorkload::generate(n_prefill, 0, 64);
        let mut rank_keys = base.prefill.clone();
        let mut rng = XorShift::new(SEED_BLOB_RANK);
        for i in (1..rank_keys.len()).rev() {
            let j = (rng.next() % (i as u64 + 1)) as usize;
            rank_keys.swap(i, j);
        }
        Self {
            zipf: ZipfianGenerator::new(n_prefill as u64, ZIPFIAN_THETA),
            prefill: base.prefill,
            rank_keys,
            keyspace_bits: base.keyspace_bits,
        }
    }

    /// Writer `w`'s ranks for `round`: `len` draws from its own stream, seeded
    /// from (suite seed, writer index, round).
    fn stream(&self, dist: KeyDist, w: usize, round: usize, len: usize) -> Vec<u32> {
        let seed = SEED_BLOB_OVERWRITE
            ^ (w as u64 + 1).wrapping_mul(STREAM_WRITER_MIX)
            ^ (round as u64 + 1).wrapping_mul(STREAM_ROUND_MIX);
        let mut rng = XorShift64::new(seed);
        let n = self.rank_keys.len() as u64;
        (0..len)
            .map(|_| match dist {
                KeyDist::Zipfian => self.zipf.next(rng.next_f64()) as u32,
                KeyDist::Uniform => (rng.next_u64() % n) as u32,
            })
            .collect()
    }
}

/// The per-op counter of writer `w`'s `i`-th overwrite. Never zero.
#[inline]
fn overwrite_counter(w: usize, i: usize) -> u64 {
    ((w as u64 + 1) << OVERWRITE_OP_BITS) | (i as u64 + 1)
}

/// An overwrite's payload: the counter, then three rotations of a word
/// derived from the key and the counter, so a payload names the op that wrote
/// it and cannot be assembled from two ops' halves.
#[inline]
fn overwrite_payload(k: u64, c: u64) -> [u8; BLOB_PAYLOAD_LEN] {
    let v = value_of(k ^ c.wrapping_mul(STREAM_ROUND_MIX));
    let mut out = [0u8; BLOB_PAYLOAD_LEN];
    out[..8].copy_from_slice(&c.to_le_bytes());
    for i in 1..BLOB_PAYLOAD_LEN / 8 {
        out[8 * i..8 * (i + 1)].copy_from_slice(&v.rotate_left(13 * i as u32).to_le_bytes());
    }
    out
}

/// An overwrite's 24-bit metadata. Never zero, as [`blob_meta`].
#[inline]
fn overwrite_meta(k: u64, c: u64) -> u32 {
    ((value_of(k ^ c.wrapping_mul(STREAM_ROUND_MIX)) >> 40) as u32 & 0x00FF_FFFF) | 1
}

/// What one overwrite cell reports beyond its time and counters.
struct BlobOverwriteOutcome {
    elapsed_s: f64,
    final_pop: u64,
    counters: Counters,
    /// Overwrites performed, summed over the writers.
    overwrites: u64,
    /// Distinct prefill keys the streams targeted.
    distinct_keys: u64,
    /// Keys read back after the window, and how many of them held an overwrite.
    verified_keys: u64,
    verified_overwritten: u64,
}

/// Every rank the check reads back: the 64 hottest, then every 1,000th.
fn overwrite_check_ranks(n: usize) -> impl Iterator<Item = usize> {
    (0..n.min(64)).chain((64..n).step_by(1_000))
}

/// What a blob read returns: the payload and its metadata word, or nothing.
type BlobRead = Option<(Vec<u8>, u32)>;

/// One read-back key: the payload and metadata must be the prefill's when no
/// stream targeted the key, and otherwise one single overwrite's — naming a
/// writer and an op index whose stream entry is this key.
fn check_overwrite_readback(
    workload: &BlobOverwriteWorkload,
    streams: &[Vec<u32>],
    rank: usize,
    targeted: bool,
    got: BlobRead,
) -> Result<bool, String> {
    let k = workload.rank_keys[rank];
    let (payload, meta) = got.ok_or_else(|| format!("key {k:#x} (rank {rank}) is absent"))?;
    let is_prefill = payload == blob_payload(k) && meta == blob_meta(k);
    if !targeted {
        return if is_prefill {
            Ok(false)
        } else {
            Err(format!(
                "key {k:#x} (rank {rank}) was never targeted but does not hold its prefill payload"
            ))
        };
    }
    if is_prefill {
        return Err(format!(
            "key {k:#x} (rank {rank}) was targeted but still holds its prefill payload"
        ));
    }
    if payload.len() != BLOB_PAYLOAD_LEN {
        return Err(format!(
            "key {k:#x} (rank {rank}) holds {} payload bytes",
            payload.len()
        ));
    }
    let c = u64::from_le_bytes(payload[..8].try_into().expect("eight bytes"));
    let w = (c >> OVERWRITE_OP_BITS) as usize;
    let i = (c & ((1u64 << OVERWRITE_OP_BITS) - 1)) as usize;
    let named = w
        .checked_sub(1)
        .and_then(|w| streams.get(w))
        .zip(i.checked_sub(1))
        .and_then(|(s, i)| s.get(i));
    if named != Some(&(rank as u32)) {
        return Err(format!(
            "key {k:#x} (rank {rank}) holds counter {c:#x}, which names no overwrite of this key"
        ));
    }
    if payload != overwrite_payload(k, c) || meta != overwrite_meta(k, c) {
        return Err(format!(
            "key {k:#x} (rank {rank}) holds a payload or metadata that is not counter {c:#x}'s"
        ));
    }
    Ok(true)
}

fn run_blob_overwrite_cell(
    workload: &BlobOverwriteWorkload,
    overwrites: usize,
    dist: KeyDist,
    writers: usize,
    round: usize,
    is_counters: bool,
    perf_ctl: &mut PerfControl,
) -> Result<BlobOverwriteOutcome, String> {
    let map = SyncExpanseBlobMap::new();
    for &k in &workload.prefill {
        map.insert(k, &blob_payload(k), blob_meta(k))
            .expect("blob prefill insert");
    }

    // Each writer's share of the M overwrites, the last taking the remainder,
    // as the fresh-insert cell splits its keys.
    let per = overwrites / writers.max(1);
    let streams: Vec<Vec<u32>> = (0..writers)
        .map(|w| {
            let len = if w + 1 == writers {
                overwrites - per * w
            } else {
                per
            };
            workload.stream(dist, w, round, len)
        })
        .collect();
    let mut targeted = vec![false; workload.rank_keys.len()];
    for &r in streams.iter().flatten() {
        targeted[r as usize] = true;
    }

    let barrier = Barrier::new(writers + 1);
    if is_counters {
        occ_stats::reset();
    }

    let start = std::thread::scope(|s| {
        for (w, stream) in streams.iter().enumerate() {
            let b = &barrier;
            let m = &map;
            let rank_keys = &workload.rank_keys;
            s.spawn(move || {
                b.wait();
                for (i, &r) in stream.iter().enumerate() {
                    let k = rank_keys[r as usize];
                    let c = overwrite_counter(w, i);
                    m.insert(k, &overwrite_payload(k, c), overwrite_meta(k, c))
                        .expect("blob overwrite");
                }
            });
        }

        perf_ctl.enable();
        barrier.wait();
        Instant::now()
    });
    perf_ctl.disable();

    let elapsed_s = if is_counters {
        0.0
    } else {
        start.elapsed().as_secs_f64()
    };
    let counters = Counters::read(is_counters);
    let final_pop = map.with_locked(|m| m.len());
    if final_pop != workload.prefill.len() as u64 {
        return Err(format!(
            "population {final_pop} after {overwrites} overwrites of {} prefilled keys at round {round}: \
             an overwrite must not change it",
            workload.prefill.len()
        ));
    }

    let mut reader = map.reader();
    let (mut verified_keys, mut verified_overwritten) = (0u64, 0u64);
    for rank in overwrite_check_ranks(workload.rank_keys.len()) {
        let got = reader.get(workload.rank_keys[rank]);
        let overwritten = check_overwrite_readback(workload, &streams, rank, targeted[rank], got)
            .map_err(|e| format!("round {round}: {e}"))?;
        verified_keys += 1;
        verified_overwritten += u64::from(overwritten);
    }
    if verified_overwritten == 0 {
        return Err(format!(
            "round {round}: none of the {verified_keys} keys read back held an overwrite"
        ));
    }

    Ok(BlobOverwriteOutcome {
        elapsed_s,
        final_pop,
        counters,
        overwrites: streams.iter().map(|s| s.len() as u64).sum(),
        distinct_keys: targeted.iter().filter(|&&t| t).count() as u64,
        verified_keys,
        verified_overwritten,
    })
}

// ---------------------------------------------------------------------------
// Reader mode (#900, docs/benchmarks/concurrency/METHODOLOGY.md §12.4)
// ---------------------------------------------------------------------------

/// Width of the hotspot expanse (§12.4): 2^16 keys.
pub const HOTSPOT_WIDTH: u64 = 1 << 16;
/// One terminal byte of the hotspot expanse.
pub const HOTSPOT_BUCKET: u64 = 1 << 8;
/// Terminal bytes in the hotspot expanse, which is also the hotspot prefill size.
pub const HOTSPOT_BUCKETS: u64 = HOTSPOT_WIDTH / HOTSPOT_BUCKET;
/// Offset of the one prefill key in each terminal byte. It must be above
/// offset 0, so that `prev_before` of it searches its own terminal byte and
/// fails there (see the module doc).
pub const HOTSPOT_PREFILL_OFFSET: u64 = 1;

// The hotspot prefill must not fit one linear leaf, or a two-byte leaf would
// answer every probe in place and no search would reach a branch.
// `LEAF_CAP` (crates/expanse/src/types.rs:99) caps linear leaves at levels
// 2..=7, and map inserts apply it to every multi-byte leaf
// (crates/expanse/src/mutate_map.rs:791, :1438).
const _: () = assert!(
    HOTSPOT_BUCKETS as usize > expanse_trie::types::LEAF_CAP,
    "the hotspot prefill fits one linear leaf, so a level-2 branch is not guaranteed"
);
/// Seed whose first draw, with its low 16 bits cleared, is the hotspot base `H`.
pub const SEED_HOTSPOT_BASE: u64 = SEED_PREFILL ^ 0x5EED_C0DE_0000_0900;
/// Seed of the Fisher–Yates shuffle of the hotspot writers' fresh keys.
pub const SEED_HOTSPOT_FRESH: u64 = SEED_PREFILL ^ 0x5EED_C0DE_0000_0901;
/// Base of the per-reader probe shuffle seeds; see [`reader_seed`].
pub const SEED_READER: u64 = SEED_PREFILL ^ 0x5EED_C0DE_0000_0902;

/// What a reader thread asks on each probe.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ReadOp {
    /// `MapReader::get`, optimistic.
    Get,
    /// `ExpanseMap::prev_before` under `SyncExpanseMap::with_locked`.
    PrevLocked,
    /// `MapReader::prev_before`, optimistic.
    Prev,
}

impl ReadOp {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "get" => Ok(Self::Get),
            "prev_locked" => Ok(Self::PrevLocked),
            "prev" => Ok(Self::Prev),
            other => Err(format!(
                "unknown --read-op {other:?} (expected get, prev_locked or prev)"
            )),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::PrevLocked => "prev_locked",
            Self::Prev => "prev",
        }
    }
}

/// Where the probe keys come from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Probe {
    /// The present uniform prefill keys.
    Uniform,
    /// The offset-1 hotspot keys of every terminal byte above the first.
    Hotspot,
}

impl Probe {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "uniform" => Ok(Self::Uniform),
            "hotspot" => Ok(Self::Hotspot),
            other => Err(format!(
                "unknown --probe {other:?} (expected uniform or hotspot)"
            )),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Uniform => "uniform",
            Self::Hotspot => "hotspot",
        }
    }
}

/// Fisher–Yates with the suite XorShift. The modulo reduction's bias is at
/// most `len / 2^64`, below 2^-43 at the sizes used here.
fn fisher_yates(keys: &mut [u64], seed: u64) {
    assert_ne!(seed, 0, "XorShift64 has a fixed point at 0");
    let mut rng = XorShift::new(seed);
    for i in (1..keys.len()).rev() {
        let j = (rng.next() % (i as u64 + 1)) as usize;
        keys.swap(i, j);
    }
}

/// Probe shuffle seed of reader `reader`: `SEED_READER ^ (reader + 1) * φ64`.
fn reader_seed(reader: usize) -> u64 {
    SEED_READER ^ (reader as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// The 2^16-wide expanse the hotspot probes and the hotspot writers share.
struct HotspotExpanse {
    base: u64,
    /// `H + 256·j + HOTSPOT_PREFILL_OFFSET` for every terminal byte `j`, ascending.
    prefill: Vec<u64>,
    /// The prefill keys of terminal bytes `j ≥ 1`: block 0 has no lower
    /// sibling inside the expanse to backtrack into.
    probes: Vec<u64>,
    /// Every other key of every terminal byte, shuffled with `SEED_HOTSPOT_FRESH`.
    fresh: Vec<u64>,
}

impl HotspotExpanse {
    fn generate() -> Self {
        // Clearing the low 16 bits leaves `base + HOTSPOT_WIDTH <= 2^64`.
        let base = XorShift::new(SEED_HOTSPOT_BASE).next() & !(HOTSPOT_WIDTH - 1);
        let prefill: Vec<u64> = (0..HOTSPOT_BUCKETS)
            .map(|j| base + j * HOTSPOT_BUCKET + HOTSPOT_PREFILL_OFFSET)
            .collect();
        let probes = prefill[1..].to_vec();
        let mut fresh = Vec::with_capacity((HOTSPOT_WIDTH - HOTSPOT_BUCKETS) as usize);
        for j in 0..HOTSPOT_BUCKETS {
            for offset in (0..HOTSPOT_BUCKET).filter(|&o| o != HOTSPOT_PREFILL_OFFSET) {
                fresh.push(base + j * HOTSPOT_BUCKET + offset);
            }
        }
        fisher_yates(&mut fresh, SEED_HOTSPOT_FRESH);
        Self {
            base,
            prefill,
            probes,
            fresh,
        }
    }

    fn contains(&self, k: u64) -> bool {
        k.wrapping_sub(self.base) < HOTSPOT_WIDTH
    }

    /// Whether any prefill key lies in `[lo, hi]`.
    fn prefill_in(&self, lo: u64, hi: u64) -> bool {
        self.prefill.iter().any(|&p| lo <= p && p <= hi)
    }

    /// Checks, on the prefilled tree, that `prev_before` of every probe fails
    /// in one terminal byte and must descend the sibling below it.
    ///
    /// Stated on the search target `t = k − 1`, which is what `nav::prev`
    /// descends by, not on the probe's own terminal byte: for a probe at
    /// offset 0, `t` is already in the byte below and answers there.
    fn check_backtrack_geometry(&self) -> Result<(), String> {
        if self.probes.len() as u64 != HOTSPOT_BUCKETS - 1 {
            return Err(format!(
                "{} hotspot probes, expected {}",
                self.probes.len(),
                HOTSPOT_BUCKETS - 1
            ));
        }
        for &k in &self.probes {
            let t = k
                .checked_sub(1)
                .ok_or_else(|| "a hotspot probe of 0 has no search target".to_string())?;
            if !self.contains(t) {
                return Err(format!(
                    "probe {k:#x}: its search target leaves the expanse"
                ));
            }
            let j = (t - self.base) / HOTSPOT_BUCKET;
            if j == 0 {
                return Err(format!(
                    "probe {k:#x}: its search target is in terminal byte 0, which has no lower \
                     sibling inside the expanse"
                ));
            }
            let lo = self.base + j * HOTSPOT_BUCKET;
            let hi = lo + HOTSPOT_BUCKET - 1;
            if self.prefill_in(lo, t) {
                return Err(format!(
                    "probe {k:#x}: a prefill key in [{lo:#x}, {t:#x}] answers inside terminal \
                     byte {j}, so the search makes no sibling descent"
                ));
            }
            if t == hi || !self.prefill_in(t + 1, hi) {
                return Err(format!(
                    "probe {k:#x}: terminal byte {j} holds no prefill key above the target, so \
                     there is no child for the search to fail in"
                ));
            }
            if !self.prefill_in(lo - HOTSPOT_BUCKET, lo - 1) {
                return Err(format!(
                    "probe {k:#x}: terminal byte {} holds no prefill key for the sibling \
                     descent to answer",
                    j - 1
                ));
            }
        }
        Ok(())
    }
}

/// A reader-mode workload: the writer sweep's map workload, the hotspot
/// expanse, and one probe permutation per reader.
struct ReaderWorkload {
    uniform: WriterWorkload,
    hotspot: HotspotExpanse,
    /// Every prefilled key, sorted: the uniform prefill and the hotspot keys.
    prefill_all: Vec<u64>,
    /// One probe permutation per reader.
    probes: Vec<Vec<u64>>,
}

impl ReaderWorkload {
    fn generate(
        n_prefill: usize,
        m_fresh: usize,
        probe: Probe,
        readers: usize,
    ) -> Result<Self, String> {
        let uniform = WriterWorkload::generate(n_prefill, m_fresh, 64);
        let hotspot = HotspotExpanse::generate();
        let inside_prefill = uniform
            .prefill
            .iter()
            .filter(|&&k| hotspot.contains(k))
            .count();
        let inside_fresh = uniform
            .fresh_keys
            .iter()
            .filter(|&&k| hotspot.contains(k))
            .count();
        if inside_prefill + inside_fresh != 0 {
            return Err(format!(
                "the hotspot expanse at {:#x} overlaps the uniform workload: {inside_prefill} \
                 prefill and {inside_fresh} fresh keys fall inside it",
                hotspot.base
            ));
        }
        let mut prefill_all = uniform.prefill.clone();
        prefill_all.extend_from_slice(&hotspot.prefill);
        prefill_all.sort_unstable();
        let source = match probe {
            Probe::Uniform => &uniform.prefill,
            Probe::Hotspot => &hotspot.probes,
        };
        let probes = (0..readers)
            .map(|r| {
                let mut v = source.clone();
                fisher_yates(&mut v, reader_seed(r));
                v
            })
            .collect();
        Ok(Self {
            uniform,
            hotspot,
            prefill_all,
            probes,
        })
    }

    /// The keys the writers of a `probe` cell insert.
    fn fresh(&self, probe: Probe) -> &[u64] {
        match probe {
            Probe::Uniform => &self.uniform.fresh_keys,
            Probe::Hotspot => &self.hotspot.fresh,
        }
    }
}

/// One reader-mode cell.
#[derive(Clone, Copy)]
struct ReaderCell {
    writers: usize,
    readers: usize,
    op: ReadOp,
    probe: Probe,
}

/// What one reader-mode cell measured.
struct ReaderOutcome {
    /// `None` at W = 0.
    writer_elapsed_s: Option<f64>,
    reader_elapsed_s: f64,
    /// Each reader thread's own probe-loop time, from its return from the
    /// barrier to the end of its loop, in reader order. `reader_elapsed_s` is
    /// the slowest reader plus the join; these give the mean beside it.
    thread_elapsed_s: Vec<f64>,
    reader_ops: u64,
    fresh_keys: u64,
    final_pop: u64,
    counters: Counters,
}

/// Raises the stop flag when dropped, so a panicking writer join cannot leave
/// the readers probing and the scope waiting on them forever.
struct StopOnDrop<'a>(&'a AtomicBool);

impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// An ordered-read answer folded into the accumulator.
#[inline(always)]
fn fold_entry(e: Option<(u64, u64)>) -> u64 {
    match e {
        Some((k, v)) => k ^ v,
        None => 0,
    }
}

/// One reader's timed loop: exactly `limit` probes when given, otherwise
/// until `stop` is raised. Every answer is folded into an accumulator that is
/// `black_box`ed (AGENTS.md §8.6). Returns the probes made.
#[inline(always)]
fn probe_loop(
    probes: &[u64],
    limit: Option<u64>,
    stop: &AtomicBool,
    mut op: impl FnMut(u64) -> u64,
) -> u64 {
    let mut sink = 0u64;
    let mut ops = 0u64;
    let mut i = 0usize;
    let len = probes.len();
    match limit {
        Some(n) => {
            while ops < n {
                sink = sink.wrapping_add(op(probes[i]));
                i += 1;
                if i == len {
                    i = 0;
                }
                ops += 1;
            }
        }
        None => {
            while !stop.load(Ordering::Relaxed) {
                sink = sink.wrapping_add(op(probes[i]));
                i += 1;
                if i == len {
                    i = 0;
                }
                ops += 1;
            }
        }
    }
    black_box(sink);
    ops
}

/// Checks a sample of `prev_before` answers, optimistic and locked, against
/// the sorted prefill. Untimed, and only meaningful at W = 0, where the
/// prefill is the whole tree.
fn verify_prev_answers(
    map: &SyncExpanseMap,
    sorted: &[u64],
    probes: &[u64],
    round: usize,
) -> Result<(), String> {
    let reader = map.reader();
    let step = (probes.len() / 512).max(1);
    for &k in probes.iter().step_by(step) {
        let Ok(i) = sorted.binary_search(&k) else {
            return Err(format!("probe {k:#x} is not a prefilled key"));
        };
        let want = i.checked_sub(1).map(|j| (sorted[j], value_of(sorted[j])));
        let optimistic = reader.prev_before(k);
        let locked = map.with_locked(|m| m.prev_before(k));
        if optimistic != want || locked != want {
            return Err(format!(
                "round {round}: prev_before({k:#x}) answered {optimistic:?} optimistically and \
                 {locked:?} locked; the sorted prefill says {want:?}"
            ));
        }
    }
    Ok(())
}

fn run_reader_cell(
    wl: &ReaderWorkload,
    cell: ReaderCell,
    round: usize,
    is_counters: bool,
    perf_ctl: &mut PerfControl,
) -> Result<ReaderOutcome, String> {
    if wl.probes.len() != cell.readers {
        return Err(format!(
            "workload carries {} probe streams for {} readers",
            wl.probes.len(),
            cell.readers
        ));
    }
    let map = SyncExpanseMap::new();
    for &k in &wl.prefill_all {
        map.insert(k, value_of(k));
    }

    let fresh = if cell.writers == 0 {
        &[][..]
    } else {
        wl.fresh(cell.probe)
    };
    let per = fresh.len() / cell.writers.max(1);
    let limit = (cell.writers == 0).then_some(wl.uniform.prefill.len() as u64);
    let stop = AtomicBool::new(false);
    let barrier = Barrier::new(cell.writers + cell.readers + 1);

    let allocs_before = if is_counters {
        map.with_locked(|inner| inner.total_node_allocs() as u64)
    } else {
        0
    };

    // Nothing but the cell's own threads runs between here and the snapshot:
    // no `len`, no `with_locked`, no verification read.
    if is_counters {
        occ_stats::reset();
    }

    let (writer_elapsed_s, reader_elapsed_s, reader_ops, thread_elapsed_s) =
        std::thread::scope(|s| {
            let writer_handles: Vec<_> = (0..cell.writers)
                .map(|w| {
                    let lo = w * per;
                    let hi = if w + 1 == cell.writers {
                        fresh.len()
                    } else {
                        lo + per
                    };
                    let slice = &fresh[lo..hi];
                    let b = &barrier;
                    let m = &map;
                    s.spawn(move || {
                        b.wait();
                        for &k in slice {
                            m.insert(k, value_of(k));
                        }
                    })
                })
                .collect();
            let reader_handles: Vec<_> = wl
                .probes
                .iter()
                .map(|probes| {
                    let probes = probes.as_slice();
                    let b = &barrier;
                    let m = &map;
                    let stop = &stop;
                    s.spawn(move || match cell.op {
                        ReadOp::Prev => {
                            let rd = m.reader();
                            b.wait();
                            let t0 = Instant::now();
                            let n =
                                probe_loop(probes, limit, stop, |k| fold_entry(rd.prev_before(k)));
                            (n, t0.elapsed().as_secs_f64())
                        }
                        ReadOp::Get => {
                            let rd = m.reader();
                            b.wait();
                            let t0 = Instant::now();
                            let n = probe_loop(probes, limit, stop, |k| rd.get(k).unwrap_or(0));
                            (n, t0.elapsed().as_secs_f64())
                        }
                        ReadOp::PrevLocked => {
                            b.wait();
                            let t0 = Instant::now();
                            let n = probe_loop(probes, limit, stop, |k| {
                                fold_entry(m.with_locked(|t| t.prev_before(k)))
                            });
                            (n, t0.elapsed().as_secs_f64())
                        }
                    })
                })
                .collect();

            perf_ctl.enable();
            barrier.wait();
            let start = Instant::now();
            let guard = StopOnDrop(&stop);
            let mut writer_elapsed = None;
            if !writer_handles.is_empty() {
                for h in writer_handles {
                    h.join().expect("writer thread panicked");
                }
                writer_elapsed = Some(start.elapsed().as_secs_f64());
            }
            drop(guard);
            let mut ops = 0u64;
            let mut per_thread = Vec::with_capacity(cell.readers);
            for h in reader_handles {
                let (n, secs) = h.join().expect("reader thread panicked");
                ops += n;
                per_thread.push(secs);
            }
            (
                writer_elapsed,
                start.elapsed().as_secs_f64(),
                ops,
                per_thread,
            )
        });
    perf_ctl.disable();

    let mut counters = Counters::read(is_counters);
    if is_counters {
        counters.total_allocs = Some(
            map.with_locked(|inner| inner.total_node_allocs() as u64)
                .saturating_sub(allocs_before),
        );
    }
    let final_pop = map.len();
    let expected_pop = (wl.prefill_all.len() + fresh.len()) as u64;
    if final_pop != expected_pop {
        return Err(format!(
            "round {round}: population {final_pop} after the cell, expected {expected_pop}"
        ));
    }
    if reader_ops == 0 {
        return Err(format!(
            "round {round}: the readers made no probes, so the cell has no reader throughput"
        ));
    }

    // Untimed answer checks.
    let reader = map.reader();
    for &k in fresh.iter().step_by(10_000) {
        if reader.get(k) != Some(value_of(k)) {
            return Err(format!("round {round}: map missing fresh key {k:#x}"));
        }
    }
    if cell.writers == 0 {
        verify_prev_answers(&map, &wl.prefill_all, &wl.probes[0], round)?;
    }

    Ok(ReaderOutcome {
        writer_elapsed_s,
        reader_elapsed_s,
        thread_elapsed_s,
        reader_ops,
        fresh_keys: fresh.len() as u64,
        final_pop,
        counters,
    })
}

// ---------------------------------------------------------------------------
// Readers-only cells on the set and string arms (#730)
// ---------------------------------------------------------------------------

/// The key population of a readers-only cell, and each reader's probe order.
///
/// Probes are indices into `prefill`, one Fisher–Yates permutation per reader
/// with the reader-mode seeds, so every probe is a present key and the three
/// arms draw their orders from one generator.
struct ReadersOnlyWorkload<K> {
    prefill: Vec<K>,
    probes: Vec<Vec<u64>>,
}

impl<K> ReadersOnlyWorkload<K> {
    fn new(prefill: Vec<K>, readers: usize) -> Self {
        let probes = (0..readers)
            .map(|r| {
                let mut v: Vec<u64> = (0..prefill.len() as u64).collect();
                fisher_yates(&mut v, reader_seed(r));
                v
            })
            .collect();
        Self { prefill, probes }
    }
}

/// `readers` threads behind one barrier, W = 0, each making exactly one
/// probe per prefilled key. `body(r, barrier)` registers its reader, waits on
/// the barrier and returns its probe count and its own loop time. Main's clock
/// runs from the barrier release to the last join.
fn timed_readers<F>(readers: usize, perf_ctl: &mut PerfControl, body: F) -> (f64, u64, Vec<f64>)
where
    F: Fn(usize, &Barrier) -> (u64, f64) + Sync,
{
    let barrier = Barrier::new(readers + 1);
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..readers)
            .map(|r| {
                let b = &barrier;
                let body = &body;
                s.spawn(move || body(r, b))
            })
            .collect();
        perf_ctl.enable();
        barrier.wait();
        let start = Instant::now();
        let mut ops = 0u64;
        let mut per_thread = Vec::with_capacity(readers);
        for h in handles {
            let (n, secs) = h.join().expect("reader thread panicked");
            ops += n;
            per_thread.push(secs);
        }
        let elapsed = start.elapsed().as_secs_f64();
        perf_ctl.disable();
        (elapsed, ops, per_thread)
    })
}

/// A readers-only cell on [`SyncExpanseSet`]: `contains` on a reader handle.
fn run_set_readers_cell(
    wl: &ReadersOnlyWorkload<u64>,
    round: usize,
    is_counters: bool,
    perf_ctl: &mut PerfControl,
) -> Result<ReaderOutcome, String> {
    let set = SyncExpanseSet::new();
    for &k in &wl.prefill {
        set.insert(k);
    }
    let limit = Some(wl.prefill.len() as u64);
    let stop = AtomicBool::new(false);
    if is_counters {
        occ_stats::reset();
    }
    let (reader_elapsed_s, reader_ops, thread_elapsed_s) =
        timed_readers(wl.probes.len(), perf_ctl, |r, b| {
            let rd = set.reader();
            let keys = wl.prefill.as_slice();
            b.wait();
            let t0 = Instant::now();
            let n = probe_loop(&wl.probes[r], limit, &stop, |i| {
                u64::from(rd.contains(keys[i as usize]))
            });
            (n, t0.elapsed().as_secs_f64())
        });
    let counters = Counters::read(is_counters);
    let final_pop = set.len();
    if final_pop != wl.prefill.len() as u64 {
        return Err(format!(
            "round {round}: set population {final_pop}, expected {}",
            wl.prefill.len()
        ));
    }
    let reader = set.reader();
    let step = (wl.prefill.len() / 512).max(1);
    for &k in wl.prefill.iter().step_by(step) {
        if !reader.contains(k) {
            return Err(format!("round {round}: set missing prefilled key {k:#x}"));
        }
    }
    Ok(ReaderOutcome {
        writer_elapsed_s: None,
        reader_elapsed_s,
        thread_elapsed_s,
        reader_ops,
        fresh_keys: 0,
        final_pop,
        counters,
    })
}

/// A readers-only cell on [`SyncExpanseStrMap`]: `get` on a reader handle.
fn run_str_readers_cell(
    wl: &ReadersOnlyWorkload<Vec<u8>>,
    round: usize,
    is_counters: bool,
    perf_ctl: &mut PerfControl,
) -> Result<ReaderOutcome, String> {
    // Validated once, outside the timed window: the probe loop measures the
    // descent, not the NUL scan.
    let nks: Vec<&NulFreeStr> = wl
        .prefill
        .iter()
        .map(|k| NulFreeStr::new(k).expect("alnum bytes are NUL-free"))
        .collect();
    let map = SyncExpanseStrMap::new();
    for (nk, k) in nks.iter().zip(&wl.prefill) {
        map.insert(nk, str_value_of(k));
    }
    let limit = Some(wl.prefill.len() as u64);
    let stop = AtomicBool::new(false);
    if is_counters {
        occ_stats::reset();
    }
    let (reader_elapsed_s, reader_ops, thread_elapsed_s) =
        timed_readers(wl.probes.len(), perf_ctl, |r, b| {
            let rd = map.reader();
            let keys = nks.as_slice();
            b.wait();
            let t0 = Instant::now();
            let n = probe_loop(&wl.probes[r], limit, &stop, |i| {
                rd.get(keys[i as usize]).unwrap_or(0)
            });
            (n, t0.elapsed().as_secs_f64())
        });
    let counters = Counters::read(is_counters);
    let final_pop = map.len();
    if final_pop != wl.prefill.len() as u64 {
        return Err(format!(
            "round {round}: string map population {final_pop}, expected {}",
            wl.prefill.len()
        ));
    }
    let reader = map.reader();
    let step = (wl.prefill.len() / 512).max(1);
    for (nk, k) in nks.iter().zip(&wl.prefill).step_by(step) {
        if reader.get(nk) != Some(str_value_of(k)) {
            return Err(format!("round {round}: string map missing a prefilled key"));
        }
    }
    Ok(ReaderOutcome {
        writer_elapsed_s: None,
        reader_elapsed_s,
        thread_elapsed_s,
        reader_ops,
        fresh_keys: 0,
        final_pop,
        counters,
    })
}

/// Per-thread loop times as a JSON array.
fn f64_array_json(v: &[f64]) -> String {
    let items: Vec<String> = v.iter().map(|x| format!("{x:.6}")).collect();
    format!("[{}]", items.join(","))
}

/// `s` as a JSON string literal.
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The core pin the process inherited (`EXPANSE_BENCH_PIN_APPLIED`), or `null`.
fn cpu_pin_json() -> String {
    match std::env::var("EXPANSE_BENCH_PIN_APPLIED") {
        Ok(v) if !v.is_empty() => json_str(&v),
        _ => "null".to_string(),
    }
}

fn opt_f64_json(v: Option<f64>) -> String {
    v.map_or_else(|| "null".to_string(), |x| format!("{x:.6}"))
}

/// The value after `flag`: `Ok(None)` when the flag is absent, an error when
/// it is the last argument.
fn flag_value<'a>(args: &'a [String], flag: &str) -> Result<Option<&'a str>, String> {
    match args.iter().position(|a| a == flag) {
        None => Ok(None),
        Some(i) => args
            .get(i + 1)
            .map(|s| Some(s.as_str()))
            .ok_or_else(|| format!("{flag} needs a value")),
    }
}

fn parse_flag<T: std::str::FromStr>(args: &[String], flag: &str) -> Result<Option<T>, String> {
    flag_value(args, flag)?
        .map(|s| {
            s.parse()
                .map_err(|_| format!("{flag} takes a non-negative integer, got {s:?}"))
        })
        .transpose()
}

/// `--blob-op` and `--key-dist`, both optional. Either flag needs `--arm blob`
/// exactly and writer mode; `--key-dist` needs `--blob-op overwrite`, since a
/// fresh insert has no key to choose.
fn blob_flags(args: &[String], readers: usize) -> Result<(BlobOp, KeyDist), String> {
    let op_arg = flag_value(args, "--blob-op")?;
    let dist_arg = flag_value(args, "--key-dist")?;
    let op = op_arg.map(BlobOp::parse).transpose()?;
    let dist = dist_arg.map(KeyDist::parse).transpose()?;
    if op.is_some() || dist.is_some() {
        let arm = flag_value(args, "--arm")?.unwrap_or("all");
        if arm != "blob" || readers > 0 {
            return Err(format!(
                "--blob-op and --key-dist select the blob arm's writer cell: they need --arm blob \
                 and no --readers, got --arm {arm} and --readers {readers}"
            ));
        }
    }
    let op = op.unwrap_or(BlobOp::Insert);
    if op == BlobOp::Insert && dist.is_some() {
        return Err(
            "--key-dist chooses which existing key an overwrite targets: it needs --blob-op overwrite"
                .into(),
        );
    }
    Ok((op, dist.unwrap_or(KeyDist::Uniform)))
}

/// Reader mode: one (W, R, read op, probe) cell over `--round` or `--rounds`.
fn reader_main(args: &[String], is_counters: bool, readers: usize) -> Result<(), String> {
    match flag_value(args, "--arm")?.unwrap_or("map") {
        "map" => {}
        arm @ ("set" | "str") => return readers_only_main(args, is_counters, readers, arm),
        other => {
            return Err(format!(
                "--readers {readers} runs the map, set and str arms, got --arm {other}"
            ));
        }
    }
    let writers: usize = parse_flag(args, "--writers")?
        .ok_or_else(|| "reader mode needs --writers <W>, a single count (0 allowed)".to_string())?;
    let op = ReadOp::parse(flag_value(args, "--read-op")?.unwrap_or("prev"))?;
    let probe = Probe::parse(flag_value(args, "--probe")?.unwrap_or("uniform"))?;
    let position: usize = parse_flag(args, "--position")?.unwrap_or(0);
    let rounds: usize = parse_flag(args, "--rounds")?.unwrap_or(8);
    let (round_start, round_end) = match parse_flag::<usize>(args, "--round")? {
        Some(r) => (r, r + 1),
        None => (0, rounds),
    };
    let is_quick = args.iter().any(|a| a == "--quick");
    let (n0, m) = if is_quick {
        (4096, 4096)
    } else {
        (N_PREFILL, M_FRESH)
    };

    let tsc_hz = occ_stats::cycles_hz(std::time::Duration::from_millis(200));
    let mut perf_ctl = PerfControl::new(
        flag_value(args, "--perf-ctl-fifo")?,
        flag_value(args, "--perf-ack-fifo")?,
    );

    eprintln!(
        "generating reader workload (prefill={n0}, fresh={m}, probe={}, readers={readers})...",
        probe.name()
    );
    let wl = ReaderWorkload::generate(n0, m, probe, readers)?;
    let cell = ReaderCell {
        writers,
        readers,
        op,
        probe,
    };
    let label = format!("map_w{writers}_r{readers}_{}_{}", op.name(), probe.name());
    let pin = cpu_pin_json();
    let (op_name, probe_name) = (op.name(), probe.name());
    let (hotspot_prefill, hotspot_base) = (HOTSPOT_BUCKETS, wl.hotspot.base);

    for round in round_start..round_end {
        let out = run_reader_cell(&wl, cell, round, is_counters, &mut perf_ctl)?;
        let fresh = out.fresh_keys;
        let reader_ops = out.reader_ops;
        let final_pop = out.final_pop;
        if is_counters {
            let ctx = format!("{label} round {round}");
            out.counters.check(fresh, out.counters.locked_reads, &ctx)?;
            out.counters.check_reader(op, reader_ops, &ctx)?;
            println!(
                "{{\"workload_id\":\"concurrency_ordered_readers_map_64bit\",\"role\":\"counters\",\
                 \"arm\":\"expanse\",\"cell\":\"{label}\",\"keyspace_bits\":64,\
                 \"prefill\":{n0},\"hotspot_prefill\":{hotspot_prefill},\"hotspot_base\":{hotspot_base},\
                 \"fresh_keys\":{fresh},\"writers\":{writers},\"readers\":{readers},\
                 \"read_op\":\"{op_name}\",\"probe\":\"{probe_name}\",\
                 \"round\":{round},\"position\":{position},\"write_ops\":{fresh},\"reader_ops\":{reader_ops},\
                 \"cpu_pin\":{pin},\"tsc_hz\":{tsc_hz},\
                 \"lock_fallbacks\":{fb},\"inserts\":{ins},{extra},{reads},\
                 \"fallback_causes\":{causes},\"population_after\":{final_pop}}}",
                fb = out.counters.lock_fallbacks,
                ins = out.counters.inserts,
                extra = out.counters.extra_counters_json(),
                reads = out.counters.reader_counters_json(),
                causes = out.counters.causes_json(),
            );
        } else {
            let reader_elapsed_s = out.reader_elapsed_s;
            let reader_mops = reader_ops as f64 / reader_elapsed_s / 1e6;
            let writer_mops = out.writer_elapsed_s.map(|e| fresh as f64 / e / 1e6);
            println!(
                "{{\"workload_id\":\"concurrency_ordered_readers_map_64bit\",\"role\":\"throughput\",\
                 \"arm\":\"expanse\",\"cell\":\"{label}\",\"keyspace_bits\":64,\
                 \"prefill\":{n0},\"hotspot_prefill\":{hotspot_prefill},\"hotspot_base\":{hotspot_base},\
                 \"fresh_keys\":{fresh},\"writers\":{writers},\"readers\":{readers},\
                 \"read_op\":\"{op_name}\",\"probe\":\"{probe_name}\",\
                 \"round\":{round},\"position\":{position},\"write_ops\":{fresh},\
                 \"writer_elapsed_s\":{we},\"writer_mops\":{wm},\
                 \"reader_ops\":{reader_ops},\"reader_elapsed_s\":{reader_elapsed_s:.6},\
                 \"reader_thread_elapsed_s\":{te},\
                 \"reader_mops\":{reader_mops:.6},\"cpu_pin\":{pin},\"tsc_hz\":{tsc_hz},\
                 \"population_after\":{final_pop}}}",
                we = opt_f64_json(out.writer_elapsed_s),
                wm = opt_f64_json(writer_mops),
                te = f64_array_json(&out.thread_elapsed_s),
            );
        }
    }
    Ok(())
}

/// Readers-only mode on the set and string arms (#730): W = 0, `get`
/// (`contains` on the set), uniform probes over the prefill, one cell per
/// invocation. Anything else is refused by name rather than run as something
/// the row would not describe.
fn readers_only_main(
    args: &[String],
    is_counters: bool,
    readers: usize,
    arm: &str,
) -> Result<(), String> {
    let writers: usize = parse_flag(args, "--writers")?
        .ok_or_else(|| format!("--arm {arm} in reader mode needs --writers 0"))?;
    if writers != 0 {
        return Err(format!(
            "--arm {arm} in reader mode is the readers-only cell: --writers 0 only, got --writers {writers}"
        ));
    }
    let op = ReadOp::parse(flag_value(args, "--read-op")?.unwrap_or("get"))?;
    if op != ReadOp::Get {
        return Err(format!(
            "--arm {arm} in reader mode takes --read-op get only, got --read-op {}",
            op.name()
        ));
    }
    let probe = Probe::parse(flag_value(args, "--probe")?.unwrap_or("uniform"))?;
    if probe != Probe::Uniform {
        return Err(format!(
            "--arm {arm} in reader mode takes --probe uniform only, got --probe {}",
            probe.name()
        ));
    }
    let position: usize = parse_flag(args, "--position")?.unwrap_or(0);
    let rounds: usize = parse_flag(args, "--rounds")?.unwrap_or(8);
    let (round_start, round_end) = match parse_flag::<usize>(args, "--round")? {
        Some(r) => (r, r + 1),
        None => (0, rounds),
    };
    let n0 = if args.iter().any(|a| a == "--quick") {
        4096
    } else {
        N_PREFILL
    };

    let tsc_hz = occ_stats::cycles_hz(std::time::Duration::from_millis(200));
    let mut perf_ctl = PerfControl::new(
        flag_value(args, "--perf-ctl-fifo")?,
        flag_value(args, "--perf-ack-fifo")?,
    );
    let label = format!("{arm}_w0_r{readers}_get_uniform");
    let pin = cpu_pin_json();
    eprintln!("generating {arm} readers-only workload (prefill={n0}, readers={readers})...");

    let (workload_id, shape_field, outcomes) = if arm == "set" {
        let wl = ReadersOnlyWorkload::new(WriterWorkload::generate(n0, 0, 63).prefill, readers);
        let mut v = Vec::with_capacity(round_end - round_start);
        for round in round_start..round_end {
            v.push((
                round,
                run_set_readers_cell(&wl, round, is_counters, &mut perf_ctl)?,
            ));
        }
        ("concurrency_readers_set_63bit", "\"keyspace_bits\":63", v)
    } else {
        let wl = ReadersOnlyWorkload::new(WriterStrWorkload::generate(n0, 0).prefill, readers);
        let mut v = Vec::with_capacity(round_end - round_start);
        for round in round_start..round_end {
            v.push((
                round,
                run_str_readers_cell(&wl, round, is_counters, &mut perf_ctl)?,
            ));
        }
        ("concurrency_readers_str", "\"dist\":\"short\"", v)
    };

    for (round, out) in outcomes {
        let reader_ops = out.reader_ops;
        let final_pop = out.final_pop;
        if reader_ops != readers as u64 * n0 as u64 {
            return Err(format!(
                "{label} round {round}: {reader_ops} probes, expected exactly {}",
                readers as u64 * n0 as u64
            ));
        }
        if is_counters {
            let ctx = format!("{label} round {round}");
            out.counters.check(0, out.counters.locked_reads, &ctx)?;
            out.counters.check_reader(ReadOp::Get, reader_ops, &ctx)?;
            println!(
                "{{\"workload_id\":\"{workload_id}\",\"role\":\"counters\",\
                 \"arm\":\"expanse\",\"cell\":\"{label}\",{shape_field},\
                 \"prefill\":{n0},\"fresh_keys\":0,\"writers\":0,\"readers\":{readers},\
                 \"read_op\":\"get\",\"probe\":\"uniform\",\
                 \"round\":{round},\"position\":{position},\"write_ops\":0,\"reader_ops\":{reader_ops},\
                 \"cpu_pin\":{pin},\"tsc_hz\":{tsc_hz},\
                 \"lock_fallbacks\":{fb},\"inserts\":{ins},{extra},{reads},\
                 \"fallback_causes\":{causes},\"population_after\":{final_pop}}}",
                fb = out.counters.lock_fallbacks,
                ins = out.counters.inserts,
                extra = out.counters.extra_counters_json(),
                reads = out.counters.reader_counters_json(),
                causes = out.counters.causes_json(),
            );
        } else {
            let reader_elapsed_s = out.reader_elapsed_s;
            let reader_mops = reader_ops as f64 / reader_elapsed_s / 1e6;
            println!(
                "{{\"workload_id\":\"{workload_id}\",\"role\":\"throughput\",\
                 \"arm\":\"expanse\",\"cell\":\"{label}\",{shape_field},\
                 \"prefill\":{n0},\"fresh_keys\":0,\"writers\":0,\"readers\":{readers},\
                 \"read_op\":\"get\",\"probe\":\"uniform\",\
                 \"round\":{round},\"position\":{position},\"write_ops\":0,\
                 \"writer_elapsed_s\":null,\"writer_mops\":null,\
                 \"reader_ops\":{reader_ops},\"reader_elapsed_s\":{reader_elapsed_s:.6},\
                 \"reader_thread_elapsed_s\":{te},\
                 \"reader_mops\":{reader_mops:.6},\"cpu_pin\":{pin},\"tsc_hz\":{tsc_hz},\
                 \"population_after\":{final_pop}}}",
                te = f64_array_json(&out.thread_elapsed_s),
            );
        }
    }
    Ok(())
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
        fb_map.check(m as u64, 0, "self-test map")?;
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
        fb_set.check(m as u64, 0, "self-test set")?;
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
        fb_str.check(m as u64, 0, "self-test str")?;
    } else if el_str <= 0.0 {
        return Err(format!("throughput test: invalid str elapsed {el_str}"));
    }

    // The bytes and blob arms (#929 step 1) serialise every insert on the
    // writer mutex, as the str arm does, so they owe the same: every key
    // present, no lock fallback, no allocator count, the identities checked.
    let (el_bytes, pop_bytes, fb_bytes) =
        run_bytes_cell(&wl_str, 2, 0, is_counters, &mut dummy_ctl);
    let wl_blob = WriterWorkload::generate(n0, m, 64);
    let (el_blob, pop_blob, fb_blob) = run_blob_cell(&wl_blob, 2, 0, is_counters, &mut dummy_ctl);
    for (name, el, pop, fb) in [
        ("bytes", el_bytes, pop_bytes, fb_bytes),
        ("blob", el_blob, pop_blob, fb_blob),
    ] {
        if pop != (n0 + m) as u64 {
            return Err(format!("{name} expected pop {}, got {pop}", n0 + m));
        }
        if is_counters {
            if fb.lock_fallbacks != 0 || fb.total_allocs.is_some() {
                return Err(format!(
                    "counters test: {name} expected 0 lock fallbacks and no allocator count, got {} and {:?}",
                    fb.lock_fallbacks, fb.total_allocs
                ));
            }
            fb.check(m as u64, 0, &format!("self-test {name}"))?;
        } else if el <= 0.0 {
            return Err(format!("throughput test: invalid {name} elapsed {el}"));
        }
    }
    if blob_meta(0) == 0 || blob_payload(1) == blob_payload(2) {
        return Err("blob workload: metadata must be non-zero and payloads key-derived".into());
    }

    // The blob overwrite cell (METHODOLOGY §21.4 G4, §21.11): same prefill as
    // the fresh-insert cell, a rank table that is a permutation of it, streams
    // that are a function of (writer, round), a skew the uniform stream lacks,
    // and a read-back check that refuses what it must.
    let wl_ow = BlobOverwriteWorkload::generate(n0);
    if wl_ow.prefill != wl_blob.prefill {
        return Err("blob overwrite: prefill differs from the fresh-insert cell's".into());
    }
    let mut sorted_ranks = wl_ow.rank_keys.clone();
    sorted_ranks.sort_unstable();
    if sorted_ranks != wl_ow.prefill || wl_ow.rank_keys == wl_ow.prefill {
        return Err(
            "blob overwrite: rank table must be a non-identity permutation of the prefill".into(),
        );
    }
    for dist in [KeyDist::Uniform, KeyDist::Zipfian] {
        let a = wl_ow.stream(dist, 0, 0, m);
        if a != wl_ow.stream(dist, 0, 0, m)
            || a == wl_ow.stream(dist, 1, 0, m)
            || a == wl_ow.stream(dist, 0, 1, m)
        {
            return Err(format!(
                "blob overwrite: the {} stream must be a function of (writer, round) and differ across both",
                dist.name()
            ));
        }
        if a.iter().any(|&r| r as usize >= n0) {
            return Err(format!("blob overwrite: {} rank out of range", dist.name()));
        }
    }
    let rank0 = |dist| {
        wl_ow
            .stream(dist, 0, 0, m)
            .iter()
            .filter(|&&r| r == 0)
            .count()
    };
    // Under θ = 0.99 rank 0 draws 1/ζ(n0) of the stream, above 10% at this
    // n0; uniform choice gives it 1/n0. A factor of 20 separates them at any
    // seed without pinning a draw count.
    if rank0(KeyDist::Zipfian) < 20 * rank0(KeyDist::Uniform).max(1) {
        return Err(format!(
            "blob overwrite: rank 0 drew {} of {m} under zipfian and {} under uniform",
            rank0(KeyDist::Zipfian),
            rank0(KeyDist::Uniform)
        ));
    }
    for dist in [KeyDist::Uniform, KeyDist::Zipfian] {
        let ctx = format!("self-test blob overwrite {}", dist.name());
        let out = run_blob_overwrite_cell(&wl_ow, m, dist, 2, 0, is_counters, &mut dummy_ctl)
            .map_err(|e| format!("{ctx}: {e}"))?;
        if out.final_pop != n0 as u64 || out.overwrites != m as u64 {
            return Err(format!(
                "{ctx}: population {} after {} overwrites, expected {n0} after {m}",
                out.final_pop, out.overwrites
            ));
        }
        if out.distinct_keys == 0 || out.distinct_keys > n0 as u64 {
            return Err(format!("{ctx}: {} distinct keys", out.distinct_keys));
        }
        if is_counters {
            out.counters.check(m as u64, 0, &ctx)?;
        } else if out.elapsed_s <= 0.0 {
            return Err(format!("{ctx}: invalid elapsed {}", out.elapsed_s));
        }
    }
    // The read-back check, one refusal per clause, on a one-writer stream
    // whose first op targets rank 5.
    {
        let streams = vec![vec![5u32, 9]];
        let k = wl_ow.rank_keys[5];
        let c = overwrite_counter(0, 0);
        let good = Some((overwrite_payload(k, c).to_vec(), overwrite_meta(k, c)));
        let prefill = Some((blob_payload(k).to_vec(), blob_meta(k)));
        let foreign_c = overwrite_counter(0, 1); // an op that targeted rank 9
        let foreign = Some((
            overwrite_payload(k, foreign_c).to_vec(),
            overwrite_meta(k, foreign_c),
        ));
        let mut torn = overwrite_payload(k, c).to_vec();
        torn[16..].copy_from_slice(&blob_payload(k)[16..]);
        let wrong_meta = Some((overwrite_payload(k, c).to_vec(), blob_meta(k) ^ 2));
        let cases: [(&str, bool, BlobRead, Option<bool>); 7] = [
            ("overwritten", true, good.clone(), Some(true)),
            ("untouched", false, prefill.clone(), Some(false)),
            ("absent", true, None, None),
            ("targeted but prefill", true, prefill, None),
            ("untargeted but overwritten", false, good, None),
            ("foreign counter", true, foreign, None),
            (
                "torn payload",
                true,
                Some((torn, overwrite_meta(k, c))),
                None,
            ),
        ];
        for (name, targeted, got, want) in cases {
            let res = check_overwrite_readback(&wl_ow, &streams, 5, targeted, got).ok();
            if res != want {
                return Err(format!(
                    "blob overwrite read-back `{name}`: got {res:?}, expected {want:?}"
                ));
            }
        }
        if check_overwrite_readback(&wl_ow, &streams, 5, true, wrong_meta).is_ok() {
            return Err("blob overwrite read-back accepted a foreign metadata word".into());
        }
    }
    // Flag combinations that would run a different workload than the one named.
    let argv = |s: &str| -> Vec<String> { s.split_whitespace().map(String::from).collect() };
    for (line, readers, ok) in [
        ("--arm blob --blob-op overwrite --key-dist zipfian", 0, true),
        ("--arm blob --blob-op overwrite", 0, true),
        ("--arm blob", 0, true),
        ("--arm all", 0, true),
        ("--arm map --blob-op overwrite", 0, false),
        ("--arm all --blob-op insert", 0, false),
        ("--blob-op overwrite", 0, false),
        ("--arm blob --key-dist zipfian", 0, false),
        ("--arm blob --blob-op insert --key-dist uniform", 0, false),
        ("--arm blob --blob-op overwrite --key-dist skewed", 0, false),
        ("--arm blob --blob-op replace", 0, false),
        ("--arm blob --blob-op overwrite", 2, false),
        ("--arm blob --blob-op", 0, false),
    ] {
        if blob_flags(&argv(line), readers).is_ok() != ok {
            return Err(format!(
                "blob flags `{line}` (readers {readers}): expected {}",
                if ok { "accepted" } else { "refused" }
            ));
        }
    }

    // Reader mode (#900). First the hotspot geometry the §12.4 cells rest on.
    let hot = HotspotExpanse::generate();
    if hot.base & (HOTSPOT_WIDTH - 1) != 0 {
        return Err(format!("hotspot base {:#x} has low bits set", hot.base));
    }
    if hot.prefill.len() as u64 != HOTSPOT_BUCKETS
        || hot.fresh.len() as u64 != HOTSPOT_WIDTH - HOTSPOT_BUCKETS
    {
        return Err(format!(
            "hotspot carries {} prefill and {} fresh keys, expected {HOTSPOT_BUCKETS} and {}",
            hot.prefill.len(),
            hot.fresh.len(),
            HOTSPOT_WIDTH - HOTSPOT_BUCKETS
        ));
    }
    let mut tiles: Vec<u64> = hot.prefill.iter().chain(&hot.fresh).copied().collect();
    tiles.sort_unstable();
    tiles.dedup();
    if tiles.len() as u64 != HOTSPOT_WIDTH
        || tiles.first() != Some(&hot.base)
        || tiles.last() != Some(&(hot.base + HOTSPOT_WIDTH - 1))
    {
        return Err("hotspot prefill and fresh keys must tile the expanse exactly".into());
    }
    // On the prefilled tree, every hotspot probe's search fails in one terminal
    // byte and descends the sibling below it. The relation that makes that a
    // branch rather than one leaf is the compile-time assertion beside
    // HOTSPOT_PREFILL_OFFSET.
    hot.check_backtrack_geometry()
        .map_err(|e| format!("hotspot geometry: {e}"))?;

    // The reader identities must reject a wrong count, not only accept a right one.
    let probe_counters = Counters {
        read_ops: 10,
        read_fallbacks: 1,
        locked_reads: 1,
        ..Counters::default()
    };
    if probe_counters
        .check_reader(ReadOp::Prev, 10, "probe")
        .is_err()
        || probe_counters
            .check_reader(ReadOp::Prev, 11, "probe")
            .is_ok()
        || probe_counters
            .check_reader(ReadOp::PrevLocked, 1, "probe")
            .is_ok()
    {
        return Err("check_reader accepted a mismatched count or refused a matching one".into());
    }

    // One cell per read op and probe, at W = 0 and W = 1, in this build's role.
    for op in [ReadOp::Prev, ReadOp::PrevLocked, ReadOp::Get] {
        for probe in [Probe::Uniform, Probe::Hotspot] {
            let wl = ReaderWorkload::generate(n0, m, probe, 2)?;
            for writers in [0usize, 1] {
                let ctx = format!(
                    "self-test reader {} {} W={writers}",
                    op.name(),
                    probe.name()
                );
                let cell = ReaderCell {
                    writers,
                    readers: 2,
                    op,
                    probe,
                };
                let out = run_reader_cell(&wl, cell, 0, is_counters, &mut dummy_ctl)
                    .map_err(|e| format!("{ctx}: {e}"))?;
                let expected_fresh = if writers == 0 {
                    0
                } else {
                    wl.fresh(probe).len() as u64
                };
                if out.fresh_keys != expected_fresh {
                    return Err(format!(
                        "{ctx}: {} fresh keys, expected {expected_fresh}",
                        out.fresh_keys
                    ));
                }
                if writers == 0 && out.reader_ops != 2 * n0 as u64 {
                    return Err(format!(
                        "{ctx}: {} probes at W = 0, expected exactly {}",
                        out.reader_ops,
                        2 * n0
                    ));
                }
                if is_counters {
                    out.counters
                        .check(expected_fresh, out.counters.locked_reads, &ctx)?;
                    out.counters.check_reader(op, out.reader_ops, &ctx)?;
                } else if out.reader_elapsed_s <= 0.0
                    || (writers > 0) != out.writer_elapsed_s.is_some()
                {
                    return Err(format!(
                        "{ctx}: invalid elapsed (readers {}, writers {:?})",
                        out.reader_elapsed_s, out.writer_elapsed_s
                    ));
                }
            }
        }
    }

    // Readers-only cells on the set and string arms (#730): exactly one probe
    // per prefilled key per reader, one loop time per reader thread, and the
    // counters build's identities at W = 0.
    let wl_set_ro = ReadersOnlyWorkload::new(WriterWorkload::generate(n0, 0, 63).prefill, 2);
    let wl_str_ro = ReadersOnlyWorkload::new(WriterStrWorkload::generate(n0, 0).prefill, 2);
    let set_ro = run_set_readers_cell(&wl_set_ro, 0, is_counters, &mut dummy_ctl)
        .map_err(|e| format!("self-test set readers-only: {e}"))?;
    let str_ro = run_str_readers_cell(&wl_str_ro, 0, is_counters, &mut dummy_ctl)
        .map_err(|e| format!("self-test str readers-only: {e}"))?;
    for (name, out) in [("set", &set_ro), ("str", &str_ro)] {
        let ctx = format!("self-test {name} readers-only");
        if out.reader_ops != 2 * n0 as u64 || out.final_pop != n0 as u64 {
            return Err(format!(
                "{ctx}: {} probes over a population of {}, expected {} over {n0}",
                out.reader_ops,
                out.final_pop,
                2 * n0
            ));
        }
        if out.thread_elapsed_s.len() != 2 {
            return Err(format!(
                "{ctx}: {} per-thread loop times for 2 readers",
                out.thread_elapsed_s.len()
            ));
        }
        if is_counters {
            out.counters.check(0, out.counters.locked_reads, &ctx)?;
            out.counters
                .check_reader(ReadOp::Get, out.reader_ops, &ctx)?;
        } else if out.reader_elapsed_s <= 0.0 || out.thread_elapsed_s.iter().any(|&t| t <= 0.0) {
            return Err(format!(
                "{ctx}: invalid elapsed (readers {}, threads {:?})",
                out.reader_elapsed_s, out.thread_elapsed_s
            ));
        }
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

    // Reader mode (#900) is a separate row family; without `--readers` (or
    // with `--readers 0`) everything below is the writer sweep, unchanged.
    let readers = match parse_flag::<usize>(&args, "--readers") {
        Ok(r) => r.unwrap_or(0),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    // The blob arm's op and key choice (METHODOLOGY §21.4 G4, §21.11). Refused
    // anywhere they would be ignored: a cell that silently ran fresh inserts
    // under `--blob-op overwrite` would publish the wrong workload (AGENTS.md §8.1).
    let (blob_op, key_dist) = match blob_flags(&args, readers) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    if readers > 0 {
        if let Err(e) = reader_main(&args, is_counters, readers) {
            eprintln!("reader mode: {e}");
            std::process::exit(1);
        }
        return;
    }
    for flag in ["--read-op", "--probe"] {
        if args.iter().any(|a| a == flag) {
            eprintln!("{flag} is a reader-mode flag and needs --readers > 0");
            std::process::exit(1);
        }
    }

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

    // Parsed strictly: a per-cell invocation that silently ignored a malformed
    // `--round` would run every round in one process, the carryover this
    // form exists to remove (AGENTS.md §8.1).
    let round_opt: Option<usize> = match parse_flag(&args, "--round") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let position_opt: Option<usize> = match parse_flag(&args, "--position") {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

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

    if writers_list.contains(&0) {
        eprintln!("--writers 0 is accepted only in reader mode (--readers > 0)");
        std::process::exit(1);
    }

    let n_w = writers_list.len();
    if position_opt.is_some() && (round_opt.is_none() || n_w != 1) {
        eprintln!(
            "--position names one cell's place in its round: it needs --round <r> and a single \
             --writers <W>, got --writers {writers_arg} and --round {round_opt:?}"
        );
        std::process::exit(1);
    }
    // A single-cell invocation cannot be balanced on its own; the driver that
    // schedules the cells owns the Williams balance, so no notice here.
    if position_opt.is_none() && (!n_w.is_multiple_of(2) || !rounds.is_multiple_of(n_w)) {
        eprintln!(
            "notice: Williams square balance requires even writer count and rounds multiple of len(writers); \
             got len(writers)={n_w}, rounds={rounds} — position/carryover balance will be incomplete"
        );
    }

    let run_map = arm_arg == "map" || arm_arg == "all" || arm_arg == "both";
    let run_set = arm_arg == "set" || arm_arg == "all" || arm_arg == "both";
    let run_str = arm_arg == "str" || arm_arg == "all";
    let run_bytes = arm_arg == "bytes" || arm_arg == "all";
    let run_blob = arm_arg == "blob" || arm_arg == "all";

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
                let pos = position_opt.unwrap_or(pos);
                let (elapsed_s, final_pop, counters) =
                    run_map_cell(&wl, w, round, is_counters, &mut perf_ctl);
                let write_ops = m;
                if is_counters {
                    if let Err(e) =
                        counters.check(m as u64, 0, &format!("map_w{w}_r0 round {round}"))
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
                let pos = position_opt.unwrap_or(pos);
                let (elapsed_s, final_pop, counters) =
                    run_set_cell(&wl, w, round, is_counters, &mut perf_ctl);
                let write_ops = m;
                if is_counters {
                    if let Err(e) =
                        counters.check(m as u64, 0, &format!("set_w{w}_r0 round {round}"))
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
                let pos = position_opt.unwrap_or(pos);
                let (elapsed_s, final_pop, counters) =
                    run_str_cell(&wl, w, round, is_counters, &mut perf_ctl);
                let write_ops = m;
                if is_counters {
                    if let Err(e) =
                        counters.check(m as u64, 0, &format!("str_w{w}_r0 round {round}"))
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

    if run_bytes {
        eprintln!("generating bytes workload (prefill={n0}, fresh={m})...");
        let wl = WriterStrWorkload::generate(n0, m);
        for round in round_start..round_end {
            let round_writers = williams_order(&writers_list, round);
            for (pos, &w) in round_writers.iter().enumerate() {
                let pos = position_opt.unwrap_or(pos);
                let (elapsed_s, final_pop, counters) =
                    run_bytes_cell(&wl, w, round, is_counters, &mut perf_ctl);
                let write_ops = m;
                if is_counters {
                    if let Err(e) =
                        counters.check(m as u64, 0, &format!("bytes_w{w}_r0 round {round}"))
                    {
                        eprintln!("counter identity violated: {e}");
                        std::process::exit(1);
                    }
                    println!(
                        "{{\"workload_id\":\"concurrency_writer_bytes\",\"role\":\"counters\",\
                         \"arm\":\"expanse\",\"cell\":\"bytes_w{w}_r0\",\"dist\":\"short\",\
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
                        "{{\"workload_id\":\"concurrency_writer_bytes\",\"role\":\"throughput\",\
                         \"arm\":\"expanse\",\"cell\":\"bytes_w{w}_r0\",\"dist\":\"short\",\
                         \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{w},\"readers\":0,\
                         \"round\":{round},\"position\":{pos},\"write_ops\":{write_ops},\"writer_elapsed_s\":{elapsed_s:.6},\
                         \"writer_mops\":{writer_mops:.4},\"tsc_hz\":{tsc_hz},\"population_after\":{final_pop}}}"
                    );
                }
            }
        }
    }

    if run_blob && blob_op == BlobOp::Overwrite {
        eprintln!(
            "generating blob 64-bit overwrite workload (prefill={n0}, overwrites={m}, key_dist={})...",
            key_dist.name()
        );
        let wl = BlobOverwriteWorkload::generate(n0);
        let bits = wl.keyspace_bits;
        let dist = key_dist.name();
        let theta = key_dist.theta();
        for round in round_start..round_end {
            let round_writers = williams_order(&writers_list, round);
            for (pos, &w) in round_writers.iter().enumerate() {
                let pos = position_opt.unwrap_or(pos);
                let cell = format!("blob_overwrite_{dist}_w{w}_r0");
                let out = match run_blob_overwrite_cell(
                    &wl,
                    m,
                    key_dist,
                    w,
                    round,
                    is_counters,
                    &mut perf_ctl,
                ) {
                    Ok(out) => out,
                    Err(e) => {
                        eprintln!("{cell}: {e}");
                        std::process::exit(1);
                    }
                };
                let write_ops = out.overwrites;
                // Shared by both roles: the insert cell's row shape, plus what
                // names this workload (METHODOLOGY §21.11).
                let head = format!(
                    "\"arm\":\"expanse\",\"cell\":\"{cell}\",\"keyspace_bits\":{bits},\
                     \"payload_bytes\":{BLOB_PAYLOAD_LEN},\"blob_op\":\"overwrite\",\
                     \"key_dist\":\"{dist}\",\"theta\":{theta},\
                     \"prefill\":{n0},\"fresh_keys\":0,\"overwrites\":{write_ops},\
                     \"distinct_keys_overwritten\":{dk},\"verified_keys\":{vk},\
                     \"verified_overwritten\":{vo},\"writers\":{w},\"readers\":0,\
                     \"round\":{round},\"position\":{pos},\"write_ops\":{write_ops}",
                    dk = out.distinct_keys,
                    vk = out.verified_keys,
                    vo = out.verified_overwritten,
                );
                let final_pop = out.final_pop;
                if is_counters {
                    let counters = out.counters;
                    if let Err(e) = counters.check(write_ops, 0, &format!("{cell} round {round}")) {
                        eprintln!("counter identity violated: {e}");
                        std::process::exit(1);
                    }
                    println!(
                        "{{\"workload_id\":\"concurrency_writer_blob_overwrite_64bit\",\"role\":\"counters\",\
                         {head},\"tsc_hz\":{tsc_hz},\
                         \"lock_fallbacks\":{fb},\"inserts\":{ins},{extra},\"fallback_causes\":{causes},\"population_after\":{final_pop}}}",
                        fb = counters.lock_fallbacks,
                        ins = counters.inserts,
                        extra = counters.extra_counters_json(),
                        causes = counters.causes_json(),
                    );
                } else {
                    let elapsed_s = out.elapsed_s;
                    let writer_mops = (write_ops as f64) / elapsed_s / 1e6;
                    println!(
                        "{{\"workload_id\":\"concurrency_writer_blob_overwrite_64bit\",\"role\":\"throughput\",\
                         {head},\"writer_elapsed_s\":{elapsed_s:.6},\
                         \"writer_mops\":{writer_mops:.4},\"tsc_hz\":{tsc_hz},\"population_after\":{final_pop}}}"
                    );
                }
            }
        }
    } else if run_blob {
        eprintln!("generating blob 64-bit workload (prefill={n0}, fresh={m})...");
        let wl = WriterWorkload::generate(n0, m, 64);
        let bits = wl.keyspace_bits;
        for round in round_start..round_end {
            let round_writers = williams_order(&writers_list, round);
            for (pos, &w) in round_writers.iter().enumerate() {
                let pos = position_opt.unwrap_or(pos);
                let (elapsed_s, final_pop, counters) =
                    run_blob_cell(&wl, w, round, is_counters, &mut perf_ctl);
                let write_ops = m;
                if is_counters {
                    if let Err(e) =
                        counters.check(m as u64, 0, &format!("blob_w{w}_r0 round {round}"))
                    {
                        eprintln!("counter identity violated: {e}");
                        std::process::exit(1);
                    }
                    println!(
                        "{{\"workload_id\":\"concurrency_writer_blob_64bit\",\"role\":\"counters\",\
                         \"arm\":\"expanse\",\"cell\":\"blob_w{w}_r0\",\"keyspace_bits\":{bits},\
                         \"payload_bytes\":{BLOB_PAYLOAD_LEN},\
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
                        "{{\"workload_id\":\"concurrency_writer_blob_64bit\",\"role\":\"throughput\",\
                         \"arm\":\"expanse\",\"cell\":\"blob_w{w}_r0\",\"keyspace_bits\":{bits},\
                         \"payload_bytes\":{BLOB_PAYLOAD_LEN},\
                         \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{w},\"readers\":0,\
                         \"round\":{round},\"position\":{pos},\"write_ops\":{write_ops},\"writer_elapsed_s\":{elapsed_s:.6},\
                         \"writer_mops\":{writer_mops:.4},\"tsc_hz\":{tsc_hz},\"population_after\":{final_pop}}}"
                    );
                }
            }
        }
    }
}
