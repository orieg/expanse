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

use core::ffi::{c_char, c_int};

#[cfg(not(feature = "std"))]
use crate::core_alloc::{boxed::Box, vec::Vec};
use crate::modern::CWord;
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
            // Declared first so it drops last: the handles borrow it.
            owner: Box<$container>,
            writer: $writer,
            readers: Box<[$reader]>,
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
                    owner,
                    writer: $writer(writer),
                    readers: readers.into_boxed_slice(),
                })
            }
        }

        impl Drop for $outer {
            fn drop(&mut self) {
                // Handles are plain data; `owner` drops last by field order.
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
            // SAFETY: null or live per contract.
            match unsafe { map.as_mut() } {
                Some(m) => core::ptr::from_mut(&mut m.writer),
                None => core::ptr::null_mut(),
            }
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
            // SAFETY: null or live per contract.
            match unsafe { map.as_mut() } {
                Some(m) => match m.readers.get_mut(idx) {
                    Some(r) => core::ptr::from_mut(r),
                    None => core::ptr::null_mut(),
                },
                None => core::ptr::null_mut(),
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
