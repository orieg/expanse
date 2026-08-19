//! Fuzz target: `ExpanseSet` against a `BTreeSet` model
//! (`docs/TESTING.md` layer 4).
//!
//! The fuzzer decodes its input bytes into an op sequence and asserts
//! agreement on every operation, plus the structural validator and a
//! leak check at the end. What this catches that the seeded model tests
//! and proptest cannot: op *shapes* nobody thought to generate — the
//! coverage-guided engine finds the key patterns that reach rare
//! ladder transitions on its own.
//!
//! Run: `cargo +nightly fuzz run set_ops`

#![no_main]

use arbitrary::Arbitrary;
use expanse_trie::set::ExpanseSet;
use libfuzzer_sys::fuzz_target;
use std::collections::BTreeSet;

/// Keys are drawn from a small template set so the fuzzer spends its
/// budget on op *sequences* rather than on rediscovering that clustered
/// keys matter; the raw variant keeps full-width keys reachable.
#[derive(Arbitrary, Debug)]
enum Key {
    Dense(u8),
    ClusterA(u16),
    ClusterB(u16),
    Sparse(u8),
    Raw(u64),
}

impl Key {
    fn to_u64(&self) -> u64 {
        match *self {
            Key::Dense(b) => u64::from(b),
            Key::ClusterA(i) => 0xAABB_CCDD_EE00 + u64::from(i),
            Key::ClusterB(i) => 0x1122_3344_0000 + u64::from(i),
            Key::Sparse(b) => u64::from(b) << 40,
            Key::Raw(k) => k,
        }
    }
}

#[derive(Arbitrary, Debug)]
enum Op {
    Insert(Key),
    Remove(Key),
    Contains(Key),
    Audit,
}

fuzz_target!(|ops: Vec<Op>| {
    // Bound the work per input: the fuzzer is looking for shapes, not
    // scale, and long inputs starve the corpus.
    if ops.len() > 2048 {
        return;
    }
    let mut set = ExpanseSet::new();
    let mut model = BTreeSet::new();

    for op in &ops {
        match op {
            Op::Insert(k) => {
                let k = k.to_u64();
                assert_eq!(set.insert(k), model.insert(k), "insert {k:#x}");
            }
            Op::Remove(k) => {
                let k = k.to_u64();
                assert_eq!(set.remove(k), model.remove(&k), "remove {k:#x}");
            }
            Op::Contains(k) => {
                let k = k.to_u64();
                assert_eq!(set.contains(k), model.contains(&k), "contains {k:#x}");
            }
            Op::Audit => {
                set.validate();
                assert_eq!(set.len(), model.len() as u64);
                assert!(set.iter().eq(model.iter().copied()));
            }
        }
    }

    set.validate();
    assert!(set.iter().eq(model.iter().copied()), "final ordering");
    // Rank/select must agree with the model's ordering everywhere.
    for (n, &k) in model.iter().enumerate() {
        assert_eq!(set.count_below(k), n as u64, "rank {k:#x}");
        assert_eq!(set.by_count(n as u64), Some(k), "select {n}");
    }
    // Drain: every key removable exactly once, no bytes left behind.
    for &k in &model {
        assert!(set.remove(k), "drain {k:#x}");
    }
    assert!(set.is_empty());
    assert_eq!(set.mem_used(), 0, "leak after drain");
});
