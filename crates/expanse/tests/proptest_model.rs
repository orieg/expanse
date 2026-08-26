//! Property tests with shrinking (`docs/TESTING.md` layer 2).
//!
//! The in-crate `model_*` tests run fixed-seed op sequences: reproducible,
//! but a failure arrives as a 6000-op transcript. Proptest explores the
//! same space with generated sequences and **shrinks** any failure to a
//! minimal counterexample — the difference between "something in these
//! 6000 ops breaks" and "insert(0x100), insert(0x1_0000), remove(0x100)
//! breaks". Counterexamples persist under `tests/*.proptest-regressions`
//! and are replayed first on later runs; commit that file when one
//! appears, exactly as a hand-written regression test would be.
//!
//! Not run under Miri: proptest resolves its regression-file path via
//! `getcwd`, which Miri's isolation forbids, and a full proptest sweep
//! under Miri costs ~15 minutes for coverage the in-crate `model_*`
//! suites already provide there.
//!
//! Keys are drawn from the `docs/TESTING.md` distribution classes rather
//! than uniformly at random: uniform 64-bit keys almost never collide in
//! their high bytes, so they exercise neither the cascade ladder nor the
//! narrow-pointer paths where the interesting bugs live.

#![cfg(not(miri))]

use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use proptest::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

/// One operation against a container and its model.
#[derive(Debug, Clone)]
enum Op {
    Insert(u64),
    Remove(u64),
    Get(u64),
    /// Structural check plus a full ordered comparison against the model.
    Audit,
}

/// Keys biased toward the structure's interesting regions: small dense
/// runs, clusters sharing high bytes (cascades, narrow pointers), sparse
/// high-byte-only keys (single-child chains), and the extremes.
fn key_strategy() -> impl Strategy<Value = u64> {
    prop_oneof![
        // Dense low run: immediates → linear leaves → bitmap leaves.
        2 => 0u64..256,
        // Two clusters sharing 6 high bytes: last-byte divergence.
        3 => (0u64..2).prop_flat_map(|c| {
            let base = if c == 0 { 0xAABB_CCDD_EE00 } else { 0x1122_3344_5500 };
            (0u64..512).prop_map(move |i| base + i)
        }),
        // Multi-byte divergence: exercises branch-targeted skips.
        2 => (0u64..4096).prop_map(|i| 0x7777_0000_0000 + i),
        // Sparse: one populated digit per level.
        2 => (0u64..64).prop_map(|i| i << 40),
        // Extremes and full-width keys.
        1 => prop_oneof![Just(0u64), Just(u64::MAX), any::<u64>()],
    ]
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => key_strategy().prop_map(Op::Insert),
        3 => key_strategy().prop_map(Op::Remove),
        2 => key_strategy().prop_map(Op::Get),
        1 => Just(Op::Audit),
    ]
}

/// Runs `ops` against `ExpanseSet` and a `BTreeSet` model, asserting
/// agreement on every operation and full ordered equality at each audit.
fn run_set(ops: &[Op]) {
    let mut set = ExpanseSet::new();
    let mut model = BTreeSet::new();
    for op in ops {
        match *op {
            Op::Insert(k) => assert_eq!(set.insert(k), model.insert(k), "insert {k:#x}"),
            Op::Remove(k) => assert_eq!(set.remove(k), model.remove(&k), "remove {k:#x}"),
            Op::Get(k) => assert_eq!(set.contains(k), model.contains(&k), "contains {k:#x}"),
            Op::Audit => {
                set.validate();
                assert_eq!(set.len(), model.len() as u64, "len");
                assert!(set.iter().eq(model.iter().copied()), "ordered iteration");
                assert!(
                    set.iter_rev().eq(model.iter().rev().copied()),
                    "reverse iteration"
                );
                // The reverse iterator is double-ended: `.rev()` recovers ascending.
                assert!(
                    set.iter_rev().rev().eq(model.iter().copied()),
                    "iter_rev().rev()"
                );
                if let (Some(&lo), Some(&hi)) = (model.iter().next(), model.iter().next_back()) {
                    assert!(
                        set.range_rev(lo..=hi)
                            .eq(model.range(lo..=hi).rev().copied()),
                        "reverse range"
                    );
                    assert!(
                        set.range_rev(lo..=hi)
                            .rev()
                            .eq(model.range(lo..=hi).copied()),
                        "range_rev().rev()"
                    );
                }
                // Rank/select agree with the model's own ordering.
                if let Some(&first) = model.iter().next() {
                    assert_eq!(set.first(), Some(first));
                    assert_eq!(set.by_count(0), Some(first));
                    assert_eq!(set.count_below(first), 0);
                }
                if let Some(&last) = model.iter().next_back() {
                    assert_eq!(set.last(), Some(last));
                    assert_eq!(set.count_below(last), model.len() as u64 - 1);
                }
            }
        }
    }
    // Final state: full agreement, then drain to empty with no leak.
    set.validate();
    assert!(set.iter().eq(model.iter().copied()));
    for k in &model {
        assert!(set.remove(*k), "final drain {k:#x}");
    }
    assert!(set.is_empty());
    assert_eq!(set.mem_used(), 0, "leak after drain");
}

/// Map mirror of [`run_set`]: values are a function of the key, so a torn
/// or misplaced value is detectable rather than merely absent.
fn run_map(ops: &[Op]) {
    let val_of = |k: u64| !k ^ 0x5EED;
    let mut map = ExpanseMap::new();
    let mut model = BTreeMap::new();
    for op in ops {
        match *op {
            Op::Insert(k) => assert_eq!(
                map.insert(k, val_of(k)),
                model.insert(k, val_of(k)),
                "insert {k:#x}"
            ),
            Op::Remove(k) => assert_eq!(map.remove(k), model.remove(&k), "remove {k:#x}"),
            Op::Get(k) => assert_eq!(map.get(k), model.get(&k).copied(), "get {k:#x}"),
            Op::Audit => {
                map.validate();
                assert_eq!(map.len(), model.len() as u64, "len");
                assert!(
                    map.iter().eq(model.iter().map(|(k, v)| (*k, *v))),
                    "ordered iteration"
                );
                assert!(
                    map.iter_rev().eq(model.iter().rev().map(|(k, v)| (*k, *v))),
                    "reverse iteration"
                );
                assert!(
                    map.iter_rev().rev().eq(model.iter().map(|(k, v)| (*k, *v))),
                    "iter_rev().rev()"
                );
                if let (Some((&lo, _)), Some((&hi, _))) =
                    (model.iter().next(), model.iter().next_back())
                {
                    assert!(
                        map.range_rev(lo..=hi)
                            .eq(model.range(lo..=hi).rev().map(|(k, v)| (*k, *v))),
                        "reverse range"
                    );
                    assert!(
                        map.range_rev(lo..=hi)
                            .rev()
                            .eq(model.range(lo..=hi).map(|(k, v)| (*k, *v))),
                        "range_rev().rev()"
                    );
                }
                if let Some((&k, &v)) = model.iter().next() {
                    assert_eq!(map.first(), Some((k, v)));
                    assert_eq!(map.by_count(0), Some((k, v)));
                }
            }
        }
    }
    map.validate();
    assert!(map.iter().eq(model.iter().map(|(k, v)| (*k, *v))));
    for (k, v) in &model {
        assert_eq!(map.remove(*k), Some(*v), "final drain {k:#x}");
    }
    assert!(map.is_empty());
    assert_eq!(map.mem_used(), 0, "leak after drain");
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 96,
        max_shrink_iters: 4096,
        ..ProptestConfig::default()
    })]

    #[test]
    fn set_matches_btreeset(ops in prop::collection::vec(op_strategy(), 0..400)) {
        run_set(&ops);
    }

    #[test]
    fn map_matches_btreemap(ops in prop::collection::vec(op_strategy(), 0..400)) {
        run_map(&ops);
    }

    /// Inserting a set of keys then removing them in a *different* order
    /// must return the container to empty — the ladder's down-conversions
    /// are order-sensitive in a way plain op sequences reach only rarely.
    #[test]
    fn insert_all_then_remove_in_shuffled_order(
        keys in prop::collection::vec(key_strategy(), 1..300),
        rotate in 0usize..300,
    ) {
        let mut set = ExpanseSet::new();
        let mut model = BTreeSet::new();
        for &k in &keys {
            set.insert(k);
            model.insert(k);
        }
        set.validate();

        let mut order: Vec<u64> = model.iter().copied().collect();
        let n = order.len();
        order.rotate_left(rotate % n);
        for (i, k) in order.iter().enumerate() {
            prop_assert!(set.remove(*k), "remove {:#x}", k);
            if i % 32 == 0 {
                set.validate();
            }
        }
        prop_assert!(set.is_empty());
        prop_assert_eq!(set.mem_used(), 0);
    }

    /// Rank/select are inverses over the whole population: the key with
    /// `n` keys below it must itself have rank `n`.
    #[test]
    fn rank_select_round_trip(keys in prop::collection::vec(key_strategy(), 1..300)) {
        let mut set = ExpanseSet::new();
        let mut model = BTreeSet::new();
        for &k in &keys {
            set.insert(k);
            model.insert(k);
        }
        for (n, &k) in model.iter().enumerate() {
            prop_assert_eq!(set.by_count(n as u64), Some(k), "select {}", n);
            prop_assert_eq!(set.count_below(k), n as u64, "rank {:#x}", k);
        }
        prop_assert_eq!(set.by_count(model.len() as u64), None, "select past the end");
    }
}
