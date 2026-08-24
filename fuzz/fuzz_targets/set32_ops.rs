//! Fuzz target: `ExpanseSet32` against a `BTreeSet<u32>` model
//! (`docs/TESTING.md` layer 4), mirroring `set_ops` for the 32-bit engine.
//!
//! The fuzzer decodes its input into an op sequence and asserts agreement
//! on every operation, then checks navigation, range counts, drain, and
//! leak-freedom. It exercises op *shapes* the seeded model tests don't
//! generate — the patterns that reach rare leaf/branch ladder transitions.
//!
//! Run: `cargo +nightly fuzz run set32_ops`

#![no_main]

use arbitrary::Arbitrary;
use expanse_trie::set32::ExpanseSet32;
use libfuzzer_sys::fuzz_target;
use std::collections::BTreeSet;

/// Keys drawn from a small template so the fuzzer spends its budget on op
/// *sequences*; the raw variant keeps full-width keys reachable.
#[derive(Arbitrary, Debug)]
enum Key {
    Dense(u8),
    ClusterA(u16),
    ClusterB(u16),
    Sparse(u8),
    Raw(u32),
}

impl Key {
    fn to_u32(&self) -> u32 {
        match *self {
            Key::Dense(b) => u32::from(b),
            Key::ClusterA(i) => 0x00AB_0000 + u32::from(i),
            Key::ClusterB(i) => 0x1234_0000 + u32::from(i),
            Key::Sparse(b) => u32::from(b) << 24,
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
    if ops.len() > 2048 {
        return;
    }
    let mut set = ExpanseSet32::new();
    let mut model = BTreeSet::new();

    for op in &ops {
        match op {
            Op::Insert(k) => {
                let k = k.to_u32();
                assert_eq!(set.insert(k), model.insert(k), "insert {k:#x}");
            }
            Op::Remove(k) => {
                let k = k.to_u32();
                assert_eq!(set.remove(k), model.remove(&k), "remove {k:#x}");
            }
            Op::Contains(k) => {
                let k = k.to_u32();
                assert_eq!(set.contains(k), model.contains(&k), "contains {k:#x}");
            }
            Op::Audit => {
                assert_eq!(set.len(), model.len());
                // Forward ordering must match the model exactly.
                let mut cur = set.first();
                let mut it = model.iter().copied();
                while let Some(k) = cur {
                    assert_eq!(Some(k), it.next(), "ordering");
                    cur = set.next(k);
                }
                assert_eq!(it.next(), None, "trailing model keys");
            }
        }
    }

    // Navigation + range agreement.
    let expected: Vec<u32> = model.iter().copied().collect();
    assert_eq!(set.first(), expected.first().copied());
    assert_eq!(set.last(), expected.last().copied());
    for &k in &expected {
        assert_eq!(set.next(k), expected.iter().copied().find(|&x| x > k));
        assert_eq!(set.prev(k), expected.iter().rev().copied().find(|&x| x < k));
    }
    assert_eq!(set.count_range(0, u32::MAX), expected.len());

    // Drain: every key removable once, no bytes left behind.
    for &k in &expected {
        assert!(set.remove(k), "drain {k:#x}");
    }
    assert!(set.is_empty());
    assert_eq!(set.mem_used(), 0, "leak after drain");
});
