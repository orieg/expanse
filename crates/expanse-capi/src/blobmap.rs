//! The modern `expanse_blob_map_*` C API exports.
//!
//! Provides C ABI bindings for polymorphic large-value maps with inline payload
//! packing (0..=7 bytes), arena slab backing, hot metadata filtering, and
//! in-place garbage collection.

use core::ffi::c_void;
use expanse_trie::blobmap::ExpanseBlobMap;

/// C representation of a retrieved blob payload view.
///
/// `ptr` borrows directly into the map's inline-slot or arena memory and stays
/// valid only until the next structural mutation of that map — the classic
/// JudyL value-slot contract (mirrors `ExpanseMap::get_value_slot`). Any
/// `expanse_blob_map_insert`/`_remove`/`_clear`/`_compact`/`_free` invalidates
/// every previously returned view's `ptr`; reading through it afterwards is
/// undefined. Views handed to a scan callback are valid only for that call.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ExpanseBlobView {
    /// Pointer to the byte payload data.
    pub ptr: *const u8,
    /// Length of payload in bytes.
    pub len: usize,
    /// 32-bit hot metadata word.
    pub hot_meta: u32,
    /// `true` if payload is stored inline in the 64-bit value slot.
    pub is_inline: bool,
}

/// Predicate callback function type evaluated against 32-bit hot metadata.
pub type ExpansePredicateFn =
    unsafe extern "C" fn(key: u64, hot_meta: u32, user_ctx: *mut c_void) -> bool;

/// Scan consumer callback function type receiving zero-copy blob views.
pub type ExpanseScanCbFn =
    unsafe extern "C" fn(key: u64, view: ExpanseBlobView, user_ctx: *mut c_void) -> bool;

/// Creates a new empty `ExpanseBlobMap`. If `chunk_size == 0`, the default
/// 2 MiB chunk capacity is used.
#[unsafe(no_mangle)]
pub extern "C" fn expanse_blob_map_new(chunk_size: usize) -> *mut ExpanseBlobMap {
    let map = if chunk_size == 0 {
        ExpanseBlobMap::new()
    } else {
        ExpanseBlobMap::with_chunk_size(chunk_size)
    };
    Box::into_raw(Box::new(map))
}

/// Frees an `ExpanseBlobMap` and all associated arena memory.
///
/// # Safety
///
/// `map` must be null or a live handle returned by `expanse_blob_map_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_blob_map_free(map: *mut ExpanseBlobMap) {
    if !map.is_null() {
        // SAFETY: map was allocated with Box::into_raw in expanse_blob_map_new.
        drop(unsafe { Box::from_raw(map) });
    }
}

/// Inserts a key-blob pair with 32-bit hot metadata. Returns `true` on success.
///
/// # Safety
///
/// `map` must be a valid non-null handle. `data` must point to at least `len` readable bytes
/// (or be null if `len == 0`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_blob_map_insert(
    map: *mut ExpanseBlobMap,
    key: u64,
    data: *const u8,
    len: usize,
    hot_meta: u32,
) -> bool {
    // SAFETY: map is null or points to a live ExpanseBlobMap per caller contract.
    let Some(map_ref) = (unsafe { map.as_mut() }) else {
        return false;
    };
    let slice = if data.is_null() {
        if len == 0 {
            &[]
        } else {
            return false;
        }
    } else {
        // SAFETY: data is non-null and valid for len bytes per caller contract.
        unsafe { core::slice::from_raw_parts(data, len) }
    };
    map_ref.insert(key, slice, hot_meta).is_ok()
}

/// Removes a key from the map. Returns `true` if the key was present.
///
/// # Safety
///
/// `map` must be a valid non-null handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_blob_map_remove(map: *mut ExpanseBlobMap, key: u64) -> bool {
    // SAFETY: map is null or points to a live ExpanseBlobMap per caller contract.
    let Some(map_ref) = (unsafe { map.as_mut() }) else {
        return false;
    };
    map_ref.remove(key)
}

/// Looks up a key, writing the zero-copy view to `out_view` if present.
/// Returns `true` if found.
///
/// The written [`ExpanseBlobView::ptr`] borrows into the map and is valid only
/// until the next structural mutation of `map` (any
/// insert/remove/clear/compact/free); using it after that is undefined. Copy
/// the bytes out first if they must outlive the next mutation.
///
/// # Safety
///
/// `map` must be a valid handle. `out_view` must be non-null and writable (or null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_blob_map_get(
    map: *const ExpanseBlobMap,
    key: u64,
    out_view: *mut ExpanseBlobView,
) -> bool {
    // SAFETY: map is null or points to a live ExpanseBlobMap per caller contract.
    let Some(map_ref) = (unsafe { map.as_ref() }) else {
        return false;
    };
    if let Some((view, meta)) = map_ref.get(key) {
        if !out_view.is_null() {
            let is_inline = view.is_inline();
            let bytes = view.as_bytes();
            // SAFETY: out_view is non-null and writable per contract.
            unsafe {
                *out_view = ExpanseBlobView {
                    ptr: bytes.as_ptr(),
                    len: bytes.len(),
                    hot_meta: meta,
                    is_inline,
                };
            }
        }
        true
    } else {
        false
    }
}

/// Executes a range scan with optional hot metadata predicate filtering.
/// Returns the number of entries passed to the callback.
///
/// Each [`ExpanseBlobView`] passed to `callback` borrows into the map and is
/// valid only for the duration of that callback invocation; do not retain its
/// `ptr`, and do not mutate the map from within the callback.
///
/// # Safety
///
/// `map` must be a valid handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_blob_map_scan_filtered(
    map: *const ExpanseBlobMap,
    start_key: u64,
    end_key: u64,
    predicate: Option<ExpansePredicateFn>,
    callback: Option<ExpanseScanCbFn>,
    user_ctx: *mut c_void,
) -> usize {
    // SAFETY: map is null or points to a live ExpanseBlobMap per caller contract.
    let Some(map_ref) = (unsafe { map.as_ref() }) else {
        return 0;
    };
    if start_key > end_key {
        return 0;
    }

    let mut count = 0usize;
    map_ref.scan_filtered(
        start_key..=end_key,
        |key, meta| {
            if let Some(pred) = predicate {
                // SAFETY: caller supplied predicate function pointer and user_ctx.
                unsafe { pred(key, meta, user_ctx) }
            } else {
                true
            }
        },
        |key, view, meta| {
            count += 1;
            if let Some(cb) = callback {
                let c_view = ExpanseBlobView {
                    ptr: view.as_bytes().as_ptr(),
                    len: view.len(),
                    hot_meta: meta,
                    is_inline: view.is_inline(),
                };
                // SAFETY: caller supplied callback function pointer and user_ctx.
                unsafe { cb(key, c_view, user_ctx) }
            } else {
                true
            }
        },
    );
    count
}

/// Runs in-place arena garbage collection and compaction. Returns `true` on success.
///
/// # Safety
///
/// `map` must be a valid non-null handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_blob_map_compact(map: *mut ExpanseBlobMap) -> bool {
    // SAFETY: map is null or points to a live ExpanseBlobMap per caller contract.
    let Some(map_ref) = (unsafe { map.as_mut() }) else {
        return false;
    };
    map_ref.compact().is_ok()
}

/// Returns the number of entries in the map.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_blob_map_len(map: *const ExpanseBlobMap) -> u64 {
    // SAFETY: map is null or points to a live ExpanseBlobMap per caller contract.
    unsafe { map.as_ref() }.map_or(0, ExpanseBlobMap::len)
}

/// Returns the total heap bytes used by the map and arena.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_blob_map_mem_used(map: *const ExpanseBlobMap) -> usize {
    // SAFETY: map is null or points to a live ExpanseBlobMap per caller contract.
    unsafe { map.as_ref() }.map_or(0, ExpanseBlobMap::mem_used)
}

/// Clears all entries and frees all arena slabs.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_blob_map_clear(map: *mut ExpanseBlobMap) {
    // SAFETY: map is null or points to a live ExpanseBlobMap per caller contract.
    if let Some(map_ref) = unsafe { map.as_mut() } {
        map_ref.clear();
    }
}

/// Returns `true` if `key` is present in the map.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_blob_map_contains_key(
    map: *const ExpanseBlobMap,
    key: u64,
) -> bool {
    // SAFETY: map is null or points to a live ExpanseBlobMap per caller contract.
    unsafe { map.as_ref() }.is_some_and(|m| m.contains_key(key))
}
