//! The concurrent half of the modern `expanse_*` C API.
//!
//! `expanse_trie::sync` is gated on `all(target_pointer_width = "64",
//! feature = "std")` — the one-writer/many-reader containers need
//! `std::sync` — so these entry points are absent from every `no_std`
//! build, 64-bit ones included. See the surface matrix in
//! `docs/COMPAT.md`.

use super::super::{bytes, cstr, put};
use super::{ExpanseOrderedBytesNavStatus, ExpanseStrNavStatus};
use crate::blobmap::ExpanseBlobStatus;
use core::ffi::{c_char, c_void};
use core::ptr::NonNull;
use expanse_trie::blobmap::CompareExchangeError;
use expanse_trie::ordered_bytesmap::{EscapeDecodeError, ExpanseOrderedBytesMap};
use expanse_trie::sync::{
    BlobBufferTooSmall, BlobReader, BytesReader, MapReader, OrderedBytesReader, SetReader,
    StrReader, SyncExpanseBlobMap, SyncExpanseBytesMap, SyncExpanseMap, SyncExpanseOrderedBytesMap,
    SyncExpanseSet, SyncExpanseStrMap,
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

/// A registered reader of a [`SyncExpanseBlobMap`]. See [`SyncSetReader`].
///
/// Its calls take the handle as `*mut`: the Rust reader's reads take
/// `&mut self`, and the header's one-thread-at-a-time rule is what makes
/// that exclusive borrow sound.
pub struct SyncBlobMapReader(BlobReader<'static>);

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
    assert_send::<SyncBlobMapReader>();
};

/// Generates the handle, size and reader-registration entry points of one
/// shared container: `_new` (when it takes no argument), `_free`, `_len`,
/// `_mem_used` (when listed), `_mem_held`, `_shrink_to_fit`, `_reader_new`
/// and `_reader_free`. Every one returns 0 or null, or does nothing, for a
/// null handle.
macro_rules! sync_surface {
    (
        container: $Map:ty,
        handle: $h:ident,
        what: $what:expr,
        $(new: $new:ident,)?
        free: $free:ident,
        len: $len:ident,
        $(mem_used: $mem_used:ident,)?
        mem_held: $mem_held:ident,
        shrink_to_fit: $shrink:ident,
        reader: $Reader:ident($RustReader:ident),
        reader_new: $reader_new:ident,
        reader_free: $reader_free:ident $(,)?
    ) => {
        $(
            #[doc = concat!("Creates an empty concurrent ", $what, ".")]
            #[unsafe(no_mangle)]
            pub extern "C" fn $new() -> *mut $Map {
                Box::into_raw(Box::new(<$Map>::new()))
            }
        )?

        #[doc = concat!("Frees a concurrent ", $what, ". All readers must be freed first.")]
        ///
        /// # Safety
        ///
        #[doc = concat!("`", stringify!($h), "` must be null or come from the matching `_new`, with")]
        /// no live reader and no call on it in progress.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $free($h: *mut $Map) {
            if !$h.is_null() {
                // SAFETY: handle per contract.
                drop(unsafe { Box::from_raw($h) });
            }
        }

        /// Number of entries; 0 for a null handle.
        ///
        /// # Safety
        ///
        #[doc = concat!("`", stringify!($h), "` must be null or a live handle.")]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $len($h: *const $Map) -> u64 {
            // SAFETY: null or live handle per contract.
            unsafe { $h.as_ref() }.map_or(0, <$Map>::len)
        }

        $(
            #[doc = concat!("Heap bytes used by the ", $what, ", read with writers excluded")]
            #[doc = concat!("([`", stringify!($Map), "::mem_used`]); 0 for a null handle.")]
            ///
            /// # Safety
            ///
            #[doc = concat!("`", stringify!($h), "` must be null or a live handle.")]
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn $mem_used($h: *const $Map) -> usize {
                // SAFETY: null or live handle per contract.
                unsafe { $h.as_ref() }.map_or(0, <$Map>::mem_used)
            }
        )?

        #[doc = concat!("Heap bytes the ", $what, " holds from the global allocator: its own share")]
        /// plus the blocks its epoch collector holds for reuse or is waiting
        #[doc = concat!("to reclaim ([`", stringify!($Map), "::mem_held`]); 0 for a null handle.")]
        /// Writers wait for it.
        ///
        /// # Safety
        ///
        #[doc = concat!("`", stringify!($h), "` must be null or a live handle.")]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $mem_held($h: *const $Map) -> usize {
            // SAFETY: null or live handle per contract.
            unsafe { $h.as_ref() }.map_or(0, <$Map>::mem_held)
        }

        #[doc = concat!("Returns the freed blocks the ", $what, "'s epoch collector keeps for reuse")]
        #[doc = concat!("to the global allocator and returns the bytes released ([`", stringify!($Map), "::shrink_to_fit`]);")]
        /// 0 for a null handle. Runs beside readers and writers without
        /// excluding either.
        ///
        /// # Safety
        ///
        #[doc = concat!("`", stringify!($h), "` must be null or a live handle.")]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $shrink($h: *const $Map) -> usize {
            // SAFETY: null or live handle per contract.
            unsafe { $h.as_ref() }.map_or(0, <$Map>::shrink_to_fit)
        }

        #[doc = concat!("Registers a reader handle for this thread. Free it before the ", $what, ".")]
        ///
        /// # Safety
        ///
        #[doc = concat!("`", stringify!($h), "` must be null or a live handle that outlives the reader.")]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $reader_new($h: *const $Map) -> *mut $Reader {
            // SAFETY: null or live handle per contract.
            let Some(c) = (unsafe { $h.as_ref() }) else {
                return core::ptr::null_mut();
            };
            // SAFETY: lifetime extension only — the reader borrows the
            // container, and the header's contract requires freeing the
            // reader before its container (documented in expanse.h).
            let reader: $RustReader<'static> = unsafe { core::mem::transmute(c.reader()) };
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
        #[doc = concat!("with no call on it in progress on any thread, and its ", $what, " still")]
        /// alive.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $reader_free(reader: *mut $Reader) {
            if !reader.is_null() {
                // SAFETY: handle per contract.
                drop(unsafe { Box::from_raw(reader) });
            }
        }
    };
}

sync_surface!(
    container: SyncExpanseSet,
    handle: set,
    what: "set",
    new: expanse_sync_set_new,
    free: expanse_sync_set_free,
    len: expanse_sync_set_len,
    mem_held: expanse_sync_set_mem_held,
    shrink_to_fit: expanse_sync_set_shrink_to_fit,
    reader: SyncSetReader(SetReader),
    reader_new: expanse_sync_set_reader_new,
    reader_free: expanse_sync_set_reader_free,
);

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

sync_surface!(
    container: SyncExpanseMap,
    handle: map,
    what: "map",
    new: expanse_sync_map_new,
    free: expanse_sync_map_free,
    len: expanse_sync_map_len,
    mem_used: expanse_sync_map_mem_used,
    mem_held: expanse_sync_map_mem_held,
    shrink_to_fit: expanse_sync_map_shrink_to_fit,
    reader: SyncMapReader(MapReader),
    reader_new: expanse_sync_map_reader_new,
    reader_free: expanse_sync_map_reader_free,
);

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

map_nav!(
    /// Smallest entry with key `>= key`, through a registered reader.
    ///
    /// # Safety
    ///
    /// Same contract as [`expanse_sync_map_reader_first`].
    handle: reader: SyncMapReader, |r| r.0,
    expanse_sync_map_reader_next_at_or_after, next_at_or_after
);

map_nav!(
    /// Smallest entry with key `> key`, through a registered reader; false for
/// `key == u64::MAX`.
    ///
    /// # Safety
    ///
    /// Same contract as [`expanse_sync_map_reader_first`].
    handle: reader: SyncMapReader, |r| r.0,
    expanse_sync_map_reader_next_after, next_after
);

map_nav!(
    /// Largest entry with key `<= key`, through a registered reader.
    ///
    /// # Safety
    ///
    /// Same contract as [`expanse_sync_map_reader_first`].
    handle: reader: SyncMapReader, |r| r.0,
    expanse_sync_map_reader_prev_at_or_before, prev_at_or_before
);

map_nav!(
    /// Largest entry with key `< key`, through a registered reader; false for
/// `key == 0`.
    ///
    /// # Safety
    ///
    /// Same contract as [`expanse_sync_map_reader_first`].
    handle: reader: SyncMapReader, |r| r.0,
    expanse_sync_map_reader_prev_before, prev_before
);

// ---- The shared string map (#1538) ----
//
// The `u64` map's conventions, with the key as a NUL-terminated C string read
// as `expanse_strmap_*` reads it (`cstr`): a NULL key is treated like a NULL
// handle, returning false or 0 and writing nothing. A C string cannot carry an
// interior NUL, so no key is refused for its content.

sync_surface!(
    container: SyncExpanseStrMap,
    handle: map,
    what: "string map",
    new: expanse_sync_strmap_new,
    free: expanse_sync_strmap_free,
    len: expanse_sync_strmap_len,
    mem_used: expanse_sync_strmap_mem_used,
    mem_held: expanse_sync_strmap_mem_held,
    shrink_to_fit: expanse_sync_strmap_shrink_to_fit,
    reader: SyncStrMapReader(StrReader),
    reader_new: expanse_sync_strmap_reader_new,
    reader_free: expanse_sync_strmap_reader_free,
);

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
        sync_surface!(
            container: $Map,
            handle: map,
            what: $what,
            new: $new,
            free: $free,
            len: $len,
            mem_used: $mem_used,
            mem_held: $mem_held,
            shrink_to_fit: $shrink,
            reader: $Reader($RustReader),
            reader_new: $reader_new,
            reader_free: $reader_free,
        );
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

// ---- The shared blob map's handle and reader (#1538) ----
//
// Values never leave as a borrowed pointer: the `_get_into` calls copy into
// the caller's buffer (AGENTS.md §2.3 item 4, §2.4). Each is ONE call to
// `BlobReader::get_into`, which takes the length, the metadata and the bytes
// from one validated read (or one locked fallback) and checks the length
// before it writes (`blob_copy_into` in `expanse_trie::sync`), so a buffer too
// small is never written and the length reported is that of the value that
// would have been copied. No probe precedes the copy.

/// The blob map's `chunk_size` convention: `0` selects the default chunk, as
/// `expanse_blob_map_new` does.
fn blob_chunk_size(chunk_size: usize) -> usize {
    if chunk_size == 0 {
        expanse_trie::blobmap::DEFAULT_CHUNK_SIZE
    } else {
        chunk_size
    }
}

/// Creates an empty shared blob map. `chunk_size == 0` selects the default
/// 2 MiB chunk; the capacity cap is the default 1 GiB.
#[unsafe(no_mangle)]
pub extern "C" fn expanse_sync_blob_map_new(chunk_size: usize) -> *mut SyncExpanseBlobMap {
    Box::into_raw(Box::new(SyncExpanseBlobMap::with_chunk_size(
        blob_chunk_size(chunk_size),
    )))
}

/// Creates an empty shared blob map whose arena capacity cap is
/// `max_capacity` bytes of allocated chunks. `0` selects the default chunk
/// and the default 1 GiB cap; the cap is clamped as
/// `expanse_blob_map_new_with_capacity` clamps it.
#[unsafe(no_mangle)]
pub extern "C" fn expanse_sync_blob_map_new_with_capacity(
    chunk_size: usize,
    max_capacity: usize,
) -> *mut SyncExpanseBlobMap {
    let cap = if max_capacity == 0 {
        expanse_trie::blobmap::DEFAULT_ARENA_CAPACITY
    } else {
        max_capacity
    };
    Box::into_raw(Box::new(
        SyncExpanseBlobMap::with_chunk_size_and_max_capacity(blob_chunk_size(chunk_size), cap),
    ))
}

// `_new` takes a chunk size and `_mem_used` waits for a running compaction,
// so both are written out above and below; the rest is the shared surface.
sync_surface!(
    container: SyncExpanseBlobMap,
    handle: map,
    what: "blob map",
    free: expanse_sync_blob_map_free,
    len: expanse_sync_blob_map_len,
    mem_held: expanse_sync_blob_map_mem_held,
    shrink_to_fit: expanse_sync_blob_map_shrink_to_fit,
    reader: SyncBlobMapReader(BlobReader),
    reader_new: expanse_sync_blob_map_reader_new,
    reader_free: expanse_sync_blob_map_reader_free,
);

/// Stores `key -> data` with 24-bit hot metadata, replacing any value.
/// Returns `Ok`, the refusal (`MetaOverflow`, `AllocationFailed`,
/// `CapRefused`, `ArenaFull`; the map's contents unchanged), or
/// `InvalidArgument` for a null handle, null `data` with `len > 0`, or `len`
/// above `isize::MAX`.
///
/// # Safety
///
/// `map` must be null or a live handle; `data` readable for `len` bytes
/// (null only when `len == 0`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_blob_map_insert(
    map: *const SyncExpanseBlobMap,
    key: u64,
    data: *const u8,
    len: usize,
    hot_meta: u32,
) -> ExpanseBlobStatus {
    // SAFETY: null or live handle, and `data` readable for `len` bytes, per
    // contract.
    let (Some(m), Some(d)) = (unsafe { map.as_ref() }, unsafe { bytes(data.cast(), len) }) else {
        return ExpanseBlobStatus::InvalidArgument;
    };
    match m.insert(key, d, hot_meta) {
        Ok(()) => ExpanseBlobStatus::Ok,
        Err(e) => e.into(),
    }
}

/// Removes `key`; true if it was present.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_blob_map_remove(
    map: *const SyncExpanseBlobMap,
    key: u64,
) -> bool {
    // SAFETY: null or live handle per contract.
    unsafe { map.as_ref() }.is_some_and(|m| m.remove(key))
}

/// The caller-buffer read behind both `_get_into` entry points: validates the
/// buffer arguments, runs `read` ONCE over the caller's buffer, and writes the
/// outcome. `read` is `BlobReader::get_into`, which copies nothing when the
/// value does not fit, so `buf` is written only on `Ok`.
///
/// # Safety
///
/// `buf` must be null or writable for `buf_len` bytes; `len_out` and
/// `meta_out` null or writable.
unsafe fn blob_get_into(
    read: impl FnOnce(&mut [u8]) -> Result<Option<(usize, u32)>, BlobBufferTooSmall>,
    buf: *mut u8,
    buf_len: usize,
    len_out: *mut usize,
    meta_out: *mut u32,
) -> ExpanseBlobStatus {
    if buf_len > isize::MAX as usize {
        return ExpanseBlobStatus::InvalidArgument;
    }
    let dst: &mut [u8] = if buf.is_null() {
        if buf_len != 0 {
            return ExpanseBlobStatus::InvalidArgument;
        }
        &mut []
    } else {
        // SAFETY: `buf` is non-null and writable for `buf_len <= isize::MAX`
        // bytes per contract.
        unsafe { core::slice::from_raw_parts_mut(buf, buf_len) }
    };
    let (status, len, meta) = match read(dst) {
        Ok(Some((len, meta))) => (ExpanseBlobStatus::Ok, len, meta),
        Ok(None) => return ExpanseBlobStatus::NotFound,
        Err(e) => (ExpanseBlobStatus::BufferTooSmall, e.required, e.hot_meta),
    };
    // SAFETY: `len_out` and `meta_out` null or writable per contract.
    unsafe {
        put(len_out, len);
        put(meta_out, meta);
    }
    status
}

/// One-shot caller-buffer read (registers a throwaway reader; prefer a reader
/// handle in hot loops). The contract is
/// [`expanse_sync_blob_map_reader_get_into`]'s.
///
/// # Safety
///
/// `map` must be null or a live handle; `buf` null or writable for `buf_len`
/// bytes; `len_out` and `meta_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_blob_map_get_into(
    map: *const SyncExpanseBlobMap,
    key: u64,
    buf: *mut u8,
    buf_len: usize,
    len_out: *mut usize,
    meta_out: *mut u32,
) -> ExpanseBlobStatus {
    // SAFETY: null or live handle per contract.
    let Some(m) = (unsafe { map.as_ref() }) else {
        return ExpanseBlobStatus::InvalidArgument;
    };
    // SAFETY: buffer and out-pointers forwarded per contract.
    unsafe {
        blob_get_into(
            |dst| m.reader().get_into(key, dst),
            buf,
            buf_len,
            len_out,
            meta_out,
        )
    }
}

/// One-shot metadata lookup (registers a throwaway reader): true with the
/// value's hot metadata in `*meta_out` (`0` for a value stored in its slot
/// word) when `key` is present; false, nothing written, otherwise.
///
/// # Safety
///
/// `map` must be null or a live handle; `meta_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_blob_map_get_meta(
    map: *const SyncExpanseBlobMap,
    key: u64,
    meta_out: *mut u32,
) -> bool {
    // SAFETY: null or live handle per contract.
    let Some(meta) = (unsafe { map.as_ref() }).and_then(|m| m.get_meta(key)) else {
        return false;
    };
    // SAFETY: `meta_out` null or writable per contract.
    unsafe { put(meta_out, meta) };
    true
}

/// One-shot membership test (registers a throwaway reader).
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_blob_map_contains(
    map: *const SyncExpanseBlobMap,
    key: u64,
) -> bool {
    // SAFETY: null or live handle per contract.
    unsafe { map.as_ref() }.is_some_and(|m| m.contains_key(key))
}

/// Heap bytes used by the index and the arena, read with writers excluded:
/// it waits for a running compaction, and writers wait for it. 0 for a null
/// handle.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_blob_map_mem_used(map: *const SyncExpanseBlobMap) -> usize {
    // SAFETY: null or live handle per contract.
    unsafe { map.as_ref() }.map_or(0, SyncExpanseBlobMap::mem_used)
}

/// [`expanse_sync_blob_map_mem_used`] without the wait: an estimate that
/// takes none of the map's locks (`SyncExpanseBlobMap::mem_used_estimate`).
/// 0 for a null handle.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_blob_map_mem_used_estimate(
    map: *const SyncExpanseBlobMap,
) -> usize {
    // SAFETY: null or live handle per contract.
    unsafe { map.as_ref() }.map_or(0, SyncExpanseBlobMap::mem_used_estimate)
}

/// [`expanse_sync_blob_map_mem_held`] without the wait: the map's share as
/// [`expanse_sync_blob_map_mem_used_estimate`] reads it, plus what the epoch
/// collector holds, read without waiting for any lock
/// (`SyncExpanseBlobMap::mem_held_estimate`). 0 for a null handle.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_blob_map_mem_held_estimate(
    map: *const SyncExpanseBlobMap,
) -> usize {
    // SAFETY: null or live handle per contract.
    unsafe { map.as_ref() }.map_or(0, SyncExpanseBlobMap::mem_held_estimate)
}

/// Turns the reclaim at the capacity cap on (the default) or off; a null
/// handle is a no-op.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_blob_map_set_reclaim_at_cap(
    map: *const SyncExpanseBlobMap,
    on: bool,
) {
    // SAFETY: null or live handle per contract.
    if let Some(m) = unsafe { map.as_ref() } {
        m.set_reclaim_at_cap(on);
    }
}

/// Compacts the arena: `Ok`, the mapped engine error, or `InvalidArgument`
/// for a null handle. Readers keep reading during the payload copy; the
/// shared surface hands out no view, so no C pointer is invalidated.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_blob_map_compact(
    map: *const SyncExpanseBlobMap,
) -> ExpanseBlobStatus {
    // SAFETY: null or live handle per contract.
    let Some(m) = (unsafe { map.as_ref() }) else {
        return ExpanseBlobStatus::InvalidArgument;
    };
    match m.compact() {
        Ok(_) => ExpanseBlobStatus::Ok,
        Err(e) => e.into(),
    }
}

/// Optimistic caller-buffer read through a registered reader.
///
/// `Ok`: the payload is in `buf[..*len_out]`, `*meta_out` is its hot
/// metadata, the rest of `buf` is untouched. `NotFound`: nothing written.
/// `BufferTooSmall`: the key is present and its payload is longer than
/// `buf_len`; nothing is copied and `buf` is untouched, `*len_out` is the
/// payload's length and `*meta_out` its metadata, both from the same
/// validated read. A retry is a new read. `InvalidArgument`: a null reader,
/// null `buf` with `buf_len > 0`, or `buf_len` above `isize::MAX`; nothing
/// written. Null `buf` with `buf_len == 0` is a length probe.
///
/// # Safety
///
/// `reader` must be null or live (its map still alive) and in use by no
/// other thread; `buf` null or writable for `buf_len` bytes; `len_out` and
/// `meta_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_blob_map_reader_get_into(
    reader: *mut SyncBlobMapReader,
    key: u64,
    buf: *mut u8,
    buf_len: usize,
    len_out: *mut usize,
    meta_out: *mut u32,
) -> ExpanseBlobStatus {
    // SAFETY: null or live reader, used by this thread alone, per contract.
    let Some(r) = (unsafe { reader.as_mut() }) else {
        return ExpanseBlobStatus::InvalidArgument;
    };
    // SAFETY: buffer and out-pointers forwarded per contract.
    unsafe {
        blob_get_into(
            |dst| r.0.get_into(key, dst),
            buf,
            buf_len,
            len_out,
            meta_out,
        )
    }
}

/// Optimistic metadata lookup through a registered reader, with the return
/// convention of [`expanse_sync_blob_map_get_meta`].
///
/// # Safety
///
/// `reader` must be null or live (its map still alive) and in use by no
/// other thread; `meta_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_blob_map_reader_get_meta(
    reader: *mut SyncBlobMapReader,
    key: u64,
    meta_out: *mut u32,
) -> bool {
    // SAFETY: null or live reader, used by this thread alone, per contract.
    let Some(meta) = (unsafe { reader.as_mut() }).and_then(|r| r.0.get_meta(key)) else {
        return false;
    };
    // SAFETY: `meta_out` null or writable per contract.
    unsafe { put(meta_out, meta) };
    true
}

/// Optimistic membership test through a registered reader.
///
/// # Safety
///
/// `reader` must be null or live (its map still alive) and in use by no
/// other thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_blob_map_reader_contains(
    reader: *mut SyncBlobMapReader,
    key: u64,
) -> bool {
    // SAFETY: null or live reader, used by this thread alone, per contract.
    unsafe { reader.as_mut() }.is_some_and(|r| r.0.contains(key))
}

// ---- Read-modify-write on the shared string, bytes, ordered-bytes and blob
// maps (#1538) ----
//
// Each entry point is ONE call of the Rust map's `compare_exchange` (or
// `get_or_insert`, itself `compare_exchange(key, None, Some(value))`), so it
// is the same single linearizable step with the same optimistic form and
// exclusive fallback; the shim adds no engine path. The word-valued maps take
// the `u64` map's convention (`expanse_sync_map_compare_exchange`,
// `expanse_sync_map_get_or_insert`): presence flags on both sides, the
// observed word reported on both outcomes. The blob map reports through
// `expanse_blob_status_t` and copies an observed payload into a caller buffer
// after the exchange has returned, from the one value that exchange read.

/// Writes a word-valued exchange's outcome under the
/// [`expanse_sync_map_compare_exchange`] convention and returns whether the
/// store happened.
///
/// # Safety
///
/// `observed_present_out` and `observed_out` null or writable.
unsafe fn put_word_exchange(
    res: Result<Option<u64>, Option<u64>>,
    observed_present_out: *mut bool,
    observed_out: *mut u64,
) -> bool {
    let (exchanged, observed) = match res {
        Ok(prev) => (true, prev),
        Err(seen) => (false, seen),
    };
    // SAFETY: both outputs null or writable per this function's contract.
    unsafe {
        put(observed_present_out, observed.is_some());
        if let Some(v) = observed {
            put(observed_out, v);
        }
    }
    exchanged
}

/// Writes a word-valued `get_or_insert` outcome under the
/// [`expanse_sync_map_get_or_insert`] convention: true when stored, else
/// false with the present value in `*existing_out`.
///
/// # Safety
///
/// `existing_out` null or writable.
unsafe fn put_word_get_or_insert(existing: Option<u64>, existing_out: *mut u64) -> bool {
    match existing {
        // SAFETY: `existing_out` null or writable per this function's contract.
        Some(v) => unsafe {
            put(existing_out, v);
            false
        },
        None => true,
    }
}

/// Stores `new_value` under `key` only if the key's current word is
/// `expected_value`, as one linearizable step; the convention, presence flags
/// and outputs are [`expanse_sync_map_compare_exchange`]'s. A null `map` or
/// `key` returns false and writes nothing.
///
/// # Safety
///
/// `map` must be null or a live handle; `key` null or a valid NUL-terminated
/// C string; `observed_present_out` and `observed_out` null or writable.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)] // the `u64` map's C signature, with a string key
pub unsafe extern "C" fn expanse_sync_strmap_compare_exchange(
    map: *const SyncExpanseStrMap,
    key: *const c_char,
    expected_present: bool,
    expected_value: u64,
    new_present: bool,
    new_value: u64,
    observed_present_out: *mut bool,
    observed_out: *mut u64,
) -> bool {
    // SAFETY: null or live handle and null or valid C string per contract.
    let (Some(m), Some(k)) = (unsafe { map.as_ref() }, unsafe { cstr(key) }) else {
        return false;
    };
    let res = m.compare_exchange(
        k,
        expected_present.then_some(expected_value),
        new_present.then_some(new_value),
    );
    // SAFETY: outputs null or writable per contract.
    unsafe { put_word_exchange(res, observed_present_out, observed_out) }
}

/// Stores `value` under `key` only if `key` is absent, as one linearizable
/// step; the convention is [`expanse_sync_map_get_or_insert`]'s. A null `map`
/// or `key` returns false and writes nothing.
///
/// # Safety
///
/// `map` must be null or a live handle; `key` null or a valid NUL-terminated
/// C string; `existing_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync_strmap_get_or_insert(
    map: *const SyncExpanseStrMap,
    key: *const c_char,
    value: u64,
    existing_out: *mut u64,
) -> bool {
    // SAFETY: null or live handle and null or valid C string per contract.
    let (Some(m), Some(k)) = (unsafe { map.as_ref() }, unsafe { cstr(key) }) else {
        return false;
    };
    // SAFETY: `existing_out` null or writable per contract.
    unsafe { put_word_get_or_insert(m.get_or_insert(k, value), existing_out) }
}

/// The two byte-string maps' read-modify-write entry points, each one call
/// of the map's `compare_exchange` or `get_or_insert`.
macro_rules! sync_bytes_rmw {
    (
        map: $Map:ty,
        compare_exchange: $cx:ident,
        get_or_insert: $goi:ident $(,)?
    ) => {
        /// Stores `new_value` under `key` only if the key's current word is
        /// `expected_value`, as one linearizable step; the convention,
        /// presence flags and outputs are [`expanse_sync_map_compare_exchange`]'s.
        /// A null `map`, or a null `key` with `len > 0`, returns false and
        /// writes nothing.
        ///
        /// # Safety
        ///
        /// `map` must be null or a live handle; `key` readable for `len` bytes
        /// (null only when `len == 0`); `observed_present_out` and
        /// `observed_out` null or writable.
        #[unsafe(no_mangle)]
        #[allow(clippy::too_many_arguments)] // the `u64` map's C signature, with a byte-string key
        pub unsafe extern "C" fn $cx(
            map: *const $Map,
            key: *const c_void,
            len: usize,
            expected_present: bool,
            expected_value: u64,
            new_present: bool,
            new_value: u64,
            observed_present_out: *mut bool,
            observed_out: *mut u64,
        ) -> bool {
            // SAFETY: null or live handle and a key readable for `len` bytes
            // per contract.
            let (Some(m), Some(k)) = (unsafe { map.as_ref() }, unsafe { bytes(key, len) }) else {
                return false;
            };
            let res = m.compare_exchange(
                k,
                expected_present.then_some(expected_value),
                new_present.then_some(new_value),
            );
            // SAFETY: outputs null or writable per contract.
            unsafe { put_word_exchange(res, observed_present_out, observed_out) }
        }

        /// Stores `value` under `key` only if `key` is absent, as one
        /// linearizable step; the convention is
        /// [`expanse_sync_map_get_or_insert`]'s. A null `map`, or a null `key`
        /// with `len > 0`, returns false and writes nothing.
        ///
        /// # Safety
        ///
        /// `map` must be null or a live handle; `key` readable for `len` bytes
        /// (null only when `len == 0`); `existing_out` null or writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $goi(
            map: *const $Map,
            key: *const c_void,
            len: usize,
            value: u64,
            existing_out: *mut u64,
        ) -> bool {
            // SAFETY: null or live handle and a key readable for `len` bytes
            // per contract.
            let (Some(m), Some(k)) = (unsafe { map.as_ref() }, unsafe { bytes(key, len) }) else {
                return false;
            };
            // SAFETY: `existing_out` null or writable per contract.
            unsafe { put_word_get_or_insert(m.get_or_insert(k, value), existing_out) }
        }
    };
}

sync_bytes_rmw!(
    map: SyncExpanseBytesMap,
    compare_exchange: expanse_sync_bytesmap_compare_exchange,
    get_or_insert: expanse_sync_bytesmap_get_or_insert,
);

sync_bytes_rmw!(
    map: SyncExpanseOrderedBytesMap,
    compare_exchange: expanse_sync_ordered_bytesmap_compare_exchange,
    get_or_insert: expanse_sync_ordered_bytesmap_get_or_insert,
);

/// A blob value argument: `Ok(None)` when `present` is false (the pointer,
/// length and metadata are then ignored), `Ok(Some((bytes, meta)))` when it
/// is present and readable, `Err(())` for a null `data` with `len > 0` or a
/// `len` above `isize::MAX`.
///
/// # Safety
///
/// When `present`, `data` readable for `len` bytes (null only when `len == 0`).
unsafe fn blob_arg<'a>(
    present: bool,
    data: *const u8,
    len: usize,
    meta: u32,
) -> Result<Option<(&'a [u8], u32)>, ()> {
    if !present {
        return Ok(None);
    }
    // SAFETY: `data` readable for `len` bytes per this function's contract.
    match unsafe { bytes(data.cast(), len) } {
        Some(d) => Ok(Some((d, meta))),
        None => Err(()),
    }
}

/// Whether a caller buffer is usable, as `blob_get_into` checks it: false
/// for a null `buf` with `buf_len > 0` or a `buf_len` above `isize::MAX`.
fn blob_buf_ok(buf: *mut u8, buf_len: usize) -> bool {
    buf_len <= isize::MAX as usize && !(buf.is_null() && buf_len != 0)
}

/// Reports a present value a read-modify-write observed: `Mismatch` with the
/// payload copied into `buf[..len]` when it fits, else `BufferTooSmall` with
/// nothing copied; `*len_out` and `*meta_out` are written either way. The
/// value is the one the exchange read, so length, metadata and bytes agree.
///
/// The copy goes through raw pointers after every borrow of the caller's
/// input payloads has ended, so `buf` may be the buffer the caller passed as
/// `expected_data` or `data` (a retry loop's pattern).
///
/// # Safety
///
/// `buf` null or writable for `buf_len` bytes (`blob_buf_ok` holds);
/// `len_out` and `meta_out` null or writable.
unsafe fn put_blob_observed(
    data: &[u8],
    meta: u32,
    buf: *mut u8,
    buf_len: usize,
    len_out: *mut usize,
    meta_out: *mut u32,
) -> ExpanseBlobStatus {
    let status = if data.len() <= buf_len {
        if !data.is_empty() {
            // SAFETY: `buf` is non-null (a null buffer has `buf_len == 0`,
            // and the payload is non-empty) and writable for `buf_len >=
            // data.len()` bytes per contract; `data` is an owned copy the
            // exchange returned, so the two do not overlap.
            unsafe { core::ptr::copy_nonoverlapping(data.as_ptr(), buf, data.len()) };
        }
        ExpanseBlobStatus::Mismatch
    } else {
        ExpanseBlobStatus::BufferTooSmall
    };
    // SAFETY: out-pointers null or writable per contract.
    unsafe {
        put(len_out, data.len());
        put(meta_out, meta);
    }
    status
}

/// Stores `new` under `key` only if the key's current value is `expected`,
/// as one linearizable step. A value is the payload bytes and the 24-bit
/// metadata, both compared; `*_present == false` means absent (the pointer,
/// length and metadata are then ignored): an absent `expected` stores only
/// into an absent key, an absent `new` removes the key.
///
/// - `Ok`: stored (or removed); nothing written. The value replaced was
///   `expected`.
/// - `Mismatch`: the current value is not `expected`; the map is unchanged.
///   `*observed_present_out` is written; when the key is present, its payload
///   is in `buf[..*len_out]` and its metadata in `*meta_out`; when absent,
///   `*len_out` and `*meta_out` are `0`.
/// - `BufferTooSmall`: a mismatch whose present payload is longer than
///   `buf_len`: nothing copied, `*observed_present_out`, `*len_out` and
///   `*meta_out` written from the same exchange.
/// - A refusal (`MetaOverflow`, `AllocationFailed`, `CapRefused`,
///   `ArenaFull`): the current value **was** `expected` but `new` cannot be
///   stored; the map is unchanged and nothing written.
/// - `InvalidArgument`: a null `map`, a present payload with null data and
///   `len > 0` or a length above `isize::MAX`, or a null `buf` with
///   `buf_len > 0` or `buf_len` above `isize::MAX`; nothing done.
///
/// # Safety
///
/// `map` must be null or a live handle; `expected_data` and `new_data`, when
/// their `_present` flag is set, readable for their lengths (null only when
/// the length is 0); `buf` null or writable for `buf_len` bytes, and it may
/// overlap `expected_data` or `new_data`; the other outputs null or writable.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)] // C has no tuple: each value is flag, data, length, metadata
pub unsafe extern "C" fn expanse_sync_blob_map_compare_exchange(
    map: *const SyncExpanseBlobMap,
    key: u64,
    expected_present: bool,
    expected_data: *const u8,
    expected_len: usize,
    expected_meta: u32,
    new_present: bool,
    new_data: *const u8,
    new_len: usize,
    new_meta: u32,
    observed_present_out: *mut bool,
    buf: *mut u8,
    buf_len: usize,
    len_out: *mut usize,
    meta_out: *mut u32,
) -> ExpanseBlobStatus {
    // SAFETY: null or live handle per contract.
    let Some(m) = (unsafe { map.as_ref() }) else {
        return ExpanseBlobStatus::InvalidArgument;
    };
    if !blob_buf_ok(buf, buf_len) {
        return ExpanseBlobStatus::InvalidArgument;
    }
    let (Ok(expected), Ok(new)) = (
        // SAFETY: `expected_data` readable for `expected_len` bytes when
        // present, per contract.
        unsafe { blob_arg(expected_present, expected_data, expected_len, expected_meta) },
        // SAFETY: `new_data` readable for `new_len` bytes when present, per
        // contract.
        unsafe { blob_arg(new_present, new_data, new_len, new_meta) },
    ) else {
        return ExpanseBlobStatus::InvalidArgument;
    };
    // The borrows of the caller's payloads end with this call: what follows
    // writes only from the owned value it returns.
    let observed = match m.compare_exchange(key, expected, new) {
        Ok(_) => return ExpanseBlobStatus::Ok,
        Err(CompareExchangeError::Unstorable(cause)) => return cause.into(),
        Err(CompareExchangeError::Mismatch(seen)) => seen,
    };
    // SAFETY: outputs null or writable, `buf` valid for `buf_len` bytes, per
    // contract and `blob_buf_ok`.
    unsafe {
        put(observed_present_out, observed.is_some());
        match observed {
            Some((data, meta)) => put_blob_observed(&data, meta, buf, buf_len, len_out, meta_out),
            None => {
                put(len_out, 0);
                put(meta_out, 0);
                ExpanseBlobStatus::Mismatch
            }
        }
    }
}

/// Swaps the 24-bit hot metadata of `key` in its slot, with no payload copy,
/// only if the current metadata is `expected_meta`, as one linearizable step.
/// The current metadata of a present key is its stored field, or `0` for a
/// payload stored in its slot word; `expected_present == false` expects the
/// key absent, `new_present == false` removes the key.
///
/// - `Ok`: swapped (or removed); nothing written.
/// - `Mismatch`: the current metadata is not `expected`; the map is unchanged;
///   `*observed_present_out` and, when present, `*observed_meta_out` written.
/// - `MetaOverflow`: the comparison held but `new_meta` is wider than 24 bits.
/// - `NoMetaField`: the comparison held but the value has no metadata field:
///   the key is absent (metadata alone cannot create it), or `new_meta` is
///   non-zero for a payload stored in its slot word.
/// - `InvalidArgument`: a null `map`.
///
/// Refusals leave the map unchanged and write nothing.
///
/// # Safety
///
/// `map` must be null or a live handle; `observed_present_out` and
/// `observed_meta_out` null or writable.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)] // the `u64` map's C signature, metadata for words
pub unsafe extern "C" fn expanse_sync_blob_map_compare_exchange_meta(
    map: *const SyncExpanseBlobMap,
    key: u64,
    expected_present: bool,
    expected_meta: u32,
    new_present: bool,
    new_meta: u32,
    observed_present_out: *mut bool,
    observed_meta_out: *mut u32,
) -> ExpanseBlobStatus {
    // SAFETY: null or live handle per contract.
    let Some(m) = (unsafe { map.as_ref() }) else {
        return ExpanseBlobStatus::InvalidArgument;
    };
    let res = m.compare_exchange_meta(
        key,
        expected_present.then_some(expected_meta),
        new_present.then_some(new_meta),
    );
    match res {
        Ok(_) => ExpanseBlobStatus::Ok,
        Err(CompareExchangeError::Unstorable(cause)) => cause.into(),
        Err(CompareExchangeError::Mismatch(seen)) => {
            // SAFETY: outputs null or writable per contract.
            unsafe {
                put(observed_present_out, seen.is_some());
                if let Some(meta) = seen {
                    put(observed_meta_out, meta);
                }
            }
            ExpanseBlobStatus::Mismatch
        }
    }
}

/// Stores `data` with `hot_meta` under `key` only if `key` is absent, as one
/// linearizable step.
///
/// - `Ok`: the key was absent and the value is stored; nothing written.
/// - `Mismatch`: the key was present and is left untouched; its payload is
///   in `buf[..*len_out]` and its metadata in `*meta_out`.
/// - `BufferTooSmall`: the key was present and its payload is longer than
///   `buf_len`: nothing stored, nothing copied, `*len_out` and `*meta_out`
///   written from the same exchange. A too-small buffer never coexists with
///   an insert: the buffer is written only when nothing was stored.
/// - A refusal (`MetaOverflow`, `AllocationFailed`, `CapRefused`,
///   `ArenaFull`): the key was absent and `data` cannot be stored; the map
///   is unchanged and nothing written.
/// - `InvalidArgument`: a null `map`, null `data` with `len > 0`, `len` above
///   `isize::MAX`, or a null `buf` with `buf_len > 0` or `buf_len` above
///   `isize::MAX`; nothing done.
///
/// Of any number of callers racing on one absent key, exactly one gets `Ok`.
///
/// # Safety
///
/// `map` must be null or a live handle; `data` readable for `len` bytes
/// (null only when `len == 0`); `buf` null or writable for `buf_len` bytes,
/// and it may overlap `data`; `len_out` and `meta_out` null or writable.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)] // insert's arguments plus the caller buffer
pub unsafe extern "C" fn expanse_sync_blob_map_get_or_insert(
    map: *const SyncExpanseBlobMap,
    key: u64,
    data: *const u8,
    len: usize,
    hot_meta: u32,
    buf: *mut u8,
    buf_len: usize,
    len_out: *mut usize,
    meta_out: *mut u32,
) -> ExpanseBlobStatus {
    // SAFETY: null or live handle, and `data` readable for `len` bytes, per
    // contract.
    let (Some(m), Some(d)) = (unsafe { map.as_ref() }, unsafe { bytes(data.cast(), len) }) else {
        return ExpanseBlobStatus::InvalidArgument;
    };
    if !blob_buf_ok(buf, buf_len) {
        return ExpanseBlobStatus::InvalidArgument;
    }
    // The borrow of `data` ends with this call.
    match m.get_or_insert(key, d, hot_meta) {
        Ok(None) => ExpanseBlobStatus::Ok,
        // SAFETY: outputs null or writable, `buf` valid for `buf_len` bytes,
        // per contract and `blob_buf_ok`.
        Ok(Some((present, meta))) => unsafe {
            put_blob_observed(&present, meta, buf, buf_len, len_out, meta_out)
        },
        Err(cause) => cause.into(),
    }
}
