//! Fuzz target: `ExpanseMap` against a `BTreeMap` model
//! (`docs/TESTING.md` layer 4). See `set_ops.rs` for the approach.
//!
//! Values are carried explicitly (not derived from the key) so a
//! misplaced value — the failure mode a set-flavor target structurally
//! cannot see — shows up as a mismatch rather than as a silent pass.
//!
//! Run: `cargo +nightly fuzz run map_ops`

#![no_main]

use arbitrary::Arbitrary;
use expanse_trie::map::ExpanseMap;
use libfuzzer_sys::fuzz_target;
use std::collections::BTreeMap;

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
    Insert(Key, u64),
    Remove(Key),
    Get(Key),
    /// The JudyL slot contract: insert-if-absent, then write through the
    /// returned pointer.
    InsSlotWrite(Key, u64),
    Audit,
}

fuzz_target!(|ops: Vec<Op>| {
    if ops.len() > 2048 {
        return;
    }
    let mut map = ExpanseMap::new();
    let mut model = BTreeMap::new();

    for op in &ops {
        match op {
            Op::Insert(k, v) => {
                let k = k.to_u64();
                assert_eq!(map.insert(k, *v), model.insert(k, *v), "insert {k:#x}");
            }
            Op::Remove(k) => {
                let k = k.to_u64();
                assert_eq!(map.remove(k), model.remove(&k), "remove {k:#x}");
            }
            Op::Get(k) => {
                let k = k.to_u64();
                assert_eq!(map.get(k), model.get(&k).copied(), "get {k:#x}");
            }
            Op::InsSlotWrite(k, v) => {
                let k = k.to_u64();
                let slot = map.ins_slot(k);
                // SAFETY: the slot is valid until the next mutation, and
                // nothing mutates between here and the write.
                unsafe { slot.as_ptr().write(*v) };
                model.insert(k, *v);
            }
            Op::Audit => {
                map.validate();
                assert_eq!(map.len(), model.len() as u64);
                assert!(map.iter().eq(model.iter().map(|(k, v)| (*k, *v))));
            }
        }
    }

    map.validate();
    assert!(
        map.iter().eq(model.iter().map(|(k, v)| (*k, *v))),
        "final ordering"
    );
    for (n, (&k, &v)) in model.iter().enumerate() {
        assert_eq!(map.count_below(k), n as u64, "rank {k:#x}");
        assert_eq!(map.by_count(n as u64), Some((k, v)), "select {n}");
    }
    let entries: Vec<(u64, u64)> = model.into_iter().collect();
    for (k, v) in entries {
        assert_eq!(map.remove(k), Some(v), "drain {k:#x}");
    }
    assert!(map.is_empty());
    assert_eq!(map.mem_used(), 0, "leak after drain");
});
