//! `Judy1Count` / `JudyLCount` and the `PJError` out-parameter
//! (`docs/COMPAT.md` doc-gap D7).
//!
//! `Judy1_funcs(3)` tells a caller to read `JU_ERRNO(&JError)` when the
//! count is 0, and the classic `J1C` / `JLC` macros do exactly that: a zero
//! count with anything but `JU_ERRNO_NONE` in the error record is taken as
//! an error and handed to `JUDYERROR`, which exits by default. So every zero
//! count must write `JU_ERRNO_NONE`. The stock-oracle harness settled the
//! cases the man pages leave open: a reversed range is a zero count like any
//! other, a nonzero count leaves the record untouched, and a null `PJError`
//! returns the same count.
#![cfg(target_pointer_width = "64")]

use core::ffi::{c_int, c_void};
use core::ptr::null_mut;
use expanse::{
    JError, JU_ERRNO_NONE, Judy1Count, Judy1FreeArray, Judy1Set, JudyLCount, JudyLFreeArray,
    JudyLIns, Word,
};

const SENTINEL: c_int = 99;

type Count = unsafe extern "C" fn(*const c_void, Word, Word, *mut JError) -> Word;

/// Calls `count` with an error record pre-filled with a sentinel, then with
/// a null `PJError`, and returns the count and the errno it left.
fn probe(count: Count, array: *const c_void, a: Word, b: Word) -> (Word, c_int) {
    let mut je = JError {
        je_errno: SENTINEL,
        je_err_id: 0,
        je_reserved: [0; 4],
    };
    // SAFETY: `array` is null or a live array of `count`'s family; `je` is a
    // live, writable JError.
    let rc = unsafe { count(array, a, b, &raw mut je) };
    // SAFETY: a null PJError is part of the documented contract.
    let rc_null = unsafe { count(array, a, b, null_mut()) };
    assert_eq!(rc, rc_null, "count {a:#x}..={b:#x} with a null PJError");
    (rc, je.je_errno)
}

/// Every zero count, whatever its cause, writes `JU_ERRNO_NONE`; a nonzero
/// count leaves the record as it was.
fn check(family: &str, count: Count, empty: *const c_void, populated: *const c_void) {
    let zero = (0, JU_ERRNO_NONE);
    for (a, b) in [(0, Word::MAX), (5, 5), (Word::MAX, 0)] {
        assert_eq!(
            probe(count, empty, a, b),
            zero,
            "{family} empty array, {a:#x}..={b:#x}"
        );
    }
    let cases: [(Word, Word, (Word, c_int)); 7] = [
        (0, Word::MAX, (3, SENTINEL)),
        (10, 30, (3, SENTINEL)),
        (20, 20, (1, SENTINEL)),
        (11, 19, zero),
        (25, 25, zero),
        (31, Word::MAX, zero),
        (30, 10, zero),
    ];
    for (a, b, want) in cases {
        assert_eq!(
            probe(count, populated, a, b),
            want,
            "{family} {{10, 20, 30}}, {a:#x}..={b:#x}"
        );
    }
}

#[test]
fn judy1_count_writes_errno_none_on_every_zero_count() {
    let mut set: *mut c_void = null_mut();
    // SAFETY: `set` is a Judy1 array word owned by this test, freed below.
    unsafe {
        for k in [10, 20, 30] {
            assert_eq!(Judy1Set(&raw mut set, k, null_mut()), 1);
        }
        check("Judy1", Judy1Count, null_mut(), set);
        assert!(Judy1FreeArray(&raw mut set, null_mut()) > 0);
    }
}

#[test]
fn judyl_count_writes_errno_none_on_every_zero_count() {
    let mut map: *mut c_void = null_mut();
    // SAFETY: `map` is a JudyL array word owned by this test, freed below;
    // each slot is written right after `JudyLIns` returns it.
    unsafe {
        for k in [10, 20, 30] {
            *JudyLIns(&raw mut map, k, null_mut()).cast::<Word>() = k;
        }
        check("JudyL", JudyLCount, null_mut(), map);
        assert!(JudyLFreeArray(&raw mut map, null_mut()) > 0);
    }
}
