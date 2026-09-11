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

    // Exact identity 1: QuiesceCalls == LockFallbacks on insert-only sweeps
    assert_eq!(
        d(Stat::QuiesceCalls),
        fallbacks,
        "map: QuiesceCalls ({}) must equal LockFallbacks ({}) on insert-only workload",
        d(Stat::QuiesceCalls),
        fallbacks
    );
    assert_eq!(
        sd(Stat::QuiesceCalls),
        s_fallbacks,
        "set: QuiesceCalls ({}) must equal LockFallbacks ({}) on insert-only workload",
        sd(Stat::QuiesceCalls),
        s_fallbacks
    );

    // Exact identity 2: Contention partition
    assert_eq!(
        d(Stat::ContentionGateClosed) + d(Stat::ContentionRetryExhausted),
        d(Stat::FallbackContention),
        "map: Contention subsets must sum to FallbackContention"
    );
    assert_eq!(
        sd(Stat::ContentionGateClosed) + sd(Stat::ContentionRetryExhausted),
        sd(Stat::FallbackContention),
        "set: Contention subsets must sum to FallbackContention"
    );

    // Exact identity 3: BranchSplit partition
    let bs_partition_map = d(Stat::BranchSplitSubarray)
        + d(Stat::BranchSplitLinear)
        + d(Stat::BranchSplitPrefix)
        + d(Stat::BranchSplitRemove)
        + d(Stat::BranchSplitUpgrade);
    assert_eq!(
        bs_partition_map,
        d(Stat::FallbackBranchSplit),
        "map: BranchSplit subsets must sum to FallbackBranchSplit"
    );
    let bs_partition_set = sd(Stat::BranchSplitSubarray)
        + sd(Stat::BranchSplitLinear)
        + sd(Stat::BranchSplitPrefix)
        + sd(Stat::BranchSplitRemove)
        + sd(Stat::BranchSplitUpgrade);
    assert_eq!(
        bs_partition_set,
        sd(Stat::FallbackBranchSplit),
        "set: BranchSplit subsets must sum to FallbackBranchSplit"
    );

    // Remove partition must be 0 on insert-only workloads
    assert_eq!(d(Stat::BranchSplitRemove), 0);
    assert_eq!(sd(Stat::BranchSplitRemove), 0);

    // Exact identity 4: CapExpansion partition (#568)
    let ce_partition_map = d(Stat::CapExpansionClass)
        + d(Stat::CapExpansionLeafFull)
        + d(Stat::CapExpansionBitmapNearFull)
        + d(Stat::CapExpansionMapBitmapSub)
        + d(Stat::CapExpansionRemove);
    eprintln!(
        "CapExpansion map breakdown: Class={}, LeafFull={}, BitmapNearFull={}, MapBitmapSub={}, Remove={}",
        d(Stat::CapExpansionClass),
        d(Stat::CapExpansionLeafFull),
        d(Stat::CapExpansionBitmapNearFull),
        d(Stat::CapExpansionMapBitmapSub),
        d(Stat::CapExpansionRemove)
    );
    eprintln!(
        "CapExpansion set breakdown: Class={}, LeafFull={}, BitmapNearFull={}, MapBitmapSub={}, Remove={}",
        sd(Stat::CapExpansionClass),
        sd(Stat::CapExpansionLeafFull),
        sd(Stat::CapExpansionBitmapNearFull),
        sd(Stat::CapExpansionMapBitmapSub),
        sd(Stat::CapExpansionRemove)
    );
    assert_eq!(
        ce_partition_map,
        d(Stat::FallbackCapExpansion),
        "map: CapExpansion subsets must sum to FallbackCapExpansion"
    );
    let ce_partition_set = sd(Stat::CapExpansionClass)
        + sd(Stat::CapExpansionLeafFull)
        + sd(Stat::CapExpansionBitmapNearFull)
        + sd(Stat::CapExpansionMapBitmapSub)
        + sd(Stat::CapExpansionRemove);
    assert_eq!(
        ce_partition_set,
        sd(Stat::FallbackCapExpansion),
        "set: CapExpansion subsets must sum to FallbackCapExpansion"
    );

    // Verify discrimination: both Class and LeafFull must be non-zero on both map and set workloads
    assert!(
        d(Stat::CapExpansionClass) > 0,
        "map: CapExpansionClass must discriminate (> 0)"
    );
    assert!(
        d(Stat::CapExpansionLeafFull) > 0,
        "map: CapExpansionLeafFull must discriminate (> 0)"
    );
    assert!(
        sd(Stat::CapExpansionClass) > 0,
        "set: CapExpansionClass must discriminate (> 0)"
    );
    assert!(
        sd(Stat::CapExpansionLeafFull) > 0,
        "set: CapExpansionLeafFull must discriminate (> 0)"
    );

    // Remove partition must be 0 on insert-only workloads
    assert_eq!(d(Stat::CapExpansionRemove), 0);
    assert_eq!(sd(Stat::CapExpansionRemove), 0);

    // MapBitmapSub is map-only: must be 0 on set
    assert_eq!(sd(Stat::CapExpansionMapBitmapSub), 0);

    // Phase 4D: BranchB subarray growth is concurrent, eliminating Subarray
    // fallbacks (the dominant branch split cause). BranchSplitSubarray is exactly 0.
    assert_eq!(
        d(Stat::BranchSplitSubarray),
        0,
        "map: BranchSplitSubarray must be 0 after Phase 4D"
    );
    assert_eq!(
        sd(Stat::BranchSplitSubarray),
        0,
        "set: BranchSplitSubarray must be 0 after Phase 4D"
    );

    // A single-threaded test workload experiences zero lock contention (F3).
    // Under W >= 2 concurrent writers, the gate-closure feedback loop in
    // sync.rs makes contention non-zero by design.
    assert_eq!(
        d(Stat::FallbackContention),
        0,
        "single-threaded map test workload must have zero contention fallbacks"
    );
    assert_eq!(
        d(Stat::ContentionGateClosed),
        0,
        "single-threaded map test workload must have zero gate-closed contention"
    );
    assert_eq!(
        d(Stat::ContentionRetryExhausted),
        0,
        "single-threaded map test workload must have zero retry-exhausted contention"
    );
    assert_eq!(
        d(Stat::GateBlockedEntries),
        0,
        "single-threaded map test workload must have zero gate blocked entries"
    );
    assert_eq!(
        d(Stat::GateWaitCycles),
        0,
        "single-threaded map test workload must have zero gate wait cycles"
    );
    assert_eq!(
        sd(Stat::FallbackContention),
        0,
        "single-threaded set test workload must have zero contention fallbacks"
    );
    assert_eq!(
        sd(Stat::ContentionGateClosed),
        0,
        "single-threaded set test workload must have zero gate-closed contention"
    );
    assert_eq!(
        sd(Stat::ContentionRetryExhausted),
        0,
        "single-threaded set test workload must have zero retry-exhausted contention"
    );
    assert_eq!(
        sd(Stat::GateBlockedEntries),
        0,
        "single-threaded set test workload must have zero gate blocked entries"
    );
    assert_eq!(
        sd(Stat::GateWaitCycles),
        0,
        "single-threaded set test workload must have zero gate wait cycles"
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

    // Structural invariant: ensure no raw `OlcOutcome::Fallback(FallbackCause::CapExpansion)`
    // exists in sync.rs outside `fn cap_expansion` (#568).
    let sync_src = include_str!("../src/sync.rs");
    let raw_pattern = "OlcOutcome::Fallback(FallbackCause::CapExpansion)";
    let count = sync_src.matches(raw_pattern).count();
    assert_eq!(
        count, 1,
        "only fn cap_expansion may return raw Fallback(FallbackCause::CapExpansion)"
    );
    assert_eq!(
        sync_src.matches("cap_expansion(").count(),
        10,
        "cap_expansion must be called at exactly 10 exit sites in sync.rs (AGENTS.md §2.3)"
    );
    for kind in [
        "CapExpansionKind::Class",
        "CapExpansionKind::LeafFull",
        "CapExpansionKind::BitmapNearFull",
        "CapExpansionKind::MapBitmapSub",
        "CapExpansionKind::Remove",
    ] {
        assert!(
            sync_src.contains(kind),
            "CapExpansionKind::{kind} must be used in sync.rs"
        );
    }

    // Decision-point structural routing and classification helper (AGENTS.md §2.3):
    let prod_sync = sync_src.split("mod tests {").next().expect("prod code");
    let def_count = prod_sync.matches("fn classify_leaf_expansion").count();
    let call_count = prod_sync.matches("classify_leaf_expansion(").count() - def_count;
    assert_eq!(
        def_count, 1,
        "classify_leaf_expansion must be defined exactly once as a pure decision helper"
    );
    assert_eq!(
        call_count, 2,
        "classify_leaf_expansion must be called at exactly 2 decision points in prod code (set and map insert)"
    );

    // Targeted discrimination checks: verify all 5 `CapExpansion` sub-causes discriminate (> 0)
    // under targeted workloads designed to trigger each respective engine transition across both
    // map and set engines (Refs #568, AGENTS.md §2.3).

    // 1. CapExpansionBitmapNearFull on SET:
    // Inserting 256 keys (0..256) into a single level-1 leaf reaches pop0 >= 254 and triggers BitmapNearFull.
    let targeted_set = expanse_trie::sync::SyncExpanseSet::new();
    let s_before_bm = occ_stats::snapshot();
    for k in 0..256u64 {
        targeted_set.insert(k);
    }
    let s_after_bm = occ_stats::snapshot();
    let bm_near_full_set = s_after_bm[Stat::CapExpansionBitmapNearFull as usize]
        - s_before_bm[Stat::CapExpansionBitmapNearFull as usize];
    assert!(
        bm_near_full_set > 0,
        "set: CapExpansionBitmapNearFull must discriminate (> 0) on dense level-1 leaf, got {bm_near_full_set}"
    );

    // 2. CapExpansionClass, CapExpansionLeafFull, and CapExpansionMapBitmapSub on MAP:
    // A map root is only a tree when pop > ROOT_LEAF_CAP (31).
    // Populate 32 keys in prefix 0x0100 and 26 keys in prefix 0x0000 (total 58 > 31).
    // Child 0 (26 keys > LEAF1_CAP=25) converts to LeafB1 (sub-expanse 0).
    let targeted_map = expanse_trie::sync::SyncExpanseMap::new();
    for k in 0..32u64 {
        targeted_map.insert(0x0100 + k, k);
    }
    let s_before_fill = occ_stats::snapshot();
    for k in 0..26u64 {
        targeted_map.insert(k, k * 10);
    }
    let s_after_fill = occ_stats::snapshot();
    let class_cnt = s_after_fill[Stat::CapExpansionClass as usize]
        - s_before_fill[Stat::CapExpansionClass as usize];
    let leaf_full_cnt = s_after_fill[Stat::CapExpansionLeafFull as usize]
        - s_before_fill[Stat::CapExpansionLeafFull as usize];
    assert!(
        class_cnt > 0,
        "map: CapExpansionClass must discriminate (> 0) during leaf class growth, got {class_cnt}"
    );
    assert!(
        leaf_full_cnt > 0,
        "map: CapExpansionLeafFull must discriminate (> 0) when leaf reaches capacity, got {leaf_full_cnt}"
    );

    // MapBitmapSub:
    // (a) Sub-expanse 1 empty insert: key 32 has old_n == 0, triggering MapBitmapSub.
    let s_before_sub = occ_stats::snapshot();
    targeted_map.insert(32, 320);
    let s_after_sub = occ_stats::snapshot();
    let map_sub_empty = s_after_sub[Stat::CapExpansionMapBitmapSub as usize]
        - s_before_sub[Stat::CapExpansionMapBitmapSub as usize];
    assert!(
        map_sub_empty > 0,
        "map: CapExpansionMapBitmapSub must discriminate (> 0) on empty subarray entry, got {map_sub_empty}"
    );

    // (b) Sub-expanse 1 populated growth: inserting key 33 has old_n == 1, growing class 1 -> 2.
    let s_before_sub_grow = occ_stats::snapshot();
    targeted_map.insert(33, 330);
    let s_after_sub_grow = occ_stats::snapshot();
    let map_sub_grow = s_after_sub_grow[Stat::CapExpansionMapBitmapSub as usize]
        - s_before_sub_grow[Stat::CapExpansionMapBitmapSub as usize];
    assert!(
        map_sub_grow > 0,
        "map: CapExpansionMapBitmapSub must discriminate (> 0) on populated subarray class growth, got {map_sub_grow}"
    );

    // 3. CapExpansionBitmapNearFull on MAP:
    // Insert keys 26..256 into prefix 0x0000. Child 0 is LeafB1; as it reaches 254+ keys,
    // keys 254 and 255 hit pop0 >= 254 (sync.rs:4357), triggering Map BitmapNearFull.
    let s_before_map_bm = occ_stats::snapshot();
    for k in 26..256u64 {
        targeted_map.insert(k, k * 10);
    }
    let s_after_map_bm = occ_stats::snapshot();
    let bm_near_full_map = s_after_map_bm[Stat::CapExpansionBitmapNearFull as usize]
        - s_before_map_bm[Stat::CapExpansionBitmapNearFull as usize];
    assert!(
        bm_near_full_map > 0,
        "map: CapExpansionBitmapNearFull must discriminate (> 0) on dense LeafB1 (pop0 >= 254), got {bm_near_full_map}"
    );

    // 4. CapExpansionRemove on MAP:
    // Removals that shrink nodes or drop below threshold (pop0 <= 32).
    let s_before_rem = occ_stats::snapshot();
    for k in 0..25u64 {
        targeted_map.remove(k);
    }
    let s_after_rem = occ_stats::snapshot();
    let rem_map = s_after_rem[Stat::CapExpansionRemove as usize]
        - s_before_rem[Stat::CapExpansionRemove as usize];
    assert!(
        rem_map > 0,
        "map: CapExpansionRemove must discriminate (> 0) on removals, got {rem_map}"
    );

    // 5. CapExpansionRemove on SET:
    // Removing keys from targeted_set (which has 256 keys in a LeafB1 child) triggers
    // CapExpansionRemove when popping below threshold (pop0 <= 32) or linear leaf demotion.
    let s_before_set_rem = occ_stats::snapshot();
    for k in 0..250u64 {
        targeted_set.remove(k);
    }
    let s_after_set_rem = occ_stats::snapshot();
    let rem_set = s_after_set_rem[Stat::CapExpansionRemove as usize]
        - s_before_set_rem[Stat::CapExpansionRemove as usize];
    assert!(
        rem_set > 0,
        "set: CapExpansionRemove must discriminate (> 0) on removals, got {rem_set}"
    );
}
