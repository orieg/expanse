//! The concurrent half of the modern `expanse_*` C API.
//!
//! `expanse_trie::sync` is gated on `all(target_pointer_width = "64",
//! feature = "std")` — the one-writer/many-reader containers need
//! `std::sync` — so these entry points are absent from every `no_std`
//! build, 64-bit ones included. See the surface matrix in
//! `docs/COMPAT.md`.

use super::super::put;
use expanse_trie::sync::{MapReader, SetReader, SyncExpanseMap, SyncExpanseSet};

// ---------------------------------------------------------------------

/// A registered reader of a [`SyncExpanseSet`].
///
/// The Rust handle borrows its container; the C handle stores that
/// borrow as `'static`, which the header's contract upholds (a reader
/// must be freed before its container).
pub struct SyncSetReader(SetReader<'static>);

/// A registered reader of a [`SyncExpanseMap`]. See [`SyncSetReader`].
pub struct SyncMapReader(MapReader<'static>);

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
/// # Safety
///
/// `reader` must come from `expanse_sync_set_reader_new`, unused after.
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
/// # Safety
///
/// `reader` must come from `expanse_sync_map_reader_new`, unused after.
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
