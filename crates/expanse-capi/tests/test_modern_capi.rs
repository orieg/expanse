//! Integration tests for modern expanse C API exports and version stamping.

use core::ffi::{CStr, c_void};
use expanse::modern::{
    expanse_bytesmap_free, expanse_bytesmap_get, expanse_bytesmap_insert, expanse_bytesmap_new,
    expanse_map_free, expanse_map_get_batch, expanse_map_insert, expanse_map_new,
    expanse_set_contains_batch, expanse_set_free, expanse_set_insert, expanse_set_new,
    expanse_version,
};
use expanse::{
    JError, JU_ERRNO_NULLPINDEX, Judy1FreeArray, Judy1Next, Judy1Prev, Judy1Set, JudyHSFreeArray,
    JudyHSGet, JudyHSIns, JudyLFreeArray, JudyLIns, JudyLNext, JudyLPrev, Word,
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

#[test]
fn test_modern_capi_null_and_bounds_safety() {
    // SAFETY: Testing null pointer rejection and bounds safety.
    unsafe {
        let keys = [1u64, 2, 3];
        let mut present = [true; 3];
        let mut values = [99u64; 3];

        // 1. Null set/map
        assert_eq!(
            expanse_set_contains_batch(core::ptr::null(), keys.as_ptr(), present.as_mut_ptr(), 3),
            0
        );
        assert_eq!(
            expanse_map_get_batch(
                core::ptr::null(),
                keys.as_ptr(),
                values.as_mut_ptr(),
                core::ptr::null_mut(),
                3
            ),
            0
        );

        // 2. Count = 0
        let set = expanse_set_new();
        assert_eq!(
            expanse_set_contains_batch(set, keys.as_ptr(), present.as_mut_ptr(), 0),
            0
        );

        // 3. Count exceeding isize::MAX bounds (verifies the new isize::MAX / 8 guard)
        let huge_count = (isize::MAX as usize / core::mem::size_of::<u64>()) + 1;
        assert_eq!(
            expanse_set_contains_batch(set, keys.as_ptr(), core::ptr::null_mut(), huge_count),
            0
        );

        let map = expanse_map_new();
        assert_eq!(
            expanse_map_get_batch(
                map,
                keys.as_ptr(),
                values.as_mut_ptr(),
                core::ptr::null_mut(),
                huge_count
            ),
            0
        );

        expanse_set_free(set);
        expanse_map_free(map);
    }
}

#[test]
fn test_bytesmap_and_judyhs_isize_bounds() {
    // SAFETY: Testing FFI bounds checking against > isize::MAX lengths.
    unsafe {
        let dummy_ptr = 0x1000 as *const c_void;
        let huge_len = (isize::MAX as usize) + 1;

        // 1. Modern bytesmap surface
        let bmap = expanse_bytesmap_new();
        assert!(!expanse_bytesmap_insert(
            bmap,
            dummy_ptr,
            huge_len,
            42,
            core::ptr::null_mut()
        ));
        let mut val = 0u64;
        assert!(!expanse_bytesmap_get(bmap, dummy_ptr, huge_len, &mut val));
        expanse_bytesmap_free(bmap);

        // 2. Legacy JudyHS surface
        let mut judy_hs: *mut c_void = core::ptr::null_mut();
        let mut jerr = JError {
            je_errno: 0,
            je_err_id: 0,
            je_reserved: [0; 4],
        };
        let slot = JudyHSIns(&mut judy_hs, dummy_ptr, huge_len, &mut jerr);
        assert_eq!(slot, usize::MAX as *mut c_void); // PJERR
        assert_eq!(jerr.je_errno, JU_ERRNO_NULLPINDEX);

        let get_slot = JudyHSGet(judy_hs, dummy_ptr, huge_len);
        assert_eq!(get_slot, core::ptr::null_mut());

        JudyHSFreeArray(&mut judy_hs, &mut jerr);
    }
}

#[test]
fn test_legacy_judy_navigation_boundary_guards() {
    // SAFETY: Exercising Judy1 and JudyL boundary navigation.
    unsafe {
        let mut j1: *mut c_void = core::ptr::null_mut();
        let mut jl: *mut c_void = core::ptr::null_mut();
        let mut jerr = JError {
            je_errno: 0,
            je_err_id: 0,
            je_reserved: [0; 4],
        };

        // Populate with boundary keys
        Judy1Set(&mut j1, 0, &mut jerr);
        Judy1Set(&mut j1, Word::MAX, &mut jerr);

        let slot = JudyLIns(&mut jl, 0, &mut jerr);
        assert!(!slot.is_null());
        *(slot.cast::<Word>()) = 100;

        let slot_max = JudyLIns(&mut jl, Word::MAX, &mut jerr);
        assert!(!slot_max.is_null());
        *(slot_max.cast::<Word>()) = 200;

        // Verify Prev on 0 returns 0 (None), does NOT wrap to Word::MAX
        let mut index: Word = 0;
        let rc = Judy1Prev(j1, &mut index, &mut jerr);
        assert_eq!(rc, 0, "Judy1Prev on 0 must return 0 (not found)");
        assert_eq!(index, 0);

        let mut l_index: Word = 0;
        let pvalue = JudyLPrev(jl, &mut l_index, &mut jerr);
        assert_eq!(
            pvalue,
            core::ptr::null_mut(),
            "JudyLPrev on 0 must return null"
        );
        assert_eq!(l_index, 0);

        // Verify Next on Word::MAX returns 0 (None), does NOT wrap to 0
        let mut index_max: Word = Word::MAX;
        let rc = Judy1Next(j1, &mut index_max, &mut jerr);
        assert_eq!(rc, 0, "Judy1Next on Word::MAX must return 0 (not found)");
        assert_eq!(index_max, Word::MAX);

        let mut l_index_max: Word = Word::MAX;
        let pvalue = JudyLNext(jl, &mut l_index_max, &mut jerr);
        assert_eq!(
            pvalue,
            core::ptr::null_mut(),
            "JudyLNext on Word::MAX must return null"
        );
        assert_eq!(l_index_max, Word::MAX);

        Judy1FreeArray(&mut j1, &mut jerr);
        JudyLFreeArray(&mut jl, &mut jerr);
    }
}
