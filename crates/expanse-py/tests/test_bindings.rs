//! Comprehensive unit tests for Expanse PyO3 Python bindings.

use _expanse::blobmap::ExpanseBlobMap;
use _expanse::bytesmap::ExpanseBytesMap;
use _expanse::map::ExpanseMap;
use _expanse::set::ExpanseSet;
use _expanse::strmap::ExpanseStrMap;
use _expanse::sync::{SyncExpanseMap, SyncExpanseSet};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyString};
use std::thread;

#[test]
fn test_expanse_map_basic_and_mapping_protocol() {
    Python::initialize();
    Python::attach(|_py| {
        let mut map = ExpanseMap::default();
        assert_eq!(map.__len__(), 0);
        assert!(map.is_empty());
        assert!(!map.__bool__());

        // Insert
        assert_eq!(map.insert(100, 1000), None);
        assert_eq!(map.insert(200, 2000), None);
        assert_eq!(map.insert(100, 1001), Some(1000));
        assert_eq!(map.__len__(), 2);
        assert!(!map.is_empty());
        assert!(map.__bool__());

        // Contains and get
        assert!(map.__contains__(100));
        assert!(map.__contains__(200));
        assert!(!map.__contains__(300));
        assert_eq!(map.get(100, None), Some(1001));
        assert_eq!(map.get(300, Some(999)), Some(999));
        assert_eq!(map.get(300, None), None);

        // __getitem__ and __setitem__
        assert_eq!(map.__getitem__(100).unwrap(), 1001);
        map.__setitem__(300, 3000);
        assert_eq!(map.__getitem__(300).unwrap(), 3000);
        assert!(map.__getitem__(400).is_err());

        // __delitem__
        assert!(map.__delitem__(200).is_ok());
        assert!(!map.__contains__(200));
        assert!(map.__delitem__(200).is_err());

        // pop
        assert_eq!(map.pop(300, None).unwrap(), 3000);
        assert_eq!(map.pop(300, Some(42)).unwrap(), 42);
        assert!(map.pop(300, None).is_err());

        // remove
        assert_eq!(map.remove(100), Some(1001));
        assert_eq!(map.remove(100), None);
        assert_eq!(map.__len__(), 0);

        // memory and stats
        assert_eq!(map.mem_used(), 0);
        assert_eq!(map.__repr__(), "ExpanseMap(len=0)");

        // clear
        map.insert(1, 10);
        map.insert(2, 20);
        assert_eq!(map.__len__(), 2);
        map.clear();
        assert_eq!(map.__len__(), 0);
    });
}

#[test]
fn test_expanse_map_ordered_navigation_and_iterators() {
    Python::initialize();
    Python::attach(|_py| {
        let mut map = ExpanseMap::default();
        let keys = [10u64, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        for &k in &keys {
            map.insert(k, k * 10);
        }

        // first and last
        assert_eq!(map.first(), Some((10, 100)));
        assert_eq!(map.last(), Some((100, 1000)));

        // next (exclusive / inclusive)
        assert_eq!(map.next(30, false), Some((40, 400)));
        assert_eq!(map.next(30, true), Some((30, 300)));
        assert_eq!(map.next(35, false), Some((40, 400)));
        assert_eq!(map.next(100, false), None);

        // prev (exclusive / inclusive)
        assert_eq!(map.prev(50, false), Some((40, 400)));
        assert_eq!(map.prev(50, true), Some((50, 500)));
        assert_eq!(map.prev(45, false), Some((40, 400)));
        assert_eq!(map.prev(10, false), None);

        // count_below (rank)
        assert_eq!(map.count_below(10), 0);
        assert_eq!(map.count_below(30), 2);
        assert_eq!(map.count_below(35), 3);
        assert_eq!(map.count_below(101), 10);

        // by_count (select)
        assert_eq!(map.by_count(0), Some((10, 100)));
        assert_eq!(map.by_count(4), Some((50, 500)));
        assert_eq!(map.by_count(9), Some((100, 1000)));
        assert_eq!(map.by_count(10), None);

        // count_range
        assert_eq!(map.count_range(20, 50), 4);
        assert_eq!(map.count_range(25, 55), 3);

        // range
        let r = map.range(Some(30), Some(60), false);
        assert_eq!(r, vec![(30, 300), (40, 400), (50, 500)]);

        let r_inc = map.range(Some(30), Some(60), true);
        assert_eq!(r_inc, vec![(30, 300), (40, 400), (50, 500), (60, 600)]);

        // keys / values / items
        let k_vec = map.keys();
        assert_eq!(k_vec.len(), 10);
        assert_eq!(k_vec[0], 10);
        assert_eq!(k_vec[1], 20);

        let v_vec = map.values();
        assert_eq!(v_vec[0], 100);
        assert_eq!(v_vec[1], 200);

        let it_vec = map.items();
        assert_eq!(it_vec[0], (10, 100));
    });
}

#[test]
fn test_expanse_map_bulk_ingest_and_update() {
    Python::initialize();
    Python::attach(|py| {
        let mut map = ExpanseMap::default();

        // Dict update
        let dict = PyDict::new(py);
        dict.set_item(1u64, 10u64).unwrap();
        dict.set_item(2u64, 20u64).unwrap();
        map.update(dict.as_any()).unwrap();
        assert_eq!(map.__len__(), 2);
        assert_eq!(map.get(1, None), Some(10));
    });
}

#[test]
fn test_expanse_set_basic_and_navigation() {
    Python::initialize();
    Python::attach(|_py| {
        let mut set = ExpanseSet::default();
        assert_eq!(set.__len__(), 0);
        assert!(set.is_empty());
        assert!(!set.__bool__());

        // Insert / Add
        assert!(set.insert(10));
        assert!(!set.insert(10));
        assert!(set.add(20));
        assert!(!set.add(20));
        set.add(30);
        assert_eq!(set.__len__(), 3);

        // Contains
        assert!(set.__contains__(10));
        assert!(set.__contains__(20));
        assert!(!set.__contains__(40));

        // Discard / Remove
        assert!(set.discard(20));
        assert!(!set.discard(20));
        assert_eq!(set.__len__(), 2);
        assert!(set.remove(10));
        assert!(!set.remove(10));

        // Navigation
        set.clear();
        for &k in &[5u64, 15, 25, 35, 45, 55] {
            set.insert(k);
        }
        assert_eq!(set.first(), Some(5));
        assert_eq!(set.last(), Some(55));
        assert_eq!(set.next(25, false), Some(35));
        assert_eq!(set.next(25, true), Some(25));
        assert_eq!(set.prev(35, false), Some(25));
        assert_eq!(set.prev(35, true), Some(35));
        assert_eq!(set.count_below(25), 2);
        assert_eq!(set.by_count(3), Some(35));
        assert_eq!(set.count_range(15, 45), 4);

        // Pop
        assert_eq!(set.pop().unwrap(), 5);
        assert_eq!(set.first(), Some(15));

        // Range
        let r = set.range(Some(15), Some(45), false);
        assert_eq!(r, vec![15, 25, 35]);
    });
}

#[test]
fn test_sync_expanse_map_multithreaded_gil_free() {
    Python::initialize();

    let sync_map = SyncExpanseMap::default();

    // Populate initial data
    Python::attach(|py| {
        for i in 0u64..1000 {
            sync_map.insert(py, i, i * 2);
        }
        assert_eq!(sync_map.__len__(py), 1000);
        assert!(!sync_map.is_empty(py));
        assert!(sync_map.__contains__(py, 500));
        assert_eq!(sync_map.__getitem__(py, 500).unwrap(), 1000);
        assert_eq!(sync_map.first(py), Some((0, 0)));
        assert_eq!(sync_map.last(py), Some((999, 1998)));
        assert_eq!(sync_map.count_below(py, 500), 500);
        assert_eq!(sync_map.by_count(py, 100), Some((100, 200)));
    });

    // Concurrent multithreaded queries without GIL contention
    let mut handles = Vec::new();
    for thread_id in 0..8 {
        let map_clone = sync_map.clone();
        let handle = thread::spawn(move || {
            Python::initialize();
            Python::attach(|py| {
                for i in 0..500 {
                    let key = ((thread_id * 100 + i) % 1000) as u64;
                    let val = map_clone.get(py, key, None);
                    assert_eq!(val, Some(key * 2));
                    assert!(map_clone.__contains__(py, key));

                    let next_val = map_clone.next(py, key, false);
                    if key < 999 {
                        assert_eq!(next_val, Some((key + 1, (key + 1) * 2)));
                    }

                    let r = map_clone.range(py, Some(key), Some(key + 5), false);
                    assert!(r.__len__() <= 6);
                }
            });
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().unwrap();
    }

    // Verify mutating and clearing
    Python::attach(|py| {
        assert_eq!(sync_map.remove(py, 500).unwrap(), 1000);
        assert!(!sync_map.__contains__(py, 500));
        sync_map.clear(py);
        assert_eq!(sync_map.__len__(py), 0);
    });
}

#[test]
fn test_sync_expanse_set_multithreaded_gil_free() {
    Python::initialize();

    let sync_set = SyncExpanseSet::default();

    Python::attach(|py| {
        for i in 0u64..1000 {
            sync_set.insert(py, i);
        }
        assert_eq!(sync_set.__len__(py), 1000);
        assert!(sync_set.__contains__(py, 500));
        assert_eq!(sync_set.first(py), Some(0));
        assert_eq!(sync_set.last(py), Some(999));
        assert_eq!(sync_set.count_below(py, 500), 500);
        assert_eq!(sync_set.by_count(py, 100), Some(100));
    });

    let mut handles = Vec::new();
    for thread_id in 0..8 {
        let set_clone = sync_set.clone();
        let handle = thread::spawn(move || {
            Python::initialize();
            Python::attach(|py| {
                for i in 0..500 {
                    let key = ((thread_id * 100 + i) % 1000) as u64;
                    assert!(set_clone.__contains__(py, key));

                    let next_key = set_clone.next(py, key, false);
                    if key < 999 {
                        assert_eq!(next_key, Some(key + 1));
                    }

                    let r = set_clone.range(py, Some(key), Some(key + 5), false);
                    assert!(r.__len__() <= 6);
                }
            });
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().unwrap();
    }

    Python::attach(|py| {
        // remove now returns a bool (mirrors ExpanseSet.remove), not a PyResult.
        assert!(sync_set.remove(py, 500));
        assert!(!sync_set.__contains__(py, 500));
        assert!(!sync_set.remove(py, 500)); // already gone -> false, no KeyError
        sync_set.clear(py);
        assert_eq!(sync_set.__len__(py), 0);
    });
}

#[test]
fn test_expanse_str_map_lexicographical_and_nul_check() {
    Python::initialize();
    Python::attach(|py| {
        let mut strmap = ExpanseStrMap::default();
        assert_eq!(strmap.__len__(), 0);

        let k1 = PyString::new(py, "apple");
        let k2 = PyString::new(py, "banana");
        let k3 = PyString::new(py, "cherry");

        strmap.insert(k1.as_any(), 100).unwrap();
        strmap.insert(k2.as_any(), 200).unwrap();
        strmap.insert(k3.as_any(), 300).unwrap();

        assert_eq!(strmap.__len__(), 3);
        assert!(strmap.__contains__(k2.as_any()).unwrap());
        assert_eq!(strmap.__getitem__(k2.as_any()).unwrap(), 200);

        // Keys now come back as a Python object (str for UTF-8, bytes otherwise).
        let key_str = |obj: &Py<PyAny>| obj.bind(py).extract::<String>().unwrap();

        // Lexicographical navigation
        let first = strmap.first(py).unwrap();
        assert_eq!((key_str(&first.0), first.1), ("apple".to_string(), 100));
        let last = strmap.last(py).unwrap();
        assert_eq!((key_str(&last.0), last.1), ("cherry".to_string(), 300));
        let nxt = strmap.next(py, k1.as_any(), false).unwrap().unwrap();
        assert_eq!((key_str(&nxt.0), nxt.1), ("banana".to_string(), 200));
        let prv = strmap.prev(py, k3.as_any(), false).unwrap().unwrap();
        assert_eq!((key_str(&prv.0), prv.1), ("banana".to_string(), 200));

        // NUL byte rejection
        let nul_key = PyString::new(py, "bad\0key");
        assert!(strmap.insert(nul_key.as_any(), 999).is_err());
        assert!(strmap.__contains__(nul_key.as_any()).is_err());

        // Range
        let r = strmap
            .range(py, Some(k1.as_any()), Some(k3.as_any()), false)
            .unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!((key_str(&r[0].0), r[0].1), ("apple".to_string(), 100));
        assert_eq!((key_str(&r[1].0), r[1].1), ("banana".to_string(), 200));

        // Iteration
        let k_vec: Vec<String> = strmap.keys(py).iter().map(key_str).collect();
        assert_eq!(k_vec, vec!["apple", "banana", "cherry"]);

        // Removal and clear
        assert_eq!(strmap.remove(k2.as_any()).unwrap(), Some(200));
        assert_eq!(strmap.__len__(), 2);
        strmap.clear();
        assert_eq!(strmap.__len__(), 0);
    });
}

#[test]
fn test_expanse_bytes_map_arbitrary_binary_keys_and_nul() {
    Python::initialize();
    Python::attach(|py| {
        let mut bytesmap = ExpanseBytesMap::default();
        assert_eq!(bytesmap.__len__(), 0);

        // Keys containing NUL bytes and binary data
        let b1 = PyBytes::new(py, b"\x00\x01\x02\x03");
        let b2 = PyBytes::new(py, b"\x00\x01\x02\x04");
        let b3 = PyBytes::new(py, b"hello\x00world");

        bytesmap.insert(b1.as_any(), 10).unwrap();
        bytesmap.insert(b2.as_any(), 20).unwrap();
        bytesmap.insert(b3.as_any(), 30).unwrap();

        assert_eq!(bytesmap.__len__(), 3);
        assert!(bytesmap.__contains__(b1.as_any()).unwrap());
        assert!(bytesmap.__contains__(b3.as_any()).unwrap());
        assert_eq!(bytesmap.__getitem__(b3.as_any()).unwrap(), 30);

        // Values and items
        let v_iter = bytesmap.values();
        assert_eq!(v_iter.__len__(), 3);

        let it_iter = bytesmap.items();
        assert_eq!(it_iter.__len__(), 3);

        // Removal and clear
        assert_eq!(bytesmap.remove(b1.as_any()).unwrap(), Some(10));
        assert_eq!(bytesmap.__len__(), 2);
        bytesmap.clear();
        assert_eq!(bytesmap.__len__(), 0);
    });
}

#[test]
fn test_expanse_blob_map_pyo3_bindings() {
    Python::initialize();
    Python::attach(|py| {
        let mut map = ExpanseBlobMap::new(Some(64 * 1024));
        assert_eq!(map.__len__(), 0);
        assert!(map.is_empty());
        assert!(!map.__bool__());

        // Insert inline (0..7 bytes)
        let b_inline0 = PyBytes::new(py, b"");
        let b_inline4 = PyBytes::new(py, b"test");
        let b_inline7 = PyBytes::new(py, b"1234567");

        map.insert(1, b_inline0.as_any(), 0).unwrap();
        map.insert(2, b_inline4.as_any(), 0).unwrap();
        map.insert(3, b_inline7.as_any(), 0).unwrap();

        // Insert arena (>7 bytes)
        let b_arena = PyBytes::new(py, b"a longer payload that goes into arena");
        map.insert(4, b_arena.as_any(), 100).unwrap();

        assert_eq!(map.__len__(), 4);
        assert!(map.__contains__(1));
        assert!(map.__contains__(4));
        assert!(!map.__contains__(5));

        // Get
        let (v1, m1) = map.get(py, 1).unwrap();
        assert_eq!(v1.as_bytes(), b"");
        assert_eq!(m1, 0);

        let (v2, m2) = map.get(py, 2).unwrap();
        assert_eq!(v2.as_bytes(), b"test");
        assert_eq!(m2, 0);

        let (v4, m4) = map.get(py, 4).unwrap();
        assert_eq!(v4.as_bytes(), b"a longer payload that goes into arena");
        assert_eq!(m4, 100);

        // __getitem__ and __setitem__
        let v2_bytes = map.__getitem__(py, 2).unwrap();
        assert_eq!(v2_bytes.as_bytes(), b"test");

        let b_new = PyBytes::new(py, b"new_val");
        map.__setitem__(5, b_new.as_any()).unwrap();
        assert_eq!(map.__getitem__(py, 5).unwrap().as_bytes(), b"new_val");

        // Scan filtered
        let scan_results = map.scan_filtered(py, 1, 5, None, None).unwrap();
        assert_eq!(scan_results.len(), 5);

        // Delete
        assert!(map.remove(2));
        assert_eq!(map.__len__(), 4);
        assert!(map.__delitem__(1).is_ok());
        assert_eq!(map.__len__(), 3);

        // Compact
        let (live_b, live_a, _, _) = map.compact().unwrap();
        assert_eq!(live_b, live_a);

        // File serialization
        let temp_file = std::env::temp_dir().join("py_blobmap_test.bin");
        let path_str = temp_file.to_str().unwrap();
        map.save_to_file(path_str).unwrap();

        let loaded = ExpanseBlobMap::load_from_file(path_str).unwrap();
        assert_eq!(loaded.__len__(), 3);
        let (v4_loaded, m4_loaded) = loaded.get(py, 4).unwrap();
        assert_eq!(
            v4_loaded.as_bytes(),
            b"a longer payload that goes into arena"
        );
        assert_eq!(m4_loaded, 100);

        let _ = std::fs::remove_file(temp_file);

        // Clear
        map.clear();
        assert_eq!(map.__len__(), 0);
    });
}
