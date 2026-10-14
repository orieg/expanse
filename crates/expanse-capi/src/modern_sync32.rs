//! The 32-bit-only concurrent C surface: `expanse_sync32_*` over the
//! `sync32` wrapper (one writer, optimistic single-attempt readers).
//!
//! What Rust enforced with types — one writer (`split(&mut self)` + a
//! non-clonable handle), one reader handle per execution context (`&mut
//! self` methods) — C cannot. This surface therefore makes those
//! properties *structural* rather than checked: the writer is born with
//! the container and reached through an idempotent accessor, readers are
//! addressed by index into handles the container owns, and the wrapper
//! performs no atomic read-modify-write anywhere (the primary target,
//! `riscv32imc`, has none; the module's whole protocol is load/store plus
//! fences). The remaining contract — which execution context uses which
//! handle — lives in `include/expanse.h`, next to the declarations.
//!
//! Ownership: the `Sync32` container is boxed once and never moves, so the
//! `Writer32`/`Reader32` handles that borrow it can be lifetime-extended to
//! `'static` and stored beside it in one outer allocation. `free` drops the
//! handles (plain data) and then the container; every handle pointer
//! dangles from that point, as the header states.

use core::ffi::{c_char, c_int, c_void};

#[cfg(not(feature = "std"))]
use crate::core_alloc::{boxed::Box, vec::Vec};
#[cfg(target_pointer_width = "32")]
use crate::modern::CWord;
#[cfg(not(target_pointer_width = "32"))]
type CWord = u32;
use expanse_trie::sync32::{
    Busy, MUTATION_HEADROOM, Reader32, Sync32, SyncExpanseMap32, SyncExpanseSet32, WriteError,
    Writer32,
};
use expanse_trie::{ExpanseMap32, ExpanseSet32};

/// Status codes, in three documented bands (see `expanse.h`): outcomes
/// 0..16, refusals that leave the tree untouched 16..32, usage errors
/// 32..48. New values may be added within a band.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpanseSync32Status {
    /// The call completed; any out-parameter is written.
    Ok = 0,
    /// The key is absent (reads, removes, `try_contains`).
    NotFound = 1,
    /// The read overlapped an open write bracket and was abandoned.
    Busy = 2,
    /// Fewer than `MUTATION_HEADROOM` free node slots remain.
    ArenaFull = 16,
    /// The pending-reclamation list is nearly full and a reader is stalled
    /// inside a walk.
    ReclaimBacklog = 17,
    /// A NULL handle. A precondition violation, not a runtime condition:
    /// debug builds assert on it.
    NullHandle = 32,
}

/// Arena figures a writer can report without a bracket. Append-only:
/// callers pass `sizeof` and receive the prefix they know about.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ExpanseSync32Stats {
    /// Live entries.
    pub len: u64,
    /// Bytes retained by live nodes.
    pub mem_used: usize,
    /// Nodes parked awaiting reader quiescence.
    pub pending_len: usize,
    /// Bytes parked in the pending list.
    pub pending_bytes: usize,
    /// Free node slots in the fixed arena.
    pub free_slots: usize,
}

/// The number of free node slots a mutation must see before it is
/// attempted (`MUTATION_HEADROOM`), as a function so the value stays on
/// the library side.
#[unsafe(no_mangle)]
pub extern "C" fn expanse_sync32_mutation_headroom() -> usize {
    MUTATION_HEADROOM
}

/// A static, NUL-terminated name for `status`; "unknown" for any other
/// value. Takes an `int` so an out-of-band value from C is never an
/// invalid Rust enum.
#[unsafe(no_mangle)]
pub extern "C" fn expanse_sync32_status_str(status: c_int) -> *const c_char {
    let s: &'static [u8] = match status {
        0 => b"ok\0",
        1 => b"not_found\0",
        2 => b"busy\0",
        16 => b"arena_full\0",
        17 => b"reclaim_backlog\0",
        32 => b"null_handle\0",
        _ => b"unknown\0",
    };
    s.as_ptr().cast()
}

#[inline]
fn write_status(e: WriteError) -> ExpanseSync32Status {
    match e {
        WriteError::ArenaFull => ExpanseSync32Status::ArenaFull,
        WriteError::ReclaimBacklog => ExpanseSync32Status::ReclaimBacklog,
    }
}

/// Copies `src` into the caller's `stats` buffer of `stats_size` bytes,
/// writing the prefix both sides know about.
///
/// # Safety
///
/// `stats` null or valid for `stats_size` bytes of writes.
#[inline]
unsafe fn put_stats(stats: *mut ExpanseSync32Stats, stats_size: usize, src: &ExpanseSync32Stats) {
    if stats.is_null() {
        return;
    }
    let n = stats_size.min(core::mem::size_of::<ExpanseSync32Stats>());
    // SAFETY: `stats` is valid for `stats_size` bytes per contract and
    // `n` never exceeds either side's size.
    unsafe {
        core::ptr::copy_nonoverlapping(
            core::ptr::from_ref(src).cast::<u8>(),
            stats.cast::<u8>(),
            n,
        );
    }
}

macro_rules! sync32_surface {
    (
        $container:ty, $engine:ty, $outer:ident, $writer:ident, $reader:ident,
        new = $new:ident, free = $free:ident, writer = $writer_fn:ident, reader = $reader_fn:ident,
        try_reclaim = $try_reclaim:ident, stats = $stats:ident, reader_try_len = $reader_try_len:ident
    ) => {
        /// Writer handle: exactly one per container, owned by it.
        pub struct $writer(Writer32<'static, $engine>);
        /// Reader handle: one per execution context, owned by the container.
        pub struct $reader(Reader32<'static, $engine>);
        /// The container plus the handles that borrow it.
        pub struct $outer {
            // Rust drops fields in declaration order, so the handles (plain
            // data today, but they borrow `owner`) are declared first and
            // the container last.
            writer: $writer,
            readers: Box<[$reader]>,
            owner: Box<$container>,
        }

        impl $outer {
            fn build(node_cap: usize, max_readers: usize) -> Box<Self> {
                let mut owner: Box<$container> =
                    Box::new(Sync32::with_capacity(node_cap, max_readers));
                let ptr: *mut $container = core::ptr::from_mut(&mut *owner);
                // SAFETY: the container lives in its own heap allocation that
                // never moves for the outer object's lifetime, and the outer
                // object owns both it and every handle; the `'static`
                // lifetime is a bound the outer object upholds by dropping the
                // handles before the container.
                let (writer, mut pool) = unsafe { (*ptr).split() };
                let mut readers = Vec::with_capacity(max_readers);
                while let Some(r) = pool.take() {
                    readers.push($reader(r));
                }
                Box::new(Self {
                    writer: $writer(writer),
                    readers: readers.into_boxed_slice(),
                    owner,
                })
            }
        }

        impl Drop for $outer {
            fn drop(&mut self) {
                // Field order drops the handles before the container.
                debug_assert!(
                    !self.owner.any_reader_pinned(),
                    "expanse_sync32 free with a reader inside a walk"
                );
            }
        }

        /// Creates a container with a fixed arena of `node_cap` node slots
        /// and `max_readers` reader handles. NULL if `node_cap` is below
        /// `expanse_sync32_mutation_headroom()`.
        #[unsafe(no_mangle)]
        pub extern "C" fn $new(node_cap: usize, max_readers: usize) -> *mut $outer {
            if node_cap < MUTATION_HEADROOM {
                return core::ptr::null_mut();
            }
            Box::into_raw($outer::build(node_cap, max_readers))
        }

        /// Frees the container and every handle it owns. Null-tolerant.
        ///
        /// # Safety
        ///
        /// `map` must be null or come from the matching `_new`; no reader
        /// may be inside a walk and no writer call may be in progress.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $free(map: *mut $outer) {
            if !map.is_null() {
                // SAFETY: from `_new`, unused after per contract.
                drop(unsafe { Box::from_raw(map) });
            }
        }

        /// The container's single writer handle (the same pointer every
        /// call). Null for a null container.
        ///
        /// # Safety
        ///
        /// `map` must be null or a live handle.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $writer_fn(map: *mut $outer) -> *mut $writer {
            debug_assert!(!map.is_null(), "expanse_sync32: null container");
            if map.is_null() {
                return core::ptr::null_mut();
            }
            // SAFETY: `map` is live per contract. Raw place projection only:
            // a `&mut $outer` here would, under Stacked Borrows, invalidate
            // every handle pointer previously handed to C from this struct.
            unsafe { &raw mut (*map).writer }
        }

        /// Reader handle `idx` (`0 <= idx < max_readers`), the same pointer
        /// every call; null for a null container or an out-of-range index.
        ///
        /// # Safety
        ///
        /// `map` must be null or a live handle.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $reader_fn(map: *mut $outer, idx: usize) -> *mut $reader {
            debug_assert!(!map.is_null(), "expanse_sync32: null container");
            if map.is_null() {
                return core::ptr::null_mut();
            }
            // SAFETY: `map` is live per contract. Raw place projection only
            // (see the writer accessor); the bounds check reads the slice
            // length through the raw place without reborrowing the struct.
            unsafe {
                let readers: *mut [$reader] = &raw mut *(*map).readers;
                if idx >= readers.len() {
                    return core::ptr::null_mut();
                }
                &raw mut (*readers)[idx]
            }
        }

        /// Drains the pending-reclamation list if every reader is outside
        /// a walk: `OK` (drained or nothing pending) or `RECLAIM_BACKLOG`.
        ///
        /// # Safety
        ///
        /// `w` must be null or a live writer handle.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $try_reclaim(w: *mut $writer) -> ExpanseSync32Status {
            debug_assert!(!w.is_null(), "expanse_sync32: null writer");
            // SAFETY: null or live per contract.
            let Some(w) = (unsafe { w.as_mut() }) else {
                return ExpanseSync32Status::NullHandle;
            };
            if w.0.try_reclaim() {
                ExpanseSync32Status::Ok
            } else {
                ExpanseSync32Status::ReclaimBacklog
            }
        }

        /// Fills `stats` (the first `stats_size` bytes the caller knows
        /// about); `OK` or `NULL_HANDLE`.
        ///
        /// # Safety
        ///
        /// `w` must be null or a live writer handle; `stats` null or valid
        /// for `stats_size` bytes of writes.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $stats(
            w: *const $writer,
            stats: *mut ExpanseSync32Stats,
            stats_size: usize,
        ) -> ExpanseSync32Status {
            debug_assert!(!w.is_null(), "expanse_sync32: null writer");
            // SAFETY: null or live per contract.
            let Some(w) = (unsafe { w.as_ref() }) else {
                return ExpanseSync32Status::NullHandle;
            };
            let src = ExpanseSync32Stats {
                len: w.0.len() as u64,
                mem_used: w.0.mem_used(),
                pending_len: w.0.pending_len(),
                pending_bytes: w.0.pending_bytes(),
                free_slots: w.0.free_slots(),
            };
            // SAFETY: `stats` contract above.
            unsafe { put_stats(stats, stats_size, &src) };
            ExpanseSync32Status::Ok
        }

        /// Single-attempt population read: `OK` with `len_out` written, or
        /// `BUSY`. Never blocks, never allocates.
        ///
        /// # Safety
        ///
        /// `r` must be null or a live reader handle used from one execution
        /// context; `len_out` null or writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $reader_try_len(
            r: *mut $reader,
            len_out: *mut u64,
        ) -> ExpanseSync32Status {
            debug_assert!(!r.is_null(), "expanse_sync32: null reader");
            // SAFETY: null or live per contract.
            let Some(r) = (unsafe { r.as_mut() }) else {
                return ExpanseSync32Status::NullHandle;
            };
            match r.0.try_len() {
                Ok(n) => {
                    // SAFETY: `len_out` null or writable per contract.
                    unsafe { crate::modern::put(len_out, n as u64) };
                    ExpanseSync32Status::Ok
                }
                Err(Busy) => ExpanseSync32Status::Busy,
            }
        }
    };
}

sync32_surface!(
    SyncExpanseMap32,
    ExpanseMap32,
    SyncMap32,
    SyncMap32Writer,
    SyncMap32Reader,
    new = expanse_sync32_map_new,
    free = expanse_sync32_map_free,
    writer = expanse_sync32_map_writer,
    reader = expanse_sync32_map_reader,
    try_reclaim = expanse_sync32_map_writer_try_reclaim,
    stats = expanse_sync32_map_writer_stats,
    reader_try_len = expanse_sync32_map_reader_try_len
);

sync32_surface!(
    SyncExpanseSet32,
    ExpanseSet32,
    SyncSet32,
    SyncSet32Writer,
    SyncSet32Reader,
    new = expanse_sync32_set_new,
    free = expanse_sync32_set_free,
    writer = expanse_sync32_set_writer,
    reader = expanse_sync32_set_reader,
    try_reclaim = expanse_sync32_set_writer_try_reclaim,
    stats = expanse_sync32_set_writer_stats,
    reader_try_len = expanse_sync32_set_reader_try_len
);

// ---- map-specific entry points ----

/// Inserts `key -> value` inside one write bracket. `OK` (with
/// `replaced_out`/`old_out` written when the key was present),
/// `ARENA_FULL` or `RECLAIM_BACKLOG` (tree untouched), `NULL_HANDLE`.
/// Allocates; never call from an interrupt handler.
///
/// # Safety
///
/// `w` must be null or the live writer handle, used from one execution
/// context; out-pointers null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync32_map_writer_try_insert(
    w: *mut SyncMap32Writer,
    key: CWord,
    value: CWord,
    replaced_out: *mut bool,
    old_out: *mut CWord,
) -> ExpanseSync32Status {
    debug_assert!(!w.is_null(), "expanse_sync32: null writer");
    // SAFETY: null or live per contract.
    let Some(w) = (unsafe { w.as_mut() }) else {
        return ExpanseSync32Status::NullHandle;
    };
    match w.0.try_insert(key, value) {
        Ok(old) => {
            // SAFETY: out-pointers null or writable per contract.
            unsafe {
                crate::modern::put(replaced_out, old.is_some());
                if let Some(v) = old {
                    crate::modern::put(old_out, v);
                }
            }
            ExpanseSync32Status::Ok
        }
        Err(e) => write_status(e),
    }
}

/// Removes `key` inside one write bracket. `OK` (with `old_out` written),
/// `NOT_FOUND`, `ARENA_FULL` / `RECLAIM_BACKLOG` (headroom is checked
/// before the key is looked up, so an absent key can still be refused),
/// `NULL_HANDLE`. Allocates; never call from an interrupt handler.
///
/// # Safety
///
/// `w` must be null or the live writer handle, used from one execution
/// context; `old_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync32_map_writer_try_remove(
    w: *mut SyncMap32Writer,
    key: CWord,
    old_out: *mut CWord,
) -> ExpanseSync32Status {
    debug_assert!(!w.is_null(), "expanse_sync32: null writer");
    // SAFETY: null or live per contract.
    let Some(w) = (unsafe { w.as_mut() }) else {
        return ExpanseSync32Status::NullHandle;
    };
    match w.0.try_remove(key) {
        Ok(Some(v)) => {
            // SAFETY: `old_out` null or writable per contract.
            unsafe { crate::modern::put(old_out, v) };
            ExpanseSync32Status::Ok
        }
        Ok(None) => ExpanseSync32Status::NotFound,
        Err(e) => write_status(e),
    }
}

/// Callback type for [`expanse_sync32_map_writer_update`]: receives
/// `(current_present, current_value, new_value_out, user_ctx)` with
/// `*new_value_out` pre-filled to `current_value` (or `0` when absent).
/// Returning `true` stores `*new_value_out`; returning `false` removes `key`.
pub type ExpanseSync32MapUpdateFn = unsafe extern "C" fn(
    current_present: bool,
    current_value: CWord,
    new_value_out: *mut CWord,
    user_ctx: *mut c_void,
) -> bool;

/// Compares the entry at `key` with `(expected_present, expected_value)` and,
/// on match, replaces it with `(new_present, new_value)` inside at most one
/// write bracket (`!new_present` removes; a match where `new` equals the
/// observed entry is a no-op that opens no bracket and consults no headroom).
///
/// On `OK`, writes `*exchanged_out` (`true` on match, `false` on mismatch),
/// `*observed_present_out` (`true` if `key` was present before the call), and,
/// when present, `*observed_out`. Returns `ARENA_FULL` or `RECLAIM_BACKLOG`
/// (tree untouched) when a mutating match lacks headroom, or `NULL_HANDLE`.
///
/// # Safety
///
/// `w` must be null or the live writer handle, used from one execution
/// context; out-pointers null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync32_map_writer_compare_exchange(
    w: *mut SyncMap32Writer,
    key: CWord,
    expected_present: bool,
    expected_value: CWord,
    new_present: bool,
    new_value: CWord,
    exchanged_out: *mut bool,
    observed_present_out: *mut bool,
    observed_out: *mut CWord,
) -> ExpanseSync32Status {
    debug_assert!(!w.is_null(), "expanse_sync32: null writer");
    // SAFETY: null or live per contract.
    let Some(w) = (unsafe { w.as_mut() }) else {
        return ExpanseSync32Status::NullHandle;
    };
    let expected = expected_present.then_some(expected_value);
    let new = new_present.then_some(new_value);
    match w.0.compare_exchange(key, expected, new) {
        Ok(outcome) => {
            let (exchanged, observed) = match outcome {
                Ok(prev) => (true, prev),
                Err(seen) => (false, seen),
            };
            // SAFETY: out-pointers null or writable per contract.
            unsafe {
                crate::modern::put(exchanged_out, exchanged);
                crate::modern::put(observed_present_out, observed.is_some());
                if let Some(v) = observed {
                    crate::modern::put(observed_out, v);
                }
            }
            ExpanseSync32Status::Ok
        }
        Err(e) => write_status(e),
    }
}

/// Applies `callback` to the current entry at `key` and stores the result
/// inside at most one write bracket. Headroom is checked before `callback`
/// runs, so `callback` is never invoked when `ARENA_FULL` or `RECLAIM_BACKLOG`
/// is returned. On `OK`, `*old_present_out` and (when present) `*old_out`
/// receive the entry as it stood before `callback` ran. Returns `NULL_HANDLE`
/// when `w` or `callback` is null.
///
/// # Safety
///
/// `w` must be null or the live writer handle, used from one execution
/// context; `callback` must not touch `w` or its owning map and must return
/// normally; out-pointers null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync32_map_writer_update(
    w: *mut SyncMap32Writer,
    key: CWord,
    callback: Option<ExpanseSync32MapUpdateFn>,
    user_ctx: *mut c_void,
    old_present_out: *mut bool,
    old_out: *mut CWord,
) -> ExpanseSync32Status {
    debug_assert!(!w.is_null(), "expanse_sync32: null writer");
    // SAFETY: null or live per contract.
    let Some(w) = (unsafe { w.as_mut() }) else {
        return ExpanseSync32Status::NullHandle;
    };
    let Some(cb) = callback else {
        return ExpanseSync32Status::NullHandle;
    };
    let res = w.0.update(key, |current| {
        let (present, cur_val) = match current {
            Some(v) => (true, v),
            None => (false, 0),
        };
        let mut next_val: CWord = cur_val;
        // SAFETY: `cb` is caller-supplied and `next_val` is a valid local.
        let keep = unsafe { cb(present, cur_val, &raw mut next_val, user_ctx) };
        keep.then_some(next_val)
    });
    match res {
        Ok(old) => {
            // SAFETY: out-pointers null or writable per contract.
            unsafe {
                crate::modern::put(old_present_out, old.is_some());
                if let Some(v) = old {
                    crate::modern::put(old_out, v);
                }
            }
            ExpanseSync32Status::Ok
        }
        Err(e) => write_status(e),
    }
}

/// Returns the existing value for `key` without opening a write bracket or
/// consulting headroom (`*inserted_out = false`, `*value_out = existing`), or
/// inserts `value` in one write bracket when absent (`*inserted_out = true`,
/// `*value_out = value`). Returns `OK`, `ARENA_FULL` / `RECLAIM_BACKLOG` (tree
/// untouched, only when `key` is absent), or `NULL_HANDLE`.
///
/// # Safety
///
/// `w` must be null or the live writer handle, used from one execution
/// context; out-pointers null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync32_map_writer_get_or_insert(
    w: *mut SyncMap32Writer,
    key: CWord,
    value: CWord,
    inserted_out: *mut bool,
    value_out: *mut CWord,
) -> ExpanseSync32Status {
    debug_assert!(!w.is_null(), "expanse_sync32: null writer");
    // SAFETY: null or live per contract.
    let Some(w) = (unsafe { w.as_mut() }) else {
        return ExpanseSync32Status::NullHandle;
    };
    match w.0.get_or_insert(key, value) {
        Ok(existing) => {
            // SAFETY: out-pointers null or writable per contract.
            unsafe {
                crate::modern::put(inserted_out, existing.is_none());
                crate::modern::put(value_out, existing.unwrap_or(value));
            }
            ExpanseSync32Status::Ok
        }
        Err(e) => write_status(e),
    }
}

/// The writer's own consistent read (no bracket, never `BUSY`): true with
/// `value_out` written when present. Never allocates.
///
/// # Safety
///
/// `w` must be null or the live writer handle; `value_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync32_map_writer_get(
    w: *const SyncMap32Writer,
    key: CWord,
    value_out: *mut CWord,
) -> bool {
    debug_assert!(!w.is_null(), "expanse_sync32: null writer");
    // SAFETY: null or live per contract.
    let Some(v) = (unsafe { w.as_ref() }).and_then(|w| w.0.get(key)) else {
        return false;
    };
    // SAFETY: `value_out` null or writable per contract.
    unsafe { crate::modern::put(value_out, v) };
    true
}

/// Single-attempt optimistic read: `OK` with `value_out` written,
/// `NOT_FOUND`, or `BUSY` (the read overlapped a write bracket — surface
/// it and retry on the next invocation, never spin). Never blocks, never
/// allocates: the one interrupt-safe entry point.
///
/// # Safety
///
/// `r` must be null or a live reader handle used from one execution
/// context; `value_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync32_map_reader_try_get(
    r: *mut SyncMap32Reader,
    key: CWord,
    value_out: *mut CWord,
) -> ExpanseSync32Status {
    debug_assert!(!r.is_null(), "expanse_sync32: null reader");
    // SAFETY: null or live per contract.
    let Some(r) = (unsafe { r.as_mut() }) else {
        return ExpanseSync32Status::NullHandle;
    };
    match r.0.try_get(key) {
        Ok(Some(v)) => {
            // SAFETY: `value_out` null or writable per contract.
            unsafe { crate::modern::put(value_out, v) };
            ExpanseSync32Status::Ok
        }
        Ok(None) => ExpanseSync32Status::NotFound,
        Err(Busy) => ExpanseSync32Status::Busy,
    }
}

// ---- ordered reads through the map reader handle (#900) ----
//
// Each is one bounded validated search under the tree version
// (`Reader32::try_*`): the same single-attempt, BUSY-instead-of-spin contract
// as `expanse_sync32_map_reader_try_get`, and equally interrupt-safe.

/// Maps a single-attempt ordered read to its status, writing the entry
/// through the caller's out-pointers on `OK`.
///
/// # Safety
///
/// `key_out` and `value_out` each null or writable.
#[inline]
unsafe fn put_try_entry(
    read: Result<Option<(CWord, CWord)>, Busy>,
    key_out: *mut CWord,
    value_out: *mut CWord,
) -> ExpanseSync32Status {
    match read {
        Ok(Some((k, v))) => {
            // SAFETY: out-pointers null or writable per this function's contract.
            unsafe {
                crate::modern::put(key_out, k);
                crate::modern::put(value_out, v);
            }
            ExpanseSync32Status::Ok
        }
        Ok(None) => ExpanseSync32Status::NotFound,
        Err(Busy) => ExpanseSync32Status::Busy,
    }
}

/// Single-attempt smallest entry: `OK` with `key_out`/`value_out` written,
/// `NOT_FOUND`, `BUSY`, or `NULL_HANDLE`. Never blocks, never allocates.
///
/// # Safety
///
/// `r` must be null or a live reader handle used from one execution
/// context; `key_out` and `value_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync32_map_reader_try_first(
    r: *mut SyncMap32Reader,
    key_out: *mut CWord,
    value_out: *mut CWord,
) -> ExpanseSync32Status {
    debug_assert!(!r.is_null(), "expanse_sync32: null reader");
    // SAFETY: null or live per contract.
    let Some(r) = (unsafe { r.as_mut() }) else {
        return ExpanseSync32Status::NullHandle;
    };
    let read = r.0.try_first();
    // SAFETY: out-pointers null or writable per contract.
    unsafe { put_try_entry(read, key_out, value_out) }
}

/// Single-attempt largest entry; statuses as
/// [`expanse_sync32_map_reader_try_first`].
///
/// # Safety
///
/// Same contract as [`expanse_sync32_map_reader_try_first`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync32_map_reader_try_last(
    r: *mut SyncMap32Reader,
    key_out: *mut CWord,
    value_out: *mut CWord,
) -> ExpanseSync32Status {
    debug_assert!(!r.is_null(), "expanse_sync32: null reader");
    // SAFETY: null or live per contract.
    let Some(r) = (unsafe { r.as_mut() }) else {
        return ExpanseSync32Status::NullHandle;
    };
    let read = r.0.try_last();
    // SAFETY: out-pointers null or writable per contract.
    unsafe { put_try_entry(read, key_out, value_out) }
}

/// Single-attempt smallest entry with key `>= key`; statuses as
/// [`expanse_sync32_map_reader_try_first`].
///
/// # Safety
///
/// Same contract as [`expanse_sync32_map_reader_try_first`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync32_map_reader_try_next_at_or_after(
    r: *mut SyncMap32Reader,
    key: CWord,
    key_out: *mut CWord,
    value_out: *mut CWord,
) -> ExpanseSync32Status {
    debug_assert!(!r.is_null(), "expanse_sync32: null reader");
    // SAFETY: null or live per contract.
    let Some(r) = (unsafe { r.as_mut() }) else {
        return ExpanseSync32Status::NullHandle;
    };
    let read = r.0.try_next_at_or_after(key);
    // SAFETY: out-pointers null or writable per contract.
    unsafe { put_try_entry(read, key_out, value_out) }
}

/// Single-attempt smallest entry with key `> key` (`NOT_FOUND` for
/// `key == UINT32_MAX`); statuses as [`expanse_sync32_map_reader_try_first`].
///
/// # Safety
///
/// Same contract as [`expanse_sync32_map_reader_try_first`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync32_map_reader_try_next_after(
    r: *mut SyncMap32Reader,
    key: CWord,
    key_out: *mut CWord,
    value_out: *mut CWord,
) -> ExpanseSync32Status {
    debug_assert!(!r.is_null(), "expanse_sync32: null reader");
    // SAFETY: null or live per contract.
    let Some(r) = (unsafe { r.as_mut() }) else {
        return ExpanseSync32Status::NullHandle;
    };
    let read = r.0.try_next_after(key);
    // SAFETY: out-pointers null or writable per contract.
    unsafe { put_try_entry(read, key_out, value_out) }
}

/// Single-attempt largest entry with key `<= key`; statuses as
/// [`expanse_sync32_map_reader_try_first`].
///
/// # Safety
///
/// Same contract as [`expanse_sync32_map_reader_try_first`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync32_map_reader_try_prev_at_or_before(
    r: *mut SyncMap32Reader,
    key: CWord,
    key_out: *mut CWord,
    value_out: *mut CWord,
) -> ExpanseSync32Status {
    debug_assert!(!r.is_null(), "expanse_sync32: null reader");
    // SAFETY: null or live per contract.
    let Some(r) = (unsafe { r.as_mut() }) else {
        return ExpanseSync32Status::NullHandle;
    };
    let read = r.0.try_prev_at_or_before(key);
    // SAFETY: out-pointers null or writable per contract.
    unsafe { put_try_entry(read, key_out, value_out) }
}

/// Single-attempt largest entry with key `< key` (`NOT_FOUND` for
/// `key == 0`); statuses as [`expanse_sync32_map_reader_try_first`].
///
/// # Safety
///
/// Same contract as [`expanse_sync32_map_reader_try_first`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync32_map_reader_try_prev_before(
    r: *mut SyncMap32Reader,
    key: CWord,
    key_out: *mut CWord,
    value_out: *mut CWord,
) -> ExpanseSync32Status {
    debug_assert!(!r.is_null(), "expanse_sync32: null reader");
    // SAFETY: null or live per contract.
    let Some(r) = (unsafe { r.as_mut() }) else {
        return ExpanseSync32Status::NullHandle;
    };
    let read = r.0.try_prev_before(key);
    // SAFETY: out-pointers null or writable per contract.
    unsafe { put_try_entry(read, key_out, value_out) }
}

// ---- set-specific entry points ----

/// Inserts `key` inside one write bracket. `OK` (with `inserted_out`
/// written: false if already present), `ARENA_FULL` / `RECLAIM_BACKLOG`
/// (tree untouched), `NULL_HANDLE`. Allocates; never call from an
/// interrupt handler.
///
/// # Safety
///
/// `w` must be null or the live writer handle, used from one execution
/// context; `inserted_out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync32_set_writer_try_insert(
    w: *mut SyncSet32Writer,
    key: CWord,
    inserted_out: *mut bool,
) -> ExpanseSync32Status {
    debug_assert!(!w.is_null(), "expanse_sync32: null writer");
    // SAFETY: null or live per contract.
    let Some(w) = (unsafe { w.as_mut() }) else {
        return ExpanseSync32Status::NullHandle;
    };
    match w.0.try_insert(key) {
        Ok(inserted) => {
            // SAFETY: `inserted_out` null or writable per contract.
            unsafe { crate::modern::put(inserted_out, inserted) };
            ExpanseSync32Status::Ok
        }
        Err(e) => write_status(e),
    }
}

/// Removes `key` inside one write bracket. `OK`, `NOT_FOUND`, `ARENA_FULL`
/// / `RECLAIM_BACKLOG` (headroom is checked before the key is looked up),
/// `NULL_HANDLE`. Allocates; never call from an interrupt handler.
///
/// # Safety
///
/// `w` must be null or the live writer handle, used from one execution
/// context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync32_set_writer_try_remove(
    w: *mut SyncSet32Writer,
    key: CWord,
) -> ExpanseSync32Status {
    debug_assert!(!w.is_null(), "expanse_sync32: null writer");
    // SAFETY: null or live per contract.
    let Some(w) = (unsafe { w.as_mut() }) else {
        return ExpanseSync32Status::NullHandle;
    };
    match w.0.try_remove(key) {
        Ok(true) => ExpanseSync32Status::Ok,
        Ok(false) => ExpanseSync32Status::NotFound,
        Err(e) => write_status(e),
    }
}

/// The writer's own consistent membership test (no bracket, never `BUSY`).
/// Never allocates.
///
/// # Safety
///
/// `w` must be null or the live writer handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync32_set_writer_contains(
    w: *const SyncSet32Writer,
    key: CWord,
) -> bool {
    debug_assert!(!w.is_null(), "expanse_sync32: null writer");
    // SAFETY: null or live per contract.
    (unsafe { w.as_ref() }).is_some_and(|w| w.0.contains(key))
}

/// Single-attempt optimistic membership test: `OK` (present), `NOT_FOUND`,
/// or `BUSY`. Never blocks, never allocates: the one interrupt-safe entry
/// point.
///
/// # Safety
///
/// `r` must be null or a live reader handle used from one execution
/// context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_sync32_set_reader_try_contains(
    r: *mut SyncSet32Reader,
    key: CWord,
) -> ExpanseSync32Status {
    debug_assert!(!r.is_null(), "expanse_sync32: null reader");
    // SAFETY: null or live per contract.
    let Some(r) = (unsafe { r.as_mut() }) else {
        return ExpanseSync32Status::NullHandle;
    };
    match r.0.try_contains(key) {
        Ok(true) => ExpanseSync32Status::Ok,
        Ok(false) => ExpanseSync32Status::NotFound,
        Err(Busy) => ExpanseSync32Status::Busy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct UpdatePlan {
        seen_present: bool,
        seen_val: CWord,
        prefilled_val: CWord,
        write_next: Option<CWord>,
        keep: bool,
        calls: u32,
    }

    unsafe extern "C" fn apply_plan(
        current_present: bool,
        current_value: CWord,
        new_value_out: *mut CWord,
        user_ctx: *mut c_void,
    ) -> bool {
        // SAFETY: `user_ctx` points to a live `UpdatePlan` and `new_value_out`
        // points to a live local in `expanse_sync32_map_writer_update`.
        unsafe {
            let plan = &mut *user_ctx.cast::<UpdatePlan>();
            plan.seen_present = current_present;
            plan.seen_val = current_value;
            plan.prefilled_val = *new_value_out;
            plan.calls += 1;
            if let Some(next) = plan.write_next {
                *new_value_out = next;
            }
            plan.keep
        }
    }

    #[test]
    fn map_writer_rmw_compare_exchange_update_and_get_or_insert() {
        // SAFETY: all handles come from `m`, are used on this thread only, and
        // `m` is freed last.
        unsafe {
            let m = expanse_sync32_map_new(4096, 1);
            assert!(!m.is_null());
            let w = expanse_sync32_map_writer(m);
            let r = expanse_sync32_map_reader(m, 0);

            // ---- compare_exchange ----
            let (mut exchanged, mut obs_present, mut obs_val) = (false, true, 0xDEAD_BEEFu32);
            // Expect Some(1) on absent key 10 -> mismatch, observed_out untouched.
            assert_eq!(
                expanse_sync32_map_writer_compare_exchange(
                    w,
                    10,
                    true,
                    1,
                    true,
                    100,
                    &raw mut exchanged,
                    &raw mut obs_present,
                    &raw mut obs_val,
                ),
                ExpanseSync32Status::Ok
            );
            assert!(!exchanged);
            assert!(!obs_present);
            assert_eq!(obs_val, 0xDEAD_BEEF);

            // Expect None -> None on absent key 10 -> no-op match.
            assert_eq!(
                expanse_sync32_map_writer_compare_exchange(
                    w,
                    10,
                    false,
                    999,
                    false,
                    999,
                    &raw mut exchanged,
                    &raw mut obs_present,
                    &raw mut obs_val,
                ),
                ExpanseSync32Status::Ok
            );
            assert!(exchanged);
            assert!(!obs_present);

            // Expect None -> Some(100) on absent key 10 -> inserts 100.
            assert_eq!(
                expanse_sync32_map_writer_compare_exchange(
                    w,
                    10,
                    false,
                    0,
                    true,
                    100,
                    &raw mut exchanged,
                    &raw mut obs_present,
                    &raw mut obs_val,
                ),
                ExpanseSync32Status::Ok
            );
            assert!(exchanged);
            assert!(!obs_present);
            let mut got = 0u32;
            assert!(expanse_sync32_map_writer_get(w, 10, &raw mut got) && got == 100);

            // Expect None -> Some(200) on present key 10 -> mismatch, observes Some(100).
            assert_eq!(
                expanse_sync32_map_writer_compare_exchange(
                    w,
                    10,
                    false,
                    0,
                    true,
                    200,
                    &raw mut exchanged,
                    &raw mut obs_present,
                    &raw mut obs_val,
                ),
                ExpanseSync32Status::Ok
            );
            assert!(!exchanged);
            assert!(obs_present);
            assert_eq!(obs_val, 100);

            // Expect Some(99) -> Some(200) on present key 10 -> mismatch, observes Some(100).
            obs_val = 0;
            assert_eq!(
                expanse_sync32_map_writer_compare_exchange(
                    w,
                    10,
                    true,
                    99,
                    true,
                    200,
                    &raw mut exchanged,
                    &raw mut obs_present,
                    &raw mut obs_val,
                ),
                ExpanseSync32Status::Ok
            );
            assert!(!exchanged);
            assert!(obs_present);
            assert_eq!(obs_val, 100);

            // Expect Some(100) -> Some(200) -> overwrites with 200.
            obs_val = 0;
            assert_eq!(
                expanse_sync32_map_writer_compare_exchange(
                    w,
                    10,
                    true,
                    100,
                    true,
                    200,
                    &raw mut exchanged,
                    &raw mut obs_present,
                    &raw mut obs_val,
                ),
                ExpanseSync32Status::Ok
            );
            assert!(exchanged);
            assert!(obs_present);
            assert_eq!(obs_val, 100);
            assert_eq!(
                expanse_sync32_map_reader_try_get(r, 10, &raw mut got),
                ExpanseSync32Status::Ok
            );
            assert_eq!(got, 200);

            // Expect Some(200) -> None with null out-pointers -> removes key 10.
            assert_eq!(
                expanse_sync32_map_writer_compare_exchange(
                    w,
                    10,
                    true,
                    200,
                    false,
                    0,
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                ),
                ExpanseSync32Status::Ok
            );
            assert!(!expanse_sync32_map_writer_get(w, 10, &raw mut got));

            // ---- update ----
            assert_eq!(
                expanse_sync32_map_writer_update(
                    w,
                    20,
                    None,
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                ),
                ExpanseSync32Status::NullHandle
            );

            let (mut old_present, mut old_val) = (true, 0xCAFE_BABEu32);
            let mut plan = UpdatePlan {
                seen_present: true,
                seen_val: 99,
                prefilled_val: 99,
                write_next: Some(300),
                keep: true,
                calls: 0,
            };
            // Insert via update on absent key 20.
            assert_eq!(
                expanse_sync32_map_writer_update(
                    w,
                    20,
                    Some(apply_plan),
                    (&raw mut plan).cast(),
                    &raw mut old_present,
                    &raw mut old_val,
                ),
                ExpanseSync32Status::Ok
            );
            assert_eq!(plan.calls, 1);
            assert!(!plan.seen_present && plan.seen_val == 0 && plan.prefilled_val == 0);
            assert!(!old_present);
            assert_eq!(old_val, 0xCAFE_BABE, "old_out untouched when absent");
            assert!(expanse_sync32_map_writer_get(w, 20, &raw mut got) && got == 300);

            // Modify existing key 20 -> 350.
            plan.write_next = Some(350);
            plan.keep = true;
            assert_eq!(
                expanse_sync32_map_writer_update(
                    w,
                    20,
                    Some(apply_plan),
                    (&raw mut plan).cast(),
                    &raw mut old_present,
                    &raw mut old_val,
                ),
                ExpanseSync32Status::Ok
            );
            assert_eq!(plan.calls, 2);
            assert!(plan.seen_present && plan.seen_val == 300 && plan.prefilled_val == 300);
            assert!(old_present && old_val == 300);
            assert!(expanse_sync32_map_writer_get(w, 20, &raw mut got) && got == 350);

            // Keep existing value without writing *new_value_out (pre-filled to 350).
            plan.write_next = None;
            plan.keep = true;
            assert_eq!(
                expanse_sync32_map_writer_update(
                    w,
                    20,
                    Some(apply_plan),
                    (&raw mut plan).cast(),
                    &raw mut old_present,
                    &raw mut old_val,
                ),
                ExpanseSync32Status::Ok
            );
            assert!(old_present && old_val == 350);
            assert!(expanse_sync32_map_writer_get(w, 20, &raw mut got) && got == 350);

            // Remove existing key 20 by returning false.
            plan.keep = false;
            old_val = 0;
            assert_eq!(
                expanse_sync32_map_writer_update(
                    w,
                    20,
                    Some(apply_plan),
                    (&raw mut plan).cast(),
                    &raw mut old_present,
                    &raw mut old_val,
                ),
                ExpanseSync32Status::Ok
            );
            assert!(old_present && old_val == 350);
            assert!(!expanse_sync32_map_writer_get(w, 20, &raw mut got));

            // ---- get_or_insert ----
            let (mut inserted, mut val_out) = (false, 0u32);
            assert_eq!(
                expanse_sync32_map_writer_get_or_insert(
                    w,
                    30,
                    777,
                    &raw mut inserted,
                    &raw mut val_out,
                ),
                ExpanseSync32Status::Ok
            );
            assert!(inserted && val_out == 777);
            assert_eq!(
                expanse_sync32_map_writer_get_or_insert(
                    w,
                    30,
                    888,
                    &raw mut inserted,
                    &raw mut val_out,
                ),
                ExpanseSync32Status::Ok
            );
            assert!(!inserted && val_out == 777);
            assert!(expanse_sync32_map_writer_get(w, 30, &raw mut got) && got == 777);

            expanse_sync32_map_free(m);
        }
    }

    #[test]
    fn map_writer_rmw_headroom_refusals() {
        let headroom = expanse_sync32_mutation_headroom();
        // SAFETY: `m` is live for the block and freed once at the end.
        unsafe {
            let m = expanse_sync32_map_new(headroom, 1);
            assert!(!m.is_null());
            let w = expanse_sync32_map_writer(m);

            // Key 0 fits in the root immediate edge; keys that force node
            // allocation exhaust the `headroom`-sized arena after one split.
            assert_eq!(
                expanse_sync32_map_writer_try_insert(
                    w,
                    1,
                    10,
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                ),
                ExpanseSync32Status::Ok
            );
            for k in 2..64u32 {
                let st = expanse_sync32_map_writer_try_insert(
                    w,
                    k * 257,
                    k,
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                );
                if st == ExpanseSync32Status::ArenaFull {
                    break;
                }
                assert_eq!(st, ExpanseSync32Status::Ok);
            }

            // Now free_slots < headroom:
            // 1. get_or_insert on present key 1 succeeds without consulting headroom.
            let (mut inserted, mut val_out) = (true, 0u32);
            assert_eq!(
                expanse_sync32_map_writer_get_or_insert(
                    w,
                    1,
                    999,
                    &raw mut inserted,
                    &raw mut val_out,
                ),
                ExpanseSync32Status::Ok
            );
            assert!(!inserted && val_out == 10);

            // 2. get_or_insert on absent key 0xFFFF_FFFE is refused with ArenaFull.
            assert_eq!(
                expanse_sync32_map_writer_get_or_insert(
                    w,
                    0xFFFF_FFFE,
                    999,
                    &raw mut inserted,
                    &raw mut val_out,
                ),
                ExpanseSync32Status::ArenaFull
            );

            // 3. compare_exchange mismatch and no-op match succeed without headroom;
            //    mutating match is refused with ArenaFull.
            let (mut exchanged, mut obs_present, mut obs_val) = (true, false, 0u32);
            assert_eq!(
                expanse_sync32_map_writer_compare_exchange(
                    w,
                    1,
                    true,
                    999,
                    true,
                    20,
                    &raw mut exchanged,
                    &raw mut obs_present,
                    &raw mut obs_val,
                ),
                ExpanseSync32Status::Ok
            );
            assert!(!exchanged && obs_present && obs_val == 10);

            assert_eq!(
                expanse_sync32_map_writer_compare_exchange(
                    w,
                    1,
                    true,
                    10,
                    true,
                    10,
                    &raw mut exchanged,
                    &raw mut obs_present,
                    &raw mut obs_val,
                ),
                ExpanseSync32Status::Ok
            );
            assert!(exchanged && obs_present && obs_val == 10);

            assert_eq!(
                expanse_sync32_map_writer_compare_exchange(
                    w,
                    1,
                    true,
                    10,
                    true,
                    20,
                    &raw mut exchanged,
                    &raw mut obs_present,
                    &raw mut obs_val,
                ),
                ExpanseSync32Status::ArenaFull
            );

            // 4. update checks headroom before invoking the callback.
            let mut plan = UpdatePlan {
                seen_present: false,
                seen_val: 0,
                prefilled_val: 0,
                write_next: Some(20),
                keep: true,
                calls: 0,
            };
            assert_eq!(
                expanse_sync32_map_writer_update(
                    w,
                    1,
                    Some(apply_plan),
                    (&raw mut plan).cast(),
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                ),
                ExpanseSync32Status::ArenaFull
            );
            assert_eq!(plan.calls, 0, "callback must not run on ArenaFull refusal");

            expanse_sync32_map_free(m);
        }
    }
}
