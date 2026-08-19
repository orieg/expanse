//! Fuzz target: `ExpanseStrMap` against a `BTreeMap` model
//! (`docs/TESTING.md` layer 4).
//!
//! The gap this fills: `ExpanseStrMap` is a meta-trie over 8-byte chunks,
//! so **key length is tree depth**. Every other layer of testing reached
//! only shallow chains — the differential oracle's generator topped out
//! around 35 bytes (≈5 levels) — which meant the deep-chain paths
//! (chunk-chain descent, emptied-node pruning, teardown) had effectively
//! no coverage in the regime where they actually differ from the shallow
//! case. That is exactly where a recursive teardown overflows the stack.
//!
//! `BTreeMap<Vec<u8>, u64>` is the model: byte-lexicographic ordering
//! over the same keys, which is the ordering `ExpanseStrMap` claims to
//! provide (numeric order over big-endian chunks == byte order).
//!
//! Run: `cargo +nightly fuzz run strmap_ops`

#![no_main]

use arbitrary::Arbitrary;
use expanse_trie::strmap::ExpanseStrMap;
use libfuzzer_sys::fuzz_target;
use std::collections::BTreeMap;

/// Keys are built from a template so the fuzzer spends its budget on op
/// sequences and *depths* rather than rediscovering that shared prefixes
/// matter. `Deep` is the point of this target.
#[derive(Arbitrary, Debug)]
enum Key {
    /// Short, arbitrary — the common shape.
    Short(Vec<u8>),
    /// A shared prefix plus a tail: forces chunk-chain sharing.
    Prefixed(u8, Vec<u8>),
    /// A long run of one byte: `len` chunks of depth, capped so the
    /// corpus stays manageable.
    Deep(u8, u16),
}

impl Key {
    fn bytes(&self) -> Vec<u8> {
        // NUL terminates a key in this map (the JudySL contract), so it
        // can never appear inside one.
        let clean = |v: &Vec<u8>| -> Vec<u8> {
            v.iter().map(|&b| if b == 0 { 1 } else { b }).collect()
        };
        match self {
            Key::Short(v) => clean(v),
            Key::Prefixed(p, v) => {
                let mut k = vec![if *p == 0 { 1 } else { *p }; 24];
                k.extend(clean(v));
                k
            }
            Key::Deep(b, len) => {
                let b = if *b == 0 { 1 } else { *b };
                vec![b; (*len as usize % 8192) + 1]
            }
        }
    }
}

#[derive(Arbitrary, Debug)]
enum Op {
    Insert(Key, u64),
    Remove(Key),
    Get(Key),
    /// Ordered navigation from a probe key — the paths that had to be
    /// made iterative alongside removal and teardown.
    NextAtOrAfter(Key),
    First,
    Last,
    Audit,
}

fuzz_target!(|ops: Vec<Op>| {
    if ops.len() > 512 {
        return;
    }
    let mut map = ExpanseStrMap::new();
    let mut model: BTreeMap<Vec<u8>, u64> = BTreeMap::new();

    for op in &ops {
        match op {
            Op::Insert(k, v) => {
                let k = k.bytes();
                assert_eq!(map.insert(&k, *v), model.insert(k.clone(), *v));
            }
            Op::Remove(k) => {
                let k = k.bytes();
                assert_eq!(map.remove(&k), model.remove(&k));
            }
            Op::Get(k) => {
                let k = k.bytes();
                assert_eq!(map.get(&k), model.get(&k).copied());
            }
            Op::NextAtOrAfter(k) => {
                let k = k.bytes();
                let got = map.next_at_or_after(&k).map(|(key, slot)| {
                    // SAFETY: the slot is valid until the next mutation,
                    // and nothing mutates between here and the read.
                    (key, unsafe { *slot.as_ptr() })
                });
                let want = model
                    .range(k.clone()..)
                    .next()
                    .map(|(key, v)| (key.clone(), *v));
                assert_eq!(got, want, "next_at_or_after {:02x?}", &k[..k.len().min(16)]);
            }
            Op::First => {
                let got = map
                    .first()
                    // SAFETY: as above.
                    .map(|(key, slot)| (key, unsafe { *slot.as_ptr() }));
                let want = model.iter().next().map(|(k, v)| (k.clone(), *v));
                assert_eq!(got, want, "first");
            }
            Op::Last => {
                let got = map
                    .last()
                    // SAFETY: as above.
                    .map(|(key, slot)| (key, unsafe { *slot.as_ptr() }));
                let want = model.iter().next_back().map(|(k, v)| (k.clone(), *v));
                assert_eq!(got, want, "last");
            }
            Op::Audit => {
                assert_eq!(map.len(), model.len() as u64);
            }
        }
    }

    // Every model entry is retrievable, then drain — the removal path
    // and its emptied-node pruning, at whatever depths the fuzzer built.
    for (k, &v) in &model {
        assert_eq!(map.get(k), Some(v), "final {:02x?}", &k[..k.len().min(16)]);
    }
    let keys: Vec<Vec<u8>> = model.keys().cloned().collect();
    for k in keys {
        assert!(map.remove(&k).is_some(), "drain");
    }
    assert!(map.is_empty());
    // `clear` on an emptied map must report nothing left to free, which
    // is also the accounting walk that had to stop recursing.
    assert_eq!(map.clear(), 0, "leak after drain");
});
