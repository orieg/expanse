//! Tests for `ExpanseOrderedBytesMap` wrapper over arbitrary byte keys (Refs #808).
//!
//! Verifies:
//! - Differential correctness against `BTreeMap<Vec<u8>, u64>`.
//! - Ordered navigation agreement (`first`, `last`, `next_at_or_after`, `next_after`,
//!   `prev_at_or_before`, `prev_before`).
//! - Iterator ordering (`iter`, `into_iter`, cursors).
//! - Clean-key fast path vs escaped slow path.
//! - Caller-buffer `*_decode_into` APIs and `BufferTooSmall` error checking.
//! - Empty key `b""` handling.
//! - Proptest property verification over arbitrary and 0x00/0x01-heavy byte keys.

use core::ptr::NonNull;
use expanse_trie::domain::EscapeDecodeError;
use expanse_trie::ordered_bytesmap::ExpanseOrderedBytesMap;
#[cfg(not(miri))]
use proptest::prelude::*;
use std::collections::BTreeMap;
use std::ops::Bound;

fn read_slot(slot: NonNull<u64>) -> u64 {
    // SAFETY: slot is non-null and points to an initialized u64 value in the map.
    unsafe { slot.as_ptr().read() }
}

fn write_slot(mut slot: NonNull<u64>, val: u64) {
    // SAFETY: slot is non-null and points to a writable u64 value slot in the map.
    unsafe {
        *slot.as_mut() = val;
    }
}

#[test]
fn test_send_auto_trait() {
    fn _assert_send<T: Send>() {}
    _assert_send::<ExpanseOrderedBytesMap>();
    let map = ExpanseOrderedBytesMap::new();
    let handle = std::thread::spawn(move || {
        assert!(map.is_empty());
    });
    assert!(handle.join().is_ok());
}

#[test]
fn test_empty_key_lifecycle() {
    let mut map = ExpanseOrderedBytesMap::new();
    assert!(map.is_empty());
    assert_eq!(map.len(), 0);
    assert_eq!(map.get(b""), None);
    assert!(!map.contains_key(b""));

    // Insert empty key
    assert_eq!(map.insert(b"", 100), None);
    assert!(!map.is_empty());
    assert_eq!(map.len(), 1);
    assert_eq!(map.get(b""), Some(100));
    assert!(map.contains_key(b""));

    // Replace empty key
    assert_eq!(map.insert(b"", 200), Some(100));
    assert_eq!(map.len(), 1);
    assert_eq!(map.get(b""), Some(200));

    // Ordered navigation with only empty key
    let (first_k, first_slot) = map.first().expect("first exists");
    assert_eq!(first_k, b"");
    assert_eq!(read_slot(first_slot), 200);

    let (last_k, last_slot) = map.last().expect("last exists");
    assert_eq!(last_k, b"");
    assert_eq!(read_slot(last_slot), 200);

    assert_eq!(map.next_at_or_after(b"").unwrap().0, b"");
    assert_eq!(map.next_after(b""), None);
    assert_eq!(map.prev_at_or_before(b"").unwrap().0, b"");
    assert_eq!(map.prev_before(b""), None);

    // Insert non-empty keys alongside empty key
    map.insert(b"\x00", 300);
    map.insert(b"a", 400);
    assert_eq!(map.len(), 3);

    // Empty key sorts strictly before all non-empty keys
    assert_eq!(map.first().unwrap().0, b"");
    assert_eq!(map.next_after(b"").unwrap().0, b"\x00");
    assert_eq!(map.prev_before(b"\x00").unwrap().0, b"");

    // Remove empty key
    assert_eq!(map.remove(b""), Some(200));
    assert_eq!(map.len(), 2);
    assert_eq!(map.get(b""), None);
    assert_eq!(map.first().unwrap().0, b"\x00");
}

#[test]
fn test_clean_and_escaped_point_ops() {
    let mut map = ExpanseOrderedBytesMap::new();

    // Clean keys (fast path: no bytes <= 1)
    let clean_keys = [
        b"apple".to_vec(),
        b"banana".to_vec(),
        b"cherry".to_vec(),
        vec![2, 3, 4, 5, 6, 7, 8, 9],
    ];

    // Escaped keys (slow path: containing 0x00 and 0x01)
    let escaped_keys = [
        b"uuid\x00alpha".to_vec(),
        b"uuid\x00beta".to_vec(),
        b"uuid\x01gamma".to_vec(),
        vec![0],
        vec![1],
        vec![0, 0, 0, 0],
        vec![1, 1, 1, 1],
        vec![0, 1, 0, 1],
        vec![1, 2, 1, 2],
        vec![0; 32],
        vec![1; 32],
        vec![0; 64],
        vec![1; 64],
    ];

    let mut all_keys = Vec::new();
    all_keys.extend_from_slice(&clean_keys);
    all_keys.extend_from_slice(&escaped_keys);

    for (i, k) in all_keys.iter().enumerate() {
        assert_eq!(map.insert(k, i as u64), None);
    }
    assert_eq!(map.len(), all_keys.len() as u64);

    for (i, k) in all_keys.iter().enumerate() {
        assert_eq!(map.get(k), Some(i as u64), "key: {:?}", k);
        assert!(map.contains_key(k));
        assert!(map.contains(k));
    }

    // In-place mutation via value slot pointer
    for (i, k) in all_keys.iter().enumerate() {
        let slot = map.get_value_slot(k).expect("slot exists");
        write_slot(slot, (i as u64) + 10_000);
    }

    for (i, k) in all_keys.iter().enumerate() {
        assert_eq!(map.get(k), Some((i as u64) + 10_000));
    }

    // ins_slot on absent key
    let new_key = b"new_key\x00with_nul";
    let slot = map.ins_slot(new_key);
    assert_eq!(read_slot(slot), 0);
    write_slot(slot, 42);
    assert_eq!(map.get(new_key), Some(42));

    // ins_slot on existing key preserves value
    let slot = map.ins_slot(new_key);
    assert_eq!(read_slot(slot), 42);

    // Removal
    assert_eq!(map.remove(new_key), Some(42));
    assert_eq!(map.get(new_key), None);
}

#[test]
fn test_decode_into_buffer_too_small() {
    let mut map = ExpanseOrderedBytesMap::new();
    let key = b"binary\x00key\x01test";
    map.insert(key, 999);

    // 1. Buffer too small for first_decode_into
    let mut small_buf = [0u8; 5]; // required is 15
    match map.first_decode_into(&mut small_buf) {
        Err(EscapeDecodeError::BufferTooSmall { required, provided }) => {
            assert_eq!(provided, 5);
            assert!(required >= 15);
        }
        other => panic!("expected BufferTooSmall, got {:?}", other),
    }

    // 2. Exact size buffer succeeds
    let mut exact_buf = [0u8; 15];
    let (written, slot) = map
        .first_decode_into(&mut exact_buf)
        .expect("succeeds")
        .expect("present");
    assert_eq!(written, 15);
    assert_eq!(&exact_buf[..written], key);
    assert_eq!(read_slot(slot), 999);

    // 3. Navigation decode_into buffer-too-small checks
    assert!(matches!(
        map.last_decode_into(&mut small_buf),
        Err(EscapeDecodeError::BufferTooSmall { .. })
    ));
    assert!(matches!(
        map.next_at_or_after_decode_into(b"binary", &mut small_buf),
        Err(EscapeDecodeError::BufferTooSmall { .. })
    ));
    assert!(matches!(
        map.next_after_decode_into(b"bina", &mut small_buf),
        Err(EscapeDecodeError::BufferTooSmall { .. })
    ));
    assert!(matches!(
        map.prev_at_or_before_decode_into(b"binary\xff", &mut small_buf),
        Err(EscapeDecodeError::BufferTooSmall { .. })
    ));
    assert!(matches!(
        map.prev_before_decode_into(b"z", &mut small_buf),
        Err(EscapeDecodeError::BufferTooSmall { .. })
    ));

    // 4. Cursor next_decode_into buffer-too-small
    let mut cur = map.cursor();
    assert!(matches!(
        cur.next_decode_into(&mut small_buf),
        Err(EscapeDecodeError::BufferTooSmall { .. })
    ));
}

#[test]
fn test_ordered_navigation_agreement_exhaustive() {
    let test_keys: Vec<Vec<u8>> = vec![
        b"".to_vec(),
        b"\x00".to_vec(),
        b"\x00\x00".to_vec(),
        b"\x00\x01".to_vec(),
        b"\x00\x02".to_vec(),
        b"\x01".to_vec(),
        b"\x01\x00".to_vec(),
        b"\x01\x01".to_vec(),
        b"\x01\x02".to_vec(),
        b"\x02".to_vec(),
        b"\x02\x00".to_vec(),
        b"a".to_vec(),
        b"a\x00".to_vec(),
        b"a\x00\x00".to_vec(),
        b"a\x00b".to_vec(),
        b"a\x01".to_vec(),
        b"ab".to_vec(),
        b"uuid\x00alpha".to_vec(),
        b"uuid\x00alpha\x001".to_vec(),
        b"uuid\x00beta".to_vec(),
        b"uuid\x01gamma".to_vec(),
        b"uuid_plain".to_vec(),
        vec![0xFF, 0xFE],
        vec![0xFF, 0xFF],
    ];

    let mut map = ExpanseOrderedBytesMap::new();
    let mut model = BTreeMap::new();

    for (i, k) in test_keys.iter().enumerate() {
        let val = (i as u64) * 7 + 13;
        map.insert(k, val);
        model.insert(k.clone(), val);
    }

    assert_eq!(map.len(), model.len() as u64);

    // Test first and last
    let (model_first_k, model_first_v) = model.first_key_value().unwrap();
    let (map_first_k, map_first_slot) = map.first().unwrap();
    assert_eq!(&map_first_k, model_first_k);
    assert_eq!(read_slot(map_first_slot), *model_first_v);

    let (model_last_k, model_last_v) = model.last_key_value().unwrap();
    let (map_last_k, map_last_slot) = map.last().unwrap();
    assert_eq!(&map_last_k, model_last_k);
    assert_eq!(read_slot(map_last_slot), *model_last_v);

    // Probes for range searches: present keys, boundary keys, and intermediate probes
    let mut probe_keys = test_keys.clone();
    probe_keys.extend(vec![
        b"\x00\x00\x00".to_vec(),
        b"\x00\x00\x01".to_vec(),
        b"\x01\x00\x00".to_vec(),
        b"uuid\x00".to_vec(),
        b"uuid\x00a".to_vec(),
        b"uuid\x00alpha\x00".to_vec(),
        b"uuid\x00alpha\x000".to_vec(),
        b"uuid\x00alpha\x002".to_vec(),
        b"uuid\x00az".to_vec(),
        b"uuid\x00b".to_vec(),
        b"uuid\x00beta\x00".to_vec(),
        b"uuid\x01".to_vec(),
        b"uuid\x02".to_vec(),
        b"z".to_vec(),
        vec![0xFF, 0xFF, 0xFF],
    ]);

    for probe in &probe_keys {
        // next_at_or_after vs btree.range(probe..)
        let model_next_at = model.range(probe.clone()..).next();
        let map_next_at = map.next_at_or_after(probe);
        match (model_next_at, map_next_at) {
            (Some((mk, mv)), Some((k, slot))) => {
                assert_eq!(&k, mk, "next_at_or_after mismatch for probe {:?}", probe);
                assert_eq!(read_slot(slot), *mv);
            }
            (None, None) => {}
            (m, actual) => panic!(
                "next_at_or_after existence mismatch for probe {:?}: model={:?}, actual={:?}",
                probe,
                m.map(|(k, v)| (k, *v)),
                actual.map(|(k, s)| (k, read_slot(s)))
            ),
        }

        // next_after vs btree.range((Excluded(probe), Unbounded))
        let model_next = model
            .range((Bound::Excluded(probe.clone()), Bound::Unbounded))
            .next();
        let map_next = map.next_after(probe);
        match (model_next, map_next) {
            (Some((mk, mv)), Some((k, slot))) => {
                assert_eq!(&k, mk, "next_after mismatch for probe {:?}", probe);
                assert_eq!(read_slot(slot), *mv);
            }
            (None, None) => {}
            (m, actual) => panic!(
                "next_after existence mismatch for probe {:?}: model={:?}, actual={:?}",
                probe,
                m.map(|(k, v)| (k, *v)),
                actual.map(|(k, s)| (k, read_slot(s)))
            ),
        }

        // prev_at_or_before vs btree.range(..=probe).next_back()
        let model_prev_at = model.range(..=probe.clone()).next_back();
        let map_prev_at = map.prev_at_or_before(probe);
        match (model_prev_at, map_prev_at) {
            (Some((mk, mv)), Some((k, slot))) => {
                assert_eq!(&k, mk, "prev_at_or_before mismatch for probe {:?}", probe);
                assert_eq!(read_slot(slot), *mv);
            }
            (None, None) => {}
            (m, actual) => panic!(
                "prev_at_or_before existence mismatch for probe {:?}: model={:?}, actual={:?}",
                probe,
                m.map(|(k, v)| (k, *v)),
                actual.map(|(k, s)| (k, read_slot(s)))
            ),
        }

        // prev_before vs btree.range(..probe).next_back()
        let model_prev = model.range(..probe.clone()).next_back();
        let map_prev = map.prev_before(probe);
        match (model_prev, map_prev) {
            (Some((mk, mv)), Some((k, slot))) => {
                assert_eq!(&k, mk, "prev_before mismatch for probe {:?}", probe);
                assert_eq!(read_slot(slot), *mv);
            }
            (None, None) => {}
            (m, actual) => panic!(
                "prev_before existence mismatch for probe {:?}: model={:?}, actual={:?}",
                probe,
                m.map(|(k, v)| (k, *v)),
                actual.map(|(k, s)| (k, read_slot(s)))
            ),
        }
    }
}

#[test]
fn test_iterators_and_collection() {
    let mut map = ExpanseOrderedBytesMap::new();
    let mut model = BTreeMap::new();

    let keys = [
        b"dog".to_vec(),
        b"cat".to_vec(),
        b"fish\x00tail".to_vec(),
        b"bird\x01wing".to_vec(),
        b"ape".to_vec(),
        b"".to_vec(),
    ];

    for (i, k) in keys.iter().enumerate() {
        map.insert(k, i as u64);
        model.insert(k.clone(), i as u64);
    }

    // Iter
    let map_entries: Vec<(Vec<u8>, u64)> = map.iter().collect();
    let model_entries: Vec<(Vec<u8>, u64)> = model.iter().map(|(k, v)| (k.clone(), *v)).collect();
    assert_eq!(map_entries, model_entries);

    // Cursor
    let mut cur = map.cursor();
    let mut cursor_entries = Vec::new();
    while let Some((k, v)) = cur.next_owned() {
        cursor_entries.push((k, v));
    }
    assert_eq!(cursor_entries, model_entries);

    // Cursor streaming decode_into
    let mut cur = map.cursor();
    let mut stream_entries = Vec::new();
    let mut buf = [0u8; 64];
    while let Some((len, v)) = cur.next_entry_decode_into(&mut buf).unwrap() {
        stream_entries.push((buf[..len].to_vec(), v));
    }
    assert_eq!(stream_entries, model_entries);

    // IntoIter
    let map_into_entries: Vec<(Vec<u8>, u64)> = map.clone().into_iter().collect();
    assert_eq!(map_into_entries, model_entries);

    // FromIterator and Extend
    let map_from_iter: ExpanseOrderedBytesMap = model_entries.iter().cloned().collect();
    assert_eq!(map_from_iter.len(), model.len() as u64);
    assert_eq!(map_from_iter.iter().collect::<Vec<_>>(), model_entries);
}

#[test]
fn test_stack_buffer_boundary_32_33_bytes() {
    let mut map = ExpanseOrderedBytesMap::new();

    // 32 bytes of 0x00 -> encoded is exactly 64 bytes (fits within [u8; 64] stack buffer)
    let key32 = vec![0u8; 32];
    // 33 bytes of 0x00 -> encoded is 66 bytes (spills past [u8; 64] to escape_encode heap fallback)
    let key33 = vec![0u8; 33];

    assert_eq!(map.insert(&key32, 3200), None);
    assert_eq!(map.insert(&key33, 3300), None);
    assert_eq!(map.len(), 2);

    assert_eq!(map.get(&key32), Some(3200));
    assert_eq!(map.get(&key33), Some(3300));
    assert!(map.contains_key(&key32));
    assert!(map.contains_key(&key33));

    // Lexicographical ordering: key32 is prefix of key33, so key32 < key33
    let (first_k, first_v) = map.first_entry().unwrap();
    assert_eq!(first_k, key32);
    assert_eq!(first_v, 3200);

    let (last_k, last_v) = map.last_entry().unwrap();
    assert_eq!(last_k, key33);
    assert_eq!(last_v, 3300);

    let next = map.next_after_entry(&key32).unwrap();
    assert_eq!(next.0, key33);
    assert_eq!(next.1, 3300);

    let prev = map.prev_before_entry(&key33).unwrap();
    assert_eq!(prev.0, key32);
    assert_eq!(prev.1, 3200);

    // Buffer decode_into with exact buffer sizes
    let mut buf32 = [0u8; 32];
    let (len32, v32) = map.first_entry_decode_into(&mut buf32).unwrap().unwrap();
    assert_eq!(len32, 32);
    assert_eq!(&buf32, key32.as_slice());
    assert_eq!(v32, 3200);

    let mut buf33 = [0u8; 33];
    let (len33, v33) = map.last_entry_decode_into(&mut buf33).unwrap().unwrap();
    assert_eq!(len33, 33);
    assert_eq!(&buf33, key33.as_slice());
    assert_eq!(v33, 3300);

    // Buffer too small checks for both boundary keys
    let mut small_buf = [0u8; 10];
    assert!(matches!(
        map.first_entry_decode_into(&mut small_buf),
        Err(EscapeDecodeError::BufferTooSmall {
            required: 32,
            provided: 10
        })
    ));
    assert!(matches!(
        map.last_entry_decode_into(&mut small_buf),
        Err(EscapeDecodeError::BufferTooSmall {
            required: 33,
            provided: 10
        })
    ));
}

// ---------------------------------------------------------------------------
// Proptest Model Tests
// ---------------------------------------------------------------------------

#[cfg(not(miri))]
mod proptest_model {
    use super::*;

    #[derive(Clone, Debug)]
    enum Op {
        Insert(Vec<u8>, u64),
        Remove(Vec<u8>),
        Get(Vec<u8>),
        Contains(Vec<u8>),
        NextAtOrAfter(Vec<u8>),
        PrevAtOrBefore(Vec<u8>),
    }

    fn op_strategy() -> impl Strategy<Value = Op> {
        prop_oneof![
            3 => (
                prop_oneof![
                    Just(vec![]),
                    prop::collection::vec(prop_oneof![Just(0u8), Just(1u8), Just(2u8)], 1..32),
                    prop::collection::vec(any::<u8>(), 16..=16),
                    prop::collection::vec(any::<u8>(), 0..48),
                ],
                any::<u64>()
            )
                .prop_map(|(k, v)| Op::Insert(k, v)),
            2 => prop_oneof![
                Just(vec![]),
                prop::collection::vec(prop_oneof![Just(0u8), Just(1u8), Just(2u8)], 1..32),
                prop::collection::vec(any::<u8>(), 16..=16),
                prop::collection::vec(any::<u8>(), 0..48),
            ]
            .prop_map(Op::Remove),
            2 => prop_oneof![
                Just(vec![]),
                prop::collection::vec(prop_oneof![Just(0u8), Just(1u8), Just(2u8)], 1..32),
                prop::collection::vec(any::<u8>(), 16..=16),
                prop::collection::vec(any::<u8>(), 0..48),
            ]
            .prop_map(Op::Get),
            1 => prop_oneof![
                Just(vec![]),
                prop::collection::vec(prop_oneof![Just(0u8), Just(1u8), Just(2u8)], 1..32),
                prop::collection::vec(any::<u8>(), 16..=16),
                prop::collection::vec(any::<u8>(), 0..48),
            ]
            .prop_map(Op::Contains),
            1 => prop_oneof![
                Just(vec![]),
                prop::collection::vec(prop_oneof![Just(0u8), Just(1u8), Just(2u8)], 1..32),
                prop::collection::vec(any::<u8>(), 16..=16),
                prop::collection::vec(any::<u8>(), 0..48),
            ]
            .prop_map(Op::NextAtOrAfter),
            1 => prop_oneof![
                Just(vec![]),
                prop::collection::vec(prop_oneof![Just(0u8), Just(1u8), Just(2u8)], 1..32),
                prop::collection::vec(any::<u8>(), 16..=16),
                prop::collection::vec(any::<u8>(), 0..48),
            ]
            .prop_map(Op::PrevAtOrBefore),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 64,
            max_shrink_iters: 2048,
            ..ProptestConfig::default()
        })]

        #[test]
        fn differential_model_matches_btreemap(ops in prop::collection::vec(op_strategy(), 1..200)) {
            let mut map = ExpanseOrderedBytesMap::new();
            let mut model = BTreeMap::new();

            for op in ops {
                match op {
                    Op::Insert(k, v) => {
                        let prev_map = map.insert(&k, v);
                        let prev_model = model.insert(k.clone(), v);
                        prop_assert_eq!(prev_map, prev_model);
                    }
                    Op::Remove(k) => {
                        let rem_map = map.remove(&k);
                        let rem_model = model.remove(&k);
                        prop_assert_eq!(rem_map, rem_model);
                    }
                    Op::Get(k) => {
                        let get_map = map.get(&k);
                        let get_model = model.get(&k).copied();
                        prop_assert_eq!(get_map, get_model);
                    }
                    Op::Contains(k) => {
                        let cont_map = map.contains_key(&k);
                        let cont_model = model.contains_key(&k);
                        prop_assert_eq!(cont_map, cont_model);
                    }
                    Op::NextAtOrAfter(probe) => {
                        let model_res = model.range(probe.clone()..).next();
                        let map_res = map.next_at_or_after(&probe);
                        match (model_res, map_res) {
                            (Some((mk, mv)), Some((k, slot))) => {
                                prop_assert_eq!(&k, mk);
                                prop_assert_eq!(read_slot(slot), *mv);
                            }
                            (None, None) => {}
                            (m, actual) => {
                                panic!(
                                    "next_at_or_after mismatch for probe {:?}: model={:?}, actual={:?}",
                                    probe,
                                    m.map(|(k, v)| (k, *v)),
                                    actual.map(|(k, s)| (k, read_slot(s)))
                                );
                            }
                        }
                    }
                    Op::PrevAtOrBefore(probe) => {
                        let model_res = model.range(..=probe.clone()).next_back();
                        let map_res = map.prev_at_or_before(&probe);
                        match (model_res, map_res) {
                            (Some((mk, mv)), Some((k, slot))) => {
                                prop_assert_eq!(&k, mk);
                                prop_assert_eq!(read_slot(slot), *mv);
                            }
                            (None, None) => {}
                            (m, actual) => {
                                panic!(
                                    "prev_at_or_before mismatch for probe {:?}: model={:?}, actual={:?}",
                                    probe,
                                    m.map(|(k, v)| (k, *v)),
                                    actual.map(|(k, s)| (k, read_slot(s)))
                                );
                            }
                        }
                    }
                }
                prop_assert_eq!(map.len(), model.len() as u64);
                prop_assert_eq!(map.is_empty(), model.is_empty());
            }

            // Final iterator order check
            let map_items: Vec<(Vec<u8>, u64)> = map.iter().collect();
            let model_items: Vec<(Vec<u8>, u64)> = model.into_iter().collect();
            prop_assert_eq!(map_items, model_items);
        }
    }
}
