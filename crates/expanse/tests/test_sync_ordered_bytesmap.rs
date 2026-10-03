//! Integration tests for `SyncExpanseOrderedBytesMap` (issue #808).
//!
//! Verifies:
//! - Thread-safety and `Send` / `Sync` traits
//! - Concurrent multi-threaded CRUD operations
//! - Reader handles with optimistic lookups
//! - Caller-buffer zero-allocation `*_decode_into` APIs and `BufferTooSmall` error
//! - Bijective escape transcoding on clean, escaped, and edge-case binary keys
//! - Lexicographical order agreement with `BTreeMap` under concurrency
//! - Atomic sections via `with_exclusive` and consistent reads via `with_locked`
//! - Construction from single-threaded `ExpanseOrderedBytesMap` and `into_inner`

#![cfg(not(miri))]
#![cfg(all(target_pointer_width = "64", feature = "std"))]

use expanse_trie::domain::EscapeDecodeError;
use expanse_trie::ordered_bytesmap::ExpanseOrderedBytesMap;
use expanse_trie::sync::{OrderedBytesReader, SyncExpanseOrderedBytesMap};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

#[test]
fn test_send_sync_traits() {
    assert_send::<SyncExpanseOrderedBytesMap>();
    assert_sync::<SyncExpanseOrderedBytesMap>();
    assert_send::<OrderedBytesReader<'static>>();

    let map = Arc::new(SyncExpanseOrderedBytesMap::new());
    map.insert(b"init", 42);
    let map_clone = Arc::clone(&map);
    let handle = thread::spawn(move || {
        let reader = map_clone.reader();
        assert_eq!(reader.get(b"init"), Some(42));
        assert!(reader.contains(b"init"));
    });
    assert!(handle.join().is_ok());
    assert_eq!(map.len(), 1);
}

#[test]
fn test_empty_map_lifecycle() {
    let map = SyncExpanseOrderedBytesMap::new();
    assert!(map.is_empty());
    assert_eq!(map.len(), 0);
    assert_eq!(map.get(b""), None);
    assert!(!map.contains_key(b"key"));
    assert_eq!(map.first(), None);
    assert_eq!(map.last(), None);
    assert_eq!(map.clear(), 0);
}

#[test]
fn test_basic_crud_and_fast_paths() {
    let map = SyncExpanseOrderedBytesMap::new();

    // 1. Clean key (no bytes <= 1): exercises zero-allocation fast path
    let clean_key = b"alpha_beta_gamma";
    assert_eq!(map.insert(clean_key, 100), None);
    assert_eq!(map.len(), 1);
    assert!(map.contains_key(clean_key));
    assert_eq!(map.get(clean_key), Some(100));

    // Overwrite
    assert_eq!(map.insert(clean_key, 101), Some(100));
    assert_eq!(map.get(clean_key), Some(101));
    assert_eq!(map.len(), 1);

    // 2. Escaped short key (<= 32 bytes with 0x00 and 0x01): exercises stack buffer
    let escaped_short = [0x00, 0x01, 0xAA, 0xBB, 0x00];
    assert_eq!(map.insert(&escaped_short, 200), None);
    assert_eq!(map.len(), 2);
    assert!(map.contains_key(&escaped_short));
    assert_eq!(map.get(&escaped_short), Some(200));

    // 3. Escaped long key (> 32 bytes): exercises heap fallback
    let mut escaped_long = vec![0x42; 40];
    escaped_long[5] = 0x00;
    escaped_long[15] = 0x01;
    assert_eq!(map.insert(&escaped_long, 300), None);
    assert_eq!(map.len(), 3);
    assert_eq!(map.get(&escaped_long), Some(300));

    // 4. Empty key
    assert_eq!(map.insert(b"", 400), None);
    assert_eq!(map.len(), 4);
    assert_eq!(map.get(b""), Some(400));

    // 5. Remove
    assert_eq!(map.remove(clean_key), Some(101));
    assert_eq!(map.len(), 3);
    assert!(!map.contains_key(clean_key));
    assert_eq!(map.remove(clean_key), None);

    assert_eq!(map.remove(&escaped_short), Some(200));
    assert_eq!(map.remove(&escaped_long), Some(300));
    assert_eq!(map.remove(b""), Some(400));
    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
}

#[test]
fn test_reader_handle_concurrency() {
    let map = Arc::new(SyncExpanseOrderedBytesMap::new());
    for i in 0..100u64 {
        let key = format!("item_{i:04}").into_bytes();
        map.insert(&key, i * 10);
    }

    let mut handles = Vec::new();
    for thread_idx in 0..4 {
        let map = Arc::clone(&map);
        handles.push(thread::spawn(move || {
            let reader = map.reader();
            for i in 0..100u64 {
                let key = format!("item_{i:04}").into_bytes();
                assert_eq!(reader.get(&key), Some(i * 10));
                assert!(reader.contains(&key));
            }
            let absent = format!("absent_{thread_idx}").into_bytes();
            assert_eq!(reader.get(&absent), None);
            assert!(!reader.contains(&absent));
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn test_multi_threaded_concurrent_writers_and_readers() {
    let map = Arc::new(SyncExpanseOrderedBytesMap::new());
    let n_writers = 4;
    let n_items_per_writer = 250;

    // Concurrent writers
    let mut writer_handles = Vec::new();
    for w_idx in 0..n_writers {
        let map = Arc::clone(&map);
        writer_handles.push(thread::spawn(move || {
            for i in 0..n_items_per_writer {
                let val = (w_idx * 1000 + i) as u64;
                // Mix clean keys and escaped keys with NUL bytes
                let mut key = Vec::with_capacity(16);
                key.push(w_idx as u8);
                key.push(0x00); // embedded NUL
                key.extend_from_slice(&(i as u32).to_be_bytes());
                map.insert(&key, val);
            }
        }));
    }

    // Concurrent readers running concurrently with writers
    let mut reader_handles = Vec::new();
    for _ in 0..2 {
        let map = Arc::clone(&map);
        reader_handles.push(thread::spawn(move || {
            let reader = map.reader();
            for _ in 0..500 {
                let _ = map.len();
                let _ = reader.get(&[0, 0, 0, 0, 0, 1]);
            }
        }));
    }

    for h in writer_handles {
        h.join().unwrap();
    }
    for h in reader_handles {
        h.join().unwrap();
    }

    assert_eq!(map.len(), (n_writers * n_items_per_writer) as u64);

    // Verify all keys present
    for w_idx in 0..n_writers {
        for i in 0..n_items_per_writer {
            let val = (w_idx * 1000 + i) as u64;
            let mut key = Vec::with_capacity(16);
            key.push(w_idx as u8);
            key.push(0x00);
            key.extend_from_slice(&(i as u32).to_be_bytes());
            assert_eq!(map.get(&key), Some(val));
        }
    }
}

#[test]
fn test_concurrent_readers_and_writers_with_escape_integrity() {
    let map = Arc::new(SyncExpanseOrderedBytesMap::new());

    // 1. Pre-populate permanent model keys across all representations (clean, escaped, edge cases).
    // These keys remain present throughout the entire concurrent execution.
    let permanent_model: Vec<(Vec<u8>, u64)> = vec![
        (vec![], 1_000_001),
        (vec![0x00], 1_000_002),
        (vec![0x00, 0x00], 1_000_003),
        (vec![0x00, 0x01], 1_000_004),
        (vec![0x01], 1_000_005),
        (vec![0x01, 0x01], 1_000_006), // Disambiguation: distinct from unescaped 0x00
        (vec![0x01, 0x02], 1_000_007), // Disambiguation: distinct from unescaped 0x01
        (b"clean_permanent_alpha".to_vec(), 1_000_008),
        (b"clean_permanent_beta".to_vec(), 1_000_009),
        (vec![0x00, 0x42, 0x01, 0x99, 0x00], 1_000_010),
        (vec![0xFF, 0xFE, 0x00, 0x01, 0xFD], 1_000_011),
    ];

    for (k, v) in &permanent_model {
        assert_eq!(map.insert(k, *v), None);
    }

    let n_writers = 4;
    let n_readers = 4;
    let n_iterations_per_writer = 200;
    let running = Arc::new(AtomicBool::new(true));

    // Spawn writers mutating dynamic keys concurrently
    let mut writer_handles = Vec::new();
    for w_idx in 0..n_writers {
        let map = Arc::clone(&map);
        writer_handles.push(thread::spawn(move || {
            for i in 0..n_iterations_per_writer {
                let val_base = (w_idx * 100_000 + i * 10) as u64;

                // (a) Clean key (exercises zero-allocation fast path)
                let clean_key = format!("writer_{w_idx}_iter_{i}_clean").into_bytes();
                map.insert(&clean_key, val_base + 1);

                // (b) Short escaped key (<= 32 bytes) with embedded 0x00 and 0x01 (exercises stack buffer)
                let mut short_esc = vec![0x00, w_idx as u8, 0x01];
                short_esc.extend_from_slice(&(i as u16).to_be_bytes());
                map.insert(&short_esc, val_base + 2);

                // (c) Long escaped key (> 32 bytes) (exercises heap fallback)
                let mut long_esc = vec![0x42; 40];
                long_esc[0] = 0x00;
                long_esc[1] = w_idx as u8;
                long_esc[35] = 0x01;
                long_esc[38] = (i & 0xFF) as u8;
                map.insert(&long_esc, val_base + 3);

                // Verification under writer
                assert_eq!(map.get(&clean_key), Some(val_base + 1));
                assert_eq!(map.get(&short_esc), Some(val_base + 2));
                assert_eq!(map.get(&long_esc), Some(val_base + 3));

                // Evict half to induce deletions and structural re-balancing concurrently
                if i % 2 == 0 {
                    assert_eq!(map.remove(&clean_key), Some(val_base + 1));
                    assert_eq!(map.remove(&long_esc), Some(val_base + 3));
                }
            }
        }));
    }

    // Spawn readers on OrderedBytesReader concurrently with writers
    let mut reader_handles = Vec::new();
    for _ in 0..n_readers {
        let map = Arc::clone(&map);
        let running = Arc::clone(&running);
        let permanent_model = permanent_model.clone();
        reader_handles.push(thread::spawn(move || {
            let reader = map.reader();
            let mut read_cycles = 0;
            let mut buf = [0u8; 64];

            while running.load(Ordering::Acquire) {
                // 1. Assert EVERY permanent key is found with its exact decoded value
                for (k, expected_val) in &permanent_model {
                    let val = reader.get(k);
                    assert_eq!(
                        val,
                        Some(*expected_val),
                        "Permanent key {k:?} must be present and match model value"
                    );
                    assert!(
                        reader.contains(k),
                        "Permanent key {k:?} must be reported as present by contains"
                    );
                }

                // 2. Caller-buffer decode integrity on permanent keys:
                // Verify that decoded keys are returned intact without raw escape leakage.
                // For key [0x00]: decoded length MUST be 1, slice &[0x00], NEVER [0x01, 0x01] (len 2).
                if let Ok(Some((len, val))) = map.next_at_or_after_decode_into(&[0x00], &mut buf) {
                    assert_eq!(
                        len, 1,
                        "Decoded length for [0x00] must be 1 byte, not 2 escape bytes"
                    );
                    assert_eq!(
                        &buf[..len],
                        &[0x00],
                        "Key [0x00] must decode with 0x00 byte intact"
                    );
                    assert_eq!(val, 1_000_002);
                }

                // For key [0x00, 0x00]: decoded length MUST be 2, slice &[0x00, 0x00].
                if let Ok(Some((len, val))) =
                    map.next_at_or_after_decode_into(&[0x00, 0x00], &mut buf)
                {
                    assert_eq!(len, 2, "Decoded length for [0x00, 0x00] must be 2 bytes");
                    assert_eq!(
                        &buf[..len],
                        &[0x00, 0x00],
                        "Key [0x00, 0x00] must decode intact"
                    );
                    assert_eq!(val, 1_000_003);
                }

                // For key [0x01, 0x01]: distinct from [0x00]
                if let Ok(Some((len, val))) =
                    map.next_at_or_after_decode_into(&[0x01, 0x01], &mut buf)
                {
                    assert_eq!(len, 2, "Decoded length for [0x01, 0x01] must be 2 bytes");
                    assert_eq!(&buf[..len], &[0x01, 0x01]);
                    assert_eq!(val, 1_000_006);
                }

                // 3. Navigation via first(): empty key has length 0, not [0x01, 0x01] or any escape
                if let Some((first_k, first_v)) = map.first() {
                    assert_eq!(
                        first_k,
                        Vec::<u8>::new(),
                        "Smallest key in map must be empty key []"
                    );
                    assert_eq!(first_v, 1_000_001);
                }

                read_cycles += 1;
            }

            assert!(
                read_cycles > 0,
                "Reader thread must have completed at least 1 read cycle"
            );
        }));
    }

    // Wait for writers to complete
    for h in writer_handles {
        h.join().unwrap();
    }

    // Stop readers
    running.store(false, Ordering::Release);
    for h in reader_handles {
        h.join().unwrap();
    }

    // Final integrity verification of permanent keys
    let reader = map.reader();
    for (k, expected_val) in &permanent_model {
        assert_eq!(reader.get(k), Some(*expected_val));
        assert!(reader.contains(k));
    }
}

#[test]
fn test_ordered_navigation_agreement() {
    let map = SyncExpanseOrderedBytesMap::new();
    let mut model = BTreeMap::new();

    let keys: Vec<Vec<u8>> = vec![
        vec![],
        vec![0],
        vec![0, 0],
        vec![0, 1],
        vec![1],
        vec![1, 0],
        vec![1, 1],
        vec![1, 2],
        vec![2],
        b"a".to_vec(),
        b"aa".to_vec(),
        b"ab".to_vec(),
        b"b".to_vec(),
        vec![255, 0],
        vec![255, 255],
    ];

    for (idx, k) in keys.iter().enumerate() {
        let val = (idx + 1) as u64;
        map.insert(k, val);
        model.insert(k.clone(), val);
    }

    assert_eq!(map.len(), model.len() as u64);

    // first & last
    let model_first = model.iter().next().map(|(k, &v)| (k.clone(), v));
    let model_last = model.iter().next_back().map(|(k, &v)| (k.clone(), v));
    assert_eq!(map.first(), model_first);
    assert_eq!(map.last(), model_last);

    // next_after & prev_before & next_at_or_after & prev_at_or_before
    for probe in &keys {
        // next_after (> probe)
        let model_next = model
            .range((
                std::ops::Bound::Excluded(probe.clone()),
                std::ops::Bound::Unbounded,
            ))
            .next()
            .map(|(k, &v)| (k.clone(), v));
        assert_eq!(
            map.next_after(probe),
            model_next,
            "next_after mismatch for probe {probe:?}"
        );

        // prev_before (< probe)
        let model_prev = model
            .range((
                std::ops::Bound::Unbounded,
                std::ops::Bound::Excluded(probe.clone()),
            ))
            .next_back()
            .map(|(k, &v)| (k.clone(), v));
        assert_eq!(
            map.prev_before(probe),
            model_prev,
            "prev_before mismatch for probe {probe:?}"
        );

        // next_at_or_after (>= probe)
        let model_at_after = model
            .range((
                std::ops::Bound::Included(probe.clone()),
                std::ops::Bound::Unbounded,
            ))
            .next()
            .map(|(k, &v)| (k.clone(), v));
        assert_eq!(
            map.next_at_or_after(probe),
            model_at_after,
            "next_at_or_after mismatch for probe {probe:?}"
        );

        // prev_at_or_before (<= probe)
        let model_at_before = model
            .range((
                std::ops::Bound::Unbounded,
                std::ops::Bound::Included(probe.clone()),
            ))
            .next_back()
            .map(|(k, &v)| (k.clone(), v));
        assert_eq!(
            map.prev_at_or_before(probe),
            model_at_before,
            "prev_at_or_before mismatch for probe {probe:?}"
        );
    }
}

#[test]
fn test_caller_buffer_decode_into() {
    let map = SyncExpanseOrderedBytesMap::new();
    map.insert(b"short", 10);
    map.insert(&[0x00, 0x01, 0x02, 0x03, 0x04], 20); // 5 bytes decoded

    let mut buf = [0u8; 32];

    // first_decode_into
    let (len, val) = map.first_decode_into(&mut buf).unwrap().unwrap();
    assert_eq!(&buf[..len], &[0x00, 0x01, 0x02, 0x03, 0x04]);
    assert_eq!(val, 20);

    // last_decode_into
    let (len, val) = map.last_decode_into(&mut buf).unwrap().unwrap();
    assert_eq!(&buf[..len], b"short");
    assert_eq!(val, 10);

    // BufferTooSmall error
    let mut tiny_buf = [0u8; 3];
    let err = map.first_decode_into(&mut tiny_buf).unwrap_err();
    assert_eq!(
        err,
        EscapeDecodeError::BufferTooSmall {
            required: 5,
            provided: 3
        }
    );

    // next_after_decode_into
    let (len, val) = map
        .next_after_decode_into(&[0x00, 0x01, 0x02, 0x03, 0x04], &mut buf)
        .unwrap()
        .unwrap();
    assert_eq!(&buf[..len], b"short");
    assert_eq!(val, 10);

    // prev_before_decode_into
    let (len, val) = map
        .prev_before_decode_into(b"short", &mut buf)
        .unwrap()
        .unwrap();
    assert_eq!(&buf[..len], &[0x00, 0x01, 0x02, 0x03, 0x04]);
    assert_eq!(val, 20);
}

#[test]
fn test_with_exclusive_and_locked() {
    let map = SyncExpanseOrderedBytesMap::new();

    // with_exclusive: batch atomic updates
    map.with_exclusive(|ex| {
        assert!(ex.is_empty());
        ex.insert(b"k1", 10);
        ex.insert(b"k2", 20);
        assert_eq!(ex.len(), 2);
        assert_eq!(ex.get(b"k1"), Some(10));
        assert!(ex.contains_key(b"k2"));

        // update
        let old = ex.update(b"k1", |cur| cur.map(|v| v * 2));
        assert_eq!(old, Some(10));
        assert_eq!(ex.get(b"k1"), Some(20));

        let rem = ex.remove(b"k2");
        assert_eq!(rem, Some(20));
        assert_eq!(ex.len(), 1);
    });

    assert_eq!(map.len(), 1);
    assert_eq!(map.get(b"k1"), Some(20));

    // with_locked: single-threaded read escape hatch
    let count = map.with_locked(|m| {
        assert_eq!(m.len(), 1);
        assert_eq!(m.get(b"k1"), Some(20));
        m.len()
    });
    assert_eq!(count, 1);
}

#[test]
fn test_from_conversions_and_into_inner() {
    let mut plain = ExpanseOrderedBytesMap::new();
    plain.insert(b"hello", 1);
    plain.insert(b"world", 2);
    plain.insert(&[0, 1, 2], 3);

    let sync_map = SyncExpanseOrderedBytesMap::from(plain);
    assert_eq!(sync_map.len(), 3);
    assert_eq!(sync_map.get(b"hello"), Some(1));
    assert_eq!(sync_map.get(b"world"), Some(2));
    assert_eq!(sync_map.get(&[0, 1, 2]), Some(3));

    let inner_str = sync_map.into_inner();
    assert_eq!(inner_str.len(), 3);
}
