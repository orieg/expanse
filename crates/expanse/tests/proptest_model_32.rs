//! Randomized differential model for the **32-bit** engine (#763).
//!
//! `proptest_model.rs` — the oracle CI runs at `PROPTEST_CASES=500` on every
//! PR — imports only `ExpanseMap` and `ExpanseSet`. `trie32.rs` is thousands of
//! lines whose only differential coverage was `fuzz/fuzz_targets/map32_ops.rs`
//! and `set32_ops.rs`, which run under the coverage-guided `fuzz-smoke` job
//! rather than the deterministic per-PR sweep, and over a narrower op set.
//!
//! This is the 32-bit twin, sharing the 64-bit file's shape so the two stay
//! comparable: the same op mix, the same audit-on-a-schedule structure, the
//! same drain-to-empty leak check at the end.
//!
//! **One deliberate difference.** The 64-bit model calls `set.validate()` at
//! every audit; the 32-bit engines expose no `validate()` or `stats()`, so this
//! file asserts behaviour against the ordered model and nothing structural. It
//! is a weaker instrument than its twin, and saying so is better than implying
//! parity it does not have.

// Same exclusion as the 64-bit twin: proptest under Miri is far too slow to
// finish a shard, and `scripts/check_miri_shards.py` requires every integration
// target to be either in the nightly matrix or opted out here, by declaration.
#![cfg(not(miri))]

use expanse_trie::map32::ExpanseMap32;
use expanse_trie::set32::ExpanseSet32;
use proptest::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
enum Op {
    Insert(u32),
    Remove(u32),
    Get(u32),
    Audit,
}

/// Keys biased toward the 32-bit structure's interesting regions, mirroring the
/// 64-bit strategy scaled to four bytes: dense low runs, clusters sharing high
/// bytes, one populated digit per level, and the extremes.
fn key_strategy() -> impl Strategy<Value = u32> {
    prop_oneof![
        2 => 0u32..256,
        3 => (0u32..2).prop_flat_map(|c| {
            let base = if c == 0 { 0xAABB_0000u32 } else { 0x1122_0000u32 };
            (0u32..512).prop_map(move |i| base.wrapping_add(i))
        }),
        2 => (0u32..4096).prop_map(|i| 0x7777_0000u32.wrapping_add(i)),
        2 => (0u32..64).prop_map(|i| i << 24),
        1 => prop_oneof![Just(0u32), Just(u32::MAX), any::<u32>()],
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

fn run_set(ops: &[Op]) {
    let mut set = ExpanseSet32::new();
    let mut model = BTreeSet::new();
    for op in ops {
        match *op {
            Op::Insert(k) => assert_eq!(set.insert(k), model.insert(k), "insert {k:#x}"),
            Op::Remove(k) => assert_eq!(set.remove(k), model.remove(&k), "remove {k:#x}"),
            Op::Get(k) => assert_eq!(set.contains(k), model.contains(&k), "contains {k:#x}"),
            Op::Audit => {
                assert_eq!(set.len(), model.len(), "len");
                assert!(set.iter().eq(model.iter().copied()), "ordered iteration");
                assert!(
                    set.iter_rev().eq(model.iter().rev().copied()),
                    "reverse iteration"
                );
                assert_eq!(set.first(), model.iter().next().copied(), "first");
                assert_eq!(set.last(), model.iter().next_back().copied(), "last");
                // Cursor navigation, which the fuzz targets do not reach.
                for &probe in &[0u32, 1, 0x7FFF_FFFF, u32::MAX] {
                    // `next_after` is strict, so at u32::MAX there is no
                    // successor: `saturating_add` would make the model include
                    // the probe itself and disagree with a correct engine.
                    let strictly_after = probe
                        .checked_add(1)
                        .and_then(|lo| model.range(lo..).next().copied());
                    assert_eq!(
                        set.next_after(probe),
                        strictly_after,
                        "next_after {probe:#x}"
                    );
                    assert_eq!(
                        set.prev_before(probe),
                        model.range(..probe).next_back().copied(),
                        "prev_before {probe:#x}"
                    );
                }
            }
        }
    }
    assert!(
        set.iter().eq(model.iter().copied()),
        "final ordered equality"
    );
    for k in &model {
        assert!(set.remove(*k), "final drain {k:#x}");
    }
    assert!(set.is_empty());
    assert_eq!(set.mem_used(), 0, "leak after drain");
}

/// Map mirror: values are a function of the key, so a misplaced value is
/// detectable rather than merely absent.
fn run_map(ops: &[Op]) {
    let val = |k: u32| k.rotate_left(7) ^ 0xDEAD_BEEF;
    let mut map = ExpanseMap32::new();
    let mut model: BTreeMap<u32, u32> = BTreeMap::new();
    for op in ops {
        match *op {
            Op::Insert(k) => {
                assert_eq!(
                    map.insert(k, val(k)),
                    model.insert(k, val(k)),
                    "insert {k:#x}"
                )
            }
            Op::Remove(k) => assert_eq!(map.remove(k), model.remove(&k), "remove {k:#x}"),
            Op::Get(k) => assert_eq!(map.get(k), model.get(&k).copied(), "get {k:#x}"),
            Op::Audit => {
                assert_eq!(map.len(), model.len(), "len");
                assert!(
                    map.iter().eq(model.iter().map(|(k, v)| (*k, *v))),
                    "ordered iteration"
                );
                assert!(
                    map.iter_rev().eq(model.iter().rev().map(|(k, v)| (*k, *v))),
                    "reverse iteration"
                );
                for (k, v) in &model {
                    assert_eq!(map.get(*k), Some(*v), "value for {k:#x}");
                }
            }
        }
    }
    for (k, v) in &model {
        assert_eq!(map.remove(*k), Some(*v), "final drain {k:#x}");
    }
    assert!(map.is_empty());
    assert_eq!(map.mem_used(), 0, "leak after drain");
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: std::env::var("PROPTEST_CASES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(64),
        ..ProptestConfig::default()
    })]

    #[test]
    fn set32_matches_btreeset(ops in prop::collection::vec(op_strategy(), 1..400)) {
        run_set(&ops);
    }

    #[test]
    fn map32_matches_btreemap(ops in prop::collection::vec(op_strategy(), 1..400)) {
        run_map(&ops);
    }

    #[test]
    fn sync32_map_writer_matches_plain_map(ops in prop::collection::vec(shared_op_strategy(), 1..800)) {
        run_shared_map(&ops);
    }

    #[test]
    fn sync32_set_writer_matches_plain_set(ops in prop::collection::vec(shared_op_strategy(), 1..800)) {
        run_shared_set(&ops);
    }

    #[test]
    fn set32_from_sorted_iter_matches_insert_and_model(keys in prop::collection::vec(key_strategy(), 0..500)) {
        let model: BTreeSet<u32> = keys.iter().copied().collect();
        let built = ExpanseSet32::from_sorted_iter(keys.iter().copied());
        assert_eq!(built.len(), model.len());
        assert!(built.iter().eq(model.iter().copied()));
        assert_eq!(built.live_allocs(), built.total_node_allocs());

        let mut inserted = ExpanseSet32::new();
        for &k in &model {
            inserted.insert(k);
        }
        assert_eq!(built.mem_used(), inserted.mem_used());
        assert_eq!(built.node_census(), inserted.node_census());
    }

    #[test]
    fn map32_from_sorted_iter_matches_insert_and_model(entries in prop::collection::vec((key_strategy(), any::<u32>()), 0..500)) {
        let mut model = BTreeMap::new();
        for &(k, v) in &entries {
            model.insert(k, v);
        }
        let built = ExpanseMap32::from_sorted_iter(entries.iter().copied());
        assert_eq!(built.len(), model.len());
        assert!(built.iter().eq(model.iter().map(|(&k, &v)| (k, v))));
        assert_eq!(built.live_allocs(), built.total_node_allocs());

        let mut inserted = ExpanseMap32::new();
        for (&k, &v) in &model {
            inserted.insert(k, v);
        }
        assert_eq!(built.mem_used(), inserted.mem_used());
        assert_eq!(built.node_census(), inserted.node_census());
    }
}

/// Keys for the shared-writer model (#1233): two full level-1 expanses (256
/// keys each, dense enough to promote a leaf past `MAP_BITMAP_ENTER_32` and
/// `SET_BITMAP_ENTER_32` and to demote it back), plus the general strategy,
/// so bitmap-leaf key-set edits, subarray class crossings and demotions all
/// occur.
fn shared_key_strategy() -> impl Strategy<Value = u32> {
    prop_oneof![
        4 => (0u32..256).prop_map(|i| 0x0102_0300 | i),
        3 => (0u32..256).prop_map(|i| 0xC0DE_0000 | i),
        2 => key_strategy(),
    ]
}

fn shared_op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        5 => shared_key_strategy().prop_map(Op::Insert),
        3 => shared_key_strategy().prop_map(Op::Remove),
        1 => shared_key_strategy().prop_map(Op::Get),
        1 => Just(Op::Audit),
    ]
}

/// The concurrent wrapper's writer runs the shared instantiation of the walks
/// (in-place word stores into bitmap leaves, #1233); the plain container runs
/// the plain one. Every answer, the ordered contents as a reader walks them,
/// and the byte accounting must agree at every audit.
fn run_shared_map(ops: &[Op]) {
    use expanse_trie::sync32::SyncExpanseMap32;
    let val = |k: u32| k.rotate_left(11) ^ 0x5A5A_A5A5;
    let mut plain = ExpanseMap32::new();
    let mut shared = SyncExpanseMap32::with_capacity(8_192, 1);
    let (mut w, mut pool) = shared.split();
    let mut r = pool.take().expect("reader slot");
    for (i, op) in ops.iter().enumerate() {
        match *op {
            Op::Insert(k) => {
                // Alternate the value so overwrites change it.
                let v = val(k) ^ (i as u32 & 1);
                let got = w.try_insert(k, v).expect("insert");
                assert_eq!(got, plain.insert(k, v), "insert {k:#x}");
            }
            Op::Remove(k) => {
                let got = w.try_remove(k).expect("remove");
                assert_eq!(got, plain.remove(k), "remove {k:#x}");
            }
            Op::Get(k) => {
                let got = r.try_get(k).expect("no writer runs");
                assert_eq!(got, plain.get(k), "get {k:#x}");
            }
            Op::Audit => {
                assert!(w.try_reclaim(), "no reader is pinned");
                assert_eq!(w.len(), plain.len(), "len");
                assert_eq!(w.mem_used(), plain.mem_used(), "mem_used");
                let mut walked = Vec::new();
                let mut next = r.try_first().expect("no writer runs");
                while let Some((k, v)) = next {
                    walked.push((k, v));
                    next = r.try_next_after(k).expect("no writer runs");
                }
                assert!(walked.iter().copied().eq(plain.iter()), "ordered contents");
            }
        }
    }
    let keys: Vec<u32> = plain.iter().map(|(k, _)| k).collect();
    for k in keys {
        let got = w.try_remove(k).expect("drain");
        assert_eq!(got, plain.remove(k), "drain {k:#x}");
        assert!(w.try_reclaim(), "no reader is pinned");
    }
    assert!(w.is_empty());
    assert_eq!(w.mem_used(), 0, "leak after drain");
}

/// Set mirror of [`run_shared_map`].
fn run_shared_set(ops: &[Op]) {
    use expanse_trie::sync32::SyncExpanseSet32;
    let mut plain = ExpanseSet32::new();
    let mut shared = SyncExpanseSet32::with_capacity(8_192, 1);
    let (mut w, mut pool) = shared.split();
    let mut r = pool.take().expect("reader slot");
    for op in ops {
        match *op {
            Op::Insert(k) => {
                let got = w.try_insert(k).expect("insert");
                assert_eq!(got, plain.insert(k), "insert {k:#x}");
            }
            Op::Remove(k) => {
                let got = w.try_remove(k).expect("remove");
                assert_eq!(got, plain.remove(k), "remove {k:#x}");
            }
            Op::Get(k) => {
                let got = r.try_contains(k).expect("no writer runs");
                assert_eq!(got, plain.contains(k), "contains {k:#x}");
            }
            Op::Audit => {
                assert!(w.try_reclaim(), "no reader is pinned");
                assert_eq!(w.len(), plain.len(), "len");
                assert_eq!(w.mem_used(), plain.mem_used(), "mem_used");
                for k in plain.iter() {
                    assert!(r.try_contains(k).expect("no writer runs"), "member {k:#x}");
                }
            }
        }
    }
    let keys: Vec<u32> = plain.iter().collect();
    for k in keys {
        let got = w.try_remove(k).expect("drain");
        assert_eq!(got, plain.remove(k), "drain {k:#x}");
        assert!(w.try_reclaim(), "no reader is pinned");
    }
    assert!(w.is_empty());
    assert_eq!(w.mem_used(), 0, "leak after drain");
}
