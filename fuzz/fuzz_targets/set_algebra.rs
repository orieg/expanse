//! Fuzz target: `ExpanseSet` native set algebra against a `BTreeSet` model
//! (issue #339; `docs/TESTING.md` layer 4).
//!
//! The fuzzer decodes two key lists into two sets and asserts every
//! set-algebra kernel (cardinality + materialized result + operator sugar)
//! agrees with `BTreeSet`, validating the invariants of each materialized
//! result. The coverage-guided engine finds the shared-prefix / narrow-skip
//! alignments where the lockstep descent's rare branches live.
//!
//! Run: `cargo +nightly fuzz run set_algebra`

#![no_main]

use arbitrary::Arbitrary;
use expanse_trie::set::ExpanseSet;
use libfuzzer_sys::fuzz_target;
use std::collections::BTreeSet;

/// Keys biased toward overlap-prone regions so the two sets actually
/// intersect and share high-byte prefixes (where the lockstep descent's
/// subtree-skip and bitmap-`AND` paths are exercised).
#[derive(Arbitrary, Debug)]
enum Key {
    Dense(u8),
    ClusterA(u16),
    ClusterB(u16),
    Sparse(u8),
    /// A contiguous block; several of these fill whole level-1/level-2
    /// expanses, exercising the `FullExpanse` structural clone / complement
    /// paths of the direct-emission materializer (#348).
    Block(u16),
    Raw(u64),
}

impl Key {
    fn to_u64(&self) -> u64 {
        match *self {
            Key::Dense(b) => u64::from(b),
            Key::ClusterA(i) => 0xAABB_CCDD_EE00 + u64::from(i),
            Key::ClusterB(i) => 0x1122_3344_0000 + u64::from(i),
            Key::Sparse(b) => u64::from(b) << 40,
            Key::Block(i) => u64::from(i),
            Key::Raw(k) => k,
        }
    }
}

fn build(keys: &[Key]) -> (ExpanseSet, BTreeSet<u64>) {
    let mut s = ExpanseSet::new();
    let mut m = BTreeSet::new();
    for k in keys {
        let k = k.to_u64();
        s.insert(k);
        m.insert(k);
    }
    (s, m)
}

fuzz_target!(|lists: (Vec<Key>, Vec<Key>)| {
    let (a_keys, b_keys) = lists;
    if a_keys.len() > 2048 || b_keys.len() > 2048 {
        return;
    }
    let (a, ma) = build(&a_keys);
    let (b, mb) = build(&b_keys);

    let inter = ma.intersection(&mb).count() as u64;
    assert_eq!(a.intersection_len(&b), inter, "intersection_len");
    assert_eq!(b.intersection_len(&a), inter, "intersection_len rev");
    assert_eq!(a.union_len(&b), ma.union(&mb).count() as u64, "union_len");
    assert_eq!(
        a.difference_len(&b),
        ma.difference(&mb).count() as u64,
        "difference_len"
    );
    assert_eq!(
        a.symmetric_difference_len(&b),
        ma.symmetric_difference(&mb).count() as u64,
        "symmetric_difference_len"
    );

    let inter_set = &a & &b;
    inter_set.validate();
    assert!(
        inter_set.iter().eq(ma.intersection(&mb).copied()),
        "intersection"
    );
    let union_set = &a | &b;
    union_set.validate();
    assert!(union_set.iter().eq(ma.union(&mb).copied()), "union");
    let diff_set = &a - &b;
    diff_set.validate();
    assert!(
        diff_set.iter().eq(ma.difference(&mb).copied()),
        "difference"
    );
    let sym_set = &a ^ &b;
    sym_set.validate();
    assert!(
        sym_set.iter().eq(ma.symmetric_difference(&mb).copied()),
        "symmetric_difference"
    );

    // Bulk builder (#348): from_sorted_iter equals key-by-key insertion and
    // passes the validator; unsorted input is corrected.
    let sorted: Vec<u64> = ma.iter().copied().collect();
    let built = ExpanseSet::from_sorted_iter(sorted.iter().copied());
    built.validate();
    assert!(built.iter().eq(ma.iter().copied()), "from_sorted_iter");
    let raw: Vec<u64> = a_keys.iter().map(Key::to_u64).collect();
    let from_raw = ExpanseSet::from_sorted_iter(raw);
    from_raw.validate();
    assert!(from_raw.iter().eq(ma.iter().copied()), "from_sorted_iter unsorted");
});
