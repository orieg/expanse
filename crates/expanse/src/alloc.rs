//! Phase 5: the allocation subsystem.
//!
//! All nodes and leaves are carved from the global allocator at
//! [`CACHE_LINE`] alignment, through a [`NodeAlloc`] handle that keeps
//! byte-exact accounting. The accounting is load-bearing: it backs the
//! compat layer's `MemUsed` surface and the bytes/key benchmark metric
//! (`docs/BENCHMARKING.md`), so every allocation path must go through
//! here.
//!
//! Deliberately simple: the modern global allocators this crate targets
//! (mimalloc/jemalloc/system) already run segregated size-class caches, so
//! a bespoke slab layer is pure speculation until the Phase 8 benches can
//! measure it. If mutation-burst profiles justify one, it slots in behind
//! this same interface.
//!
//! The tree itself is single-writer; Phase 7's concurrent wrappers add
//! shared readers, so the counters are (relaxed) atomics.

use crate::occ::Collector;
use crate::types::CACHE_LINE;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};
use std::alloc::{Layout, alloc_zeroed, dealloc, handle_alloc_error};
use std::sync::{Arc, OnceLock};

/// Allocation handle owned by a tree: hands out cache-line-aligned,
/// zeroed memory and keeps byte-exact accounting.
///
/// Counters are relaxed atomics (Phase 7): accounting must stay exact
/// when a concurrent wrapper shares the tree across threads, and the
/// counters order nothing — the OCC read protocol carries the fences.
#[derive(Debug, Default)]
pub struct NodeAlloc {
    bytes_in_use: AtomicUsize,
    live_allocs: AtomicUsize,
    /// Phase 7: when set, frees are retired to the collector instead of
    /// released — concurrent readers may still hold the pointers.
    deferred: OnceLock<Arc<Collector>>,
    /// Debug-only: how many node version brackets are open on this
    /// tree's mutation stack (see [`Self::assert_bracketed`]).
    #[cfg(debug_assertions)]
    bracket_depth: AtomicUsize,
    /// Cumulative allocation count (never decremented). Lets a test
    /// separate the engine's own node/leaf allocations from incidental
    /// scratch allocations elsewhere in a code path — see
    /// `tests/no_heap_churn.rs`.
    total_allocs: AtomicUsize,
}

impl NodeAlloc {
    /// A fresh handle with zeroed counters.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Bytes currently allocated through this handle.
    #[must_use]
    pub fn bytes_in_use(&self) -> usize {
        self.bytes_in_use.load(Ordering::Relaxed)
    }

    /// Number of live allocations (diagnostics / leak assertions in tests).
    #[must_use]
    pub fn live_allocs(&self) -> usize {
        self.live_allocs.load(Ordering::Relaxed)
    }

    /// Cumulative allocations made through this handle since it was
    /// created (never decremented). Used to separate the engine's own
    /// node and leaf allocations from incidental scratch allocations in
    /// the same code path.
    #[must_use]
    pub fn total_allocs(&self) -> usize {
        self.total_allocs.load(Ordering::Relaxed)
    }

    fn layout_for(bytes: usize) -> Layout {
        debug_assert!(bytes > 0);
        // SAFETY-adjacent invariant: CACHE_LINE is a nonzero power of two,
        // and node/leaf sizes never approach the rounding overflow bound.
        Layout::from_size_align(bytes, CACHE_LINE).expect("valid node layout")
    }

    /// Allocates `bytes` of zeroed, cache-line-aligned memory.
    #[must_use]
    pub fn alloc_bytes(&self, bytes: usize) -> NonNull<u8> {
        let layout = Self::layout_for(bytes);
        // SAFETY: `layout` has nonzero size (asserted in `layout_for`).
        let raw = unsafe { alloc_zeroed(layout) };
        let Some(ptr) = NonNull::new(raw) else {
            handle_alloc_error(layout)
        };
        self.bytes_in_use.fetch_add(bytes, Ordering::Relaxed);
        self.live_allocs.fetch_add(1, Ordering::Relaxed);
        self.total_allocs.fetch_add(1, Ordering::Relaxed);
        ptr
    }

    /// Frees an allocation made by [`Self::alloc_bytes`] with this handle.
    ///
    /// # Safety
    ///
    /// `ptr` must come from `alloc_bytes(bytes)` on this handle, not yet
    /// freed, and nothing may use it afterwards.
    pub unsafe fn free_bytes(&self, ptr: NonNull<u8>, bytes: usize) {
        if let Some(c) = self.deferred.get() {
            // Deferred mode: the structure no longer references `ptr`,
            // but pinned readers may — reclamation waits out the grace
            // period. Accounting is logical (the bytes left the tree).
            c.retire(ptr, bytes);
        } else {
            let layout = Self::layout_for(bytes);
            // SAFETY: per this function's contract, `ptr`/`layout` match
            // the original allocation.
            unsafe { dealloc(ptr.as_ptr(), layout) };
        }
        self.bytes_in_use.fetch_sub(bytes, Ordering::Relaxed);
        self.live_allocs.fetch_sub(1, Ordering::Relaxed);
    }

    /// True once this tree is shared through a Phase 7 concurrent
    /// wrapper: the mutation engine then maintains per-node OCC versions
    /// (single-threaded trees skip those fences entirely).
    #[inline]
    pub(crate) fn occ_enabled(&self) -> bool {
        self.deferred.get().is_some()
    }

    /// Debug-only bracket bookkeeping (see [`Self::assert_bracketed`]).
    #[cfg(debug_assertions)]
    pub(crate) fn bracket_enter(&self) {
        self.bracket_depth.fetch_add(1, Ordering::Relaxed);
    }

    /// Debug-only bracket bookkeeping (see [`Self::assert_bracketed`]).
    #[cfg(debug_assertions)]
    pub(crate) fn bracket_leave(&self) {
        self.bracket_depth.fetch_sub(1, Ordering::Relaxed);
    }

    /// Asserts the Phase 7 coverage invariant at a mutation site: **every
    /// mutation of a node's interior happens with an enclosing node's
    /// version bracket open**, so a concurrent reader validating against
    /// that node's version cannot miss it.
    ///
    /// Terminal nodes (leaves, bitmap leaves) carry no version of their
    /// own; readers validate their payloads against the parent branch's
    /// version, which is exactly why the parent's bracket must still be
    /// open while a leaf is being rewritten. Checked only in debug
    /// builds, and only for concurrently shared trees — a single-threaded
    /// tree has no readers to protect.
    #[inline]
    pub(crate) fn assert_bracketed(&self) {
        #[cfg(debug_assertions)]
        debug_assert!(
            !self.occ_enabled() || self.bracket_depth.load(Ordering::Relaxed) > 0,
            "node interior mutated outside any version bracket: a concurrent \
             reader could observe it mid-write"
        );
    }

    /// Switches this handle to deferred reclamation through `collector`,
    /// permanently (Phase 7 concurrent wrappers call this once at
    /// construction). Idempotent for the same collector; a second call
    /// with a different collector is a bug and panics.
    pub fn defer_to(&self, collector: Arc<Collector>) {
        let stored = self.deferred.get_or_init(|| Arc::clone(&collector));
        assert!(
            Arc::ptr_eq(stored, &collector),
            "NodeAlloc already deferred to a different collector"
        );
    }

    /// Allocates a node and moves `init` into it.
    #[must_use]
    pub fn alloc_node<T>(&self, init: T) -> NonNull<T> {
        debug_assert!(align_of::<T>() <= CACHE_LINE);
        let ptr = self.alloc_bytes(size_of::<T>()).cast::<T>();
        // SAFETY: freshly allocated, correctly sized and aligned for T
        // (alignment asserted ≤ CACHE_LINE, which alloc_bytes provides).
        unsafe { ptr.write(init) };
        ptr
    }

    /// Frees a node allocated by [`Self::alloc_node`], dropping its value.
    ///
    /// # Safety
    ///
    /// `ptr` must come from `alloc_node::<T>` on this handle, not yet
    /// freed, and nothing may use it afterwards.
    pub unsafe fn free_node<T>(&self, ptr: NonNull<T>) {
        // SAFETY: `ptr` holds a live T per this function's contract.
        unsafe { ptr.drop_in_place() };
        // SAFETY: same allocation, same size as alloc_node made.
        unsafe { self.free_bytes(ptr.cast::<u8>(), size_of::<T>()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{BranchB, BranchL3, BranchU};

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "outside any version bracket")]
    fn negative_control_bracket_assert_must_fire() {
        // docs/TESTING.md: an assertion that has never fired is not known
        // to work. A concurrently-shared tree with no bracket open must
        // trip the Phase 7 coverage check.
        let a = NodeAlloc::new();
        a.defer_to(std::sync::Arc::new(crate::occ::Collector::new()));
        a.assert_bracketed();
    }

    #[test]
    fn bracket_assert_is_quiet_when_covered() {
        // Single-threaded trees have no readers: never trips.
        let a = NodeAlloc::new();
        a.assert_bracketed();
        // Shared tree with a bracket open: also fine.
        a.defer_to(std::sync::Arc::new(crate::occ::Collector::new()));
        let mut version = 0u32;
        crate::occ::version_begin_if::<true>(&a, &mut version);
        a.assert_bracketed();
        crate::occ::version_end_if::<true>(&a, &mut version);
    }

    #[test]
    fn accounting_round_trip() {
        let a = NodeAlloc::new();
        assert_eq!(a.bytes_in_use(), 0);

        let n1 = a.alloc_node(BranchL3::new(2));
        let n2 = a.alloc_node(BranchB::new(2));
        let n3 = a.alloc_node(BranchU::new());
        let leaf = a.alloc_bytes(21);
        assert_eq!(a.bytes_in_use(), 64 + 128 + 4160 + 21);
        assert_eq!(a.live_allocs(), 4);

        // Alignment: every allocation is cache-line aligned.
        assert_eq!(n1.as_ptr() as usize % CACHE_LINE, 0);
        assert_eq!(n3.as_ptr() as usize % CACHE_LINE, 0);
        assert_eq!(leaf.as_ptr() as usize % CACHE_LINE, 0);

        // Zeroed memory: a fresh leaf allocation reads as zeros.
        for i in 0..21 {
            // SAFETY: in-bounds read of the 21-byte allocation.
            assert_eq!(unsafe { *leaf.as_ptr().add(i) }, 0);
        }

        // SAFETY: freeing exactly what was allocated above, once.
        unsafe {
            a.free_node(n1);
            a.free_node(n2);
            a.free_node(n3);
            a.free_bytes(leaf, 21);
        }
        assert_eq!(a.bytes_in_use(), 0);
        assert_eq!(a.live_allocs(), 0);
    }
}
