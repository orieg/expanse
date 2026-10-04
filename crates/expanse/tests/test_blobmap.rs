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
        let slot = ValueSlot::new_arena_meta(meta, i).unwrap();
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

#[test]
#[cfg(not(miri))]
fn test_mmap_and_binary_serialization() {
    let mut map = ExpanseBlobMap::with_chunk_size(64 * 1024);

    for i in 0..100u64 {
        let payload = if i % 2 == 0 {
            format!("in-{i}") // <= 7 bytes, inline
        } else {
            format!("large-arena-payload-data-entry-number-{i}") // > 7 bytes, arena
        };
        map.insert(i, payload.as_bytes(), (i * 5) as u32).unwrap();
    }

    let temp_file = std::env::temp_dir().join("test_blobmap_integ.bin");
    map.save_to_file(&temp_file).unwrap();

    let loaded = ExpanseBlobMap::load_from_file(&temp_file).unwrap();
    assert_eq!(loaded.len(), 100);

    for i in 0..100u64 {
        let (view, meta) = loaded.get(i).unwrap();
        if i % 2 == 0 {
            assert!(view.is_inline());
            let expected = format!("in-{i}");
            assert_eq!(view.as_bytes(), expected.as_bytes());
            assert_eq!(meta, 0); // Inline slots do not store separate hot_meta
        } else {
            assert!(view.is_arena());
            let expected = format!("large-arena-payload-data-entry-number-{i}");
            assert_eq!(view.as_bytes(), expected.as_bytes());
            assert_eq!(meta, (i * 5) as u32);
        }
    }

    let _ = std::fs::remove_file(temp_file);
}

/// Images are not a cross-version format (docs/COMPAT.md "Binary image
/// compatibility"): an image whose header carries another
/// `EXPANSE_FORMAT_VERSION` is refused with `UnsupportedFormatVersion`, naming
/// both versions, and never mistaken for corruption; a bad magic is corruption.
///
/// Writes and reads a file, which Miri's isolation refuses (`open`), so it is
/// native-only like the round-trip test above; the header check itself is
/// pure and covered under Miri by the unit tests in `blobmap.rs`.
#[test]
#[cfg(not(miri))]
fn test_image_format_version_is_checked_before_corruption() {
    use expanse_trie::blobmap::{ArenaError, EXPANSE_FORMAT_VERSION};
    let mut map = ExpanseBlobMap::new();
    map.insert(1, b"alpha", 0).unwrap();
    map.insert(2, &[0x5Au8; 300], 0).unwrap();
    let dir = std::env::temp_dir();
    let path = dir.join(format!("expanse_fmt_{}.img", std::process::id()));
    map.save_to_file(&path).unwrap();
    let image = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert_eq!(
        &image[8..12],
        &EXPANSE_FORMAT_VERSION.to_le_bytes(),
        "header word 2 is the format version"
    );

    // the version this build no longer reads (what a v0.5.0 image carries)
    let mut old = image.clone();
    old[8..12].copy_from_slice(&(EXPANSE_FORMAT_VERSION - 1).to_le_bytes());
    match ExpanseBlobMap::from_bytes_slice(&old) {
        Err(ArenaError::UnsupportedFormatVersion { found, supported }) => {
            assert_eq!(found, EXPANSE_FORMAT_VERSION - 1);
            assert_eq!(supported, EXPANSE_FORMAT_VERSION);
        }
        Err(other) => panic!("expected UnsupportedFormatVersion, got {other:?}"),
        Ok(_) => panic!("a version-1 image must not load"),
    }
    // a version from the future is refused the same way, not as corruption
    let mut future = image.clone();
    future[8..12].copy_from_slice(&(EXPANSE_FORMAT_VERSION + 1).to_le_bytes());
    assert!(matches!(
        ExpanseBlobMap::from_bytes_slice(&future),
        Err(ArenaError::UnsupportedFormatVersion { .. })
    ));
    // a damaged magic is corruption
    let mut bad = image.clone();
    bad[0] ^= 0xFF;
    assert!(matches!(
        ExpanseBlobMap::from_bytes_slice(&bad),
        Err(ArenaError::CorruptedHeader)
    ));
    // the untouched image loads
    let loaded = ExpanseBlobMap::from_bytes_slice(&image).unwrap();
    assert_eq!(loaded.get(1).unwrap().0.as_bytes(), b"alpha");
}

#[test]
fn test_blob_record_header_layout() {
    use core::mem::{align_of, offset_of, size_of};
    use expanse_trie::blobmap::BlobRecordHeader;

    assert_eq!(
        size_of::<BlobRecordHeader>(),
        16,
        "BlobRecordHeader must be exactly 16 bytes"
    );
    assert_eq!(
        align_of::<BlobRecordHeader>(),
        8,
        "BlobRecordHeader must have 8-byte alignment"
    );
    assert_eq!(
        offset_of!(BlobRecordHeader, key),
        0,
        "key must be at offset 0"
    );
    assert_eq!(
        offset_of!(BlobRecordHeader, len),
        8,
        "len must be at offset 8"
    );
    assert_eq!(
        offset_of!(BlobRecordHeader, generation),
        12,
        "generation must be at offset 12"
    );
}

#[test]
#[cfg(not(miri))]
fn test_image_format_version_rejection_matrix() {
    use expanse_trie::blobmap::{ArenaError, EXPANSE_FORMAT_VERSION};

    let mut map = ExpanseBlobMap::new();
    map.insert(1, b"hello world payload", 0).unwrap();
    let dir = std::env::temp_dir();
    let path = dir.join(format!("expanse_fmt_matrix_{}.img", std::process::id()));
    map.save_to_file(&path).unwrap();
    let image = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).ok();

    assert_eq!(EXPANSE_FORMAT_VERSION, 3);
    assert_eq!(
        &image[8..12],
        &3u32.to_le_bytes(),
        "header word 2 must be format version 3"
    );

    // Test rejection of versions 0, 1, 2, 4
    for &unsupported_version in &[0u32, 1u32, 2u32, 4u32] {
        let mut corrupted = image.clone();
        corrupted[8..12].copy_from_slice(&unsupported_version.to_le_bytes());
        match ExpanseBlobMap::from_bytes_slice(&corrupted) {
            Err(ArenaError::UnsupportedFormatVersion { found, supported }) => {
                assert_eq!(found, unsupported_version);
                assert_eq!(supported, 3);
            }
            Err(other) => {
                panic!("expected UnsupportedFormatVersion for {unsupported_version}, got {other:?}")
            }
            Ok(_) => panic!("version {unsupported_version} must not load"),
        }
    }

    // Supported version 3 loads correctly
    let loaded = ExpanseBlobMap::from_bytes_slice(&image).unwrap();
    assert_eq!(loaded.get(1).unwrap().0.as_bytes(), b"hello world payload");
}

#[test]
fn test_blob_arena_validate_invariants_positive_and_negative_control() {
    use expanse_trie::blobmap::ArenaError;

    let mut map = ExpanseBlobMap::with_chunk_size(4096);
    assert!(map.validate_invariants().is_ok());

    // Positive control: insert varied payloads
    for i in 0..50 {
        let payload = vec![(i & 0xff) as u8; 64];
        map.insert(i, &payload, (i as u32) % 10).unwrap();
    }
    assert!(map.validate_invariants().is_ok());

    // Delete some keys (keep key 0 live as the first record in chunk 0)
    for i in (10..50).step_by(2) {
        map.remove(i);
    }
    assert!(map.validate_invariants().is_ok());

    // Serialize to buffer
    let mut bytes = Vec::new();
    map.save_to_writer(&mut bytes).unwrap();

    let loaded = ExpanseBlobMap::from_bytes_slice(&bytes).unwrap();
    loaded
        .validate_invariants()
        .expect("loaded validate_invariants");

    // Negative control 1: corrupt the chunk_size in the global header
    // In global header: bytes 48..56 is chunk_size (after magic[8], ver[4], flags[4], entry_count[8], idx_off[8], arena_off[8], total_size[8]) -> chunk_size is offset 48
    let mut corrupted_hdr = bytes.clone();
    corrupted_hdr[48..56].copy_from_slice(&99999u64.to_le_bytes());
    assert!(matches!(
        ExpanseBlobMap::from_bytes_slice(&corrupted_hdr),
        Err(ArenaError::CorruptedHeader)
    ));

    // Negative control 2: corrupt first record header's key
    // File header is 64 bytes. Index is map.len() * 16 bytes.
    // Chunk 0 header is 24 bytes. First record starts right after chunk 0 header.
    let arena_offset = 64 + (map.len() as usize) * 16;
    let first_record_key_offset = arena_offset + 24;
    let mut corrupted_rec = bytes.clone();
    corrupted_rec[first_record_key_offset] ^= 0xFF; // corrupt key of first record
    let loaded_bad = ExpanseBlobMap::from_bytes_slice(&corrupted_rec).unwrap();
    let res = loaded_bad.validate_invariants();
    assert!(
        matches!(res, Err(ArenaError::GenerationMismatch)),
        "corrupted record key mismatch must be caught by validate_invariants, got {res:?}"
    );
}

#[test]
fn test_bounded_victim_chunk_evacuation() {
    let mut map = ExpanseBlobMap::with_chunk_size(4096);
    let payload = |k: u64| -> Vec<u8> { (0..128).map(|i| (k ^ i) as u8).collect() };
    for k in 0..35 {
        map.insert(k, &payload(k), 1).unwrap();
    }
    assert!(map.validate_invariants().is_ok());

    // Initial state: chunk 0 is full (u > 0.9). No victim chunk should be selectable.
    assert!(
        !map.evacuate_victim_for_insert().unwrap(),
        "no victim when utilization is high"
    );

    // Invalidate records in chunk 0: delete 18 records (0..18) from chunk 0.
    // Utilization drops below 0.5. Chunk 0 is now a valid victim.
    for k in 0..18 {
        assert!(map.remove(k), "remove key {k}");
    }
    assert!(map.validate_invariants().is_ok());

    // Evacuate victim chunk
    let evacuated = map.evacuate_victim_for_insert().unwrap();
    assert!(evacuated, "victim chunk with u < 0.5 must be evacuated");

    // Invariants must hold after evacuation
    assert!(map.validate_invariants().is_ok());

    // Verify all remaining live keys are present with intact payloads
    for k in 18..35 {
        let (view, meta) = map.get(k).expect("live key after evacuation");
        assert_eq!(view.as_bytes(), &payload(k)[..]);
        assert_eq!(meta, 1);
    }

    // Verify deleted keys are absent
    for k in 0..18 {
        assert!(map.get(k).is_none());
    }
}

#[test]
fn test_bounded_per_insert_evacuation_under_capacity_cap() {
    let mut map = ExpanseBlobMap::with_chunk_size_and_max_capacity(4096, 3 * 4096);
    map.set_reclaim_at_cap(true);

    let payload = |k: u64| -> Vec<u8> { (0..128).map(|i| (k ^ i) as u8).collect() };

    for k in 0..50 {
        map.insert(k, &payload(k), 1).unwrap();
    }
    assert!(map.validate_invariants().is_ok());

    // Delete records from chunk 0 so that it becomes a victim (u < 0.5)
    for k in 0..18 {
        map.remove(k);
    }
    assert!(map.validate_invariants().is_ok());

    // Insert more records up to and beyond the cap:
    // With chunk 0 wasted space, cap would normally be exceeded.
    // But per-insert evacuation reclaims the victim chunk and allows inserts to proceed!
    for k in 50..80 {
        map.insert(k, &payload(k), 1)
            .expect("insert should succeed due to bounded evacuation");
    }

    assert!(map.validate_invariants().is_ok());
    for k in 18..80 {
        let (view, _) = map.get(k).expect("key present");
        assert_eq!(view.as_bytes(), &payload(k)[..]);
    }
}
