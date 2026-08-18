//! # expanse-capi — `libexpanse`, the drop-in C ABI for libjudy
//!
//! Exports the classic `Judy.h` surface (`Judy1*` backed by `ExpanseSet`,
//! `JudyL*` backed by `ExpanseMap`) with matching symbol names, calling
//! convention, and documented semantics, so existing consumers of the C
//! library link-swap or `LD_PRELOAD` `libexpanse` in place of `libJudy`
//! without source changes. The contract — target surface, guarantees,
//! non-goals, doc-gap resolutions, acceptance gates — is `docs/COMPAT.md`;
//! the shipped header is `include/Judy.h`.
//!
//! Array-handle convention: a `Pvoid_t` array word is null when the array
//! is empty and otherwise holds a pointer to the boxed tree. Removing the
//! last key returns the word to null, matching the classic representation
//! of an empty array.
//!
//! v1 note (correctness over cycles, per `docs/BENCHMARKING.md`): the
//! `JudyL` slot-returning entry points run the tree walk up to twice
//! (mutate/locate + slot fetch); a fused single-walk variant is a Phase 8
//! bench-driven follow-up. `JudySL`/`JudyHS` land with `ExpanseStrMap`/
//! `ExpanseBytesMap`.

// The entry points below are C ABI exports whose safety contract is the
// documented Judy C API itself (docs/COMPAT.md); per-call obligations are
// stated in SAFETY comments inside each body rather than per-fn sections.
#![allow(clippy::missing_safety_doc)]

use core::ffi::{c_int, c_void};
use core::ptr::{NonNull, null_mut};
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;

/// `Word_t`: pointer-width unsigned integer (see COMPAT.md doc-gap D1).
pub type Word = usize;

/// The classic `JError_t` error record (see COMPAT.md doc-gap D2).
#[repr(C)]
pub struct JError {
    /// A `JU_ERRNO_*` code.
    pub je_errno: c_int,
    /// Internal error location id (always 0 here).
    pub je_err_id: c_int,
    /// Reserved words, kept for layout compatibility.
    pub je_reserved: [Word; 4],
}

/// No error.
pub const JU_ERRNO_NONE: c_int = 0;
/// Out of memory (see COMPAT.md doc-gap D3: allocation failure aborts).
pub const JU_ERRNO_NOMEM: c_int = 2;
/// A null `PPArray` was passed where one is required.
pub const JU_ERRNO_NULLPPARRAY: c_int = 3;
/// A null `PIndex` was passed where one is required.
pub const JU_ERRNO_NULLPINDEX: c_int = 5;

const JERR_INT: c_int = -1;
/// The `PJERR` sentinel returned by pointer-returning entry points.
const PJERR: *mut c_void = usize::MAX as *mut c_void;

#[inline]
unsafe fn set_err(pj: *mut JError, code: c_int) {
    if !pj.is_null() {
        // SAFETY: caller passed a valid JError pointer (or null, excluded).
        unsafe {
            (*pj).je_errno = code;
            (*pj).je_err_id = 0;
        }
    }
}

/// Finds the smallest absent key `>= from`, using rank queries — shared by
/// the four `*Empty` search families. `count_range` is inclusive.
fn first_absent(
    contains: impl Fn(u64) -> bool,
    count_range: impl Fn(u64, u64) -> u64,
    from: u64,
) -> Option<u64> {
    if !contains(from) {
        return Some(from);
    }
    let tail_len = u128::from(u64::MAX - from) + 1;
    if u128::from(count_range(from, u64::MAX)) == tail_len {
        return None;
    }
    // Smallest m > from whose prefix [from..=m] holds an absent key; the
    // absent key is m itself (the prefix below m is fully present).
    let (mut lo, mut hi) = (from, u64::MAX);
    while lo + 1 < hi {
        let mid = lo + (hi - lo) / 2;
        if u128::from(count_range(from, mid)) < u128::from(mid - from) + 1 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some(hi)
}

/// Finds the largest absent key `<= upto` (mirror of [`first_absent`]).
fn last_absent(
    contains: impl Fn(u64) -> bool,
    count_range: impl Fn(u64, u64) -> u64,
    upto: u64,
) -> Option<u64> {
    if !contains(upto) {
        return Some(upto);
    }
    if u128::from(count_range(0, upto)) == u128::from(upto) + 1 {
        return None;
    }
    let (mut lo, mut hi) = (0u64, upto);
    while lo + 1 < hi {
        let mid = lo + (hi - lo) / 2;
        if u128::from(count_range(mid, upto)) < u128::from(upto - mid) + 1 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(lo)
}

// ---------------------------------------------------------------------
// Judy1 (bit set) — backed by ExpanseSet
// ---------------------------------------------------------------------

/// # Safety
/// `pparray` must be a valid pointer to the caller's array word; the word
/// must be null or a handle previously produced by these entry points.
unsafe fn set_handle_mut<'a>(pparray: *mut *mut c_void) -> &'a mut ExpanseSet {
    // SAFETY: per this function's contract; creates the tree on demand.
    unsafe {
        if (*pparray).is_null() {
            *pparray = Box::into_raw(Box::new(ExpanseSet::new())).cast();
        }
        &mut *(*pparray).cast::<ExpanseSet>()
    }
}

unsafe fn set_handle<'a>(parray: *const c_void) -> Option<&'a ExpanseSet> {
    // SAFETY: null or a handle produced by these entry points.
    unsafe { parray.cast::<ExpanseSet>().as_ref() }
}

/// Sets `index`; returns 1 if newly set, 0 if it was already set.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Judy1Set(
    pparray: *mut *mut c_void,
    index: Word,
    pj: *mut JError,
) -> c_int {
    // SAFETY: forwarded C contract.
    unsafe {
        if pparray.is_null() {
            set_err(pj, JU_ERRNO_NULLPPARRAY);
            return JERR_INT;
        }
        c_int::from(set_handle_mut(pparray).insert(index as u64))
    }
}

/// Unsets `index`; returns 1 if it was present. An emptied array word
/// returns to null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Judy1Unset(
    pparray: *mut *mut c_void,
    index: Word,
    pj: *mut JError,
) -> c_int {
    // SAFETY: forwarded C contract.
    unsafe {
        if pparray.is_null() {
            set_err(pj, JU_ERRNO_NULLPPARRAY);
            return JERR_INT;
        }
        if (*pparray).is_null() {
            return 0;
        }
        let set = &mut *(*pparray).cast::<ExpanseSet>();
        let removed = set.remove(index as u64);
        if removed && set.is_empty() {
            drop(Box::from_raw((*pparray).cast::<ExpanseSet>()));
            *pparray = null_mut();
        }
        c_int::from(removed)
    }
}

/// Membership test: 1 if `index` is set.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Judy1Test(parray: *const c_void, index: Word, _pj: *mut JError) -> c_int {
    // SAFETY: forwarded C contract.
    unsafe { c_int::from(set_handle(parray).is_some_and(|s| s.contains(index as u64))) }
}

/// Number of set indexes in `index1..=index2`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Judy1Count(
    parray: *const c_void,
    index1: Word,
    index2: Word,
    _pj: *mut JError,
) -> Word {
    // SAFETY: forwarded C contract.
    unsafe {
        set_handle(parray).map_or(0, |s| s.count_range(index1 as u64..=index2 as u64)) as Word
    }
}

/// Locates the `nth` (1-based) set index; 1 on success with `*pindex`
/// updated, 0 if the population is smaller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Judy1ByCount(
    parray: *const c_void,
    nth: Word,
    pindex: *mut Word,
    pj: *mut JError,
) -> c_int {
    // SAFETY: forwarded C contract.
    unsafe {
        if pindex.is_null() {
            set_err(pj, JU_ERRNO_NULLPINDEX);
            return JERR_INT;
        }
        let Some(set) = set_handle(parray) else {
            return 0;
        };
        if nth == 0 {
            return 0;
        }
        match set.by_count(nth as u64 - 1) {
            Some(k) => {
                *pindex = k as Word;
                1
            }
            None => 0,
        }
    }
}

macro_rules! judy1_nav {
    ($name:ident, $method:ident, $doc:literal) => {
        #[doc = $doc]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            parray: *const c_void,
            pindex: *mut Word,
            pj: *mut JError,
        ) -> c_int {
            // SAFETY: forwarded C contract.
            unsafe {
                if pindex.is_null() {
                    set_err(pj, JU_ERRNO_NULLPINDEX);
                    return JERR_INT;
                }
                let Some(set) = set_handle(parray) else {
                    return 0;
                };
                match set.$method(*pindex as u64) {
                    Some(k) => {
                        *pindex = k as Word;
                        1
                    }
                    None => 0,
                }
            }
        }
    };
}

judy1_nav!(
    Judy1First,
    next_at_or_after,
    "Smallest set index >= *pindex."
);
judy1_nav!(Judy1Next, next_after, "Smallest set index > *pindex.");
judy1_nav!(
    Judy1Last,
    prev_at_or_before,
    "Largest set index <= *pindex."
);
judy1_nav!(Judy1Prev, prev_before, "Largest set index < *pindex.");

macro_rules! judy1_empty {
    ($name:ident, $absent:ident, $adjust:expr, $doc:literal) => {
        #[doc = $doc]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            parray: *const c_void,
            pindex: *mut Word,
            pj: *mut JError,
        ) -> c_int {
            // SAFETY: forwarded C contract.
            unsafe {
                if pindex.is_null() {
                    set_err(pj, JU_ERRNO_NULLPINDEX);
                    return JERR_INT;
                }
                #[allow(clippy::redundant_closure_call)]
                let Some(start) = ($adjust)(*pindex as u64) else {
                    return 0;
                };
                let found = match set_handle(parray) {
                    None => Some(start),
                    Some(set) => $absent(|k| set.contains(k), |a, b| set.count_range(a..=b), start),
                };
                match found {
                    Some(k) => {
                        *pindex = k as Word;
                        1
                    }
                    None => 0,
                }
            }
        }
    };
}

judy1_empty!(
    Judy1FirstEmpty,
    first_absent,
    |k: u64| Some(k),
    "Smallest unset index >= *pindex."
);
judy1_empty!(
    Judy1NextEmpty,
    first_absent,
    |k: u64| k.checked_add(1),
    "Smallest unset index > *pindex."
);
judy1_empty!(
    Judy1LastEmpty,
    last_absent,
    |k: u64| Some(k),
    "Largest unset index <= *pindex."
);
judy1_empty!(
    Judy1PrevEmpty,
    last_absent,
    |k: u64| k.checked_sub(1),
    "Largest unset index < *pindex."
);

/// Frees the whole array; returns the bytes freed and nulls the word.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Judy1FreeArray(pparray: *mut *mut c_void, pj: *mut JError) -> Word {
    // SAFETY: forwarded C contract.
    unsafe {
        if pparray.is_null() {
            set_err(pj, JU_ERRNO_NULLPPARRAY);
            return 0;
        }
        if (*pparray).is_null() {
            return 0;
        }
        let boxed = Box::from_raw((*pparray).cast::<ExpanseSet>());
        let bytes = boxed.mem_used() + size_of::<ExpanseSet>();
        drop(boxed);
        *pparray = null_mut();
        bytes as Word
    }
}

/// Heap bytes used by the array (order-of-magnitude contract; see
/// COMPAT.md non-goals).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Judy1MemUsed(parray: *const c_void) -> Word {
    // SAFETY: forwarded C contract.
    unsafe { set_handle(parray).map_or(0, |s| s.mem_used() + size_of::<ExpanseSet>()) as Word }
}

// ---------------------------------------------------------------------
// JudyL (word → word map) — backed by ExpanseMap
// ---------------------------------------------------------------------

unsafe fn map_handle_mut<'a>(pparray: *mut *mut c_void) -> &'a mut ExpanseMap {
    // SAFETY: per set_handle_mut's contract, for maps.
    unsafe {
        if (*pparray).is_null() {
            *pparray = Box::into_raw(Box::new(ExpanseMap::new())).cast();
        }
        &mut *(*pparray).cast::<ExpanseMap>()
    }
}

/// JudyL handles are always treated as mutable internally: the C contract
/// returns writable value slots even from `Pcvoid_t` entry points.
unsafe fn map_handle<'a>(parray: *const c_void) -> Option<&'a mut ExpanseMap> {
    // SAFETY: null or a handle produced by these entry points.
    unsafe { parray.cast_mut().cast::<ExpanseMap>().as_mut() }
}

fn slot_ptr(slot: Option<NonNull<u64>>) -> *mut c_void {
    slot.map_or(null_mut(), |p| p.as_ptr().cast())
}

/// Inserts `index` (value slot zero-initialized if new) and returns a
/// writable pointer to its value slot; `PJERR` on error. The slot stays
/// valid until the next structural mutation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn JudyLIns(
    pparray: *mut *mut c_void,
    index: Word,
    pj: *mut JError,
) -> *mut c_void {
    // SAFETY: forwarded C contract.
    unsafe {
        if pparray.is_null() {
            set_err(pj, JU_ERRNO_NULLPPARRAY);
            return PJERR;
        }
        let map = map_handle_mut(pparray);
        let key = index as u64;
        if !map.contains_key(key) {
            map.insert(key, 0);
        }
        slot_ptr(map.get_value_slot(key))
    }
}

/// Deletes `index`; returns 1 if it was present. An emptied array word
/// returns to null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn JudyLDel(
    pparray: *mut *mut c_void,
    index: Word,
    pj: *mut JError,
) -> c_int {
    // SAFETY: forwarded C contract.
    unsafe {
        if pparray.is_null() {
            set_err(pj, JU_ERRNO_NULLPPARRAY);
            return JERR_INT;
        }
        if (*pparray).is_null() {
            return 0;
        }
        let map = &mut *(*pparray).cast::<ExpanseMap>();
        let removed = map.remove(index as u64).is_some();
        if removed && map.is_empty() {
            drop(Box::from_raw((*pparray).cast::<ExpanseMap>()));
            *pparray = null_mut();
        }
        c_int::from(removed)
    }
}

/// Returns a writable pointer to `index`'s value slot, or null if absent.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn JudyLGet(
    parray: *const c_void,
    index: Word,
    _pj: *mut JError,
) -> *mut c_void {
    // SAFETY: forwarded C contract.
    unsafe { map_handle(parray).map_or(null_mut(), |m| slot_ptr(m.get_value_slot(index as u64))) }
}

/// Number of keys in `index1..=index2`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn JudyLCount(
    parray: *const c_void,
    index1: Word,
    index2: Word,
    _pj: *mut JError,
) -> Word {
    // SAFETY: forwarded C contract.
    unsafe {
        map_handle(parray).map_or(0, |m| m.count_range(index1 as u64..=index2 as u64)) as Word
    }
}

/// Locates the `nth` (1-based) key; returns its value slot and updates
/// `*pindex`, or null if the population is smaller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn JudyLByCount(
    parray: *const c_void,
    nth: Word,
    pindex: *mut Word,
    pj: *mut JError,
) -> *mut c_void {
    // SAFETY: forwarded C contract.
    unsafe {
        if pindex.is_null() {
            set_err(pj, JU_ERRNO_NULLPINDEX);
            return PJERR;
        }
        let Some(map) = map_handle(parray) else {
            return null_mut();
        };
        if nth == 0 {
            return null_mut();
        }
        match map.by_count(nth as u64 - 1) {
            Some((k, _)) => {
                *pindex = k as Word;
                slot_ptr(map.get_value_slot(k))
            }
            None => null_mut(),
        }
    }
}

macro_rules! judyl_nav {
    ($name:ident, $method:ident, $doc:literal) => {
        #[doc = $doc]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            parray: *const c_void,
            pindex: *mut Word,
            pj: *mut JError,
        ) -> *mut c_void {
            // SAFETY: forwarded C contract.
            unsafe {
                if pindex.is_null() {
                    set_err(pj, JU_ERRNO_NULLPINDEX);
                    return PJERR;
                }
                let Some(map) = map_handle(parray) else {
                    return null_mut();
                };
                match map.$method(*pindex as u64) {
                    Some((k, _)) => {
                        *pindex = k as Word;
                        slot_ptr(map.get_value_slot(k))
                    }
                    None => null_mut(),
                }
            }
        }
    };
}

judyl_nav!(
    JudyLFirst,
    next_at_or_after,
    "Smallest key >= *pindex; returns its value slot."
);
judyl_nav!(
    JudyLNext,
    next_after,
    "Smallest key > *pindex; returns its value slot."
);
judyl_nav!(
    JudyLLast,
    prev_at_or_before,
    "Largest key <= *pindex; returns its value slot."
);
judyl_nav!(
    JudyLPrev,
    prev_before,
    "Largest key < *pindex; returns its value slot."
);

macro_rules! judyl_empty {
    ($name:ident, $absent:ident, $adjust:expr, $doc:literal) => {
        #[doc = $doc]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            parray: *const c_void,
            pindex: *mut Word,
            pj: *mut JError,
        ) -> c_int {
            // SAFETY: forwarded C contract.
            unsafe {
                if pindex.is_null() {
                    set_err(pj, JU_ERRNO_NULLPINDEX);
                    return JERR_INT;
                }
                #[allow(clippy::redundant_closure_call)]
                let Some(start) = ($adjust)(*pindex as u64) else {
                    return 0;
                };
                let found = match map_handle(parray) {
                    None => Some(start),
                    Some(map) => $absent(
                        |k| map.contains_key(k),
                        |a, b| map.count_range(a..=b),
                        start,
                    ),
                };
                match found {
                    Some(k) => {
                        *pindex = k as Word;
                        1
                    }
                    None => 0,
                }
            }
        }
    };
}

judyl_empty!(
    JudyLFirstEmpty,
    first_absent,
    |k: u64| Some(k),
    "Smallest absent key >= *pindex."
);
judyl_empty!(
    JudyLNextEmpty,
    first_absent,
    |k: u64| k.checked_add(1),
    "Smallest absent key > *pindex."
);
judyl_empty!(
    JudyLLastEmpty,
    last_absent,
    |k: u64| Some(k),
    "Largest absent key <= *pindex."
);
judyl_empty!(
    JudyLPrevEmpty,
    last_absent,
    |k: u64| k.checked_sub(1),
    "Largest absent key < *pindex."
);

/// Frees the whole array; returns the bytes freed and nulls the word.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn JudyLFreeArray(pparray: *mut *mut c_void, pj: *mut JError) -> Word {
    // SAFETY: forwarded C contract.
    unsafe {
        if pparray.is_null() {
            set_err(pj, JU_ERRNO_NULLPPARRAY);
            return 0;
        }
        if (*pparray).is_null() {
            return 0;
        }
        let boxed = Box::from_raw((*pparray).cast::<ExpanseMap>());
        let bytes = boxed.mem_used() + size_of::<ExpanseMap>();
        drop(boxed);
        *pparray = null_mut();
        bytes as Word
    }
}

/// Heap bytes used by the array (order-of-magnitude contract).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn JudyLMemUsed(parray: *const c_void) -> Word {
    // SAFETY: forwarded C contract.
    unsafe { map_handle(parray).map_or(0, |m| m.mem_used() + size_of::<ExpanseMap>()) as Word }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arr() -> *mut c_void {
        null_mut()
    }

    #[test]
    fn judy1_smoke() {
        // SAFETY: valid pointers throughout; handles managed per contract.
        unsafe {
            let mut a = arr();
            assert_eq!(Judy1Set(&raw mut a, 42, null_mut()), 1);
            assert_eq!(Judy1Set(&raw mut a, 42, null_mut()), 0);
            assert_eq!(Judy1Set(&raw mut a, 7, null_mut()), 1);
            assert_eq!(Judy1Set(&raw mut a, usize::MAX, null_mut()), 1);
            assert_eq!(Judy1Test(a, 42, null_mut()), 1);
            assert_eq!(Judy1Test(a, 43, null_mut()), 0);
            assert_eq!(Judy1Count(a, 0, usize::MAX, null_mut()), 3);
            assert_eq!(Judy1Count(a, 8, 42, null_mut()), 1);

            let mut idx: Word = 0;
            assert_eq!(Judy1First(a, &raw mut idx, null_mut()), 1);
            assert_eq!(idx, 7);
            assert_eq!(Judy1Next(a, &raw mut idx, null_mut()), 1);
            assert_eq!(idx, 42);
            idx = usize::MAX;
            assert_eq!(Judy1Last(a, &raw mut idx, null_mut()), 1);
            assert_eq!(idx, usize::MAX);
            assert_eq!(Judy1Prev(a, &raw mut idx, null_mut()), 1);
            assert_eq!(idx, 42);

            let mut nth: Word = 0;
            assert_eq!(Judy1ByCount(a, 2, &raw mut nth, null_mut()), 1);
            assert_eq!(nth, 42);
            assert_eq!(Judy1ByCount(a, 4, &raw mut nth, null_mut()), 0);

            idx = 7;
            assert_eq!(Judy1FirstEmpty(a, &raw mut idx, null_mut()), 1);
            assert_eq!(idx, 8);
            idx = usize::MAX;
            assert_eq!(Judy1LastEmpty(a, &raw mut idx, null_mut()), 1);
            assert_eq!(idx, usize::MAX - 1);

            assert_eq!(Judy1Unset(&raw mut a, 43, null_mut()), 0);
            assert_eq!(Judy1Unset(&raw mut a, 42, null_mut()), 1);
            assert!(Judy1MemUsed(a) > 0);
            let freed = Judy1FreeArray(&raw mut a, null_mut());
            assert!(freed > 0);
            assert!(a.is_null());

            // Emptying by Unset also nulls the handle.
            assert_eq!(Judy1Set(&raw mut a, 5, null_mut()), 1);
            assert_eq!(Judy1Unset(&raw mut a, 5, null_mut()), 1);
            assert!(a.is_null());
        }
    }

    #[test]
    fn judyl_smoke() {
        // SAFETY: valid pointers throughout; slots written before any
        // further structural mutation, per the JudyL contract.
        unsafe {
            let mut a = arr();
            let slot = JudyLIns(&raw mut a, 100, null_mut()).cast::<Word>();
            assert!(!slot.is_null() && slot != PJERR.cast());
            assert_eq!(*slot, 0, "new slots are zero-initialized");
            *slot = 12345;

            // Re-insert keeps the existing value.
            let slot2 = JudyLIns(&raw mut a, 100, null_mut()).cast::<Word>();
            assert_eq!(*slot2, 12345);

            let got = JudyLGet(a, 100, null_mut()).cast::<Word>();
            assert_eq!(*got, 12345);
            assert!(JudyLGet(a, 101, null_mut()).is_null());

            *JudyLIns(&raw mut a, 5, null_mut()).cast::<Word>() = 55;
            *JudyLIns(&raw mut a, usize::MAX, null_mut()).cast::<Word>() = 99;

            let mut idx: Word = 0;
            let first = JudyLFirst(a, &raw mut idx, null_mut()).cast::<Word>();
            assert_eq!((idx, *first), (5, 55));
            let next = JudyLNext(a, &raw mut idx, null_mut()).cast::<Word>();
            assert_eq!((idx, *next), (100, 12345));

            assert_eq!(JudyLCount(a, 0, usize::MAX, null_mut()), 3);
            let mut nth: Word = 0;
            let bc = JudyLByCount(a, 3, &raw mut nth, null_mut()).cast::<Word>();
            assert_eq!((nth, *bc), (usize::MAX, 99));

            idx = 5;
            assert_eq!(JudyLFirstEmpty(a, &raw mut idx, null_mut()), 1);
            assert_eq!(idx, 6);

            assert_eq!(JudyLDel(&raw mut a, 100, null_mut()), 1);
            assert_eq!(JudyLDel(&raw mut a, 100, null_mut()), 0);
            let freed = JudyLFreeArray(&raw mut a, null_mut());
            assert!(freed > 0);
            assert!(a.is_null());
        }
    }

    #[test]
    fn null_argument_errors() {
        // SAFETY: exercising the documented error paths.
        unsafe {
            let mut err = JError {
                je_errno: JU_ERRNO_NONE,
                je_err_id: 0,
                je_reserved: [0; 4],
            };
            assert_eq!(Judy1Set(null_mut(), 1, &raw mut err), JERR_INT);
            assert_eq!(err.je_errno, JU_ERRNO_NULLPPARRAY);
            assert_eq!(JudyLIns(null_mut(), 1, &raw mut err), PJERR);
            let a = arr();
            assert_eq!(Judy1First(a, null_mut(), &raw mut err), JERR_INT);
            assert_eq!(err.je_errno, JU_ERRNO_NULLPINDEX);
        }
    }
}

/// Differential oracle against a stock libjudy, loaded with `dlopen` so
/// our exported `Judy*` symbols never collide with the reference library's
/// at link time (COMPAT.md gate G1). Enabled by the `oracle` feature; the
/// CI job installs `libjudy-dev` (locally: Homebrew `judy` on macOS).
#[cfg(all(
    test,
    feature = "oracle",
    any(target_os = "linux", target_os = "macos")
))]
mod oracle {
    use super::*;

    unsafe extern "C" {
        fn dlopen(filename: *const u8, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const u8) -> *mut c_void;
    }
    const RTLD_NOW: c_int = 2;

    type F2i = unsafe extern "C" fn(*mut *mut c_void, Word, *mut c_void) -> c_int;
    type F2t = unsafe extern "C" fn(*const c_void, Word, *mut c_void) -> c_int;
    type FCount = unsafe extern "C" fn(*const c_void, Word, Word, *mut c_void) -> Word;
    type FNav = unsafe extern "C" fn(*const c_void, *mut Word, *mut c_void) -> c_int;
    type FByC = unsafe extern "C" fn(*const c_void, Word, *mut Word, *mut c_void) -> c_int;
    type FFree = unsafe extern "C" fn(*mut *mut c_void, *mut c_void) -> Word;
    type FpIns = unsafe extern "C" fn(*mut *mut c_void, Word, *mut c_void) -> *mut c_void;
    type FpGet = unsafe extern "C" fn(*const c_void, Word, *mut c_void) -> *mut c_void;
    type FpNav = unsafe extern "C" fn(*const c_void, *mut Word, *mut c_void) -> *mut c_void;
    type FpByC = unsafe extern "C" fn(*const c_void, Word, *mut Word, *mut c_void) -> *mut c_void;

    struct Lib(*mut c_void);
    impl Lib {
        fn open() -> Self {
            let names: [&core::ffi::CStr; 4] = [
                c"libJudy.so.1",
                c"libJudy.so",
                c"/opt/homebrew/opt/judy/lib/libJudy.dylib",
                c"libJudy.dylib",
            ];
            for name in names.map(core::ffi::CStr::to_bytes_with_nul) {
                // SAFETY: valid NUL-terminated name.
                let h = unsafe { dlopen(name.as_ptr(), RTLD_NOW) };
                if !h.is_null() {
                    return Self(h);
                }
            }
            panic!("stock libjudy not found — install libjudy-dev");
        }
        fn sym<T: Copy>(&self, name: &core::ffi::CStr) -> T {
            // SAFETY: valid handle + NUL-terminated name; the caller names
            // the correct fn-pointer type for the symbol.
            let p = unsafe { dlsym(self.0, name.to_bytes_with_nul().as_ptr()) };
            assert!(!p.is_null(), "missing symbol {name:?}");
            // SAFETY: fn-pointer transmute of a resolved symbol.
            unsafe { core::mem::transmute_copy(&p) }
        }
    }

    struct XorShift(u64);
    impl XorShift {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
    }

    fn keygen(rng: &mut XorShift) -> Word {
        (match rng.next() % 5 {
            0 => rng.next() % 512,
            1 => 0xAB_CD00_0000 + (rng.next() % 300),
            2 => (rng.next() % 1024) << 48,
            // 256-aligned run at a fixed deep base: exercises the
            // narrow-pointer (skip-carrying leaf) paths against stock.
            3 => 0x99_8877_6655_4400 + (rng.next() % 256),
            _ => rng.next(),
        }) as Word
    }

    #[test]
    fn judy1_matches_stock_libjudy() {
        let lib = Lib::open();
        let o_set: F2i = lib.sym(c"Judy1Set");
        let o_unset: F2i = lib.sym(c"Judy1Unset");
        let o_test: F2t = lib.sym(c"Judy1Test");
        let o_count: FCount = lib.sym(c"Judy1Count");
        let o_byc: FByC = lib.sym(c"Judy1ByCount");
        let o_first: FNav = lib.sym(c"Judy1First");
        let o_next: FNav = lib.sym(c"Judy1Next");
        let _o_last: FNav = lib.sym(c"Judy1Last");
        let o_prev: FNav = lib.sym(c"Judy1Prev");
        let o_fe: FNav = lib.sym(c"Judy1FirstEmpty");
        let o_le: FNav = lib.sym(c"Judy1LastEmpty");
        let o_free: FFree = lib.sym(c"Judy1FreeArray");

        for seed in [0xA1u64, 0xB2, 0xC3] {
            let mut rng = XorShift(seed | 1);
            let mut ours: *mut c_void = null_mut();
            let mut theirs: *mut c_void = null_mut();
            // SAFETY: both stacks driven through their C contracts with
            // valid pointers; sequences identical on both sides.
            unsafe {
                for _ in 0..4000 {
                    let k = keygen(&mut rng);
                    match rng.next() % 8 {
                        0..=2 => assert_eq!(
                            Judy1Set(&raw mut ours, k, null_mut()),
                            o_set(&raw mut theirs, k, null_mut()),
                            "set {k:#x}"
                        ),
                        3 => assert_eq!(
                            Judy1Unset(&raw mut ours, k, null_mut()),
                            o_unset(&raw mut theirs, k, null_mut()),
                            "unset {k:#x}"
                        ),
                        4 => assert_eq!(
                            Judy1Test(ours, k, null_mut()),
                            o_test(theirs, k, null_mut()),
                            "test {k:#x}"
                        ),
                        5 => {
                            let k2 = keygen(&mut rng);
                            let (a, b) = (k.min(k2), k.max(k2));
                            assert_eq!(
                                Judy1Count(ours, a, b, null_mut()),
                                o_count(theirs, a, b, null_mut()),
                                "count {a:#x}..={b:#x}"
                            );
                        }
                        6 => {
                            let (mut i1, mut i2) = (k, k);
                            let r1 = Judy1Next(ours, &raw mut i1, null_mut());
                            let r2 = o_next(theirs, &raw mut i2, null_mut());
                            assert_eq!(
                                (r1, i1),
                                (r2, if r2 == 1 { i2 } else { i1 }),
                                "next {k:#x}"
                            );
                            let (mut i1, mut i2) = (k, k);
                            let r1 = Judy1Prev(ours, &raw mut i1, null_mut());
                            let r2 = o_prev(theirs, &raw mut i2, null_mut());
                            assert_eq!(
                                (r1, i1),
                                (r2, if r2 == 1 { i2 } else { i1 }),
                                "prev {k:#x}"
                            );
                        }
                        _ => {
                            let (mut i1, mut i2) = (k, k);
                            let r1 = Judy1FirstEmpty(ours, &raw mut i1, null_mut());
                            let r2 = o_fe(theirs, &raw mut i2, null_mut());
                            assert_eq!((r1, i1), (r2, if r2 == 1 { i2 } else { i1 }), "fe {k:#x}");
                            let (mut i1, mut i2) = (k, k);
                            let r1 = Judy1LastEmpty(ours, &raw mut i1, null_mut());
                            let r2 = o_le(theirs, &raw mut i2, null_mut());
                            assert_eq!((r1, i1), (r2, if r2 == 1 { i2 } else { i1 }), "le {k:#x}");
                        }
                    }
                }
                // Full ordered sweep must agree in both directions.
                let (mut i1, mut i2) = (0 as Word, 0 as Word);
                let (mut r1, mut r2) = (
                    Judy1First(ours, &raw mut i1, null_mut()),
                    o_first(theirs, &raw mut i2, null_mut()),
                );
                let mut n = 0u64;
                while r1 == 1 || r2 == 1 {
                    assert_eq!((r1, i1), (r2, i2), "sweep step {n}");
                    n += 1;
                    r1 = Judy1Next(ours, &raw mut i1, null_mut());
                    r2 = o_next(theirs, &raw mut i2, null_mut());
                }
                // Rank agreement over the whole population.
                for nth in [1 as Word, 2, n as Word / 2, n as Word] {
                    let (mut i1, mut i2) = (0 as Word, 0 as Word);
                    let r1 = Judy1ByCount(ours, nth, &raw mut i1, null_mut());
                    let r2 = o_byc(theirs, nth, &raw mut i2, null_mut());
                    assert_eq!(
                        (r1, i1),
                        (r2, if r2 == 1 { i2 } else { i1 }),
                        "bycount {nth}"
                    );
                }
                assert!(Judy1FreeArray(&raw mut ours, null_mut()) > 0);
                assert!(o_free(&raw mut theirs, null_mut()) > 0);
                assert!(ours.is_null() && theirs.is_null());
            }
        }
    }

    #[test]
    fn judyl_matches_stock_libjudy() {
        let lib = Lib::open();
        let o_ins: FpIns = lib.sym(c"JudyLIns");
        let o_del: F2i = lib.sym(c"JudyLDel");
        let o_get: FpGet = lib.sym(c"JudyLGet");
        let o_count: FCount = lib.sym(c"JudyLCount");
        let o_byc: FpByC = lib.sym(c"JudyLByCount");
        let o_first: FpNav = lib.sym(c"JudyLFirst");
        let o_next: FpNav = lib.sym(c"JudyLNext");
        let o_free: FFree = lib.sym(c"JudyLFreeArray");

        for seed in [0x11u64, 0x22, 0x33] {
            let mut rng = XorShift(seed | 1);
            let mut ours: *mut c_void = null_mut();
            let mut theirs: *mut c_void = null_mut();
            // SAFETY: as in the Judy1 oracle; slots written immediately
            // after the call that returned them.
            unsafe {
                for _ in 0..4000 {
                    let k = keygen(&mut rng);
                    match rng.next() % 6 {
                        0..=2 => {
                            let v = rng.next() as Word;
                            let s1 = JudyLIns(&raw mut ours, k, null_mut()).cast::<Word>();
                            let s2 = o_ins(&raw mut theirs, k, null_mut()).cast::<Word>();
                            assert_eq!(*s1, *s2, "ins existing value {k:#x}");
                            *s1 = v;
                            *s2 = v;
                        }
                        3 => assert_eq!(
                            JudyLDel(&raw mut ours, k, null_mut()),
                            o_del(&raw mut theirs, k, null_mut()),
                            "del {k:#x}"
                        ),
                        4 => {
                            let s1 = JudyLGet(ours, k, null_mut()).cast::<Word>();
                            let s2 = o_get(theirs, k, null_mut()).cast::<Word>();
                            assert_eq!(s1.is_null(), s2.is_null(), "get null-ness {k:#x}");
                            if !s1.is_null() {
                                assert_eq!(*s1, *s2, "get value {k:#x}");
                            }
                        }
                        _ => {
                            let k2 = keygen(&mut rng);
                            let (a, b) = (k.min(k2), k.max(k2));
                            assert_eq!(
                                JudyLCount(ours, a, b, null_mut()),
                                o_count(theirs, a, b, null_mut()),
                                "count {a:#x}..={b:#x}"
                            );
                        }
                    }
                }
                // Ordered sweep with value agreement.
                let (mut i1, mut i2) = (0 as Word, 0 as Word);
                let (mut s1, mut s2) = (
                    JudyLFirst(ours, &raw mut i1, null_mut()),
                    o_first(theirs, &raw mut i2, null_mut()),
                );
                let mut n: Word = 0;
                while !s1.is_null() || !s2.is_null() {
                    assert_eq!(s1.is_null(), s2.is_null(), "sweep null-ness at {n}");
                    assert_eq!(i1, i2, "sweep key at {n}");
                    assert_eq!(*s1.cast::<Word>(), *s2.cast::<Word>(), "sweep value at {n}");
                    n += 1;
                    s1 = JudyLNext(ours, &raw mut i1, null_mut());
                    s2 = o_next(theirs, &raw mut i2, null_mut());
                }
                for nth in [1 as Word, n / 2, n] {
                    let (mut i1, mut i2) = (0 as Word, 0 as Word);
                    let s1 = o_byc(theirs, nth, &raw mut i2, null_mut());
                    let s2 = JudyLByCount(ours, nth, &raw mut i1, null_mut());
                    assert_eq!(s2.is_null(), s1.is_null(), "bycount null-ness {nth}");
                    if !s1.is_null() {
                        assert_eq!(i1, i2, "bycount key {nth}");
                        assert_eq!(
                            *s2.cast::<Word>(),
                            *s1.cast::<Word>(),
                            "bycount value {nth}"
                        );
                    }
                }
                assert!(JudyLFreeArray(&raw mut ours, null_mut()) > 0);
                assert!(o_free(&raw mut theirs, null_mut()) > 0);
                assert!(ours.is_null() && theirs.is_null());
            }
        }
    }
}
