//! Integration test suite for lightweight value compression and inline promotion (#392).

use expanse_trie::blobmap::{EXPANSE_FORMAT_VERSION, ExpanseBlobMap};
use expanse_trie::slot::{SlotTag, ValueSlot};
use expanse_trie::sync::SyncExpanseBlobMap;
use proptest::prelude::*;
use std::collections::BTreeMap;
use std::sync::Arc;

#[test]
fn test_zero_trim_8_inlining_and_roundtrip() {
    let mut map = ExpanseBlobMap::new();

    // 1. Small integers (u64 < 2^56) with hot_meta == 0 must be inlined with tag CompressedZeroTrim8
    let val_small: u64 = 0x0012_3456_789A_BCDE;
    let data_small = val_small.to_le_bytes();
    map.insert(1, &data_small, 0).expect("insert must succeed");

    let (view, meta) = map.get(1).expect("key 1 must exist");
    assert_eq!(meta, 0);
    assert!(view.is_inline());
    assert!(!view.is_arena());
    assert_eq!(view.as_bytes(), &data_small);

    // Verify slot tag directly from index
    let raw_slot = map.index().get(1).expect("key 1 in index");
    let slot = ValueSlot::from_raw(raw_slot);
    assert_eq!(slot.tag(), SlotTag::CompressedZeroTrim8);

    // 2. Large integers (u64 >= 2^56) cannot fit into 56 bits and must spill to ArenaMeta
    let val_large: u64 = 0xFF12_3456_789A_BCDE;
    let data_large = val_large.to_le_bytes();
    map.insert(2, &data_large, 0).expect("insert must succeed");

    let (view, meta) = map.get(2).expect("key 2 must exist");
    assert_eq!(meta, 0);
    assert!(!view.is_inline());
    assert!(view.is_arena());
    assert_eq!(view.as_bytes(), &data_large);

    let raw_slot = map.index().get(2).expect("key 2 in index");
    let slot = ValueSlot::from_raw(raw_slot);
    assert_eq!(slot.tag(), SlotTag::ArenaMeta);
}

#[test]
fn test_nibble_4_inlining_and_roundtrip() {
    let mut map = ExpanseBlobMap::new();

    // 8 to 14 ASCII decimal digits must be inlined with CompressedNibble8..=CompressedNibble14
    for len in 8..=14 {
        let digit_str: String = (0..len).map(|i| ((i % 10) as u8 + b'0') as char).collect();
        let key = len as u64;
        map.insert(key, digit_str.as_bytes(), 0)
            .expect("insert must succeed");

        let (view, meta) = map.get(key).expect("key must exist");
        assert_eq!(meta, 0);
        assert!(view.is_inline());
        assert_eq!(view.as_bytes(), digit_str.as_bytes());

        let raw_slot = map.index().get(key).expect("key in index");
        let slot = ValueSlot::from_raw(raw_slot);
        let expected_tag_u8 = (SlotTag::CompressedNibble8 as u8) + (len - 8) as u8;
        assert_eq!(slot.tag() as u8, expected_tag_u8);
    }

    // 15 digits cannot fit in 56 bits (15 * 4 = 60 bits > 56) and must spill to ArenaMeta
    let str_15 = "012345678901234";
    map.insert(15, str_15.as_bytes(), 0)
        .expect("insert must succeed");
    let (view, meta) = map.get(15).expect("key 15 must exist");
    assert_eq!(meta, 0);
    assert!(view.is_arena());
    assert_eq!(view.as_bytes(), str_15.as_bytes());

    // Non-digit character inside 8..14 byte string must not use Nibble4
    let non_digit = "0123456a";
    map.insert(16, non_digit.as_bytes(), 0)
        .expect("insert must succeed");
    let (view, _) = map.get(16).expect("key 16 must exist");
    // "0123456a" compresses via Alnum6 (8 chars)
    assert!(view.is_inline());
    assert_eq!(view.as_bytes(), non_digit.as_bytes());
}

#[test]
fn test_alnum_6_inlining_and_roundtrip() {
    let mut map = ExpanseBlobMap::new();

    // 8-character alphanumeric string -> CompressedAlnum8
    let slug8 = "usr_1001";
    map.insert(8, slug8.as_bytes(), 0)
        .expect("insert must succeed");
    let (view, meta) = map.get(8).expect("key 8 must exist");
    assert_eq!(meta, 0);
    assert!(view.is_inline());
    assert_eq!(view.as_bytes(), slug8.as_bytes());

    let raw_slot = map.index().get(8).expect("key 8 in index");
    let slot = ValueSlot::from_raw(raw_slot);
    assert_eq!(slot.tag(), SlotTag::CompressedAlnum8);

    // 9-character alphanumeric string -> CompressedAlnum9
    let slug9 = "tok-a8b9z";
    map.insert(9, slug9.as_bytes(), 0)
        .expect("insert must succeed");
    let (view, meta) = map.get(9).expect("key 9 must exist");
    assert_eq!(meta, 0);
    assert!(view.is_inline());
    assert_eq!(view.as_bytes(), slug9.as_bytes());

    let raw_slot = map.index().get(9).expect("key 9 in index");
    let slot = ValueSlot::from_raw(raw_slot);
    assert_eq!(slot.tag(), SlotTag::CompressedAlnum9);

    // 10-character alphanumeric string -> 10 * 6 = 60 bits > 56 bits -> ArenaMeta
    let slug10 = "tok-a8b9z0";
    map.insert(10, slug10.as_bytes(), 0)
        .expect("insert must succeed");
    let (view, _) = map.get(10).expect("key 10 must exist");
    assert!(view.is_arena());
    assert_eq!(view.as_bytes(), slug10.as_bytes());

    // Unsupported character (e.g. space, symbol '@') -> ArenaMeta
    let slug_invalid = "usr@1001";
    map.insert(11, slug_invalid.as_bytes(), 0)
        .expect("insert must succeed");
    let (view, _) = map.get(11).expect("key 11 must exist");
    assert!(view.is_arena());
    assert_eq!(view.as_bytes(), slug_invalid.as_bytes());
}

#[test]
fn test_metadata_preservation_invariant() {
    let mut map = ExpanseBlobMap::new();

    // Invariant: If hot_meta > 0, NEVER inline a value > 7 bytes, even if it is compressible!
    // It MUST spill to ArenaMeta to store the 24-bit metadata.
    let compressible_data = b"20260829125825"; // 14 decimal digits, compressible
    let meta_val = 0x123456;

    map.insert(100, compressible_data, meta_val)
        .expect("insert must succeed");

    let (view, meta) = map.get(100).expect("key 100 must exist");
    assert_eq!(meta, meta_val);
    assert!(view.is_arena(), "Must spill to arena when hot_meta > 0");
    assert_eq!(view.as_bytes(), compressible_data);

    let raw_slot = map.index().get(100).expect("key 100 in index");
    let slot = ValueSlot::from_raw(raw_slot);
    assert_eq!(slot.tag(), SlotTag::ArenaMeta);

    // Inserting the same data with hot_meta == 0 WILL inline
    map.insert(101, compressible_data, 0)
        .expect("insert must succeed");
    let (view2, meta2) = map.get(101).expect("key 101 must exist");
    assert_eq!(meta2, 0);
    assert!(view2.is_inline(), "Must inline when hot_meta == 0");
    assert_eq!(view2.as_bytes(), compressible_data);

    // Scan filtered must see meta_val for key 100, and 0 for key 101
    let mut filtered_keys = Vec::new();
    map.scan_filtered(
        0..=200,
        |_k, meta| meta == meta_val,
        |k, view, _meta| {
            filtered_keys.push((k, view.as_bytes().to_vec()));
            true
        },
    );
    assert_eq!(filtered_keys, vec![(100, compressible_data.to_vec())]);
}

#[test]
fn test_binary_image_format_version_2_roundtrip() {
    let mut map = ExpanseBlobMap::new();
    map.insert(1, b"raw", 0).unwrap(); // Raw inline (3 bytes)
    map.insert(2, b"20260829125825", 0).unwrap(); // Compressed inline (Nibble4, 14 bytes)
    map.insert(3, b"user_slug_889", 0).unwrap(); // Arena blob (13 bytes, not all digits)
    map.insert(4, b"meta_blob", 0xABCDEF).unwrap(); // Arena blob with hot_meta

    let mut buf = Vec::new();
    map.save_to_writer(&mut buf).expect("save must succeed");

    // Verify format version in header bytes 8..12
    let version = u32::from_le_bytes(buf[8..12].try_into().unwrap());
    assert_eq!(version, EXPANSE_FORMAT_VERSION);
    assert_eq!(version, 2);

    let loaded = ExpanseBlobMap::from_bytes_slice(&buf).expect("load must succeed");
    assert_eq!(loaded.len(), 4);

    let (v1, m1) = loaded.get(1).unwrap();
    assert_eq!(v1.as_bytes(), b"raw");
    assert_eq!(m1, 0);

    let (v2, m2) = loaded.get(2).unwrap();
    assert_eq!(v2.as_bytes(), b"20260829125825");
    assert_eq!(m2, 0);
    assert!(v2.is_inline());

    let (v3, m3) = loaded.get(3).unwrap();
    assert_eq!(v3.as_bytes(), b"user_slug_889");
    assert_eq!(m3, 0);
    assert!(v3.is_arena());

    let (v4, m4) = loaded.get(4).unwrap();
    assert_eq!(v4.as_bytes(), b"meta_blob");
    assert_eq!(m4, 0xABCDEF);
    assert!(v4.is_arena());
}

#[test]
fn test_concurrent_reader_compressed_inlines() {
    let mut map = ExpanseBlobMap::new();
    for i in 0..1000u64 {
        let s = format!("slug_{i:04}");
        map.insert(i, s.as_bytes(), 0).unwrap();
    }
    let sync_map = Arc::new(SyncExpanseBlobMap::from(map));

    let mut handles = Vec::new();
    for _thread_idx in 0..4 {
        let sm = Arc::clone(&sync_map);
        handles.push(std::thread::spawn(move || {
            let mut reader = sm.reader();
            for i in 0..1000u64 {
                let expected = format!("slug_{i:04}");
                let guard = reader.pin();
                let (view, meta) = guard.get(i).expect("key must exist");
                assert_eq!(meta, 0);
                assert_eq!(view.as_bytes(), expected.as_bytes());
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

// proptest's file failure persistence calls `getcwd`, which Miri's isolation
// refuses, so this one test is native-only; the five hand-written tests above
// run under Miri and cover the same promotion, spill and image paths.
#[cfg(not(miri))]
mod native_only {
    use super::*;

    proptest! {
        #[test]
        fn proptest_value_compression_model_differential(
            entries in prop::collection::vec((any::<u64>(), prop::collection::vec(any::<u8>(), 0..64), 0..0x00FF_FFFFu32), 1..100)
        ) {
            let mut map = ExpanseBlobMap::new();
            let mut model: BTreeMap<u64, (Vec<u8>, u32)> = BTreeMap::new();

            for (k, data, meta) in &entries {
                map.insert(*k, data, *meta).unwrap();
                let expected_meta = if data.len() <= 7
                    || (*meta == 0 && expanse_trie::codec::try_compress_inline(data).is_some())
                {
                    0
                } else {
                    *meta
                };
                model.insert(*k, (data.clone(), expected_meta));
            }

            prop_assert_eq!(map.len(), model.len() as u64);

            for (k, (expected_data, expected_meta)) in &model {
                let (view, meta) = map.get(*k).unwrap();
                prop_assert_eq!(view.as_bytes(), expected_data.as_slice());
                prop_assert_eq!(meta, *expected_meta);
            }

            // Test scan_filtered
            let mut scanned = BTreeMap::new();
            map.scan_filtered(
                0..=u64::MAX,
                |_k, _meta| true,
                |k, view, meta| {
                    scanned.insert(k, (view.as_bytes().to_vec(), meta));
                    true
                },
            );
            prop_assert_eq!(scanned, model);
        }
    }
}
