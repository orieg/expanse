//! The concurrent half of the modern `expanse_*` C API.
//!
//! `expanse_trie::sync` is gated on `all(target_pointer_width = "64",
//! feature = "std")` — the one-writer/many-reader containers need
//! `std::sync` — so these entry points are absent from every `no_std`
//! build, 64-bit ones included. See the surface matrix in
//! `docs/COMPAT.md`.

use super::super::{bytes, cstr, put};
use super::{ExpanseOrderedBytesNavStatus, ExpanseStrNavStatus};
use core::ffi::{c_char, c_void};
use core::ptr::NonNull;
use expanse_trie::ordered_bytesmap::{EscapeDecodeError, ExpanseOrderedBytesMap};
use expanse_trie::sync::{
    BytesReader, MapReader, OrderedBytesReader, SetReader, StrReader, SyncExpanseBytesMap,
    SyncExpanseMap, SyncExpanseOrderedBytesMap, SyncExpanseSet, SyncExpanseStrMap,
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

/// A registered reader of a [`SyncExpanseBytesMap`]. See [`SyncSetReader`].
pub struct SyncBytesMapReader(BytesReader<'static>);

/// A registered reader of a [`SyncExpanseOrderedBytesMap`]. See
/// [`SyncSetReader`].
pub struct SyncOrderedBytesMapReader(OrderedBytesReader<'static>);

// `include/expanse.h` documents that a reader handle may be freed from a thread
// other than the one that created it. That holds because the handle types are
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
    assert_send::<SyncBytesMapReader>();
    assert_send::<SyncOrderedBytesMapReader>();
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

// ---- The shared bytes and ordered-bytes maps (#1538) ----
//
// The `u64` map's conventions, with the key as `(pointer, length)` read as
// `expanse_bytesmap_*` and `expanse_ordered_bytesmap_*` read it (`bytes`):
// opaque bytes, embedded NULs ordinary, NULL with length 0 the empty key. A
// NULL key with a non-zero length, or a length above `isize::MAX`, is treated
// like a NULL handle, returning false or 0 and writing nothing.

/// Generates the fourteen handle, point-operation, memory and reader entry
/// points of one shared byte-keyed map.
macro_rules! sync_bytes_family {
    (
        map: $Map:ty,
        reader: $Reader:ident($RustReader:ident),
        what: $what:literal,
        new: $new:ident,
        free: $free:ident,
        insert: $insert:ident,
        get: $get:ident,
        contains: $contains:ident,
        remove: $remove:ident,
        len: $len:ident,
        mem_used: $mem_used:ident,
        mem_held: $mem_held:ident,
        shrink_to_fit: $shrink:ident,
        reader_new: $reader_new:ident,
        reader_free: $reader_free:ident,
        reader_get: $reader_get:ident,
        reader_contains: $reader_contains:ident $(,)?
    ) => {
        #[doc = concat!("Creates an empty concurrent ", $what, ".")]
        #[unsafe(no_mangle)]
        pub extern "C" fn $new() -> *mut $Map {
            Box::into_raw(Box::new(<$Map>::new()))
        }

        #[doc = concat!("Frees a concurrent ", $what, ". All readers must be freed first.")]
        ///
        /// # Safety
        ///
        #[doc = concat!("`map` must come from `", stringify!($new), "`, with no live readers")]
        /// and no call on it in progress.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $free(map: *mut $Map) {
            if !map.is_null() {
                // SAFETY: handle per contract.
                drop(unsafe { Box::from_raw(map) });
            }
        }

        /// Stores `key -> value` (return convention as [`expanse_sync_map_insert`]).
        ///
        /// # Safety
        ///
        /// `map` must be null or a live handle; `key` readable for `len` bytes
        /// (null only when `len == 0`); `old_out` null or writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $insert(
            map: *const $Map,
            key: *const c_void,
            len: usize,
            value: u64,
            old_out: *mut u64,
        ) -> bool {
            // SAFETY: null or live handle and a key readable for `len` bytes
            // per contract.
            let (Some(m), Some(k)) = (unsafe { map.as_ref() }, unsafe { bytes(key, len) }) else {
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

        /// One-shot lookup (registers a throwaway reader; prefer a reader
        /// handle in hot loops).
        ///
        /// # Safety
        ///
        /// `map` must be null or a live handle; `key` readable for `len` bytes
        /// (null only when `len == 0`); `value_out` null or writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $get(
            map: *const $Map,
            key: *const c_void,
            len: usize,
            value_out: *mut u64,
        ) -> bool {
            // SAFETY: null or live handle and a key readable for `len` bytes
            // per contract.
            let (Some(m), Some(k)) = (unsafe { map.as_ref() }, unsafe { bytes(key, len) }) else {
                return false;
            };
            let Some(v) = m.get(k) else {
                return false;
            };
            // SAFETY: `value_out` null or writable per contract.
            unsafe { put(value_out, v) };
            true
        }

        /// One-shot membership test (registers a throwaway reader; prefer a
        /// reader handle in hot loops).
        ///
        /// # Safety
        ///
        /// `map` must be null or a live handle; `key` readable for `len` bytes
        /// (null only when `len == 0`).
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $contains(
            map: *const $Map,
            key: *const c_void,
            len: usize,
        ) -> bool {
            // SAFETY: null or live handle and a key readable for `len` bytes
            // per contract.
            let (Some(m), Some(k)) = (unsafe { map.as_ref() }, unsafe { bytes(key, len) }) else {
                return false;
            };
            m.contains_key(k)
        }

        /// Removes `key`, reporting its value; false if absent.
        ///
        /// # Safety
        ///
        /// `map` must be null or a live handle; `key` readable for `len` bytes
        /// (null only when `len == 0`); `old_out` null or writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $remove(
            map: *const $Map,
            key: *const c_void,
            len: usize,
            old_out: *mut u64,
        ) -> bool {
            // SAFETY: null or live handle and a key readable for `len` bytes
            // per contract.
            let (Some(m), Some(k)) = (unsafe { map.as_ref() }, unsafe { bytes(key, len) }) else {
                return false;
            };
            let Some(v) = m.remove(k) else {
                return false;
            };
            // SAFETY: `old_out` null or writable per contract.
            unsafe { put(old_out, v) };
            true
        }

        /// Number of keys; 0 for a null handle.
        ///
        /// # Safety
        ///
        /// `map` must be null or a live handle.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $len(map: *const $Map) -> u64 {
            // SAFETY: null or live handle per contract.
            unsafe { map.as_ref() }.map_or(0, <$Map>::len)
        }

        /// Heap bytes used by the map, read with writers excluded; 0 for a
        /// null handle.
        ///
        /// # Safety
        ///
        /// `map` must be null or a live handle.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $mem_used(map: *const $Map) -> usize {
            // SAFETY: null or live handle per contract.
            unsafe { map.as_ref() }.map_or(0, <$Map>::mem_used)
        }

        /// Heap bytes the map holds from the global allocator: its tree's own
        /// share plus the blocks its epoch collector holds for reuse or is
        /// waiting to reclaim; 0 for a null handle. Writers wait for it.
        ///
        /// # Safety
        ///
        /// `map` must be null or a live handle.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $mem_held(map: *const $Map) -> usize {
            // SAFETY: null or live handle per contract.
            unsafe { map.as_ref() }.map_or(0, <$Map>::mem_held)
        }

        /// Returns the freed blocks the map's epoch collector keeps for reuse
        /// to the global allocator and returns the bytes released; 0 for a
        /// null handle. Runs beside readers and writers without excluding
        /// either.
        ///
        /// # Safety
        ///
        /// `map` must be null or a live handle.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $shrink(map: *const $Map) -> usize {
            // SAFETY: null or live handle per contract.
            unsafe { map.as_ref() }.map_or(0, <$Map>::shrink_to_fit)
        }

        /// Registers a reader handle for this thread. Free it before the map.
        ///
        /// # Safety
        ///
        /// `map` must be null or a live handle that outlives the reader.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $reader_new(map: *const $Map) -> *mut $Reader {
            // SAFETY: null or live handle per contract.
            let Some(m) = (unsafe { map.as_ref() }) else {
                return core::ptr::null_mut();
            };
            // SAFETY: lifetime extension only (see expanse_sync_set_reader_new):
            // the reader borrows `map`, and the header's contract requires
            // freeing the reader before its map.
            let reader: $RustReader<'static> = unsafe { core::mem::transmute(m.reader()) };
            Box::into_raw(Box::new($Reader(reader)))
        }

        /// Frees a reader handle.
        ///
        /// May be called from a thread other than the one that created the
        /// reader, once no call on it is in progress (`include/expanse.h`).
        ///
        /// # Safety
        ///
        #[doc = concat!("`reader` must come from `", stringify!($reader_new), "`, unused after,")]
        /// with no call on it in progress on any thread, and its map still
        /// alive.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $reader_free(reader: *mut $Reader) {
            if !reader.is_null() {
                // SAFETY: handle per contract.
                drop(unsafe { Box::from_raw(reader) });
            }
        }

        /// Optimistic lookup through a registered reader.
        ///
        /// # Safety
        ///
        /// `reader` must be null or live (its map still alive); `key` readable
        /// for `len` bytes (null only when `len == 0`); `value_out` null or
        /// writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $reader_get(
            reader: *const $Reader,
            key: *const c_void,
            len: usize,
            value_out: *mut u64,
        ) -> bool {
            // SAFETY: null or live reader and a key readable for `len` bytes
            // per contract.
            let (Some(r), Some(k)) = (unsafe { reader.as_ref() }, unsafe { bytes(key, len) })
            else {
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
        /// `reader` must be null or live (its map still alive); `key` readable
        /// for `len` bytes (null only when `len == 0`).
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $reader_contains(
            reader: *const $Reader,
            key: *const c_void,
            len: usize,
        ) -> bool {
            // SAFETY: null or live reader and a key readable for `len` bytes
            // per contract.
            let (Some(r), Some(k)) = (unsafe { reader.as_ref() }, unsafe { bytes(key, len) })
            else {
                return false;
            };
            r.0.contains(k)
        }
    };
}

sync_bytes_family!(
    map: SyncExpanseBytesMap,
    reader: SyncBytesMapReader(BytesReader),
    what: "byte-string map",
    new: expanse_sync_bytesmap_new,
    free: expanse_sync_bytesmap_free,
    insert: expanse_sync_bytesmap_insert,
    get: expanse_sync_bytesmap_get,
    contains: expanse_sync_bytesmap_contains,
    remove: expanse_sync_bytesmap_remove,
    len: expanse_sync_bytesmap_len,
    mem_used: expanse_sync_bytesmap_mem_used,
    mem_held: expanse_sync_bytesmap_mem_held,
    shrink_to_fit: expanse_sync_bytesmap_shrink_to_fit,
    reader_new: expanse_sync_bytesmap_reader_new,
    reader_free: expanse_sync_bytesmap_reader_free,
    reader_get: expanse_sync_bytesmap_reader_get,
    reader_contains: expanse_sync_bytesmap_reader_contains,
);

sync_bytes_family!(
    map: SyncExpanseOrderedBytesMap,
    reader: SyncOrderedBytesMapReader(OrderedBytesReader),
    what: "ordered byte-string map",
    new: expanse_sync_ordered_bytesmap_new,
    free: expanse_sync_ordered_bytesmap_free,
    insert: expanse_sync_ordered_bytesmap_insert,
    get: expanse_sync_ordered_bytesmap_get,
    contains: expanse_sync_ordered_bytesmap_contains,
    remove: expanse_sync_ordered_bytesmap_remove,
    len: expanse_sync_ordered_bytesmap_len,
    mem_used: expanse_sync_ordered_bytesmap_mem_used,
    mem_held: expanse_sync_ordered_bytesmap_mem_held,
    shrink_to_fit: expanse_sync_ordered_bytesmap_shrink_to_fit,
    reader_new: expanse_sync_ordered_bytesmap_reader_new,
    reader_free: expanse_sync_ordered_bytesmap_reader_free,
    reader_get: expanse_sync_ordered_bytesmap_reader_get,
    reader_contains: expanse_sync_ordered_bytesmap_reader_contains,
);

// ---- Ordered reads on the shared ordered-bytes map's handle (#1538) ----
//
// `SyncExpanseOrderedBytesMap` has no optimistic ordered read: its navigation
// runs under `with_locked`, with every writer excluded, so these calls block
// writers for their duration and sit on the container handle, not the reader.
// Each call is ONE `with_locked` hold, in which the single-threaded map's
// `*_decode_into` form runs into an empty buffer for the length and then, if
// the caller's buffer holds the key, into that buffer. Both runs see the same
// map, so the length reported and the key written agree, and a buffer too
// small for the key is never written: `escape_decode_into` writes the prefix
// that fits before it reports `BufferTooSmall`, so one decode straight into
// the caller's buffer could not keep that promise for a key holding a 0x00 or
// 0x01 byte.

/// The `*_decode_into` form one ordered read runs, given the search key.
type DecodeInto = fn(
    &ExpanseOrderedBytesMap,
    &[u8],
    &mut [u8],
) -> Result<Option<(usize, NonNull<u64>)>, EscapeDecodeError>;

/// One ordered read of the ordered-bytes map under one writer-excluding
/// hold, with its answer written under the `expanse_ordered_bytesmap_*`
/// navigation contract: `NotFound` with nothing written; `BufferTooSmall`
/// with only `*required_len` written; `Ok` with the key in
/// `key_out[..*required_len]` (nothing for the empty key), `*required_len`
/// and `*value_out` written.
///
/// `search` is an owned copy of the caller's search key (empty for `first`
/// and `last`, which ignore it): the caller may pass one buffer as both the
/// search key and `key_out`, and the copy keeps every view of that buffer out
/// of the hold that writes it.
///
/// # Safety
///
/// `key_out` null or writable for `buf_len` bytes; `required_len` and
/// `value_out` null or writable.
unsafe fn sync_ordered_bytes_nav(
    map: &SyncExpanseOrderedBytesMap,
    search: &[u8],
    read: DecodeInto,
    key_out: *mut c_void,
    buf_len: usize,
    required_len: *mut usize,
    value_out: *mut u64,
) -> ExpanseOrderedBytesNavStatus {
    map.with_locked(|m| {
        let probe = read(m, search, &mut []);
        // Every key in the map went in through its escape encoding, so its
        // decode fails only for want of room; any other error is a defect.
        debug_assert!(
            matches!(probe, Ok(_) | Err(EscapeDecodeError::BufferTooSmall { .. })),
            "a stored ordered-bytes key failed to decode: {probe:?}"
        );
        let (needed, slot) = match probe {
            Ok(Some((0, slot))) => (0, Some(slot)),
            Err(EscapeDecodeError::BufferTooSmall { required, .. }) => (required, None),
            _ => return ExpanseOrderedBytesNavStatus::NotFound,
        };
        if needed > 0 && (key_out.is_null() || buf_len < needed) {
            // SAFETY: `required_len` null or writable per contract.
            unsafe { put(required_len, needed) };
            return ExpanseOrderedBytesNavStatus::BufferTooSmall;
        }
        let slot = match slot {
            Some(slot) => slot,
            None => {
                // SAFETY: `key_out` is non-null and writable for
                // `buf_len >= needed > 0` bytes, and nothing else in this
                // call views it: the search key is an owned copy.
                let dest =
                    unsafe { core::slice::from_raw_parts_mut(key_out.cast::<u8>(), buf_len) };
                let decoded = read(m, search, dest);
                debug_assert!(
                    matches!(decoded, Ok(Some((len, _))) if len == needed),
                    "the decode disagreed with its probe under one hold: {decoded:?}"
                );
                let Ok(Some((_, slot))) = decoded else {
                    return ExpanseOrderedBytesNavStatus::NotFound;
                };
                slot
            }
        };
        // SAFETY: `slot` is the value word of a leaf of the map `with_locked`
        // lends this closure with every writer excluded, so it is live,
        // aligned and initialized; the out-pointers are null or writable per
        // contract.
        unsafe {
            put(value_out, slot.as_ptr().read());
            put(required_len, needed);
        }
        ExpanseOrderedBytesNavStatus::Ok
    })
}

/// Smallest entry of a shared ordered-bytes map, read with writers excluded.
///
/// On `Ok` the decoded key is in `key_out[..*required_len]`, and
/// `*required_len` and `*value_out` are written; on `BufferTooSmall` only
/// `*required_len` (NULL with 0 is a length probe); on `NotFound`, which
/// includes a NULL map, nothing.
///
/// # Safety
///
/// `map` must be null or a live handle; `key_out` null or writable for
/// `buf_len` bytes; `required_len` and `value_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_ordered_bytesmap_first(
    map: *const SyncExpanseOrderedBytesMap,
    key_out: *mut c_void,
    buf_len: usize,
    required_len: *mut usize,
    value_out: *mut u64,
) -> ExpanseOrderedBytesNavStatus {
    // SAFETY: null or live handle per contract.
    let Some(m) = (unsafe { map.as_ref() }) else {
        return ExpanseOrderedBytesNavStatus::NotFound;
    };
    // SAFETY: out-pointers forwarded per contract.
    unsafe {
        sync_ordered_bytes_nav(
            m,
            &[],
            |m, _, buf| m.first_decode_into(buf),
            key_out,
            buf_len,
            required_len,
            value_out,
        )
    }
}

/// Largest entry of a shared ordered-bytes map, read with writers excluded.
///
/// # Safety
///
/// Same contract as [`expanse_sync_ordered_bytesmap_first`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_ordered_bytesmap_last(
    map: *const SyncExpanseOrderedBytesMap,
    key_out: *mut c_void,
    buf_len: usize,
    required_len: *mut usize,
    value_out: *mut u64,
) -> ExpanseOrderedBytesNavStatus {
    // SAFETY: null or live handle per contract.
    let Some(m) = (unsafe { map.as_ref() }) else {
        return ExpanseOrderedBytesNavStatus::NotFound;
    };
    // SAFETY: out-pointers forwarded per contract.
    unsafe {
        sync_ordered_bytes_nav(
            m,
            &[],
            |m, _, buf| m.last_decode_into(buf),
            key_out,
            buf_len,
            required_len,
            value_out,
        )
    }
}

/// Generates a by-key ordered read on a shared ordered-bytes map's handle.
macro_rules! sync_ordered_bytesmap_nav {
    ($name:ident, $method:ident, $doc:literal) => {
        #[doc = $doc]
        ///
        /// Read with writers excluded. Status and outputs as
        /// [`expanse_sync_ordered_bytesmap_first`]; a NULL map, or a NULL
        /// `key` with a non-zero `len`, returns `NotFound` and writes nothing.
        /// `key` and `key_out` may be the same buffer.
        ///
        /// # Safety
        ///
        /// `map` must be null or a live handle; `key` readable for `len` bytes
        /// (null only when `len == 0`); `key_out` null or writable for
        /// `buf_len` bytes; `required_len` and `value_out` null or writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            map: *const SyncExpanseOrderedBytesMap,
            key: *const c_void,
            len: usize,
            key_out: *mut c_void,
            buf_len: usize,
            required_len: *mut usize,
            value_out: *mut u64,
        ) -> ExpanseOrderedBytesNavStatus {
            // SAFETY: null or live handle per contract.
            let Some(m) = (unsafe { map.as_ref() }) else {
                return ExpanseOrderedBytesNavStatus::NotFound;
            };
            // SAFETY: `key` readable for `len` bytes per contract. The copy
            // ends every view of the caller's buffer here, before `key_out`,
            // which may be the same buffer, is written.
            let Some(search) = (unsafe { bytes(key, len) }).map(<[u8]>::to_vec) else {
                return ExpanseOrderedBytesNavStatus::NotFound;
            };
            // SAFETY: out-pointers forwarded per contract.
            unsafe {
                sync_ordered_bytes_nav(
                    m,
                    &search,
                    |m, k, buf| m.$method(k, buf),
                    key_out,
                    buf_len,
                    required_len,
                    value_out,
                )
            }
        }
    };
}

sync_ordered_bytesmap_nav!(
    expanse_sync_ordered_bytesmap_next_at_or_after,
    next_at_or_after_decode_into,
    "Smallest entry with key `>= key` of a shared ordered-bytes map."
);
sync_ordered_bytesmap_nav!(
    expanse_sync_ordered_bytesmap_next_after,
    next_after_decode_into,
    "Smallest entry with key `> key` of a shared ordered-bytes map."
);
sync_ordered_bytesmap_nav!(
    expanse_sync_ordered_bytesmap_prev_at_or_before,
    prev_at_or_before_decode_into,
    "Largest entry with key `<= key` of a shared ordered-bytes map."
);
sync_ordered_bytesmap_nav!(
    expanse_sync_ordered_bytesmap_prev_before,
    prev_before_decode_into,
    "Largest entry with key `< key` of a shared ordered-bytes map; `NotFound` \
     for the empty key."
);
