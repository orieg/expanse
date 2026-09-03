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

    /// Native set algebra (issue #339) matches `BTreeSet` for both the
    /// cardinality kernels and the materialized results, on independently
    /// generated key sets drawn from the interesting-region strategy (which
    /// makes real overlap, shared high-byte prefixes, and narrow-pointer
    /// skips likely). Every materialized result is invariant-validated.
    #[test]
    fn set_algebra_matches_btreeset(
        a_keys in prop::collection::vec(key_strategy(), 0..400),
        b_keys in prop::collection::vec(key_strategy(), 0..400),
    ) {
        let mut a = ExpanseSet::new();
        let mut ma = BTreeSet::new();
        for &k in &a_keys { a.insert(k); ma.insert(k); }
        let mut b = ExpanseSet::new();
        let mut mb = BTreeSet::new();
        for &k in &b_keys { b.insert(k); mb.insert(k); }

        let inter = ma.intersection(&mb).count() as u64;
        prop_assert_eq!(a.intersection_len(&b), inter, "intersection_len");
        prop_assert_eq!(b.intersection_len(&a), inter, "intersection_len rev");
        prop_assert_eq!(a.union_len(&b), ma.union(&mb).count() as u64, "union_len");
        prop_assert_eq!(a.difference_len(&b), ma.difference(&mb).count() as u64, "difference_len");
        prop_assert_eq!(
            a.symmetric_difference_len(&b),
            ma.symmetric_difference(&mb).count() as u64,
            "symmetric_difference_len"
        );

        let inter_set = &a & &b;
        inter_set.validate();
        prop_assert!(inter_set.iter().eq(ma.intersection(&mb).copied()), "intersection");
        let union_set = &a | &b;
        union_set.validate();
        prop_assert!(union_set.iter().eq(ma.union(&mb).copied()), "union");
        let diff_set = &a - &b;
        diff_set.validate();
        prop_assert!(diff_set.iter().eq(ma.difference(&mb).copied()), "difference");
        let sym_set = &a ^ &b;
        sym_set.validate();
        prop_assert!(
            sym_set.iter().eq(ma.symmetric_difference(&mb).copied()),
            "symmetric_difference"
        );

        // Self-ops: A∩A = A∪A = A, A\A = A△A = ∅.
        let self_and = &a & &a;
        self_and.validate();
        prop_assert!(self_and.iter().eq(ma.iter().copied()), "self intersection");
        let self_diff = &a - &a;
        self_diff.validate();
        prop_assert_eq!(self_diff.len(), 0, "self difference empty");

        // A materialized result stays a valid, mutable tree.
        let mut mutd = union_set;
        for &k in a_keys.iter().take(4) { mutd.remove(k); }
        mutd.insert(u64::MAX);
        mutd.validate();
    }

    /// k-way aggregate set algebra (#610) matches `BTreeSet` for both
    /// cardinality and materialization on k in 1..=6 key streams.
    #[test]
    fn kway_set_algebra_matches_btreeset(
        sets_keys in prop::collection::vec(prop::collection::vec(key_strategy(), 0..200), 1..6),
    ) {
        let mut expanse_sets = Vec::with_capacity(sets_keys.len());
        let mut btree_sets = Vec::with_capacity(sets_keys.len());

        for keys in &sets_keys {
            let mut s = ExpanseSet::new();
            let mut m = BTreeSet::new();
            for &k in keys {
                s.insert(k);
                m.insert(k);
            }
            s.validate();
            expanse_sets.push(s);
            btree_sets.push(m);
        }

        let s_refs: Vec<&ExpanseSet> = expanse_sets.iter().collect();

        let mut model_and = btree_sets[0].clone();
        for bs in &btree_sets[1..] {
            model_and = model_and.intersection(bs).copied().collect();
        }

        let mut model_or = BTreeSet::new();
        for bs in &btree_sets {
            model_or = model_or.union(bs).copied().collect();
        }

        prop_assert_eq!(
            ExpanseSet::intersection_len_many(&s_refs),
            model_and.len() as u64,
            "intersection_len_many"
        );
        prop_assert_eq!(
            ExpanseSet::union_len_many(&s_refs),
            model_or.len() as u64,
            "union_len_many"
        );

        let res_and = ExpanseSet::intersection_many(&s_refs);
        res_and.validate();
        prop_assert!(res_and.iter().eq(model_and.iter().copied()), "intersection_many");

        let res_or = ExpanseSet::union_many(&s_refs);
        res_or.validate();
        prop_assert!(res_or.iter().eq(model_or.iter().copied()), "union_many");
    }

    /// `from_sorted_iter` bulk-build equals key-by-key insertion in content and
    /// passes the invariants validator, on the interesting-region strategy.
    #[test]
    fn from_sorted_iter_matches_insert(keys in prop::collection::vec(key_strategy(), 0..500)) {
        let mut sorted: Vec<u64> = keys.clone();
        sorted.sort_unstable();
        sorted.dedup();

        let built = ExpanseSet::from_sorted_iter(sorted.iter().copied());
        built.validate();
        prop_assert!(built.iter().eq(sorted.iter().copied()), "from_sorted_iter contents");

        let mut inserted = ExpanseSet::new();
        for &k in &keys { inserted.insert(k); }
        prop_assert!(built.iter().eq(inserted.iter()), "built vs inserted");
        // Unsorted input is corrected, not trusted.
        let shuffled = ExpanseSet::from_sorted_iter(keys.iter().copied());
        shuffled.validate();
        prop_assert!(shuffled.iter().eq(sorted.iter().copied()), "unsorted tolerated");
    }

    /// The stateful `advance_to` cursor (issue #340) matches a `BTreeSet`
    /// reference across an arbitrary interleaving of `advance_to` / `next`
    /// against a generated target stream — including non-monotone targets,
    /// which must never rewind the cursor. Driven for both set and map.
    #[test]
    fn cursor_advance_to_matches_model(
        keys in prop::collection::vec(key_strategy(), 0..300),
        targets in prop::collection::vec(key_strategy(), 0..200),
        start in prop::option::of(key_strategy()),
        next_mask in any::<u64>(),
    ) {
        let val_of = |k: u64| !k ^ 0x5EED;
        let mut set = ExpanseSet::new();
        let mut map = ExpanseMap::new();
        let mut model = BTreeSet::new();
        for &k in &keys {
            set.insert(k);
            map.insert(k, val_of(k));
            model.insert(k);
        }
        let sorted: Vec<u64> = model.iter().copied().collect();

        // Reference: `idx` is the current position, peeked one step ahead.
        let ref_idx = |from: Option<u64>| -> usize {
            match from {
                Some(s) => sorted.partition_point(|&k| k < s),
                None => 0,
            }
        };
        let mut idx = ref_idx(start);
        let (mut sc, mut mc) = match start {
            Some(s) => (set.cursor_from(s), map.cursor_from(s)),
            None => (set.cursor(), map.cursor()),
        };
        prop_assert_eq!(sc.current(), sorted.get(idx).copied(), "initial current");
        prop_assert_eq!(mc.current().map(|(k, _)| k), sorted.get(idx).copied());

        for (i, &t) in targets.iter().enumerate() {
            // Reference advance_to with never-rewind semantics.
            let expect = match sorted.get(idx).copied() {
                Some(k) if k >= t => Some(k),
                Some(_) => {
                    idx = sorted.partition_point(|&k| k < t);
                    sorted.get(idx).copied()
                }
                None => None,
            };
            prop_assert_eq!(sc.advance_to(t), expect, "advance_to {:#x} @ {}", t, i);
            let gm = mc.advance_to(t);
            prop_assert_eq!(gm.map(|(k, _)| k), expect, "map advance_to {:#x} @ {}", t, i);
            if let Some((k, v)) = gm {
                prop_assert_eq!(v, val_of(k), "value for {:#x}", k);
            }
            prop_assert_eq!(sc.current(), sorted.get(idx).copied(), "current @ {}", i);

            // Occasionally consume with `next`, tracked by a deterministic mask.
            if (next_mask >> (i % 64)) & 1 == 1 {
                let e = sorted.get(idx).copied();
                if e.is_some() {
                    idx += 1;
                }
                prop_assert_eq!(sc.next(), e, "next @ {}", i);
                prop_assert_eq!(mc.next().map(|(k, _)| k), e, "map next @ {}", i);
            }
        }
    }
}
