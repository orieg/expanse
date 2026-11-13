//! `*PIndex` after a miss in `Judy1`/`JudyL` `First`/`Next`/`Last`/`Prev`
//! (`docs/COMPAT.md` doc-gap D8).
//!
//! The man pages define `*PIndex` only when the search finds an index. The
//! stock-oracle harness (`nav_miss_index_matches_stock_libjudy`) settled the
//! miss: on a non-empty array stock libjudy writes one step past the probe in
//! the search direction, wrapping at either end of the key space, and on a
//! null array it writes nothing. A consumer that reads `Index` after a miss
//! (a loop that resumes from it, a differential trace) sees the same value on
//! libexpanse. The `*Empty` functions leave `*PIndex` untouched on a miss, as
//! stock does.
#![cfg(target_pointer_width = "64")]

use core::ffi::{c_int, c_void};
use core::ptr::null_mut;
use expanse::{
    JError, Judy1First, Judy1FirstEmpty, Judy1FreeArray, Judy1Last, Judy1LastEmpty, Judy1Next,
    Judy1Prev, Judy1Set, Judy1Unset, JudyLDel, JudyLFirst, JudyLFreeArray, JudyLIns, JudyLLast,
    JudyLNext, JudyLPrev, Word,
};

type Nav1 = unsafe extern "C" fn(*const c_void, *mut Word, *mut JError) -> c_int;
type NavL = unsafe extern "C" fn(*const c_void, *mut Word, *mut JError) -> *mut c_void;

/// Forward search: First, Next. Backward: Last, Prev.
const NAV1: [(&str, Nav1, bool); 4] = [
    ("Judy1First", Judy1First, true),
    ("Judy1Next", Judy1Next, true),
    ("Judy1Last", Judy1Last, false),
    ("Judy1Prev", Judy1Prev, false),
];
const NAVL: [(&str, NavL, bool); 4] = [
    ("JudyLFirst", JudyLFirst, true),
    ("JudyLNext", JudyLNext, true),
    ("JudyLLast", JudyLLast, false),
    ("JudyLPrev", JudyLPrev, false),
];

/// `(found, index)` from each of the eight functions at `probe`.
fn probe(set: *const c_void, map: *const c_void, probe: Word) -> Vec<(&'static str, bool, Word)> {
    let mut out = Vec::new();
    for (name, f, _) in NAV1 {
        let mut i = probe;
        // SAFETY: `set` is null or a live Judy1 array; `i` is writable.
        let r = unsafe { f(set, &raw mut i, null_mut()) };
        out.push((name, r == 1, i));
    }
    for (name, f, _) in NAVL {
        let mut i = probe;
        // SAFETY: `map` is null or a live JudyL array; `i` is writable.
        let r = unsafe { f(map, &raw mut i, null_mut()) };
        out.push((name, !r.is_null(), i));
    }
    out
}

fn forward(name: &str) -> bool {
    NAV1.iter()
        .map(|&(n, _, fwd)| (n, fwd))
        .chain(NAVL.iter().map(|&(n, _, fwd)| (n, fwd)))
        .find(|&(n, _)| n == name)
        .expect("known function")
        .1
}

#[test]
fn a_miss_on_a_non_empty_array_steps_past_the_probe_wrapping() {
    let (mut set, mut map) = (null_mut::<c_void>(), null_mut::<c_void>());
    // SAFETY: both arrays are owned by this test and freed below; each slot is
    // written right after `JudyLIns` returns it.
    unsafe {
        for k in [100, 200, 300] {
            assert_eq!(Judy1Set(&raw mut set, k, null_mut()), 1);
            *JudyLIns(&raw mut map, k, null_mut()).cast::<Word>() = k;
        }
    }
    let mut misses = 0;
    for p in [0, 1, 99, 100, 300, 301, 1 << 63, Word::MAX - 1, Word::MAX] {
        for (name, found, index) in probe(set, map, p) {
            if found {
                continue;
            }
            misses += 1;
            let want = if forward(name) {
                p.wrapping_add(1)
            } else {
                p.wrapping_sub(1)
            };
            assert_eq!(index, want, "{name} miss at {p:#x}");
        }
    }
    // Per family: First misses above 300 (4 probes), Next at or above 300
    // (5), Last below 100 (3), Prev at or below 100 (4).
    assert_eq!(
        misses,
        2 * (4 + 5 + 3 + 4),
        "the probes must reach the miss path"
    );
    // The wrap at both ends, spelled out.
    let mut i = Word::MAX;
    // SAFETY: `set` is a live Judy1 array.
    assert_eq!(unsafe { Judy1Next(set, &raw mut i, null_mut()) }, 0);
    assert_eq!(i, 0, "Judy1Next(MAX) wraps to 0");
    let mut i = 0;
    // SAFETY: `map` is a live JudyL array.
    assert!(unsafe { JudyLLast(map, &raw mut i, null_mut()) }.is_null());
    assert_eq!(i, Word::MAX, "JudyLLast(0) wraps to MAX");
    // SAFETY: both arrays are live and owned here.
    unsafe {
        assert!(Judy1FreeArray(&raw mut set, null_mut()) > 0);
        assert!(JudyLFreeArray(&raw mut map, null_mut()) > 0);
    }
}

#[test]
fn a_miss_on_a_null_array_leaves_the_index_untouched() {
    let (mut set, mut map) = (null_mut::<c_void>(), null_mut::<c_void>());
    for p in [0, 7, Word::MAX] {
        for (name, found, index) in probe(set, map, p) {
            assert!(!found, "{name} on a null array");
            assert_eq!(index, p, "{name} on a null array at {p:#x}");
        }
    }
    // An array emptied back to null by removal is a null array.
    // SAFETY: both arrays are owned by this test.
    unsafe {
        assert_eq!(Judy1Set(&raw mut set, 42, null_mut()), 1);
        *JudyLIns(&raw mut map, 42, null_mut()).cast::<Word>() = 1;
        assert_eq!(Judy1Unset(&raw mut set, 42, null_mut()), 1);
        assert_eq!(JudyLDel(&raw mut map, 42, null_mut()), 1);
    }
    assert!(set.is_null() && map.is_null(), "emptied arrays are null");
    for (name, found, index) in probe(set, map, 42) {
        assert!(!found && index == 42, "{name} after emptying");
    }
}

#[test]
fn a_miss_in_the_empty_functions_leaves_the_index_untouched() {
    let mut set = null_mut::<c_void>();
    // SAFETY: `set` is owned by this test and freed below.
    unsafe {
        for k in [0, Word::MAX] {
            assert_eq!(Judy1Set(&raw mut set, k, null_mut()), 1);
        }
        let mut i = Word::MAX;
        assert_eq!(Judy1FirstEmpty(set, &raw mut i, null_mut()), 0);
        assert_eq!(i, Word::MAX, "Judy1FirstEmpty miss");
        let mut i = 0;
        assert_eq!(Judy1LastEmpty(set, &raw mut i, null_mut()), 0);
        assert_eq!(i, 0, "Judy1LastEmpty miss");
        assert!(Judy1FreeArray(&raw mut set, null_mut()) > 0);
    }
}
