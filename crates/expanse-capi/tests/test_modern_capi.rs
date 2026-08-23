//! Integration tests for modern expanse C API exports and version stamping.

use core::ffi::CStr;
use expanse::modern::{expanse_map_free, expanse_map_insert, expanse_map_new, expanse_version};

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
