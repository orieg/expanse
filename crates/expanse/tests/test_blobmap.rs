//! Integration tests for polymorphic value slots, ExpanseBlobMap, and BlobArena.

use expanse_trie::blobmap::ExpanseBlobMap;
use expanse_trie::slot::{ValueSlot, filter_slots_predicate, filter_slots_range};
use std::collections::BTreeMap;

#[test]
fn test_inline_payloads_0_to_7_bytes() {
    let mut map = ExpanseBlobMap::new();
    assert!(map.is_empty());
    assert_eq!(map.len(), 0);

    // Insert 0..=7 byte payloads
    for len in 0..=7 {
        let key = 100 + len as u64;
        let data: Vec<u8> = (0..len).map(|i| (i + 1) as u8 * 0x22).collect();
        map.insert(key, &data, 0).expect("insert inline");
    }

    assert_eq!(map.len(), 8);
    assert!(!map.is_empty());

    for len in 0..=7 {
        let key = 100 + len as u64;
        let expected: Vec<u8> = (0..len).map(|i| (i + 1) as u8 * 0x22).collect();
        let (view, meta) = map.get(key).expect("present key");
        assert!(view.is_inline());
        assert!(!view.is_arena());
        assert_eq!(view.len(), len);
        assert_eq!(view.as_bytes(), &expected[..]);
        assert_eq!(view.is_empty(), len == 0);
        assert_eq!(meta, 0);
    }

    // Overwrite an inline payload
    map.insert(103, b"xyz", 0).expect("overwrite inline");
    let (v, _) = map.get(103).unwrap();
    assert_eq!(v.as_bytes(), b"xyz");
    assert_eq!(map.len(), 8);
}

#[test]
fn test_arena_large_blobs_1kb_to_64kb() {
    let mut map = ExpanseBlobMap::with_chunk_size(2 * 1024 * 1024);

    let sizes = [1024, 4096, 16384, 65536];
    for (idx, &size) in sizes.iter().enumerate() {
        let key = (idx as u64 + 1) * 1000;
        let data: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
        let meta = 0x1000 + idx as u32;
        map.insert(key, &data, meta).expect("insert arena blob");
    }

    assert_eq!(map.len(), 4);

    for (idx, &size) in sizes.iter().enumerate() {
        let key = (idx as u64 + 1) * 1000;
        let expected: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
        let expected_meta = 0x1000 + idx as u32;

        let (view, meta) = map.get(key).expect("present key");
        assert!(view.is_arena());
        assert!(!view.is_inline());
        assert_eq!(view.len(), size);
        assert_eq!(view.as_bytes(), &expected[..]);
        assert_eq!(meta, expected_meta);
    }

    assert!(map.mem_used() >= 2 * 1024 * 1024);
}

#[test]
fn test_hot_metadata_predicate_filtering_and_range_scans() {
    let mut map = ExpanseBlobMap::with_chunk_size(512 * 1024);
    let mut model = BTreeMap::new();

    // Ingest 500 keys
    for i in 1..=500u64 {
        let key = i * 2; // Even keys: 2, 4, ..., 1000
        let payload = format!("large-value-payload-content-record-{i}");
        let hot_meta = (i % 10) as u32; // Meta in 0..=9
        map.insert(key, payload.as_bytes(), hot_meta)
            .expect("insert");
        model.insert(key, (payload.into_bytes(), hot_meta));
    }

    assert_eq!(map.len(), 500);

    // Range scan 200..=600 with filter: hot_meta == 7
    let mut scanned_entries = Vec::new();
    map.scan_filtered(
        200..=600,
        |_k, meta| meta == 7,
        |key, view, meta| {
            scanned_entries.push((key, view.as_bytes().to_vec(), meta));
            true
        },
    );

    let mut expected_entries = Vec::new();
    for (&k, (data, meta)) in model.range(200..=600) {
        if *meta == 7 {
            expected_entries.push((k, data.clone(), *meta));
        }
    }

    assert_eq!(scanned_entries, expected_entries);
    assert!(!scanned_entries.is_empty());

    // Test early termination in callback
    let mut count = 0;
    map.scan_filtered(
        1..=1000,
        |_k, _meta| true,
        |_k, _view, _meta| {
            count += 1;
            count < 10 // Stop after 10 entries
        },
    );
    assert_eq!(count, 10);
}

#[test]
fn test_inplace_updates_and_deletions() {
    let mut map = ExpanseBlobMap::with_chunk_size(64 * 1024);

    // 1. Insert inline, then update to large arena blob
    map.insert(42, b"small", 1).unwrap();
    let (v, m) = map.get(42).unwrap();
    assert!(v.is_inline());
    assert_eq!(v.as_bytes(), b"small");
    assert_eq!(m, 0);

    let big_payload = vec![0xEE; 1024];
    map.insert(42, &big_payload, 999).unwrap();
    let (v, m) = map.get(42).unwrap();
    assert!(v.is_arena());
    assert_eq!(v.len(), 1024);
    assert_eq!(v.as_bytes(), &big_payload[..]);
    assert_eq!(m, 999);

    // 2. Overwrite arena blob with small inline payload
    map.insert(42, b"tiny", 2).unwrap();
    let (v, m) = map.get(42).unwrap();
    assert!(v.is_inline());
    assert_eq!(v.as_bytes(), b"tiny");
    assert_eq!(m, 0);

    // 3. Remove key
    assert!(map.remove(42));
    assert!(!map.contains_key(42));
    assert!(map.get(42).is_none());
    assert_eq!(map.len(), 0);
    assert!(!map.remove(42));
}

#[test]
fn test_gc_compaction_reclaims_churn_space() {
    let mut map = ExpanseBlobMap::with_chunk_size(64 * 1024);

    // Insert 600 blobs of 256 bytes each (~153 KB payloads across multiple chunks)
    for i in 0..600u64 {
        let payload = vec![(i & 0xFF) as u8; 256];
        map.insert(i, &payload, (i * 3) as u32).unwrap();
    }

    assert_eq!(map.len(), 600);
    let total_alloc_before = map.arena().mem_used();
    let chunks_before = map.arena().chunks_count();
    assert!(chunks_before >= 2);

    // Delete 500 entries (keys 0..500)
    for i in 0..500u64 {
        assert!(map.remove(i));
    }
    assert_eq!(map.len(), 100);

    // Compact the arena
    let stats = map.compact().expect("compaction must succeed");
    assert_eq!(stats.live_records_moved, 100);
    assert_eq!(stats.chunks_before, chunks_before);
    assert!(stats.chunks_after < stats.chunks_before);
    assert!(stats.total_allocated_after < total_alloc_before);

    // Verify remaining 100 blobs (keys 500..600)
    for i in 500..600u64 {
        let (view, meta) = map.get(i).expect("key 500..600 must exist");
        assert_eq!(meta, (i * 3) as u32);
        assert_eq!(view.len(), 256);
        let expected = vec![(i & 0xFF) as u8; 256];
        assert_eq!(view.as_bytes(), &expected[..]);
    }
}

#[test]
fn test_slot_vectorized_filter_kernels() {
    let mut raw_slots = Vec::new();

    // Create 32 slots with varied tags and metadata
    for i in 0..32u32 {
        let meta = i * 100;
        let slot = ValueSlot::new_arena_short(meta, i * 16).unwrap();
        raw_slots.push(slot.to_raw());
    }

    // Range filter: meta in 500..=1500 -> indices 5..=15
    let range_mask = filter_slots_range(&raw_slots, 500, 1500);
    for i in 0..32 {
        let bit = (range_mask >> i) & 1;
        if (5..=15).contains(&i) {
            assert_eq!(bit, 1, "bit {i} should be set");
        } else {
            assert_eq!(bit, 0, "bit {i} should not be set");
        }
    }

    // Predicate filter: meta % 300 == 0 -> indices 0, 3, 6, 9, 12, 15, 18, 21, 24, 27, 30
    let pred_mask = filter_slots_predicate(&raw_slots, |meta| meta % 300 == 0);
    for i in 0..32 {
        let bit = (pred_mask >> i) & 1;
        if i % 3 == 0 {
            assert_eq!(bit, 1, "bit {i} should be set");
        } else {
            assert_eq!(bit, 0, "bit {i} should not be set");
        }
    }
}

#[test]
fn test_edge_cases_and_clear() {
    let mut map = ExpanseBlobMap::new();

    // Empty map
    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
    assert!(map.get(0).is_none());
    assert!(!map.remove(0));
    let stats = map.compact().unwrap();
    assert_eq!(stats.live_records_moved, 0);

    // Insert 1 item, then clear
    map.insert(1, b"sample", 10).unwrap();
    assert_eq!(map.len(), 1);
    map.clear();
    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
    assert!(map.get(1).is_none());
}
