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

#[test]
fn test_modern_capi_map_navigation() {
    use expanse::modern::{
        expanse_map_first, expanse_map_free, expanse_map_insert, expanse_map_last, expanse_map_new,
        expanse_map_next_after, expanse_map_next_at_or_after, expanse_map_prev_at_or_before,
        expanse_map_prev_before,
    };
    // SAFETY: Exercising C ABI map navigation methods.
    unsafe {
        let map = expanse_map_new();
        assert!(!map.is_null());

        expanse_map_insert(map, 10, 100, core::ptr::null_mut());
        expanse_map_insert(map, 20, 200, core::ptr::null_mut());
        expanse_map_insert(map, 30, 300, core::ptr::null_mut());

        let mut k = 0;
        let mut v = 0;

        assert!(expanse_map_first(map, &raw mut k, &raw mut v));
        assert_eq!(k, 10);
        assert_eq!(v, 100);

        assert!(expanse_map_last(map, &raw mut k, &raw mut v));
        assert_eq!(k, 30);
        assert_eq!(v, 300);

        assert!(expanse_map_next_at_or_after(
            map, 15, &raw mut k, &raw mut v
        ));
        assert_eq!(k, 20);
        assert_eq!(v, 200);

        assert!(expanse_map_next_at_or_after(
            map, 20, &raw mut k, &raw mut v
        ));
        assert_eq!(k, 20);
        assert_eq!(v, 200);

        assert!(expanse_map_next_after(map, 20, &raw mut k, &raw mut v));
        assert_eq!(k, 30);
        assert_eq!(v, 300);

        assert!(!expanse_map_next_after(map, 30, &raw mut k, &raw mut v));

        assert!(expanse_map_prev_at_or_before(
            map, 25, &raw mut k, &raw mut v
        ));
        assert_eq!(k, 20);
        assert_eq!(v, 200);

        assert!(expanse_map_prev_at_or_before(
            map, 20, &raw mut k, &raw mut v
        ));
        assert_eq!(k, 20);
        assert_eq!(v, 200);

        assert!(expanse_map_prev_before(map, 20, &raw mut k, &raw mut v));
        assert_eq!(k, 10);
        assert_eq!(v, 100);

        assert!(!expanse_map_prev_before(map, 10, &raw mut k, &raw mut v));

        expanse_map_free(map);
    }
}

/// The six ordered reads through a sync map reader handle (#900): found,
/// absent, both ends of the key space, NULL out-pointers and a NULL reader,
/// then the same reads against a live writer, where every probe has exactly
/// one right answer or one of two named ones.
#[test]
fn test_sync_map_reader_ordered_reads() {
    use expanse::modern::{
        expanse_sync_map_free, expanse_sync_map_insert, expanse_sync_map_new,
        expanse_sync_map_reader_first, expanse_sync_map_reader_free, expanse_sync_map_reader_last,
        expanse_sync_map_reader_new, expanse_sync_map_reader_next_after,
        expanse_sync_map_reader_next_at_or_after, expanse_sync_map_reader_prev_at_or_before,
        expanse_sync_map_reader_prev_before, expanse_sync_map_remove,
    };
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Runs one ordered read into locals: `Some((key, value))` when it found
    /// an entry. The receiver call runs before the tuple is built.
    fn entry(read: impl FnOnce(*mut u64, *mut u64) -> bool) -> Option<(u64, u64)> {
        let (mut k, mut v) = (0u64, 0u64);
        read(&raw mut k, &raw mut v).then_some((k, v))
    }

    let e = |key: u64| Some((key, key ^ 0x5A5A));
    // SAFETY: `m` is live until the final free and outlives the reader, which
    // is freed first and used only from this thread; out-pointers are live
    // locals or null; a null reader is a documented `false`.
    unsafe {
        let m = expanse_sync_map_new();
        let r = expanse_sync_map_reader_new(m);
        assert!(!r.is_null());

        // Empty: nothing in either direction, from either end.
        assert_eq!(entry(|k, v| expanse_sync_map_reader_first(r, k, v)), None);
        assert_eq!(entry(|k, v| expanse_sync_map_reader_last(r, k, v)), None);
        assert_eq!(
            entry(|k, v| expanse_sync_map_reader_next_at_or_after(r, 0, k, v)),
            None
        );
        assert_eq!(
            entry(|k, v| expanse_sync_map_reader_prev_at_or_before(r, u64::MAX, k, v)),
            None
        );

        for key in [0u64, 10, 20, 30, u64::MAX] {
            assert!(expanse_sync_map_insert(
                m,
                key,
                key ^ 0x5A5A,
                core::ptr::null_mut()
            ));
        }

        assert_eq!(entry(|k, v| expanse_sync_map_reader_first(r, k, v)), e(0));
        assert_eq!(
            entry(|k, v| expanse_sync_map_reader_last(r, k, v)),
            e(u64::MAX)
        );
        assert_eq!(
            entry(|k, v| expanse_sync_map_reader_next_at_or_after(r, 15, k, v)),
            e(20)
        );
        assert_eq!(
            entry(|k, v| expanse_sync_map_reader_next_at_or_after(r, 20, k, v)),
            e(20)
        );
        assert_eq!(
            entry(|k, v| expanse_sync_map_reader_next_after(r, 20, k, v)),
            e(30)
        );
        assert_eq!(
            entry(|k, v| expanse_sync_map_reader_next_after(r, 30, k, v)),
            e(u64::MAX)
        );
        assert_eq!(
            entry(|k, v| expanse_sync_map_reader_next_at_or_after(r, u64::MAX, k, v)),
            e(u64::MAX)
        );
        assert_eq!(
            entry(|k, v| expanse_sync_map_reader_next_after(r, u64::MAX, k, v)),
            None
        );
        assert_eq!(
            entry(|k, v| expanse_sync_map_reader_prev_at_or_before(r, 25, k, v)),
            e(20)
        );
        assert_eq!(
            entry(|k, v| expanse_sync_map_reader_prev_at_or_before(r, 20, k, v)),
            e(20)
        );
        assert_eq!(
            entry(|k, v| expanse_sync_map_reader_prev_before(r, 20, k, v)),
            e(10)
        );
        assert_eq!(
            entry(|k, v| expanse_sync_map_reader_prev_at_or_before(r, 0, k, v)),
            e(0)
        );
        assert_eq!(
            entry(|k, v| expanse_sync_map_reader_prev_before(r, 0, k, v)),
            None
        );

        // Remove both ends: the searches that reached them now come up empty.
        for key in [0u64, u64::MAX] {
            assert!(expanse_sync_map_remove(m, key, core::ptr::null_mut()));
        }
        assert_eq!(
            entry(|k, v| expanse_sync_map_reader_prev_at_or_before(r, 5, k, v)),
            None
        );
        assert_eq!(
            entry(|k, v| expanse_sync_map_reader_next_at_or_after(r, 31, k, v)),
            None
        );
        assert_eq!(entry(|k, v| expanse_sync_map_reader_first(r, k, v)), e(10));
        assert_eq!(entry(|k, v| expanse_sync_map_reader_last(r, k, v)), e(30));

        // NULL out-pointers: the found flag alone. NULL reader: false.
        assert!(expanse_sync_map_reader_next_after(
            r,
            10,
            core::ptr::null_mut(),
            core::ptr::null_mut()
        ));
        assert_eq!(
            entry(|k, v| expanse_sync_map_reader_first(core::ptr::null(), k, v)),
            None
        );

        expanse_sync_map_reader_free(r);
        expanse_sync_map_free(m);
    }

    // Against a live writer. Even keys below N are inserted up front and never
    // touched; the writer churns the odd keys between them, always with value
    // `k * 3`. So an even probe's at-or-after / at-or-before answer is the probe
    // itself, and its strict neighbour is the odd key (with its only value) or
    // the next even key.
    const N: u64 = 4096;
    let m = expanse_sync_map_new();
    for k in (0..N).step_by(2) {
        // SAFETY: live handle; null `old_out`.
        assert!(unsafe { expanse_sync_map_insert(m, k, k * 3, core::ptr::null_mut()) });
    }
    let m_addr = m as usize;
    let stop = AtomicBool::new(false);
    std::thread::scope(|s| {
        for t in 0..2u64 {
            let stop = &stop;
            s.spawn(move || {
                // SAFETY: the map outlives this scope; the reader is created,
                // used and freed on this thread; out-pointers are locals.
                unsafe {
                    let r = expanse_sync_map_reader_new(m_addr as *const _);
                    assert!(!r.is_null());
                    let mut i = 0u64;
                    while !stop.load(Ordering::Relaxed) || i < 20_000 {
                        let even = ((i * 7919 + t) % N) & !1;
                        let at = |key: u64| Some((key, key * 3));
                        assert_eq!(
                            entry(|k, v| expanse_sync_map_reader_next_at_or_after(r, even, k, v)),
                            at(even)
                        );
                        assert_eq!(
                            entry(|k, v| expanse_sync_map_reader_prev_at_or_before(r, even, k, v)),
                            at(even)
                        );
                        if even + 2 < N {
                            let got =
                                entry(|k, v| expanse_sync_map_reader_next_after(r, even, k, v));
                            assert!(got == at(even + 1) || got == at(even + 2), "{got:?}");
                        }
                        if even >= 2 {
                            let got =
                                entry(|k, v| expanse_sync_map_reader_prev_before(r, even, k, v));
                            assert!(got == at(even - 1) || got == at(even - 2), "{got:?}");
                        }
                        assert_eq!(entry(|k, v| expanse_sync_map_reader_first(r, k, v)), at(0));
                        i += 1;
                    }
                    expanse_sync_map_reader_free(r);
                }
            });
        }
        for round in 0..20u64 {
            for k in (1..N).step_by(2) {
                // SAFETY: live handle; writes serialize internally; null `old_out`.
                unsafe {
                    if (k + round) % 3 == 0 {
                        expanse_sync_map_remove(m_addr as *const _, k, core::ptr::null_mut());
                    } else {
                        expanse_sync_map_insert(
                            m_addr as *const _,
                            k,
                            k * 3,
                            core::ptr::null_mut(),
                        );
                    }
                }
            }
        }
        stop.store(true, Ordering::Relaxed);
    });
    // SAFETY: every thread that held a reader has been joined and freed it.
    unsafe { expanse_sync_map_free(m) };
}

#[test]
fn test_modern_capi_set_navigation() {
    use expanse::modern::{
        expanse_set_first, expanse_set_free, expanse_set_insert, expanse_set_last, expanse_set_new,
        expanse_set_next_after, expanse_set_next_at_or_after, expanse_set_prev_at_or_before,
        expanse_set_prev_before,
    };
    // SAFETY: Exercising C ABI set navigation methods.
    unsafe {
        let set = expanse_set_new();
        assert!(!set.is_null());

        expanse_set_insert(set, 100);
        expanse_set_insert(set, 200);
        expanse_set_insert(set, 300);

        let mut k = 0;

        assert!(expanse_set_first(set, &raw mut k));
        assert_eq!(k, 100);

        assert!(expanse_set_last(set, &raw mut k));
        assert_eq!(k, 300);

        assert!(expanse_set_next_at_or_after(set, 150, &raw mut k));
        assert_eq!(k, 200);

        assert!(expanse_set_next_after(set, 200, &raw mut k));
        assert_eq!(k, 300);

        assert!(!expanse_set_next_after(set, 300, &raw mut k));

        assert!(expanse_set_prev_at_or_before(set, 250, &raw mut k));
        assert_eq!(k, 200);

        assert!(expanse_set_prev_before(set, 200, &raw mut k));
        assert_eq!(k, 100);

        assert!(!expanse_set_prev_before(set, 100, &raw mut k));

        expanse_set_free(set);
    }
}

/// Drives one JudyL-shaped handle through a slot-API fill that leaves the
/// insert-path cache warm on a tree root: blocks 0 and 1 in full, then a
/// bitmap terminal (32 even digits of block 2, then its odd digits) and a
/// linear terminal (digits 0x10..=0xA0 of block 3). A fill into the block
/// the cache holds is served from its level-1 terminal wherever that has
/// spare capacity, and so is a lookup inside the last block written; a key
/// of another block, probed while the cache is warm, must not be. The
/// `expanse-trie` twin is `map::tests::slot_calls_on_a_warm_insert_path`,
/// which asserts the cache state directly. `select` (0-based rank to
/// key) descends by the terminal edges' populations, which the cached
/// inserts maintain.
fn drive_warm_slot_calls(
    ins: impl Fn(u64) -> *mut u64,
    get: impl Fn(u64) -> *mut u64,
    select: impl Fn(u64) -> Option<u64>,
) {
    let val = |k: u64| k.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let fill = |k: u64| {
        // SAFETY: every slot is used before the handle's next mutation.
        let slot = unsafe { ins(k).as_mut() }.unwrap_or_else(|| panic!("null slot for {k:#x}"));
        assert_eq!(*slot, 0, "fresh slot for {k:#x}");
        *slot = val(k);
    };
    // SAFETY: as above.
    let read = |k: u64| unsafe { get(k).as_ref().copied() };
    let mut keys: Vec<u64> = (0..0x200).collect();
    keys.extend((0x200..0x240).step_by(2));
    keys.extend((0x201..0x240).step_by(2));
    for &k in &keys {
        fill(k);
    }
    // Cache on block 2's bitmap terminal.
    assert_eq!(read(0x210), Some(val(0x210)));
    assert_eq!(read(0x2F0), None);
    assert_eq!(
        read(0x110),
        Some(val(0x110)),
        "block 1 served from the block-2 cache"
    );
    // SAFETY: as above. `as_ref` turns a null slot into a failed assertion
    // rather than a dereference.
    let kept = unsafe { ins(0x23F).as_ref() }.copied();
    assert_eq!(kept, Some(val(0x23F)), "ins on a present key keeps it");
    assert_eq!(select(0x23F), Some(0x23F));
    assert_eq!(select(0x240), None);

    let tail: Vec<u64> = (0x310..=0x3A0).step_by(0x10).collect();
    for &k in &tail {
        fill(k);
    }
    keys.extend(&tail);
    // Cache on block 3's linear terminal.
    assert_eq!(read(0x330), Some(val(0x330)));
    assert_eq!(read(0x335), None);
    assert_eq!(
        read(0x230),
        Some(val(0x230)),
        "block 2 served from the block-3 cache"
    );
    // SAFETY: as above.
    let kept = unsafe { ins(0x3A0).as_ref() }.copied();
    assert_eq!(kept, Some(val(0x3A0)), "ins on the last key keeps it");

    keys.sort_unstable();
    for (rank, &k) in keys.iter().enumerate() {
        assert_eq!(read(k), Some(val(k)), "value of {k:#x}");
        assert_eq!(select(rank as u64), Some(k), "select({rank})");
    }
    assert_eq!(select(keys.len() as u64), None);
}

#[test]
fn test_map_slot_calls_on_a_warm_insert_path() {
    use expanse::modern::{expanse_map_by_count, expanse_map_ins_slot, expanse_map_slot};
    use expanse::{JudyLByCount, JudyLGet};

    // Legacy: `*JudyLIns(&a, k) = v`, the classic JudyL fill. The array word
    // lives in a `Cell`: `as_ptr` is the `PPvoid_t` JudyLIns writes through,
    // and `get` reads the current root without dereferencing a raw pointer.
    let jl = core::cell::Cell::new(core::ptr::null_mut::<c_void>());
    let pjl = jl.as_ptr();
    let mut jerr = JError {
        je_errno: 0,
        je_err_id: 0,
        je_reserved: [0; 4],
    };
    let pj = &raw mut jerr;
    // SAFETY: `pjl` is `jl`'s interior pointer, valid while `jl` lives, and
    // no reference into the cell is held across these calls.
    unsafe {
        drive_warm_slot_calls(
            |k| JudyLIns(pjl, k as Word, pj).cast(),
            |k| JudyLGet(jl.get(), k as Word, pj).cast(),
            |n| {
                // JudyLByCount is 1-based.
                let mut k: Word = 0;
                let slot = JudyLByCount(jl.get(), (n + 1) as Word, &raw mut k, pj);
                (!slot.is_null()).then_some(k as u64)
            },
        );
        JudyLFreeArray(pjl, pj);
    }

    // Modern: the same drive through `expanse_map_ins_slot`.
    // SAFETY: `m` is a live handle until the free below.
    unsafe {
        let m = expanse_map_new();
        drive_warm_slot_calls(
            |k| expanse_map_ins_slot(m, k),
            |k| expanse_map_slot(m, k),
            |n| {
                let mut k = 0;
                expanse_map_by_count(m, n, &raw mut k, core::ptr::null_mut()).then_some(k)
            },
        );
        expanse_map_free(m);
    }
}

/// A reader handle may be freed from a thread other than the one that created
/// it, once no call on it is in progress (`include/expanse.h`, reader-handle
/// ownership). Worker threads each create a map and a set reader handle, read
/// through them, and send them back; the main thread frees every handle — while
/// a writer thread churns other keys, so epoch advances run against the
/// registry the frees deregister from — and only then frees the containers.
/// The `Send` bound this relies on is pinned at compile time in `modern_sync.rs`.
#[test]
fn test_sync_reader_handles_freed_on_another_thread() {
    use expanse::modern::{
        expanse_sync_map_free, expanse_sync_map_insert, expanse_sync_map_new,
        expanse_sync_map_reader_free, expanse_sync_map_reader_get, expanse_sync_map_reader_new,
        expanse_sync_map_remove, expanse_sync_set_free, expanse_sync_set_insert,
        expanse_sync_set_new, expanse_sync_set_reader_contains, expanse_sync_set_reader_free,
        expanse_sync_set_reader_new, expanse_sync_set_remove,
    };
    use std::sync::atomic::{AtomicBool, Ordering};

    const THREADS: u64 = 8;
    const N: u64 = 2048;

    let m = expanse_sync_map_new();
    let s = expanse_sync_set_new();
    for k in 0..N {
        // SAFETY: live handles; null `old_out`.
        unsafe {
            assert!(expanse_sync_map_insert(m, k, k * 7, core::ptr::null_mut()));
            assert!(expanse_sync_set_insert(s, k));
        }
    }
    // Raw pointers are not `Send`; the addresses cross threads as integers.
    let (m_addr, s_addr) = (m as usize, s as usize);

    // Phase 1: each worker creates its handles, reads every stable key through
    // them, and returns the handles' addresses to the main thread.
    let handles: Vec<(usize, usize)> = std::thread::scope(|sc| {
        let workers: Vec<_> = (0..THREADS)
            .map(|_| {
                sc.spawn(move || {
                    // SAFETY: both containers outlive every handle (freed last,
                    // below); each handle is used only by this thread until it
                    // is returned, and no call on it is in progress afterwards.
                    unsafe {
                        let r = expanse_sync_map_reader_new(m_addr as *const _);
                        let rs = expanse_sync_set_reader_new(s_addr as *const _);
                        assert!(!r.is_null() && !rs.is_null());
                        for k in 0..N {
                            let mut v = 0u64;
                            assert!(expanse_sync_map_reader_get(r, k, &raw mut v));
                            assert_eq!(v, k * 7);
                            assert!(expanse_sync_set_reader_contains(rs, k));
                        }
                        assert!(!expanse_sync_map_reader_get(r, N, core::ptr::null_mut()));
                        assert!(!expanse_sync_set_reader_contains(rs, N));
                        (r as usize, rs as usize)
                    }
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|w| w.join().expect("reader thread panicked"))
            .collect()
    });
    assert_eq!(handles.len(), THREADS as usize);

    // Phase 2: free every handle on the main thread while a writer churns keys
    // at and above N, retiring nodes and advancing epochs.
    let stop = AtomicBool::new(false);
    std::thread::scope(|sc| {
        let stop = &stop;
        sc.spawn(move || {
            let mut i = 0u64;
            while !stop.load(Ordering::Relaxed) || i < 50_000 {
                let k = N + (i % 4096);
                // SAFETY: live handles; writes serialize internally.
                unsafe {
                    if i.is_multiple_of(2) {
                        expanse_sync_map_insert(m_addr as *const _, k, k, core::ptr::null_mut());
                        expanse_sync_set_insert(s_addr as *const _, k);
                    } else {
                        expanse_sync_map_remove(m_addr as *const _, k, core::ptr::null_mut());
                        expanse_sync_set_remove(s_addr as *const _, k);
                    }
                }
                i += 1;
            }
        });
        for &(r, rs) in &handles {
            // SAFETY: each handle came from `_reader_new`, its creating thread
            // has returned (no call on it is in progress), it is freed exactly
            // once, and its container is still alive.
            unsafe {
                expanse_sync_map_reader_free(r as *mut _);
                expanse_sync_set_reader_free(rs as *mut _);
            }
        }
        stop.store(true, Ordering::Relaxed);
    });

    // A reader registered after the frees still reads the stable keys.
    // SAFETY: live containers; the reader is freed before them.
    unsafe {
        let r = expanse_sync_map_reader_new(m);
        let mut v = 0u64;
        assert!(expanse_sync_map_reader_get(r, N - 1, &raw mut v));
        assert_eq!(v, (N - 1) * 7);
        expanse_sync_map_reader_free(r);
        expanse_sync_set_free(s);
        expanse_sync_map_free(m);
    }
}

/// `expanse_sync_map_mem_used`: 0 for NULL (as `expanse_sync_map_len`), the
/// writer-excluding answer of `SyncExpanseMap::with_locked` at every quiescent
/// point, the same figure as a plain `expanse_map_t` holding the same entries
/// inserted in the same order, and callable while a writer thread runs.
#[test]
fn test_sync_map_mem_used() {
    use expanse::modern::{
        expanse_map_mem_used, expanse_map_remove, expanse_sync_map_free, expanse_sync_map_insert,
        expanse_sync_map_len, expanse_sync_map_mem_used, expanse_sync_map_new,
        expanse_sync_map_remove,
    };
    use expanse_trie::map::ExpanseMap;
    use std::sync::atomic::{AtomicBool, Ordering};

    // SAFETY: a null handle is a documented 0.
    assert_eq!(unsafe { expanse_sync_map_mem_used(core::ptr::null()) }, 0);
    // SAFETY: a null handle is a documented 0.
    assert_eq!(unsafe { expanse_sync_map_len(core::ptr::null()) }, 0);

    let m = expanse_sync_map_new();
    let plain = expanse_map_new();
    // SAFETY: `m` and `plain` are live until freed at the end; `&*m` is a
    // shared borrow of a handle whose methods take `&self`; null `old_out`s.
    unsafe {
        let locked = || (*m).with_locked(ExpanseMap::mem_used);
        let empty = expanse_sync_map_mem_used(m);
        assert_eq!(empty, locked());
        assert_eq!(empty, expanse_map_mem_used(plain));

        for k in (0..60_000u64).step_by(3) {
            expanse_sync_map_insert(m, k, k, core::ptr::null_mut());
            expanse_map_insert(plain, k, k, core::ptr::null_mut());
        }
        let full = expanse_sync_map_mem_used(m);
        assert!(full > empty, "{full} <= {empty}");
        assert_eq!(full, locked());
        assert_eq!(full, expanse_map_mem_used(plain));

        for k in (0..60_000u64).step_by(6) {
            assert!(expanse_sync_map_remove(m, k, core::ptr::null_mut()));
            assert!(expanse_map_remove(plain, k, core::ptr::null_mut()));
        }
        let half = expanse_sync_map_mem_used(m);
        assert_eq!(half, locked());
        assert_eq!(half, expanse_map_mem_used(plain));
    }

    // Against a live writer: every call returns (the writer is quiesced, not
    // deadlocked) and the final quiescent figure is the locked one.
    let m_addr = m as usize;
    let stop = AtomicBool::new(false);
    std::thread::scope(|sc| {
        let stop = &stop;
        sc.spawn(move || {
            let mut i = 0u64;
            while !stop.load(Ordering::Relaxed) {
                let k = 1_000_000 + (i % 50_000);
                // SAFETY: live handle; writes serialize internally.
                unsafe { expanse_sync_map_insert(m_addr as *const _, k, i, core::ptr::null_mut()) };
                i += 1;
            }
        });
        for _ in 0..2_000 {
            // SAFETY: live handle for the whole scope.
            let used = unsafe { expanse_sync_map_mem_used(m_addr as *const _) };
            assert!(used > 0);
        }
        stop.store(true, Ordering::Relaxed);
    });
    // SAFETY: the writer has been joined; both handles are freed exactly once.
    unsafe {
        assert_eq!(
            expanse_sync_map_mem_used(m),
            (*m).with_locked(ExpanseMap::mem_used)
        );
        expanse_sync_map_free(m);
        expanse::modern::expanse_map_free(plain);
    }
}

/// `expanse_{map,set,strmap}_mem_held` and `_shrink_to_fit`: 0 for NULL; a
/// map or set drained by `remove` keeps its freed blocks, so `mem_held` stays
/// above `mem_used`; `shrink_to_fit` returns exactly the difference, after
/// which `mem_held == mem_used` (0 for an empty tree) and a second call
/// releases nothing.
#[test]
fn test_mem_held_and_shrink_to_fit() {
    use expanse::modern::{
        expanse_map_mem_held, expanse_map_mem_used, expanse_map_remove, expanse_map_shrink_to_fit,
        expanse_set_mem_held, expanse_set_mem_used, expanse_set_remove, expanse_set_shrink_to_fit,
        expanse_strmap_free, expanse_strmap_insert, expanse_strmap_len, expanse_strmap_mem_held,
        expanse_strmap_mem_used, expanse_strmap_new, expanse_strmap_remove,
        expanse_strmap_shrink_to_fit,
    };
    use std::ffi::CString;

    // SAFETY: null handles are a documented 0 for every entry point.
    unsafe {
        assert_eq!(expanse_map_mem_held(core::ptr::null()), 0);
        assert_eq!(expanse_map_shrink_to_fit(core::ptr::null_mut()), 0);
        assert_eq!(expanse_set_mem_held(core::ptr::null()), 0);
        assert_eq!(expanse_set_shrink_to_fit(core::ptr::null_mut()), 0);
        assert_eq!(expanse_strmap_mem_held(core::ptr::null()), 0);
        assert_eq!(expanse_strmap_shrink_to_fit(core::ptr::null_mut()), 0);
    }

    const N: u64 = 20_000;
    let m = expanse_map_new();
    // SAFETY: `m` is live until freed at the end; null `old_out`s.
    unsafe {
        for k in 0..N {
            expanse_map_insert(m, k * 7, k, core::ptr::null_mut());
        }
        assert!(expanse_map_mem_held(m) >= expanse_map_mem_used(m));
        for k in 0..N {
            assert!(expanse_map_remove(m, k * 7, core::ptr::null_mut()));
        }
        assert_eq!(expanse_map_mem_used(m), 0);
        let held = expanse_map_mem_held(m);
        assert!(held > 0, "a drained map keeps its freed blocks");
        assert_eq!(expanse_map_shrink_to_fit(m), held);
        assert_eq!(expanse_map_mem_held(m), expanse_map_mem_used(m));
        assert_eq!(expanse_map_mem_held(m), 0);
        assert_eq!(expanse_map_shrink_to_fit(m), 0);
        expanse_map_free(m);
    }

    let s = expanse_set_new();
    // SAFETY: `s` is live until freed at the end.
    unsafe {
        for k in 0..N {
            expanse_set_insert(s, k * 7);
        }
        assert!(expanse_set_mem_held(s) >= expanse_set_mem_used(s));
        for k in 0..N {
            assert!(expanse_set_remove(s, k * 7));
        }
        assert_eq!(expanse_set_mem_used(s), 0);
        let held = expanse_set_mem_held(s);
        assert!(held > 0, "a drained set keeps its freed blocks");
        assert_eq!(expanse_set_shrink_to_fit(s), held);
        assert_eq!(expanse_set_mem_held(s), expanse_set_mem_used(s));
        assert_eq!(expanse_set_mem_held(s), 0);
        assert_eq!(expanse_set_shrink_to_fit(s), 0);
        expanse_set_free(s);
    }

    let keys: Vec<CString> = (0..N)
        .map(|k| CString::new(format!("key/{k:08}")).unwrap())
        .collect();
    let sm = expanse_strmap_new();
    // SAFETY: `sm` is live until freed at the end; every key is a valid
    // NUL-terminated C string; null `old_out`s.
    unsafe {
        for (v, k) in keys.iter().enumerate() {
            expanse_strmap_insert(sm, k.as_ptr(), v as u64, core::ptr::null_mut());
        }
        assert!(expanse_strmap_mem_held(sm) >= expanse_strmap_mem_used(sm));
        for k in &keys {
            assert!(expanse_strmap_remove(sm, k.as_ptr(), core::ptr::null_mut()));
        }
        assert_eq!(expanse_strmap_len(sm), 0);
        let (held, used) = (expanse_strmap_mem_held(sm), expanse_strmap_mem_used(sm));
        assert_eq!(expanse_strmap_shrink_to_fit(sm), held - used);
        assert_eq!(expanse_strmap_mem_held(sm), expanse_strmap_mem_used(sm));
        assert_eq!(expanse_strmap_mem_used(sm), used);
        assert_eq!(expanse_strmap_shrink_to_fit(sm), 0);
        expanse_strmap_free(sm);
    }
}
