//! Fuzz target: `ExpanseMap32` against a `BTreeMap<u32, u32>` model
//! (`docs/TESTING.md` layer 4), mirroring `map_ops` for the 32-bit engine.
//!
//! Decodes its input into an op sequence, asserts agreement on every
//! operation, then checks navigation, range counts, drain, and
//! leak-freedom — exercising leaf/branch ladder transitions the seeded
//! model tests do not reach.
//!
//! Run: `cargo +nightly fuzz run map32_ops`

#![no_main]

use arbitrary::Arbitrary;
use expanse_trie::map32::ExpanseMap32;
use libfuzzer_sys::fuzz_target;
use std::collections::BTreeMap;

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
    Insert(Key, u32),
    Remove(Key),
    Get(Key),
    Audit,
}

fuzz_target!(|ops: Vec<Op>| {
    if ops.len() > 2048 {
        return;
    }
    let mut map = ExpanseMap32::new();
    let mut model = BTreeMap::new();

    for op in &ops {
        match op {
            Op::Insert(k, v) => {
                let k = k.to_u32();
                assert_eq!(map.insert(k, *v), model.insert(k, *v), "insert {k:#x}");
            }
            Op::Remove(k) => {
                let k = k.to_u32();
                assert_eq!(map.remove(k), model.remove(&k), "remove {k:#x}");
            }
            Op::Get(k) => {
                let k = k.to_u32();
                assert_eq!(map.get(k), model.get(&k).copied(), "get {k:#x}");
            }
            Op::Audit => {
                assert_eq!(map.len(), model.len());
                let mut cur = map.first();
                let mut it = model.iter().map(|(&k, &v)| (k, v));
                while let Some((k, v)) = cur {
                    assert_eq!(Some((k, v)), it.next(), "ordering");
                    cur = map.next(k);
                }
                assert_eq!(it.next(), None, "trailing model entries");
            }
        }
    }

    // Navigation + range agreement.
    let expected: Vec<(u32, u32)> = model.iter().map(|(&k, &v)| (k, v)).collect();
    assert_eq!(map.first(), expected.first().copied());
    assert_eq!(map.last(), expected.last().copied());
    for &(k, _) in &expected {
        assert_eq!(map.next(k), expected.iter().copied().find(|&(x, _)| x > k));
        assert_eq!(
            map.prev(k),
            expected.iter().rev().copied().find(|&(x, _)| x < k)
        );
    }
    assert_eq!(map.count_range(0, u32::MAX), expected.len());

    // Drain: every key removable once, no bytes left behind.
    for &(k, v) in &expected {
        assert_eq!(map.remove(k), Some(v), "drain {k:#x}");
    }
    assert!(map.is_empty());
    assert_eq!(map.mem_used(), 0, "leak after drain");
});
