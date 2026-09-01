//! The legacy `Judy*` drop-in surface (`Judy1*`, `JudyL*`, `JudySL*`,
//! `JudyHS*`).
//!
//! 64-bit only: `ExpanseStrMap`/`ExpanseBytesMap` exist only at that width,
//! and the 32-bit engine has no rank/select or value-slot accessors for the
//! `*ByCount` and `*Ins` contracts. The whole module is gated rather than
//! partially stubbed — a `Judy*` symbol that is present but wrong is worse
//! than one that is absent, and a link error names the gap. `docs/COMPAT.md`
//! carries the surface matrix.
//!
//! Semantics are unchanged here; this is a `cfg` boundary only (#558).

// Same rationale as the crate root: the safety contract is the documented
// Judy C API itself.
#![allow(clippy::missing_safety_doc)]

#[cfg(not(feature = "std"))]
use crate::core_alloc::boxed::Box;
use crate::{
    ExpanseBytesMap, ExpanseMap, ExpanseSet, ExpanseStrMap, JError, JU_ERRNO_NULLPINDEX,
    JU_ERRNO_NULLPPARRAY, Word,
};
use core::ffi::{c_int, c_void};
use core::ptr::{NonNull, null_mut};

pub(crate) const JERR_INT: c_int = -1;
/// The `PJERR` sentinel returned by pointer-returning entry points.
pub(crate) const PJERR: *mut c_void = usize::MAX as *mut c_void;

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
#[inline(always)]
unsafe fn set_handle_mut<'a>(pparray: *mut *mut c_void) -> &'a mut ExpanseSet {
    // SAFETY: per this function's contract; creates the tree on demand.
    unsafe {
        let arr = *pparray;
        if !arr.is_null() {
            &mut *arr.cast::<ExpanseSet>()
        } else {
            let boxed = Box::into_raw(Box::new(ExpanseSet::new())).cast();
            *pparray = boxed;
            &mut *boxed.cast::<ExpanseSet>()
        }
    }
}

#[inline(always)]
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
        set_handle_mut(pparray).insert_c_int(index as u64)
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
    unsafe {
        if parray.is_null() {
            return 0;
        }
        let set = &*parray.cast::<ExpanseSet>();
        set.contains_c_int(index as u64)
    }
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

#[inline(always)]
unsafe fn map_handle_mut<'a>(pparray: *mut *mut c_void) -> &'a mut ExpanseMap {
    // SAFETY: per set_handle_mut's contract, for maps.
    unsafe {
        let arr = *pparray;
        if !arr.is_null() {
            &mut *arr.cast::<ExpanseMap>()
        } else {
            let boxed = Box::into_raw(Box::new(ExpanseMap::new())).cast();
            *pparray = boxed;
            &mut *boxed.cast::<ExpanseMap>()
        }
    }
}

/// JudyL handles are always treated as mutable internally: the C contract
/// returns writable value slots even from `Pcvoid_t` entry points.
#[inline(always)]
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
        // Single fused walk: insert-if-absent and slot in one descent.
        map_handle_mut(pparray)
            .ins_slot(index as u64)
            .as_ptr()
            .cast()
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
    unsafe {
        if parray.is_null() {
            return null_mut();
        }
        let map = &mut *parray.cast_mut().cast::<ExpanseMap>();
        match map.get_value_slot(index as u64) {
            Some(p) => p.as_ptr().cast(),
            None => null_mut(),
        }
    }
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

// ---------------------------------------------------------------------
// JudySL (C-string → word map) — backed by ExpanseStrMap
// ---------------------------------------------------------------------

/// # Safety
/// `p` must be a valid NUL-terminated C string.
unsafe fn cstr_bytes<'a>(p: *const u8) -> &'a [u8] {
    // SAFETY: caller passes a NUL-terminated string.
    unsafe {
        let mut n = 0usize;
        while *p.add(n) != 0 {
            n += 1;
        }
        core::slice::from_raw_parts(p, n)
    }
}

/// Copies `bytes` + NUL into the caller's index buffer (the documented
/// `JudySL` navigation contract: the buffer must be large enough for the
/// longest stored string).
///
/// # Safety
/// `dst` must be valid for `bytes.len() + 1` writes.
unsafe fn write_cstr(dst: *mut u8, bytes: &[u8]) {
    // SAFETY: per this function's contract.
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len());
        *dst.add(bytes.len()) = 0;
    }
}

unsafe fn strmap_handle_mut<'a>(pparray: *mut *mut c_void) -> &'a mut ExpanseStrMap {
    // SAFETY: per set_handle_mut's contract, for string maps.
    unsafe {
        if (*pparray).is_null() {
            *pparray = Box::into_raw(Box::new(ExpanseStrMap::new())).cast();
        }
        &mut *(*pparray).cast::<ExpanseStrMap>()
    }
}

unsafe fn strmap_handle<'a>(parray: *const c_void) -> Option<&'a mut ExpanseStrMap> {
    // SAFETY: null or a handle produced by these entry points; the C
    // contract returns writable slots even from Pcvoid_t entry points.
    unsafe { parray.cast_mut().cast::<ExpanseStrMap>().as_mut() }
}

/// Inserts `index` (value slot zero-initialized if new) and returns a
/// writable pointer to its value slot; `PJERR` on error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn JudySLIns(
    pparray: *mut *mut c_void,
    index: *const u8,
    pj: *mut JError,
) -> *mut c_void {
    // SAFETY: forwarded C contract.
    unsafe {
        if pparray.is_null() {
            set_err(pj, JU_ERRNO_NULLPPARRAY);
            return PJERR;
        }
        if index.is_null() {
            set_err(pj, JU_ERRNO_NULLPINDEX);
            return PJERR;
        }
        strmap_handle_mut(pparray)
            .ins_slot(cstr_bytes(index))
            .as_ptr()
            .cast()
    }
}

/// Deletes `index`; returns 1 if it was present. An emptied array word
/// returns to null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn JudySLDel(
    pparray: *mut *mut c_void,
    index: *const u8,
    pj: *mut JError,
) -> c_int {
    // SAFETY: forwarded C contract.
    unsafe {
        if pparray.is_null() {
            set_err(pj, JU_ERRNO_NULLPPARRAY);
            return JERR_INT;
        }
        if index.is_null() {
            set_err(pj, JU_ERRNO_NULLPINDEX);
            return JERR_INT;
        }
        if (*pparray).is_null() {
            return 0;
        }
        let map = &mut *(*pparray).cast::<ExpanseStrMap>();
        let removed = map.remove(cstr_bytes(index)).is_some();
        if removed && map.is_empty() {
            drop(Box::from_raw((*pparray).cast::<ExpanseStrMap>()));
            *pparray = null_mut();
        }
        c_int::from(removed)
    }
}

/// Returns a writable pointer to `index`'s value slot, or null if absent.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn JudySLGet(
    parray: *const c_void,
    index: *const u8,
    _pj: *mut JError,
) -> *mut c_void {
    // SAFETY: forwarded C contract.
    unsafe {
        if index.is_null() {
            return null_mut();
        }
        strmap_handle(parray).map_or(null_mut(), |m| {
            slot_ptr(m.get_value_slot(cstr_bytes(index)))
        })
    }
}

macro_rules! judysl_nav {
    ($name:ident, $method:ident, $doc:literal) => {
        #[doc = $doc]
        #[doc = ""]
        #[doc = "Writes the found string (NUL-terminated) back into the"]
        #[doc = "caller's `index` buffer, which must be large enough for"]
        #[doc = "the longest stored string (the documented contract)."]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            parray: *const c_void,
            index: *mut u8,
            pj: *mut JError,
        ) -> *mut c_void {
            // SAFETY: forwarded C contract.
            unsafe {
                if index.is_null() {
                    set_err(pj, JU_ERRNO_NULLPINDEX);
                    return PJERR;
                }
                let Some(map) = strmap_handle(parray) else {
                    return null_mut();
                };
                match map.$method(cstr_bytes(index)) {
                    Some((key, slot)) => {
                        write_cstr(index, &key);
                        slot.as_ptr().cast()
                    }
                    None => null_mut(),
                }
            }
        }
    };
}

judysl_nav!(
    JudySLFirst,
    next_at_or_after,
    "Smallest stored string >= *index; returns its value slot."
);
judysl_nav!(
    JudySLNext,
    next_after,
    "Smallest stored string > *index; returns its value slot."
);
judysl_nav!(
    JudySLLast,
    prev_at_or_before,
    "Largest stored string <= *index; returns its value slot."
);
judysl_nav!(
    JudySLPrev,
    prev_before,
    "Largest stored string < *index; returns its value slot."
);

/// Frees the whole array; returns the bytes freed and nulls the word.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn JudySLFreeArray(pparray: *mut *mut c_void, pj: *mut JError) -> Word {
    // SAFETY: forwarded C contract.
    unsafe {
        if pparray.is_null() {
            set_err(pj, JU_ERRNO_NULLPPARRAY);
            return 0;
        }
        if (*pparray).is_null() {
            return 0;
        }
        let mut boxed = Box::from_raw((*pparray).cast::<ExpanseStrMap>());
        let bytes = boxed.clear() + size_of::<ExpanseStrMap>() as u64;
        drop(boxed);
        *pparray = null_mut();
        bytes as Word
    }
}

// ---------------------------------------------------------------------
// JudyHS (byte-string → word hash map) — backed by ExpanseBytesMap
// ---------------------------------------------------------------------

unsafe fn bytesmap_handle_mut<'a>(pparray: *mut *mut c_void) -> &'a mut ExpanseBytesMap {
    // SAFETY: per set_handle_mut's contract, for byte-string maps.
    unsafe {
        if (*pparray).is_null() {
            *pparray = Box::into_raw(Box::new(ExpanseBytesMap::new())).cast();
        }
        &mut *(*pparray).cast::<ExpanseBytesMap>()
    }
}

/// Reads the (index, length) byte-string argument. A null index is only
/// legal for a zero-length string.
unsafe fn hs_key<'a>(index: *const c_void, length: Word) -> Option<&'a [u8]> {
    if length > (isize::MAX as usize) {
        return None;
    }
    if index.is_null() {
        return (length == 0).then_some(&[]);
    }
    // SAFETY: caller passes `length` readable bytes per the C contract; length <= isize::MAX.
    Some(unsafe { core::slice::from_raw_parts(index.cast::<u8>(), length) })
}

/// Inserts the byte string (value slot zero-initialized if new) and
/// returns a writable pointer to its value slot; `PJERR` on error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn JudyHSIns(
    pparray: *mut *mut c_void,
    index: *const c_void,
    length: Word,
    pj: *mut JError,
) -> *mut c_void {
    // SAFETY: forwarded C contract.
    unsafe {
        if pparray.is_null() {
            set_err(pj, JU_ERRNO_NULLPPARRAY);
            return PJERR;
        }
        let Some(key) = hs_key(index, length) else {
            set_err(pj, JU_ERRNO_NULLPINDEX);
            return PJERR;
        };
        bytesmap_handle_mut(pparray).ins_slot(key).as_ptr().cast()
    }
}

/// Returns the value slot of the byte string, or null if absent.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn JudyHSGet(
    parray: *const c_void,
    index: *const c_void,
    length: Word,
) -> *mut c_void {
    // SAFETY: forwarded C contract (the C surface hands out writable
    // slots even from Pcvoid_t entry points).
    unsafe {
        let Some(map) = parray.cast_mut().cast::<ExpanseBytesMap>().as_mut() else {
            return null_mut();
        };
        let Some(key) = hs_key(index, length) else {
            return null_mut();
        };
        map.get_value_slot(key)
            .map_or(null_mut(), |p| p.as_ptr().cast())
    }
}

/// Deletes the byte string; returns 1 if it was present. An emptied
/// array word returns to null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn JudyHSDel(
    pparray: *mut *mut c_void,
    index: *const c_void,
    length: Word,
    pj: *mut JError,
) -> c_int {
    // SAFETY: forwarded C contract.
    unsafe {
        if pparray.is_null() {
            set_err(pj, JU_ERRNO_NULLPPARRAY);
            return JERR_INT;
        }
        let Some(key) = hs_key(index, length) else {
            set_err(pj, JU_ERRNO_NULLPINDEX);
            return JERR_INT;
        };
        if (*pparray).is_null() {
            return 0;
        }
        let map = &mut *(*pparray).cast::<ExpanseBytesMap>();
        let removed = map.remove(key).is_some();
        if removed && map.is_empty() {
            drop(Box::from_raw((*pparray).cast::<ExpanseBytesMap>()));
            *pparray = null_mut();
        }
        c_int::from(removed)
    }
}

/// Frees the whole array; returns the bytes freed (implementation-
/// defined here — see docs/COMPAT.md doc-gap D4) and nulls the word.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn JudyHSFreeArray(pparray: *mut *mut c_void, pj: *mut JError) -> Word {
    // SAFETY: forwarded C contract.
    unsafe {
        if pparray.is_null() {
            set_err(pj, JU_ERRNO_NULLPPARRAY);
            return 0;
        }
        if (*pparray).is_null() {
            return 0;
        }
        let boxed = Box::from_raw((*pparray).cast::<ExpanseBytesMap>());
        let bytes = boxed.mem_used() + size_of::<ExpanseBytesMap>();
        drop(boxed);
        *pparray = null_mut();
        bytes as Word
    }
}
