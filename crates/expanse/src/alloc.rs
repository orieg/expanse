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
//! **Slab Allocation & Freelist Pooling**:
//! For size classes $\le 256$ bytes, `NodeAlloc` allocates memory in 4KB
//! slab pages, embedding an intrusive `SlabPage` header and pre-slicing
//! the remaining capacity into local freelist blocks. This avoids system
//! `libc` `malloc`/`free` overhead on tree growth and churn.
//! See `docs/HARDWARE.md` §1.7 and [#431](https://github.com/orieg/expanse/issues/431)
//! for the 4KB page granularity and STLB reach trade-offs at 1M+ keys.
//!
//! The tree itself is single-writer; Phase 7's concurrent wrappers add
//! shared readers, so the counters are (relaxed) atomics.

#[cfg(feature = "std")]
use crate::occ::Collector;
use crate::types::{CACHE_LINE, RAW_ALIGN};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
#[cfg(feature = "std")]
use std::alloc::{Layout, alloc_zeroed, dealloc, handle_alloc_error};
#[cfg(feature = "std")]
use std::sync::{Arc, OnceLock};

#[cfg(not(feature = "std"))]
use core::alloc::Layout;
#[cfg(not(feature = "std"))]
use core_alloc::alloc::{alloc_zeroed, dealloc, handle_alloc_error};

#[repr(C)]
pub(crate) struct FreeBlock {
    pub(crate) next: *mut FreeBlock,
}

#[repr(C)]
pub(crate) struct SlabPage {
    pub(crate) next: *mut SlabPage,
    pub(crate) layout: Layout,
}

pub(crate) const NUM_CLASSES: usize = 62;

pub(crate) const CLASS_SPECS: [(usize, usize); NUM_CLASSES] = [
    (32, CACHE_LINE),
    (64, CACHE_LINE),
    (96, CACHE_LINE),
    (128, CACHE_LINE),
    (256, CACHE_LINE),
    (2048, CACHE_LINE),
    (8, RAW_ALIGN),
    (9, RAW_ALIGN),
    (10, RAW_ALIGN),
    (11, RAW_ALIGN),
    (12, RAW_ALIGN),
    (13, RAW_ALIGN),
    (14, RAW_ALIGN),
    (15, RAW_ALIGN),
    (16, RAW_ALIGN),
    (18, RAW_ALIGN),
    (20, RAW_ALIGN),
    (22, RAW_ALIGN),
    (24, RAW_ALIGN),
    (25, RAW_ALIGN),
    (26, RAW_ALIGN),
    (28, RAW_ALIGN),
    (30, RAW_ALIGN),
    (32, RAW_ALIGN),
    (36, RAW_ALIGN),
    (40, RAW_ALIGN),
    (44, RAW_ALIGN),
    (48, RAW_ALIGN),
    (50, RAW_ALIGN),
    (52, RAW_ALIGN),
    (56, RAW_ALIGN),
    (60, RAW_ALIGN),
    (64, RAW_ALIGN),
    (72, RAW_ALIGN),
    (75, RAW_ALIGN),
    (80, RAW_ALIGN),
    (88, RAW_ALIGN),
    (96, RAW_ALIGN),
    (100, RAW_ALIGN),
    (104, RAW_ALIGN),
    (112, RAW_ALIGN),
    (120, RAW_ALIGN),
    (125, RAW_ALIGN),
    (128, RAW_ALIGN),
    (144, RAW_ALIGN),
    (150, RAW_ALIGN),
    (160, RAW_ALIGN),
    (175, RAW_ALIGN),
    (176, RAW_ALIGN),
    (192, RAW_ALIGN),
    (200, RAW_ALIGN),
    (208, RAW_ALIGN),
    (224, RAW_ALIGN),
    (225, RAW_ALIGN),
    (240, RAW_ALIGN),
    (248, RAW_ALIGN),
    (250, RAW_ALIGN),
    (275, RAW_ALIGN),
    (300, RAW_ALIGN),
    (325, RAW_ALIGN),
    (350, RAW_ALIGN),
    (375, RAW_ALIGN),
];

const NO_CLASS: u8 = 0xFF;

const fn build_raw_class_table() -> [u8; 376] {
    let mut table = [NO_CLASS; 376];
    let mut i = 6;
    while i < NUM_CLASSES {
        let (bytes, align) = CLASS_SPECS[i];
        if align == RAW_ALIGN && bytes < 376 {
            table[bytes] = i as u8;
        }
        i += 1;
    }
    table
}

pub(crate) const RAW_CLASS_TABLE: [u8; 376] = build_raw_class_table();

#[inline(always)]
pub(crate) fn class_for_raw(bytes: usize) -> Option<usize> {
    if bytes < RAW_CLASS_TABLE.len() {
        let c = RAW_CLASS_TABLE[bytes];
        if c != NO_CLASS {
            return Some(c as usize);
        }
    }
    None
}

#[inline(always)]
pub(crate) fn class_for(bytes: usize, align: usize) -> Option<usize> {
    if align == RAW_ALIGN {
        class_for_raw(bytes)
    } else if align == CACHE_LINE {
        match bytes {
            32 => Some(0),
            64 => Some(1),
            96 => Some(2),
            128 => Some(3),
            256 => Some(4),
            2048 => Some(5),
            _ => None,
        }
    } else {
        None
    }
}

/// Bytes an allocation of `bytes` at `align` adds to `bytes_in_use`: the
/// request rounded up to its alignment. This is the one rule behind
/// `mem_used()`, and `validate::NodeBytes` charges every form through it so
/// the per-form breakdown sums to the published number exactly.
#[inline(always)]
pub(crate) const fn accounted_size(bytes: usize, align: usize) -> usize {
    (bytes + (align - 1)) & !(align - 1)
}

/// Allocation handle owned by a tree: hands out zeroed memory — at
/// `align_of::<T>()` via [`Self::alloc_node`], at [`RAW_ALIGN`] via
/// [`Self::alloc_bytes`] — and keeps byte-exact accounting.
///
/// Counters are relaxed atomics (Phase 7): accounting must stay exact
/// when a concurrent wrapper shares the tree across threads, and the
/// counters order nothing — the OCC read protocol carries the fences.
#[derive(Debug)]
pub struct NodeAlloc {
    bytes_in_use: AtomicUsize,
    live_allocs: AtomicUsize,
    /// Phase 7: when set, frees are retired to the collector instead of
    /// released — concurrent readers may still hold the pointers.
    #[cfg(feature = "std")]
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
    freelists: [AtomicPtr<FreeBlock>; NUM_CLASSES],
    slab_pages: AtomicPtr<SlabPage>,
}

impl Default for NodeAlloc {
    fn default() -> Self {
        Self {
            bytes_in_use: AtomicUsize::new(0),
            live_allocs: AtomicUsize::new(0),
            #[cfg(feature = "std")]
            deferred: OnceLock::new(),
            #[cfg(debug_assertions)]
            bracket_depth: AtomicUsize::new(0),
            total_allocs: AtomicUsize::new(0),
            freelists: [const { AtomicPtr::new(core::ptr::null_mut()) }; NUM_CLASSES],
            slab_pages: AtomicPtr::new(core::ptr::null_mut()),
        }
    }
}

impl Drop for NodeAlloc {
    fn drop(&mut self) {
        let mut cur_page = *self.slab_pages.get_mut();
        while !cur_page.is_null() {
            // SAFETY: cur_page is the raw base pointer of an allocated 4096-byte slab page.
            unsafe {
                let next = (*cur_page).next;
                let layout = (*cur_page).layout;
                dealloc(cur_page.cast::<u8>(), layout);
                cur_page = next;
            }
        }

        for (class, &(bytes, align)) in CLASS_SPECS.iter().enumerate() {
            if bytes > 256 || self.occ_enabled() {
                let mut cur = *self.freelists[class].get_mut();
                let layout = Self::layout_for(bytes, align);
                while !cur.is_null() {
                    // SAFETY: cur was allocated with `layout`.
                    let next = unsafe { (*cur).next };
                    // SAFETY: deallocating unreferenced freelist block with its original layout.
                    unsafe { dealloc(cur.cast::<u8>(), layout) };
                    cur = next;
                }
            }
        }
    }
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

    #[inline(always)]
    pub(crate) fn layout_for(bytes: usize, align: usize) -> Layout {
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
    #[inline(always)]
    fn alloc_raw(&self, bytes: usize, align: usize) -> NonNull<u8> {
        let accounted_size = accounted_size(bytes, align);
        self.bytes_in_use
            .fetch_add(accounted_size, Ordering::Relaxed);
        self.live_allocs.fetch_add(1, Ordering::Relaxed);
        self.total_allocs.fetch_add(1, Ordering::Relaxed);

        if let Some(class) = class_for(bytes, align) {
            let head = self.freelists[class].load(Ordering::Relaxed);
            if !head.is_null() {
                // SAFETY: head points to a valid FreeBlock previously freed to this class.
                let next = unsafe { (*head).next };
                self.freelists[class].store(next, Ordering::Relaxed);
                let raw = head.cast::<u8>();
                // SAFETY: zero out the reused memory before returning.
                unsafe { core::ptr::write_bytes(raw, 0, bytes) };
                return NonNull::new(raw).expect("non-null free block");
            }

            #[cfg(feature = "std")]
            let mut raw = if let Some(c) = self.deferred.get() {
                c.pop_freelist(class)
            } else {
                core::ptr::null_mut()
            };

            #[cfg(not(feature = "std"))]
            let mut raw: *mut u8 = core::ptr::null_mut();

            if raw.is_null() && bytes <= 256 && !self.occ_enabled() {
                // Pre-populate freelist from an intrusive 4KB slab page
                const SLAB_PAGE_SIZE: usize = 4096;
                let page_align = align.max(CACHE_LINE);
                let page_layout = Layout::from_size_align(SLAB_PAGE_SIZE, page_align)
                    .expect("valid slab page layout");
                // SAFETY: page_layout has non-zero size.
                let page_raw = unsafe { alloc_zeroed(page_layout) };
                let Some(page_ptr) = NonNull::new(page_raw) else {
                    handle_alloc_error(page_layout)
                };

                // Embed intrusive SlabPage header at the start of the page
                let slab_page = page_ptr.as_ptr().cast::<SlabPage>();
                // SAFETY: page_raw is a fresh 4KB zeroed allocation.
                unsafe {
                    (*slab_page).next = self.slab_pages.load(Ordering::Relaxed);
                    (*slab_page).layout = page_layout;
                }
                self.slab_pages.store(slab_page, Ordering::Relaxed);

                let header_offset = CACHE_LINE;
                let step = accounted_size;
                let available_bytes = SLAB_PAGE_SIZE - header_offset;
                let num_blocks = available_bytes / step;

                for i in (1..num_blocks).rev() {
                    // SAFETY: ptr is inside the allocated SLAB_PAGE_SIZE buffer.
                    let blk_ptr =
                        unsafe { page_raw.add(header_offset + i * step) }.cast::<FreeBlock>();
                    let cur_head = self.freelists[class].load(Ordering::Relaxed);
                    // SAFETY: blk_ptr is valid memory.
                    unsafe { (*blk_ptr).next = cur_head };
                    self.freelists[class].store(blk_ptr, Ordering::Relaxed);
                }

                // SAFETY: header_offset is aligned to CACHE_LINE.
                raw = unsafe { page_raw.add(header_offset) };
            }

            if !raw.is_null() {
                // SAFETY: zero out the reused memory before returning.
                unsafe { core::ptr::write_bytes(raw, 0, bytes) };
                return NonNull::new(raw).expect("non-null free block");
            }
        }

        let layout = Self::layout_for(bytes, align);
        // SAFETY: `layout` has nonzero size (asserted in `layout_for`).
        let raw = unsafe { alloc_zeroed(layout) };
        let Some(ptr) = NonNull::new(raw) else {
            handle_alloc_error(layout)
        };
        ptr
    }

    /// Frees an `alloc_raw(bytes, align)` allocation.
    ///
    /// # Safety
    ///
    /// `ptr` must come from `alloc_raw(bytes, align)` on this handle with
    /// **the same `align`**, not yet freed, and nothing may use it after.
    #[inline(always)]
    unsafe fn free_raw(&self, ptr: NonNull<u8>, bytes: usize, align: usize) {
        let accounted_size = accounted_size(bytes, align);
        self.bytes_in_use
            .fetch_sub(accounted_size, Ordering::Relaxed);
        self.live_allocs.fetch_sub(1, Ordering::Relaxed);

        #[cfg(feature = "std")]
        if let Some(c) = self.deferred.get() {
            // Deferred mode: the structure no longer references `ptr`,
            // but pinned readers may — reclamation waits out the grace
            // period. The alignment travels with the pointer, because the
            // collector frees it later and elsewhere.
            c.retire(ptr, bytes, align);
            return;
        }

        if let Some(class) = class_for(bytes, align) {
            let block = ptr.as_ptr().cast::<FreeBlock>();
            let head = self.freelists[class].load(Ordering::Relaxed);
            // SAFETY: block points to a valid allocation of at least size_of::<FreeBlock>().
            unsafe { (*block).next = head };
            self.freelists[class].store(block, Ordering::Relaxed);
            return;
        }

        let layout = Self::layout_for(bytes, align);
        // SAFETY: per this function's contract, `ptr`/`layout` match
        // the original allocation.
        unsafe { dealloc(ptr.as_ptr(), layout) };
    }

    /// Allocates `bytes` of zeroed memory for **raw byte storage** —
    /// packed leaves and subarrays, at [`RAW_ALIGN`]. Never use this for a
    /// type that declares a stronger alignment; that is `alloc_node`.
    #[must_use]
    #[inline(always)]
    pub fn alloc_bytes(&self, bytes: usize) -> NonNull<u8> {
        self.alloc_raw(bytes, RAW_ALIGN)
    }

    /// Frees an allocation made by [`Self::alloc_bytes`] with this handle.
    ///
    /// # Safety
    ///
    /// `ptr` must come from `alloc_bytes(bytes)` on this handle, not yet
    /// freed, and nothing may use it afterwards.
    #[inline(always)]
    pub unsafe fn free_bytes(&self, ptr: NonNull<u8>, bytes: usize) {
        // SAFETY: `alloc_bytes` is the only producer of these pointers and
        // always uses RAW_ALIGN, so the layout matches.
        unsafe { self.free_raw(ptr, bytes, RAW_ALIGN) };
    }

    /// True once this tree is shared through a Phase 7 concurrent
    /// wrapper: the mutation engine then maintains per-node OCC versions
    /// (single-threaded trees skip those fences entirely).
    #[cfg(feature = "std")]
    #[inline]
    pub(crate) fn occ_enabled(&self) -> bool {
        self.deferred.get().is_some()
    }

    /// True once this tree is shared through a Phase 7 concurrent
    /// wrapper: the mutation engine then maintains per-node OCC versions
    /// (single-threaded trees skip those fences entirely).
    #[cfg(not(feature = "std"))]
    #[inline]
    pub(crate) fn occ_enabled(&self) -> bool {
        false
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

    /// Switches this allocator to deferred reclamation through
    /// `collector`, permanently. Free calls from this point on route
    /// through `collector.retire` instead of returning immediately to
    /// freelists or the system allocator.
    ///
    /// Idempotent: calling with the same collector handle is a no-op;
    /// calling with a different collector panics.
    ///
    /// # Slabs and migration
    ///
    /// `defer_to` requires an allocator that has **never allocated from
    /// 4KB slab pages** (i.e. was deferred before any node was created,
    /// or has only ever performed allocations above the slab ceiling).
    /// Slab-carved blocks share a 4KB page and cannot be retired to
    /// the collector, which frees blocks individually after the grace
    /// period. Migrating a slab-using allocator therefore aborts in the
    /// allocator later; wrap populated structures by **rebuilding** them
    /// through a pre-deferred allocator instead (see the `sync` wrappers'
    /// `From` impls).
    #[cfg(feature = "std")]
    pub fn defer_to(&self, collector: Arc<Collector>) {
        // Hard assert (not debug): the failure mode this guards is silent
        // heap corruption in release builds, and this is a cold once-per-
        // structure call.
        assert!(
            self.slab_pages.load(Ordering::Relaxed).is_null(),
            "defer_to on an allocator that already slab-carved memory: retired \
             slab-carved blocks would later be dealloc'ed individually (heap \
             corruption). Rebuild the structure through a pre-deferred \
             allocator instead."
        );
        let stored = self.deferred.get_or_init(|| Arc::clone(&collector));
        assert!(
            Arc::ptr_eq(stored, &collector),
            "NodeAlloc already deferred to a different collector"
        );
    }

    /// Allocates a node and moves `init` into it.
    #[must_use]
    #[inline(always)]
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

    /// Allocates a node of type T with all bytes zero-initialized.
    ///
    /// This is useful for construct-in-place node initialization, which avoids
    /// stack-allocation and copy overhead.
    #[inline(always)]
    #[must_use]
    pub fn alloc_node_zeroed<T>(&self) -> NonNull<T> {
        debug_assert!(align_of::<T>() <= CACHE_LINE);
        self.alloc_raw(size_of::<T>(), align_of::<T>()).cast::<T>()
    }

    /// Frees a node allocated by [`Self::alloc_node`], dropping its value.
    ///
    /// # Safety
    ///
    /// `ptr` must come from `alloc_node::<T>` on this handle, not yet
    /// freed, and nothing may use it afterwards.
    #[inline(always)]
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
    #[cfg(feature = "std")]
    use crate::occ::Collector;
    #[cfg(feature = "std")]
    use core_alloc::sync::Arc;
    use core_alloc::vec::Vec;

    #[test]
    #[cfg(feature = "std")]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "outside any version bracket")]
    fn negative_control_bracket_assert_must_fire() {
        // docs/TESTING.md: an assertion that has never fired is not known
        // to work. A concurrently-shared tree with no bracket open must
        // trip the Phase 7 coverage check.
        let a = NodeAlloc::new();
        a.defer_to(Arc::new(crate::occ::Collector::new()));
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
    #[cfg(feature = "std")]
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
    #[cfg(feature = "std")]
    fn bracket_assert_is_quiet_when_covered() {
        // Single-threaded trees have no readers: never trips.
        let a = NodeAlloc::new();
        a.assert_bracketed();
        // Shared tree with a bracket open: also fine.
        a.defer_to(Arc::new(crate::occ::Collector::new()));
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
        let n4 = a.alloc_node_zeroed::<BranchL3>();
        let leaf = a.alloc_bytes(21);
        assert_eq!(a.bytes_in_use(), 64 + 128 + 4160 + 64 + 32);
        assert_eq!(a.live_allocs(), 5);

        // Alignment is now per kind, and the distinction is load-bearing:
        // the `repr(C, align(64))` node types are cast from these pointers,
        // so their alignment is a soundness requirement; raw byte storage
        // is addressed by offset and only needs RAW_ALIGN.
        assert_eq!(n1.as_ptr() as usize % CACHE_LINE, 0, "BranchL3 alignment");
        assert_eq!(n2.as_ptr() as usize % CACHE_LINE, 0, "BranchB alignment");
        assert_eq!(n3.as_ptr() as usize % CACHE_LINE, 0, "BranchU alignment");
        assert_eq!(
            n4.as_ptr() as usize % CACHE_LINE,
            0,
            "alloc_node_zeroed BranchL3 alignment"
        );
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

        // Verify that alloc_node_zeroed is actually zeroed
        // SAFETY: n4 is a live BranchL3 pointer allocated above.
        unsafe {
            assert_eq!((*n4.as_ptr()).hdr.version, 0);
            assert_eq!((*n4.as_ptr()).hdr.level, 0);
            for edge in &(*n4.as_ptr()).edges {
                assert!(edge.is_null());
            }
        }

        // SAFETY: freeing exactly what was allocated above, once.
        unsafe {
            a.free_node(n1);
            a.free_node(n2);
            a.free_node(n3);
            a.free_node(n4);
            a.free_bytes(leaf, 21);
        }
        assert_eq!(a.bytes_in_use(), 0);
        assert_eq!(a.live_allocs(), 0);
    }

    #[test]
    fn slab_freelist_recycling_round_trip() {
        let a = NodeAlloc::new();
        let p1 = a.alloc_bytes(32);
        let total_after_p1 = a.total_allocs();
        assert_eq!(total_after_p1, 1);

        // Write non-zero data into p1
        // SAFETY: p1 is a live 32-byte allocation.
        unsafe {
            core::ptr::write_bytes(p1.as_ptr(), 0xAA, 32);
            a.free_bytes(p1, 32);
        }
        assert_eq!(a.live_allocs(), 0);
        assert_eq!(a.bytes_in_use(), 0);

        // Allocating the same size class must pop from the freelist
        let p2 = a.alloc_bytes(32);
        assert_eq!(p2, p1);
        assert_eq!(a.total_allocs(), 2);
        assert_eq!(a.live_allocs(), 1);
        assert_eq!(a.bytes_in_use(), 32);

        // Verify that the reused memory is guaranteed to be zeroed
        for i in 0..32 {
            // SAFETY: p2 is a live 32-byte allocation.
            unsafe {
                assert_eq!(*p2.as_ptr().add(i), 0);
            }
        }

        // SAFETY: freeing p2 allocation.
        unsafe {
            a.free_bytes(p2, 32);
        }
        assert_eq!(a.live_allocs(), 0);
        assert_eq!(a.bytes_in_use(), 0);
    }

    #[test]
    #[cfg(feature = "std")]
    fn occ_collector_freelist_recycling_round_trip() {
        let collector = Arc::new(Collector::new());
        let a = NodeAlloc::new();
        a.defer_to(Arc::clone(&collector));

        let p1 = a.alloc_bytes(32);
        let total_after_p1 = a.total_allocs();
        assert_eq!(total_after_p1, 1);

        // Write non-zero data into p1
        // SAFETY: p1 is a live 32-byte allocation.
        unsafe {
            core::ptr::write_bytes(p1.as_ptr(), 0xBB, 32);
            a.free_bytes(p1, 32);
        }

        // Before epoch advance, p1 is queued in collector bins
        assert_eq!(a.total_allocs(), 1);

        // Advance epochs to reclaim the retired block
        collector.try_advance();
        collector.try_advance();

        // Allocating the same size class must pop from the recycled freelist
        let p2 = a.alloc_bytes(32);
        assert_eq!(p2, p1);
        assert_eq!(a.total_allocs(), 2);
        assert_eq!(a.live_allocs(), 1);
        assert_eq!(a.bytes_in_use(), 32);

        // Memory must be zeroed upon reuse
        for i in 0..32 {
            // SAFETY: p2 is a live 32-byte allocation.
            unsafe {
                assert_eq!(*p2.as_ptr().add(i), 0);
            }
        }

        // SAFETY: freeing p2 allocation.
        unsafe {
            a.free_bytes(p2, 32);
        }
    }

    #[test]
    fn slab_page_pooling_and_cleanup_test() {
        let a = NodeAlloc::new();
        let count = if cfg!(miri) { 10 } else { 100 };
        // Allocate blocks (more than 1 block, spanning multiple freelist pops from 4KB pages)
        let mut ptrs = Vec::new();
        for _ in 0..count {
            ptrs.push(a.alloc_bytes(32));
        }
        assert_eq!(a.live_allocs(), count);
        assert_eq!(a.bytes_in_use(), count * 32);

        // Free all blocks
        for ptr in ptrs {
            // SAFETY: freeing allocated pointer.
            unsafe { a.free_bytes(ptr, 32) };
        }
        assert_eq!(a.live_allocs(), 0);
        assert_eq!(a.bytes_in_use(), 0);

        // Allocate blocks again: they must all be fulfilled from the freelist
        let mut ptrs2 = Vec::new();
        for _ in 0..count {
            ptrs2.push(a.alloc_bytes(32));
        }
        assert_eq!(a.live_allocs(), count);
        assert_eq!(a.bytes_in_use(), count * 32);

        for ptr in ptrs2 {
            // SAFETY: freeing allocated pointer.
            unsafe { a.free_bytes(ptr, 32) };
        }
        assert_eq!(a.live_allocs(), 0);
        assert_eq!(a.bytes_in_use(), 0);
    }

    #[test]
    fn raw_class_table_matches_class_specs() {
        for (class, &(bytes, align)) in CLASS_SPECS.iter().enumerate() {
            if align == RAW_ALIGN {
                assert_eq!(class_for_raw(bytes), Some(class));
                assert_eq!(class_for(bytes, RAW_ALIGN), Some(class));
            }
        }
        for b in 0..8 {
            assert_eq!(class_for_raw(b), None);
        }
        assert_eq!(class_for_raw(376), None);
        assert_eq!(class_for_raw(1000), None);
    }
}
