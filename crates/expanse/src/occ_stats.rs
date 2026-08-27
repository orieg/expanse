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
//! Run the probe with
//! `cargo run --release -p expanse-trie --features occ-stats --example occ_stats_probe`.

/// A counted event in the OCC protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Stat {
    /// Public lock-free read calls entered.
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
}

/// Number of distinct counters.
pub const NUM_STATS: usize = 7;

/// Human-readable counter names, indexed by [`Stat`].
pub const NAMES: [&str; NUM_STATS] = [
    "read_ops",
    "read_attempts",
    "read_fallbacks",
    "write_ops",
    "advance_calls",
    "advance_ok",
    "sample_spins",
];

#[cfg(feature = "occ-stats")]
static COUNTERS: [core::sync::atomic::AtomicU64; NUM_STATS] =
    [const { core::sync::atomic::AtomicU64::new(0) }; NUM_STATS];

/// Records one occurrence of `s`. A no-op without `occ-stats`.
#[inline(always)]
pub fn bump(s: Stat) {
    let _ = s;
    #[cfg(feature = "occ-stats")]
    COUNTERS[s as usize].fetch_add(1, core::sync::atomic::Ordering::Relaxed);
}

/// Records `n` occurrences of `s`. A no-op without `occ-stats`.
#[inline(always)]
pub fn bump_by(s: Stat, n: u64) {
    let _ = (s, n);
    #[cfg(feature = "occ-stats")]
    COUNTERS[s as usize].fetch_add(n, core::sync::atomic::Ordering::Relaxed);
}

/// Reads every counter. All zero without `occ-stats`.
#[must_use]
pub fn snapshot() -> [u64; NUM_STATS] {
    #[cfg(feature = "occ-stats")]
    {
        core::array::from_fn(|i| COUNTERS[i].load(core::sync::atomic::Ordering::Relaxed))
    }
    #[cfg(not(feature = "occ-stats"))]
    {
        [0; NUM_STATS]
    }
}

/// Zeroes every counter (call between measurement phases).
pub fn reset() {
    #[cfg(feature = "occ-stats")]
    for c in &COUNTERS {
        c.store(0, core::sync::atomic::Ordering::Relaxed);
    }
}

/// True when the counters are live (the `occ-stats` feature is on).
#[must_use]
pub const fn enabled() -> bool {
    cfg!(feature = "occ-stats")
}
