//! The modern `expanse_*` C API (header: `include/expanse.h`).
//!
//! Additive to the legacy `Judy*` compat surface in `lib.rs`, which never
//! changes semantics. Everything here is a thin ABI translation over the
//! core crate: typed opaque handles, plain value returns instead of
//! `JError_t` out-parameters, and the capabilities classic libjudy has no
//! equivalent for — rank/select on every ordered type, byte-exact memory
//! accounting, and the concurrent one-writer/many-reader containers.
//!
//! Slot pointers follow the classic JudyL contract: valid until the next
//! structural mutation of the container.

use core::ffi::{CStr, c_char, c_void};
use expanse_trie::bytesmap::ExpanseBytesMap;
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use expanse_trie::strmap::ExpanseStrMap;
use expanse_trie::sync::{MapReader, SetReader, SyncExpanseMap, SyncExpanseSet};

/// Writes `v` through `out` when `out` is non-null.
#[inline]
unsafe fn put(out: *mut u64, v: u64) {
    if !out.is_null() {
        // SAFETY: non-null, caller-provided writable u64 per the C contract.
        unsafe { *out = v };
    }
}

/// Reads a NUL-terminated C string argument into a byte slice.
#[inline]
unsafe fn cstr<'a>(key: *const c_char) -> Option<&'a [u8]> {
    if key.is_null() {
        return None;
    }
    // SAFETY: caller guarantees a valid NUL-terminated C string.
    let s = unsafe { CStr::from_ptr(key) };
    Some(s.to_bytes())
}

/// Copies a byte slice plus terminating NUL into the caller's buffer.
/// Returns false if the buffer is too small.
#[inline]
unsafe fn write_cstr_buf(key_out: *mut c_char, buf_len: usize, bytes: &[u8]) -> bool {
    if key_out.is_null() || buf_len <= bytes.len() {
        return false;
    }
    // SAFETY: key_out is non-null and writable for buf_len > bytes.len() bytes.
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr().cast::<c_char>(), key_out, bytes.len());
        *key_out.add(bytes.len()) = 0;
    }
    true
}

/// Reads a `(pointer, length)` byte-string argument. A null pointer is
/// legal only for the empty key.
#[inline]
unsafe fn bytes<'a>(key: *const c_void, len: usize) -> Option<&'a [u8]> {
    if key.is_null() {
        return (len == 0).then_some(&[]);
    }
    // SAFETY: caller guarantees `len` readable bytes at `key`.
    Some(unsafe { core::slice::from_raw_parts(key.cast::<u8>(), len) })
}

/// Library version as a NUL-terminated string.
#[unsafe(no_mangle)]
pub extern "C" fn expanse_version() -> *const c_char {
    concat!(env!("EXPANSE_VERSION_FULL"), "\0")
        .as_ptr()
        .cast::<c_char>()
}

/// Generates the `_new`/`_free` pair plus the shared accessors for a
/// single-threaded container type.
macro_rules! container {
    ($rust:ty, $new:ident, $free:ident, $len:ident, $mem:ident, $clear:ident, $what:literal) => {
        #[doc = concat!("Creates an empty ", $what, ".")]
        #[unsafe(no_mangle)]
        pub extern "C" fn $new() -> *mut $rust {
            Box::into_raw(Box::new(<$rust>::new()))
        }

        #[doc = concat!("Frees a ", $what, " and everything in it. Null is a no-op.")]
        ///
        /// # Safety
        ///
        /// `h` must come from the matching `_new` and not be used after.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $free(h: *mut $rust) {
            if !h.is_null() {
                // SAFETY: handle from the matching `_new` per contract.
                drop(unsafe { Box::from_raw(h) });
            }
        }

        #[doc = concat!("Number of keys in the ", $what, " (0 for a null handle).")]
        ///
        /// # Safety
        ///
        /// `h` must be null or a live handle.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $len(h: *const $rust) -> u64 {
            // SAFETY: null or live handle per contract.
            unsafe { h.as_ref() }.map_or(0, <$rust>::len)
        }

        #[doc = concat!("Heap bytes used by the ", $what, " (0 for a null handle).")]
        ///
        /// # Safety
        ///
        /// `h` must be null or a live handle.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $mem(h: *const $rust) -> usize {
            // SAFETY: null or live handle per contract.
            unsafe { h.as_ref() }.map_or(0, <$rust>::mem_used)
        }

        #[doc = concat!("Removes every key from the ", $what, ".")]
        ///
        /// # Safety
        ///
        /// `h` must be null or a live handle.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $clear(h: *mut $rust) {
            // SAFETY: null or live handle per contract.
            if let Some(c) = unsafe { h.as_mut() } {
                c.clear();
            }
        }
    };
}

// ---------------------------------------------------------------------
// expanse_set_t — ordered set of u64 keys (cf. Judy1)
// ---------------------------------------------------------------------

container!(
    ExpanseSet,
    expanse_set_new,
    expanse_set_free,
    expanse_set_len,
    expanse_set_mem_used,
    expanse_set_clear,
    "set"
);

/// Inserts `key`; true if it was newly inserted.
///
/// # Safety
///
/// `set` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_set_insert(set: *mut ExpanseSet, key: u64) -> bool {
    // SAFETY: null or live handle per contract.
    unsafe { set.as_mut() }.is_some_and(|s| s.insert(key))
}

/// Removes `key`; true if it was present.
///
/// # Safety
///
/// Same contract as [`expanse_set_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_set_remove(set: *mut ExpanseSet, key: u64) -> bool {
    // SAFETY: null or live handle per contract.
    unsafe { set.as_mut() }.is_some_and(|s| s.remove(key))
}

/// Membership test.
///
/// # Safety
///
/// Same contract as [`expanse_set_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_set_contains(set: *const ExpanseSet, key: u64) -> bool {
    // SAFETY: null or live handle per contract.
    if set.is_null() {
        return false;
    }
    // SAFETY: set is non-null and points to a live ExpanseSet per contract.
    unsafe { (*set).contains(key) }
}

/// Generates a set navigation entry point returning `bool` + `key_out`.
macro_rules! set_nav {
    ($name:ident, $method:ident, $doc:literal) => {
        #[doc = $doc]
        ///
        /// # Safety
        ///
        /// `set` must be null or a live handle; `key_out` null or writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(set: *const ExpanseSet, key: u64, out: *mut u64) -> bool {
            // SAFETY: null or live handle per contract.
            let Some(s) = (unsafe { set.as_ref() }) else {
                return false;
            };
            match s.$method(key) {
                // SAFETY: `out` null or writable per contract.
                Some(k) => unsafe {
                    put(out, k);
                    true
                },
                None => false,
            }
        }
    };
}

set_nav!(
    expanse_set_next_at_or_after,
    next_at_or_after,
    "Smallest key >= `key`."
);
set_nav!(expanse_set_next_after, next_after, "Smallest key > `key`.");
set_nav!(
    expanse_set_prev_at_or_before,
    prev_at_or_before,
    "Largest key <= `key`."
);
set_nav!(expanse_set_prev_before, prev_before, "Largest key < `key`.");

/// Smallest key in the set.
///
/// # Safety
///
/// `set` must be null or a live handle; `out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_set_first(set: *const ExpanseSet, out: *mut u64) -> bool {
    // SAFETY: null or live handle per contract.
    let Some(k) = (unsafe { set.as_ref() }).and_then(ExpanseSet::first) else {
        return false;
    };
    // SAFETY: `out` null or writable per contract.
    unsafe { put(out, k) };
    true
}

/// Largest key in the set.
///
/// # Safety
///
/// Same contract as [`expanse_set_first`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_set_last(set: *const ExpanseSet, out: *mut u64) -> bool {
    // SAFETY: null or live handle per contract.
    let Some(k) = (unsafe { set.as_ref() }).and_then(ExpanseSet::last) else {
        return false;
    };
    // SAFETY: `out` null or writable per contract.
    unsafe { put(out, k) };
    true
}

/// Number of keys strictly below `key` (O(depth) rank).
///
/// # Safety
///
/// `set` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_set_count_below(set: *const ExpanseSet, key: u64) -> u64 {
    // SAFETY: null or live handle per contract.
    unsafe { set.as_ref() }.map_or(0, |s| s.count_below(key))
}

/// Number of keys in the inclusive range `[lo, hi]` (empty if `lo > hi`).
///
/// # Safety
///
/// `set` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_set_count_range(set: *const ExpanseSet, lo: u64, hi: u64) -> u64 {
    if lo > hi {
        return 0;
    }
    // SAFETY: null or live handle per contract.
    unsafe { set.as_ref() }.map_or(0, |s| s.count_range(lo..=hi))
}

/// The key with exactly `n` keys below it (0-based select).
///
/// # Safety
///
/// `set` must be null or a live handle; `out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_set_by_count(
    set: *const ExpanseSet,
    n: u64,
    out: *mut u64,
) -> bool {
    // SAFETY: null or live handle per contract.
    let Some(k) = (unsafe { set.as_ref() }).and_then(|s| s.by_count(n)) else {
        return false;
    };
    // SAFETY: `out` null or writable per contract.
    unsafe { put(out, k) };
    true
}

// ---------------------------------------------------------------------
// expanse_map_t — ordered u64 -> u64 map (cf. JudyL)
// ---------------------------------------------------------------------

container!(
    ExpanseMap,
    expanse_map_new,
    expanse_map_free,
    expanse_map_len,
    expanse_map_mem_used,
    expanse_map_clear,
    "map"
);

/// Stores `key -> value`. Returns true when the key is new; when it
/// replaced an existing entry, writes the old value through `old_out`
/// (if non-null) and returns false.
///
/// # Safety
///
/// `map` must be null or a live handle; `old_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_map_insert(
    map: *mut ExpanseMap,
    key: u64,
    value: u64,
    old_out: *mut u64,
) -> bool {
    // SAFETY: null or live handle per contract.
    let Some(m) = (unsafe { map.as_mut() }) else {
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

/// Reads `key`'s value into `value_out`; false if absent.
///
/// # Safety
///
/// `map` must be null or a live handle; `value_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_map_get(
    map: *const ExpanseMap,
    key: u64,
    value_out: *mut u64,
) -> bool {
    // SAFETY: null or live handle per contract.
    if map.is_null() {
        return false;
    }
    // SAFETY: map is non-null and points to a live ExpanseMap per contract.
    let Some(v) = (unsafe { (*map).get(key) }) else {
        return false;
    };
    // SAFETY: `value_out` null or writable per contract.
    unsafe { put(value_out, v) };
    true
}

/// Removes `key`, reporting its value through `old_out`; false if absent.
///
/// # Safety
///
/// `map` must be null or a live handle; `old_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_map_remove(
    map: *mut ExpanseMap,
    key: u64,
    old_out: *mut u64,
) -> bool {
    // SAFETY: null or live handle per contract.
    let Some(v) = (unsafe { map.as_mut() }).and_then(|m| m.remove(key)) else {
        return false;
    };
    // SAFETY: `old_out` null or writable per contract.
    unsafe { put(old_out, v) };
    true
}

/// Writable pointer to `key`'s value slot, or null if absent. Valid
/// until the next structural mutation.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_map_slot(map: *mut ExpanseMap, key: u64) -> *mut u64 {
    // SAFETY: null or live handle per contract.
    unsafe { map.as_mut() }
        .and_then(|m| m.get_value_slot(key))
        .map_or(core::ptr::null_mut(), core::ptr::NonNull::as_ptr)
}

/// Inserts `key` with value 0 if absent (an existing value is kept) and
/// returns its writable value slot — the classic `JudyLIns` contract in
/// one tree walk. Null only for a null handle.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_map_ins_slot(map: *mut ExpanseMap, key: u64) -> *mut u64 {
    // SAFETY: null or live handle per contract.
    unsafe { map.as_mut() }.map_or(core::ptr::null_mut(), |m| m.ins_slot(key).as_ptr())
}

/// Generates a map navigation entry point returning `bool` + key/value.
macro_rules! map_nav {
    ($name:ident, $method:ident, $doc:literal) => {
        #[doc = $doc]
        ///
        /// # Safety
        ///
        /// `map` must be null or a live handle; the out-pointers null or
        /// writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            map: *const ExpanseMap,
            key: u64,
            key_out: *mut u64,
            value_out: *mut u64,
        ) -> bool {
            // SAFETY: null or live handle per contract.
            let Some((k, v)) = (unsafe { map.as_ref() }).and_then(|m| m.$method(key)) else {
                return false;
            };
            // SAFETY: out-pointers null or writable per contract.
            unsafe {
                put(key_out, k);
                put(value_out, v);
            }
            true
        }
    };
}

map_nav!(
    expanse_map_next_at_or_after,
    next_at_or_after,
    "Smallest entry with key >= `key`."
);
map_nav!(
    expanse_map_next_after,
    next_after,
    "Smallest entry with key > `key`."
);
map_nav!(
    expanse_map_prev_at_or_before,
    prev_at_or_before,
    "Largest entry with key <= `key`."
);
map_nav!(
    expanse_map_prev_before,
    prev_before,
    "Largest entry with key < `key`."
);

/// Smallest entry in the map.
///
/// # Safety
///
/// `map` must be null or a live handle; out-pointers null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_map_first(
    map: *const ExpanseMap,
    key_out: *mut u64,
    value_out: *mut u64,
) -> bool {
    // SAFETY: null or live handle per contract.
    let Some((k, v)) = (unsafe { map.as_ref() }).and_then(ExpanseMap::first) else {
        return false;
    };
    // SAFETY: out-pointers null or writable per contract.
    unsafe {
        put(key_out, k);
        put(value_out, v);
    }
    true
}

/// Largest entry in the map.
///
/// # Safety
///
/// Same contract as [`expanse_map_first`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_map_last(
    map: *const ExpanseMap,
    key_out: *mut u64,
    value_out: *mut u64,
) -> bool {
    // SAFETY: null or live handle per contract.
    let Some((k, v)) = (unsafe { map.as_ref() }).and_then(ExpanseMap::last) else {
        return false;
    };
    // SAFETY: out-pointers null or writable per contract.
    unsafe {
        put(key_out, k);
        put(value_out, v);
    }
    true
}

/// Number of entries strictly below `key` (O(depth) rank).
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_map_count_below(map: *const ExpanseMap, key: u64) -> u64 {
    // SAFETY: null or live handle per contract.
    unsafe { map.as_ref() }.map_or(0, |m| m.count_below(key))
}

/// Number of entries in the inclusive range `[lo, hi]`.
///
/// # Safety
///
/// `map` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_map_count_range(map: *const ExpanseMap, lo: u64, hi: u64) -> u64 {
    if lo > hi {
        return 0;
    }
    // SAFETY: null or live handle per contract.
    unsafe { map.as_ref() }.map_or(0, |m| m.count_range(lo..=hi))
}

/// The entry with exactly `n` entries below it (0-based select).
///
/// # Safety
///
/// `map` must be null or a live handle; out-pointers null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_map_by_count(
    map: *const ExpanseMap,
    n: u64,
    key_out: *mut u64,
    value_out: *mut u64,
) -> bool {
    // SAFETY: null or live handle per contract.
    let Some((k, v)) = (unsafe { map.as_ref() }).and_then(|m| m.by_count(n)) else {
        return false;
    };
    // SAFETY: out-pointers null or writable per contract.
    unsafe {
        put(key_out, k);
        put(value_out, v);
    }
    true
}

// ---------------------------------------------------------------------
// expanse_bytesmap_t — unordered bytes -> u64 (cf. JudyHS)
// ---------------------------------------------------------------------

container!(
    ExpanseBytesMap,
    expanse_bytesmap_new,
    expanse_bytesmap_free,
    expanse_bytesmap_len,
    expanse_bytesmap_mem_used,
    expanse_bytesmap_clear,
    "byte-string map"
);

/// Stores `key -> value` (see [`expanse_map_insert`] for the return
/// convention). Embedded NULs are ordinary bytes.
///
/// # Safety
///
/// `map` null or live; `key` readable for `len` bytes (null only when
/// `len == 0`); `old_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_bytesmap_insert(
    map: *mut ExpanseBytesMap,
    key: *const c_void,
    len: usize,
    value: u64,
    old_out: *mut u64,
) -> bool {
    // SAFETY: forwarded C contract.
    unsafe {
        let (Some(m), Some(k)) = (map.as_mut(), bytes(key, len)) else {
            return false;
        };
        match m.insert(k, value) {
            Some(old) => {
                put(old_out, old);
                false
            }
            None => true,
        }
    }
}

/// Reads the byte string's value into `value_out`; false if absent.
///
/// # Safety
///
/// Same contract as [`expanse_bytesmap_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_bytesmap_get(
    map: *const ExpanseBytesMap,
    key: *const c_void,
    len: usize,
    value_out: *mut u64,
) -> bool {
    // SAFETY: forwarded C contract.
    unsafe {
        let (Some(m), Some(k)) = (map.as_ref(), bytes(key, len)) else {
            return false;
        };
        let Some(v) = m.get(k) else {
            return false;
        };
        put(value_out, v);
        true
    }
}

/// Removes the byte string, reporting its value; false if absent.
///
/// # Safety
///
/// Same contract as [`expanse_bytesmap_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_bytesmap_remove(
    map: *mut ExpanseBytesMap,
    key: *const c_void,
    len: usize,
    old_out: *mut u64,
) -> bool {
    // SAFETY: forwarded C contract.
    unsafe {
        let (Some(m), Some(k)) = (map.as_mut(), bytes(key, len)) else {
            return false;
        };
        let Some(v) = m.remove(k) else {
            return false;
        };
        put(old_out, v);
        true
    }
}

/// Writable value slot of the byte string, or null if absent.
///
/// # Safety
///
/// Same contract as [`expanse_bytesmap_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_bytesmap_slot(
    map: *mut ExpanseBytesMap,
    key: *const c_void,
    len: usize,
) -> *mut u64 {
    // SAFETY: forwarded C contract.
    unsafe {
        let (Some(m), Some(k)) = (map.as_mut(), bytes(key, len)) else {
            return core::ptr::null_mut();
        };
        m.get_value_slot(k)
            .map_or(core::ptr::null_mut(), core::ptr::NonNull::as_ptr)
    }
}

/// Inserts the byte string with value 0 if absent and returns its slot.
///
/// # Safety
///
/// Same contract as [`expanse_bytesmap_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_bytesmap_ins_slot(
    map: *mut ExpanseBytesMap,
    key: *const c_void,
    len: usize,
) -> *mut u64 {
    // SAFETY: forwarded C contract.
    unsafe {
        let (Some(m), Some(k)) = (map.as_mut(), bytes(key, len)) else {
            return core::ptr::null_mut();
        };
        m.ins_slot(k).as_ptr()
    }
}

// ---------------------------------------------------------------------
// expanse_strmap_t — ordered C-string -> u64 map (cf. JudySL)
// ---------------------------------------------------------------------

container!(
    ExpanseStrMap,
    expanse_strmap_new,
    expanse_strmap_free,
    expanse_strmap_len,
    expanse_strmap_mem_used,
    expanse_strmap_clear,
    "string map"
);

/// Stores `key -> value`. Returns true when the key is new; when it
/// replaced an existing entry, writes the old value through `old_out`
/// (if non-null) and returns false.
///
/// # Safety
///
/// `map` null or live; `key` valid NUL-terminated C string; `old_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_strmap_insert(
    map: *mut ExpanseStrMap,
    key: *const c_char,
    value: u64,
    old_out: *mut u64,
) -> bool {
    // SAFETY: forwarded C contract.
    unsafe {
        let (Some(m), Some(k)) = (map.as_mut(), cstr(key)) else {
            return false;
        };
        match m.insert(k, value) {
            Some(old) => {
                put(old_out, old);
                false
            }
            None => true,
        }
    }
}

/// Reads the string's value into `value_out`; false if absent.
///
/// # Safety
///
/// Same contract as [`expanse_strmap_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_strmap_get(
    map: *const ExpanseStrMap,
    key: *const c_char,
    value_out: *mut u64,
) -> bool {
    // SAFETY: forwarded C contract.
    unsafe {
        let (Some(m), Some(k)) = (map.as_ref(), cstr(key)) else {
            return false;
        };
        let Some(v) = m.get(k) else {
            return false;
        };
        put(value_out, v);
        true
    }
}

/// Removes the string, reporting its value; false if absent.
///
/// # Safety
///
/// Same contract as [`expanse_strmap_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_strmap_remove(
    map: *mut ExpanseStrMap,
    key: *const c_char,
    old_out: *mut u64,
) -> bool {
    // SAFETY: forwarded C contract.
    unsafe {
        let (Some(m), Some(k)) = (map.as_mut(), cstr(key)) else {
            return false;
        };
        let Some(v) = m.remove(k) else {
            return false;
        };
        put(old_out, v);
        true
    }
}

/// Writable value slot of the string, or null if absent.
///
/// # Safety
///
/// Same contract as [`expanse_strmap_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_strmap_slot(
    map: *mut ExpanseStrMap,
    key: *const c_char,
) -> *mut u64 {
    // SAFETY: forwarded C contract.
    unsafe {
        let (Some(m), Some(k)) = (map.as_mut(), cstr(key)) else {
            return core::ptr::null_mut();
        };
        m.get_value_slot(k)
            .map_or(core::ptr::null_mut(), core::ptr::NonNull::as_ptr)
    }
}

/// Inserts the string with value 0 if absent and returns its slot.
///
/// # Safety
///
/// Same contract as [`expanse_strmap_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_strmap_ins_slot(
    map: *mut ExpanseStrMap,
    key: *const c_char,
) -> *mut u64 {
    // SAFETY: forwarded C contract.
    unsafe {
        let (Some(m), Some(k)) = (map.as_mut(), cstr(key)) else {
            return core::ptr::null_mut();
        };
        m.ins_slot(k).as_ptr()
    }
}

/// Generates a string map navigation entry point returning `bool` + key/value.
macro_rules! strmap_nav {
    ($name:ident, $method:ident, $doc:literal) => {
        #[doc = $doc]
        ///
        /// # Safety
        ///
        /// `map` must be null or a live handle; `key` a NUL-terminated C string;
        /// `key_out` non-null with `buf_len` bytes capacity; `value_out` null or writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            map: *mut ExpanseStrMap,
            key: *const c_char,
            key_out: *mut c_char,
            buf_len: usize,
            value_out: *mut u64,
        ) -> bool {
            // SAFETY: null or live handle per contract.
            let (Some(m), Some(k)) = (unsafe { map.as_mut() }, unsafe { cstr(key) }) else {
                return false;
            };
            let Some((found_k, slot)) = m.$method(k) else {
                return false;
            };
            // SAFETY: caller guarantees key_out writable for buf_len bytes.
            if !unsafe { write_cstr_buf(key_out, buf_len, &found_k) } {
                return false;
            }
            // SAFETY: slot is valid until next mutation; value_out null or writable.
            unsafe {
                put(value_out, *slot.as_ptr());
            }
            true
        }
    };
}

strmap_nav!(
    expanse_strmap_next_at_or_after,
    next_at_or_after,
    "Smallest entry with key >= `key`."
);
strmap_nav!(
    expanse_strmap_next_after,
    next_after,
    "Smallest entry with key > `key`."
);
strmap_nav!(
    expanse_strmap_prev_at_or_before,
    prev_at_or_before,
    "Largest entry with key <= `key`."
);
strmap_nav!(
    expanse_strmap_prev_before,
    prev_before,
    "Largest entry with key < `key`."
);

/// Smallest entry in the string map.
///
/// # Safety
///
/// `map` null or live handle; `key_out` non-null with `buf_len` bytes capacity; `value_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_strmap_first(
    map: *mut ExpanseStrMap,
    key_out: *mut c_char,
    buf_len: usize,
    value_out: *mut u64,
) -> bool {
    // SAFETY: forwarded C contract.
    let Some(m) = (unsafe { map.as_mut() }) else {
        return false;
    };
    let Some((found_k, slot)) = m.first() else {
        return false;
    };
    // SAFETY: caller guarantees key_out writable for buf_len bytes.
    if !unsafe { write_cstr_buf(key_out, buf_len, &found_k) } {
        return false;
    }
    // SAFETY: slot valid until next mutation.
    unsafe { put(value_out, *slot.as_ptr()) };
    true
}

/// Largest entry in the string map.
///
/// # Safety
///
/// Same contract as [`expanse_strmap_first`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_strmap_last(
    map: *mut ExpanseStrMap,
    key_out: *mut c_char,
    buf_len: usize,
    value_out: *mut u64,
) -> bool {
    // SAFETY: forwarded C contract.
    let Some(m) = (unsafe { map.as_mut() }) else {
        return false;
    };
    let Some((found_k, slot)) = m.last() else {
        return false;
    };
    // SAFETY: caller guarantees key_out writable for buf_len bytes.
    if !unsafe { write_cstr_buf(key_out, buf_len, &found_k) } {
        return false;
    }
    // SAFETY: slot valid until next mutation.
    unsafe { put(value_out, *slot.as_ptr()) };
    true
}

// ---------------------------------------------------------------------
// Concurrent containers — one writer, lock-free readers
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

/// Lock-free membership test through a registered reader.
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

/// Lock-free lookup through a registered reader.
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

/// The version string as a Rust `&str` (test helper / Rust consumers).
#[must_use]
pub fn version_str() -> &'static str {
    // SAFETY: the constant is NUL-terminated and valid UTF-8.
    unsafe { CStr::from_ptr(expanse_version()) }
        .to_str()
        .expect("valid UTF-8 version")
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ptr::null_mut;

    #[test]
    fn set_surface() {
        // SAFETY: handles managed per the C contract throughout.
        unsafe {
            let s = expanse_set_new();
            for k in 0u64..100 {
                assert!(expanse_set_insert(s, k * 7));
            }
            assert!(!expanse_set_insert(s, 0));
            assert!(expanse_set_contains(s, 693)); // 99 * 7, the last key
            assert!(!expanse_set_contains(s, 700)); // 100 * 7, one past
            assert!(!expanse_set_contains(s, 694));
            assert_eq!(expanse_set_len(s), 100);
            assert!(expanse_set_mem_used(s) > 0);

            let mut out = 0u64;
            assert!(expanse_set_first(s, &raw mut out) && out == 0);
            assert!(expanse_set_last(s, &raw mut out) && out == 99 * 7);
            assert!(expanse_set_next_at_or_after(s, 8, &raw mut out) && out == 14);
            assert!(expanse_set_next_after(s, 14, &raw mut out) && out == 21);
            assert!(expanse_set_prev_at_or_before(s, 20, &raw mut out) && out == 14);
            assert!(expanse_set_prev_before(s, 14, &raw mut out) && out == 7);
            assert!(!expanse_set_next_after(s, 99 * 7, &raw mut out));

            assert_eq!(expanse_set_count_below(s, 70), 10);
            assert_eq!(expanse_set_count_range(s, 0, 70), 11);
            assert_eq!(expanse_set_count_range(s, 70, 0), 0);
            assert!(expanse_set_by_count(s, 3, &raw mut out) && out == 21);
            assert!(!expanse_set_by_count(s, 1000, &raw mut out));

            assert!(expanse_set_remove(s, 0));
            assert!(!expanse_set_remove(s, 0));
            expanse_set_clear(s);
            assert_eq!(expanse_set_len(s), 0);
            assert_eq!(expanse_set_mem_used(s), 0);
            expanse_set_free(s);
            // Null handles are inert, never a crash.
            assert!(!expanse_set_contains(null_mut(), 1));
            assert_eq!(expanse_set_len(null_mut()), 0);
            expanse_set_free(null_mut());
        }
    }

    #[test]
    fn map_surface_and_slots() {
        // SAFETY: handles managed per the C contract throughout.
        unsafe {
            let m = expanse_map_new();
            let mut old = 0u64;
            assert!(expanse_map_insert(m, 5, 50, &raw mut old));
            assert!(!expanse_map_insert(m, 5, 55, &raw mut old) && old == 50);

            let mut v = 0u64;
            assert!(expanse_map_get(m, 5, &raw mut v) && v == 55);
            assert!(!expanse_map_get(m, 6, &raw mut v));

            // Slot conventions.
            let slot = expanse_map_ins_slot(m, 9);
            assert!(!slot.is_null() && *slot == 0);
            *slot = 99;
            assert!(expanse_map_get(m, 9, &raw mut v) && v == 99);
            let slot = expanse_map_slot(m, 9);
            assert_eq!(*slot, 99);
            assert!(expanse_map_slot(m, 1000).is_null());
            // ins_slot keeps an existing value.
            assert_eq!(*expanse_map_ins_slot(m, 9), 99);

            for k in 100u64..200 {
                expanse_map_insert(m, k, !k, null_mut());
            }
            let (mut k, mut val) = (0u64, 0u64);
            assert!(expanse_map_first(m, &raw mut k, &raw mut val) && k == 5 && val == 55);
            assert!(expanse_map_last(m, &raw mut k, &raw mut val) && k == 199);
            assert!(expanse_map_next_at_or_after(m, 100, &raw mut k, &raw mut val) && k == 100);
            assert!(expanse_map_next_after(m, 100, &raw mut k, null_mut()) && k == 101);
            assert!(expanse_map_prev_before(m, 100, &raw mut k, null_mut()) && k == 9);
            assert_eq!(expanse_map_count_range(m, 100, 199), 100);
            assert!(expanse_map_by_count(m, 0, &raw mut k, null_mut()) && k == 5);

            assert!(expanse_map_remove(m, 5, &raw mut old) && old == 55);
            assert!(!expanse_map_remove(m, 5, &raw mut old));
            expanse_map_free(m);
        }
    }

    #[test]
    fn bytesmap_surface() {
        // SAFETY: handles managed per the C contract throughout.
        unsafe {
            let m = expanse_bytesmap_new();
            let key = b"a\0b";
            let kp = key.as_ptr().cast::<c_void>();
            assert!(expanse_bytesmap_insert(m, kp, key.len(), 1, null_mut()));
            let mut v = 0u64;
            assert!(expanse_bytesmap_get(m, kp, key.len(), &raw mut v) && v == 1);
            // A prefix is a different key; embedded NUL is data.
            assert!(!expanse_bytesmap_get(m, kp, 1, &raw mut v));
            // Empty key, null pointer.
            assert!(expanse_bytesmap_insert(m, null_mut(), 0, 7, null_mut()));
            assert!(expanse_bytesmap_get(m, null_mut(), 0, &raw mut v) && v == 7);
            assert_eq!(expanse_bytesmap_len(m), 2);

            let slot = expanse_bytesmap_ins_slot(m, kp, key.len());
            assert_eq!(*slot, 1);
            *slot = 42;
            assert!(expanse_bytesmap_get(m, kp, key.len(), &raw mut v) && v == 42);
            assert!(expanse_bytesmap_slot(m, b"zz".as_ptr().cast(), 2).is_null());

            let mut old = 0u64;
            assert!(expanse_bytesmap_remove(m, kp, key.len(), &raw mut old) && old == 42);
            expanse_bytesmap_clear(m);
            assert_eq!(expanse_bytesmap_len(m), 0);
            assert_eq!(expanse_bytesmap_mem_used(m), 0);
            expanse_bytesmap_free(m);
        }
    }

    #[test]
    fn strmap_surface() {
        // SAFETY: handles managed per the C contract throughout.
        unsafe {
            let m = expanse_strmap_new();
            let k1 = c"apple".as_ptr();
            let k2 = c"banana".as_ptr();
            let k3 = c"cherry".as_ptr();
            let mut old = 0u64;

            assert!(expanse_strmap_insert(m, k1, 100, &raw mut old));
            assert!(expanse_strmap_insert(m, k3, 300, null_mut()));
            assert!(expanse_strmap_insert(m, k2, 200, null_mut()));
            assert_eq!(expanse_strmap_len(m), 3);
            assert!(expanse_strmap_mem_used(m) > 0);

            // Re-insert updates and reports old value
            assert!(!expanse_strmap_insert(m, k1, 105, &raw mut old));
            assert_eq!(old, 100);

            let mut v = 0u64;
            assert!(expanse_strmap_get(m, k1, &raw mut v) && v == 105);
            assert!(expanse_strmap_get(m, k2, &raw mut v) && v == 200);
            assert!(!expanse_strmap_get(m, c"durian".as_ptr(), &raw mut v));

            // Slot conventions
            let slot = expanse_strmap_ins_slot(m, c"date".as_ptr());
            assert!(!slot.is_null() && *slot == 0);
            *slot = 400;
            assert!(expanse_strmap_get(m, c"date".as_ptr(), &raw mut v) && v == 400);
            let slot = expanse_strmap_slot(m, c"date".as_ptr());
            assert_eq!(*slot, 400);
            assert!(expanse_strmap_slot(m, c"fig".as_ptr()).is_null());

            // Navigation
            let mut buf = [0 as c_char; 64];
            let mut val = 0u64;
            assert!(expanse_strmap_first(
                m,
                buf.as_mut_ptr(),
                buf.len(),
                &raw mut val
            ));
            assert_eq!(CStr::from_ptr(buf.as_ptr()).to_bytes(), b"apple");
            assert_eq!(val, 105);

            assert!(expanse_strmap_last(
                m,
                buf.as_mut_ptr(),
                buf.len(),
                &raw mut val
            ));
            assert_eq!(CStr::from_ptr(buf.as_ptr()).to_bytes(), b"date");
            assert_eq!(val, 400);

            assert!(expanse_strmap_next_after(
                m,
                c"apple".as_ptr(),
                buf.as_mut_ptr(),
                buf.len(),
                &raw mut val
            ));
            assert_eq!(CStr::from_ptr(buf.as_ptr()).to_bytes(), b"banana");
            assert_eq!(val, 200);

            assert!(expanse_strmap_prev_before(
                m,
                c"cherry".as_ptr(),
                buf.as_mut_ptr(),
                buf.len(),
                &raw mut val
            ));
            assert_eq!(CStr::from_ptr(buf.as_ptr()).to_bytes(), b"banana");
            assert_eq!(val, 200);

            // Buffer too small returns false
            let mut tiny_buf = [0 as c_char; 3];
            assert!(!expanse_strmap_first(
                m,
                tiny_buf.as_mut_ptr(),
                tiny_buf.len(),
                &raw mut val
            ));

            assert!(expanse_strmap_remove(m, k1, &raw mut old) && old == 105);
            assert_eq!(expanse_strmap_len(m), 3);
            expanse_strmap_clear(m);
            assert_eq!(expanse_strmap_len(m), 0);
            assert_eq!(expanse_strmap_mem_used(m), 0);
            expanse_strmap_free(m);
        }
    }

    #[test]
    fn sync_surface_threaded() {
        // The capability classic Judy lacks: readers on other threads
        // while a writer mutates the same container.
        let s = expanse_sync_set_new();
        // SAFETY: handles managed per the C contract; the set outlives
        // every reader (all joined before the free).
        unsafe {
            for k in 0u64..500 {
                assert!(expanse_sync_set_insert(s, k));
            }
            assert_eq!(expanse_sync_set_len(s), 500);
            assert!(expanse_sync_set_contains(s, 499));

            let addr = s as usize;
            let readers: Vec<_> = (0..3)
                .map(|_| {
                    std::thread::spawn(move || {
                        let set = addr as *const SyncExpanseSet;
                        // The set outlives this thread (joined below,
                        // before the free); the reader is freed here,
                        // before its set.
                        let r = expanse_sync_set_reader_new(set);
                        let mut hits = 0;
                        for k in 0u64..500 {
                            if expanse_sync_set_reader_contains(r, k) {
                                hits += 1;
                            }
                        }
                        expanse_sync_set_reader_free(r);
                        hits
                    })
                })
                .collect();
            for (i, h) in readers.into_iter().enumerate() {
                assert_eq!(h.join().expect("reader thread"), 500, "reader {i}");
            }
            expanse_sync_set_free(s);

            let m = expanse_sync_map_new();
            let mut old = 0u64;
            assert!(expanse_sync_map_insert(m, 1, 10, &raw mut old));
            assert!(!expanse_sync_map_insert(m, 1, 11, &raw mut old) && old == 10);
            let r = expanse_sync_map_reader_new(m);
            let mut v = 0u64;
            assert!(expanse_sync_map_reader_get(r, 1, &raw mut v) && v == 11);
            assert!(!expanse_sync_map_reader_get(r, 2, &raw mut v));
            expanse_sync_map_reader_free(r);
            assert!(expanse_sync_map_get(m, 1, &raw mut v) && v == 11);
            assert!(expanse_sync_map_remove(m, 1, &raw mut old) && old == 11);
            assert_eq!(expanse_sync_map_len(m), 0);
            expanse_sync_map_free(m);
        }
    }

    #[test]
    fn version_is_semver() {
        let v = version_str();
        let pkg = env!("CARGO_PKG_VERSION");
        assert!(
            v.contains(pkg),
            "expected version '{v}' to contain package version '{pkg}'"
        );
    }
}
