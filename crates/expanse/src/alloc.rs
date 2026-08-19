//! Phase 5: the allocation subsystem.
//!
//! All nodes and leaves are carved from the global allocator through a
//! [`NodeAlloc`] handle that keeps byte-exact accounting. The accounting
//! is load-bearing: it backs the compat layer's `MemUsed` surface and the
//! bytes/key benchmark metric (`docs/BENCHMARKING.md`), so every
//! allocation path must go through here.
//!
//! **Alignment is per kind, and the split is a soundness boundary.**
//! [`NodeAlloc::alloc_node`] allocates at `align_of::<T>()`, which is
//! [`CACHE_LINE`] for the six `#[repr(C, align(64))]` node types whose
//! pointers are cast from these allocations.
//! [`NodeAlloc::alloc_bytes`] allocates raw byte storage — packed leaves
//! and subarrays, addressed by computed offset and never cast to an
//! aligned type — at the weaker [`RAW_ALIGN`].
//!
//! Each pair must be freed with the alignment it was allocated at: a
//! `dealloc` layout mismatch is undefined behaviour, not a leak. That is
//! why `free_node` does **not** route through `free_bytes`, and why the
//! EBR collector carries an alignment alongside each retired pointer
//! rather than assuming one. Miri catches a mismatch here, so the model
//! suites are the real guard on this file.
//!
//! Why the split exists: asking for 64-byte alignment takes glibc off its
//! `calloc` fast path and onto `aligned_alloc` plus an explicit memset.
//! Per-function profiling measured `_int_malloc` at 11.2% and
//! `_mid_memalign` at 4.7% of `map_insert/random` when every allocation
//! asked for 64 — and in that benchmark raw leaves outnumber aligned
//! nodes roughly 45:1.
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
use crate::types::{CACHE_LINE, RAW_ALIGN};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};
use std::alloc::{Layout, alloc_zeroed, dealloc, handle_alloc_error};
use std::sync::{Arc, OnceLock};

/// Allocation handle owned by a tree: hands out zeroed memory — at
/// `align_of::<T>()` via [`Self::alloc_node`], at [`RAW_ALIGN`] via
/// [`Self::alloc_bytes`] — and keeps byte-exact accounting.
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

    fn layout_for(bytes: usize, align: usize) -> Layout {
        debug_assert!(bytes > 0);
        // SAFETY-adjacent invariant: `align` is a nonzero power of two
        // (both call paths pass a constant or `align_of`), and node/leaf
        // sizes never approach the rounding overflow bound.
        Layout::from_size_align(bytes, align).expect("valid node layout")
    }

    /// Allocates `bytes` of zeroed memory at `align`.
    ///
    /// **Every free must pass the same `align`** — `dealloc` requires the
    /// layout to match, so a mismatch is undefined behaviour rather than a
    /// leak. The two public pairs below are what keeps that structural
    /// rather than remembered: `alloc_bytes`/`free_bytes` are always
    /// [`RAW_ALIGN`], `alloc_node`/`free_node` are always `align_of::<T>()`.
    fn alloc_raw(&self, bytes: usize, align: usize) -> NonNull<u8> {
        let layout = Self::layout_for(bytes, align);
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

    /// Frees an `alloc_raw(bytes, align)` allocation.
    ///
    /// # Safety
    ///
    /// `ptr` must come from `alloc_raw(bytes, align)` on this handle with
    /// **the same `align`**, not yet freed, and nothing may use it after.
    unsafe fn free_raw(&self, ptr: NonNull<u8>, bytes: usize, align: usize) {
        if let Some(c) = self.deferred.get() {
            // Deferred mode: the structure no longer references `ptr`,
            // but pinned readers may — reclamation waits out the grace
            // period. The alignment travels with the pointer, because the
            // collector frees it later and elsewhere.
            c.retire(ptr, bytes, align);
        } else {
            let layout = Self::layout_for(bytes, align);
            // SAFETY: per this function's contract, `ptr`/`layout` match
            // the original allocation.
            unsafe { dealloc(ptr.as_ptr(), layout) };
        }
        self.bytes_in_use.fetch_sub(bytes, Ordering::Relaxed);
        self.live_allocs.fetch_sub(1, Ordering::Relaxed);
    }

    /// Allocates `bytes` of zeroed memory for **raw byte storage** —
    /// packed leaves and subarrays, at [`RAW_ALIGN`]. Never use this for a
    /// type that declares a stronger alignment; that is `alloc_node`.
    #[must_use]
    pub fn alloc_bytes(&self, bytes: usize) -> NonNull<u8> {
        self.alloc_raw(bytes, RAW_ALIGN)
    }

    /// Frees an allocation made by [`Self::alloc_bytes`] with this handle.
    ///
    /// # Safety
    ///
    /// `ptr` must come from `alloc_bytes(bytes)` on this handle, not yet
    /// freed, and nothing may use it afterwards.
    pub unsafe fn free_bytes(&self, ptr: NonNull<u8>, bytes: usize) {
        // SAFETY: `alloc_bytes` is the only producer of these pointers and
        // always uses RAW_ALIGN, so the layout matches.
        unsafe { self.free_raw(ptr, bytes, RAW_ALIGN) };
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
        // `align_of::<T>()`, NOT `alloc_bytes`: the six `repr(C, align(64))`
        // node types need the full cache line, while raw byte storage does
        // not, and routing both through one alignment is what made every
        // allocation take glibc's `memalign` path.
        let ptr = self.alloc_raw(size_of::<T>(), align_of::<T>()).cast::<T>();
        // SAFETY: freshly allocated, correctly sized, and allocated at
        // exactly `align_of::<T>()`.
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
        // SAFETY: same allocation, same size AND same alignment as
        // `alloc_node` used — deliberately not routed through
        // `free_bytes`, whose alignment is RAW_ALIGN.
        unsafe { self.free_raw(ptr.cast::<u8>(), size_of::<T>(), align_of::<T>()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{BranchB, BranchL3, BranchU};
    use crate::occ::Collector;

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

    /// Both allocation kinds must survive a round trip through the EBR
    /// collector, which frees them later and elsewhere than the code that
    /// retired them.
    ///
    /// This is the path where a per-kind alignment scheme goes wrong: the
    /// collector cannot see `align_of::<T>()`, so the alignment has to
    /// travel with the retired pointer. If it is dropped and the collector
    /// assumes one alignment for everything, `dealloc` gets a mismatched
    /// `Layout` — undefined behaviour, not a leak, and invisible in a
    /// normal test run. Miri sees it, so this test exists to put the
    /// deferred path in front of Miri **without** needing threads: the
    /// concurrent churn tests reach the same code but Miri does not run
    /// them.
    #[test]
    fn deferred_free_round_trips_both_alignments() {
        let collector = Arc::new(Collector::new());
        let a = NodeAlloc::new();
        a.defer_to(Arc::clone(&collector));

        // A 64-byte-aligned node and raw byte storage at RAW_ALIGN, so
        // both alignments are in flight at once.
        let node = a.alloc_node(BranchL3::new(2));
        let raw = a.alloc_bytes(21);
        assert_eq!(node.as_ptr() as usize % CACHE_LINE, 0);
        assert_eq!(raw.as_ptr() as usize % RAW_ALIGN, 0);
        assert_eq!(a.live_allocs(), 2);

        // SAFETY: freeing exactly what was allocated above, once each.
        unsafe {
            a.free_node(node);
            a.free_bytes(raw, 21);
        }
        // Retired, not yet reclaimed: accounting is logical.
        assert_eq!(a.live_allocs(), 0);
        assert_eq!(a.bytes_in_use(), 0);

        // Drain through the collector — this is where a mismatched layout
        // would reach `dealloc`.
        drop(collector);
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

        // Alignment is now per kind, and the distinction is load-bearing:
        // the `repr(C, align(64))` node types are cast from these pointers,
        // so their alignment is a soundness requirement; raw byte storage
        // is addressed by offset and only needs RAW_ALIGN.
        assert_eq!(n1.as_ptr() as usize % CACHE_LINE, 0, "BranchL3 alignment");
        assert_eq!(n2.as_ptr() as usize % CACHE_LINE, 0, "BranchB alignment");
        assert_eq!(n3.as_ptr() as usize % CACHE_LINE, 0, "BranchU alignment");
        assert_eq!(leaf.as_ptr() as usize % RAW_ALIGN, 0, "raw leaf alignment");
        // Guard the reason the node assertions above are not vacuous: if a
        // node type ever loses its `align(64)` declaration, `alloc_node`
        // would quietly start handing back 16-byte-aligned memory and
        // these would keep passing by luck of the allocator.
        assert_eq!(align_of::<BranchL3>(), CACHE_LINE);
        assert_eq!(align_of::<BranchB>(), CACHE_LINE);
        assert_eq!(align_of::<BranchU>(), CACHE_LINE);

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
