//! Integration tests for the ExpanseBlobMap modern C ABI surface.

use core::ffi::c_void;
use expanse::blobmap::{
    ExpanseBlobView, expanse_blob_map_clear, expanse_blob_map_compact,
    expanse_blob_map_contains_key, expanse_blob_map_free, expanse_blob_map_get,
    expanse_blob_map_insert, expanse_blob_map_len, expanse_blob_map_mem_used, expanse_blob_map_new,
    expanse_blob_map_remove, expanse_blob_map_scan_filtered,
};

#[test]
fn test_capi_blob_map_lifecycle_and_basic_ops() {
    // SAFETY: Exercising C ABI functions with valid allocations and lifecycles.
    unsafe {
        let map = expanse_blob_map_new(64 * 1024);
        assert!(!map.is_null());
        assert_eq!(expanse_blob_map_len(map), 0);
        assert_eq!(expanse_blob_map_mem_used(map), 0);

        // Insert inline (0..=7 bytes)
        assert!(expanse_blob_map_insert(map, 1, core::ptr::null(), 0, 0));
        assert!(expanse_blob_map_insert(map, 2, b"hello".as_ptr(), 5, 0));
        assert!(expanse_blob_map_insert(map, 3, b"1234567".as_ptr(), 7, 0));

        // Insert arena (>7 bytes)
        assert!(expanse_blob_map_insert(
            map,
            4,
            b"12345678".as_ptr(),
            8,
            100
        ));
        let large_payload = vec![0xFEu8; 1024];
        assert!(expanse_blob_map_insert(
            map,
            5,
            large_payload.as_ptr(),
            1024,
            200
        ));

        assert_eq!(expanse_blob_map_len(map), 5);
        assert!(expanse_blob_map_contains_key(map, 1));
        assert!(expanse_blob_map_contains_key(map, 2));
        assert!(expanse_blob_map_contains_key(map, 3));
        assert!(expanse_blob_map_contains_key(map, 4));
        assert!(expanse_blob_map_contains_key(map, 5));
        assert!(!expanse_blob_map_contains_key(map, 6));

        // Get key 1 (0-length inline)
        let mut view = ExpanseBlobView {
            ptr: core::ptr::null(),
            len: 999,
            hot_meta: 999,
            is_inline: false,
        };
        assert!(expanse_blob_map_get(map, 1, &mut view));
        assert!(view.is_inline);
        assert_eq!(view.len, 0);

        // Get key 2 (5-byte inline)
        assert!(expanse_blob_map_get(map, 2, &mut view));
        assert!(view.is_inline);
        assert_eq!(view.len, 5);
        let slice2 = core::slice::from_raw_parts(view.ptr, view.len);
        assert_eq!(slice2, b"hello");

        // Get key 4 (8-byte arena)
        assert!(expanse_blob_map_get(map, 4, &mut view));
        assert!(!view.is_inline);
        assert_eq!(view.len, 8);
        assert_eq!(view.hot_meta, 100);
        let slice4 = core::slice::from_raw_parts(view.ptr, view.len);
        assert_eq!(slice4, b"12345678");

        // Get key 5 (1024-byte arena)
        assert!(expanse_blob_map_get(map, 5, &mut view));
        assert!(!view.is_inline);
        assert_eq!(view.len, 1024);
        assert_eq!(view.hot_meta, 200);
        let slice5 = core::slice::from_raw_parts(view.ptr, view.len);
        assert_eq!(slice5, &large_payload[..]);

        // Remove key 2
        assert!(expanse_blob_map_remove(map, 2));
        assert_eq!(expanse_blob_map_len(map), 4);
        assert!(!expanse_blob_map_contains_key(map, 2));
        assert!(!expanse_blob_map_remove(map, 2));

        // Clear
        expanse_blob_map_clear(map);
        assert_eq!(expanse_blob_map_len(map), 0);

        expanse_blob_map_free(map);
    }
}

struct ScanContext {
    visited_keys: Vec<u64>,
    visited_metas: Vec<u32>,
}

unsafe extern "C" fn test_predicate(key: u64, hot_meta: u32, _ctx: *mut c_void) -> bool {
    let _ = key;
    hot_meta >= 50
}

unsafe extern "C" fn test_callback(key: u64, view: ExpanseBlobView, ctx: *mut c_void) -> bool {
    // SAFETY: ctx is guaranteed to be a valid pointer to ScanContext.
    let context = unsafe { &mut *ctx.cast::<ScanContext>() };
    context.visited_keys.push(key);
    context.visited_metas.push(view.hot_meta);
    true
}

#[test]
fn test_capi_blob_map_filtered_scan_and_compaction() {
    // SAFETY: Testing C ABI scan and compaction with valid heap handles and context.
    unsafe {
        let map = expanse_blob_map_new(64 * 1024);

        // Populate with 20 items: keys 1..=20, payload 16 bytes each, meta = i * 10
        for i in 1..=20u64 {
            let data = [(i & 0xFF) as u8; 16];
            assert!(expanse_blob_map_insert(
                map,
                i,
                data.as_ptr(),
                data.len(),
                (i * 10) as u32
            ));
        }

        let mut context = ScanContext {
            visited_keys: Vec::new(),
            visited_metas: Vec::new(),
        };

        // Scan keys 1..=20 with predicate: meta >= 50 (keys 5..=20)
        let scanned = expanse_blob_map_scan_filtered(
            map,
            1,
            20,
            Some(test_predicate),
            Some(test_callback),
            (&mut context as *mut ScanContext).cast::<c_void>(),
        );

        assert_eq!(scanned, 16);
        assert_eq!(context.visited_keys.len(), 16);
        assert_eq!(context.visited_keys[0], 5);
        assert_eq!(context.visited_keys[15], 20);

        // Delete keys 1..=15
        for i in 1..=15u64 {
            assert!(expanse_blob_map_remove(map, i));
        }
        assert_eq!(expanse_blob_map_len(map), 5);

        // Compact
        assert!(expanse_blob_map_compact(map));
        assert_eq!(expanse_blob_map_len(map), 5);

        // Remaining keys 16..=20 still readable
        let mut view = ExpanseBlobView {
            ptr: core::ptr::null(),
            len: 0,
            hot_meta: 0,
            is_inline: false,
        };
        for i in 16..=20u64 {
            assert!(expanse_blob_map_get(map, i, &mut view));
            assert_eq!(view.hot_meta, (i * 10) as u32);
            assert_eq!(view.len, 16);
        }

        expanse_blob_map_free(map);
    }
}

#[test]
fn test_capi_null_safety() {
    // SAFETY: Verifying null pointer defensive safety across all exports.
    unsafe {
        assert_eq!(expanse_blob_map_len(core::ptr::null()), 0);
        assert_eq!(expanse_blob_map_mem_used(core::ptr::null()), 0);
        assert!(!expanse_blob_map_contains_key(core::ptr::null(), 42));
        assert!(!expanse_blob_map_insert(
            core::ptr::null_mut(),
            1,
            core::ptr::null(),
            0,
            0
        ));
        assert!(!expanse_blob_map_remove(core::ptr::null_mut(), 1));
        assert!(!expanse_blob_map_get(
            core::ptr::null(),
            1,
            core::ptr::null_mut()
        ));
        assert!(!expanse_blob_map_compact(core::ptr::null_mut()));
        expanse_blob_map_clear(core::ptr::null_mut());
        expanse_blob_map_free(core::ptr::null_mut());
    }
}
