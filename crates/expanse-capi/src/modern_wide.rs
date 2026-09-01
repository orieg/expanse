//! The 64-bit-only half of the modern `expanse_*` C API.
//!
//! Split out of [`super`] so the width-parametric core (`expanse_set_*`,
//! `expanse_map_*`) still compiles on 32-bit targets, where the engine
//! has no byte-string, string, or concurrent containers. See the surface
//! matrix in `docs/COMPAT.md`; the omitted symbols are absent from the
//! 32-bit library, not stubbed.

use super::{bytes, cstr, put, write_cstr_buf};
#[cfg(not(feature = "std"))]
use crate::core_alloc::boxed::Box;
#[cfg(not(feature = "std"))]
use crate::core_alloc::vec::Vec;
use core::ffi::{c_char, c_void};
use expanse_trie::bytesmap::ExpanseBytesMap;
use expanse_trie::strmap::ExpanseStrMap;

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
// Truncation-aware string navigation (`_ex` variants)
// ---------------------------------------------------------------------
//
// The plain expanse_strmap_first/last/next/prev entry points return `false`
// for BOTH "no more keys" and "buffer too small", so a binding cannot tell a
// missing key from a truncated one and may silently drop long keys. These
// `_ex` variants disambiguate via an explicit status and report the buffer
// size the key needs through `required_len`. The original symbols are
// unchanged.

/// Status returned by the truncation-aware `expanse_strmap_*_ex` navigation
/// functions. ABI: a C `enum` (int).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExpanseStrNavStatus {
    /// A key was found and written to `key_out` (`*value_out` set if non-null).
    Ok = 0,
    /// No key matched; nothing was written.
    NotFound = 1,
    /// A key was found but `key_out`/`buf_len` was too small; `*required_len`
    /// (if non-null) holds the byte length needed (key length + 1 for the NUL).
    BufferTooSmall = 2,
}

/// Shared body for the `_ex` navigation entry points: reports the required
/// buffer size and distinguishes not-found from buffer-too-small.
///
/// # Safety
///
/// `key_out` null or writable for `buf_len` bytes; `required_len`/`value_out`
/// null or writable; `found`'s slot is valid until the next map mutation.
unsafe fn strmap_nav_ex(
    found: Option<(Vec<u8>, core::ptr::NonNull<u64>)>,
    key_out: *mut c_char,
    buf_len: usize,
    required_len: *mut usize,
    value_out: *mut u64,
) -> ExpanseStrNavStatus {
    let Some((found_k, slot)) = found else {
        return ExpanseStrNavStatus::NotFound;
    };
    let needed = found_k.len() + 1; // payload bytes + terminating NUL
    if !required_len.is_null() {
        // SAFETY: caller-provided writable size_t per contract.
        unsafe { *required_len = needed };
    }
    if key_out.is_null() || buf_len < needed {
        return ExpanseStrNavStatus::BufferTooSmall;
    }
    // SAFETY: key_out is non-null and writable for buf_len >= needed bytes.
    unsafe {
        core::ptr::copy_nonoverlapping(found_k.as_ptr().cast::<c_char>(), key_out, found_k.len());
        *key_out.add(found_k.len()) = 0;
        // SAFETY: slot valid until next mutation; value_out null or writable.
        put(value_out, *slot.as_ptr());
    }
    ExpanseStrNavStatus::Ok
}

/// Generates a truncation-aware string navigation entry point (by-key).
macro_rules! strmap_nav_ex {
    ($name:ident, $method:ident, $doc:literal) => {
        #[doc = $doc]
        ///
        /// On success writes the key to `key_out` and the value to `value_out`;
        /// on buffer-too-small sets `*required_len` to the needed size. See
        /// [`ExpanseStrNavStatus`].
        ///
        /// # Safety
        ///
        /// `map` null or live; `key` a NUL-terminated C string; `key_out` null
        /// or writable for `buf_len` bytes; `required_len`/`value_out` null or
        /// writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            map: *mut ExpanseStrMap,
            key: *const c_char,
            key_out: *mut c_char,
            buf_len: usize,
            required_len: *mut usize,
            value_out: *mut u64,
        ) -> ExpanseStrNavStatus {
            // SAFETY: null or live handle + NUL-terminated key per contract.
            let (Some(m), Some(k)) = (unsafe { map.as_mut() }, unsafe { cstr(key) }) else {
                return ExpanseStrNavStatus::NotFound;
            };
            // SAFETY: out-pointers forwarded per contract.
            unsafe { strmap_nav_ex(m.$method(k), key_out, buf_len, required_len, value_out) }
        }
    };
}

strmap_nav_ex!(
    expanse_strmap_next_at_or_after_ex,
    next_at_or_after,
    "Smallest entry with key >= `key` (truncation-aware)."
);
strmap_nav_ex!(
    expanse_strmap_next_after_ex,
    next_after,
    "Smallest entry with key > `key` (truncation-aware)."
);
strmap_nav_ex!(
    expanse_strmap_prev_at_or_before_ex,
    prev_at_or_before,
    "Largest entry with key <= `key` (truncation-aware)."
);
strmap_nav_ex!(
    expanse_strmap_prev_before_ex,
    prev_before,
    "Largest entry with key < `key` (truncation-aware)."
);

/// Smallest entry in the string map (truncation-aware; see
/// [`ExpanseStrNavStatus`]).
///
/// # Safety
///
/// `map` null or live; `key_out` null or writable for `buf_len` bytes;
/// `required_len`/`value_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_strmap_first_ex(
    map: *mut ExpanseStrMap,
    key_out: *mut c_char,
    buf_len: usize,
    required_len: *mut usize,
    value_out: *mut u64,
) -> ExpanseStrNavStatus {
    // SAFETY: null or live handle per contract.
    let Some(m) = (unsafe { map.as_mut() }) else {
        return ExpanseStrNavStatus::NotFound;
    };
    // SAFETY: out-pointers forwarded per contract.
    unsafe { strmap_nav_ex(m.first(), key_out, buf_len, required_len, value_out) }
}

/// Largest entry in the string map (truncation-aware; see
/// [`ExpanseStrNavStatus`]).
///
/// # Safety
///
/// Same contract as [`expanse_strmap_first_ex`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_strmap_last_ex(
    map: *mut ExpanseStrMap,
    key_out: *mut c_char,
    buf_len: usize,
    required_len: *mut usize,
    value_out: *mut u64,
) -> ExpanseStrNavStatus {
    // SAFETY: null or live handle per contract.
    let Some(m) = (unsafe { map.as_mut() }) else {
        return ExpanseStrNavStatus::NotFound;
    };
    // SAFETY: out-pointers forwarded per contract.
    unsafe { strmap_nav_ex(m.last(), key_out, buf_len, required_len, value_out) }
}

// The concurrent containers additionally need `std` (see the child
// module's own note), so they are gated one level deeper (#558).
#[cfg(feature = "std")]
#[path = "modern_sync.rs"]
mod sync;
#[cfg(feature = "std")]
pub use sync::*;
