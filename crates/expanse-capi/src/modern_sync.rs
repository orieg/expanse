//! The concurrent half of the modern `expanse_*` C API.
//!
//! `expanse_trie::sync` is gated on `all(target_pointer_width = "64",
//! feature = "std")` — the one-writer/many-reader containers need
//! `std::sync` — so these entry points are absent from every `no_std`
//! build, 64-bit ones included. See the surface matrix in
//! `docs/COMPAT.md`.

use super::super::{cstr, put};
use super::ExpanseStrNavStatus;
use core::ffi::c_char;
use expanse_trie::sync::{
    MapReader, SetReader, StrReader, SyncExpanseMap, SyncExpanseSet, SyncExpanseStrMap,
};

// ---------------------------------------------------------------------

/// A registered reader of a [`SyncExpanseSet`].
///
/// The Rust handle borrows its container; the C handle stores that
/// borrow as `'static`, which the header's contract upholds (a reader
/// must be freed before its container).
pub struct SyncSetReader(SetReader<'static>);

/// A registered reader of a [`SyncExpanseMap`]. See [`SyncSetReader`].
pub struct SyncMapReader(MapReader<'static>);

/// A registered reader of a [`SyncExpanseStrMap`]. See [`SyncSetReader`].
pub struct SyncStrMapReader(StrReader<'static>);

// `include/expanse.h` documents that a reader handle may be freed from a thread
// other than the one that created it. That holds because both handle types are
// `Send`, which is auto-derived from their fields; assert it, so a future
// `!Send` field breaks the build instead of the documented contract.
//
// The Rust handles inside are `Send` and not `Sync`. That bound stops at this
// boundary: C callers hold raw pointers, so "one handle per reading thread"
// is the header's contract here and not something this crate can check.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<SyncSetReader>();
    assert_send::<SyncMapReader>();
    assert_send::<SyncStrMapReader>();
};

/// Creates an empty concurrent set.
#[unsafe(no_mangle)]
pub extern "C" fn expanse_sync_set_new() -> *mut SyncExpanseSet {
    Box::into_raw(Box::new(SyncExpanseSet::new()))
}

/// Frees a concurrent set. All readers must be freed first.
///
/// # Safety
///
/// `set` must come from `expanse_sync_set_new`, with no live readers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_set_free(set: *mut SyncExpanseSet) {
    if !set.is_null() {
        // SAFETY: handle per contract.
        drop(unsafe { Box::from_raw(set) });
    }
}

/// Inserts `key`; true if newly inserted. Serializes with other writers.
///
/// # Safety
///
/// `set` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_set_insert(set: *const SyncExpanseSet, key: u64) -> bool {
    // SAFETY: null or live handle per contract (writes go through &self).
    unsafe { set.as_ref() }.is_some_and(|s| s.insert(key))
}

/// Removes `key`; true if it was present.
///
/// # Safety
///
/// Same contract as [`expanse_sync_set_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_set_remove(set: *const SyncExpanseSet, key: u64) -> bool {
    // SAFETY: null or live handle per contract.
    unsafe { set.as_ref() }.is_some_and(|s| s.remove(key))
}

/// One-shot membership test (registers a throwaway reader; prefer a
/// reader handle in hot loops).
///
/// # Safety
///
/// Same contract as [`expanse_sync_set_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_set_contains(set: *const SyncExpanseSet, key: u64) -> bool {
    // SAFETY: null or live handle per contract.
    unsafe { set.as_ref() }.is_some_and(|s| s.contains(key))
}

/// Number of keys.
///
/// # Safety
///
/// Same contract as [`expanse_sync_set_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_set_len(set: *const SyncExpanseSet) -> u64 {
    // SAFETY: null or live handle per contract.
    unsafe { set.as_ref() }.map_or(0, SyncExpanseSet::len)
}

/// Heap bytes the set holds from the global allocator: its tree's own share
/// plus the blocks its epoch collector holds for reuse or is waiting to
/// reclaim ([`SyncExpanseSet::mem_held`]); 0 for a null handle. Writers wait
/// for it.
///
/// # Safety
///
/// Same contract as [`expanse_sync_set_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_set_mem_held(set: *const SyncExpanseSet) -> usize {
    // SAFETY: null or live handle per contract.
    unsafe { set.as_ref() }.map_or(0, SyncExpanseSet::mem_held)
}

/// Returns the freed blocks the set's epoch collector keeps for reuse to the
/// global allocator and returns the bytes released
/// ([`SyncExpanseSet::shrink_to_fit`]); 0 for a null handle. Runs beside
/// readers and writers without excluding either.
///
/// # Safety
///
/// Same contract as [`expanse_sync_set_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_set_shrink_to_fit(set: *const SyncExpanseSet) -> usize {
    // SAFETY: null or live handle per contract.
    unsafe { set.as_ref() }.map_or(0, SyncExpanseSet::shrink_to_fit)
}

/// Registers a reader handle for this thread. Free it before the set.
///
/// # Safety
///
/// `set` must be null or a live handle that outlives the reader.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_set_reader_new(
    set: *const SyncExpanseSet,
) -> *mut SyncSetReader {
    // SAFETY: null or live handle per contract; the reader's borrow is
    // extended to 'static, which the caller upholds by freeing the
    // reader first (documented in expanse.h).
    let Some(s) = (unsafe { set.as_ref() }) else {
        return core::ptr::null_mut();
    };
    // SAFETY: lifetime extension only — the reader borrows `set`, and
    // the header's contract requires freeing the reader before its set.
    let reader: SetReader<'static> = unsafe { core::mem::transmute(s.reader()) };
    Box::into_raw(Box::new(SyncSetReader(reader)))
}

/// Frees a reader handle.
///
/// May be called from a thread other than the one that created the reader,
/// once no call on it is in progress (`include/expanse.h`).
///
/// # Safety
///
/// `reader` must come from `expanse_sync_set_reader_new`, unused after,
/// with no call on it in progress on any thread, and its set still alive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_set_reader_free(reader: *mut SyncSetReader) {
    if !reader.is_null() {
        // SAFETY: handle per contract.
        drop(unsafe { Box::from_raw(reader) });
    }
}

/// Optimistic membership test through a registered reader.
///
/// # Safety
///
/// `reader` must be null or live, and its set still alive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_set_reader_contains(
    reader: *const SyncSetReader,
    key: u64,
) -> bool {
    // SAFETY: null or live reader per contract.
    unsafe { reader.as_ref() }.is_some_and(|r| r.0.contains(key))
}

/// Creates an empty concurrent map.
#[unsafe(no_mangle)]
pub extern "C" fn expanse_sync_map_new() -> *mut SyncExpanseMap {
    Box::into_raw(Box::new(SyncExpanseMap::new()))
}

/// Frees a concurrent map. All readers must be freed first.
///
/// # Safety
///
/// `map` must come from `expanse_sync_map_new`, with no live readers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_free(map: *mut SyncExpanseMap) {
    if !map.is_null() {
        // SAFETY: handle per contract.
        drop(unsafe { Box::from_raw(map) });
    }
}

/// Stores `key -> value` (return convention as [`expanse_map_insert`]).
///
/// # Safety
///
/// `map` must be null or a live handle; `old_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_insert(
    map: *const SyncExpanseMap,
    key: u64,
    value: u64,
    old_out: *mut u64,
) -> bool {
    // SAFETY: null or live handle per contract.
    let Some(m) = (unsafe { map.as_ref() }) else {
        return false;
    };
    match m.insert(key, value) {
        // SAFETY: `old_out` null or writable per contract.
        Some(old) => unsafe {
            put(old_out, old);
            false
        },
        None => true,
    }
}

/// One-shot lookup (prefer a reader handle in hot loops).
///
/// # Safety
///
/// `map` must be null or a live handle; `value_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_get(
    map: *const SyncExpanseMap,
    key: u64,
    value_out: *mut u64,
) -> bool {
    // SAFETY: null or live handle per contract.
    let Some(v) = (unsafe { map.as_ref() }).and_then(|m| m.get(key)) else {
        return false;
    };
    // SAFETY: `value_out` null or writable per contract.
    unsafe { put(value_out, v) };
    true
}

/// Removes `key`, reporting its value; false if absent.
///
/// # Safety
///
/// `map` must be null or a live handle; `old_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_remove(
    map: *const SyncExpanseMap,
    key: u64,
    old_out: *mut u64,
) -> bool {
    // SAFETY: null or live handle per contract.
    let Some(v) = (unsafe { map.as_ref() }).and_then(|m| m.remove(key)) else {
        return false;
    };
    // SAFETY: `old_out` null or writable per contract.
    unsafe { put(old_out, v) };
    true
}

/// Stores `new_value` under `key` only if the key's current word is
/// `expected_value`, as one linearizable step. An absent key is expressed by
/// the `_present` flag being false, on the input and the output side alike
/// (`new_present == false` removes the key; `expected_present == false`
/// stores only if the key is absent), so every `u64` stays a legal value.
///
/// Returns true when the store happened. On both outcomes the observed
/// word, the one the compare saw, is reported through `observed_present_out`
/// and `observed_out` (each null or writable; `observed_out` is written only
/// when the key was present), so a failed call hands the caller the word to
/// retry against. A NULL `map` returns false and writes nothing.
///
/// # Safety
///
/// `map` must be null or a live handle; `observed_present_out` and
/// `observed_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_compare_exchange(
    map: *const SyncExpanseMap,
    key: u64,
    expected_present: bool,
    expected_value: u64,
    new_present: bool,
    new_value: u64,
    observed_present_out: *mut bool,
    observed_out: *mut u64,
) -> bool {
    // SAFETY: null or live handle per contract.
    let Some(m) = (unsafe { map.as_ref() }) else {
        return false;
    };
    let expected = expected_present.then_some(expected_value);
    let new = new_present.then_some(new_value);
    let (exchanged, observed) = match m.compare_exchange(key, expected, new) {
        Ok(prev) => (true, prev),
        Err(seen) => (false, seen),
    };
    // SAFETY: both outputs null or writable per contract.
    unsafe {
        put(observed_present_out, observed.is_some());
        if let Some(v) = observed {
            put(observed_out, v);
        }
    }
    exchanged
}

/// Stores `value` under `key` only if `key` is absent, as one linearizable
/// step. Returns true when it was stored. Returns false when the key was
/// present, leaving it untouched and reporting its value through
/// `existing_out` (return convention as [`expanse_sync_map_insert`]); a NULL
/// `map` also returns false and writes nothing.
///
/// # Safety
///
/// `map` must be null or a live handle; `existing_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_get_or_insert(
    map: *const SyncExpanseMap,
    key: u64,
    value: u64,
    existing_out: *mut u64,
) -> bool {
    // SAFETY: null or live handle per contract.
    let Some(m) = (unsafe { map.as_ref() }) else {
        return false;
    };
    match m.get_or_insert(key, value) {
        // SAFETY: `existing_out` null or writable per contract.
        Some(existing) => unsafe {
            put(existing_out, existing);
            false
        },
        None => true,
    }
}

/// Number of entries.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_len(map: *const SyncExpanseMap) -> u64 {
    // SAFETY: null or live handle per contract.
    unsafe { map.as_ref() }.map_or(0, SyncExpanseMap::len)
}

/// Heap bytes used by the map's nodes and leaves, read with writers excluded
/// ([`SyncExpanseMap::mem_used`]); 0 for a null handle, as
/// [`expanse_sync_map_len`].
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_mem_used(map: *const SyncExpanseMap) -> usize {
    // SAFETY: null or live handle per contract.
    unsafe { map.as_ref() }.map_or(0, SyncExpanseMap::mem_used)
}

/// Heap bytes the map holds from the global allocator: its tree's own share
/// plus the blocks its epoch collector holds for reuse or is waiting to
/// reclaim ([`SyncExpanseMap::mem_held`]); 0 for a null handle, as
/// [`expanse_sync_map_mem_used`]. Writers wait for it.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_mem_held(map: *const SyncExpanseMap) -> usize {
    // SAFETY: null or live handle per contract.
    unsafe { map.as_ref() }.map_or(0, SyncExpanseMap::mem_held)
}

/// Returns the freed blocks the map's epoch collector keeps for reuse to the
/// global allocator and returns the bytes released
/// ([`SyncExpanseMap::shrink_to_fit`]); 0 for a null handle. Runs beside
/// readers and writers without excluding either.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_shrink_to_fit(map: *const SyncExpanseMap) -> usize {
    // SAFETY: null or live handle per contract.
    unsafe { map.as_ref() }.map_or(0, SyncExpanseMap::shrink_to_fit)
}

/// Registers a reader handle for this thread. Free it before the map.
///
/// # Safety
///
/// `map` must be null or a live handle that outlives the reader.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_reader_new(
    map: *const SyncExpanseMap,
) -> *mut SyncMapReader {
    // SAFETY: as in expanse_sync_set_reader_new.
    let Some(m) = (unsafe { map.as_ref() }) else {
        return core::ptr::null_mut();
    };
    // SAFETY: lifetime extension only (see expanse_sync_set_reader_new).
    let reader: MapReader<'static> = unsafe { core::mem::transmute(m.reader()) };
    Box::into_raw(Box::new(SyncMapReader(reader)))
}

/// Frees a reader handle.
///
/// May be called from a thread other than the one that created the reader,
/// once no call on it is in progress (`include/expanse.h`).
///
/// # Safety
///
/// `reader` must come from `expanse_sync_map_reader_new`, unused after,
/// with no call on it in progress on any thread, and its map still alive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_reader_free(reader: *mut SyncMapReader) {
    if !reader.is_null() {
        // SAFETY: handle per contract.
        drop(unsafe { Box::from_raw(reader) });
    }
}

/// Optimistic lookup through a registered reader.
///
/// # Safety
///
/// `reader` must be null or live (its map still alive); `value_out`
/// null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_reader_get(
    reader: *const SyncMapReader,
    key: u64,
    value_out: *mut u64,
) -> bool {
    // SAFETY: null or live reader per contract.
    let Some(v) = (unsafe { reader.as_ref() }).and_then(|r| r.0.get(key)) else {
        return false;
    };
    // SAFETY: `value_out` null or writable per contract.
    unsafe { put(value_out, v) };
    true
}

// ---- Ordered reads through a reader handle (#900) ----
//
// Each is one validated ordered search (`sync_nav`) under the reader's epoch
// pin, with the same retry budget and writer-excluding fallback as
// `expanse_sync_map_reader_get`: no lock on the common path, and a read that
// exhausts its retries answers under the writer mutex after quiescing writers.

/// Writes an ordered read's answer through the caller's out-pointers: true
/// with both written when an entry was found, false (nothing written) when
/// none was.
///
/// # Safety
///
/// `key_out` and `value_out` each null or writable.
#[inline]
unsafe fn put_entry(found: Option<(u64, u64)>, key_out: *mut u64, value_out: *mut u64) -> bool {
    let Some((k, v)) = found else {
        return false;
    };
    // SAFETY: out-pointers null or writable per this function's contract.
    unsafe {
        put(key_out, k);
        put(value_out, v);
    }
    true
}

/// Smallest entry, through a registered reader.
///
/// # Safety
///
/// `reader` must be null or live (its map still alive), used from the thread
/// that created it; `key_out` and `value_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_reader_first(
    reader: *const SyncMapReader,
    key_out: *mut u64,
    value_out: *mut u64,
) -> bool {
    // SAFETY: null or live reader per contract.
    let found = unsafe { reader.as_ref() }.and_then(|r| r.0.first());
    // SAFETY: out-pointers null or writable per contract.
    unsafe { put_entry(found, key_out, value_out) }
}

/// Largest entry, through a registered reader.
///
/// # Safety
///
/// Same contract as [`expanse_sync_map_reader_first`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_reader_last(
    reader: *const SyncMapReader,
    key_out: *mut u64,
    value_out: *mut u64,
) -> bool {
    // SAFETY: null or live reader per contract.
    let found = unsafe { reader.as_ref() }.and_then(|r| r.0.last());
    // SAFETY: out-pointers null or writable per contract.
    unsafe { put_entry(found, key_out, value_out) }
}

/// Smallest entry with key `>= key`, through a registered reader.
///
/// # Safety
///
/// Same contract as [`expanse_sync_map_reader_first`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_reader_next_at_or_after(
    reader: *const SyncMapReader,
    key: u64,
    key_out: *mut u64,
    value_out: *mut u64,
) -> bool {
    // SAFETY: null or live reader per contract.
    let found = unsafe { reader.as_ref() }.and_then(|r| r.0.next_at_or_after(key));
    // SAFETY: out-pointers null or writable per contract.
    unsafe { put_entry(found, key_out, value_out) }
}

/// Smallest entry with key `> key`, through a registered reader; false for
/// `key == u64::MAX`.
///
/// # Safety
///
/// Same contract as [`expanse_sync_map_reader_first`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_reader_next_after(
    reader: *const SyncMapReader,
    key: u64,
    key_out: *mut u64,
    value_out: *mut u64,
) -> bool {
    // SAFETY: null or live reader per contract.
    let found = unsafe { reader.as_ref() }.and_then(|r| r.0.next_after(key));
    // SAFETY: out-pointers null or writable per contract.
    unsafe { put_entry(found, key_out, value_out) }
}

/// Largest entry with key `<= key`, through a registered reader.
///
/// # Safety
///
/// Same contract as [`expanse_sync_map_reader_first`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_reader_prev_at_or_before(
    reader: *const SyncMapReader,
    key: u64,
    key_out: *mut u64,
    value_out: *mut u64,
) -> bool {
    // SAFETY: null or live reader per contract.
    let found = unsafe { reader.as_ref() }.and_then(|r| r.0.prev_at_or_before(key));
    // SAFETY: out-pointers null or writable per contract.
    unsafe { put_entry(found, key_out, value_out) }
}

/// Largest entry with key `< key`, through a registered reader; false for
/// `key == 0`.
///
/// # Safety
///
/// Same contract as [`expanse_sync_map_reader_first`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_map_reader_prev_before(
    reader: *const SyncMapReader,
    key: u64,
    key_out: *mut u64,
    value_out: *mut u64,
) -> bool {
    // SAFETY: null or live reader per contract.
    let found = unsafe { reader.as_ref() }.and_then(|r| r.0.prev_before(key));
    // SAFETY: out-pointers null or writable per contract.
    unsafe { put_entry(found, key_out, value_out) }
}

// ---- The shared string map (#1538) ----
//
// The `u64` map's conventions, with the key as a NUL-terminated C string read
// as `expanse_strmap_*` reads it (`cstr`): a NULL key is treated like a NULL
// handle, returning false or 0 and writing nothing. A C string cannot carry an
// interior NUL, so no key is refused for its content.

/// Creates an empty concurrent string map.
#[unsafe(no_mangle)]
pub extern "C" fn expanse_sync_strmap_new() -> *mut SyncExpanseStrMap {
    Box::into_raw(Box::new(SyncExpanseStrMap::new()))
}

/// Frees a concurrent string map. All readers must be freed first.
///
/// # Safety
///
/// `map` must come from `expanse_sync_strmap_new`, with no live readers and
/// no call on it in progress.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_strmap_free(map: *mut SyncExpanseStrMap) {
    if !map.is_null() {
        // SAFETY: handle per contract.
        drop(unsafe { Box::from_raw(map) });
    }
}

/// Stores `key -> value` (return convention as [`expanse_sync_map_insert`]).
///
/// # Safety
///
/// `map` must be null or a live handle; `key` null or a valid NUL-terminated
/// C string; `old_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_strmap_insert(
    map: *const SyncExpanseStrMap,
    key: *const c_char,
    value: u64,
    old_out: *mut u64,
) -> bool {
    // SAFETY: null or live handle and null or valid C string per contract.
    let (Some(m), Some(k)) = (unsafe { map.as_ref() }, unsafe { cstr(key) }) else {
        return false;
    };
    match m.insert(k, value) {
        // SAFETY: `old_out` null or writable per contract.
        Some(old) => unsafe {
            put(old_out, old);
            false
        },
        None => true,
    }
}

/// One-shot lookup (registers a throwaway reader; prefer a reader handle in
/// hot loops).
///
/// # Safety
///
/// `map` must be null or a live handle; `key` null or a valid NUL-terminated
/// C string; `value_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_strmap_get(
    map: *const SyncExpanseStrMap,
    key: *const c_char,
    value_out: *mut u64,
) -> bool {
    // SAFETY: null or live handle and null or valid C string per contract.
    let (Some(m), Some(k)) = (unsafe { map.as_ref() }, unsafe { cstr(key) }) else {
        return false;
    };
    let Some(v) = m.get(k) else {
        return false;
    };
    // SAFETY: `value_out` null or writable per contract.
    unsafe { put(value_out, v) };
    true
}

/// One-shot membership test (registers a throwaway reader; prefer a reader
/// handle in hot loops).
///
/// # Safety
///
/// `map` must be null or a live handle; `key` null or a valid NUL-terminated
/// C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_strmap_contains(
    map: *const SyncExpanseStrMap,
    key: *const c_char,
) -> bool {
    // SAFETY: null or live handle and null or valid C string per contract.
    let (Some(m), Some(k)) = (unsafe { map.as_ref() }, unsafe { cstr(key) }) else {
        return false;
    };
    m.contains_key(k)
}

/// Removes `key`, reporting its value; false if absent.
///
/// # Safety
///
/// `map` must be null or a live handle; `key` null or a valid NUL-terminated
/// C string; `old_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_strmap_remove(
    map: *const SyncExpanseStrMap,
    key: *const c_char,
    old_out: *mut u64,
) -> bool {
    // SAFETY: null or live handle and null or valid C string per contract.
    let (Some(m), Some(k)) = (unsafe { map.as_ref() }, unsafe { cstr(key) }) else {
        return false;
    };
    let Some(v) = m.remove(k) else {
        return false;
    };
    // SAFETY: `old_out` null or writable per contract.
    unsafe { put(old_out, v) };
    true
}

/// Number of keys.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_strmap_len(map: *const SyncExpanseStrMap) -> u64 {
    // SAFETY: null or live handle per contract.
    unsafe { map.as_ref() }.map_or(0, SyncExpanseStrMap::len)
}

/// Heap bytes used by the map, read with writers excluded
/// ([`SyncExpanseStrMap::mem_used`]); 0 for a null handle.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_strmap_mem_used(map: *const SyncExpanseStrMap) -> usize {
    // SAFETY: null or live handle per contract.
    unsafe { map.as_ref() }.map_or(0, SyncExpanseStrMap::mem_used)
}

/// Heap bytes the map holds from the global allocator: its tree's own share
/// plus the blocks its epoch collector holds for reuse or is waiting to
/// reclaim ([`SyncExpanseStrMap::mem_held`]); 0 for a null handle. Writers
/// wait for it.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_strmap_mem_held(map: *const SyncExpanseStrMap) -> usize {
    // SAFETY: null or live handle per contract.
    unsafe { map.as_ref() }.map_or(0, SyncExpanseStrMap::mem_held)
}

/// Returns the freed blocks the map's epoch collector keeps for reuse to the
/// global allocator and returns the bytes released
/// ([`SyncExpanseStrMap::shrink_to_fit`]); 0 for a null handle. Runs beside
/// readers and writers without excluding either.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_strmap_shrink_to_fit(map: *const SyncExpanseStrMap) -> usize {
    // SAFETY: null or live handle per contract.
    unsafe { map.as_ref() }.map_or(0, SyncExpanseStrMap::shrink_to_fit)
}

/// Registers a reader handle for this thread. Free it before the map.
///
/// # Safety
///
/// `map` must be null or a live handle that outlives the reader.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_strmap_reader_new(
    map: *const SyncExpanseStrMap,
) -> *mut SyncStrMapReader {
    // SAFETY: null or live handle per contract.
    let Some(m) = (unsafe { map.as_ref() }) else {
        return core::ptr::null_mut();
    };
    // SAFETY: lifetime extension only (see expanse_sync_set_reader_new): the
    // reader borrows `map`, and the header's contract requires freeing the
    // reader before its map.
    let reader: StrReader<'static> = unsafe { core::mem::transmute(m.reader()) };
    Box::into_raw(Box::new(SyncStrMapReader(reader)))
}

/// Frees a reader handle.
///
/// May be called from a thread other than the one that created the reader,
/// once no call on it is in progress (`include/expanse.h`).
///
/// # Safety
///
/// `reader` must come from `expanse_sync_strmap_reader_new`, unused after,
/// with no call on it in progress on any thread, and its map still alive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_strmap_reader_free(reader: *mut SyncStrMapReader) {
    if !reader.is_null() {
        // SAFETY: handle per contract.
        drop(unsafe { Box::from_raw(reader) });
    }
}

/// Optimistic lookup through a registered reader.
///
/// # Safety
///
/// `reader` must be null or live (its map still alive); `key` null or a
/// valid NUL-terminated C string; `value_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_strmap_reader_get(
    reader: *const SyncStrMapReader,
    key: *const c_char,
    value_out: *mut u64,
) -> bool {
    // SAFETY: null or live reader and null or valid C string per contract.
    let (Some(r), Some(k)) = (unsafe { reader.as_ref() }, unsafe { cstr(key) }) else {
        return false;
    };
    let Some(v) = r.0.get(k) else {
        return false;
    };
    // SAFETY: `value_out` null or writable per contract.
    unsafe { put(value_out, v) };
    true
}

/// Optimistic membership test through a registered reader.
///
/// # Safety
///
/// `reader` must be null or live (its map still alive); `key` null or a
/// valid NUL-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_strmap_reader_contains(
    reader: *const SyncStrMapReader,
    key: *const c_char,
) -> bool {
    // SAFETY: null or live reader and null or valid C string per contract.
    let (Some(r), Some(k)) = (unsafe { reader.as_ref() }, unsafe { cstr(key) }) else {
        return false;
    };
    r.0.contains(k)
}

// ---- Ordered reads through a string-map reader handle (#1538) ----
//
// Each is one optimistic ordered read of `StrReader` (#1143): validated, with
// the retry budget and writer-excluding fallback of `_reader_get`. The key
// comes back as an owned `Vec`, copied out under validation, and reaches the
// caller's buffer under the `expanse_strmap_*_ex` contract.

/// Writes an ordered read's answer under the `expanse_strmap_*_ex` contract:
/// `NotFound` with nothing written; `BufferTooSmall` with only
/// `*required_len` written (key length plus the NUL); `Ok` with the key, its
/// NUL, `*required_len` and `*value_out` written.
///
/// # Safety
///
/// `key_out` null or writable for `buf_len` bytes; `required_len` and
/// `value_out` null or writable.
unsafe fn put_str_entry(
    found: Option<(Vec<u8>, u64)>,
    key_out: *mut c_char,
    buf_len: usize,
    required_len: *mut usize,
    value_out: *mut u64,
) -> ExpanseStrNavStatus {
    let Some((key, value)) = found else {
        return ExpanseStrNavStatus::NotFound;
    };
    let needed = key.len() + 1;
    // SAFETY: `required_len` null or writable per contract.
    unsafe { put(required_len, needed) };
    if key_out.is_null() || buf_len < needed {
        return ExpanseStrNavStatus::BufferTooSmall;
    }
    // SAFETY: `key_out` is non-null and writable for `buf_len >= needed`
    // bytes; `key` is a separate allocation, so the ranges do not overlap;
    // `value_out` null or writable per contract.
    unsafe {
        core::ptr::copy_nonoverlapping(key.as_ptr().cast::<c_char>(), key_out, key.len());
        *key_out.add(key.len()) = 0;
        put(value_out, value);
    }
    ExpanseStrNavStatus::Ok
}

/// Smallest entry, through a registered string-map reader.
///
/// # Safety
///
/// `reader` must be null or live (its map still alive); `key_out` null or
/// writable for `buf_len` bytes; `required_len` and `value_out` null or
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_strmap_reader_first(
    reader: *const SyncStrMapReader,
    key_out: *mut c_char,
    buf_len: usize,
    required_len: *mut usize,
    value_out: *mut u64,
) -> ExpanseStrNavStatus {
    // SAFETY: null or live reader per contract.
    let found = unsafe { reader.as_ref() }.and_then(|r| r.0.first());
    // SAFETY: out-pointers forwarded per contract.
    unsafe { put_str_entry(found, key_out, buf_len, required_len, value_out) }
}

/// Largest entry, through a registered string-map reader.
///
/// # Safety
///
/// Same contract as [`expanse_sync_strmap_reader_first`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_strmap_reader_last(
    reader: *const SyncStrMapReader,
    key_out: *mut c_char,
    buf_len: usize,
    required_len: *mut usize,
    value_out: *mut u64,
) -> ExpanseStrNavStatus {
    // SAFETY: null or live reader per contract.
    let found = unsafe { reader.as_ref() }.and_then(|r| r.0.last());
    // SAFETY: out-pointers forwarded per contract.
    unsafe { put_str_entry(found, key_out, buf_len, required_len, value_out) }
}

/// Generates a by-key ordered read through a registered string-map reader.
macro_rules! sync_strmap_reader_nav {
    ($name:ident, $method:ident, $doc:literal) => {
        #[doc = $doc]
        ///
        /// Status and outputs as [`expanse_sync_strmap_reader_first`]; a NULL
        /// reader or NULL `key` returns `NotFound` and writes nothing.
        ///
        /// # Safety
        ///
        /// `reader` must be null or live (its map still alive); `key` null or
        /// a valid NUL-terminated C string; `key_out` null or writable for
        /// `buf_len` bytes; `required_len` and `value_out` null or writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            reader: *const SyncStrMapReader,
            key: *const c_char,
            key_out: *mut c_char,
            buf_len: usize,
            required_len: *mut usize,
            value_out: *mut u64,
        ) -> ExpanseStrNavStatus {
            // SAFETY: null or live reader and null or valid C string per
            // contract.
            let (Some(r), Some(k)) = (unsafe { reader.as_ref() }, unsafe { cstr(key) }) else {
                return ExpanseStrNavStatus::NotFound;
            };
            // SAFETY: out-pointers forwarded per contract.
            unsafe { put_str_entry(r.0.$method(k), key_out, buf_len, required_len, value_out) }
        }
    };
}

sync_strmap_reader_nav!(
    expanse_sync_strmap_reader_next_at_or_after,
    next_at_or_after,
    "Smallest entry with key `>= key`, through a registered string-map reader."
);
sync_strmap_reader_nav!(
    expanse_sync_strmap_reader_next_after,
    next_after,
    "Smallest entry with key `> key`, through a registered string-map reader."
);
sync_strmap_reader_nav!(
    expanse_sync_strmap_reader_prev_at_or_before,
    prev_at_or_before,
    "Largest entry with key `<= key`, through a registered string-map reader."
);
sync_strmap_reader_nav!(
    expanse_sync_strmap_reader_prev_before,
    prev_before,
    "Largest entry with key `< key`, through a registered string-map reader; \
     `NotFound` for the empty key."
);
