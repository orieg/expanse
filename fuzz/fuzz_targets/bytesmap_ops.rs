//! Fuzz target: `ExpanseBytesMap` against a `HashMap` model
//! (`docs/TESTING.md` layer 4).
//!
//! Byte-string keys are where a fuzzer earns its keep: arbitrary lengths,
//! embedded NULs, shared prefixes and hash collisions arrive for free
//! from the mutation engine, and the bucket paths only real collisions
//! reach get exercised without hand-built adversarial input.
//!
//! Run: `cargo +nightly fuzz run bytesmap_ops`

#![no_main]

use arbitrary::Arbitrary;
use expanse_trie::bytesmap::ExpanseBytesMap;
use libfuzzer_sys::fuzz_target;
use std::collections::HashMap;

#[derive(Arbitrary, Debug)]
enum Op {
    Insert(Vec<u8>, u64),
    Remove(Vec<u8>),
    Get(Vec<u8>),
    InsSlotWrite(Vec<u8>, u64),
    Audit,
}

fuzz_target!(|ops: Vec<Op>| {
    if ops.len() > 1024 {
        return;
    }
    let mut map = ExpanseBytesMap::new();
    let mut model: HashMap<Vec<u8>, u64> = HashMap::new();

    for op in &ops {
        match op {
            Op::Insert(k, v) => {
                if k.len() > 4096 {
                    continue;
                }
                assert_eq!(
                    map.insert(k, *v),
                    model.insert(k.clone(), *v),
                    "insert {k:02x?}"
                );
            }
            Op::Remove(k) => {
                assert_eq!(map.remove(k), model.remove(k), "remove {k:02x?}");
            }
            Op::Get(k) => {
                assert_eq!(map.get(k), model.get(k).copied(), "get {k:02x?}");
            }
            Op::InsSlotWrite(k, v) => {
                if k.len() > 4096 {
                    continue;
                }
                let slot = map.ins_slot(k);
                // SAFETY: valid until the next mutation; nothing mutates
                // between here and the write.
                unsafe { slot.as_ptr().write(*v) };
                model.insert(k.clone(), *v);
            }
            Op::Audit => {
                assert_eq!(map.len(), model.len() as u64);
                let mut seen = 0u64;
                map.for_each(|k, v| {
                    assert_eq!(model.get(k).copied(), Some(v), "iterate {k:02x?}");
                    seen += 1;
                });
                assert_eq!(seen, map.len(), "iteration visits every entry once");
            }
        }
    }

    for (k, &v) in &model {
        assert_eq!(map.get(k), Some(v), "final {k:02x?}");
    }
    let keys: Vec<Vec<u8>> = model.keys().cloned().collect();
    for k in keys {
        assert!(map.remove(&k).is_some(), "drain {k:02x?}");
    }
    assert!(map.is_empty());
    assert_eq!(map.mem_used(), 0, "leak after drain");
});
