//! HOT's ROWEX variant — the concurrent arm (#692, METHODOLOGY.md §10).
//!
//! Compiled only under the `rowex` feature. The two wrappers here are the
//! concurrent twins of [`HotSet`](crate::HotSet) and [`HotMap`](crate::HotMap):
//! `insert`, `contains` and `get` take `&self` and may be called from any
//! number of threads at once on one trie, which is the property the arm
//! measures. `len` walks the structure and is quiescent-only — the harnesses
//! call it after every writer has joined.
//!
//! Both arms are measured **below any external lock** (§8.16; §10.3 decision
//! 4): ROWEX through its own per-node write exclusion and epoch-based
//! reclamation, Expanse through `SyncExpanseMap` / `SyncExpanseSet`. Neither is
//! wrapped here.

use std::os::raw::c_void;

use crate::InlineInsert;

unsafe extern "C" {
    fn exp_rowex_set_new() -> *mut c_void;
    fn exp_rowex_set_delete(t: *mut c_void);
    fn exp_rowex_set_insert(t: *mut c_void, k: u64) -> i32;
    fn exp_rowex_set_contains(t: *mut c_void, k: u64) -> i32;
    fn exp_rowex_set_len(t: *mut c_void) -> usize;

    fn exp_rowex_map_new() -> *mut c_void;
    fn exp_rowex_map_delete(t: *mut c_void);
    fn exp_rowex_map_insert(t: *mut c_void, k: u64, v: u64) -> i32;
    fn exp_rowex_map_get(t: *mut c_void, k: u64, out: *mut u64) -> i32;
    fn exp_rowex_map_len(t: *mut c_void) -> usize;
}

/// `HOTRowex<uint64_t, IdentityKeyExtractor>` — paired against `SyncExpanseSet`.
///
/// The value is the key, stored inline in the tagged child pointer, so the
/// 63-bit inline-payload predicate ([`hot_can_inline`](crate::hot_can_inline))
/// applies to this arm exactly as it does to the single-threaded set arm.
pub struct RowexSet(*mut c_void);

// SAFETY: HOT documents `HOTRowex::insert` and `lookup` as concurrent
// operations under its ROWEX protocol (per-node write exclusion, CAS root
// replacement, epoch-based reclamation with per-thread state). The wrapper
// exposes only those two entry points as `&self`; `len` walks the trie and is
// documented quiescent-only. The raw pointer is owned and freed exactly once.
unsafe impl Send for RowexSet {}
// SAFETY: as above — shared access maps onto operations HOT declares
// thread-safe, and nothing else is reachable through `&RowexSet`.
unsafe impl Sync for RowexSet {}

impl RowexSet {
    /// Builds an empty trie.
    pub fn new() -> Self {
        // SAFETY: the shim allocates and returns an owned trie; non-null or the
        // C++ allocation would have aborted; freed exactly once in `Drop`.
        Self(unsafe { exp_rowex_set_new() })
    }

    /// Concurrent insert. A key outside HOT's inline payload returns
    /// [`InlineInsert::NotRepresentable`] and stores nothing.
    pub fn insert(&self, k: u64) -> InlineInsert {
        // SAFETY: `self.0` is a live trie; the call is one HOT declares
        // concurrent. The shim returns -2 for a non-representable key.
        match unsafe { exp_rowex_set_insert(self.0, k) } {
            1 => InlineInsert::Inserted,
            0 => InlineInsert::AlreadyPresent,
            _ => InlineInsert::NotRepresentable,
        }
    }

    /// Concurrent membership test.
    pub fn contains(&self, k: u64) -> bool {
        // SAFETY: as `insert`.
        unsafe { exp_rowex_set_contains(self.0, k) == 1 }
    }

    /// Population by walking — quiescent-only, never inferred from `insert`.
    pub fn len(&self) -> usize {
        // SAFETY: `self.0` is a live trie; the caller guarantees no concurrent
        // writer, which is the documented contract of this method.
        unsafe { exp_rowex_set_len(self.0) }
    }

    /// Whether the trie holds no entries (quiescent-only).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for RowexSet {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for RowexSet {
    fn drop(&mut self) {
        // SAFETY: produced by `exp_rowex_set_new`; released here only.
        unsafe { exp_rowex_set_delete(self.0) }
    }
}

/// `HOTRowex<std::pair*, PairPointerKeyExtractor>` — paired against `SyncExpanseMap`.
///
/// One heap pair per entry, as on the single-threaded map arm; the value is
/// reached through that pointer on every `get`, and the census counts the pair.
pub struct RowexMap(*mut c_void);

// SAFETY: see `RowexSet` — same protocol, same exposed surface.
unsafe impl Send for RowexMap {}
// SAFETY: see `RowexSet`.
unsafe impl Sync for RowexMap {}

impl RowexMap {
    /// Builds an empty trie.
    pub fn new() -> Self {
        // SAFETY: as `RowexSet::new`.
        Self(unsafe { exp_rowex_map_new() })
    }

    /// Concurrent insert of `k -> v`; `true` if the trie reports it as new.
    pub fn insert(&self, k: u64, v: u64) -> bool {
        // SAFETY: `self.0` is a live trie; concurrent by HOT's contract.
        unsafe { exp_rowex_map_insert(self.0, k, v) == 1 }
    }

    /// Concurrent lookup that fetches the stored value.
    pub fn get(&self, k: u64) -> Option<u64> {
        let mut out = 0u64;
        // SAFETY: as `insert`; `out` is a live local for the call.
        let hit = unsafe { exp_rowex_map_get(self.0, k, &raw mut out) };
        (hit == 1).then_some(out)
    }

    /// Population by walking — quiescent-only.
    pub fn len(&self) -> usize {
        // SAFETY: as `RowexSet::len`.
        unsafe { exp_rowex_map_len(self.0) }
    }

    /// Whether the trie holds no entries (quiescent-only).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for RowexMap {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for RowexMap {
    fn drop(&mut self) {
        // SAFETY: produced by `exp_rowex_map_new`; released here only. The
        // shim frees the pairs it owns before the trie.
        unsafe { exp_rowex_map_delete(self.0) }
    }
}
