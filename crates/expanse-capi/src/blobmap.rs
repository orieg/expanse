//! The modern `expanse_blob_map_*` C API exports.
//!
//! Provides C ABI bindings for polymorphic large-value maps with inline payload
//! packing (0..=7 bytes), arena slab backing, hot metadata filtering, and
//! in-place garbage collection.

#[cfg(not(feature = "std"))]
use crate::core_alloc::boxed::Box;
use core::ffi::c_void;
use expanse_trie::blobmap::{ArenaError, BlobView, ExpanseBlobMap};

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

/// Creates a new empty `ExpanseBlobMap` whose arena capacity cap is
/// `max_capacity` bytes of allocated chunks (#1300). `chunk_size == 0` selects
/// the default 2 MiB chunk and `max_capacity == 0` the default 1 GiB cap;
/// otherwise the cap is clamped to `[chunk_size, 64 GiB]`, as
/// `ExpanseBlobMap::with_chunk_size_and_max_capacity` clamps it.
#[unsafe(no_mangle)]
pub extern "C" fn expanse_blob_map_new_with_capacity(
    chunk_size: usize,
    max_capacity: usize,
) -> *mut ExpanseBlobMap {
    let chunk = if chunk_size == 0 {
        expanse_trie::blobmap::DEFAULT_CHUNK_SIZE
    } else {
        chunk_size
    };
    let cap = if max_capacity == 0 {
        expanse_trie::blobmap::DEFAULT_ARENA_CAPACITY
    } else {
        max_capacity
    };
    Box::into_raw(Box::new(ExpanseBlobMap::with_chunk_size_and_max_capacity(
        chunk, cap,
    )))
}

/// Turns the reclaim at the capacity cap on (the default) or off: off, an
/// insert that the cap refuses compacts nothing (#1290, #1300).
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_blob_map_set_reclaim_at_cap(map: *mut ExpanseBlobMap, on: bool) {
    // SAFETY: map is null or points to a live ExpanseBlobMap per caller contract.
    if let Some(map_ref) = unsafe { map.as_mut() } {
        map_ref.set_reclaim_at_cap(on);
    }
}

/// Status of [`expanse_blob_map_insert_ex`] and
/// [`expanse_blob_map_arena_stats`], in three bands like
/// `expanse_sync32_status_t`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExpanseBlobStatus {
    /// The insert succeeded, or the stats were written.
    Ok = 0,
    /// Refused: `hot_meta` is wider than 24 bits. The map is unchanged.
    MetaOverflow = 16,
    /// Refused: the payload is larger than a chunk, or memory allocation
    /// failed. The map's contents are unchanged.
    AllocationFailed = 17,
    /// Refused: the capacity cap refused the record's chunk and no compaction
    /// ran (the reclaim rule declined or is off). Dead bytes may remain.
    CapRefused = 18,
    /// Refused: the cap refused the record's chunk after this insert
    /// compacted the arena. Every arena payload has moved.
    ArenaFull = 19,
    /// Usage error: a NULL handle or output, NULL `data` with `len > 0`, or
    /// `len` above `PTRDIFF_MAX`. Nothing was done.
    InvalidArgument = 32,
    /// Any other engine error; `expanse_blob_map_insert_ex` returns none today.
    Error = 48,
}

impl From<ArenaError> for ExpanseBlobStatus {
    fn from(e: ArenaError) -> Self {
        match e {
            ArenaError::MetaOverflow => Self::MetaOverflow,
            ArenaError::AllocationFailed => Self::AllocationFailed,
            ArenaError::OffsetOverflow => Self::CapRefused,
            ArenaError::ArenaFull => Self::ArenaFull,
            _ => Self::Error,
        }
    }
}

/// Arena accounting written by [`expanse_blob_map_arena_stats`]. Append-only:
/// a caller passes `sizeof` the struct it was compiled against and receives
/// that prefix.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpanseBlobArenaStats {
    /// Live records' payload bytes plus their 8-byte headers.
    pub live_bytes: u64,
    /// Allocated chunk bytes, dead and live: what the capacity cap counts.
    pub allocated_bytes: u64,
    /// The capacity cap on `allocated_bytes`.
    pub max_capacity: u64,
    /// The arena's chunk size.
    pub chunk_size: u64,
    /// 1 if an insert the cap refuses may compact the arena, else 0.
    pub reclaim_at_cap: u64,
}

/// Inserts a key-blob pair as [`expanse_blob_map_insert`] does, returning why
/// a refused insert was refused (#1300). `CapRefused` means no compaction ran,
/// so [`expanse_blob_map_arena_stats`]' `allocated_bytes - live_bytes` shows
/// what an [`expanse_blob_map_compact`] could free; `ArenaFull` means this
/// insert compacted and the record still did not fit.
///
/// # Safety
///
/// `map` must be null or a live handle. `data` must point to at least `len`
/// readable bytes (or be null if `len == 0`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_blob_map_insert_ex(
    map: *mut ExpanseBlobMap,
    key: u64,
    data: *const u8,
    len: usize,
    hot_meta: u32,
) -> ExpanseBlobStatus {
    // SAFETY: map is null or points to a live ExpanseBlobMap per caller contract.
    let Some(map_ref) = (unsafe { map.as_mut() }) else {
        return ExpanseBlobStatus::InvalidArgument;
    };
    let slice = if data.is_null() {
        if len == 0 {
            &[]
        } else {
            return ExpanseBlobStatus::InvalidArgument;
        }
    } else {
        if len > (isize::MAX as usize) {
            return ExpanseBlobStatus::InvalidArgument;
        }
        // SAFETY: data is non-null and valid for len bytes per caller contract; len <= isize::MAX.
        unsafe { core::slice::from_raw_parts(data, len) }
    };
    match map_ref.insert(key, slice, hot_meta) {
        Ok(()) => ExpanseBlobStatus::Ok,
        Err(e) => e.into(),
    }
}

/// Writes the arena's accounting into the caller's `stats` buffer of
/// `stats_size` bytes: the prefix of [`ExpanseBlobArenaStats`] both sides know.
/// Returns `InvalidArgument` if `map` or `stats` is null, else `Ok`.
///
/// # Safety
///
/// `map` must be null or a live handle; `stats` null or valid for
/// `stats_size` bytes of writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_blob_map_arena_stats(
    map: *const ExpanseBlobMap,
    stats: *mut ExpanseBlobArenaStats,
    stats_size: usize,
) -> ExpanseBlobStatus {
    // SAFETY: map is null or points to a live ExpanseBlobMap per caller contract.
    let Some(map_ref) = (unsafe { map.as_ref() }) else {
        return ExpanseBlobStatus::InvalidArgument;
    };
    if stats.is_null() {
        return ExpanseBlobStatus::InvalidArgument;
    }
    let arena = map_ref.arena();
    let src = ExpanseBlobArenaStats {
        live_bytes: arena.live_bytes() as u64,
        allocated_bytes: arena.mem_used() as u64,
        max_capacity: arena.max_capacity() as u64,
        chunk_size: arena.chunk_size() as u64,
        reclaim_at_cap: u64::from(map_ref.reclaim_at_cap()),
    };
    let n = stats_size.min(core::mem::size_of::<ExpanseBlobArenaStats>());
    // SAFETY: `stats` is non-null and valid for `stats_size` bytes per
    // contract, and `n` never exceeds either side's size.
    unsafe {
        core::ptr::copy_nonoverlapping(
            core::ptr::from_ref(&src).cast::<u8>(),
            stats.cast::<u8>(),
            n,
        );
    }
    ExpanseBlobStatus::Ok
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
        if len > (isize::MAX as usize) {
            return false;
        }
        // SAFETY: data is non-null and valid for len bytes per caller contract; len <= isize::MAX.
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
/// For uncompressed inline (<= 7 bytes) and arena-allocated values, the written
/// [`ExpanseBlobView::ptr`] borrows into the map and is valid until the next
/// structural mutation of `map` (any insert/remove/clear/compact/free).
/// For compressed inline values, [`ExpanseBlobView::ptr`] is `NULL` and callers
/// use [`expanse_blob_map_get_into`] to decompress into their buffer.
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
            let (ptr, len) = match view {
                BlobView::Inline(slice) => (slice.as_ptr(), slice.len()),
                BlobView::Arena(slice) => (slice.as_ptr(), slice.len()),
                BlobView::CompressedInline { len, .. } => (core::ptr::null(), len as usize),
            };
            // SAFETY: out_view is non-null and writable per contract.
            unsafe {
                *out_view = ExpanseBlobView {
                    ptr,
                    len,
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

/// Looks up a key, copying the payload into `buf` (up to `buf_len`).
/// Returns `true` if the key was found.
///
/// If `out_len` is non-null, writes the actual full length of the payload.
/// If `out_meta` is non-null, writes the 24-bit hot metadata (0 for inline).
///
/// # Safety
///
/// `map` must be a valid non-null handle. `buf` must be writable for `buf_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_blob_map_get_into(
    map: *const ExpanseBlobMap,
    key: u64,
    buf: *mut u8,
    buf_len: usize,
    out_len: *mut usize,
    out_meta: *mut u32,
) -> bool {
    // SAFETY: map is null or points to a live ExpanseBlobMap per caller contract.
    let Some(map_ref) = (unsafe { map.as_ref() }) else {
        return false;
    };
    if let Some((view, meta)) = map_ref.get(key) {
        let bytes = view.as_bytes();
        if !out_len.is_null() {
            // SAFETY: out_len is non-null and writable per contract.
            unsafe { *out_len = bytes.len() };
        }
        if !out_meta.is_null() {
            // SAFETY: out_meta is non-null and writable per contract.
            unsafe { *out_meta = meta };
        }
        if !buf.is_null() && buf_len > 0 {
            let copy_len = bytes.len().min(buf_len);
            // SAFETY: buf is valid for writes of buf_len bytes; copy_len <= buf_len.
            unsafe {
                core::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, copy_len);
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
                let is_inline = view.is_inline();
                let (ptr, len) = match view {
                    BlobView::Inline(slice) => (slice.as_ptr(), slice.len()),
                    BlobView::Arena(slice) => (slice.as_ptr(), slice.len()),
                    BlobView::CompressedInline { ref buf, len } => (buf.as_ptr(), len as usize),
                };
                let c_view = ExpanseBlobView {
                    ptr,
                    len,
                    hot_meta: meta,
                    is_inline,
                };
                // SAFETY: caller supplied callback function pointer and user_ctx.
                // ptr points to valid slice or stack buf for the duration of cb.
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

/// Convenience alias matching the standardized `expanse_*_contains` naming convention.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_blob_map_contains(map: *const ExpanseBlobMap, key: u64) -> bool {
    // SAFETY: forwarded to expanse_blob_map_contains_key.
    unsafe { expanse_blob_map_contains_key(map, key) }
}
