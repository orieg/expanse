//! Optional **counting** instrumentation for the OCC read/write protocol.
//!
//! Compiled out entirely without the `occ-stats` feature: every [`bump`]
//! is an empty `#[inline(always)]` call and no static exists, so the
//! classic engine's instruction counts are untouched (AGENTS.md §6
//! zero-regression).
//!
//! With the feature on, it counts **events, not time** — optimistic walk
//! attempts, writer-lock fallbacks, epoch-advance outcomes. Event ratios
//! (retries per read, fallbacks per read) are immune to host load, which
//! makes them the honest instrument for diagnosing reader starvation on
//! a machine that is not a quiet benchmark host: a wall-clock ratio taken
//! under contention is not a publishable measurement (AGENTS.md §8), but
//! "N% of reads took the writer mutex" is a fact about the protocol
//! regardless of what else the machine was doing.
//!
//! The one exception is [`Stat::SampleSpinCycles`], which accumulates the
//! host's cycle counter across every spin in `SeqVersion::sample`. A spin
//! *count* cannot say how long a reader waited on an open bracket; the
//! cycle total can, once the counter's rate is known ([`cycles_hz`]).
//! It is a duration, so it is host-dependent like any other timing, and
//! it is published only as a share of the reader's own elapsed time.
//!
//! ## Sharded per thread
//!
//! The counters live in per-thread shards (one padded slot per thread,
//! summed by [`snapshot`]) rather than one process-global line. With one
//! global line, nine threads bumping on every lookup made the counter
//! its own contention source: the same health cell read 0.89 and 0.58
//! spins per lookup on two identical runs. Under `no_std` there is no
//! thread-local storage, so every thread shares shard 0 — the counts are
//! still exact, only the contention returns.
//!
//! The two gauges ([`Stat::RetainedGarbageBytes`],
//! [`Stat::RetainedGarbageHwm`]) stay in one global slot: a high-water
//! mark of a sum is not the sum of per-shard high-water marks.
//!
//! Run the probe with
//! `cargo run --release -p expanse-trie --features occ-stats --example occ_stats_probe`.

/// A counted event in the OCC protocol.
///
/// `#[non_exhaustive]`: counters are added as diagnostics need them
/// (AGENTS.md §2.3 semver protection); index [`snapshot`] by
/// `Stat::X as usize` rather than matching on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
#[non_exhaustive]
pub enum Stat {
    /// Public optimistic read calls entered.
    ReadOps = 0,
    /// Optimistic walk attempts (≥ 1 per read op; > 1 is a restart).
    ReadAttempts = 1,
    /// Read ops that exhausted the retry budget and fell back to
    /// `read_locked` — i.e. took the *writer* mutex.
    ReadFallbacks = 2,
    /// Mutations run through `Shared::write`.
    WriteOps = 3,
    /// `Collector::try_advance` calls.
    AdvanceCalls = 4,
    /// `try_advance` calls that actually advanced the epoch.
    AdvanceOk = 5,
    /// Spin iterations burnt in `SeqVersion::sample` waiting for a
    /// writer's tree-level bracket to close.
    SampleSpins = 6,
    /// Bytes currently held in collector garbage bins (unreclaimed backlog).
    RetainedGarbageBytes = 7,
    /// Peak bytes held in collector garbage bins (high-water mark).
    RetainedGarbageHwm = 8,
    /// Read calls that took the writer mutex, by **any** route —
    /// retry-exhaustion fallbacks *and* the unconditional `with_locked` /
    /// `len` / `mem_used` paths that never attempt an optimistic walk.
    /// Counted inside `read_locked` itself, the one chokepoint every such
    /// route passes through, so it cannot drift as callers are added.
    ///
    /// `locked_reads - read_fallbacks` is the unconditional share.
    LockedReads = 9,
    /// Writer-lock acquisitions by a thread other than the previous holder
    /// (a real handoff). Consecutive acquisitions by the same thread — a
    /// releaser barging back in ahead of a woken waiter — are not handoffs,
    /// which is why a per-op cost model needs this count and not `write_ops`.
    /// Counted in the 64-bit `sync` wrappers, which need `std`; the
    /// single-writer `sync32` protocol has no writer lock to hand over.
    Handoffs = 10,
    /// Blocks handed to the collector for deferred reclamation.
    Retired = 11,
    /// Blocks actually freed by the collector (after their epoch passed).
    FreedRaw = 12,
    /// Cycle-counter ticks spent inside `SeqVersion::sample` from the first
    /// odd observation to the even return — the *time* behind
    /// [`Stat::SampleSpins`]. Ticks of [`cycles_now`]; divide by
    /// [`cycles_hz`] for seconds. Zero on targets without a cycle counter.
    SampleSpinCycles = 13,
    /// Branch nodes replaced or removed within a mutation — every
    /// `upgrade_*` / `downgrade_*` / `free_branch_node` / `wrap_skip_level`.
    /// Each one rewrites the slot in the node's *parent*, which under
    /// per-node write locks means the parent is in the lock set (#568).
    BranchReplacements = 14,
    /// Mutations in which two or more branch nodes were replaced — the
    /// cascade reached above the terminal's grandparent, so a lock set of
    /// "parent, or parent + grandparent" would not have covered it (#568).
    DeepCascades = 15,
    /// Mutations that changed the owner's root state: a `Root` variant
    /// change, a root-leaf reallocation, or a rewrite of the top edge.
    /// Each one is a write the tree-level bracket must cover (#568).
    RootRewrites = 16,
    /// Lock restarts during multi-writer OLC mutations (#568).
    LockRestarts = 17,
    /// Spins waiting to acquire per-node version locks (#568).
    LockSpins = 18,
    /// Cycle-counter ticks spent holding per-node version locks (#568).
    LockHoldCycles = 19,
    /// Mutations falling back to the serialized root-covered lock (#568).
    LockFallbacks = 20,
    /// Logical insert operations attempted — one per public `insert` call,
    /// counted once whether the operation completes on the OLC path or
    /// falls back. [`Stat::WriteOps`] cannot serve this role: it is bumped
    /// on the OLC attempt *and* again inside `write_root_covered` when the
    /// fallback is taken, so `write_ops = inserts + lock_fallbacks` and any
    /// per-insert rate computed against it is understated. `WriteOps` keeps
    /// its meaning — it is the published denominator of `handoffs_per_write`,
    /// `replacements_per_write`, `deep_cascade_share` and
    /// `root_rewrite_share`, and rebasing it would make every committed
    /// artifact non-comparable (AGENTS.md §8.18). Divide by this instead
    /// for a per-insert share (#568).
    Inserts = 21,
    /// Fallbacks taken because a linear leaf crossed a [`crate::leaf::cap_class`]
    /// boundary and had to be reallocated (#568).
    FallbackCapExpansion = 22,
    /// Fallbacks taken for an immediate-slot transition: an immediate slot
    /// running out of packed capacity to become a heap leaf, or an empty
    /// branch slot populating as an immediate (#568).
    FallbackImmediateConversion = 23,
    /// Fallbacks taken for a branch structural mutation — linear branch
    /// expansion, bitmap allocation, or a prefix split (#568).
    FallbackBranchSplit = 24,
    /// Fallbacks taken for a root-state transition — empty root to leaf, or
    /// root leaf to root branch (#568).
    FallbackRootGrowth = 25,
    /// Fallbacks taken for *contention*, not structure: the retry budget was
    /// exhausted, or [`crate::occ::WriterGate`] closed under a quiescing
    /// peer. These are the fallbacks a leaf-sizing or node-layout change
    /// cannot remove, so keeping them in their own bucket is what makes the
    /// four structural shares above falsifiable rather than residual (#568).
    FallbackContention = 26,
    /// Fallbacks taken because the descent met an edge tag the OLC path does
    /// not decode. Expected to be ~0; a non-zero share means the OLC walk has
    /// a hole and the structural shares are measured against the wrong
    /// denominator (#568).
    FallbackUnknownTag = 27,
    /// Ancestor `pop0` re-descents entered — a bump met an obsolete ancestor
    /// and restarted from the root per `ARCHITECTURE.md` §4.2 rule (a). Rare
    /// while structural mutation still quiesces; the rate is what says whether
    /// that stays true once it does not (#568).
    PopRedescends = 28,
    /// Re-descents that exhausted their attempt budget without applying the
    /// delta. **Every one is a branch `pop0` left drifted from its subtree**,
    /// detectable only by `validate()`. Expected 0; a non-zero value is a
    /// correctness signal, not a performance one (#568).
    PopRedescendAbandoned = 29,
    /// Retrying writers in OLC mutation loop aborting because `WriterGate` closed (#568).
    ContentionGateClosed = 30,
    /// Retrying writers in OLC mutation loop exhausting `MAX_RETRIES` (#568).
    ContentionRetryExhausted = 31,
    /// Arriving writers finding `WriterGate` closed in `enter_writer_blocking` (#568).
    GateBlockedEntries = 32,
    /// Cycle-counter ticks spent spinning in `enter_writer_blocking` while gate is closed (#568).
    GateWaitCycles = 33,
    /// Invocations of `quiesce_writers` during serialized fallback (#568).
    QuiesceCalls = 34,
    /// Cycle-counter ticks spent in `quiesce_writers` waiting for in-flight writers to drain (#568).
    QuiesceDrainCycles = 35,
    /// Branch structural mutations due to bitmap subarray missing or null child (Phase 4D, #568).
    BranchSplitSubarray = 36,
    /// Branch structural mutations due to linear branch overflow (Phase 4E, #568).
    BranchSplitLinear = 37,
    /// Branch structural mutations due to edge prefix mismatch (Phase 4E, #568).
    BranchSplitPrefix = 38,
    /// Branch structural mutations on remove paths (shrink/condense, #568).
    BranchSplitRemove = 39,
}

/// Number of distinct counters.
pub const NUM_STATS: usize = 40;

/// Human-readable counter names, indexed by [`Stat`].
pub const NAMES: [&str; NUM_STATS] = [
    "read_ops",
    "read_attempts",
    "read_fallbacks",
    "write_ops",
    "advance_calls",
    "advance_ok",
    "sample_spins",
    "retained_bytes",
    "retained_hwm",
    "locked_reads",
    "handoffs",
    "retired",
    "freed_raw",
    "sample_spin_cycles",
    "branch_replacements",
    "deep_cascades",
    "root_rewrites",
    "lock_restarts",
    "lock_spins",
    "lock_hold_cycles",
    "lock_fallbacks",
    "inserts",
    "fallback_cap_expansion",
    "fallback_immediate_conversion",
    "fallback_branch_split",
    "fallback_root_growth",
    "fallback_contention",
    "fallback_unknown_tag",
    "pop_redescends",
    "pop_redescend_abandoned",
    "contention_gate_closed",
    "contention_retry_exhausted",
    "gate_blocked_entries",
    "gate_wait_cycles",
    "quiesce_calls",
    "quiesce_drain_cycles",
    "branch_split_subarray",
    "branch_split_linear",
    "branch_split_prefix",
    "branch_split_remove",
];

/// Counters that are gauges (add / subtract / high-water), kept global.
#[cfg(feature = "occ-stats")]
const fn is_gauge(i: usize) -> bool {
    i == Stat::RetainedGarbageBytes as usize || i == Stat::RetainedGarbageHwm as usize
}

#[cfg(feature = "occ-stats")]
mod cells {
    use super::NUM_STATS;
    use core::sync::atomic::{AtomicU64, AtomicUsize};

    /// Per-thread shards. 64 covers every thread count the concurrent
    /// harnesses run (W + R ≤ 16 under the P-core pin); beyond that two
    /// threads share a shard, which costs contention, never correctness.
    pub const SHARDS: usize = 64;

    /// One thread's counters on its own cache lines.
    #[repr(align(128))]
    pub struct Shard(pub [AtomicU64; NUM_STATS]);

    pub static SHARDS_ARR: [Shard; SHARDS] =
        [const { Shard([const { AtomicU64::new(0) }; NUM_STATS]) }; SHARDS];

    /// The gauges' one global slot (indices where `is_gauge` holds).
    pub static GLOBAL: [AtomicU64; NUM_STATS] = [const { AtomicU64::new(0) }; NUM_STATS];

    static NEXT_SLOT: AtomicUsize = AtomicUsize::new(0);

    #[cfg(feature = "std")]
    std::thread_local! {
        static SLOT: core::cell::Cell<usize> = const { core::cell::Cell::new(usize::MAX) };
        /// Branch replacements seen by the current mutation (see `op_begin`).
        pub static OP_REPLACEMENTS: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
    }

    /// This thread's shard. Assigned on first use, round-robin.
    #[inline(always)]
    #[cfg(feature = "std")]
    pub fn shard() -> &'static Shard {
        let i = SLOT.with(|c| {
            let mut i = c.get();
            if i == usize::MAX {
                i = NEXT_SLOT.fetch_add(1, core::sync::atomic::Ordering::Relaxed) % SHARDS;
                c.set(i);
            }
            i
        });
        &SHARDS_ARR[i]
    }

    /// No thread-local storage without `std`: every thread shares shard 0.
    #[inline(always)]
    #[cfg(not(feature = "std"))]
    pub fn shard() -> &'static Shard {
        let _ = &NEXT_SLOT;
        &SHARDS_ARR[0]
    }
}

/// Records one occurrence of `s`. A no-op without `occ-stats`.
#[inline(always)]
pub fn bump(s: Stat) {
    let _ = s;
    #[cfg(feature = "occ-stats")]
    cells::shard().0[s as usize].fetch_add(1, core::sync::atomic::Ordering::Relaxed);
}

/// Records `n` occurrences of `s`. A no-op without `occ-stats`.
#[inline(always)]
pub fn bump_by(s: Stat, n: u64) {
    let _ = (s, n);
    #[cfg(feature = "occ-stats")]
    cells::shard().0[s as usize].fetch_add(n, core::sync::atomic::Ordering::Relaxed);
}

/// Records retired garbage bytes and updates the high-water mark. A no-op without `occ-stats`.
#[inline(always)]
pub fn record_retire(bytes: usize) {
    let _ = bytes;
    #[cfg(feature = "occ-stats")]
    {
        let prev = cells::GLOBAL[Stat::RetainedGarbageBytes as usize]
            .fetch_add(bytes as u64, core::sync::atomic::Ordering::Relaxed);
        let cur = prev.saturating_add(bytes as u64);
        let _ = cells::GLOBAL[Stat::RetainedGarbageHwm as usize]
            .fetch_max(cur, core::sync::atomic::Ordering::Relaxed);
    }
}

/// Records reclaimed garbage bytes. A no-op without `occ-stats`.
#[inline(always)]
pub fn record_reclaim(bytes: usize) {
    let _ = bytes;
    #[cfg(feature = "occ-stats")]
    {
        let _ = cells::GLOBAL[Stat::RetainedGarbageBytes as usize].fetch_update(
            core::sync::atomic::Ordering::Relaxed,
            core::sync::atomic::Ordering::Relaxed,
            |val| Some(val.saturating_sub(bytes as u64)),
        );
    }
}

/// Returns the current retained garbage bytes across all collectors. 0 without `occ-stats`.
#[must_use]
#[inline(always)]
pub fn retained_bytes() -> u64 {
    #[cfg(feature = "occ-stats")]
    {
        cells::GLOBAL[Stat::RetainedGarbageBytes as usize]
            .load(core::sync::atomic::Ordering::Relaxed)
    }
    #[cfg(not(feature = "occ-stats"))]
    {
        0
    }
}

/// Returns the peak retained garbage bytes (high-water mark) recorded. 0 without `occ-stats`.
#[must_use]
#[inline(always)]
pub fn retained_hwm() -> u64 {
    #[cfg(feature = "occ-stats")]
    {
        cells::GLOBAL[Stat::RetainedGarbageHwm as usize].load(core::sync::atomic::Ordering::Relaxed)
    }
    #[cfg(not(feature = "occ-stats"))]
    {
        0
    }
}

/// Reads every counter, summed across thread shards. All zero without `occ-stats`.
#[must_use]
pub fn snapshot() -> [u64; NUM_STATS] {
    #[cfg(feature = "occ-stats")]
    {
        use core::sync::atomic::Ordering::Relaxed;
        core::array::from_fn(|i| {
            if is_gauge(i) {
                cells::GLOBAL[i].load(Relaxed)
            } else {
                cells::SHARDS_ARR
                    .iter()
                    .map(|s| s.0[i].load(Relaxed))
                    .fold(0u64, u64::wrapping_add)
            }
        })
    }
    #[cfg(not(feature = "occ-stats"))]
    {
        [0; NUM_STATS]
    }
}

/// Zeroes every counter (call between measurement phases).
pub fn reset() {
    #[cfg(feature = "occ-stats")]
    {
        use core::sync::atomic::Ordering::Relaxed;
        for s in &cells::SHARDS_ARR {
            for c in &s.0 {
                c.store(0, Relaxed);
            }
        }
        for c in &cells::GLOBAL {
            c.store(0, Relaxed);
        }
    }
}

/// True when the counters are live (the `occ-stats` feature is on).
#[must_use]
pub const fn enabled() -> bool {
    cfg!(feature = "occ-stats")
}

// ---- cycle counter ------------------------------------------------------

/// The host's cycle counter: `rdtsc` on x86-64, `cntvct_el0` on AArch64,
/// zero elsewhere. Constant-rate on every target it reads (a wall-clock
/// tick, not a core-clock cycle), so a delta converts to seconds with
/// [`cycles_hz`]. Always compiled; costs one instruction where it exists.
#[must_use]
#[inline(always)]
pub fn cycles_now() -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        let (lo, hi): (u32, u32);
        // SAFETY: `rdtsc` reads the time-stamp counter into edx:eax and has no
        // other effect; it is unprivileged on every OS this crate targets.
        unsafe {
            core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags));
        }
        (u64::from(hi) << 32) | u64::from(lo)
    }
    #[cfg(target_arch = "aarch64")]
    {
        let v: u64;
        // SAFETY: `cntvct_el0` is the user-readable virtual counter; the read
        // has no side effects.
        unsafe {
            core::arch::asm!("mrs {}, cntvct_el0", out(reg) v, options(nomem, nostack, preserves_flags));
        }
        v
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        0
    }
}

/// Measures the rate of [`cycles_now`] in ticks per second over `window`
/// of wall-clock time. Zero on targets without a cycle counter. Publish it
/// beside any [`Stat::SampleSpinCycles`] figure; the ratio is host-specific.
#[cfg(feature = "std")]
#[must_use]
pub fn cycles_hz(window: core::time::Duration) -> u64 {
    let t0 = std::time::Instant::now();
    let c0 = cycles_now();
    std::thread::sleep(window);
    let c1 = cycles_now();
    let secs = t0.elapsed().as_secs_f64();
    if secs <= 0.0 {
        return 0;
    }
    ((c1.wrapping_sub(c0)) as f64 / secs) as u64
}

// ---- per-mutation scope ---------------------------------------------------

/// Marks the start of one mutation on this thread (`Shared::write`), so
/// [`op_end`] can tell whether its branch replacements cascaded. A no-op
/// without `occ-stats` + `std`.
#[inline(always)]
pub fn op_begin() {
    #[cfg(all(feature = "occ-stats", feature = "std"))]
    cells::OP_REPLACEMENTS.with(|c| c.set(0));
}

/// Records that the current mutation replaced or removed a branch node —
/// its parent's slot was rewritten. Counts [`Stat::BranchReplacements`].
#[inline(always)]
pub fn note_branch_replacement() {
    bump(Stat::BranchReplacements);
    #[cfg(all(feature = "occ-stats", feature = "std"))]
    cells::OP_REPLACEMENTS.with(|c| c.set(c.get().saturating_add(1)));
}

/// Records that the current mutation rewrote the owner's root state.
#[inline(always)]
pub fn note_root_rewrite() {
    bump(Stat::RootRewrites);
}

/// Closes the scope opened by [`op_begin`]: a mutation that replaced two or
/// more branch nodes is a [`Stat::DeepCascades`] event.
#[inline(always)]
pub fn op_end() {
    #[cfg(all(feature = "occ-stats", feature = "std"))]
    if cells::OP_REPLACEMENTS.with(core::cell::Cell::get) >= 2 {
        bump(Stat::DeepCascades);
    }
}

#[cfg(all(test, feature = "occ-stats", feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn names_cover_every_stat() {
        assert_eq!(NAMES.len(), NUM_STATS);
        assert_eq!(Stat::BranchSplitRemove as usize + 1, NUM_STATS);
        assert_eq!(NAMES[Stat::SampleSpinCycles as usize], "sample_spin_cycles");
        assert_eq!(NAMES[Stat::DeepCascades as usize], "deep_cascades");
        assert_eq!(NAMES[Stat::LockRestarts as usize], "lock_restarts");
        assert_eq!(NAMES[Stat::LockSpins as usize], "lock_spins");
        assert_eq!(NAMES[Stat::LockHoldCycles as usize], "lock_hold_cycles");
        assert_eq!(NAMES[Stat::LockFallbacks as usize], "lock_fallbacks");
        assert_eq!(NAMES[Stat::Inserts as usize], "inserts");
        assert_eq!(
            NAMES[Stat::FallbackCapExpansion as usize],
            "fallback_cap_expansion"
        );
        assert_eq!(
            NAMES[Stat::FallbackRootGrowth as usize],
            "fallback_root_growth"
        );
        assert_eq!(
            NAMES[Stat::ContentionGateClosed as usize],
            "contention_gate_closed"
        );
        assert_eq!(
            NAMES[Stat::ContentionRetryExhausted as usize],
            "contention_retry_exhausted"
        );
        assert_eq!(
            NAMES[Stat::GateBlockedEntries as usize],
            "gate_blocked_entries"
        );
        assert_eq!(NAMES[Stat::GateWaitCycles as usize], "gate_wait_cycles");
        assert_eq!(NAMES[Stat::QuiesceCalls as usize], "quiesce_calls");
        assert_eq!(
            NAMES[Stat::QuiesceDrainCycles as usize],
            "quiesce_drain_cycles"
        );
        assert_eq!(
            NAMES[Stat::BranchSplitSubarray as usize],
            "branch_split_subarray"
        );
        assert_eq!(
            NAMES[Stat::BranchSplitLinear as usize],
            "branch_split_linear"
        );
        assert_eq!(
            NAMES[Stat::BranchSplitPrefix as usize],
            "branch_split_prefix"
        );
        assert_eq!(
            NAMES[Stat::BranchSplitRemove as usize],
            "branch_split_remove"
        );
    }

    /// The discriminants are an unwritten contract: `snapshot()` is indexed
    /// by `Stat::X as usize` and the health rows ship counters by index, so
    /// renumbering an existing variant silently rebases every consumer.
    /// New counters append; this pins the prefix (AGENTS.md §8.18).
    #[test]
    fn existing_discriminants_are_stable() {
        assert_eq!(Stat::ReadOps as usize, 0);
        assert_eq!(Stat::WriteOps as usize, 3);
        assert_eq!(Stat::BranchReplacements as usize, 14);
        assert_eq!(Stat::LockFallbacks as usize, 20);
        // Appended for #568 Phase 0.
        assert_eq!(Stat::Inserts as usize, 21);
        assert_eq!(Stat::FallbackCapExpansion as usize, 22);
        assert_eq!(Stat::FallbackImmediateConversion as usize, 23);
        assert_eq!(Stat::FallbackBranchSplit as usize, 24);
        assert_eq!(Stat::FallbackRootGrowth as usize, 25);
        assert_eq!(Stat::FallbackContention as usize, 26);
        assert_eq!(Stat::FallbackUnknownTag as usize, 27);
        assert_eq!(Stat::PopRedescends as usize, 28);
        assert_eq!(Stat::PopRedescendAbandoned as usize, 29);
        // Appended for #568 Step 4.0.
        assert_eq!(Stat::ContentionGateClosed as usize, 30);
        assert_eq!(Stat::ContentionRetryExhausted as usize, 31);
        assert_eq!(Stat::GateBlockedEntries as usize, 32);
        assert_eq!(Stat::GateWaitCycles as usize, 33);
        assert_eq!(Stat::QuiesceCalls as usize, 34);
        assert_eq!(Stat::QuiesceDrainCycles as usize, 35);
        assert_eq!(Stat::BranchSplitSubarray as usize, 36);
        assert_eq!(Stat::BranchSplitLinear as usize, 37);
        assert_eq!(Stat::BranchSplitPrefix as usize, 38);
        assert_eq!(Stat::BranchSplitRemove as usize, 39);
    }

    #[test]
    fn shards_sum_across_threads() {
        // The counters are process-global, so this is a lower bound: other
        // tests in the same binary may bump concurrently (see #789).
        let before = snapshot()[Stat::ReadOps as usize];
        let hs: Vec<_> = (0..4)
            .map(|_| std::thread::spawn(|| (0..1000).for_each(|_| bump(Stat::ReadOps))))
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        assert!(snapshot()[Stat::ReadOps as usize] >= before + 4000);
    }

    #[test]
    fn deep_cascade_needs_two_replacements() {
        let base = snapshot();
        op_begin();
        note_branch_replacement();
        op_end();
        let one = snapshot();
        assert_eq!(
            one[Stat::DeepCascades as usize],
            base[Stat::DeepCascades as usize],
            "one replacement is not a cascade"
        );
        op_begin();
        note_branch_replacement();
        note_branch_replacement();
        op_end();
        let two = snapshot();
        assert_eq!(
            two[Stat::DeepCascades as usize],
            base[Stat::DeepCascades as usize] + 1
        );
        assert_eq!(
            two[Stat::BranchReplacements as usize],
            base[Stat::BranchReplacements as usize] + 3
        );
    }

    #[test]
    fn cycle_counter_advances_and_calibrates() {
        let a = cycles_now();
        let hz = cycles_hz(core::time::Duration::from_millis(20));
        let b = cycles_now();
        if cfg!(any(target_arch = "x86_64", target_arch = "aarch64")) {
            assert!(b > a, "cycle counter must advance");
            // Any real counter runs between 1 MHz (Arm's minimum) and 10 GHz.
            assert!((1_000_000..=10_000_000_000).contains(&hz), "hz = {hz}");
        } else {
            assert_eq!(hz, 0);
        }
    }
}
