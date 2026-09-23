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
    /// The size class this page was carved into. It fixes the page's layout
    /// ([`slab_page_layout`]) and lets [`NodeAlloc::bytes_held`] count the
    /// blocks the page holds.
    pub(crate) class: usize,
}

/// Bytes of a slab page's header: blocks start one cache line in.
const SLAB_HEADER: usize = CACHE_LINE;
/// Bytes of one slab page.
const SLAB_PAGE_SIZE: usize = 4096;

/// The layout a slab page of `class` is allocated and freed with.
fn slab_page_layout(class: usize) -> Layout {
    Layout::from_size_align(SLAB_PAGE_SIZE, CLASS_SPECS[class].1.max(CACHE_LINE))
        .expect("valid slab page layout")
}

/// Blocks a slab page of `class` is carved into.
const fn slab_blocks(class: usize) -> usize {
    let (bytes, align) = CLASS_SPECS[class];
    (SLAB_PAGE_SIZE - SLAB_HEADER) / accounted_size(bytes, align)
}

/// Whether a class is served from slab pages (every block of it came from
/// one) rather than straight from the system allocator.
const fn is_slab_class(class: usize) -> bool {
    CLASS_SPECS[class].0 <= 256
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

/// For each request size, the smallest raw size class that holds it at the
/// same accounted size; a size with no such class, or above the table, maps
/// to itself.
#[cfg(any(feature = "packed-suffix", test))]
const fn build_raw_fit_table() -> [u16; 376] {
    let mut table = [0u16; 376];
    let mut bytes = 0;
    while bytes < 376 {
        let mut fit = bytes;
        let mut i = 6;
        while i < NUM_CLASSES {
            let (b, align) = CLASS_SPECS[i];
            if align == RAW_ALIGN && b >= bytes {
                if accounted_size(b, RAW_ALIGN) == accounted_size(bytes, RAW_ALIGN) {
                    fit = b;
                }
                break;
            }
            i += 1;
        }
        table[bytes] = fit as u16;
        bytes += 1;
    }
    table
}

#[cfg(any(feature = "packed-suffix", test))]
const RAW_FIT_TABLE: [u16; 376] = build_raw_fit_table();

/// The request size [`NodeAlloc::alloc_bytes`] should be given for a block of
/// at least `bytes`: the smallest raw size class holding it, so the block
/// comes from a class (slab-carved up to 256 bytes) rather than straight from
/// the system allocator — when that class has the same accounted size, which
/// holds for every size except the few just above a class gap (251..=256
/// lies between the 250- and 275-byte classes). Otherwise `bytes`. For callers whose sizes
/// vary freely, such as string suffix leaves; the engine's own node sizes are
/// class sizes already.
#[cfg(any(feature = "packed-suffix", test))]
#[inline]
pub(crate) fn raw_class_fit(bytes: usize) -> usize {
    if bytes < RAW_FIT_TABLE.len() {
        RAW_FIT_TABLE[bytes] as usize
    } else {
        bytes
    }
}

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

#[cfg(all(feature = "std", not(feature = "ablation-unsharded-alloc")))]
pub(crate) const NUM_ALLOC_SHARDS: usize = crate::occ::MAX_WRITER_SLOTS;

#[cfg(all(feature = "std", not(feature = "ablation-unsharded-alloc")))]
#[derive(Debug)]
#[repr(align(64))]
pub(crate) struct AllocShard {
    pub(crate) bytes_in_use: core::sync::atomic::AtomicIsize,
    pub(crate) live_allocs: core::sync::atomic::AtomicIsize,
    pub(crate) total_allocs: AtomicUsize,
}

#[cfg(all(feature = "std", not(feature = "ablation-unsharded-alloc")))]
impl AllocShard {
    pub(crate) fn new() -> Self {
        Self {
            bytes_in_use: core::sync::atomic::AtomicIsize::new(0),
            live_allocs: core::sync::atomic::AtomicIsize::new(0),
            total_allocs: AtomicUsize::new(0),
        }
    }
}

/// What [`NodeAlloc::defer_to`] publishes: the collector, and with it the
/// per-writer accounting shards.
///
/// One cell rather than two. The allocator's runtime-dispatched entry points
/// (`alloc_bytes`, `free_bytes`, `alloc_node_zeroed`, `free_node`) are reached
/// from plain walks too, so every test of "is this tree deferred?" on them is
/// executed by a tree that never becomes concurrent. With the shards behind
/// their own `OnceLock`, such a tree paid a second test per allocation and per
/// free for a mechanism it cannot use (AGENTS.md §2.1 invariant 5).
///
/// Owned per-`NodeAlloc` on purpose: a `Collector` may be shared across
/// allocators (`blobmap.rs`) while `mem_used()` is summed per allocator.
#[cfg(feature = "std")]
#[derive(Debug)]
pub(crate) struct Deferred {
    collector: Arc<Collector>,
    #[cfg(not(feature = "ablation-unsharded-alloc"))]
    shards: [AllocShard; NUM_ALLOC_SHARDS],
}

/// How [`Deferred`] sits in the allocator. Boxed in the default build, so the
/// 4 KiB of shards are allocated cold in `defer_to` and `NodeAlloc` keeps the
/// size it had before the shards existed. Under `ablation-unsharded-alloc`
/// there are no shards, and the collector handle stays inline exactly as it
/// was before the promotion, so the ablation measures that state and not an
/// extra pointer hop (AGENTS.md §2.7).
#[cfg(all(feature = "std", not(feature = "ablation-unsharded-alloc")))]
type DeferredCell = core_alloc::boxed::Box<Deferred>;
#[cfg(all(feature = "std", feature = "ablation-unsharded-alloc"))]
type DeferredCell = Deferred;

#[cfg(feature = "std")]
impl Deferred {
    /// Accounts one allocation of `size` bytes on the calling writer's shard.
    /// Under `ablation-unsharded-alloc` there are none, and it lands on
    /// `a`'s inline counters, shared across writers as before the promotion.
    #[inline(always)]
    fn charge(&self, a: &NodeAlloc, size: usize) {
        #[cfg(not(feature = "ablation-unsharded-alloc"))]
        {
            let _ = a;
            let sh = &self.shards[crate::occ::writer_slot()];
            sh.bytes_in_use.fetch_add(size as isize, Ordering::Relaxed);
            sh.live_allocs.fetch_add(1, Ordering::Relaxed);
            sh.total_allocs.fetch_add(1, Ordering::Relaxed);
        }
        #[cfg(feature = "ablation-unsharded-alloc")]
        {
            a.bytes_in_use.fetch_add(size, Ordering::Relaxed);
            a.live_allocs.fetch_add(1, Ordering::Relaxed);
            a.total_allocs.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Inverse of [`Self::charge`]. A free may land on a different shard than
    /// its allocation, which is why the shard counters are signed.
    #[inline(always)]
    fn discharge(&self, a: &NodeAlloc, size: usize) {
        #[cfg(not(feature = "ablation-unsharded-alloc"))]
        {
            let _ = a;
            let sh = &self.shards[crate::occ::writer_slot()];
            sh.bytes_in_use.fetch_sub(size as isize, Ordering::Relaxed);
            sh.live_allocs.fetch_sub(1, Ordering::Relaxed);
        }
        #[cfg(feature = "ablation-unsharded-alloc")]
        {
            a.bytes_in_use.fetch_sub(size, Ordering::Relaxed);
            a.live_allocs.fetch_sub(1, Ordering::Relaxed);
        }
    }

    fn new(collector: Arc<Collector>) -> DeferredCell {
        let d = Self {
            collector,
            #[cfg(not(feature = "ablation-unsharded-alloc"))]
            shards: core::array::from_fn(|_| AllocShard::new()),
        };
        #[cfg(not(feature = "ablation-unsharded-alloc"))]
        {
            core_alloc::boxed::Box::new(d)
        }
        #[cfg(feature = "ablation-unsharded-alloc")]
        {
            d
        }
    }
}

/// `NodeAlloc::root_cover`: the wrapper brackets whole operations.
#[cfg(feature = "std")]
const ROOT_COVER_WRAPPER: u8 = 0;
/// `NodeAlloc::root_cover`: the engine brackets root-state writes.
#[cfg(feature = "std")]
const ROOT_COVER_ENGINE: u8 = 1;
/// `NodeAlloc::root_cover`: the engine covers the root, and a wrapper holds
/// the word for the current covered write (#1086).
#[cfg(feature = "std")]
const ROOT_COVER_HELD: u8 = 2;

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
    /// released — concurrent readers may still hold the pointers. The same
    /// cell carries the per-writer accounting shards, so one test of it
    /// decides both where a call is accounted and where its block goes, and a
    /// tree that never becomes concurrent carries no second word.
    #[cfg(feature = "std")]
    deferred: OnceLock<DeferredCell>,
    /// Phase 7 / #568 PR 3: who brackets root-state writes, one of the
    /// `ROOT_COVER_*` states. `ROOT_COVER_WRAPPER` when the wrapper brackets
    /// whole operations in `Shared::write` (string, bytes, blob), so the
    /// engine's tree cover is a no-op and the word is never opened twice.
    /// `ROOT_COVER_ENGINE` for the map and set, whose engine walks run the
    /// shared monomorph that brackets root state itself. `ROOT_COVER_HELD`
    /// while a map or set wrapper holds the word around a whole covered
    /// write (#1086): the monomorph is unchanged, and its own tree bracket
    /// is a no-op for that operation. The bytes wrapper also holds the word
    /// around its covered writes, but its engine is in `ROOT_COVER_WRAPPER`
    /// and the hand-over is a no-op there.
    #[cfg(feature = "std")]
    root_cover: core::sync::atomic::AtomicU8,
    /// #568 PR 3: the tree-level version word this tree's root state is
    /// bracketed by — the wrapper's `Shared::version`, bound once by
    /// [`Self::bind_tree_word`] after the wrapper is boxed, so the address
    /// is stable for the life of the tree. Null until then.
    #[cfg(feature = "std")]
    tree_word: AtomicPtr<crate::occ::SeqVersion>,
    /// Cumulative allocation count (never decremented). Lets a test
    /// separate the engine's own node/leaf allocations from incidental
    /// scratch allocations elsewhere in a code path — see
    /// `tests/no_heap_churn.rs`.
    total_allocs: AtomicUsize,
    /// Per-size-class free blocks, and the 4 KiB slab pages carved to
    /// pre-populate them.
    ///
    /// **Single-writer, by design.** Every update to these two is a plain
    /// load/store pair, never a CAS: a non-OCC tree is owned by one thread at
    /// a time, and an OCC tree allocates through the collector's locked
    /// freelists instead — which is what [`Self::defer_to`] asserts by
    /// refusing an allocator that has already carved a slab or populated a
    /// class. Two threads in here at once lose an update silently: a dropped
    /// slab page leaks the whole page, and a dropped freelist link splices
    /// blocks out of the list. [`Self::enter_bookkeeping`] turns that into a
    /// panic in debug builds.
    freelists: [AtomicPtr<FreeBlock>; NUM_CLASSES],
    /// See [`Self::freelists`] — same single-writer domain.
    slab_pages: AtomicPtr<SlabPage>,
    /// Set while a thread is inside the single-writer region above.
    #[cfg(debug_assertions)]
    bookkeeping_busy: core::sync::atomic::AtomicBool,
}

impl Default for NodeAlloc {
    fn default() -> Self {
        Self {
            bytes_in_use: AtomicUsize::new(0),
            live_allocs: AtomicUsize::new(0),
            #[cfg(feature = "std")]
            deferred: OnceLock::new(),
            #[cfg(feature = "std")]
            root_cover: core::sync::atomic::AtomicU8::new(ROOT_COVER_WRAPPER),
            #[cfg(feature = "std")]
            tree_word: AtomicPtr::new(core::ptr::null_mut()),
            total_allocs: AtomicUsize::new(0),
            freelists: [const { AtomicPtr::new(core::ptr::null_mut()) }; NUM_CLASSES],
            slab_pages: AtomicPtr::new(core::ptr::null_mut()),
            #[cfg(debug_assertions)]
            bookkeeping_busy: core::sync::atomic::AtomicBool::new(false),
        }
    }
}

/// Holds [`NodeAlloc`]'s single-writer bookkeeping region for its lifetime.
#[cfg(debug_assertions)]
struct BookkeepingGuard<'a>(&'a core::sync::atomic::AtomicBool);

#[cfg(debug_assertions)]
impl Drop for BookkeepingGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(debug_assertions)]
impl NodeAlloc {
    /// Claims the single-writer region documented on [`Self::freelists`],
    /// panicking when another thread is already inside it.
    ///
    /// Compiled out entirely when `debug_assertions` is off, so the release
    /// and benchmark builds carry neither the flag nor this exchange. It is a
    /// detector, not a fix: it reports the caller that broke the invariant
    /// instead of leaving a lost slab page to surface as a LeakSanitizer
    /// report against whichever test the process happened to end on.
    fn enter_bookkeeping(&self) -> BookkeepingGuard<'_> {
        assert!(
            self.bookkeeping_busy
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok(),
            "two threads are inside one NodeAlloc's per-tree freelists at once. They are \
             single-writer: their updates are load/store pairs, not CAS, so concurrent use \
             drops slab pages and freelist links. Share a tree through a Sync* wrapper \
             (which defers to the collector's locked freelists before carving anything), or \
             give each thread its own NodeAlloc."
        );
        BookkeepingGuard(&self.bookkeeping_busy)
    }
}

impl Drop for NodeAlloc {
    fn drop(&mut self) {
        let mut cur_page = *self.slab_pages.get_mut();
        while !cur_page.is_null() {
            // SAFETY: cur_page is the raw base pointer of an allocated 4096-byte slab page.
            unsafe {
                let next = (*cur_page).next;
                let layout = slab_page_layout((*cur_page).class);
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
        // An undeferred tree returns after this one load; only a deferred one
        // folds in its shards' net deltas.
        let inline = self.bytes_in_use.load(Ordering::Relaxed);
        #[cfg(all(feature = "std", not(feature = "ablation-unsharded-alloc")))]
        if let Some(d) = self.deferred.get() {
            let total = d.shards.iter().fold(inline as isize, |acc, s| {
                acc.saturating_add(s.bytes_in_use.load(Ordering::Relaxed))
            });
            return if total < 0 { 0 } else { total as usize };
        }
        inline
    }

    /// Bytes this handle holds from the system allocator: the live bytes
    /// [`Self::bytes_in_use`] counts, plus freed blocks kept on the
    /// per-tree size-class freelists for reuse, plus the unused blocks and
    /// headers of the 4 KiB slab pages small classes are carved from.
    ///
    /// Freed blocks are recycled within the tree and returned to the system
    /// only when the tree is dropped, so this, not `bytes_in_use`, is what
    /// the tree costs the process before the system allocator's own
    /// per-allocation overhead (chunk headers and size-class rounding),
    /// which depends on the allocator and is not included.
    ///
    /// Computed on demand by walking the slab pages and freelists: no
    /// counter is updated on the allocation path. O(slab pages + free
    /// blocks).
    ///
    /// A tree shared through a concurrent wrapper allocates from its
    /// collector's pools, which other trees may share and which are not
    /// attributed here; for such a tree this is the per-tree share, which
    /// is `bytes_in_use` plus any slab pages carved before it was shared.
    #[must_use]
    pub fn bytes_held(&self) -> usize {
        let mut pages = [0usize; NUM_CLASSES];
        let mut page = self.slab_pages.load(Ordering::Relaxed);
        while !page.is_null() {
            // SAFETY: every entry of the slab list is a live page this handle
            // carved, and its header was written before it was linked. The
            // list is single-writer and `&self` excludes a plain tree's writer.
            unsafe {
                pages[(*page).class] += 1;
                page = (*page).next;
            }
        }
        let mut slab_total = 0;
        let mut slab_carved_live = 0;
        let mut free_system = 0;
        for (class, &(bytes, align)) in CLASS_SPECS.iter().enumerate() {
            let step = accounted_size(bytes, align);
            let mut free = 0usize;
            let mut cur = self.freelists[class].load(Ordering::Relaxed);
            while !cur.is_null() {
                free += 1;
                // SAFETY: freelist entries are free blocks of this class owned
                // by this handle; `next` was written when each was pushed.
                cur = unsafe { (*cur).next };
            }
            if is_slab_class(class) {
                slab_total += pages[class] * SLAB_PAGE_SIZE;
                slab_carved_live += (pages[class] * slab_blocks(class)).saturating_sub(free) * step;
            } else {
                free_system += free * step;
            }
        }
        // Live bytes not on a slab page came from the system allocator.
        let live_system = self.bytes_in_use().saturating_sub(slab_carved_live);
        slab_total + live_system + free_system
    }

    /// Returns retained memory to the system allocator: every freed block
    /// of a class served straight from the system allocator, and every slab
    /// page none of whose blocks is in use. Returns the bytes released,
    /// by the rule [`Self::bytes_held`] counts them, so `bytes_held` falls by
    /// exactly this much.
    ///
    /// Live nodes never move, and blocks on a page that still holds a live
    /// node stay on their freelists. Costs O(slab pages · log slab pages +
    /// free blocks) and one scratch allocation; nothing on the allocation
    /// path changes.
    ///
    /// `&mut self`: the freelists and slab list are single-writer. A tree
    /// shared through a concurrent wrapper keeps no per-tree freelists, so
    /// this returns 0 for it.
    pub fn release_free(&mut self) -> usize {
        #[cfg(feature = "std")]
        if self.deferred.get().is_some() {
            return 0;
        }
        let mut released = 0;

        // Classes above the slab ceiling: each free block is its own
        // system allocation.
        for (class, &(bytes, align)) in CLASS_SPECS.iter().enumerate() {
            if is_slab_class(class) {
                continue;
            }
            let layout = Self::layout_for(bytes, align);
            let mut cur =
                core::mem::replace(self.freelists[class].get_mut(), core::ptr::null_mut());
            while !cur.is_null() {
                // SAFETY: `cur` is a free block of this class, allocated by
                // `alloc_system(bytes, align)`, and no longer reachable from
                // the freelist this loop just detached.
                let next = unsafe { (*cur).next };
                // SAFETY: as above; `layout` is the one it was allocated with.
                unsafe { dealloc(cur.cast::<u8>(), layout) };
                released += accounted_size(bytes, align);
                cur = next;
            }
        }

        // Slab pages, sorted by base address so a block finds its page by
        // binary search: (base, page, free blocks counted, blocks carved).
        let mut pages: core_alloc::vec::Vec<(usize, *mut SlabPage, usize, usize)> =
            core_alloc::vec::Vec::new();
        let mut page = *self.slab_pages.get_mut();
        while !page.is_null() {
            // SAFETY: slab list entries are live pages with written headers.
            unsafe {
                pages.push((page as usize, page, 0, slab_blocks((*page).class)));
                page = (*page).next;
            }
        }
        if pages.is_empty() {
            return released;
        }
        pages.sort_unstable_by_key(|p| p.0);
        let page_of = |pages: &[(usize, *mut SlabPage, usize, usize)], addr: usize| -> usize {
            let i = pages.partition_point(|p| p.0 <= addr) - 1;
            debug_assert!(
                addr < pages[i].0 + SLAB_PAGE_SIZE,
                "free block outside every slab page"
            );
            i
        };
        // Count each page's free blocks.
        for class in 0..NUM_CLASSES {
            if !is_slab_class(class) {
                continue;
            }
            let mut cur = *self.freelists[class].get_mut();
            while !cur.is_null() {
                let i = page_of(&pages, cur as usize);
                pages[i].2 += 1;
                // SAFETY: freelist entries are free blocks with `next` written.
                cur = unsafe { (*cur).next };
            }
        }
        // A page is free when every block carved from it is on a freelist.
        let is_free = |p: &(usize, *mut SlabPage, usize, usize)| p.2 == p.3;
        if !pages.iter().any(is_free) {
            return released;
        }
        // Drop the blocks of free pages from their freelists, keeping order.
        for class in 0..NUM_CLASSES {
            if !is_slab_class(class) {
                continue;
            }
            let mut kept: *mut FreeBlock = core::ptr::null_mut();
            let mut tail: Option<NonNull<FreeBlock>> = None;
            let mut cur = *self.freelists[class].get_mut();
            while let Some(block) = NonNull::new(cur) {
                // SAFETY: freelist entries are free blocks with `next` written.
                let next = unsafe { (*block.as_ptr()).next };
                if !is_free(&pages[page_of(&pages, cur as usize)]) {
                    match tail {
                        None => kept = cur,
                        // SAFETY: `t` is a kept free block of this class.
                        Some(t) => unsafe { (*t.as_ptr()).next = cur },
                    }
                    tail = Some(block);
                }
                cur = next;
            }
            if let Some(t) = tail {
                // SAFETY: `t` is a kept free block of this class.
                unsafe { (*t.as_ptr()).next = core::ptr::null_mut() };
            }
            *self.freelists[class].get_mut() = kept;
        }
        // Rebuild the slab list from the kept pages and free the rest.
        let mut head: *mut SlabPage = core::ptr::null_mut();
        for p in pages.iter().rev() {
            if is_free(p) {
                // SAFETY: the page is live, no block on it is live or listed
                // any more, and its header's class fixes the layout it was
                // carved with.
                unsafe {
                    let layout = slab_page_layout((*p.1).class);
                    dealloc(p.1.cast::<u8>(), layout);
                }
                released += SLAB_PAGE_SIZE;
            } else {
                // SAFETY: a kept page's header is live and single-writer here.
                unsafe { (*p.1).next = head };
                head = p.1;
            }
        }
        *self.slab_pages.get_mut() = head;
        released
    }

    /// Number of live allocations (diagnostics / leak assertions in tests).
    #[must_use]
    pub fn live_allocs(&self) -> usize {
        // An undeferred tree returns after this one load; only a deferred one
        // folds in its shards' net deltas.
        let inline = self.live_allocs.load(Ordering::Relaxed);
        #[cfg(all(feature = "std", not(feature = "ablation-unsharded-alloc")))]
        if let Some(d) = self.deferred.get() {
            let total = d.shards.iter().fold(inline as isize, |acc, s| {
                acc.saturating_add(s.live_allocs.load(Ordering::Relaxed))
            });
            return if total < 0 { 0 } else { total as usize };
        }
        inline
    }

    /// Cumulative allocations made through this handle since it was
    /// created (never decremented). Used to separate the engine's own
    /// node and leaf allocations from incidental scratch allocations in
    /// the same code path.
    #[must_use]
    pub fn total_allocs(&self) -> usize {
        let inline = self.total_allocs.load(Ordering::Relaxed);
        #[cfg(all(feature = "std", not(feature = "ablation-unsharded-alloc")))]
        if let Some(d) = self.deferred.get() {
            return d.shards.iter().fold(inline, |acc, s| {
                acc.saturating_add(s.total_allocs.load(Ordering::Relaxed))
            });
        }
        inline
    }

    /// The deferred cell when this call may act on it: `None` on every
    /// `OCC = false` path at compile time, and on an undeferred tree at run
    /// time. A caller tests it once, and a deferred tree leaves through that test.
    #[cfg(feature = "std")]
    #[inline(always)]
    fn deferred_if<const OCC: bool>(&self) -> Option<&Deferred> {
        if OCC {
            self.deferred.get().map(|d| -> &Deferred { d })
        } else {
            None
        }
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
    fn alloc_raw<const OCC: bool>(&self, bytes: usize, align: usize) -> NonNull<u8> {
        #[cfg(feature = "std")]
        debug_assert!(
            OCC || self.deferred.get().is_none(),
            "plain allocator path (OCC=false) invoked on an allocator with deferred reclamation enabled"
        );
        let accounted_size = accounted_size(bytes, align);

        // The one runtime test of the deferred cell (`OCC = false` folds it to
        // `None`). A deferred tree leaves here, so everything below is the
        // plain path and shares no accounting or freelist code with it: with
        // the two interleaved, LLVM merged their counter updates and the
        // plain path paid for the merge.
        #[cfg(feature = "std")]
        if let Some(d) = self.deferred_if::<OCC>() {
            d.charge(self, accounted_size);
            if let Some(class) = class_for(bytes, align) {
                let raw = d.collector.pop_freelist(class);
                if !raw.is_null() {
                    // SAFETY: zero out the reused memory before returning.
                    unsafe { core::ptr::write_bytes(raw, 0, bytes) };
                    return NonNull::new(raw).expect("non-null free block");
                }
            }
            // Per-tree freelists are single-writer, so a deferred tree never
            // pops them: a collector miss goes to the system allocator.
            return Self::alloc_system(bytes, align);
        }

        self.bytes_in_use
            .fetch_add(accounted_size, Ordering::Relaxed);
        self.live_allocs.fetch_add(1, Ordering::Relaxed);
        self.total_allocs.fetch_add(1, Ordering::Relaxed);

        if let Some(class) = class_for(bytes, align) {
            let head = self.freelists[class].load(Ordering::Relaxed);
            if !head.is_null() {
                #[cfg(debug_assertions)]
                let _bookkeeping = self.enter_bookkeeping();
                // The guard alone cannot see the whole pop: the load above it
                // is outside the region, so two threads that read one `head`
                // and then take the guard one after another never overlap
                // inside it, and each returns the same block to its caller.
                // Re-reading under the guard catches that interleaving, since
                // the first thread's store lands before the second gets in.
                debug_assert_eq!(
                    self.freelists[class].load(Ordering::Relaxed),
                    head,
                    "this class's freelist head moved between the load and the pop, so \
                     another thread is inside one NodeAlloc's per-tree freelists. They \
                     are single-writer: their updates are load/store pairs, not CAS, so \
                     concurrent use hands the same block to two callers. Share a tree \
                     through a Sync* wrapper (which defers to the collector's locked \
                     freelists before carving anything), or give each thread its own \
                     NodeAlloc."
                );
                debug_assert!(
                    !self.occ_enabled(),
                    "alloc_raw popped per-tree freelist under OCC: per-tree freelists \
                     are single-writer only; OCC allocations must use collector freelist"
                );
                // SAFETY: head points to a valid FreeBlock previously freed to this class.
                let next = unsafe { (*head).next };
                self.freelists[class].store(next, Ordering::Relaxed);
                let raw = head.cast::<u8>();
                // SAFETY: zero out the reused memory before returning.
                unsafe { core::ptr::write_bytes(raw, 0, bytes) };
                return NonNull::new(raw).expect("non-null free block");
            }

            if bytes <= 256 {
                #[cfg(debug_assertions)]
                let _bookkeeping = self.enter_bookkeeping();
                // Pre-populate freelist from an intrusive 4KB slab page
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
                    (*slab_page).class = class;
                }
                self.slab_pages.store(slab_page, Ordering::Relaxed);

                let header_offset = SLAB_HEADER;
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
                let raw = unsafe { page_raw.add(header_offset) };
                // SAFETY: zero out the reused memory before returning.
                unsafe { core::ptr::write_bytes(raw, 0, bytes) };
                return NonNull::new(raw).expect("non-null free block");
            }
        }

        Self::alloc_system(bytes, align)
    }

    /// A zeroed block straight from the system allocator.
    #[inline(always)]
    fn alloc_system(bytes: usize, align: usize) -> NonNull<u8> {
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
    unsafe fn free_raw<const OCC: bool>(&self, ptr: NonNull<u8>, bytes: usize, align: usize) {
        #[cfg(feature = "std")]
        debug_assert!(
            OCC || self.deferred.get().is_none(),
            "plain free path (OCC=false) invoked on an allocator with deferred reclamation enabled"
        );
        let accounted_size = accounted_size(bytes, align);

        // As in `alloc_raw`: one test, and a deferred tree leaves through it.
        #[cfg(feature = "std")]
        if let Some(d) = self.deferred_if::<OCC>() {
            d.discharge(self, accounted_size);
            // Deferred mode: the structure no longer references `ptr`,
            // but pinned readers may — reclamation waits out the grace
            // period. The alignment travels with the pointer, because the
            // collector frees it later and elsewhere.
            d.collector.retire(ptr, bytes, align);
            return;
        }

        self.bytes_in_use
            .fetch_sub(accounted_size, Ordering::Relaxed);
        self.live_allocs.fetch_sub(1, Ordering::Relaxed);

        if let Some(class) = class_for(bytes, align) {
            #[cfg(debug_assertions)]
            let _bookkeeping = self.enter_bookkeeping();
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
        self.alloc_raw::<true>(bytes, RAW_ALIGN)
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
        unsafe { self.free_raw::<true>(ptr, bytes, RAW_ALIGN) };
    }

    /// [`Self::alloc_bytes`] for a tree with no collector attached, with the
    /// collector branch compiled out instead of tested on each call
    /// (AGENTS.md §2.1.5). Valid only while `occ_enabled()` is false, which
    /// `alloc_raw` checks in debug builds; the engine calls it only from the
    /// `OCC = false` paths that `by_mode!` selects on that same condition.
    #[must_use]
    #[inline(always)]
    pub(crate) fn alloc_bytes_plain(&self, bytes: usize) -> NonNull<u8> {
        self.alloc_raw::<false>(bytes, RAW_ALIGN)
    }

    /// [`Self::free_bytes`] for a tree with no collector attached: the block
    /// returns to this tree's freelists with no retirement branch.
    ///
    /// # Safety
    ///
    /// As [`Self::free_bytes`], and `occ_enabled()` must be false (checked in
    /// debug builds): on a shared tree a reader may still hold `ptr`, and only
    /// retirement waits it out.
    #[inline(always)]
    pub(crate) unsafe fn free_bytes_plain(&self, ptr: NonNull<u8>, bytes: usize) {
        // SAFETY: forwarded contract; `alloc_bytes` and `alloc_bytes_plain`
        // both allocate at RAW_ALIGN, so the layout matches.
        unsafe { self.free_raw::<false>(ptr, bytes, RAW_ALIGN) };
    }

    /// [`Self::alloc_bytes`] with the collector branch chosen at compile time;
    /// `OCC = false` is [`Self::alloc_bytes_plain`].
    #[must_use]
    #[inline(always)]
    pub(crate) fn alloc_bytes_dispatch<const OCC: bool>(&self, bytes: usize) -> NonNull<u8> {
        self.alloc_raw::<OCC>(bytes, RAW_ALIGN)
    }

    /// [`Self::free_bytes`] with the collector branch chosen at compile time.
    ///
    /// # Safety
    ///
    /// As [`Self::free_bytes`]; with `OCC = false`, as [`Self::free_bytes_plain`].
    #[inline(always)]
    pub(crate) unsafe fn free_bytes_dispatch<const OCC: bool>(
        &self,
        ptr: NonNull<u8>,
        bytes: usize,
    ) {
        // SAFETY: forwarded contract; every byte allocation uses RAW_ALIGN.
        unsafe { self.free_raw::<OCC>(ptr, bytes, RAW_ALIGN) };
    }

    /// Frees an allocation made by [`Self::alloc_bytes`] that was **never published** to
    /// the tree or exposed to any concurrent reader.
    ///
    /// Under concurrent OCC mode, this bypasses Epoch-Based Reclamation (EBR) retirement
    /// and immediately recycles the block into the collector's size-class freelist, avoiding
    /// garbage bin retention and epoch queue bloat during retry loops.
    ///
    /// # Concurrency & AGENTS.md §2.6 Architectural Contract
    /// AGENTS.md §2.6 categorically forbids thread-local *retire* buffers for published memory
    /// because buffering garbage from epoch `e` while another writer advances to `e+1` creates
    /// S4 store-buffer pairing violations and thread-exit leaks.
    /// In contrast, `free_bytes_unpublished` operates exclusively on speculative, unshared
    /// scratch memory allocated by the current writer that aborted prior to publication. Because
    /// this memory was never reachable by any reader, it carries zero epoch-visibility hazard
    /// and is returned immediately to the allocator freelist.
    ///
    /// # Safety
    ///
    /// `ptr` must come from `alloc_bytes(bytes)` on this handle with [`RAW_ALIGN`], must
    /// NEVER have been published to any node/edge or visible to any reader, not yet freed,
    /// and nothing may use it afterwards.
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    #[inline(always)]
    pub(crate) unsafe fn free_bytes_unpublished(&self, ptr: NonNull<u8>, bytes: usize) {
        if let Some(d) = self.deferred_if::<true>() {
            d.discharge(self, accounted_size(bytes, RAW_ALIGN));

            // SAFETY: ptr was never published and matches bytes/RAW_ALIGN contract.
            unsafe { d.collector.recycle_unpublished(ptr, bytes, RAW_ALIGN) };
            return;
        }

        // Without deferred reclamation, unpublished frees are identical to ordinary frees.
        // SAFETY: per this function's contract, layout matches original allocation.
        unsafe { self.free_raw::<true>(ptr, bytes, RAW_ALIGN) };
    }

    /// Frees an allocation made by [`Self::alloc_node`] or [`Self::alloc_node_zeroed`] that was
    /// **never published** to the tree or exposed to any concurrent reader.
    ///
    /// Under concurrent OCC mode, this bypasses Epoch-Based Reclamation (EBR) retirement
    /// and immediately recycles the block into the collector's size-class freelist, avoiding
    /// garbage bin retention and epoch queue bloat during retry loops.
    ///
    /// # Concurrency & AGENTS.md §2.6 Architectural Contract
    /// AGENTS.md §2.6 categorically forbids thread-local *retire* buffers for published memory.
    /// In contrast, `free_node_unpublished` operates exclusively on speculative, unshared
    /// node memory that was never reachable by any concurrent reader. It carries zero epoch
    /// or store-buffer hazard and is returned directly to the allocator freelist.
    ///
    /// # Safety
    ///
    /// `ptr` must come from `alloc_node::<T>` or `alloc_node_zeroed::<T>` on this handle, must
    /// NEVER have been published to any node/edge or visible to any reader, not yet freed,
    /// and nothing may use it afterwards.
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    #[inline(always)]
    pub(crate) unsafe fn free_node_unpublished<T>(&self, ptr: NonNull<T>) {
        // SAFETY: ptr holds a live T per this function's contract.
        unsafe { ptr.drop_in_place() };
        let bytes = core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        if let Some(d) = self.deferred_if::<true>() {
            d.discharge(self, accounted_size(bytes, align));

            // SAFETY: ptr was never published and matches bytes/align contract.
            unsafe {
                d.collector
                    .recycle_unpublished(ptr.cast::<u8>(), bytes, align)
            };
            return;
        }

        // Without deferred reclamation, unpublished frees are identical to ordinary frees.
        // SAFETY: per this function's contract, layout matches original allocation.
        unsafe { self.free_raw::<true>(ptr.cast::<u8>(), bytes, align) };
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

    /// Debug-only bracket bookkeeping: `v` — a node version field, or the
    /// tree cover sentinel — is now open on this thread's mutation stack.
    #[cfg(debug_assertions)]
    pub(crate) fn bracket_enter(&self, v: *const u32) {
        debug_assert!(
            !bracket_stack::contains(v),
            "version bracket opened twice on one word: a nested `begin` \
             makes the word even while the outer write is still in flight"
        );
        bracket_stack::enter(v);
    }

    /// Debug-only bracket bookkeeping: the most recently opened bracket,
    /// which must be `v`, closes.
    #[cfg(debug_assertions)]
    pub(crate) fn bracket_leave(&self, v: *const u32) {
        bracket_stack::leave(v);
    }

    /// Test-only: opens an anonymous bracket (a sentinel address), for tests
    /// that need "some bracket is open" without a node.
    #[cfg(all(test, debug_assertions))]
    pub(crate) fn bracket_enter_any(&self) {
        bracket_stack::enter(core::ptr::without_provenance(usize::MAX));
    }

    /// Test-only twin of [`Self::bracket_enter_any`].
    #[cfg(all(test, debug_assertions))]
    pub(crate) fn bracket_leave_any(&self) {
        bracket_stack::leave(core::ptr::without_provenance(usize::MAX));
    }

    /// Asserts the Phase 7 coverage invariant at a mutation site: **every
    /// store to a node's interior happens with *that node's* version
    /// bracket open** (or, for a leaf, immediate or subarray payload, the
    /// bracket of the branch whose slot points at it), so a concurrent
    /// reader validating against that node's version cannot miss it.
    ///
    /// Address-checked (#568 PR 3): a bracket open on the wrong node would
    /// satisfy a depth counter and still leave a reader's validation blind,
    /// so the check is that `v` itself is on this thread's open stack.
    /// Checked only in debug builds, and only for concurrently shared trees
    /// — a single-threaded tree has no readers to protect.
    #[inline]
    pub(crate) fn assert_bracketed_by(&self, v: *const u32) {
        #[cfg(debug_assertions)]
        debug_assert!(
            !self.occ_enabled() || bracket_stack::contains(v),
            "node interior mutated outside its own version bracket ({v:p}): a \
             concurrent reader validating against that node could observe it \
             mid-write; open brackets on this thread: {:?}",
            bracket_stack::open()
        );
        let _ = v;
    }

    /// The weaker form: *some* bracket is open on this thread. Kept for the
    /// single-threaded bypass sites the concurrent engine does not take.
    #[inline]
    pub(crate) fn assert_bracketed(&self) {
        #[cfg(debug_assertions)]
        debug_assert!(
            !self.occ_enabled() || !bracket_stack::open().is_empty(),
            "node interior mutated outside any version bracket: a concurrent \
             reader could observe it mid-write"
        );
    }

    /// The tree-level version word bound by [`Self::bind_tree_word`]
    /// (#568 PR 3). Reached only on a tree whose root state the engine
    /// covers, which [`Self::cover_root`] refuses to set before the word is
    /// bound.
    #[cfg(feature = "std")]
    #[inline(always)]
    pub(crate) fn tree_version(&self) -> &crate::occ::SeqVersion {
        let p = self.tree_word.load(Ordering::Relaxed);
        assert!(
            !p.is_null(),
            "tree_version on a tree with no tree word bound"
        );
        // SAFETY: `bind_tree_word`'s contract — the word outlives every
        // operation on this allocator — and the null check above.
        unsafe { &*p }
    }

    /// Binds the tree-level version word (#568 PR 3). Called once by a
    /// `sync` wrapper after it is boxed, with the address of its own
    /// `Shared::version`; idempotent for the same word.
    ///
    /// # Safety
    ///
    /// `word` must stay valid, at that address, for as long as any
    /// operation can run on a tree allocated through this allocator. The
    /// wrappers guarantee it by owning the tree and the word in one heap
    /// block that neither leaves.
    #[cfg(feature = "std")]
    pub(crate) unsafe fn bind_tree_word(&self, word: *const crate::occ::SeqVersion) {
        let prev = self.tree_word.swap(word.cast_mut(), Ordering::Relaxed);
        assert!(
            prev.is_null() || core::ptr::eq(prev, word),
            "NodeAlloc already bound to a different tree word"
        );
    }

    /// Hands root-state coverage to the engine (#568 PR 3): the wrapper then
    /// runs mutations *without* the tree-level bracket and the engine opens
    /// it only around a `Root` variant change, a root-leaf mutation or a
    /// top-edge rewrite. Requires [`Self::defer_to`] and
    /// [`Self::bind_tree_word`] first.
    #[cfg(feature = "std")]
    pub(crate) fn cover_root(&self) {
        assert!(self.deferred.get().is_some(), "cover_root before defer_to");
        assert!(
            !self.tree_word.load(Ordering::Relaxed).is_null(),
            "cover_root before bind_tree_word"
        );
        self.root_cover.store(ROOT_COVER_ENGINE, Ordering::Relaxed);
    }

    /// A map or set wrapper takes (`true`) or returns (`false`) the tree word
    /// for one whole covered write (#1086). While it holds the word the
    /// engine's own tree bracket is a no-op ([`Self::engine_opens_tree_word`])
    /// and the engine walks keep the monomorph they run otherwise
    /// ([`Self::engine_covers_root`] stays true). Called only under the
    /// writer mutex with the optimistic writers quiesced.
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn hold_tree_word(&self, held: bool) {
        let (from, to) = if held {
            (ROOT_COVER_ENGINE, ROOT_COVER_HELD)
        } else {
            (ROOT_COVER_HELD, ROOT_COVER_ENGINE)
        };
        // The only writer of this byte after `cover_root` is the covered
        // writer, under the writer mutex, so a load and a store suffice; no
        // read-modify-write. A tree whose engine does not cover the root (no
        // `cover_root`: the wrapper already brackets whole operations) needs
        // no hand-over.
        let prev = self.root_cover.load(Ordering::Relaxed);
        if prev == from {
            self.root_cover.store(to, Ordering::Relaxed);
        } else {
            debug_assert_eq!(prev, ROOT_COVER_WRAPPER, "hold_tree_word out of order");
        }
    }

    /// Sentinel address for the tree cover on the debug bracket stack: the
    /// bound tree word, or — before a wrapper binds one (a tree being
    /// rebuilt through a pre-deferred allocator) — the collector's address,
    /// which nothing pushes, so any tree-cover assertion then fails loudly.
    #[cfg(all(debug_assertions, feature = "std"))]
    #[inline(always)]
    pub(crate) fn tree_cover_addr(&self) -> *const u32 {
        let p = self.tree_word.load(Ordering::Relaxed);
        if p.is_null() {
            return self.deferred.get().map_or(core::ptr::null(), |d| {
                Arc::as_ptr(&d.collector).cast::<u32>()
            });
        }
        p.cast_const().cast::<u32>()
    }

    /// Whether the engine brackets root-state writes on this tree (see the
    /// field): what `by_mode!` selects the brief-bracket monomorph on. False
    /// until [`Self::cover_root`]; still true while a wrapper holds the word.
    #[cfg(feature = "std")]
    #[inline(always)]
    pub(crate) fn engine_covers_root(&self) -> bool {
        self.root_cover.load(Ordering::Relaxed) != ROOT_COVER_WRAPPER
    }

    /// Whether the engine's own tree bracket opens the word now: the engine
    /// covers the root and no wrapper holds the word
    /// ([`Self::hold_tree_word`]).
    #[cfg(feature = "std")]
    #[inline(always)]
    pub(crate) fn engine_opens_tree_word(&self) -> bool {
        self.root_cover.load(Ordering::Relaxed) == ROOT_COVER_ENGINE
    }

    /// `no_std` twin: nothing is shared, nothing covers a root.
    #[cfg(not(feature = "std"))]
    #[inline(always)]
    pub(crate) fn engine_covers_root(&self) -> bool {
        false
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
        for head in &self.freelists {
            assert!(
                head.load(Ordering::Relaxed).is_null(),
                "defer_to on an allocator with active freelists: per-tree freelist pop \
                 is unsynchronized across concurrent writers"
            );
        }
        // The shard array exists only for a tree that actually became
        // concurrent: allocated once, cold, in the same cell as the collector,
        // so no call can see a deferred allocator without its shards.
        let stored = self
            .deferred
            .get_or_init(|| Deferred::new(Arc::clone(&collector)));
        assert!(
            Arc::ptr_eq(&stored.collector, &collector),
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
        let ptr = self
            .alloc_raw::<true>(size_of::<T>(), align_of::<T>())
            .cast::<T>();
        // SAFETY: freshly allocated, correctly sized, and allocated at
        // exactly `align_of::<T>()`.
        unsafe { ptr.write(init) };
        ptr
    }

    /// [`Self::alloc_node`] for a tree with no collector attached; see
    /// [`Self::alloc_bytes_plain`] for when that holds.
    #[inline(always)]
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn alloc_node_plain<T>(&self, init: T) -> NonNull<T> {
        debug_assert!(align_of::<T>() <= CACHE_LINE);
        let ptr = self
            .alloc_raw::<false>(size_of::<T>(), align_of::<T>())
            .cast::<T>();
        // SAFETY: freshly allocated, correctly sized, and allocated at
        // exactly `align_of::<T>()`.
        unsafe { ptr.write(init) };
        ptr
    }

    /// [`Self::alloc_node`] with the collector branch chosen at compile time;
    /// `OCC = false` is [`Self::alloc_node_plain`].
    #[must_use]
    #[inline(always)]
    #[allow(dead_code)]
    pub(crate) fn alloc_node_dispatch<const OCC: bool, T>(&self, init: T) -> NonNull<T> {
        debug_assert!(align_of::<T>() <= CACHE_LINE);
        let ptr = self
            .alloc_raw::<OCC>(size_of::<T>(), align_of::<T>())
            .cast::<T>();
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
        self.alloc_raw::<true>(size_of::<T>(), align_of::<T>())
            .cast::<T>()
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
        unsafe { self.free_raw::<true>(ptr.cast::<u8>(), size_of::<T>(), align_of::<T>()) };
    }

    /// [`Self::alloc_node_zeroed`] for a tree with no collector attached; see
    /// [`Self::alloc_bytes_plain`] for when that holds.
    #[inline(always)]
    #[must_use]
    pub(crate) fn alloc_node_zeroed_plain<T>(&self) -> NonNull<T> {
        debug_assert!(align_of::<T>() <= CACHE_LINE);
        self.alloc_raw::<false>(size_of::<T>(), align_of::<T>())
            .cast::<T>()
    }

    /// [`Self::free_node`] for a tree with no collector attached.
    ///
    /// # Safety
    ///
    /// As [`Self::free_node`], and `occ_enabled()` must be false; see
    /// [`Self::free_bytes_plain`].
    #[inline(always)]
    pub(crate) unsafe fn free_node_plain<T>(&self, ptr: NonNull<T>) {
        // SAFETY: caller asserts `ptr` is a valid node of type `T`.
        unsafe { ptr.drop_in_place() };
        // SAFETY: allocated at `size_of::<T>()` and `align_of::<T>()`.
        unsafe { self.free_raw::<false>(ptr.cast::<u8>(), size_of::<T>(), align_of::<T>()) };
    }

    /// [`Self::alloc_node_zeroed`] with the collector branch chosen at compile
    /// time; `OCC = false` is [`Self::alloc_node_zeroed_plain`].
    #[must_use]
    #[inline(always)]
    pub(crate) fn alloc_node_zeroed_dispatch<const OCC: bool, T>(&self) -> NonNull<T> {
        debug_assert!(align_of::<T>() <= CACHE_LINE);
        self.alloc_raw::<OCC>(size_of::<T>(), align_of::<T>())
            .cast::<T>()
    }

    /// [`Self::free_node`] with the collector branch chosen at compile time.
    ///
    /// # Safety
    ///
    /// As [`Self::free_node`]; with `OCC = false`, as [`Self::free_node_plain`].
    #[inline(always)]
    pub(crate) unsafe fn free_node_dispatch<const OCC: bool, T>(&self, ptr: NonNull<T>) {
        // SAFETY: caller asserts `ptr` is a valid node of type `T`.
        unsafe { ptr.drop_in_place() };
        // SAFETY: allocated at `size_of::<T>()` and `align_of::<T>()`.
        unsafe { self.free_raw::<OCC>(ptr.cast::<u8>(), size_of::<T>(), align_of::<T>()) };
    }
}

/// The per-thread stack of open version brackets, debug builds only.
///
/// Per thread rather than per tree: with more than one writer a tree-wide
/// counter would let thread A's bracket satisfy thread B's assert (#568),
/// and a counter cannot say *which* node is covered at all. Addresses only —
/// nothing is dereferenced.
#[cfg(debug_assertions)]
pub(crate) mod bracket_stack {
    #[cfg(feature = "std")]
    std::thread_local! {
        static OPEN: core::cell::RefCell<Vec<usize>> = const { core::cell::RefCell::new(Vec::new()) };
    }

    #[cfg(feature = "std")]
    pub(crate) fn enter(v: *const u32) {
        OPEN.with(|o| o.borrow_mut().push(v as usize));
    }

    #[cfg(feature = "std")]
    pub(crate) fn leave(v: *const u32) {
        OPEN.with(|o| {
            let top = o.borrow_mut().pop();
            debug_assert_eq!(
                top,
                Some(v as usize),
                "version brackets closed out of order"
            );
        });
    }

    #[cfg(feature = "std")]
    pub(crate) fn contains(v: *const u32) -> bool {
        OPEN.with(|o| o.borrow().contains(&(v as usize)))
    }

    #[cfg(feature = "std")]
    pub(crate) fn open() -> Vec<usize> {
        OPEN.with(|o| o.borrow().clone())
    }

    // Without `std` there is no thread-local storage and no shared tree
    // (`occ_enabled` is always false), so the stack is a no-op that reports
    // "nothing open".
    #[cfg(not(feature = "std"))]
    pub(crate) fn enter(_v: *const u32) {}
    #[cfg(not(feature = "std"))]
    pub(crate) fn leave(_v: *const u32) {}
    #[cfg(not(feature = "std"))]
    pub(crate) fn contains(_v: *const u32) -> bool {
        false
    }
    #[cfg(not(feature = "std"))]
    pub(crate) fn open() -> [usize; 0] {
        []
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

    /// Each stripe's allocations are counted in that stripe's shard, a free
    /// on another stripe is netted in the freeing stripe's shard, and the
    /// totals still balance. A shard index that ignored the stripe would put
    /// every count in one shard.
    #[test]
    #[cfg(all(feature = "std", not(feature = "ablation-unsharded-alloc")))]
    fn sharded_alloc_counts_per_stripe() {
        // The shards exist only for a tree that became concurrent, so defer
        // first — a plain tree takes the inline arm and has no stripes.
        let a = NodeAlloc::new();
        a.defer_to(Arc::new(Collector::new()));
        let sh = &a
            .deferred
            .get()
            .expect("defer_to publishes the shards")
            .shards;
        let stripes = [0, 5, NUM_ALLOC_SHARDS - 1];
        let mut ptrs = Vec::new();
        for &s in &stripes {
            crate::occ::set_writer_slot(s);
            ptrs.push(a.alloc_bytes(32));
            assert_eq!(sh[s].live_allocs.load(Ordering::Relaxed), 1);
            assert_eq!(sh[s].total_allocs.load(Ordering::Relaxed), 1);
        }
        let per = (a.bytes_in_use() / stripes.len()) as isize;
        for &s in &stripes {
            assert_eq!(sh[s].bytes_in_use.load(Ordering::Relaxed), per);
        }

        crate::occ::set_writer_slot(1);
        for p in ptrs {
            // SAFETY: `p` came from `alloc_bytes(32)` on this handle.
            unsafe { a.free_bytes(p, 32) };
        }
        let n = stripes.len() as isize;
        assert_eq!(sh[1].live_allocs.load(Ordering::Relaxed), -n);
        assert_eq!(a.live_allocs(), 0);
        assert_eq!(a.bytes_in_use(), 0);
        assert_eq!(a.total_allocs(), stripes.len());
    }

    /// The shards ride in the collector's cell, boxed, so an allocator that
    /// never becomes concurrent is no larger than it was before they existed
    /// and no hot field moved to make room for them (AGENTS.md §2.1
    /// invariant 5). A second `OnceLock` beside `deferred` fails the second
    /// assertion; shards carried inline in the cell fail the first.
    #[test]
    #[cfg(feature = "std")]
    fn deferred_cell_adds_no_word_to_the_allocator() {
        use core::mem::size_of;
        assert_eq!(
            size_of::<OnceLock<DeferredCell>>(),
            size_of::<OnceLock<Arc<Collector>>>()
        );
        #[cfg(target_pointer_width = "64")]
        {
            let classes = NUM_CLASSES * size_of::<usize>();
            // bytes_in_use, live_allocs, total_allocs, slab_pages, tree_word,
            // one word shared by root_cover and the debug-only
            // bookkeeping flag, and the two-word cell.
            assert_eq!(size_of::<NodeAlloc>(), 8 * 8 + classes);
        }
    }

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
        // SAFETY: `version` is a live local for the whole bracket.
        unsafe {
            crate::occ::version_begin_if_ptr::<true>(&a, &raw mut version);
            a.assert_bracketed();
            a.assert_bracketed_by(&raw const version);
            crate::occ::version_end_if_ptr::<true>(&a, &raw mut version);
        }
    }

    /// The address check is the point (#568 PR 3): a bracket open on
    /// another node must not satisfy a store into this one.
    #[test]
    #[cfg(feature = "std")]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "outside its own version bracket")]
    fn negative_control_wrong_node_bracket_must_fire() {
        let a = NodeAlloc::new();
        a.defer_to(Arc::new(crate::occ::Collector::new()));
        let mut other = 0u32;
        let this = 0u32;
        // SAFETY: `other` is a live local for the whole bracket.
        unsafe {
            crate::occ::version_begin_if_ptr::<true>(&a, &raw mut other);
            a.assert_bracketed_by(&raw const this);
            crate::occ::version_end_if_ptr::<true>(&a, &raw mut other);
        }
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

    /// `raw_class_fit` returns the smallest raw class holding each size, and
    /// never a size whose accounted bytes exceed the request's own; every
    /// suffix-sized request up to 250 bytes lands in a class.
    #[test]
    fn raw_class_fit_is_the_smallest_holding_class() {
        for bytes in 1..RAW_FIT_TABLE.len() + 8 {
            let fit = raw_class_fit(bytes);
            assert!(fit >= bytes, "{bytes} -> {fit}");
            assert_eq!(
                accounted_size(fit, RAW_ALIGN),
                accounted_size(bytes, RAW_ALIGN),
                "{bytes} -> {fit} changes the accounted size"
            );
            if fit != bytes {
                assert!(
                    class_for_raw(fit).is_some(),
                    "{bytes} -> {fit} has no class"
                );
                assert!(
                    (bytes..fit).all(|b| class_for_raw(b).is_none()),
                    "{bytes} -> {fit} skips a smaller class"
                );
            }
            if bytes > 375 {
                assert_eq!(fit, bytes);
            }
            if (16..=250).contains(&bytes) {
                assert!(
                    class_for_raw(fit).is_some(),
                    "{bytes} is not fitted to a class"
                );
            }
        }
    }

    /// `release_free` frees exactly the slab pages with no live block and
    /// every free system-class block, keeps a page that still holds a live
    /// block (and its free blocks on the freelist), and `bytes_held` falls
    /// by exactly the bytes it reports.
    #[test]
    fn release_free_returns_free_pages_and_system_blocks() {
        let class = class_for_raw(64).expect("64 is a raw class");
        let per_page = slab_blocks(class);
        let mut a = NodeAlloc::new();
        // Three pages: two full, one holding the last block.
        let n = 2 * per_page + 1;
        let blocks: core_alloc::vec::Vec<_> = (0..n).map(|_| a.alloc_bytes(64)).collect();
        let big = a.alloc_bytes(300);
        assert!(class_for_raw(300).is_some_and(|c| !is_slab_class(c)));
        // SAFETY: `big` came from `alloc_bytes(300)` and is not used again.
        unsafe { a.free_bytes(big, 300) };
        for &b in &blocks[..n - 1] {
            // SAFETY: each block came from `alloc_bytes(64)` and is freed once.
            unsafe { a.free_bytes(b, 64) };
        }
        assert_eq!(a.bytes_in_use(), 64);
        assert_eq!(
            a.bytes_held(),
            3 * SLAB_PAGE_SIZE + accounted_size(300, RAW_ALIGN)
        );

        let released = a.release_free();
        assert_eq!(
            released,
            2 * SLAB_PAGE_SIZE + accounted_size(300, RAW_ALIGN)
        );
        assert_eq!(
            a.bytes_held(),
            SLAB_PAGE_SIZE,
            "the page with a live block stays"
        );
        assert_eq!(
            a.release_free(),
            0,
            "a second call finds nothing to release"
        );

        // The kept page's free blocks are still served, then everything goes.
        let again = a.alloc_bytes(64);
        assert_eq!(a.bytes_held(), SLAB_PAGE_SIZE, "reuse, not a new page");
        // SAFETY: both blocks came from `alloc_bytes(64)` and are freed once.
        unsafe {
            a.free_bytes(again, 64);
            a.free_bytes(blocks[n - 1], 64);
        }
        assert_eq!(a.release_free(), SLAB_PAGE_SIZE);
        assert_eq!(a.bytes_held(), 0);
        assert_eq!(a.bytes_in_use(), 0);
    }

    #[cfg(feature = "std")]
    #[test]
    #[should_panic(expected = "defer_to on an allocator with active freelists")]
    fn defer_to_rejects_non_empty_freelist() {
        #[repr(C, align(64))]
        struct BigNode([u8; 2048]);

        let a = NodeAlloc::new();
        // Allocate a 2048-byte node (class 5) which is > 256 bytes so it does
        // not carve from slab pages (leaving slab_pages null).
        let ptr = a.alloc_node(BigNode([0; 2048]));
        // Free it before deferring: it populates a.freelists[5].
        // SAFETY: ptr points to a valid allocation returned by alloc_node.
        unsafe { a.free_node(ptr) };
        let collector = std::sync::Arc::new(crate::occ::Collector::new());
        a.defer_to(collector);
    }

    #[test]
    #[cfg(all(debug_assertions, feature = "std"))]
    #[should_panic = "plain free path (OCC=false) invoked on an allocator with deferred reclamation enabled"]
    fn plain_free_rejects_deferred_allocator() {
        struct RawBlockGuard {
            ptr: *mut u8,
            layout: std::alloc::Layout,
        }
        impl Drop for RawBlockGuard {
            fn drop(&mut self) {
                // SAFETY: self.ptr was allocated with self.layout.
                unsafe { std::alloc::dealloc(self.ptr, self.layout) };
            }
        }

        let a = NodeAlloc::new();
        let collector = std::sync::Arc::new(crate::occ::Collector::new());
        a.defer_to(collector);
        let layout = std::alloc::Layout::from_size_align(64, RAW_ALIGN).unwrap();
        // SAFETY: layout has non-zero size and valid alignment.
        let raw = unsafe { std::alloc::alloc(layout) };
        let guard = RawBlockGuard { ptr: raw, layout };
        let ptr = NonNull::new(raw).unwrap();
        // SAFETY: ptr is a valid 64-byte allocation at RAW_ALIGN; triggers invariant debug_assert.
        unsafe { a.free_bytes_plain(ptr, 64) };
        core::mem::forget(guard);
    }

    #[test]
    #[cfg(all(debug_assertions, feature = "std"))]
    #[should_panic = "plain allocator path (OCC=false) invoked on an allocator with deferred reclamation enabled"]
    fn plain_alloc_rejects_deferred_allocator() {
        let a = NodeAlloc::new();
        let collector = std::sync::Arc::new(crate::occ::Collector::new());
        a.defer_to(collector);
        let _ = a.alloc_bytes_plain(64);
    }

    /// The guard itself: a second holder is rejected while the first lives, and
    /// the region is free again once it drops. Single-threaded, so it decides
    /// deterministically rather than on an interleaving.
    #[test]
    #[cfg(debug_assertions)]
    fn bookkeeping_guard_admits_one_holder_at_a_time() {
        let a = NodeAlloc::new();
        {
            let _first = a.enter_bookkeeping();
            assert!(
                a.bookkeeping_busy.load(Ordering::Relaxed),
                "a held guard must mark the region busy"
            );
        }
        assert!(
            !a.bookkeeping_busy.load(Ordering::Relaxed),
            "dropping the guard must release the region"
        );
        // Free again, so the guard is re-entrant across calls, not once-only.
        let _second = a.enter_bookkeeping();
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic = "two threads are inside one NodeAlloc's per-tree freelists"]
    fn bookkeeping_guard_rejects_a_second_holder() {
        let a = NodeAlloc::new();
        let _first = a.enter_bookkeeping();
        let _second = a.enter_bookkeeping();
    }

    /// Pins the call sites, not just the helper: the regions that mutate the
    /// single-writer freelists and slab-page list each claim the guard.
    /// Deleting a claim leaves both tests above green while restoring the
    /// silent lost update, so this counts them in the source.
    ///
    /// Both counts are the invariant, not the numbers. The claims are the
    /// freelist pop, the slab-page carve and the freelist push; the mutations
    /// are their four stores. A new mutating region changes the store count,
    /// which fails this test until someone states where its guard is — the
    /// census cannot tell on its own whether a *new* store sits inside a
    /// guarded region, so that judgement is the reviewer's, and this test is
    /// what forces it to be made.
    ///
    /// Scanned line by line, so a CRLF checkout counts the same as an LF one,
    /// and the needles are assembled with `concat!` so this test cannot match
    /// its own source text.
    #[test]
    fn structural_every_freelist_mutation_claims_the_bookkeeping_guard() {
        let claim = concat!("enter_", "bookkeeping()");
        let freelist_store = concat!("self.freelists[", "class].store(");
        let slab_store = concat!("self.slab_pages", ".store(");
        let body: Vec<&str> = include_str!("alloc.rs")
            .lines()
            .take_while(|line| *line != "mod tests {")
            .collect();
        let claims = body.iter().filter(|line| line.contains(claim)).count();
        let stores = body
            .iter()
            .filter(|line| line.contains(freelist_store) || line.contains(slab_store))
            .count();
        assert_eq!(
            claims, 3,
            "expected the freelist pop, the slab-page carve and the freelist push to claim \
             the guard; found {claims} claims in alloc.rs above its test module"
        );
        assert_eq!(
            stores, 4,
            "expected 4 stores to the single-writer freelists and slab-page list (the pop, \
             the slab-page push, the carve's per-block push, and the free push); found \
             {stores}. A new one must sit inside a region that claims the bookkeeping guard"
        );
    }
}
