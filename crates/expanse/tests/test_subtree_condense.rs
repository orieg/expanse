//! Subtree condensation on remove (`subtree-condense`,
//! `docs/benchmarks/remove_retention/METHODOLOGY.md` §5).
//!
//! Runs only with the feature on; `cargo test -p expanse-trie --features
//! subtree-condense` (the `H1` arm) or `--features subtree-condense-wide`.
//! Every test runs the structural validator and compares contents with a
//! `BTreeSet` / `BTreeMap` model.
//!
//! Excluded from Miri: the drains build tens of thousands of keys and the
//! shared-tree test spawns threads. The condense module's own unit tests are
//! the Miri-sized ones.
#![cfg(not(miri))]
#![cfg(feature = "subtree-condense")]

use expanse_trie::condense::{THRESHOLD, THRESHOLD_H1, THRESHOLD_WIDE, is_evaluation_point};
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use expanse_trie::sync::{SyncExpanseMap, SyncExpanseSet};
use expanse_trie::types::LEAF_CAP;
use proptest::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// Keys under other top bytes, so the tree stays a tree (not a root leaf)
/// and the level-8 node is never the one under test.
fn fillers(n: usize) -> Vec<u64> {
    let mut rng = XorShift(0x5EED_F111_E500_0001);
    (0..n)
        .map(|_| (0x10u64 + rng.next() % 0xE0) << 56 | (rng.next() >> 8))
        .collect()
}

fn check_set(s: &ExpanseSet, model: &BTreeSet<u64>) {
    s.validate();
    assert_eq!(s.len(), model.len() as u64);
    assert!(
        s.iter().eq(model.iter().copied()),
        "set contents differ from the model"
    );
}

fn check_map(m: &ExpanseMap, model: &BTreeMap<u64, u64>) {
    m.validate();
    assert_eq!(m.len(), model.len() as u64);
    assert!(
        m.iter().eq(model.iter().map(|(&k, &v)| (k, v))),
        "map contents differ from the model"
    );
}

#[test]
fn thresholds_are_derived_and_the_arm_is_selected_by_feature() {
    assert_eq!(THRESHOLD_H1, LEAF_CAP - 1);
    assert_eq!(THRESHOLD_WIDE, LEAF_CAP - 8);
    let expected = if cfg!(feature = "subtree-condense-wide") {
        THRESHOLD_WIDE
    } else {
        THRESHOLD_H1
    };
    assert_eq!(THRESHOLD, expected);
    // METHODOLOGY §5.2: H1 evaluates at 31, 24, 16, 12, 8, 4, 2, 1; wide at
    // 24, 16, 12, 8, 4, 2, 1.
    let points: Vec<usize> = (1..=LEAF_CAP).filter(|&p| is_evaluation_point(p)).collect();
    let want: &[usize] = if THRESHOLD == THRESHOLD_H1 {
        &[1, 2, 4, 8, 12, 16, 24, 31]
    } else {
        &[1, 2, 4, 8, 12, 16, 24]
    };
    assert_eq!(points, want);
}

/// Level-6 expanse `e`: 2-byte prefix `0x00_e`, a distinct third byte per
/// key, so a cascade is a `BranchB` of single-key children.
fn expanse_keys(e: u64, n: usize) -> Vec<u64> {
    (0..n as u64)
        .map(|j| (e << 48) | (((j * 37 + e) & 0xFF) << 40) | (j * 0x1_0101))
        .collect()
}

#[test]
fn a_cascaded_expanse_condenses_at_the_threshold() {
    let mut s = ExpanseSet::new();
    let mut model = BTreeSet::new();
    for k in fillers(2_000) {
        s.insert(k);
        model.insert(k);
    }
    let ks = expanse_keys(7, LEAF_CAP + 1);
    for &k in &ks {
        s.insert(k);
        model.insert(k);
    }
    let before = s.stats();
    assert!(
        before.node_counts.branch_b >= 1,
        "33 keys cascade the expanse"
    );
    // Drain to one above the threshold: still a branch.
    for &k in ks[THRESHOLD + 1..].iter().rev() {
        assert!(s.remove(k));
        model.remove(&k);
    }
    check_set(&s, &model);
    assert_eq!(s.stats().leaf_pop_histogram[THRESHOLD + 1], 0);
    // The next removal lands on T, an evaluation point, and the packed leaf
    // (6 × cap_class(T) bytes) is smaller than the branch over T immediates.
    let used = s.mem_used();
    assert!(s.remove(ks[THRESHOLD]));
    model.remove(&ks[THRESHOLD]);
    check_set(&s, &model);
    let after = s.stats();
    assert_eq!(
        after.leaf_pop_histogram[THRESHOLD], 1,
        "condensed into one leaf"
    );
    assert_eq!(after.node_counts.branch_b + 1, before.node_counts.branch_b);
    assert!(s.mem_used() < used);
}

#[test]
fn map_values_survive_a_condense() {
    let mut m = ExpanseMap::new();
    let mut model = BTreeMap::new();
    for k in fillers(2_000) {
        m.insert(k, !k);
        model.insert(k, !k);
    }
    let ks = expanse_keys(9, LEAF_CAP + 1);
    for &k in &ks {
        m.insert(k, k.rotate_left(17));
        model.insert(k, k.rotate_left(17));
    }
    for &k in ks[THRESHOLD..].iter().rev() {
        assert_eq!(m.remove(k), Some(k.rotate_left(17)));
        model.remove(&k);
    }
    check_map(&m, &model);
    assert_eq!(m.stats().leaf_pop_histogram[THRESHOLD], 1);
    for &k in &ks[..THRESHOLD] {
        assert_eq!(m.get(k), Some(k.rotate_left(17)));
    }
}

/// The negative control for the byte rule (METHODOLOGY §4, §5.1): a branch
/// reached through a skip edge, whose fresh leaf would be built at the
/// parent's child level and so be wider. 16 keys (an evaluation point) under
/// one level-6 slot, sharing three more bytes, held as a `BranchL3` over
/// three level-2 immediates (7 + 7 + 2 keys): 64 B. Packed they would be a
/// `Leaf6` of 16 keys, 96 B. The rule must decline, and the drained tree must
/// stay smaller than a fresh build of the same keys.
#[test]
fn the_byte_rule_declines_a_condense_that_would_grow_memory() {
    let base = (0x00FFu64 << 48) | (0xABCDEFu64 << 24);
    let group =
        |g: u64, n: u64| -> Vec<u64> { (0..n).map(|j| base | (g << 16) | (j * 257 + 1)).collect() };
    let mut s = ExpanseSet::new();
    let mut model = BTreeSet::new();
    for k in fillers(2_000) {
        s.insert(k);
        model.insert(k);
    }
    let groups: Vec<Vec<u64>> = (1..=3).map(|g| group(g, 11)).collect();
    for ks in &groups {
        for &k in ks {
            s.insert(k);
            model.insert(k);
        }
    }
    // Drain to 7 + 7 + 2 = 16, crossing 31 and 24 on the way.
    let keep = [7usize, 7, 2];
    for (ks, &n) in groups.iter().zip(&keep) {
        for &k in ks[n..].iter().rev() {
            assert!(s.remove(k));
            model.remove(&k);
        }
    }
    check_set(&s, &model);
    assert!(is_evaluation_point(16));
    let drained = s.stats();
    assert!(drained.node_counts.branch_l3 >= 1, "the branch is kept");
    assert_eq!(
        drained.leaf_pop_histogram[16], 0,
        "no 16-key leaf was built"
    );

    let mut fresh = ExpanseSet::new();
    for &k in &model {
        fresh.insert(k);
    }
    fresh.validate();
    assert_eq!(
        fresh.stats().leaf_pop_histogram[16],
        1,
        "a fresh build is the wider leaf"
    );
    assert!(
        s.mem_used() < fresh.mem_used(),
        "drained {} B must stay below fresh {} B",
        s.mem_used(),
        fresh.mem_used()
    );
    // The sweep applies the same rule.
    s.shrink_to_fit();
    check_set(&s, &model);
    assert_eq!(s.stats().leaf_pop_histogram[16], 0);
    assert!(s.mem_used() < fresh.mem_used());
}

#[test]
fn a_uniform_drain_reaches_the_fresh_build_after_shrink() {
    // The phase 3 Callgrind arm's shape: 200,000 keys at 60 bits drained to
    // 62,500 (λ 48.8 → 15.3 per 2-byte expanse).
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut all = Vec::new();
    let mut seen = BTreeSet::new();
    while all.len() < 200_000 {
        let k = rng.next() & ((1u64 << 60) - 1);
        if seen.insert(k) {
            all.push(k);
        }
    }
    let mut s = ExpanseSet::new();
    let mut m = ExpanseMap::new();
    for &k in &all {
        s.insert(k);
        m.insert(k, !k);
    }
    let mut order = all.clone();
    for i in (1..order.len()).rev() {
        order.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    for &k in &order[..137_500] {
        assert!(s.remove(k));
        assert_eq!(m.remove(k), Some(!k));
    }
    let model: BTreeSet<u64> = order[137_500..].iter().copied().collect();
    let mmodel: BTreeMap<u64, u64> = model.iter().map(|&k| (k, !k)).collect();
    check_set(&s, &model);
    check_map(&m, &mmodel);
    let (used_s, used_m) = (s.mem_used(), m.mem_used());
    s.shrink_to_fit();
    m.shrink_to_fit();
    check_set(&s, &model);
    check_map(&m, &mmodel);
    assert!(s.mem_used() <= used_s && m.mem_used() <= used_m);
}

#[derive(Debug, Clone)]
enum Op {
    Insert(u64),
    Remove(u64),
    Shrink,
}

/// Keys concentrated in four level-6 expanses, so a few hundred operations
/// cascade and drain them repeatedly.
fn key() -> impl Strategy<Value = u64> {
    (0u64..4, 0u64..48, any::<u16>())
        .prop_map(|(e, third, low)| ((0x40 + e) << 48) | (third << 40) | u64::from(low))
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        5 => key().prop_map(Op::Insert),
        5 => key().prop_map(Op::Remove),
        1 => Just(Op::Shrink),
    ]
}

fn run_ops(ops: &[Op]) {
    let mut s = ExpanseSet::new();
    let mut m = ExpanseMap::new();
    let mut model = BTreeSet::new();
    for k in fillers(64) {
        s.insert(k);
        m.insert(k, k ^ 0x55);
        model.insert(k);
    }
    for (i, op) in ops.iter().enumerate() {
        match *op {
            Op::Insert(k) => {
                assert_eq!(s.insert(k), model.insert(k));
                m.insert(k, k ^ 0x55);
            }
            Op::Remove(k) => {
                let was = model.remove(&k);
                assert_eq!(s.remove(k), was);
                assert_eq!(m.remove(k), was.then_some(k ^ 0x55));
            }
            Op::Shrink => {
                s.shrink_to_fit();
                m.shrink_to_fit();
            }
        }
        if i % 16 == 0 {
            s.validate();
            m.validate();
        }
    }
    check_set(&s, &model);
    let mmodel: BTreeMap<u64, u64> = model.iter().map(|&k| (k, k ^ 0x55)).collect();
    check_map(&m, &mmodel);
}

/// Oscillation across the band of METHODOLOGY §7.3: an expanse cascaded at
/// `LEAF_CAP + 1`, drained by `band` keys and refilled, `cycles` times.
fn run_oscillation(band: usize, cycles: usize) {
    let mut s = ExpanseSet::new();
    let mut m = ExpanseMap::new();
    let mut model = BTreeSet::new();
    for k in fillers(256) {
        s.insert(k);
        m.insert(k, k);
        model.insert(k);
    }
    let ks = expanse_keys(3, LEAF_CAP + 1);
    for &k in &ks {
        s.insert(k);
        m.insert(k, k);
        model.insert(k);
    }
    for _ in 0..cycles {
        for &k in ks[ks.len() - band..].iter().rev() {
            assert!(s.remove(k));
            assert_eq!(m.remove(k), Some(k));
            model.remove(&k);
            s.validate();
            m.validate();
        }
        for &k in &ks[ks.len() - band..] {
            assert!(s.insert(k));
            m.insert(k, k);
            model.insert(k);
            s.validate();
            m.validate();
        }
    }
    check_set(&s, &model);
    let mmodel: BTreeMap<u64, u64> = model.iter().map(|&k| (k, k)).collect();
    check_map(&m, &mmodel);
}

#[test]
fn oscillation_across_both_bands_keeps_the_tree_valid() {
    run_oscillation(2, 6);
    run_oscillation(9, 6);
    run_oscillation(LEAF_CAP, 3);
}

proptest! {
    #[test]
    fn random_operations_match_the_model(ops in prop::collection::vec(op(), 0..600)) {
        run_ops(&ops);
    }

    #[test]
    fn drain_to_m_matches_the_model(n in 40usize..400, m_frac in 0.0f64..1.0, seed in any::<u64>()) {
        let mut rng = XorShift(seed | 1);
        let ks: Vec<u64> = (0..n).map(|_| (0x40 + rng.next() % 4) << 48 | (rng.next() >> 16)).collect();
        let mut s = ExpanseSet::new();
        let mut m = ExpanseMap::new();
        let mut model = BTreeSet::new();
        for k in fillers(64) {
            s.insert(k);
            m.insert(k, k);
            model.insert(k);
        }
        for &k in &ks {
            s.insert(k);
            m.insert(k, k);
            model.insert(k);
        }
        let mut order: Vec<u64> = ks.iter().copied().collect::<BTreeSet<_>>().into_iter().collect();
        for i in (1..order.len()).rev() {
            order.swap(i, (rng.next() % (i as u64 + 1)) as usize);
        }
        let drop_n = (order.len() as f64 * m_frac) as usize;
        for &k in &order[..drop_n] {
            prop_assert!(s.remove(k));
            prop_assert_eq!(m.remove(k), Some(k));
            model.remove(&k);
        }
        check_set(&s, &model);
        let mmodel: BTreeMap<u64, u64> = model.iter().map(|&k| (k, k)).collect();
        check_map(&m, &mmodel);
        s.shrink_to_fit();
        m.shrink_to_fit();
        check_set(&s, &model);
        check_map(&m, &mmodel);
    }
}

/// A shared tree: optimistic removes never condense, `shrink_to_fit` folds
/// and condenses, and readers running across the sweep see every key that
/// was never removed.
#[test]
fn shared_trees_condense_in_shrink_to_fit_under_readers() {
    let set = Arc::new(SyncExpanseSet::new());
    let map = Arc::new(SyncExpanseMap::new());
    for k in fillers(2_000) {
        set.insert(k);
        map.insert(k, !k);
    }
    let mut drained = Vec::new();
    let mut kept = Vec::new();
    for e in 0..64u64 {
        let ks = expanse_keys(0x20 + e, LEAF_CAP + 1);
        for &k in &ks {
            set.insert(k);
            map.insert(k, !k);
        }
        drained.extend_from_slice(&ks[8..]);
        kept.extend_from_slice(&ks[..8]);
    }
    let stop = Arc::new(AtomicBool::new(false));
    let readers: Vec<_> = (0..2)
        .map(|_| {
            let (set, map, stop, kept) = (set.clone(), map.clone(), stop.clone(), kept.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    for &k in &kept {
                        assert!(set.contains(k), "a kept key vanished from the set");
                        assert_eq!(map.get(k), Some(!k), "a kept key vanished from the map");
                    }
                }
            })
        })
        .collect();
    for &k in &drained {
        assert!(set.remove(k));
        assert_eq!(map.remove(k), Some(!k));
    }
    let used_before = (
        set.with_locked(ExpanseSet::mem_used),
        map.with_locked(ExpanseMap::mem_used),
    );
    set.shrink_to_fit();
    map.shrink_to_fit();
    stop.store(true, Ordering::Relaxed);
    for r in readers {
        r.join().expect("reader");
    }
    set.with_locked(ExpanseSet::validate);
    map.with_locked(ExpanseMap::validate);
    let used_after = (
        set.with_locked(ExpanseSet::mem_used),
        map.with_locked(ExpanseMap::mem_used),
    );
    assert!(
        used_after.0 < used_before.0,
        "the set sweep condensed nothing"
    );
    assert!(
        used_after.1 < used_before.1,
        "the map sweep condensed nothing"
    );
    for &k in &kept {
        assert!(set.contains(k));
        assert_eq!(map.get(k), Some(!k));
    }
    for &k in &drained {
        assert!(!set.contains(k));
        assert_eq!(map.get(k), None);
    }
}

/// The serialised removal condenses only when the key's top digit is clean
/// (METHODOLOGY §5.3). Needs `diag-entry` for the forced serialised route.
#[cfg(feature = "diag-entry")]
#[test]
fn serialised_removal_condenses_under_a_clean_top_digit() {
    let set = SyncExpanseSet::new();
    for k in fillers(2_000) {
        set.insert(k);
    }
    let ks = expanse_keys(0x07, LEAF_CAP + 1);
    for &k in &ks {
        set.insert(k);
    }
    // Settle every dirty digit the optimistic inserts left, so the top
    // digit is clean for the serialised removals below.
    set.shrink_to_fit();
    let before = set.with_locked(|s| s.stats());
    for &k in ks[THRESHOLD..].iter().rev() {
        assert!(set.remove_serialized(k));
    }
    let after = set.with_locked(|s| {
        s.validate();
        s.stats()
    });
    assert_eq!(
        after.leaf_pop_histogram[THRESHOLD], 1,
        "condensed on the serialised route"
    );
    assert_eq!(after.node_counts.branch_b + 1, before.node_counts.branch_b);
}
