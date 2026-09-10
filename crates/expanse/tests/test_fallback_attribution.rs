//! Phase 0 attribution invariants for the multi-writer OLC fallback counters
//! (Refs #568).
//!
//! `occ_stats` counters are process-global, so this file holds exactly one
//! test: the harness runs each integration target in its own process, and a
//! second test here would race the sums this one asserts.
//!
//! # Workload shape
//!
//! | field | value |
//! |---|---|
//! | `workload_id` | `olc_fallback_attribution_invariants` |
//! | `group` | correctness |
//! | `population` | 20000 |
//! | `insertion_order` | generator |
//! | `probes_and_reuse` | none — insert-only, each key once |
//! | `hit_rate` | n/a (no read probes) |
//! | `miss_gen_method` | n/a |
//! | `value_dereference` | n/a |
//! | `measured_region` | n/a — counter invariants, nothing is timed |
//! | `arm_symmetry` | single arm |
//! | `statistics` | none — deterministic integer counters |
//! | `verdict` | assertion on exact counter identities |
// Kept as its own attribute, and in this exact form: the nightly Miri shard
// census (`scripts/check_miri_shards.py`) matches `^#!\[cfg\(not\(miri\)\)\]`
// literally rather than parsing nested `cfg(all(..))`, so folding it into the
// clause below would read as a Miri-runnable target and fail the lint job.
#![cfg(not(miri))]
#![cfg(all(feature = "occ-stats", feature = "std", target_pointer_width = "64"))]

use expanse_trie::occ_stats::{self, Stat};
use expanse_trie::sync::SyncExpanseMap;

/// The four structural causes plus the two non-structural ones.
const CAUSES: [Stat; 6] = [
    Stat::FallbackCapExpansion,
    Stat::FallbackImmediateConversion,
    Stat::FallbackBranchSplit,
    Stat::FallbackRootGrowth,
    Stat::FallbackContention,
    Stat::FallbackUnknownTag,
];

#[test]
fn fallback_causes_account_for_every_fallback() {
    let map = SyncExpanseMap::new();
    let mut k: u64 = 0x9E37_79B9_7F4A_7C15;
    let next = |k: &mut u64| {
        *k = k
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *k
    };

    // Prefill past the root-leaf -> tree transition before snapshotting.
    //
    // The `write_ops` identity below holds only while the root is already a
    // tree. A fallback taken *before* the OLC attempt — the `!root_is_tree()`
    // pre-check — reaches `write_root_covered` without the driver having
    // bumped `WriteOps` first, so it double-counts nowhere and leaves
    // `write_ops` short by exactly the number of such inserts
    // (`ROOT_LEAF_CAP + 1` of them from an empty map). The FFI concurrency
    // suites prefill before their measured phase for the same reason, which
    // is why their committed artifacts show the identity exactly.
    for i in 0..1_000u64 {
        let key = next(&mut k);
        map.insert(key, i);
    }
    assert!(
        map.len() >= 1_000,
        "prefill must leave the root in tree state"
    );

    let before = occ_stats::snapshot();
    // A deterministic spread wide enough to build branches, bitmap nodes and
    // multi-class leaves, so more than one cause fires.
    for i in 0..20_000u64 {
        let key = next(&mut k);
        map.insert(key, 1_000 + i);
    }
    let after = occ_stats::snapshot();
    let d = |s: Stat| after[s as usize] - before[s as usize];

    let inserts = d(Stat::Inserts);
    let fallbacks = d(Stat::LockFallbacks);
    let write_ops = d(Stat::WriteOps);

    assert_eq!(inserts, 20_000, "one Inserts bump per public insert call");

    // The identity that made the published fallback shares wrong: `WriteOps`
    // is bumped on the OLC attempt and again inside `write_root_covered` when
    // the fallback is taken, so it is *not* a per-insert denominator. This
    // pins that relationship rather than leaving it to be rediscovered.
    assert_eq!(
        write_ops,
        inserts + fallbacks,
        "write_ops must stay inserts + lock_fallbacks; \
         a per-insert rate divides by Stat::Inserts, never by write_ops"
    );

    // Every fallback carries exactly one cause, so the shares are checkable
    // instead of residual (AGENTS.md §8.1).
    let summed: u64 = CAUSES.iter().map(|&s| d(s)).sum();
    eprintln!("Attribution breakdown for 20k map inserts:");
    for &c in &CAUSES {
        eprintln!(
            "  {:30}: {:6} ({:.2}%)",
            format!("{:?}", c),
            d(c),
            (d(c) as f64 / fallbacks as f64) * 100.0
        );
    }
    eprintln!(
        "  Total fallbacks: {} / {} inserts ({:.2}%)",
        fallbacks,
        inserts,
        (fallbacks as f64 / inserts as f64) * 100.0
    );

    // Also run for SyncExpanseSet
    let set = expanse_trie::sync::SyncExpanseSet::new();
    let mut sk = 0x9E37_79B9_7F4A_7C15;
    for _ in 0..1_000 {
        set.insert(next(&mut sk));
    }
    let s_before = occ_stats::snapshot();
    for _ in 0..20_000 {
        set.insert(next(&mut sk));
    }
    let s_after = occ_stats::snapshot();
    let sd = |s: Stat| s_after[s as usize] - s_before[s as usize];
    let s_inserts = sd(Stat::Inserts);
    let s_fallbacks = sd(Stat::LockFallbacks);
    eprintln!("Attribution breakdown for 20k set inserts:");
    for &c in &CAUSES {
        eprintln!(
            "  {:30}: {:6} ({:.2}%)",
            format!("{:?}", c),
            sd(c),
            (sd(c) as f64 / s_fallbacks as f64) * 100.0
        );
    }
    eprintln!(
        "  Total fallbacks: {} / {} inserts ({:.2}%)",
        s_fallbacks,
        s_inserts,
        (s_fallbacks as f64 / s_inserts as f64) * 100.0
    );

    let s_summed: u64 = CAUSES.iter().map(|&s| sd(s)).sum();
    assert_eq!(
        s_summed,
        s_fallbacks,
        "set fallback causes must sum to s_fallbacks; unattributed = {}",
        s_fallbacks as i64 - s_summed as i64
    );

    assert_eq!(
        summed,
        fallbacks,
        "fallback causes must sum to lock_fallbacks; unattributed = {}",
        fallbacks as i64 - summed as i64
    );

    // Pin the dominance of FallbackBranchSplit over FallbackCapExpansion (F4 / §8.9.1).
    // BranchSplit accounts for 68-81% of fallbacks on both structures, at least 3x
    // greater than leaf capacity expansion (~16%). This pins the composition so that
    // any silent shift is immediately detected.
    assert!(
        d(Stat::FallbackBranchSplit) > d(Stat::FallbackCapExpansion) * 3,
        "map: BranchSplit ({}) must exceed CapExpansion ({}) by at least 3x",
        d(Stat::FallbackBranchSplit),
        d(Stat::FallbackCapExpansion)
    );
    assert!(
        sd(Stat::FallbackBranchSplit) > sd(Stat::FallbackCapExpansion) * 3,
        "set: BranchSplit ({}) must exceed CapExpansion ({}) by at least 3x",
        sd(Stat::FallbackBranchSplit),
        sd(Stat::FallbackCapExpansion)
    );

    // Single-writer benchmark workloads experience zero lock contention (F3).
    assert_eq!(
        d(Stat::FallbackContention),
        0,
        "map workload under W=1 must have zero contention fallbacks"
    );
    assert_eq!(
        sd(Stat::FallbackContention),
        0,
        "set workload under W=1 must have zero contention fallbacks"
    );

    // The OLC walk does *not* decode every tag it can meet, and this counter
    // is how that became visible. `EdgeType::FullExpanse` has an arm only in
    // `olc_insert_set`; `olc_remove_set`, `olc_insert_map` and `olc_remove_map`
    // fall through to their catch-all, so every mutation reaching a dense
    // level-1 terminal on those three paths takes the serialized path for a
    // reason no structural counter names. Maps do build such terminals
    // (`mutate_map.rs:1636`, `:2298`).
    //
    // Note: On this specific insert-only map workload, `unknown` may measure 0,
    // but readers must not take `unknown == 0` as proof the OLC walk is fully
    // complete: `FullExpanse` remains undecoded in `olc_remove_set`, `olc_insert_map`,
    // and `olc_remove_map`.
    //
    // The share is small here, and the point of the bound is that it stays
    // small: an unexplained bucket that grows into a material fraction of
    // fallbacks would invalidate the four structural shares Phase 0 reports,
    // rather than merely annotating them (Refs #568).
    let unknown = d(Stat::FallbackUnknownTag);
    assert!(
        unknown * 100 < fallbacks,
        "unattributed fallbacks are {unknown} of {fallbacks} (>= 1%); the OLC \
         walk's undecoded-tag hole is now large enough to distort the \
         structural shares and must be closed before Phase 0 is interpreted"
    );

    assert!(fallbacks > 0, "workload must exercise the fallback path");
    assert_eq!(map.len(), 21_000);
    assert_eq!(set.len(), 21_000);
}
