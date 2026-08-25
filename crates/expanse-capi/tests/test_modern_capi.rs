//! Integration tests for modern expanse C API exports and version stamping.

use core::ffi::CStr;
use expanse::modern::{
    expanse_map_free, expanse_map_get_batch, expanse_map_insert, expanse_map_new,
    expanse_set_contains_batch, expanse_set_free, expanse_set_insert, expanse_set_new,
    expanse_version,
};

#[test]
fn test_expanse_version_stamping() {
    let ptr = expanse_version();
    assert!(!ptr.is_null(), "expanse_version() returned null pointer");

    // SAFETY: expanse_version() returns a static NUL-terminated C string.
    let version_str = unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .expect("expanse_version() must return valid UTF-8");

    let pkg_version = env!("CARGO_PKG_VERSION");
    assert!(
        version_str.contains(pkg_version),
        "expanse_version() '{version_str}' does not contain package version '{pkg_version}'"
    );
}

#[test]
fn test_modern_capi_basic_smoke() {
    // SAFETY: Exercising C ABI functions with valid allocations and lifecycles.
    unsafe {
        let map = expanse_map_new();
        assert!(!map.is_null());
        assert!(expanse_map_insert(map, 42, 100, core::ptr::null_mut()));
        expanse_map_free(map);
    }
}

#[test]
fn test_modern_capi_batch_operations() {
    // SAFETY: Exercising C ABI batch lookup operations.
    unsafe {
        let set = expanse_set_new();
        let map = expanse_map_new();

        for i in 0..1000u64 {
            let k = i * 7;
            assert!(expanse_set_insert(set, k));
            assert!(expanse_map_insert(map, k, i * 100, core::ptr::null_mut()));
        }

        let query_keys = [0u64, 7, 14, 21, 22, 28, 35, 42, 43, 7000];
        let mut set_present = [false; 10];
        let set_found =
            expanse_set_contains_batch(set, query_keys.as_ptr(), set_present.as_mut_ptr(), 10);
        assert_eq!(set_found, 7);
        assert_eq!(
            set_present,
            [
                true, true, true, true, false, true, true, true, false, false
            ]
        );

        let mut map_values = [0u64; 10];
        let mut map_found_flags = [false; 10];
        let map_found = expanse_map_get_batch(
            map,
            query_keys.as_ptr(),
            map_values.as_mut_ptr(),
            map_found_flags.as_mut_ptr(),
            10,
        );
        assert_eq!(map_found, 7);
        assert_eq!(
            map_found_flags,
            [
                true, true, true, true, false, true, true, true, false, false
            ]
        );
        assert_eq!(map_values[0], 0);
        assert_eq!(map_values[1], 100);
        assert_eq!(map_values[2], 200);
        assert_eq!(map_values[3], 300);
        assert_eq!(map_values[5], 400);

        expanse_set_free(set);
        expanse_map_free(map);
    }
}
