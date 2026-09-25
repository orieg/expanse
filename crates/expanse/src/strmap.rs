//! `ExpanseStrMap`: a sorted map from C-style byte strings to `u64`
//! values (compat: JudySL).
//!
//! Structure (clean-room, designed against the documented JudySL
//! *semantics* only): a meta-trie of word-map nodes with cross-chunk
//! tail collapse ([Issue #84 Item 1](https://github.com/orieg/expanse/issues/84)).
//! Each branch node is a word-map engine core (`MapCore`, the engine
//! behind [`crate::map::ExpanseMap`]) keyed by the string's next
//! **8-byte chunk, packed big-endian**, so the word maps' numeric order
//! *is* byte-lexicographic order and all ordered navigation falls out of
//! the word engine. All sub-tries allocate through the string map's
//! **single shared [`NodeAlloc`]** (issue #363 Step A), which keeps a
//! trie node at the size of one map root instead of ~700 bytes of
//! embedded allocator state and makes descent chains cache-resident.
//!
//! Keys are NUL-free byte strings (the C surface hands over
//! NUL-terminated strings, so this is the natural domain). A chunk that
//! contains the string's terminating NUL — always the case for the last
//! chunk, including the all-zero chunk of a string whose length is a
//! multiple of 8 — is a **terminal** entry holding the user value.
//!
//! For non-terminal chunks (8 non-NUL bytes), continuation entries use
//! pointer tagging:
//! - Tag `0`: Pointer to child `StrNode` branch.
//! - Tag `1`: Pointer to `StrSuffix` leaf, which stores the remaining
//!   NUL-free key bytes and the user value in a single allocation.

use crate::alloc::NodeAlloc;
use crate::cursor::RawCursor;
use crate::map::MapCore;
#[cfg(feature = "std")]
use crate::occ::Collector;
use core::alloc::Layout;
use core::ptr::NonNull;
#[cfg(feature = "std")]
use core::sync::atomic::{AtomicU32, Ordering};
use core_alloc::boxed::Box;
#[cfg(feature = "std")]
use core_alloc::sync::Arc;
use core_alloc::vec;
use core_alloc::vec::Vec;
#[cfg(feature = "std")]
use std::sync::OnceLock;

#[cfg(feature = "std")]
type DeferHandle<'a> = Option<&'a Arc<Collector>>;
#[cfg(not(feature = "std"))]
type DeferHandle<'a> = Option<&'a ()>;

const CHUNK: usize = 8;
const TAG_SUFFIX: u64 = 1;

/// Leaf suffix header: the value, then the length of the suffix bytes that
/// follow it **inside the same allocation**.
///
/// One allocation, not two. The previous shape was
/// `{ suffix: Box<[u8]>, value: u64 }`, which cost a 24-byte shell plus a
/// separate byte buffer for every key that does not resolve inside a terminal
/// 8-byte chunk — 204,791 allocations for 100,000 `short` keys against HOT's
/// 4,566 (#723) — and made every string lookup chase a second dependent
/// pointer to reach the bytes it had to compare.
///
/// Invariants that the rest of this module depends on:
///
/// - **`value` is at offset 0, and `#[repr(C)]` keeps it there.** `ins_slot`
///   and `get_value_slot` hand out `&raw mut (*sfx).value` as the JudySL
///   `*mut Word`, valid until the next structural mutation
///   (`docs/COMPAT.md`).
/// - **`align_of` is 8**, so bit 0 of a suffix pointer stays free for
///   [`TAG_SUFFIX`].
/// - **The bytes live outside this header**, at offset [`SUFFIX_BYTES`]. They
///   are therefore *not* covered by the provenance of a `&StrSuffix` or
///   `&mut StrSuffix`, so a pointer to them must always be derived from the
///   raw allocation pointer — never from a reference to the header. That is
///   why nothing in this module forms a reference to a `StrSuffix`; every
///   access is a raw field projection or [`suffix_bytes`].
/// - **Header and bytes are write-once after publication**; only `value`
///   mutates in place, which is what lets a concurrent reader load them
///   under a version bracket ([`ExpanseStrMap::get_validated`]).
#[repr(C)]
struct StrSuffix {
    value: u64,
    len: usize,
}

/// Byte offset of the inline suffix bytes within a `StrSuffix` allocation.
const SUFFIX_BYTES: usize = size_of::<StrSuffix>();

// The two layout invariants the rest of this module reads off the type,
// checked at compile time rather than trusted (AGENTS.md §6.5). Neither
// would break the build if it silently stopped holding: reordering the
// fields still compiles, and so does widening `TAG_SUFFIX`.
const _: () = {
    assert!(
        core::mem::offset_of!(StrSuffix, value) == 0,
        "`value` must stay at offset 0: `ins_slot`/`get_value_slot` hand out \
         `&raw mut (*sfx).value` as the JudySL `*mut Word` (docs/COMPAT.md)"
    );
    assert!(
        align_of::<StrSuffix>() >= 2,
        "a suffix leaf must be at least 2-byte aligned so bit 0 of its \
         pointer stays free for TAG_SUFFIX"
    );
};

/// The layout a suffix leaf holding `len` bytes was allocated with. Every
/// `alloc`, `dealloc` and `Collector::retire` for a suffix goes through this
/// one function: a `dealloc` layout mismatch is UB, not a leak.
#[inline]
fn suffix_layout(len: usize) -> Layout {
    Layout::from_size_align(SUFFIX_BYTES + len, align_of::<StrSuffix>())
        .expect("suffix layout: size is header + key remainder, align is 8")
}

/// Where suffix leaves are allocated. With `packed-suffix` they come from the
/// tree's `NodeAlloc` size classes (slab-carved up to 256 bytes), so a
/// suffix costs its size rounded to 16 bytes and no system-allocator chunk
/// header; without it this is a zero-sized marker and suffixes are separate
/// global allocations, as before the feature existed.
#[derive(Clone, Copy)]
struct SuffixArena<'a> {
    #[cfg(feature = "packed-suffix")]
    alloc: &'a NodeAlloc,
    #[cfg(not(feature = "packed-suffix"))]
    _alloc: core::marker::PhantomData<&'a NodeAlloc>,
}

impl<'a> SuffixArena<'a> {
    #[inline(always)]
    fn of(alloc: &'a NodeAlloc) -> Self {
        #[cfg(feature = "packed-suffix")]
        {
            Self { alloc }
        }
        #[cfg(not(feature = "packed-suffix"))]
        {
            let _ = alloc;
            Self {
                _alloc: core::marker::PhantomData,
            }
        }
    }
}

/// The `NodeAlloc` request for a packed suffix with this `suffix_layout`:
/// header plus bytes, fitted to a size class so it never becomes its own
/// system allocation. Every alloc and free of a packed suffix goes through
/// this, so the two always agree.
#[cfg(feature = "packed-suffix")]
#[inline]
fn packed_suffix_bytes(layout: Layout) -> usize {
    crate::alloc::raw_class_fit(layout.size())
}

/// Whether suffix bytes are already inside `NodeAlloc::bytes_in_use`.
const SUFFIX_IN_NODE_ALLOC: bool = cfg!(feature = "packed-suffix");

/// Allocates a suffix leaf holding `bytes` and `value` in one allocation.
fn new_suffix(bytes: &[u8], value: u64, arena: SuffixArena<'_>) -> *mut StrSuffix {
    let layout = suffix_layout(bytes.len());
    #[cfg(feature = "packed-suffix")]
    let raw = arena
        .alloc
        .alloc_bytes(packed_suffix_bytes(layout))
        .as_ptr();
    #[cfg(not(feature = "packed-suffix"))]
    let raw = {
        let _ = arena;
        // SAFETY: `layout` has non-zero size — the header alone is 16 bytes.
        let raw = unsafe { core_alloc::alloc::alloc(layout) };
        if raw.is_null() {
            core_alloc::alloc::handle_alloc_error(layout);
        }
        raw
    };
    let ptr = raw.cast::<StrSuffix>();
    // SAFETY: `raw` is a fresh allocation of `SUFFIX_BYTES + bytes.len()` at
    // `align_of::<StrSuffix>()`, so the header write is in bounds and aligned
    // and the byte copy lands in the tail this layout reserved for it. The
    // destination is freshly allocated, so it cannot overlap `bytes`.
    unsafe {
        ptr.write(StrSuffix {
            value,
            len: bytes.len(),
        });
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), raw.add(SUFFIX_BYTES), bytes.len());
    }
    ptr
}

/// The inline suffix bytes of a live leaf.
///
/// # Safety
///
/// `ptr` must be a live allocation from [`new_suffix`] — reached through
/// [`unpack_suffix`] or held directly — and **must not** have been derived
/// from a `&StrSuffix` or `&mut StrSuffix`: the bytes lie past the header, so
/// a reference's provenance does not reach them. The returned slice borrows
/// for as long as the caller chooses; it must not outlive the leaf, and in
/// particular must not be live across [`dispose_suffix`].
#[inline(always)]
unsafe fn suffix_bytes<'a>(ptr: *const StrSuffix) -> &'a [u8] {
    // SAFETY: the caller guarantees a live `new_suffix` allocation with raw
    // provenance over the whole block. `len` is write-once and was set to the
    // number of bytes written at `SUFFIX_BYTES`, so the slice is in bounds,
    // initialized, and — the bytes being write-once — not concurrently
    // mutated.
    unsafe { core::slice::from_raw_parts(ptr.cast::<u8>().add(SUFFIX_BYTES), (*ptr).len) }
}

#[inline(always)]
fn is_suffix_ptr(v: u64) -> bool {
    (v & TAG_SUFFIX) != 0
}

#[inline(always)]
fn unpack_suffix(v: u64) -> *mut StrSuffix {
    (v & !TAG_SUFFIX) as *mut StrSuffix
}

#[inline(always)]
fn pack_suffix(p: *mut StrSuffix) -> u64 {
    (p as u64) | TAG_SUFFIX
}

#[inline(always)]
fn unpack_child(v: u64) -> *mut StrNode {
    debug_assert_eq!(v & TAG_SUFFIX, 0);
    v as *mut StrNode
}

#[inline(always)]
fn pack_child(p: *mut StrNode) -> u64 {
    debug_assert_eq!((p as u64) & TAG_SUFFIX, 0);
    p as u64
}

/// One trie level: a word-map core over the next 8-byte chunk, headed by
/// the **cover word** for that core's root state.
///
/// Deliberately just the engine core plus that one word (issue #363 Step
/// A): the backing allocator is the string map's **single shared
/// [`NodeAlloc`]**, passed in per call, and there is no per-node
/// insert-path cache — which is what shrinks a node from ~700 bytes
/// (embedded allocator + path cache) to the root word pair, so a descent
/// chain stays cache-resident. `StrNode` has no `Drop`: teardown must
/// route through [`dispose_node`]/[`dispose_tree`] with the shared
/// allocator.
///
/// # The cover word (Refs #929)
///
/// `cover` is a per-node OCC version word of **exactly the kind every
/// branch header already carries** (`node::BranchHeader::version` and its
/// siblings): a plain `u32`, even when stable and odd while one frame
/// stores into the node, addressed through [`crate::occ::version_cell`]
/// and bracketed by the same [`crate::occ::version_begin`] /
/// [`crate::occ::version_end`] pair. It exists because the sub-map root
/// state inside this node — the `Root` variant, a root leaf, the top edge
/// — has no word of its own: `MapCore`'s root state is covered by the
/// *tree-level* word, which the engine reaches through
/// `NodeAlloc::bind_tree_word`, and `ExpanseStrMap` shares **one**
/// `NodeAlloc` across every sub-trie. One allocator is one bound word, so
/// without this field every `StrNode` in the meta-trie would contend on a
/// single tree word (`docs/benchmarks/concurrency/METHODOLOGY.md` §17.2.1).
///
/// It heads the struct, at offset 0 under `#[repr(C)]`, for the same
/// reason the tree word heads `sync::Shared`: a reader or writer that
/// holds only the `*mut StrNode` it decoded from a parent's tagged
/// continuation entry reaches the word at a fixed zero offset, with no
/// field arithmetic and on the line it is about to read the map root from.
///
/// **Who moves it.** On a shared map (one that `defer_to` switched to
/// deferred reclamation) every store to this node's root state is
/// bracketed by the word: the exclusive path (`sync::Shared::write` and
/// the fallbacks) opens a `version_begin` / `version_end` bracket around
/// each sub-map mutation, and an optimistic writer takes the word as a
/// lock (`version_try_lock_expect`) for a leaf-state sub-map, a suffix
/// value replace (T3), a split (T4), a suffix removal (T8) and a prune
/// (T9) — the transitions of `docs/benchmarks/concurrency/METHODOLOGY.md`
/// §17.2.1. A tree-state sub-map's interior is covered by the engine's own
/// per-node words, exactly as the map wrapper's is. Readers sample the
/// word before the hop and validate it after (§17.2.2), so a node that is
/// unlinked is marked obsolete first (property S3, [`dispose_node`]). The
/// unshared path never touches the word: a plain `ExpanseStrMap` leaves
/// every cover at 0, which `str_node_cover_words_are_untouched_unshared`
/// pins.
///
/// # The dirty flag (Refs #929)
///
/// An optimistic writer that mutates this node's sub-map *tree* through
/// the engine's OLC bodies leaves `MapCore::tree_pop` stale, exactly as
/// the map wrapper's optimistic writers leave `ExpanseMap`'s (they count
/// in `Shared::tree_pop` instead). The map wrapper re-syncs one field at
/// quiescence; the string map has one field per node, so each node
/// records that it is stale in `dirty`, and the exclusive path restores
/// the population from a census fold before it reads or changes it
/// ([`StrNode::resync_if_dirty`]). A dirty node is always in tree state:
/// leaf-state sub-maps are only ever mutated under the cover, which keeps
/// their population exact, and an optimistic writer never empties or
/// condenses a tree. The flag sits in the padding the cover word's
/// alignment already reserved, so the node's size does not move.
#[repr(C)]
struct StrNode {
    /// Per-node cover for this node's sub-map root state. See the type
    /// docs: even is stable, odd is a store in progress or a lock.
    cover: u32,
    /// Non-zero once an optimistic writer left `map.tree_pop` stale. See
    /// the type docs.
    dirty: u32,
    map: MapCore,
}

// The cover word's offset is load-bearing (see the type docs), and
// reordering the fields would still compile. Checked at compile time
// rather than trusted (AGENTS.md §6.5).
const _: () = {
    assert!(
        core::mem::offset_of!(StrNode, cover) == 0,
        "the cover word must head `StrNode`: the OCC protocol reaches it \
         from a bare `*mut StrNode` at a fixed zero offset"
    );
    assert!(
        core::mem::offset_of!(StrNode, dirty) == 4,
        "the dirty flag lives in the cover word's alignment padding"
    );
};

/// Disposes an unlinked suffix: freed immediately when not shared, retired
/// through the epoch collector when it is — a reader that validated the
/// tagged pointer at an earlier snapshot may still be reading the
/// (write-once) header and bytes under its pin.
///
/// One block, so one `retire` and no `Drop`: the header owns nothing, and
/// the bytes it describes are inside the same allocation. The two-allocation
/// shape this replaces had to move the byte buffer's owning `Box` out **by
/// value** so its provenance travelled to the collector, and to special-case
/// an empty suffix that owned no buffer at all; neither applies now.
fn dispose_suffix(ptr: *mut StrSuffix, defer: DeferHandle<'_>, arena: SuffixArena<'_>) {
    // SAFETY: the caller unlinked `ptr` and this is the last owner, so the
    // header is still live and `len` — write-once since publication — still
    // describes the allocation `new_suffix` made.
    let layout = suffix_layout(unsafe { (*ptr).len });
    // The tree's allocator decides between freeing and retiring by itself:
    // a shared tree's allocator is deferred, so this retires through the
    // same collector `defer` names.
    #[cfg(feature = "packed-suffix")]
    {
        let _ = defer;
        // SAFETY: `ptr` came from `alloc_bytes(packed_suffix_bytes(layout))` on this
        // allocator in `new_suffix`, is unlinked, and is freed once.
        unsafe {
            arena.alloc.free_bytes(
                NonNull::new(ptr.cast::<u8>()).expect("non-null suffix"),
                packed_suffix_bytes(layout),
            );
        }
    }
    // Every arm below is the global-allocator path, compiled out above.
    #[cfg(not(feature = "packed-suffix"))]
    let _ = arena;
    #[cfg(all(feature = "std", not(feature = "packed-suffix")))]
    match defer {
        // SAFETY: unlinked, last owner, and `layout` is by construction the
        // one this block was allocated with.
        None => unsafe { core_alloc::alloc::dealloc(ptr.cast::<u8>(), layout) },
        Some(c) => c.retire(
            NonNull::new(ptr.cast::<u8>()).expect("non-null suffix"),
            layout.size(),
            layout.align(),
        ),
    }
    #[cfg(all(not(feature = "std"), not(feature = "packed-suffix")))]
    {
        // Without `std` there is no epoch collector to defer to, so the
        // deferred arm degenerates to the immediate one.
        let _ = defer;
        // SAFETY: as above.
        unsafe { core_alloc::alloc::dealloc(ptr.cast::<u8>(), layout) };
    }
}

/// Disposes one unlinked node whose continuation children the caller
/// handles separately — pruning disposes an already-empty child, while
/// [`dispose_tree`] queues/disposes each node's children itself before
/// calling this (the map's continuation entries are plain words; nothing
/// here follows them).
///
/// The node's sub-map interior is cleared through the **shared**
/// allocator first (`StrNode` has no `Drop` since the allocator moved
/// out of the node, #363 Step A): when shared, those frees route through
/// the deferred `NodeAlloc` and retire to the collector, so pinned
/// readers keep their memory; the shell then frees (or retires) exactly
/// once with the layout it was allocated with.
fn dispose_node(ptr: *mut StrNode, alloc: &NodeAlloc, defer: DeferHandle<'_>) {
    // Property S3 (Refs #929): a shared node that ceases to be reachable is
    // marked obsolete before its interior changes, so a reader that
    // validated the entry pointing at it and was descheduled restarts on
    // its next sample instead of validating a word nobody bumps again. An
    // optimistic prune arrives with the word already odd — locked, then
    // marked through its lock — and is left alone.
    #[cfg(feature = "std")]
    if defer.is_some() {
        // SAFETY: `ptr` is live (unlinked, not yet retired) and this is
        // the only thread that stores to its cover word.
        let cell = unsafe { crate::occ::version_cell(&raw const (*ptr).cover) };
        if cell.load(Ordering::Relaxed).is_multiple_of(2) {
            crate::occ::version_obsolete(cell);
        }
    }
    // SAFETY: caller unlinked `ptr` and is the exclusive writer; the map
    // interior is cleared exactly once here and never touched again (in
    // deferred mode the shell memory stays mapped for pinned readers,
    // whose validation rejects whatever they read from it).
    unsafe { (*ptr).map.clear_pathless(alloc) };
    #[cfg(feature = "std")]
    match defer {
        None => {
            // SAFETY: last reference; the owning `Box` (created by
            // `Box::into_raw` at publication) frees the shell with its
            // original provenance and layout.
            drop(unsafe { Box::from_raw(ptr) });
        }
        Some(c) => {
            // Retire the shell raw — no `Drop` to run: the map interior
            // was cleared above, and any continuation words it held are
            // the caller's to dispose.
            c.retire(
                NonNull::new(ptr.cast::<u8>()).expect("non-null node"),
                size_of::<StrNode>(),
                align_of::<StrNode>(),
            );
        }
    }
    #[cfg(not(feature = "std"))]
    {
        // Without `std` there is no epoch collector to defer to, so the
        // deferred arm degenerates to the immediate one.
        let _ = defer;
        // SAFETY: caller unlinked `ptr`; this is the last reference.
        drop(unsafe { Box::from_raw(ptr) });
    }
}

/// Disposes a whole unlinked subtree. Iterative like every other walk in
/// this module — one frame per 8 key bytes would overflow the stack on
/// exactly the deep chains this module's teardown exists to survive
/// (`StrNode` has no `Drop`, so both the immediate and the deferred
/// arm run this same explicit worklist).
fn dispose_tree(root: *mut StrNode, alloc: &NodeAlloc, defer: DeferHandle<'_>) {
    let mut stack: Vec<*mut StrNode> = vec![root];
    while let Some(p) = stack.pop() {
        // Queue children and dispose suffixes while iterating (the
        // continuation words are plain values in the map — neither
        // disposal touches the map being iterated, and clearing the
        // map inside `dispose_node` below does not follow them).
        // SAFETY: unlinked subtree, writer-exclusive.
        for (k, v) in unsafe { (*p).map.iter() } {
            if !is_terminal(k) {
                if is_suffix_ptr(v) {
                    dispose_suffix(unpack_suffix(v), defer, SuffixArena::of(alloc));
                } else {
                    debug_assert_ne!(v, 0);
                    stack.push(unpack_child(v));
                }
            }
        }
        dispose_node(p, alloc, defer);
    }
}

/// Packs `key[off..]`'s next chunk big-endian; `true` when fewer than eight
/// bytes remain, so the chunk is the last one this key contributes.
///
/// This is a *length* rule, while every read of a stored entry decides
/// terminality by *content* ([`is_terminal`], a zero byte in the chunk). The
/// two agree on the NUL-free key domain and only there, and outside it they
/// disagreed badly enough to hand a caller a tagged heap pointer as their
/// value slot (#794).
///
/// They are not reconciled here. They are reconciled by [`NulFreeStr`], which
/// is what the public entry points take: a key with no NUL cannot produce a
/// chunk `is_terminal` calls terminal unless it was zero-padded, which is
/// exactly when this rule reports terminal too. The domain is a type, so this
/// rule can stay the cheap one -- it reduces to a bound on the descent's
/// induction variable and hoists out of the loop, where a content test on a
/// data-dependent word does not. Deciding terminality by content here was
/// measured at +4.12% on `strmap_get`; the key type costs nothing.
fn chunk_at(key: &[u8], off: usize) -> (u64, bool) {
    let rest = &key[off.min(key.len())..];
    let mut c = [0u8; CHUNK];
    let n = rest.len().min(CHUNK);
    c[..n].copy_from_slice(&rest[..n]);
    let chunk = u64::from_be_bytes(c);
    (chunk, rest.len() < CHUNK)
}

/// Appends the byte content of a terminal chunk (the bytes before its NUL).
///
/// Keys are NUL-free and [`chunk_at`] zero-pads, so every byte after a
/// terminal chunk's first NUL is also zero and the content length is
/// `CHUNK - trailing_zeros / 8`. That turns a per-byte loop into one fixed
/// 8-byte append and a truncate.
///
/// Out of line on purpose: inlined into `StrCursor::next` it added about four
/// instructions to every step, terminal or not, on the `strmap_cursor_scan`
/// Callgrind arm, which outweighed the call it saves on terminal entries.
#[inline(never)]
fn push_terminal(out: &mut Vec<u8>, chunk: u64) {
    let len = out.len() + CHUNK - (chunk.trailing_zeros() / 8) as usize;
    out.extend_from_slice(&chunk.to_be_bytes());
    out.truncate(len);
}

/// Appends a continuation chunk's 8 bytes and then `tail`, the bytes of the
/// suffix leaf it leads to: one capacity check for both, one store for the
/// chunk, and for a tail shorter than 16 bytes two overlapping loads and
/// stores in place of a `memcpy` call. Every read stays inside `tail`.
#[inline(never)]
fn push_chunk_suffix(out: &mut Vec<u8>, chunk: u64, tail: &[u8]) {
    let n = tail.len();
    let at = out.len();
    out.reserve(CHUNK + n);
    // SAFETY: `reserve` leaves capacity for `CHUNK + n` bytes past `at`, and
    // the writes below cover exactly `at..at + CHUNK + n`: the chunk at
    // `at..at + CHUNK`, and `copy_small` writes all `n` bytes of `tail`
    // after it, each read in bounds of `tail`. `set_len` then publishes only
    // bytes that were written.
    unsafe {
        let dst = out.as_mut_ptr().add(at);
        dst.cast::<[u8; CHUNK]>()
            .write_unaligned(chunk.to_be_bytes());
        copy_small(tail.as_ptr(), dst.add(CHUNK), n);
        out.set_len(at + CHUNK + n);
    }
}

/// Copies `n` bytes from `src` to `dst`. Below 16 bytes it copies the first
/// and the last `w` bytes, for the widest `w` in {8, 4, 2, 1} with `w <= n`;
/// the two ranges overlap and together cover all `n`.
///
/// # Safety
///
/// `src` is valid for `n` byte reads and `dst` for `n` byte writes, and the
/// two ranges do not overlap.
#[inline(always)]
unsafe fn copy_small(src: *const u8, dst: *mut u8, n: usize) {
    macro_rules! pair {
        ($t:ty, $w:expr) => {
            // SAFETY: `$w <= n`, so both `[0, $w)` and `[n - $w, n)` lie in
            // the caller's ranges; unaligned accesses are used throughout.
            unsafe {
                let a = src.cast::<$t>().read_unaligned();
                let b = src.add(n - $w).cast::<$t>().read_unaligned();
                dst.cast::<$t>().write_unaligned(a);
                dst.add(n - $w).cast::<$t>().write_unaligned(b);
            }
        };
    }
    if n >= 16 {
        // SAFETY: the caller's contract, as for every branch.
        unsafe { core::ptr::copy_nonoverlapping(src, dst, n) };
    } else if n >= 8 {
        pair!(u64, 8);
    } else if n >= 4 {
        pair!(u32, 4);
    } else if n >= 2 {
        pair!(u16, 2);
    } else if n == 1 {
        // SAFETY: `n == 1`, in the caller's ranges.
        unsafe { dst.write(src.read()) };
    }
}

/// The byte-scan definition of a terminal chunk's content, retained as the
/// parity oracle for [`push_terminal`].
#[cfg(test)]
fn terminal_bytes_scan(chunk: u64) -> impl Iterator<Item = u8> {
    chunk.to_be_bytes().into_iter().take_while(|&b| b != 0)
}

/// True when the chunk contains a NUL byte (terminal entry).
///
/// SWAR haszero rather than `to_be_bytes().contains(&0)`. The three-operation
/// form is branchless and byte-order independent -- it asks whether any lane
/// borrowed, which does not depend on which end the lanes came from.
///
/// No codegen claim is made for the form it replaced: §8.7 wants an
/// `--emit asm` citation for one and there is none.
///
/// This runs at every descent level, via [`chunk_at`], on every operation --
/// it is the whole of the instruction cost the pull request discloses, and
/// the reason the length rule it replaced was cheaper (that rule reduced to a
/// bound on the loop's induction variable and hoisted; this does not).
///
/// `is_terminal_scan` is the reference implementation, kept as the parity
/// oracle for `swar_haszero_matches_the_byte_scan`.
#[inline]
fn is_terminal(chunk: u64) -> bool {
    const LO: u64 = 0x0101_0101_0101_0101;
    const HI: u64 = 0x8080_8080_8080_8080;
    chunk.wrapping_sub(LO) & !chunk & HI != 0
}

/// The byte-scan definition of [`is_terminal`], retained as the parity oracle.
#[cfg(test)]
fn is_terminal_scan(chunk: u64) -> bool {
    chunk.to_be_bytes().contains(&0)
}

/// A byte string containing no NUL byte: the key domain of [`ExpanseStrMap`].
///
/// The map is a digital trie over 8-byte chunks that uses a zero byte as the
/// end-of-key sentinel, so a NUL inside a key is not a value the encoding can
/// represent -- it is the terminator. Before this type the domain was a
/// documented precondition on `&[u8]` enforced by a `debug_assert!`, and a
/// release build met an out-of-domain key with a wild pointer dereference
/// from safe code (#794).
///
/// Construction is where the cost is paid, once, rather than on every descent
/// level of every operation:
///
/// * [`new`](Self::new) validates and is the safe path;
/// * [`new_unchecked`](Self::new_unchecked) is for callers that have already
///   established the invariant -- the C ABI shims, which reach the key through
///   `strlen` or `CStr::from_ptr`, and [`escape_encode`](crate::domain) output,
///   which maps `0x00` away by construction.
///
/// Callers holding arbitrary bytes want either [`ExpanseBytesMap`] (hashed, no
/// ordered iteration) or the order-preserving escape the domain dictionary
/// uses; see #808.
///
/// [`ExpanseBytesMap`]: crate::bytesmap::ExpanseBytesMap
#[repr(transparent)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NulFreeStr([u8]);

impl NulFreeStr {
    /// Borrows `bytes` as a key, or `None` if it contains a NUL.
    #[inline]
    #[must_use]
    pub fn new(bytes: &[u8]) -> Option<&Self> {
        if bytes.contains(&0) {
            None
        } else {
            // SAFETY: just checked; `Self` is `repr(transparent)` over `[u8]`.
            Some(unsafe { Self::new_unchecked(bytes) })
        }
    }

    /// Borrows `bytes` as a key without checking.
    ///
    /// # Safety
    ///
    /// `bytes` must contain no NUL byte. A violation is not immediately
    /// unsound, but it puts the map into the unspecified state [`chunk_at`]
    /// describes, from which the ordered surfaces disagree with each other.
    #[inline]
    #[must_use]
    pub const unsafe fn new_unchecked(bytes: &[u8]) -> &Self {
        // SAFETY: `repr(transparent)` makes the layouts identical; the caller
        // guarantees the domain.
        unsafe { &*(core::ptr::from_ref(bytes) as *const Self) }
    }

    /// The underlying bytes.
    #[inline]
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Length in bytes.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the key is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl core::fmt::Debug for NulFreeStr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&self.0, f)
    }
}

impl<'a> TryFrom<&'a [u8]> for &'a NulFreeStr {
    type Error = NulInKey;

    #[inline]
    fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
        NulFreeStr::new(bytes).ok_or(NulInKey)
    }
}

/// The key handed to [`NulFreeStr`] contained a NUL byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NulInKey;

impl core::fmt::Display for NulInKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("ExpanseStrMap keys must not contain a NUL byte")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for NulInKey {}

impl StrNode {
    fn new() -> Self {
        Self {
            // Even: stable, no store in progress. Every branch header the
            // engine allocates starts its version at 0 the same way.
            cover: 0,
            dirty: 0,
            map: MapCore::new(),
        }
    }

    /// The dirty flag as the optimistic path addresses it (see the type
    /// docs).
    ///
    /// # Safety
    ///
    /// `p` must point at a live node. The flag is only ever accessed
    /// through this view, and `AtomicU32` is layout-compatible with `u32`.
    #[cfg(all(feature = "std", not(feature = "ablation-str-serial-writers")))]
    #[inline(always)]
    unsafe fn dirty_cell<'a>(p: *const StrNode) -> &'a AtomicU32 {
        // SAFETY: forwarded contract.
        unsafe { &*(&raw const (*p).dirty).cast::<AtomicU32>() }
    }

    /// Exclusive path: restores this node's sub-map population from a
    /// census fold if optimistic writers left it stale (see the type
    /// docs). Called before the exclusive path reads or changes that
    /// population; a no-op on a clean node, which is every node of a map
    /// that has never had concurrent writers.
    #[cfg(feature = "std")]
    #[inline(always)]
    fn resync_if_dirty(&mut self) {
        // Through the unique borrow, so the view is a child of it.
        // SAFETY: `self` is live and the flag is accessed only through this
        // view.
        let cell = unsafe { &*(&raw mut self.dirty).cast::<AtomicU32>() };
        if cell.load(Ordering::Relaxed) != 0 {
            let top = self.map.root_top_ptr_mut();
            if !top.is_null() {
                // SAFETY: the caller is exclusive (writers quiesced or
                // serialised), and `top` is the live top edge of this
                // node's tree.
                let pop = unsafe { crate::sync::fold_branch_pop0(top, 8) };
                self.map.set_tree_pop(pop);
            }
            cell.store(0, Ordering::Relaxed);
        }
    }

    /// The address of this node's cover word, in the form the OCC protocol
    /// addresses a branch header's version (`occ::version_cell`,
    /// `occ::node_sample`, `Cover::Node`).
    ///
    /// Read-only provenance on purpose: a writer reaches the node through
    /// the raw `*mut StrNode` it decoded from its parent's continuation
    /// entry and derives `&raw mut (*node).cover` there with write
    /// provenance, rather than casting one out of a shared borrow
    /// (AGENTS.md §5, Stacked/Tree Borrows hygiene). The reader and writer
    /// paths project the word from their raw node pointer for the same
    /// reason, so this accessor serves the tests that pin the layout.
    #[cfg(test)]
    #[inline(always)]
    fn cover_addr(&self) -> *const u32 {
        &raw const self.cover
    }

    /// # Safety
    ///
    /// `v` must be a continuation child pointer produced by `Box::into_raw`.
    unsafe fn child<'a>(v: u64) -> &'a StrNode {
        // SAFETY: per contract, `v` is a live Box<StrNode> pointer.
        unsafe { &*unpack_child(v) }
    }

    /// Largest entry in this subtree; appends its key bytes to `out`.
    fn max_entry(&self, out: &mut Vec<u8>) -> NonNull<u64> {
        self.extreme_entry(out, false)
    }

    /// The subtree's first (`min`) or last (`!min`) entry.
    ///
    /// Iterative, like every other walk in this module: one frame per 8
    /// key bytes turns a long key into a stack overflow, and this runs
    /// down the deepest chain in the tree by construction. The two
    /// directions differ only in which end of each node they take, so
    /// they share a walk.
    fn extreme_entry(&self, out: &mut Vec<u8>, min: bool) -> NonNull<u64> {
        let mut node: *const StrNode = self;
        loop {
            // SAFETY: `self` on the first turn, then continuation values,
            // which are live child nodes; the descent never revisits one.
            let n = unsafe { &*node };
            let (chunk, v) = if min {
                n.map.first().expect("non-empty node")
            } else {
                n.map.last().expect("non-empty node")
            };
            if is_terminal(chunk) {
                push_terminal(out, chunk);
                return n.map.get_slot_ptr(chunk).expect("present chunk");
            }
            out.extend_from_slice(&chunk.to_be_bytes());
            if is_suffix_ptr(v) {
                let sfx = unpack_suffix(v);
                // SAFETY: tagged pointer encodes a live suffix leaf; the raw
                // pointer carries provenance over the inline bytes, which a
                // `&StrSuffix` would not (see `StrSuffix`).
                out.extend_from_slice(unsafe { suffix_bytes(sfx) });
                // SAFETY: field-precise pointer to the value word at offset 0.
                return NonNull::new(unsafe { &raw mut (*sfx).value })
                    .expect("non-null value slot");
            }
            node = unpack_child(v);
        }
    }

    /// Emits the entry `cursor` names: a terminal chunk *is* the answer,
    /// a continuation chunk contributes its subtree extreme. `None`
    /// cursor means this node had nothing in the requested direction,
    /// which is what makes the caller backtrack.
    fn take_from(
        &self,
        cursor: Option<(u64, u64)>,
        out: &mut Vec<u8>,
        min: bool,
    ) -> Option<NonNull<u64>> {
        let (chunk, v) = cursor?;
        if is_terminal(chunk) {
            push_terminal(out, chunk);
            Some(self.map.get_slot_ptr(chunk).expect("present chunk"))
        } else if is_suffix_ptr(v) {
            let sfx = unpack_suffix(v);
            out.extend_from_slice(&chunk.to_be_bytes());
            // SAFETY: live suffix leaf, raw provenance over the inline bytes.
            out.extend_from_slice(unsafe { suffix_bytes(sfx) });
            // SAFETY: field-precise pointer to the value word at offset 0.
            Some(NonNull::new(unsafe { &raw mut (*sfx).value }).expect("non-null value slot"))
        } else {
            out.extend_from_slice(&chunk.to_be_bytes());
            // SAFETY: continuation values are child pointers.
            Some(unsafe { Self::child(v) }.extreme_entry(out, min))
        }
    }

    /// Smallest entry with key `>= key[off..]`; appends bytes to `out`.
    ///
    /// Iterative with an explicit backtrack stack. The recursion this
    /// replaces was not a plain descent: when the deeper call finds
    /// nothing at-or-after, the level *resumes* at its next sibling, so
    /// the stack has to carry both the node to resume at and the `out`
    /// length to truncate back to.
    fn next_at_or_after(
        &self,
        key: &[u8],
        off: usize,
        out: &mut Vec<u8>,
    ) -> Option<NonNull<u64>> {
        // (node to resume at, its target chunk, `out` length on entry)
        let mut stack: Vec<(*const StrNode, u64, usize)> = Vec::new();
        let mut node: *const StrNode = self;
        let mut off = off;

        loop {
            // SAFETY: `self`, then continuation values — all live nodes.
            let n = unsafe { &*node };
            let (target, _) = chunk_at(key, off);
            let cursor = n.map.next_at_or_after(target);
            if let Some((chunk, v)) = cursor
                && chunk == target
                && !is_terminal(chunk)
            {
                if is_suffix_ptr(v) {
                    let sfx = unpack_suffix(v);
                    // SAFETY: live suffix leaf, raw provenance over the bytes.
                    let bytes = unsafe { suffix_bytes(sfx) };
                    let rem = &key[off.min(key.len()) + CHUNK.min(key.len().saturating_sub(off))..];
                    if rem <= bytes {
                        out.extend_from_slice(&chunk.to_be_bytes());
                        out.extend_from_slice(bytes);
                        // SAFETY: field-precise pointer to the value word.
                        return Some(
                            NonNull::new(unsafe { &raw mut (*sfx).value })
                                .expect("non-null value slot"),
                        );
                    }
                    // Suffix is strictly less than target key remainder; resume at next sibling.
                    let sibling = n.map.next_after(target);
                    if let Some(slot) = n.take_from(sibling, out, true) {
                        return Some(slot);
                    }
                } else {
                    // Exact continuation: the answer, if there is one, is
                    // deeper. Record where to resume if it is not.
                    stack.push((node, target, out.len()));
                    out.extend_from_slice(&chunk.to_be_bytes());
                    node = unpack_child(v);
                    off += CHUNK;
                    continue;
                }
            } else if let Some(slot) = n.take_from(cursor, out, true) {
                return Some(slot);
            }

            // Nothing at or after here: unwind to the nearest ancestor
            // with an unexplored sibling.
            loop {
                let (parent, parent_target, mark) = stack.pop()?;
                out.truncate(mark);
                // SAFETY: recorded during the descent; still live.
                let p = unsafe { &*parent };
                let sibling = p.map.next_after(parent_target);
                if let Some(slot) = p.take_from(sibling, out, true) {
                    return Some(slot);
                }
            }
        }
    }

    /// Largest entry with key `<= key[off..]` (or `<` when `exclusive`
    /// and the key terminates in this chunk); appends bytes to `out`.
    ///
    /// The mirror of `next_at_or_after`, with the same explicit
    /// backtracking and the same reason for it.
    fn prev_at_or_before(
        &self,
        key: &[u8],
        off: usize,
        exclusive: bool,
        out: &mut Vec<u8>,
    ) -> Option<NonNull<u64>> {
        let mut stack: Vec<(*const StrNode, u64, usize)> = Vec::new();
        let mut node: *const StrNode = self;
        let mut off = off;

        loop {
            // SAFETY: as in `next_at_or_after`.
            let n = unsafe { &*node };
            let (target, target_terminal) = chunk_at(key, off);
            // Exact continuation first: deeper entries share this chunk
            // and all sort above anything below it in this node.
            if !target_terminal && let Some(v) = n.map.get(target) {
                if is_suffix_ptr(v) {
                    let sfx = unpack_suffix(v);
                    // SAFETY: live suffix leaf, raw provenance over the bytes.
                    let bytes = unsafe { suffix_bytes(sfx) };
                    let rem = &key[off.min(key.len()) + CHUNK.min(key.len().saturating_sub(off))..];
                    let cmp = rem.cmp(bytes);
                    let match_ok = if exclusive {
                        cmp == core::cmp::Ordering::Greater
                    } else {
                        cmp != core::cmp::Ordering::Less
                    };
                    if match_ok {
                        out.extend_from_slice(&target.to_be_bytes());
                        out.extend_from_slice(bytes);
                        // SAFETY: field-precise pointer to the value word.
                        return Some(
                            NonNull::new(unsafe { &raw mut (*sfx).value })
                                .expect("non-null value slot"),
                        );
                    }
                    let sibling = n.map.prev_before(target);
                    if let Some(slot) = n.take_from(sibling, out, false) {
                        return Some(slot);
                    }
                } else {
                    stack.push((node, target, out.len()));
                    out.extend_from_slice(&target.to_be_bytes());
                    node = unpack_child(v);
                    off += CHUNK;
                    continue;
                }
            } else {
                // Then entries at or below the target chunk in this node. A
                // terminal exclusive target must not match itself.
                let cursor = if target_terminal && !exclusive {
                    n.map.prev_at_or_before(target)
                } else {
                    n.map.prev_before(target)
                };
                if let Some(slot) = n.take_from(cursor, out, false) {
                    return Some(slot);
                }
            }

            loop {
                let (parent, parent_target, mark) = stack.pop()?;
                out.truncate(mark);
                // SAFETY: as in `next_at_or_after`.
                let p = unsafe { &*parent };
                // The descent branch is only taken for a non-terminal
                // target, whose own subtree was just exhausted — so the
                // resume point is strictly below it, never at it.
                let sibling = p.map.prev_before(parent_target);
                if let Some(slot) = p.take_from(sibling, out, false) {
                    return Some(slot);
                }
            }
        }
    }

    /// Removes `key[off..]` from this subtree; see [`ExpanseStrMap::remove`].
    ///
    /// `SHARED` selects the deferred twin (Refs #929): each sub-map
    /// mutation is bracketed by its node's cover and preceded by a census
    /// re-sync, and a pruned child is marked obsolete before it retires.
    /// The unshared instantiation is the plain path with nothing added.
    fn remove_impl<const SHARED: bool>(
        &mut self,
        key: &[u8],
        off: usize,
        alloc: &NodeAlloc,
        defer: DeferHandle<'_>,
    ) -> Option<u64> {
        let mut path: Vec<(*mut StrNode, u64)> = Vec::new();
        let mut node: *mut StrNode = &raw mut *self;
        let mut off = off;
        let removed = loop {
            let (chunk, terminal) = chunk_at(key, off);
            // SAFETY: `self` on the first turn, then continuation values
            // recorded on the path — all live, and uniquely borrowed
            // because the descent never revisits a node.
            let n = unsafe { &mut *node };
            if terminal {
                resync::<SHARED>(n);
                break covered::<SHARED, _>(n, alloc, |m| {
                    m.remove_pathless_dispatch::<SHARED, SHARED>(alloc, chunk)
                })?;
            }
            let v = n.map.get(chunk)?;
            if is_suffix_ptr(v) {
                let sfx = unpack_suffix(v);
                let rem = &key[off + CHUNK..];
                // SAFETY: live suffix leaf, raw provenance over the bytes.
                if rem == unsafe { suffix_bytes(sfx) } {
                    resync::<SHARED>(n);
                    covered::<SHARED, _>(n, alloc, |m| {
                        m.remove_pathless_dispatch::<SHARED, SHARED>(alloc, chunk)
                    });
                    // Read out before disposal: the borrow of the bytes above
                    // has ended, and nothing may reference the block once
                    // `dispose_suffix` has it.
                    // SAFETY: unlinked but still live; last owner.
                    let removed_val = unsafe { (*sfx).value };
                    // Unlinked above; retired when shared.
                    dispose_suffix(sfx, defer, SuffixArena::of(alloc));
                    break removed_val;
                }
                return None;
            }
            path.push((node, chunk));
            node = unpack_child(v);
            off += CHUNK;
        };

        // Unwind: an emptied child is unlinked and freed, which may empty
        // its parent in turn. The first non-empty ancestor stops it —
        // nothing above that can have been emptied by this removal. A
        // dirty child is in tree state and so never empty, whatever its
        // stale population reads.
        while let Some((parent_ptr, chunk)) = path.pop() {
            // SAFETY: recorded during the descent; still live.
            let parent = unsafe { &mut *parent_ptr };
            let child_v = parent.map.get(chunk).expect("path entry still linked");
            if !is_suffix_ptr(child_v) {
                // SAFETY: continuation value, a live child node.
                let empty = unsafe { &*(unpack_child(child_v)) }.map.is_empty();
                if !empty {
                    break;
                }
                resync::<SHARED>(parent);
                covered::<SHARED, _>(parent, alloc, |m| {
                    m.remove_pathless_dispatch::<SHARED, SHARED>(alloc, chunk)
                });
                // Unlinked above; marked obsolete and retired when shared.
                dispose_node(unpack_child(child_v), alloc, defer);
            }
        }
        Some(removed)
    }

    /// Heap bytes of this subtree's node shells and suffix leaves — the
    /// allocations **outside** the shared `NodeAlloc` (whose
    /// `bytes_in_use` covers every sub-map's interior byte-exactly, all
    /// sub-tries at once, since #363 Step A). `ExpanseStrMap::mem_used`
    /// is the sum of both.
    ///
    /// Iterative: `clear()` calls this, so recursing would overflow the
    /// stack on exactly the deep chains the iterative teardown exists to
    /// survive.
    fn shell_bytes(&self) -> u64 {
        let mut bytes = 0u64;
        let mut stack: Vec<*const StrNode> = vec![core::ptr::from_ref(self)];
        while let Some(p) = stack.pop() {
            // SAFETY: `self` plus continuation values, all live nodes.
            let node = unsafe { &*p };
            bytes += size_of::<Self>() as u64;
            for (k, v) in node.map.iter() {
                if !is_terminal(k) {
                    if is_suffix_ptr(v) {
                        // SAFETY: tagged pointer encodes a live suffix leaf.
                        let len = unsafe { (*unpack_suffix(v)).len };
                        // One block: header plus the inline bytes, which is
                        // exactly what `dispose_suffix` will hand back —
                        // unless it came from `NodeAlloc`, whose
                        // `bytes_in_use` already counts it.
                        if !SUFFIX_IN_NODE_ALLOC {
                            bytes += suffix_layout(len).size() as u64;
                        }
                    } else {
                        debug_assert_ne!(v, 0);
                        stack.push(unpack_child(v));
                    }
                }
            }
        }
        bytes
    }
}

/// One level of a [`StrCursor`]'s path: the node, the chunk the cursor sits
/// at inside it, and the length `key` had before that chunk contributed to it.
///
/// `key_len` is what makes the walk incremental. Advancing within a level
/// truncates the key buffer back to it and appends the next chunk, so the
/// prefix every ancestor contributed is never recomputed and never
/// reallocated.
///
/// `advanced` makes advancing within a level incremental too (#1096). The
/// first advance of a level is a positional `next_after`, one descent of the
/// level's sub-map; the second builds a sub-map cursor just past the current
/// chunk in [`Walk::subs`], and every later advance streams it instead of
/// re-descending from the sub-map root. A level advanced only once — the
/// one-entry sub-maps under a distinct key — never builds one.
struct StrFrame {
    node: *mut StrNode,
    chunk: u64,
    key_len: usize,
    advanced: bool,
}

/// An ordered cursor over [`ExpanseStrMap`], and the reason it exists: the
/// `next_at_or_after` / `next_after` pair it sits beside is a *positional*
/// surface, so a k-element scan pays k fresh root descents and allocates a
/// key `Vec` for each one (#722). This descends once and keeps the path, so
/// the scan is one descent and **zero allocations per element** — the key is
/// handed out as a slice of a buffer the cursor owns and reuses.
///
/// Ordering is byte-lexicographic, as everywhere else in this module: chunks
/// are packed big-endian, so the word maps' numeric order *is* byte order,
/// and a terminal chunk carries a NUL, which sorts below every continuation
/// byte. Nothing here re-derives that; it falls out of taking each level's
/// entries in `first` / `next_after` order.
///
/// The cursor borrows the map mutably for its lifetime, which is also how the
/// "valid until the next structural mutation" contract of the surrounding
/// surface is enforced here — by the borrow checker rather than by a comment.
///
/// ```text
/// let mut c = map.cursor_at_or_after(b"ap");
/// while let Some((key, slot)) = c.next() {
///     // `key` borrows the cursor's buffer and is valid until the next call.
/// }
/// ```
///
/// Deliberately not a doctest: this crate has none, and the ASan job runs
/// `cargo test -p expanse-trie`, whose doctest binaries do not link the
/// sanitizer runtime — the crate's first doctest would fail that job rather
/// than the code failing it. The executable form of this snippet is
/// `tests::cursor_walks_the_documented_example`, which the ASan and Miri
/// lanes both run, so the example is enforced rather than merely written.
pub struct StrCursor<'a> {
    walk: Walk<'a, false>,
}

impl<'a> StrCursor<'a> {
    /// Advances to the next entry in byte-lexicographic order.
    ///
    /// The returned key borrows the cursor's own buffer and is valid until the
    /// next call; copy it if it must outlive that. The slot follows the
    /// surrounding surface's contract and stays valid until the map is
    /// structurally mutated, which the cursor's borrow prevents.
    #[inline]
    #[allow(clippy::should_implement_trait)] // lending: the key borrows `self`
    pub fn next(&mut self) -> Option<(&[u8], NonNull<u64>)> {
        self.walk.next()
    }
}

/// An ordered cursor over the keys of an [`ExpanseStrMap`] that start with a
/// prefix, from [`ExpanseStrMap::cursor_prefix`]. It walks as [`StrCursor`]
/// does and ends at the prefix boundary by itself, so a caller needs no
/// per-key comparison.
pub struct StrPrefixCursor<'a> {
    walk: Walk<'a, true>,
}

impl<'a> StrPrefixCursor<'a> {
    /// Advances to the next entry under the prefix, in byte-lexicographic
    /// order; `None` once past it.
    ///
    /// The key and slot follow [`StrCursor::next`]'s contract.
    #[inline]
    #[allow(clippy::should_implement_trait)] // lending: the key borrows `self`
    pub fn next(&mut self) -> Option<(&[u8], NonNull<u64>)> {
        self.walk.next()
    }
}

/// The walk behind both cursors. `BOUNDED` compiles the prefix-bound checks
/// in; [`StrCursor`] instantiates it without them, so an unbounded walk pays
/// nothing for them.
struct Walk<'a, const BOUNDED: bool> {
    /// The path from the root to the entry last emitted. Empty before the
    /// first `next` and after the walk is exhausted.
    stack: Vec<StrFrame>,
    /// The sub-map cursors of the levels that have gone live — advanced twice
    /// — tagged with their frame depth, in increasing depth order. Only
    /// `subs[..live]` is in use; slots past it are retained and re-seeded in
    /// place when a later level goes live, so a scan costs no allocation per
    /// element or per level (#722) and a cursor is never moved (#1096). A
    /// level that never goes live — every single-entry node on a path — costs
    /// no cursor at all.
    subs: Vec<(usize, RawCursor<true>)>,
    /// How many leading `subs` slots belong to frames still on the stack.
    live: usize,
    /// The key last emitted, reused across elements.
    key: Vec<u8>,
    /// Set once the walk has run out, so a caller looping to `None` does not
    /// restart it.
    done: bool,
    /// Set until the first `next`, which seeds the walk rather than advancing.
    started: bool,
    /// An entry a `seek` already positioned on. The first `next` emits it
    /// instead of advancing, so `cursor_at_or_after(k)` yields `k` itself when
    /// `k` is present — the same inclusive sense as `next_at_or_after`.
    pending: Option<NonNull<u64>>,
    /// The prefix bound of a [`cursor_prefix`](ExpanseStrMap::cursor_prefix)
    /// walk. An entry produced by advancing the level at depth `bound_depth`
    /// is in range iff its chunk masked by `bound_mask` equals `bound_want`;
    /// one produced by advancing a shallower level is not, since that level's
    /// chunk is a whole chunk of the prefix. Deeper levels sit under a chunk
    /// already checked. Read only when `BOUNDED`.
    bound_depth: usize,
    bound_mask: u64,
    bound_want: u64,
    /// Held for the borrow, and to keep the root reachable.
    map: &'a mut ExpanseStrMap,
}

impl<'a, const BOUNDED: bool> Walk<'a, BOUNDED> {
    fn new(map: &'a mut ExpanseStrMap) -> Self {
        Self {
            stack: Vec::new(),
            subs: Vec::new(),
            live: 0,
            key: Vec::new(),
            done: false,
            started: false,
            pending: None,
            bound_depth: 0,
            bound_mask: 0,
            bound_want: 0,
            map,
        }
    }

    /// Whether an entry with `chunk` produced by advancing the level at `depth`
    /// lies past the cursor's prefix bound. Always `false` when unbounded.
    #[inline(always)]
    fn past_bound(&self, depth: usize, chunk: u64) -> bool {
        BOUNDED
            && depth <= self.bound_depth
            && (depth < self.bound_depth || chunk & self.bound_mask != self.bound_want)
    }

    /// Pushes a frame at depth `stack.len()`. A live cursor is dropped from
    /// `subs[..live]` when its frame is popped for good, which `sibling` does
    /// before it reads the level, so a fresh frame never finds one at its own
    /// depth and pushing needs no bookkeeping.
    #[inline(always)]
    fn push_frame(&mut self, node: *mut StrNode, chunk: u64, key_len: usize, advanced: bool) {
        self.stack.push(StrFrame {
            node,
            chunk,
            key_len,
            advanced,
        });
    }

    /// The next entry after `frame.chunk` in the level `frame` records, which
    /// sat at depth `stack.len()` before it was popped: streamed from that
    /// depth's sub-map cursor if built, a positional `next_after` on the
    /// level's first advance or on a root-leaf level, and a newly built cursor
    /// on a trie level's second advance.
    fn sibling(&mut self, frame: &StrFrame) -> Option<(u64, u64)> {
        let depth = self.stack.len();
        // Every frame deeper than `depth` has been popped, so their cursors
        // are dead; the one at `depth`, if any, is this level's.
        while self.live > 0 && self.subs[self.live - 1].0 > depth {
            self.live -= 1;
        }
        if self.live > 0 && self.subs[self.live - 1].0 == depth {
            return self.subs[self.live - 1].1.next();
        }
        // SAFETY: `node` was recorded on the walk and stays live for the
        // cursor's borrow of the map. The reference is transient: a
        // `RawCursor` holds raw pointers only, never this borrow, and
        // it reads only entries after those already emitted, which are the
        // only slots a caller can have written through.
        let node: &StrNode = unsafe { &*frame.node };
        // A root-leaf level holds at most `ROOT_LEAF_CAP` entries, where a
        // positional step is a binary search of that array; a cursor built for
        // it would cost more than the steps left in the level, and on the
        // two-entry child nodes of a dense path set it is built only to report
        // that the level is exhausted.
        if !frame.advanced || !node.map.root_is_tree() {
            return node.map.next_after(frame.chunk);
        }
        let start = frame.chunk.checked_add(1)?;
        if self.live == self.subs.len() {
            self.subs.push((depth, RawCursor::empty()));
        }
        let (d, cur) = &mut self.subs[self.live];
        *d = depth;
        self.live += 1;
        node.map.reset_raw_cursor(cur, start);
        cur.next()
    }

    /// Descends from `node` taking the smallest entry at every level until an
    /// entry that *is* a value is reached, pushing a frame per level.
    ///
    /// `entry` is the entry to take at `node`; every level below takes its
    /// `first`. Returns the value slot, or `None` for an empty node, which a
    /// well-formed trie does not contain below the root.
    fn descend(&mut self, node: *mut StrNode, entry: (u64, u64)) -> Option<NonNull<u64>> {
        self.descend_from(node, entry, false)
    }

    /// [`descend`](Self::descend), with the first level's frame carrying the
    /// advance state of the frame it replaces: with `advanced`, the level keeps
    /// its sub-map cursor slot; every level below starts fresh.
    fn descend_from(
        &mut self,
        node: *mut StrNode,
        (chunk, v): (u64, u64),
        advanced: bool,
    ) -> Option<NonNull<u64>> {
        let key_len = self.key.len();
        self.push_frame(node, chunk, key_len, advanced);
        self.finish_descent(node, chunk, v)
    }

    /// Completes a descent whose frame for `(chunk, v)` at `node` is already
    /// on the stack: appends that level's key bytes and, while the entry is a
    /// child node, pushes a fresh frame for each level below taking its
    /// `first`, until an entry that *is* a value is reached.
    #[inline(always)]
    fn finish_descent(
        &mut self,
        mut node: *mut StrNode,
        mut chunk: u64,
        mut v: u64,
    ) -> Option<NonNull<u64>> {
        loop {
            if is_terminal(chunk) {
                push_terminal(&mut self.key, chunk);
                // SAFETY: `node` is a live node on the path just walked, and
                // the chunk came from its own map, so the slot is present.
                return unsafe { &mut *node }.map.value_slot_pathless(chunk);
            }
            if is_suffix_ptr(v) {
                let sfx = unpack_suffix(v);
                // SAFETY: tagged pointer encodes a live suffix leaf; the raw
                // pointer carries provenance over the inline bytes.
                push_chunk_suffix(&mut self.key, chunk, unsafe { suffix_bytes(sfx) });
                // SAFETY: field-precise pointer to the value word at offset 0.
                return Some(
                    NonNull::new(unsafe { &raw mut (*sfx).value }).expect("non-null value slot"),
                );
            }
            self.key.extend_from_slice(&chunk.to_be_bytes());
            node = unpack_child(v);
            // SAFETY: untagged continuation value, a live child node.
            (chunk, v) = unsafe { &*node }.map.first()?;
            let key_len = self.key.len();
            self.push_frame(node, chunk, key_len, false);
        }
    }

    /// Advances to the next entry in byte-lexicographic order.
    ///
    /// The returned key borrows the cursor's own buffer and is valid until the
    /// next call; copy it if it must outlive that. The slot follows the
    /// surrounding surface's contract and stays valid until the map is
    /// structurally mutated, which the cursor's borrow prevents.
    fn next(&mut self) -> Option<(&[u8], NonNull<u64>)> {
        if self.done {
            return None;
        }
        if let Some(slot) = self.pending.take() {
            return Some((&self.key, slot));
        }
        if !self.started {
            self.started = true;
            let root: *mut StrNode = match self.map.root.as_deref_mut() {
                Some(r) => core::ptr::from_mut(r),
                None => {
                    self.done = true;
                    return None;
                }
            };
            // SAFETY: the root is live for the cursor's borrow of the map.
            let first = match unsafe { &*root }.map.first() {
                Some(e) => e,
                None => {
                    self.done = true;
                    return None;
                }
            };
            let slot = self.descend(root, first);
            return self.emit(slot);
        }
        // The deepest level streams from its live sub-map cursor: its next
        // entry replaces the top frame in place, with no pop, no push and no
        // unwind. A live cursor at the top frame's depth is that frame's own,
        // since `sibling` drops a level's cursor before any other frame can
        // reach its depth.
        let depth = self.stack.len().wrapping_sub(1);
        if self.live > 0 && self.subs[self.live - 1].0 == depth {
            let next = self.subs[self.live - 1].1.next();
            if let Some((chunk, _)) = next
                && self.past_bound(depth, chunk)
            {
                self.done = true;
                return None;
            }
            let top = &mut self.stack[depth];
            let (node, key_len) = (top.node, top.key_len);
            match next {
                Some((chunk, v)) => {
                    top.chunk = chunk;
                    self.key.truncate(key_len);
                    let slot = self.finish_descent(node, chunk, v);
                    return self.emit(slot);
                }
                None => {
                    // Exhausted: abandon the level and unwind from its parent.
                    self.stack.pop();
                    self.key.truncate(key_len);
                }
            }
        }
        // Unwind to the nearest level with an unexplored sibling, dropping the
        // key bytes each abandoned level contributed.
        while let Some(frame) = self.stack.pop() {
            // A level above the bound holds a whole chunk of the prefix, so
            // none of its siblings is in range.
            if BOUNDED && self.stack.len() < self.bound_depth {
                break;
            }
            self.key.truncate(frame.key_len);
            if let Some(entry) = self.sibling(&frame) {
                if self.past_bound(self.stack.len(), entry.0) {
                    break;
                }
                let slot = self.descend_from(frame.node, entry, true);
                return self.emit(slot);
            }
        }
        self.done = true;
        None
    }

    /// The key and slot for a completed descent, or the end of the walk.
    fn emit(&mut self, slot: Option<NonNull<u64>>) -> Option<(&[u8], NonNull<u64>)> {
        match slot {
            Some(s) => Some((&self.key, s)),
            None => {
                self.done = true;
                None
            }
        }
    }

    /// Positions the cursor so the next [`next`](Self::next) returns the first
    /// entry with key `>= key`.
    ///
    /// One descent, like the walk it seeds: the frames it pushes are the ones
    /// `next` then advances through, which is what keeps a bounded range scan
    /// to a single root descent overall.
    fn seek(&mut self, key: &[u8]) -> Option<NonNull<u64>> {
        self.started = true;
        let root: *mut StrNode = match self.map.root.as_deref_mut() {
            Some(r) => core::ptr::from_mut(r),
            None => {
                self.done = true;
                return None;
            }
        };
        let mut node = root;
        let mut off = 0usize;
        loop {
            // SAFETY: the root, then continuation values — all live nodes on a
            // descent that never revisits one.
            let n = unsafe { &*node };
            let (target, _) = chunk_at(key, off);
            let cursor = n.map.next_at_or_after(target);
            if let Some((chunk, v)) = cursor
                && chunk == target
                && !is_terminal(chunk)
            {
                if is_suffix_ptr(v) {
                    let sfx = unpack_suffix(v);
                    // SAFETY: live suffix leaf, raw provenance over the bytes.
                    let bytes = unsafe { suffix_bytes(sfx) };
                    let rem = &key[off.min(key.len()) + CHUNK.min(key.len().saturating_sub(off))..];
                    if rem <= bytes {
                        return self.descend(node, (chunk, v));
                    }
                    // The suffix sorts below the target: resume at the sibling.
                    if let Some(entry) = n.map.next_after(target) {
                        return self.descend(node, entry);
                    }
                } else {
                    // Exact continuation — the answer is deeper if it exists.
                    // Record the level so the unwind below can resume at it.
                    let key_len = self.key.len();
                    self.push_frame(node, chunk, key_len, false);
                    self.key.extend_from_slice(&chunk.to_be_bytes());
                    node = unpack_child(v);
                    off += CHUNK;
                    continue;
                }
            } else if let Some(entry) = cursor {
                return self.descend(node, entry);
            }
            // Nothing at or after here: unwind, exactly as `next` does.
            while let Some(frame) = self.stack.pop() {
                self.key.truncate(frame.key_len);
                if let Some(entry) = self.sibling(&frame) {
                    return self.descend_from(frame.node, entry, true);
                }
            }
            self.done = true;
            return None;
        }
    }
}

/// A sorted map from NUL-free byte strings to `u64` values (compat:
/// JudySL). Iteration order is byte-lexicographic.
pub struct ExpanseStrMap {
    root: Option<Box<StrNode>>,
    pop: u64,
    /// The **one** allocator behind every sub-trie's word map (issue #363
    /// Step A). Nodes store no pointer to it — every operation passes a
    /// short-lived `&self.alloc` borrow down the walk — so the map moves
    /// freely with no self-reference to invalidate, and no boxing or
    /// pinning is needed for address stability.
    alloc: NodeAlloc,
    /// Phase 7 (issue #219 Phase 2): when set, unlinked nodes/suffixes are
    /// retired through the collector instead of freed (concurrent readers
    /// may still hold pointers into them); the shared `alloc` is deferred
    /// to the same collector, so sub-map interior frees and mutation
    /// brackets participate too.
    #[cfg(feature = "std")]
    deferred: OnceLock<Arc<Collector>>,
}

// SAFETY: the map exclusively owns every reachable allocation — node
// shells, suffix leaves, and the shared `NodeAlloc`'s sub-map interiors;
// the raw pointers inside are owning edges, never aliased borrows.
// Not `Sync`: shared access goes through `sync::SyncExpanseStrMap`.
// (Before #363 this was derived from `ExpanseMap: Send`; the bare
// `MapCore` sub-trie field makes it explicit.)
unsafe impl Send for ExpanseStrMap {}

impl ExpanseStrMap {
    /// Creates an empty map.
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: None,
            pop: 0,
            alloc: NodeAlloc::new(),
            #[cfg(feature = "std")]
            deferred: OnceLock::new(),
        }
    }

    #[inline(always)]
    #[cfg(feature = "std")]
    fn defer_handle(&self) -> Option<Arc<Collector>> {
        self.deferred.get().cloned()
    }

    #[inline(always)]
    #[cfg(not(feature = "std"))]
    fn defer_handle(&self) -> Option<()> {
        None
    }

    /// Switches this map to deferred reclamation through `collector`,
    /// permanently (the Phase 7 `sync` wrapper calls this once at
    /// construction): every sub-trie created from here on is attached at
    /// creation, before it is published. Idempotent for the same
    /// collector; a second call with a different collector panics.
    ///
    /// Requires an **empty, never-populated** map: the shared allocator
    /// of a map that has held entries retains slab-carved node memory,
    /// which must never be retired to the collector —
    /// `NodeAlloc::defer_to` hard-asserts that no slab pages exist. The
    /// `sync` wrapper shares a populated map by rebuilding it through a
    /// fresh pre-deferred one.
    ///
    /// `pub(crate)` deliberately — only the `sync` wrapper drives a
    /// collector's epochs (see `BlobArena::defer_to` for the rationale).
    #[cfg(feature = "std")]
    pub(crate) fn defer_to(&self, collector: Arc<Collector>) {
        assert!(
            self.root.is_none(),
            "ExpanseStrMap::defer_to requires an empty map; rebuild a \
             populated map through a pre-deferred one instead"
        );
        // One call covers every sub-trie (#363 Step A): the sub-maps all
        // allocate through this shared handle.
        self.alloc.defer_to(Arc::clone(&collector));
        let stored = self.deferred.get_or_init(|| Arc::clone(&collector));
        assert!(
            Arc::ptr_eq(stored, &collector),
            "ExpanseStrMap already deferred to a different collector"
        );
    }

    /// Binds the wrapper's tree-level version word to the one allocator
    /// behind every sub-trie (#568 PR 3; see `NodeAlloc::bind_tree_word`).
    ///
    /// # Safety
    ///
    /// As `NodeAlloc::bind_tree_word`: `word` outlives every operation on
    /// this map.
    #[cfg(feature = "std")]
    pub(crate) unsafe fn bind_tree_word(&self, word: *const crate::occ::SeqVersion) {
        // SAFETY: forwarded contract.
        unsafe { self.alloc.bind_tree_word(word) };
    }

    /// The cover word of the meta-trie root node, or null when the map has
    /// no root (Refs #929).
    ///
    /// The first hop of every descent enters the root `StrNode`'s sub-map,
    /// so this is the word that would cover that hop's stores and the word
    /// a reader would sample before loading the root node's entry — the
    /// per-node counterpart of `NodeAlloc::tree_cover_addr`, which on this
    /// map names the one word shared by every sub-trie (see [`StrNode`]).
    ///
    /// Null is the map's genuine "no root state to cover" answer, not a
    /// failure: the root's own creation and removal (T11 and T10 of
    /// `docs/benchmarks/concurrency/METHODOLOGY.md` §17.2.1) are registered
    /// as staying behind the blocking fallback, where the tree-level word
    /// is what covers them.
    // As `StrNode::cover_addr`: the paths project the word from their raw
    // node pointer, so this serves the layout tests.
    #[cfg(test)]
    #[inline(always)]
    pub(crate) fn root_cover_addr(&self) -> *const u32 {
        self.root
            .as_deref()
            .map_or(core::ptr::null(), StrNode::cover_addr)
    }

    /// The **value** of the meta-trie root node's cover word, or `None`
    /// when the map has no root (Refs #929).
    ///
    /// The safe counterpart of [`Self::root_cover_addr`], for callers that
    /// want to observe the word rather than hand its address to the OCC
    /// protocol — which today means the tests that pin "nothing bumps it".
    /// Reading the value needs no raw pointer, so it stays out of
    /// `unsafe` entirely.
    // As its two siblings: the lib target has no caller until #929's write
    // path lands, and the tests that pin the word are `cfg(test)`.
    #[allow(dead_code)]
    #[cfg(feature = "std")]
    #[inline(always)]
    pub(crate) fn root_cover(&self) -> Option<u32> {
        self.root.as_deref().map(|r| r.cover)
    }

    /// Number of strings stored.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.pop
    }

    /// True when no strings are stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pop == 0
    }

    /// Heap bytes used by the map: the shared allocator's byte-exact
    /// `bytes_in_use` (every sub-map interior) plus a read-only walk over
    /// the node shells and suffix leaves.
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.alloc.bytes_in_use() + self.root.as_deref().map_or(0, |r| r.shell_bytes() as usize)
    }

    /// Heap bytes the map holds from the system allocator: [`Self::mem_used`]
    /// plus the freed blocks its allocator keeps on per-tree freelists for
    /// reuse and the unused part of the slab pages small nodes are carved
    /// from. Those blocks go back to the system only when the map is
    /// dropped or cleared, or on [`Self::shrink_to_fit`], so `malloc_trim`
    /// cannot recover them before then; this is the figure to
    /// compare with resident memory. The system allocator's own
    /// per-allocation overhead (chunk headers, size-class rounding) is
    /// allocator-specific and not included; the `allocator_overhead`
    /// example measures it.
    ///
    /// Computed on demand by walking the allocator's slab pages and
    /// freelists (O(pages + free blocks)); no counter is kept on the
    /// allocation path.
    #[must_use]
    pub fn mem_held(&self) -> usize {
        self.alloc.bytes_held() + self.root.as_deref().map_or(0, |r| r.shell_bytes() as usize)
    }

    /// Returns memory the map holds but does not use to the system
    /// allocator: freed blocks of the larger size classes, and slab pages
    /// with no live node on them. Returns the bytes released; afterwards
    /// [`Self::mem_held`] is lower by exactly that much and
    /// [`Self::mem_used`] is unchanged. Nothing moves, so no key, value or
    /// value pointer is affected.
    ///
    /// The map keeps freed blocks for reuse, so this pays off after a
    /// build or a burst of removals that leaves many blocks idle; it costs a
    /// walk of the allocator's pages and freelists. A no-op on a map
    /// shared through a concurrent wrapper, whose freed blocks go to its
    /// epoch collector: [`crate::sync::SyncExpanseStrMap::shrink_to_fit`] returns those.
    pub fn shrink_to_fit(&mut self) -> usize {
        self.alloc.release_free()
    }

    /// Builds, privately, the child node that replaces a suffix entry when
    /// a key diverges from it: the existing suffix's continuation, as a
    /// terminal entry or a shorter suffix. Not yet reachable by anyone.
    ///
    /// Reads `old` only through short-lived internal borrows, so the caller
    /// may dispose of it afterwards.
    fn build_split_child<const SHARED: bool>(
        old: *mut StrSuffix,
        alloc: &NodeAlloc,
    ) -> *mut StrNode {
        let mut child = Box::new(StrNode::new());
        // SAFETY: `old` is the live suffix being split, reached as a raw
        // pointer so the projection covers the inline bytes; both borrows
        // end before the caller disposes of it.
        let ((c1, t1), value) = unsafe { (chunk_at(suffix_bytes(old), 0), (*old).value) };
        if t1 {
            child
                .map
                .insert_pathless_dispatch::<SHARED, SHARED>(alloc, c1, value);
        } else {
            // The continuation bytes are copied into the new leaf here, so
            // the borrow of `old` is over before it is disposed of.
            // SAFETY: as above.
            let s1 =
                unsafe { new_suffix(&suffix_bytes(old)[CHUNK..], value, SuffixArena::of(alloc)) };
            child
                .map
                .insert_pathless_dispatch::<SHARED, SHARED>(alloc, c1, pack_suffix(s1));
        }
        Box::into_raw(child)
    }

    /// Splits a suffix entry that diverges from the key being inserted:
    /// builds a child node holding the existing suffix's continuation,
    /// publishes it over the suffix's map entry, and disposes of the old
    /// suffix (retired when shared — a concurrent reader may still hold
    /// it). Returns the raw child for the caller to descend into.
    fn split_suffix<const SHARED: bool>(
        node: &mut StrNode,
        chunk: u64,
        old: *mut StrSuffix,
        alloc: &NodeAlloc,
        defer: DeferHandle<'_>,
    ) -> *mut StrNode {
        let child_raw = Self::build_split_child::<SHARED>(old, alloc);
        resync::<SHARED>(node);
        covered::<SHARED, _>(node, alloc, |m| {
            m.insert_pathless_dispatch::<SHARED, SHARED>(alloc, chunk, pack_child(child_raw))
        });
        dispose_suffix(old, defer, SuffixArena::of(alloc));
        child_raw
    }

    /// Inserts `key → val`; returns the replaced value if present.
    pub fn insert(&mut self, key: &NulFreeStr, val: u64) -> Option<u64> {
        // The one branch the two twins share, on the state this path already
        // loaded: a map switched to deferred reclamation runs the shared
        // twin (Refs #929), every other map the plain one.
        #[cfg(feature = "std")]
        if let Some(c) = self.deferred.get().cloned() {
            return self.insert_impl::<true>(key, val, Some(&c));
        }
        self.insert_impl::<false>(key, val, None)
    }

    /// Single-threaded insert, bypassing deferred/OCC checks.
    #[doc(hidden)]
    #[inline(always)]
    pub fn insert_plain(&mut self, key: &NulFreeStr, val: u64) -> Option<u64> {
        self.insert_impl::<false>(key, val, None)
    }

    /// [`Self::insert`] for one sharing mode. `SHARED` is the deferred twin
    /// (Refs #929): every sub-map mutation runs inside its node's cover
    /// bracket and after a census re-sync; the unshared instantiation is
    /// the plain path with nothing added.
    fn insert_impl<const SHARED: bool>(
        &mut self,
        key: &NulFreeStr,
        val: u64,
        defer: DeferHandle<'_>,
    ) -> Option<u64> {
        let key = key.as_bytes();
        // Field-level borrows on purpose: `node` must borrow only
        // `self.root` so `self.pop` and `self.alloc` stay reachable in
        // the loop.
        if self.root.is_none() {
            self.root = Some(Box::new(StrNode::new()));
        }
        let alloc = &self.alloc;
        let mut node: &mut StrNode = self.root.as_deref_mut().expect("root just ensured");
        let mut off = 0;
        loop {
            let (chunk, terminal) = chunk_at(key, off);
            if terminal {
                resync::<SHARED>(node);
                let prev = covered::<SHARED, _>(node, alloc, |m| {
                    m.insert_pathless_dispatch::<SHARED, SHARED>(alloc, chunk, val)
                });
                if prev.is_none() {
                    self.pop += 1;
                }
                return prev;
            }
            match node.map.get(chunk) {
                None => {
                    let suffix = new_suffix(&key[off + CHUNK..], val, SuffixArena::of(alloc));
                    resync::<SHARED>(node);
                    covered::<SHARED, _>(node, alloc, |m| {
                        m.insert_pathless_dispatch::<SHARED, SHARED>(
                            alloc,
                            chunk,
                            pack_suffix(suffix),
                        )
                    });
                    self.pop += 1;
                    return None;
                }
                Some(v) if is_suffix_ptr(v) => {
                    let sfx = unpack_suffix(v);
                    let rem = &key[off + CHUNK..];
                    // SAFETY: tagged pointer encodes a live suffix leaf;
                    // short-lived shared borrow of the write-once bytes,
                    // taken from the raw pointer so it reaches past the
                    // header.
                    if rem == unsafe { suffix_bytes(sfx) } {
                        // In-place value update, field-precise (no `&mut`
                        // over the header whose write-once fields concurrent
                        // readers load): only the value word mutates, under
                        // this node's cover bracket when shared (T3).
                        return Some(covered::<SHARED, _>(node, alloc, |_| {
                            // SAFETY: exclusive writer; a racing reader's
                            // load is discarded unless its snapshot validates.
                            unsafe { core::ptr::replace(&raw mut (*sfx).value, val) }
                        }));
                    }
                    let child_raw = Self::split_suffix::<SHARED>(node, chunk, sfx, alloc, defer);
                    // SAFETY: freshly allocated Box<StrNode> above.
                    node = unsafe { &mut *child_raw };
                    off += CHUNK;
                }
                Some(v) => {
                    // SAFETY: v is an untagged child pointer to a live StrNode.
                    node = unsafe { &mut *unpack_child(v) };
                    off += CHUNK;
                }
            }
        }
    }

    /// Inserts `key` with value 0 if absent (existing value kept) and
    /// returns a writable pointer to its value slot — the compat
    /// `JudySLIns` contract. Valid until the next structural mutation.
    pub fn ins_slot(&mut self, key: &NulFreeStr) -> NonNull<u64> {
        // As `insert`: the deferred twin for a shared map, the plain path
        // otherwise (Refs #929).
        #[cfg(feature = "std")]
        if let Some(c) = self.deferred.get().cloned() {
            return self.ins_slot_impl::<true>(key, Some(&c));
        }
        self.ins_slot_impl::<false>(key, None)
    }

    /// Single-threaded insert-if-absent returning slot pointer, bypassing deferred/OCC checks.
    #[doc(hidden)]
    #[inline(always)]
    pub fn ins_slot_plain(&mut self, key: &NulFreeStr) -> NonNull<u64> {
        self.ins_slot_impl::<false>(key, None)
    }

    /// [`Self::ins_slot`] for one sharing mode; see [`Self::insert_impl`].
    fn ins_slot_impl<const SHARED: bool>(
        &mut self,
        key: &NulFreeStr,
        defer: DeferHandle<'_>,
    ) -> NonNull<u64> {
        let key = key.as_bytes();
        // Field-level borrows on purpose: `node` must borrow only
        // `self.root` so `self.pop` and `self.alloc` stay reachable in
        // the loop.
        if self.root.is_none() {
            self.root = Some(Box::new(StrNode::new()));
        }
        let alloc = &self.alloc;
        let mut node: &mut StrNode = self.root.as_deref_mut().expect("root just ensured");
        let mut off = 0;
        loop {
            let (chunk, terminal) = chunk_at(key, off);
            if terminal {
                // Increment B (#813): $O(1)$ len check before and after ins_slot_pathless
                // eliminates redundant contains_key lookup. The `v == 0` sentinel
                // MUST NOT be used here, as 0 is a valid terminal value.
                resync::<SHARED>(node);
                let len_before = node.map.len();
                let slot = covered::<SHARED, _>(node, alloc, |m| {
                    m.ins_slot_pathless_dispatch::<SHARED, SHARED>(alloc, chunk)
                });
                if node.map.len() > len_before {
                    self.pop += 1;
                }
                return slot;
            }
            match node.map.get(chunk) {
                None => {
                    let suffix = new_suffix(&key[off + CHUNK..], 0, SuffixArena::of(alloc));
                    resync::<SHARED>(node);
                    covered::<SHARED, _>(node, alloc, |m| {
                        m.insert_pathless_dispatch::<SHARED, SHARED>(
                            alloc,
                            chunk,
                            pack_suffix(suffix),
                        )
                    });
                    self.pop += 1;
                    // SAFETY: suffix is a live, uniquely owned pointer
                    // allocated above; `value` sits at offset 0.
                    return unsafe { NonNull::new_unchecked(&raw mut (*suffix).value) };
                }
                Some(v) if is_suffix_ptr(v) => {
                    let sfx = unpack_suffix(v);
                    let rem = &key[off + CHUNK..];
                    // SAFETY: tagged pointer encodes a live suffix leaf;
                    // short-lived shared borrow of the write-once bytes.
                    if rem == unsafe { suffix_bytes(sfx) } {
                        // SAFETY: field-precise pointer to the value word.
                        return NonNull::new(unsafe { &raw mut (*sfx).value })
                            .expect("non-null value slot");
                    }
                    // Divergence: publish a child over the suffix entry,
                    // then dispose of the old suffix (see `split_suffix`).
                    let child_raw = Self::split_suffix::<SHARED>(node, chunk, sfx, alloc, defer);
                    // SAFETY: freshly allocated Box<StrNode> above.
                    node = unsafe { &mut *child_raw };
                    off += CHUNK;
                }
                Some(v) => {
                    // SAFETY: v is an untagged child pointer to a live StrNode.
                    node = unsafe { &mut *unpack_child(v) };
                    off += CHUNK;
                }
            }
        }
    }

    /// Returns the value stored for `key`.
    #[must_use]
    pub fn get(&self, key: &NulFreeStr) -> Option<u64> {
        let key = key.as_bytes();
        let mut node = self.root.as_deref()?;
        let mut off = 0;
        loop {
            let (chunk, terminal) = chunk_at(key, off);
            if terminal {
                return node.map.get(chunk);
            }
            let v = node.map.get(chunk)?;
            if is_suffix_ptr(v) {
                let sfx = unpack_suffix(v);
                let rem = &key[off + CHUNK..];
                // SAFETY: tagged pointer encodes a live suffix leaf. Taken
                // from the raw pointer: the inline bytes sit past the header
                // and are outside a `&StrSuffix`'s provenance. This is the
                // dependent load the old two-allocation shape spent a second
                // pointer chase on (#723).
                if rem == unsafe { suffix_bytes(sfx) } {
                    // SAFETY: as above; `value` is at offset 0.
                    return Some(unsafe { (*sfx).value });
                }
                return None;
            }
            // SAFETY: untagged child pointer to a live StrNode.
            node = unsafe { &*unpack_child(v) };
            off += CHUNK;
        }
    }

    /// Phase 7 (issue #219 Phase 2): one bounded, validated optimistic
    /// lookup across the cascading sub-tries — the concurrent analogue of
    /// [`Self::get`].
    ///
    /// Each hop samples the `StrNode`'s cover word, walks that node's
    /// sub-map under it (`sync::walk_validated_node`, hand-over-hand
    /// into the sub-map's own branch words), and re-validates the cover
    /// after the entry it loaded — the word the string wrapper's writers
    /// bump for that node's root state, its continuation entries and its
    /// suffix values (Refs #929, `docs/benchmarks/concurrency/METHODOLOGY.md`
    /// §17.2.2). The terminal value is covered hand-over-hand by per-node
    /// versions, exactly [`crate::sync::SyncExpanseMap`]'s read semantics:
    /// the result is a value the key held during the call, linearizable
    /// because sub-tries are never re-parented and unlink always precedes
    /// retirement, behind an obsolete mark. The tree word is validated once
    /// before an answer is returned: it is what the exclusive path holds
    /// across the meta-trie root's own creation and removal and across
    /// `clear`, which carry no per-node word. A hop that races a writer
    /// fails validation and surfaces as `Retry`. Suffix leaves are
    /// write-once after publication (a split publishes a replacement and
    /// retires the old suffix); only the value word mutates in place, under
    /// the cover of the node whose entry points at it.
    ///
    /// # Safety
    ///
    /// Same contract as `sync::walk_validated`: `snap` must be an even
    /// version sampled from `ver` after this map switched to deferred
    /// reclamation ([`Self::defer_to`]), and the caller must hold an epoch
    /// pin for the whole call — every pointer read under a still-valid
    /// cover then references EBR-live memory. `root` is the meta-trie root
    /// the wrapper published ([`Self::root_word`], `None` when empty), loaded
    /// after `snap`: the reader does not touch the map, which a covered
    /// writer may hold `&mut` to meanwhile (#1086).
    #[cfg(feature = "std")]
    pub(crate) unsafe fn get_validated(
        root: Option<NonNull<u8>>,
        key: &NulFreeStr,
        ver: &crate::occ::SeqVersion,
        snap: u64,
    ) -> Result<Option<u64>, crate::sync::Retry> {
        use crate::occ::{node_sample, node_validate, version_cell};
        use crate::sync::{Retry, walk_validated_node};
        let key = key.as_bytes();
        // Racy single-word copy of the root pointer, through the box rather
        // than a shared borrow (`root_raw`): the root node's cover is sampled
        // before anything is read through it, an unlinked root is
        // obsolete-marked and EBR-live, and the tree word is validated before
        // any answer.
        let Some(root) = root.map(|p| p.as_ptr().cast::<StrNode>()) else {
            return if ver.validate(snap) {
                Ok(None)
            } else {
                Err(Retry)
            };
        };
        let mut node: *const StrNode = root.cast_const();
        let mut off = 0usize;
        loop {
            let (chunk, terminal) = chunk_at(key, off);
            // SAFETY: `node` is the root (see above) or a child loaded under
            // a still-valid cover, and is EBR-live under the caller's pin;
            // the word is projected from the raw pointer, never through a
            // reference to the node.
            let word: *const u32 = unsafe { &raw const (*node).cover };
            // SAFETY: as above.
            let cell = unsafe { version_cell(word) };
            let Some(csnap) = node_sample(cell) else {
                return Err(Retry);
            };
            // SAFETY: as above; a by-value copy, validated by the walk's
            // first check against the cover sampled just above.
            let msnap = unsafe { MapCore::occ_snapshot_of(&raw const (*node).map) };
            // SAFETY: the caller's pin + snapshot contract carries through.
            let found = unsafe { walk_validated_node::<true>(word, csnap, msnap, chunk) }?;
            if terminal {
                return if ver.validate(snap) {
                    Ok(found)
                } else {
                    Err(Retry)
                };
            }
            let Some(v) = found else {
                return if ver.validate(snap) {
                    Ok(None)
                } else {
                    Err(Retry)
                };
            };
            if is_suffix_ptr(v) {
                let sfx: *const StrSuffix = unpack_suffix(v);
                // SAFETY: `v` was validated under this node's cover (or the
                // sub-map branch holding it), so `sfx` was the published
                // suffix then, and EBR keeps the whole block — header and
                // inline bytes, one allocation — mapped under the pin. `len`
                // and the bytes are write-once; the value word may race a T3
                // and is validated below before use. Both reads project
                // from the raw pointer: a `&StrSuffix` would not carry
                // provenance over the bytes past the header.
                let (bytes, value) = unsafe { (suffix_bytes(sfx), (*sfx).value) };
                let matched = bytes == &key[off + CHUNK..];
                if !node_validate(cell, csnap) || !ver.validate(snap) {
                    return Err(Retry);
                }
                return Ok(matched.then_some(value));
            }
            // The cross-hop check (§17.2.2): the entry that named the child
            // still stands under the word its writers bump.
            if !node_validate(cell, csnap) {
                return Err(Retry);
            }
            node = unpack_child(v);
            off += CHUNK;
        }
    }

    /// Returns a writable pointer to `key`'s value slot (compat:
    /// `JudySLGet`), or `None` if absent.
    #[must_use]
    pub fn get_value_slot(&mut self, key: &NulFreeStr) -> Option<NonNull<u64>> {
        self.get_slot_ptr(key)
    }

    /// Returns a writable pointer to `key`'s value slot (compat:
    /// `JudySLGet`), or `None` if absent. Takes shared `&self`.
    #[must_use]
    pub fn get_slot_ptr(&self, key: &NulFreeStr) -> Option<NonNull<u64>> {
        let key = key.as_bytes();
        let mut node = self.root.as_deref()?;
        let mut off = 0;
        loop {
            let (chunk, terminal) = chunk_at(key, off);
            if terminal {
                return node.map.get_slot_ptr(chunk);
            }
            let v = node.map.get(chunk)?;
            if is_suffix_ptr(v) {
                let sfx = unpack_suffix(v);
                let rem = &key[off + CHUNK..];
                // SAFETY: tagged pointer encodes a live suffix leaf.
                if rem == unsafe { suffix_bytes(sfx) } {
                    // SAFETY: field-precise pointer to the value word at
                    // offset 0 — the JudySL `*mut Word` the C ABI hands out.
                    return Some(
                        NonNull::new(unsafe { &raw mut (*sfx).value })
                            .expect("non-null value slot"),
                    );
                }
                return None;
            }
            // SAFETY: continuation values are child pointers.
            node = unsafe { StrNode::child(v) };
            off += CHUNK;
        }
    }

    /// Returns `true` if `key` is present in the map.
    #[inline(always)]
    #[must_use]
    pub fn contains_key(&self, key: &NulFreeStr) -> bool {
        self.get(key).is_some()
    }

    /// Removes `key`; returns its value if it was present.
    ///
    /// The blocks a removal frees stay with the map for reuse, including
    /// after the last key is removed; call [`Self::shrink_to_fit`] (or
    /// [`Self::clear`]) to return them to the system allocator after a
    /// drain.
    pub fn remove(&mut self, key: &NulFreeStr) -> Option<u64> {
        // As `insert`: the deferred twin for a shared map, the plain path
        // otherwise (Refs #929).
        #[cfg(feature = "std")]
        if let Some(c) = self.deferred.get().cloned() {
            return self.remove_impl::<true>(key, Some(&c));
        }
        self.remove_impl::<false>(key, None)
    }

    /// Single-threaded remove, bypassing deferred/OCC checks.
    #[doc(hidden)]
    #[inline(always)]
    pub fn remove_plain(&mut self, key: &NulFreeStr) -> Option<u64> {
        self.remove_impl::<false>(key, None)
    }

    /// [`Self::remove`] for one sharing mode; see [`Self::insert_impl`].
    fn remove_impl<const SHARED: bool>(
        &mut self,
        key: &NulFreeStr,
        defer: DeferHandle<'_>,
    ) -> Option<u64> {
        let key = key.as_bytes();
        let alloc = &self.alloc;
        let root = self.root.as_deref_mut()?;
        let removed = root.remove_impl::<SHARED>(key, 0, alloc, defer)?;
        self.pop -= 1;
        if root.map.is_empty() {
            let root_box = self.root.take().expect("root present");
            // Unlinked (the root slot is cleared); retired when shared.
            dispose_node(Box::into_raw(root_box), alloc, defer);
        }
        Some(removed)
    }

    /// An ordered cursor over the whole map, from the smallest key.
    ///
    /// One descent for the walk, and **no allocation per element** — unlike
    /// the `next_at_or_after` / `next_after` pair below, where each step is a
    /// fresh root descent returning a freshly allocated key (#722). Prefer
    /// this for scans; the positional surface stays for the JudySL contract
    /// and for callers that genuinely jump around.
    #[must_use]
    pub fn cursor(&mut self) -> StrCursor<'_> {
        StrCursor {
            walk: Walk::new(self),
        }
    }

    /// An ordered cursor positioned at the first key `>= key`, inclusive.
    ///
    /// The seek is the one descent a bounded range scan needs; every
    /// subsequent element is a step along the path it recorded.
    #[must_use]
    pub fn cursor_at_or_after(&mut self, key: &NulFreeStr) -> StrCursor<'_> {
        StrCursor {
            walk: self.walk_at_or_after(key.as_bytes()),
        }
    }

    /// The seek behind [`cursor_at_or_after`](Self::cursor_at_or_after) and
    /// [`cursor_prefix`](Self::cursor_prefix).
    fn walk_at_or_after<const BOUNDED: bool>(&mut self, key: &[u8]) -> Walk<'_, BOUNDED> {
        let mut c = Walk::new(self);
        // A seek records one frame per chunk of `key` and the walk rarely
        // goes more than a level past it, so sizing the buffers from the
        // target replaces their doubling growth with one allocation each.
        c.key.reserve(key.len() + 2 * CHUNK);
        c.stack.reserve(key.len() / CHUNK + 2);
        c.subs.reserve(1);
        c.pending = c.seek(key);
        c
    }

    /// An ordered cursor over exactly the keys that start with `prefix`, in
    /// byte-lexicographic order.
    ///
    /// One seek, as [`cursor_at_or_after`](Self::cursor_at_or_after) does, and
    /// then the walk ends at the first key past the prefix by itself: a step
    /// that advances a level above the prefix's last chunk ends it, and a step
    /// at that chunk's level compares that one chunk under a mask. No key is
    /// compared with the prefix byte by byte, and the key past the prefix is
    /// never built.
    #[must_use]
    pub fn cursor_prefix(&mut self, prefix: &NulFreeStr) -> StrPrefixCursor<'_> {
        let p = prefix.as_bytes();
        let mut c = self.walk_at_or_after::<true>(p);
        let rem = p.len() % CHUNK;
        c.bound_depth = p.len() / CHUNK;
        if rem > 0 {
            c.bound_mask = !0u64 << (64 - 8 * rem);
            c.bound_want = chunk_at(p, p.len() - rem).0 & c.bound_mask;
        }
        // The entry the seek positioned on can come from a level above the
        // bound — a suffix leaf holds the whole tail of a key — so it gets the
        // one full comparison.
        if c.pending.is_some() && !c.key.starts_with(p) {
            c.pending = None;
            c.done = true;
        }
        StrPrefixCursor { walk: c }
    }

    /// Smallest entry with key `>= key`: `(key bytes, value slot)`
    /// (compat: `JudySLFirst`).
    pub fn next_at_or_after(&self, key: &NulFreeStr) -> Option<(Vec<u8>, NonNull<u64>)> {
        let key = key.as_bytes();
        let root = self.root.as_deref()?;
        let mut out = Vec::with_capacity(key.len() + CHUNK);
        let slot = root.next_at_or_after(key, 0, &mut out)?;
        Some((out, slot))
    }

    /// Smallest entry with key `> key` (compat: `JudySLNext`). The
    /// immediate successor of a NUL-free string is itself + `0x01`.
    pub fn next_after(&self, key: &NulFreeStr) -> Option<(Vec<u8>, NonNull<u64>)> {
        let mut succ = Vec::with_capacity(key.len() + 1);
        succ.extend_from_slice(key.as_bytes());
        succ.push(1);
        // SAFETY: `key` is NUL-free and the appended sentinel is `1`, so the
        // successor is too. `1` rather than `0` precisely because `0` would
        // leave the domain.
        self.next_at_or_after(unsafe { NulFreeStr::new_unchecked(&succ) })
    }

    /// Largest entry with key `<= key` (compat: `JudySLLast`).
    pub fn prev_at_or_before(&self, key: &NulFreeStr) -> Option<(Vec<u8>, NonNull<u64>)> {
        let key = key.as_bytes();
        let root = self.root.as_deref()?;
        let mut out = Vec::with_capacity(key.len() + CHUNK);
        let slot = root.prev_at_or_before(key, 0, false, &mut out)?;
        Some((out, slot))
    }

    /// Largest entry with key `< key` (compat: `JudySLPrev`).
    pub fn prev_before(&self, key: &NulFreeStr) -> Option<(Vec<u8>, NonNull<u64>)> {
        let key = key.as_bytes();
        let root = self.root.as_deref()?;
        let mut out = Vec::with_capacity(key.len() + CHUNK);
        let slot = root.prev_at_or_before(key, 0, true, &mut out)?;
        Some((out, slot))
    }

    /// Smallest entry.
    pub fn first(&self) -> Option<(Vec<u8>, NonNull<u64>)> {
        // SAFETY: the empty key contains no NUL.
        self.next_at_or_after(unsafe { NulFreeStr::new_unchecked(&[]) })
    }

    /// Largest entry.
    pub fn last(&self) -> Option<(Vec<u8>, NonNull<u64>)> {
        let root = self.root.as_deref()?;
        let mut out = Vec::new();
        let slot = root.max_entry(&mut out);
        Some((out, slot))
    }

    /// Removes every entry; returns the heap bytes released (the compat
    /// `JudySLFreeArray` return value). The allocator's retained blocks go
    /// back to the system allocator too, however few; the return value
    /// counts only the entries' bytes.
    pub fn clear(&mut self) -> u64 {
        let bytes = self.clear_entries();
        // As an emptying `remove`; a no-op on a shared map.
        self.alloc.release_free();
        bytes
    }

    /// [`Self::clear`] without the release: frees every entry and leaves
    /// the allocator's freed blocks in place. For `Drop` and the C ABI's
    /// `JudySLFreeArray`, which drop the allocator straight after, so a
    /// release there would only walk the blocks its own `Drop` frees.
    fn clear_entries(&mut self) -> u64 {
        let bytes = match self.root.take() {
            Some(root) => {
                // Count first — the shared allocator's byte-exact
                // `bytes_in_use` (all sub-map interiors, all of which are
                // about to be freed) plus a read-only walk over shells and
                // suffixes — then dispose of the subtree (retired through
                // the collector when shared).
                let bytes = self.alloc.bytes_in_use() as u64 + root.shell_bytes();
                dispose_tree(
                    Box::into_raw(root),
                    &self.alloc,
                    self.defer_handle().as_ref(),
                );
                debug_assert_eq!(self.alloc.bytes_in_use(), 0);
                bytes
            }
            None => 0,
        };
        self.pop = 0;
        bytes
    }

    /// Single-threaded clear, bypassing deferred/OCC checks. Unlike
    /// [`Self::clear`] it keeps the allocator's freed blocks: the C ABI's
    /// `JudySLFreeArray` drops the map straight after, which returns them
    /// anyway.
    #[doc(hidden)]
    #[inline(always)]
    pub fn clear_plain(&mut self) -> u64 {
        self.clear_entries()
    }
}

// ---------------------------------------------------------------------------
// The deferred twin's helpers (Refs #929)
// ---------------------------------------------------------------------------

/// Runs `f`, a mutation of `node`'s sub-map, inside `node`'s cover bracket
/// when `SHARED` — the deferred twin, where concurrent readers validate that
/// word — and as a plain call otherwise, so the unshared path pays nothing.
#[inline(always)]
fn covered<const SHARED: bool, R>(
    node: &mut StrNode,
    alloc: &NodeAlloc,
    f: impl FnOnce(&mut MapCore) -> R,
) -> R {
    #[cfg(feature = "std")]
    {
        if SHARED {
            // Through `node`'s own borrow, so the raw word is a child of the
            // unique reference and the reborrow of `map` below stays valid.
            let v: *mut u32 = &raw mut node.cover;
            // SAFETY: a live node this writer is exclusive on (the wrapper
            // serialised or quiesced every other writer), with no reference
            // to `cover` live; the bracket opened here closes below.
            unsafe { crate::occ::version_begin_if_ptr::<true>(alloc, v) };
            let r = f(&mut node.map);
            // SAFETY: as above.
            unsafe { crate::occ::version_end_if_ptr::<true>(alloc, v) };
            return r;
        }
    }
    let _ = alloc;
    f(&mut node.map)
}

/// [`StrNode::resync_if_dirty`] in the deferred twin; nothing otherwise.
#[inline(always)]
fn resync<const SHARED: bool>(node: &mut StrNode) {
    #[cfg(feature = "std")]
    if SHARED {
        node.resync_if_dirty();
    }
    let _ = node;
}

// ---------------------------------------------------------------------------
// The optimistic multi-writer path (Refs #929, METHODOLOGY §17.2)
// ---------------------------------------------------------------------------

/// Everything only the optimistic write path uses. Absent under
/// `ablation-str-serial-writers`, which serialises every mutation on the
/// writer mutex through the deferred twin above and never reaches any of
/// it.
#[cfg(all(feature = "std", not(feature = "ablation-str-serial-writers")))]
mod olc {
    use super::*;
    use crate::node::Edge;
    /// A `StrNode`'s sub-map as the engine's OLC bodies see it: its cover word
    /// stands where the tree word stands for the map wrapper, its top edge is
    /// the sub-map's, the allocator is the map's one shared allocator, and a
    /// mutation marks the node dirty instead of a digit.
    ///
    /// `locked` says the caller holds the cover as a lock (T4, T8, and a
    /// prune's parent): the word is odd by the caller's own doing and the
    /// root state is stable under it, so the parity check that stands in
    /// for the map wrapper's tree-word sample is answered `true` rather than
    /// sampled. Sampling it was the defect #1001's first Callgrind run
    /// found: every split, suffix removal and prune retried 64 times against
    /// its own lock and fell back.
    struct StrHost<'a> {
        node: *mut StrNode,
        alloc: &'a NodeAlloc,
        locked: bool,
    }

    impl crate::sync::OlcHost for StrHost<'_> {
        #[inline(always)]
        fn tree_word_even(&self) -> bool {
            if self.locked {
                return true;
            }
            // SAFETY: a live node under the caller's pin; the word is projected
            // from the raw pointer.
            crate::occ::node_sample(unsafe {
                crate::occ::version_cell(&raw const (*self.node).cover)
            })
            .is_some()
        }

        #[inline(always)]
        unsafe fn top_ptr(&self) -> *mut Edge {
            // SAFETY: live node; the top edge is read by value through validated
            // loads only, and an optimistic writer never stores to it (a
            // root-state change is a `RootGrowth` fallback).
            unsafe { MapCore::root_top_ptr_of(&raw mut (*self.node).map) }
        }

        #[inline(always)]
        fn alloc(&self) -> &NodeAlloc {
            self.alloc
        }

        #[inline(always)]
        fn mark_dirty_digit(&self, _d: u8) {
            // SAFETY: live node.
            let cell = unsafe { StrNode::dirty_cell(self.node) };
            // A load first: every writer after the first only reads the line.
            if cell.load(Ordering::Relaxed) == 0 {
                cell.store(1, Ordering::Relaxed);
            }
        }
    }

    /// A `StrNode`'s cover word held as a lock by an optimistic string writer:
    /// the RAII form of `occ::version_try_lock_expect`. Drops as
    /// `occ::NodeLock` does: unlocked with the version advanced unless
    /// `abort_unmodified` was called, and poisoned obsolete when dropped while
    /// panicking. It keeps off the debug bracket stack: a prune releases a
    /// child's lock while its parent's is held, so the lock order is not LIFO;
    /// [`under_lock`] puts the word on the stack for exactly one plain sub-map
    /// call instead.
    struct CoverLock<'a> {
        cell: &'a crate::occ::VersionCell,
        old_v: u32,
        modified: bool,
    }

    impl<'a> CoverLock<'a> {
        /// Locks `node`'s cover if it still reads `expected`
        /// (`lockVersionOrRestart`): the entries the caller read under that
        /// snapshot are then still what they were.
        ///
        /// # Safety
        ///
        /// `node` is live under the caller's pin.
        #[inline(always)]
        unsafe fn try_lock_expect(node: *mut StrNode, expected: u32) -> Option<Self> {
            // SAFETY: forwarded contract; the word is projected from the raw
            // node pointer.
            let cell = unsafe { crate::occ::version_cell(&raw const (*node).cover) };
            let old_v = crate::occ::version_try_lock_expect(cell, expected).ok()?;
            Some(Self {
                cell,
                old_v,
                modified: true,
            })
        }

        /// Locks `node`'s cover whatever it reads, for a step that needs
        /// exclusion but read nothing under a snapshot (a prune's parent).
        ///
        /// # Safety
        ///
        /// As [`Self::try_lock_expect`].
        #[inline(always)]
        unsafe fn try_lock(node: *mut StrNode) -> Option<Self> {
            // SAFETY: forwarded contract.
            let cell = unsafe { crate::occ::version_cell(&raw const (*node).cover) };
            let old_v = crate::occ::version_try_lock(cell).ok()?;
            Some(Self {
                cell,
                old_v,
                modified: true,
            })
        }

        /// Nothing under this word changed: restore the version on release so
        /// readers that sampled it need not restart.
        #[inline(always)]
        fn abort_unmodified(&mut self) {
            self.modified = false;
        }

        /// The node is being unlinked and retired: leave the word permanently
        /// odd (property S3) instead of unlocking it.
        #[inline(always)]
        fn mark_obsolete(self) {
            crate::occ::version_obsolete_locked(self.cell);
            core::mem::forget(self);
        }
    }

    impl Drop for CoverLock<'_> {
        #[inline]
        fn drop(&mut self) {
            if std::thread::panicking() {
                crate::occ::version_obsolete_locked(self.cell);
            } else {
                crate::occ::version_unlock(self.cell, self.old_v, self.modified);
            }
        }
    }

    /// Runs one plain sub-map call while `node`'s cover is held as a lock. In
    /// debug builds the word is on the bracket stack for exactly the call, so
    /// the engine's `assert_bracketed` sees an open bracket and the stack stays
    /// LIFO whatever order the locks themselves are released in.
    ///
    /// # Safety
    ///
    /// `node` is live.
    #[inline(always)]
    unsafe fn under_lock<R>(node: *mut StrNode, alloc: &NodeAlloc, f: impl FnOnce() -> R) -> R {
        // SAFETY: forwarded contract; a read-only projection of the word.
        let word: *const u32 = unsafe { &raw const (*node).cover };
        #[cfg(debug_assertions)]
        alloc.bracket_enter(word);
        let r = f();
        #[cfg(debug_assertions)]
        alloc.bracket_leave(word);
        let _ = (word, alloc);
        r
    }

    /// Frees a suffix leaf that was allocated and never published: no reader
    /// can hold it, so it needs no retirement.
    fn free_unpublished_suffix(ptr: *mut StrSuffix, arena: SuffixArena<'_>) {
        // SAFETY: allocated by `new_suffix` on this thread and never stored
        // anywhere; `len` describes the allocation.
        let layout = suffix_layout(unsafe { (*ptr).len });
        // With `packed-suffix` the block is the allocator's: it goes back
        // through the unpublished path, which recycles it at once without a
        // grace period — no reader ever saw it.
        #[cfg(feature = "packed-suffix")]
        // SAFETY: `ptr` came from `alloc_bytes(packed_suffix_bytes(layout))` on this
        // allocator in `new_suffix` and was never published.
        unsafe {
            arena.alloc.free_bytes_unpublished(
                NonNull::new(ptr.cast::<u8>()).expect("non-null suffix"),
                packed_suffix_bytes(layout),
            );
        }
        #[cfg(not(feature = "packed-suffix"))]
        {
            let _ = arena;
            // SAFETY: as above, and `layout` is the one the block was
            // allocated with.
            unsafe { core_alloc::alloc::dealloc(ptr.cast::<u8>(), layout) };
        }
    }

    /// The chunk-chain frames of an optimistic remove, for the prune that may
    /// follow it: inline for keys up to eight chunks, spilling to the heap
    /// beyond — so the common remove allocates nothing for its path.
    struct PathStack {
        head: [(*mut StrNode, u64); PATH_INLINE],
        len: usize,
        spill: Vec<(*mut StrNode, u64)>,
    }

    const PATH_INLINE: usize = 8;

    impl PathStack {
        fn new() -> Self {
            Self {
                head: [(core::ptr::null_mut(), 0); PATH_INLINE],
                len: 0,
                spill: Vec::new(),
            }
        }

        #[inline(always)]
        fn push(&mut self, frame: (*mut StrNode, u64)) {
            if self.len < PATH_INLINE {
                self.head[self.len] = frame;
                self.len += 1;
            } else {
                self.spill.push(frame);
            }
        }

        #[inline(always)]
        fn pop(&mut self) -> Option<(*mut StrNode, u64)> {
            if let Some(frame) = self.spill.pop() {
                return Some(frame);
            }
            if self.len == 0 {
                return None;
            }
            self.len -= 1;
            Some(self.head[self.len])
        }
    }

    impl ExpanseStrMap {
        /// The optimistic insert of `sync::SyncExpanseStrMap` (Refs #929,
        /// METHODOLOGY §17.2): one cover per hop, hand-over-hand down the chunk
        /// chain.
        ///
        /// At each `StrNode` the writer samples the cover, copies the sub-map
        /// root state and looks the chunk up under that cover
        /// (`walk_validated_node`). What it then does depends on the sub-map's
        /// state and the transition:
        ///
        /// - a sub-map in **tree** state is mutated through the engine's own
        ///   OLC body (`olc_insert_map`), under the engine's per-node locks —
        ///   T1, and T2 with the insert-if-absent mode so a suffix another
        ///   writer published first is found rather than clobbered;
        /// - a sub-map in **leaf or empty** state is mutated under the cover
        ///   taken as a lock at the snapshot the lookup ran under: its root
        ///   state has no other word;
        /// - a **continuation entry** — T3's value replace, T4's split — is
        ///   changed under the cover lock too, whatever the sub-map's state,
        ///   because the lock is what serialises the writers of one entry and
        ///   what readers of the suffix validate; the engine's own lock covers
        ///   the entry's store into a tree sub-map underneath it.
        ///
        /// The meta-trie root's creation (T11) is a `RootGrowth` fallback, and
        /// whatever the engine's body falls back on inside a sub-map is
        /// forwarded. A cover that moved or is held returns `Retry`.
        ///
        /// # Safety
        ///
        /// The caller entered the writer gate (`Shared::enter_writer_blocking`)
        /// and holds an epoch pin for the whole call, on a map that
        /// [`Self::defer_to`] switched to deferred reclamation; every exclusive
        /// operation is therefore excluded, and every pointer loaded under a
        /// still-valid cover references EBR-live memory.
        pub(crate) unsafe fn olc_insert(
            &self,
            key: &NulFreeStr,
            val: u64,
        ) -> crate::sync::OlcOutcome<Option<u64>> {
            use crate::occ::{node_sample, node_validate, version_cell};
            use crate::sync::{FallbackCause, OlcOutcome, olc_insert_map, walk_validated_node};
            let key = key.as_bytes();
            let alloc = &self.alloc;
            let defer = self.deferred.get();
            debug_assert!(
                defer.is_some(),
                "optimistic insert on a map that was never deferred"
            );
            let Some(mut node) = self.root_raw() else {
                return OlcOutcome::Fallback(FallbackCause::RootGrowth);
            };
            let mut off = 0usize;
            loop {
                let (chunk, terminal) = chunk_at(key, off);
                // SAFETY: `node` is the root (unlinked only under the tree word,
                // which the gate excludes) or a child loaded under a still-valid
                // cover, and is EBR-live under the caller's pin; the word is
                // projected from the raw pointer.
                let word: *const u32 = unsafe { &raw const (*node).cover };
                // SAFETY: as above.
                let cell = unsafe { version_cell(word) };
                let Some(csnap) = node_sample(cell) else {
                    return OlcOutcome::Retry;
                };
                // SAFETY: as above; a by-value copy validated before use.
                let msnap = unsafe { MapCore::occ_snapshot_of(&raw const (*node).map) };
                let is_tree = matches!(msnap, crate::sync::RootSnapshot::Tree { .. });
                let host = StrHost {
                    node,
                    alloc,
                    locked: false,
                };
                if terminal {
                    if is_tree {
                        // T1 in tree state: the engine's per-node locks cover it.
                        return olc_insert_map::<_, false>(&host, chunk, val);
                    }
                    // T1 in leaf or empty state: the root state is this node's.
                    // SAFETY: live node (above).
                    let Some(lock) = (unsafe { CoverLock::try_lock_expect(node, csnap) }) else {
                        return OlcOutcome::Retry;
                    };
                    // SAFETY: the cover lock excludes every other writer of this
                    // node's root state, and readers validate the word.
                    let prev = unsafe {
                        under_lock(node, alloc, || {
                            MapCore::insert_leaf_state_at(&raw mut (*node).map, alloc, chunk, val)
                        })
                    };
                    drop(lock);
                    return OlcOutcome::Done(prev);
                }
                // SAFETY: pinned, and the cover was sampled even just above.
                let found = match unsafe { walk_validated_node::<true>(word, csnap, msnap, chunk) }
                {
                    Ok(found) => found,
                    Err(_) => return OlcOutcome::Retry,
                };
                let v = match found {
                    Some(v) => v,
                    None => {
                        // T2: publish a suffix leaf holding the key's remainder.
                        let sfx = new_suffix(&key[off + CHUNK..], val, SuffixArena::of(alloc));
                        let w = pack_suffix(sfx);
                        if is_tree {
                            match olc_insert_map::<_, true>(&host, chunk, w) {
                                OlcOutcome::Done(None) => return OlcOutcome::Done(None),
                                OlcOutcome::Done(Some(existing)) => {
                                    // Another writer published this chunk first:
                                    // ours was never reachable.
                                    free_unpublished_suffix(sfx, SuffixArena::of(alloc));
                                    existing
                                }
                                other => {
                                    free_unpublished_suffix(sfx, SuffixArena::of(alloc));
                                    return other;
                                }
                            }
                        } else {
                            // SAFETY: live node (above).
                            let Some(lock) = (unsafe { CoverLock::try_lock_expect(node, csnap) })
                            else {
                                free_unpublished_suffix(sfx, SuffixArena::of(alloc));
                                return OlcOutcome::Retry;
                            };
                            // SAFETY: as for T1 in leaf state.
                            let prev = unsafe {
                                under_lock(node, alloc, || {
                                    MapCore::insert_leaf_state_at(
                                        &raw mut (*node).map,
                                        alloc,
                                        chunk,
                                        w,
                                    )
                                })
                            };
                            debug_assert!(
                                prev.is_none(),
                                "the cover was locked at the snapshot the lookup ran under"
                            );
                            drop(lock);
                            return OlcOutcome::Done(None);
                        }
                    }
                };
                if is_suffix_ptr(v) {
                    let sfx = unpack_suffix(v);
                    let rem = &key[off + CHUNK..];
                    // SAFETY: `v` was validated under this node's cover (or the
                    // sub-map branch holding it) and EBR keeps the block mapped
                    // under the pin; the bytes are write-once.
                    let same = unsafe { suffix_bytes(sfx) } == rem;
                    // SAFETY: live node (above).
                    let Some(mut lock) = (unsafe { CoverLock::try_lock_expect(node, csnap) })
                    else {
                        return OlcOutcome::Retry;
                    };
                    // Under the lock `chunk → v` is stable: a value replace, a
                    // split, a suffix removal and a prune of this node's children
                    // all take this lock, and the engine's inserts only add
                    // entries.
                    if same {
                        // T3: the one in-place store into a published suffix,
                        // covered by this word, which readers of the value
                        // validate.
                        // SAFETY: field-precise store; the lock excludes every
                        // other writer of it, and a reader's load is discarded
                        // unless the cover validates.
                        let old = unsafe { core::ptr::replace(&raw mut (*sfx).value, val) };
                        drop(lock);
                        return OlcOutcome::Done(Some(old));
                    }
                    // T4: build the child privately, publish it over the entry,
                    // retire the suffix it replaces.
                    // SAFETY: live node, locked.
                    let child_raw = unsafe {
                        under_lock(node, alloc, || Self::build_split_child::<true>(sfx, alloc))
                    };
                    let cw = pack_child(child_raw);
                    if is_tree {
                        let held = StrHost {
                            node,
                            alloc,
                            locked: true,
                        };
                        match olc_insert_map::<_, false>(&held, chunk, cw) {
                            OlcOutcome::Done(old_w) => {
                                debug_assert_eq!(
                                    old_w,
                                    Some(v),
                                    "the entry moved under the cover lock"
                                );
                            }
                            other => {
                                // Never published: nothing can hold it.
                                dispose_tree(child_raw, alloc, None);
                                lock.abort_unmodified();
                                drop(lock);
                                return other;
                            }
                        }
                    } else {
                        // SAFETY: as for T1 in leaf state.
                        let old_w = unsafe {
                            under_lock(node, alloc, || {
                                MapCore::insert_leaf_state_at(
                                    &raw mut (*node).map,
                                    alloc,
                                    chunk,
                                    cw,
                                )
                            })
                        };
                        debug_assert_eq!(old_w, Some(v), "the entry moved under the cover lock");
                    }
                    // Unlinked above; retired, since a reader may still hold it.
                    dispose_suffix(sfx, defer, SuffixArena::of(alloc));
                    drop(lock);
                    node = child_raw;
                    off += CHUNK;
                    continue;
                }
                // T5: the entry names a child. The cross-hop check (§17.2.2):
                // the entry still stands under the word its writers bump.
                if !node_validate(cell, csnap) {
                    return OlcOutcome::Retry;
                }
                node = unpack_child(v);
                off += CHUNK;
            }
        }

        /// The optimistic remove of `sync::SyncExpanseStrMap` (Refs #929); see
        /// [`Self::olc_insert`] for the protocol. T7 and T8 mirror T1 and T2.
        ///
        /// Returns the outcome and, when the removal emptied a node that could
        /// not be pruned optimistically, the cause: the removal itself is done
        /// and counted, and the caller prunes the key's path exclusively
        /// ([`Self::prune_empty_path`]). A prune is attempted under the emptied
        /// node's lock and its parent's (T9); the meta-trie root's removal
        /// (T10) is not attempted and reports `RootGrowth`.
        ///
        /// # Safety
        ///
        /// As [`Self::olc_insert`].
        pub(crate) unsafe fn olc_remove(
            &self,
            key: &NulFreeStr,
        ) -> (
            crate::sync::OlcOutcome<Option<u64>>,
            Option<crate::sync::FallbackCause>,
        ) {
            use crate::occ::{node_sample, node_validate, version_cell};
            use crate::sync::{OlcOutcome, olc_remove_map, walk_validated_node};
            let key = key.as_bytes();
            let alloc = &self.alloc;
            let defer = self.deferred.get();
            debug_assert!(
                defer.is_some(),
                "optimistic remove on a map that was never deferred"
            );
            let Some(mut node) = self.root_raw() else {
                return (OlcOutcome::Done(None), None);
            };
            let mut path = PathStack::new();
            let mut off = 0usize;
            loop {
                let (chunk, terminal) = chunk_at(key, off);
                // SAFETY: as in `olc_insert`.
                let word: *const u32 = unsafe { &raw const (*node).cover };
                // SAFETY: as above.
                let cell = unsafe { version_cell(word) };
                let Some(csnap) = node_sample(cell) else {
                    return (OlcOutcome::Retry, None);
                };
                // SAFETY: as above.
                let msnap = unsafe { MapCore::occ_snapshot_of(&raw const (*node).map) };
                let is_tree = matches!(msnap, crate::sync::RootSnapshot::Tree { .. });
                let host = StrHost {
                    node,
                    alloc,
                    locked: false,
                };
                if terminal {
                    if is_tree {
                        // T7 in tree state; the engine never empties a tree.
                        return (olc_remove_map(&host, chunk), None);
                    }
                    // SAFETY: live node.
                    let Some(mut lock) = (unsafe { CoverLock::try_lock_expect(node, csnap) })
                    else {
                        return (OlcOutcome::Retry, None);
                    };
                    // SAFETY: the cover lock excludes every other writer of this
                    // node's root state.
                    let prev = unsafe {
                        under_lock(node, alloc, || {
                            MapCore::remove_leaf_state_at(&raw mut (*node).map, alloc, chunk)
                        })
                    };
                    if prev.is_none() {
                        lock.abort_unmodified();
                        drop(lock);
                        return (OlcOutcome::Done(None), None);
                    }
                    // SAFETY: `node` is locked and live; the path frames are its
                    // ancestors, each loaded under a validated cover.
                    let deferred = unsafe { self.prune_locked(node, lock, &mut path, defer) };
                    return (OlcOutcome::Done(prev), deferred);
                }
                // SAFETY: pinned, cover sampled even above.
                let found = match unsafe { walk_validated_node::<true>(word, csnap, msnap, chunk) }
                {
                    Ok(found) => found,
                    Err(_) => return (OlcOutcome::Retry, None),
                };
                let Some(v) = found else {
                    return (OlcOutcome::Done(None), None);
                };
                if is_suffix_ptr(v) {
                    let sfx = unpack_suffix(v);
                    // SAFETY: as in `olc_insert`.
                    if unsafe { suffix_bytes(sfx) } != &key[off + CHUNK..] {
                        return (OlcOutcome::Done(None), None);
                    }
                    // T8, under the cover lock (see `olc_insert`).
                    // SAFETY: live node.
                    let Some(mut lock) = (unsafe { CoverLock::try_lock_expect(node, csnap) })
                    else {
                        return (OlcOutcome::Retry, None);
                    };
                    if is_tree {
                        let held = StrHost {
                            node,
                            alloc,
                            locked: true,
                        };
                        match olc_remove_map(&held, chunk) {
                            OlcOutcome::Done(Some(w)) => {
                                debug_assert_eq!(w, v, "the entry moved under the cover lock");
                            }
                            OlcOutcome::Done(None) => {
                                lock.abort_unmodified();
                                drop(lock);
                                return (OlcOutcome::Done(None), None);
                            }
                            other => {
                                lock.abort_unmodified();
                                drop(lock);
                                return (other, None);
                            }
                        }
                    } else {
                        // SAFETY: as for T7 in leaf state.
                        let w = unsafe {
                            under_lock(node, alloc, || {
                                MapCore::remove_leaf_state_at(&raw mut (*node).map, alloc, chunk)
                            })
                        };
                        debug_assert_eq!(w, Some(v), "the entry moved under the cover lock");
                    }
                    // SAFETY: unlinked but still live; last owner. Read out
                    // before disposal.
                    let val = unsafe { (*sfx).value };
                    dispose_suffix(sfx, defer, SuffixArena::of(alloc));
                    // SAFETY: as for T7.
                    let deferred = unsafe { self.prune_locked(node, lock, &mut path, defer) };
                    return (OlcOutcome::Done(Some(val)), deferred);
                }
                if !node_validate(cell, csnap) {
                    return (OlcOutcome::Retry, None);
                }
                path.push((node, chunk));
                node = unpack_child(v);
                off += CHUNK;
            }
        }

        /// T9: `node` is locked and just lost an entry. While the node is empty
        /// and has a parent, unlink it from the parent under the parent's lock,
        /// mark it obsolete and retire it, then continue with the parent. Two
        /// locks are held at most, child then parent, and both are `try_lock`s,
        /// so no writer ever waits on an ancestor while holding a descendant.
        ///
        /// Returns the cause when a prune was due and not done: the parent was
        /// held, the engine's remove of the entry did not go through, or the
        /// meta-trie root itself emptied (T10, `RootGrowth`). The emptied node
        /// then stays linked and empty — harmless to every reader and writer —
        /// until the caller's exclusive prune.
        ///
        /// # Safety
        ///
        /// `node` is live and locked by `lock`; every frame of `path` is a live
        /// ancestor of it, loaded under a validated cover, and the caller holds
        /// an epoch pin.
        unsafe fn prune_locked<'a>(
            &'a self,
            mut node: *mut StrNode,
            mut lock: CoverLock<'a>,
            path: &mut PathStack,
            defer: DeferHandle<'_>,
        ) -> Option<crate::sync::FallbackCause> {
            use crate::sync::{FallbackCause, OlcOutcome, olc_remove_map};
            let alloc = &self.alloc;
            loop {
                // A tree never empties on this path, whatever its stale
                // population reads; a leaf's population is exact under the lock.
                // SAFETY: `node` is live and locked by `lock`.
                if unsafe { MapCore::len_of(&raw const (*node).map) } != 0 {
                    drop(lock);
                    return None;
                }
                let Some((parent, pchunk)) = path.pop() else {
                    drop(lock);
                    return Some(FallbackCause::RootGrowth);
                };
                // SAFETY: a live ancestor (contract).
                let Some(mut plock) = (unsafe { CoverLock::try_lock(parent) }) else {
                    drop(lock);
                    return Some(FallbackCause::Contention);
                };
                // SAFETY: the parent is live and locked; its root state cannot
                // change under the lock.
                let parent_is_tree = unsafe { MapCore::root_is_tree_of(&raw const (*parent).map) };
                let removed = if parent_is_tree {
                    let held = StrHost {
                        node: parent,
                        alloc,
                        locked: true,
                    };
                    match olc_remove_map(&held, pchunk) {
                        OlcOutcome::Done(w) => w,
                        OlcOutcome::Retry => {
                            plock.abort_unmodified();
                            drop(plock);
                            drop(lock);
                            return Some(FallbackCause::Contention);
                        }
                        OlcOutcome::Fallback(cause) => {
                            plock.abort_unmodified();
                            drop(plock);
                            drop(lock);
                            return Some(cause);
                        }
                    }
                } else {
                    // SAFETY: as for T7 in leaf state, on the parent.
                    unsafe {
                        under_lock(parent, alloc, || {
                            MapCore::remove_leaf_state_at(&raw mut (*parent).map, alloc, pchunk)
                        })
                    }
                };
                debug_assert_eq!(
                    removed,
                    Some(pack_child(node)),
                    "the parent's entry moved under the parent's lock"
                );
                // Odd since the lock, so no reader validates through it; now
                // permanently odd (S3), then retired.
                lock.mark_obsolete();
                // SAFETY: live (retired below, not freed), and its interior is
                // the exclusive property of this writer since the lock.
                unsafe { under_lock(node, alloc, || dispose_node(node, alloc, defer)) };
                node = parent;
                lock = plock;
            }
        }
    }
}

#[cfg(feature = "std")]
impl ExpanseStrMap {
    /// Sets the population; the wrapper's exclusive sections re-sync it
    /// from the sharded counter optimistic writers count in (Refs #929).
    #[inline(always)]
    pub(crate) fn set_len(&mut self, pop: u64) {
        self.pop = pop;
    }

    /// The meta-trie root as a raw pointer, or null, without forming a
    /// reference to the node. An optimistic writer stores through it while
    /// other threads read the node, and a reader views its cover word as an
    /// atomic — which needs write provenance under Stacked Borrows even for
    /// a load — so the pointer must descend from the box, not from a shared
    /// borrow of its target (Miri caught the `from_ref` form on the reader).
    ///
    /// `None` on an empty map, rather than a null pointer: every caller
    /// matches before its first dereference, and no null constant flows
    /// towards one (CodeQL's pointer-validity query follows the constant
    /// through a loop-carried variable past its `is_null` check).
    #[inline(always)]
    fn root_raw(&self) -> Option<*mut StrNode> {
        // ONE load of the slot's word, then every decision on that value. The
        // slot is rewritten by the exclusive path (the meta-trie root's
        // creation and removal, `clear`) while optimistic readers and writers
        // run; testing it for `None` and then loading the box from it again
        // reads it twice, and the second load can see a root the first did
        // not — a null pointer reaching a dereference (caught on aarch64 by
        // the debug null-dereference check in `version_cell`, PR #1001).
        //
        // SAFETY: `Option<Box<T>>` has the layout of a nullable pointer to
        // `T` (the guaranteed null-pointer optimisation), so the slot can be
        // read as a `*mut StrNode` without materialising a `Box` from a word
        // another thread may be replacing. The read is a racy single-word
        // copy: whatever it returns is validated before any answer (the tree
        // word covers every store to this slot), and a node it names is
        // obsolete-marked before it retires and EBR-live under the caller's
        // pin. `self` lives inside the wrapper's `UnsafeCell`, so the pointer
        // carries write provenance for the optimistic writers.
        let p: *mut StrNode = unsafe { (&raw const self.root).cast::<*mut StrNode>().read() };
        if p.is_null() { None } else { Some(p) }
    }

    /// The meta-trie root as an untyped pointer, null when the map is empty:
    /// what the concurrent wrapper publishes for its readers (#1086).
    #[cfg(feature = "std")]
    #[inline(always)]
    pub(crate) fn root_word(&self) -> *const u8 {
        self.root_raw()
            .map_or(core::ptr::null(), |p| p.cast_const().cast::<u8>())
    }

    /// The exclusive prune (Refs #929): unlinks every empty node on `key`'s
    /// chunk chain, bottom up, and takes the meta-trie root if it is empty
    /// — what an optimistic remove leaves for the exclusive path when its
    /// own prune could not go through (`olc_remove`). Idempotent.
    #[cfg(not(feature = "ablation-str-serial-writers"))]
    pub(crate) fn prune_empty_path(&mut self, key: &NulFreeStr) {
        let key = key.as_bytes();
        let defer = self.deferred.get().cloned();
        let alloc = &self.alloc;
        let mut path: Vec<(*mut StrNode, u64)> = Vec::new();
        if let Some(root) = self.root.as_deref_mut() {
            let mut node: *mut StrNode = &raw mut *root;
            let mut off = 0usize;
            loop {
                let (chunk, terminal) = chunk_at(key, off);
                if terminal {
                    break;
                }
                // SAFETY: the root, then continuation values — live nodes,
                // and the descent never revisits one.
                let n = unsafe { &mut *node };
                match n.map.get(chunk) {
                    Some(v) if !is_suffix_ptr(v) => {
                        path.push((node, chunk));
                        node = unpack_child(v);
                        off += CHUNK;
                    }
                    _ => break,
                }
            }
            while let Some((parent_ptr, chunk)) = path.pop() {
                // SAFETY: recorded during the descent; still live.
                let parent = unsafe { &mut *parent_ptr };
                let child = unpack_child(parent.map.get(chunk).expect("path entry still linked"));
                // SAFETY: continuation value, a live child node.
                if !unsafe { &*child }.map.is_empty() {
                    break;
                }
                parent.resync_if_dirty();
                covered::<true, _>(parent, alloc, |m| m.remove_pathless(alloc, chunk));
                dispose_node(child, alloc, defer.as_ref());
            }
        }
        if self.root.as_deref().is_some_and(|r| r.map.is_empty()) {
            let root_box = self.root.take().expect("root present");
            dispose_node(Box::into_raw(root_box), alloc, defer.as_ref());
        }
    }
}

impl Default for ExpanseStrMap {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ExpanseStrMap {
    fn drop(&mut self) {
        self.clear_entries();
    }
}

#[cfg(test)]
mod tests {

    /// Wraps a test key. These are literals and generated keys the tests know
    /// are in-domain; a NUL in one is a bug in the test, so panicking is right.
    fn tk<B: AsRef<[u8]> + ?Sized>(bytes: &B) -> &NulFreeStr {
        NulFreeStr::new(bytes.as_ref()).expect("test key contains a NUL")
    }
    /// Keys that exercise every entry form the cursor has to walk: resolved
    /// inside a terminal chunk, held in a suffix leaf, and reached through a
    /// chain of child nodes; plus shared prefixes at each of those depths and
    /// the empty-remainder case a key of exactly 8 bytes produces.
    fn walk_corpus() -> Vec<Vec<u8>> {
        let mut keys: Vec<Vec<u8>> = Vec::new();
        for n in 1..=40usize {
            keys.push(vec![b'a'; n]);
        }
        for tail in [
            b"".as_slice(),
            b"x",
            b"yy",
            b"zzzzzzz",
            b"zzzzzzzz",
            b"zzzzzzzzz",
        ] {
            let mut k = b"prefix00".to_vec();
            k.extend_from_slice(tail);
            if !k.is_empty() {
                keys.push(k);
            }
        }
        let mut rng = 0x0DDB_1A5E_5EED_0001u64;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        for _ in 0..2000 {
            let n = 1 + (next() % 40) as usize;
            keys.push((0..n).map(|_| 0x21 + (next() % 94) as u8).collect());
        }
        keys.sort();
        keys.dedup();
        keys
    }

    /// The cursor visits exactly what the positional surface visits, in the
    /// same order, with the same slots.
    ///
    /// The positional pair is the ground truth here precisely because it is
    /// the surface the cursor exists to avoid using: an independent walk, not
    /// a re-derivation of the cursor's own logic.
    #[test]
    fn cursor_matches_the_positional_walk() {
        let keys = walk_corpus();
        let mut m = ExpanseStrMap::new();
        for (i, k) in keys.iter().enumerate() {
            m.insert(tk(k), i as u64);
        }

        let mut positional: Vec<(Vec<u8>, u64)> = Vec::new();
        let mut cur = m.first();
        while let Some((k, slot)) = cur {
            // SAFETY: slot is live until the next structural mutation, and
            // this walk performs none.
            positional.push((k.clone(), unsafe { *slot.as_ptr() }));
            cur = m.next_after(tk(&k));
        }

        let mut walked: Vec<(Vec<u8>, u64)> = Vec::new();
        let mut c = m.cursor();
        while let Some((k, slot)) = c.next() {
            // SAFETY: as above.
            walked.push((k.to_vec(), unsafe { *slot.as_ptr() }));
        }

        assert_eq!(walked.len(), keys.len(), "cursor visited the wrong count");
        assert_eq!(walked, positional, "cursor and positional walks diverge");
        assert!(
            walked.windows(2).all(|w| w[0].0 < w[1].0),
            "cursor order is not byte-lexicographic"
        );
    }

    /// Levels advanced many times stream from a sub-map cursor after their
    /// second advance (#1096): the walk still visits exactly what the
    /// positional surface visits, including a level whose chunk is
    /// `u64::MAX` (all `0xFF`, which has no successor), and a walk resumed
    /// from a seek into the middle of a level.
    #[test]
    fn cursor_streams_levels_it_advances_repeatedly() {
        let mut keys: Vec<Vec<u8>> = Vec::new();
        let groups = if cfg!(miri) { 6 } else { 40 };
        for g in 0..groups {
            for j in 0..(3 + g % 3) {
                let mut k = format!("grp{g:05}").into_bytes();
                k.extend_from_slice(format!("sub{j:05}").as_bytes());
                if j % 2 == 0 {
                    k.extend_from_slice(b"tail");
                }
                keys.push(k);
            }
        }
        for tail in [
            b"".as_slice(),
            b"a",
            b"b",
            b"\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF",
        ] {
            let mut k = vec![0xFF; 8];
            k.extend_from_slice(tail);
            keys.push(k);
        }
        let mut m = ExpanseStrMap::new();
        for (i, k) in keys.iter().enumerate() {
            m.insert(tk(k), i as u64);
        }
        let positional_from = |m: &mut ExpanseStrMap, start: Option<&[u8]>| {
            let mut out: Vec<(Vec<u8>, u64)> = Vec::new();
            let mut cur = match start {
                Some(s) => m.next_at_or_after(tk(s)),
                None => m.first(),
            };
            while let Some((k, slot)) = cur {
                // SAFETY: slot is live until the next structural mutation;
                // this walk performs none.
                out.push((k.clone(), unsafe { *slot.as_ptr() }));
                cur = m.next_after(tk(&k));
            }
            out
        };
        let expected = positional_from(&mut m, None);
        assert_eq!(expected.len(), keys.len());
        let mut walked: Vec<(Vec<u8>, u64)> = Vec::new();
        let mut c = m.cursor();
        while let Some((k, slot)) = c.next() {
            // SAFETY: as above.
            walked.push((k.to_vec(), unsafe { *slot.as_ptr() }));
        }
        assert_eq!(
            walked, expected,
            "streamed walk diverges from the positional walk"
        );

        let start = b"grp00001sub00001".to_vec();
        let expected = positional_from(&mut m, Some(&start));
        let mut walked: Vec<(Vec<u8>, u64)> = Vec::new();
        let mut c = m.cursor_at_or_after(tk(&start));
        while let Some((k, slot)) = c.next() {
            // SAFETY: as above.
            walked.push((k.to_vec(), unsafe { *slot.as_ptr() }));
        }
        assert_eq!(walked, expected, "walk resumed from a seek diverges");
    }

    /// Seeking lands where `next_at_or_after` lands, for keys that are
    /// present, absent, shorter and longer than what is stored, and past the
    /// end.
    #[test]
    fn cursor_seek_matches_next_at_or_after() {
        let keys = walk_corpus();
        let mut m = ExpanseStrMap::new();
        for (i, k) in keys.iter().enumerate() {
            m.insert(tk(k), i as u64);
        }

        let mut probes: Vec<Vec<u8>> = Vec::new();
        for k in keys.iter().step_by(37) {
            probes.push(k.clone());
            let mut shorter = k.clone();
            shorter.pop();
            if !shorter.is_empty() {
                probes.push(shorter);
            }
            let mut longer = k.clone();
            longer.push(b'~');
            probes.push(longer);
        }
        probes.push(b"\x7f\x7f\x7f\x7f".to_vec());
        probes.push(b"!".to_vec());

        for probe in probes {
            let expected = m.next_at_or_after(tk(&probe)).map(|(k, slot)| {
                // SAFETY: no structural mutation between here and the read.
                (k, unsafe { *slot.as_ptr() })
            });
            let mut c = m.cursor_at_or_after(tk(&probe));
            let got = c.next().map(|(k, slot)| {
                // SAFETY: as above.
                (k.to_vec(), unsafe { *slot.as_ptr() })
            });
            assert_eq!(got, expected, "seek diverged on probe {probe:?}");
        }
    }

    /// A seeked cursor continues in order, and running it to exhaustion
    /// yields exactly the tail of the full walk.
    #[test]
    fn cursor_continues_correctly_after_a_seek() {
        let keys = walk_corpus();
        let mut m = ExpanseStrMap::new();
        for (i, k) in keys.iter().enumerate() {
            m.insert(tk(k), i as u64);
        }
        let start = &keys[keys.len() / 3];
        let expected: Vec<Vec<u8>> = keys.iter().skip(keys.len() / 3).cloned().collect();

        let mut got: Vec<Vec<u8>> = Vec::new();
        let mut c = m.cursor_at_or_after(tk(start));
        while let Some((k, _)) = c.next() {
            got.push(k.to_vec());
        }
        assert_eq!(
            got, expected,
            "seeked walk is not the tail of the full walk"
        );
    }

    /// The example in [`StrCursor`]'s documentation, executable.
    ///
    /// The snippet there is a `text` block for the reason given beside it, so
    /// this is what keeps it honest; change one and change the other.
    #[test]
    fn cursor_walks_the_documented_example() {
        let mut map = ExpanseStrMap::new();
        for k in [b"apple".as_slice(), b"apricot", b"banana"] {
            map.insert(tk(k), k.len() as u64);
        }
        let mut c = map.cursor_at_or_after(tk(b"ap"));
        let mut seen = Vec::new();
        while let Some((key, slot)) = c.next() {
            // SAFETY: the cursor borrows the map for its lifetime, so the slot
            // is a live value word and nothing can mutate the map meanwhile.
            seen.push((key.to_vec(), unsafe { *slot.as_ptr() }));
        }
        assert_eq!(seen.len(), 3, "the seek should admit all three keys");
        assert_eq!(seen[0], (b"apple".to_vec(), 5));
        assert_eq!(seen[1], (b"apricot".to_vec(), 7));
        assert_eq!(seen[2], (b"banana".to_vec(), 6));
    }

    /// The cursor's raw path stack over every entry form it can meet, in a
    /// corpus small enough for the per-PR Miri lane.
    ///
    /// `cursor_matches_the_positional_walk` covers the same ground far more
    /// thoroughly, but on 2,040 keys — too slow for the interpreter, so it
    /// runs only in the nightly shard. The forms that have to be reached
    /// here are the ones the one-allocation leaf and the lending cursor
    /// introduced, and that the short-key tests above never build:
    ///
    /// * a **zero-length suffix leaf** the walk still meets (`aaaaaaab`, one
    ///   chunk exactly, no other key sharing it) — the case the old
    ///   two-allocation shape needed a special path for;
    /// * a zero-length leaf that is **created, compared and then disposed** by
    ///   a split (`aaaaaaaa`, once `aaaaaaaaBBBBBBBB` arrives), which is the
    ///   `dispose_suffix` path for an empty remainder;
    /// * a **populated suffix leaf** read back through `suffix_bytes` (`cc`,
    ///   under the level-2 child), whose provenance must reach past the
    ///   header (see [`StrSuffix`]);
    /// * **backtracking** from depth 3 back to the root, which is the only
    ///   shape that drives `next`'s unwind loop more than one frame.
    #[test]
    fn cursor_walks_a_deep_trie_with_suffix_leaves() {
        let keys: [&[u8]; 6] = [
            b"aaaaaaaa",           // one chunk exactly: zero-length suffix leaf
            b"aaaaaaaaBBBBBBBB",   // two chunks: zero-length suffix under a child
            b"aaaaaaaaBBBBBBBBcc", // splits the leaf above; depth 3
            b"aaaaaaaaXX",         // diverges at level 2: populated suffix leaf
            b"aaaaaaab",           // diverges in the first chunk
            b"zz",                 // terminal at the root: forces a full unwind
        ];
        let mut m = ExpanseStrMap::new();
        for (i, k) in keys.iter().enumerate() {
            m.insert(tk(k), i as u64);
        }

        let mut expected: Vec<(Vec<u8>, u64)> = keys
            .iter()
            .enumerate()
            .map(|(i, k)| (k.to_vec(), i as u64))
            .collect();
        expected.sort();

        let mut c = m.cursor();
        let mut walked = Vec::new();
        while let Some((key, slot)) = c.next() {
            // SAFETY: the cursor holds the map borrowed for its lifetime, so
            // the slot is a live value word and nothing can mutate it here.
            walked.push((key.to_vec(), unsafe { *slot.as_ptr() }));
        }
        assert_eq!(walked, expected, "the cursor did not return the corpus");

        // The same corpus from a seek that lands mid-trie, so the unwind runs
        // from a stack the seek built rather than one `next` built.
        let mut c = m.cursor_at_or_after(tk(b"aaaaaaaaBBBBBBBBcc"));
        let mut walked = Vec::new();
        while let Some((key, _)) = c.next() {
            walked.push(key.to_vec());
        }
        let from: Vec<Vec<u8>> = expected
            .iter()
            .filter(|(k, _)| k.as_slice() >= b"aaaaaaaaBBBBBBBBcc".as_slice())
            .map(|(k, _)| k.clone())
            .collect();
        assert_eq!(walked, from, "the seeked cursor did not resume correctly");
    }

    /// The SWAR haszero in [`is_terminal`] agrees with the byte scan it
    /// replaced, on every single-byte position and over a wide random sweep.
    ///
    /// Required by AGENTS.md §5: a vectorised or SWAR path carries a portable
    /// reference and a parity test. `is_terminal_scan` is that reference.
    #[test]
    fn swar_haszero_matches_the_byte_scan() {
        // Every byte value in every lane, which is where a borrow-propagation
        // bug in a haszero shows up first.
        for b in 0u64..=255 {
            for lane in 0..8 {
                let v = b << (8 * lane);
                assert_eq!(is_terminal(v), is_terminal_scan(v), "lane {lane} byte {b}");
                let filled = v | !(0xFFu64 << (8 * lane));
                assert_eq!(
                    is_terminal(filled),
                    is_terminal_scan(filled),
                    "lane {lane} byte {b}, others set"
                );
            }
        }
        // Boundaries the identity is most likely to trip on.
        for v in [0, 1, u64::MAX, 0x0101_0101_0101_0101, 0x8080_8080_8080_8080] {
            assert_eq!(is_terminal(v), is_terminal_scan(v), "boundary {v:#x}");
        }
        // Random sweep, deterministic seed.
        let mut rng = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..if cfg!(miri) { 500 } else { 200_000 } {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            assert_eq!(is_terminal(rng), is_terminal_scan(rng), "random {rng:#x}");
        }
    }

    /// `cursor_prefix` yields exactly the keys that start with the prefix, in
    /// order, with their own slots: checked against a sorted model over keys
    /// that share long prefixes (so some sit in suffix leaves above the
    /// bound's depth and some deep under it), for prefixes of every length
    /// through three chunks, prefixes that are keys, absent prefixes,
    /// prefixes longer than any key, and the empty prefix.
    #[test]
    fn cursor_prefix_matches_a_filtered_model() {
        let mut keys: Vec<Vec<u8>> = Vec::new();
        let n = if cfg!(miri) { 60 } else { 600 };
        let mut rng = 0xC0FF_EE00_1234_5678u64;
        for i in 0..n {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let stem = [
                "a",
                "ab",
                "abcdefg",
                "abcdefgh",
                "abcdefghij",
                "abcdefghijklmnop",
                "b",
            ];
            let mut k = stem[i % stem.len()].as_bytes().to_vec();
            let tail = (rng % 5) as usize;
            for j in 0..tail {
                k.push(b"0123456789xyz"[((rng >> (8 * j)) % 13) as usize]);
            }
            keys.push(k);
        }
        keys.sort();
        keys.dedup();
        let mut m = ExpanseStrMap::new();
        for (i, k) in keys.iter().enumerate() {
            m.insert(tk(k), i as u64);
        }
        let mut prefixes: Vec<Vec<u8>> = vec![
            Vec::new(),
            b"zz".to_vec(),
            b"abcdefghijklmnopqrstuvwxyz".to_vec(),
        ];
        for k in keys.iter().step_by(7) {
            for l in 0..=k.len() {
                prefixes.push(k[..l].to_vec());
            }
            let mut past = k.clone();
            past.push(b'~');
            prefixes.push(past);
        }
        let full = "abcdefghijklmnopqrstuvwxy".as_bytes();
        for l in 0..=full.len() {
            prefixes.push(full[..l].to_vec());
        }
        for p in &prefixes {
            let want: Vec<(Vec<u8>, u64)> = keys
                .iter()
                .enumerate()
                .filter(|(_, k)| k.starts_with(p))
                .map(|(i, k)| (k.clone(), i as u64))
                .collect();
            let mut got: Vec<(Vec<u8>, u64)> = Vec::new();
            let mut c = m.cursor_prefix(tk(p));
            while let Some((k, slot)) = c.next() {
                // SAFETY: slot is live until the next structural mutation;
                // this walk performs none.
                got.push((k.to_vec(), unsafe { *slot.as_ptr() }));
            }
            assert_eq!(
                c.next().map(|(k, _)| k.to_vec()),
                None,
                "cursor restarted after None"
            );
            assert_eq!(got, want, "prefix {:?}", String::from_utf8_lossy(p));
        }
    }

    /// [`push_chunk_suffix`] appends the chunk's bytes and then the tail, for
    /// every tail length through three words — each branch of `copy_small`
    /// and its boundaries — onto an empty buffer and onto one whose length
    /// is not a multiple of eight.
    #[test]
    fn push_chunk_suffix_appends_chunk_then_tail() {
        let chunk = u64::from_be_bytes(*b"ABCDEFGH");
        let src: Vec<u8> = (1..=40u8).collect();
        for head in [&b""[..], b"xyz"] {
            for n in 0..=src.len() {
                let mut got = head.to_vec();
                push_chunk_suffix(&mut got, chunk, &src[..n]);
                let mut want = head.to_vec();
                want.extend_from_slice(b"ABCDEFGH");
                want.extend_from_slice(&src[..n]);
                assert_eq!(got, want, "head {} tail {n}", head.len());
            }
        }
    }

    /// [`push_terminal`] appends what the byte scan it replaced appends, for
    /// every terminal chunk a NUL-free key can produce: each content length
    /// 0..8 with every byte value in every content position, and a random
    /// sweep of NUL-free contents.
    #[test]
    fn push_terminal_matches_the_byte_scan() {
        let check = |chunk: u64| {
            let mut got = b"prefix".to_vec();
            push_terminal(&mut got, chunk);
            let mut want = b"prefix".to_vec();
            want.extend(terminal_bytes_scan(chunk));
            assert_eq!(got, want, "chunk {chunk:#018x}");
        };
        for n in 0..CHUNK {
            for b in 1u8..=255 {
                for pos in 0..n {
                    let mut c = [0x41u8; CHUNK];
                    c[n..].fill(0);
                    c[pos] = b;
                    check(u64::from_be_bytes(c));
                }
            }
            let mut c = [0x7Fu8; CHUNK];
            c[n..].fill(0);
            check(u64::from_be_bytes(c));
        }
        let mut rng = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..if cfg!(miri) { 500 } else { 100_000 } {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let n = (rng % CHUNK as u64) as usize;
            let mut c = rng.to_be_bytes().map(|b| b | 1);
            c[n..].fill(0);
            check(u64::from_be_bytes(c));
        }
    }

    /// The key type is the domain, so an out-of-domain key cannot be built.
    ///
    /// This replaces a test that asserted `chunk_at` decided terminality by
    /// content. That was the previous fix for #794 and it cost 2.6-4.3% on
    /// every string operation; the domain is now carried by [`NulFreeStr`],
    /// which is why `chunk_at` can keep its hoistable length rule.
    #[test]
    fn the_key_type_rejects_every_position_of_a_nul() {
        assert!(NulFreeStr::new(b"").is_some(), "the empty key is in domain");
        assert!(NulFreeStr::new(b"abc").is_some());
        for n in 0..12usize {
            let mut k = vec![b'a'; 12];
            k[n] = 0;
            assert!(
                NulFreeStr::new(&k).is_none(),
                "a NUL at index {n} was accepted"
            );
        }
        // The #794 key specifically.
        assert!(NulFreeStr::new(b"abc\0defghij").is_none());
        // And the round trip is the identity on the bytes.
        let k = NulFreeStr::new(b"/api/v2/tenants").expect("in domain");
        assert_eq!(k.as_bytes(), b"/api/v2/tenants");
        assert_eq!(k.len(), 15);
        assert!(!k.is_empty());
    }

    /// `TryFrom` is the fallible conversion callers reach for, and its error
    /// says what the domain is.
    #[test]
    fn try_from_reports_the_domain_violation() {
        let ok: Result<&NulFreeStr, _> = b"fine".as_slice().try_into();
        assert!(ok.is_ok());
        let bad: Result<&NulFreeStr, _> = b"no\0good".as_slice().try_into();
        assert_eq!(bad.unwrap_err(), NulInKey);
    }

    /// Degenerate shapes: empty map, one key, and a cursor driven past the
    /// end, which must keep returning `None` rather than restarting.
    #[test]
    fn cursor_edges() {
        let mut empty = ExpanseStrMap::new();
        assert!(empty.cursor().next().is_none());
        assert!(empty.cursor_at_or_after(tk(b"anything")).next().is_none());

        let mut one = ExpanseStrMap::new();
        one.insert(tk(b"solo"), 7);
        let mut c = one.cursor();
        assert_eq!(c.next().map(|(k, _)| k.to_vec()), Some(b"solo".to_vec()));
        assert!(c.next().is_none());
        assert!(c.next().is_none(), "an exhausted cursor restarted");

        let mut past = one.cursor_at_or_after(tk(b"zzz"));
        assert!(past.next().is_none());
    }

    /// The value slot the cursor hands out is the map's own, not a copy.
    #[test]
    fn cursor_slots_are_writable_in_place() {
        let mut m = ExpanseStrMap::new();
        for k in [b"alpha".as_slice(), b"beta", b"gamma"] {
            m.insert(tk(k), 0);
        }
        let mut c = m.cursor();
        while let Some((_, slot)) = c.next() {
            // SAFETY: the slot is the map's value word, live for the borrow.
            unsafe { *slot.as_ptr() = 42 };
        }
        for k in [b"alpha".as_slice(), b"beta", b"gamma"] {
            assert_eq!(
                m.get(tk(k)),
                Some(42),
                "in-place write through the cursor lost"
            );
        }
    }

    /// Keys long enough that one node per 8 bytes would overflow the
    /// stack on teardown. Before the destructor was made iterative this
    /// aborted the process **while freeing** — the failure mode with no
    /// recovery path, in the one place a caller cannot guard against it.
    /// Run on a deliberately small stack so the guard is honest on every
    /// platform rather than relying on the default 8 MiB.
    #[test]
    fn very_long_keys_do_not_overflow_the_stack_on_drop() {
        let handle = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let mut m = ExpanseStrMap::new();
                let key = vec![b'k'; if cfg!(miri) { 512 } else { 64 * 1024 }];
                assert_eq!(m.insert(tk(&key), 1), None);
                assert_eq!(m.get(tk(&key)), Some(1));
                // A second key sharing most of the chain, so teardown has
                // branching to walk rather than one straight line.
                let mut other = key.clone();
                *other.last_mut().expect("non-empty") = b'z';
                assert_eq!(m.insert(tk(&other), 2), None);
                assert_eq!(m.len(), 2);
                drop(m);
            })
            .expect("spawn");
        handle
            .join()
            .expect("deep-key teardown overflowed the stack");
    }

    /// Same depth through `remove` and the emptied-node pruning path.
    #[test]
    fn very_long_keys_remove_and_empty() {
        let handle = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let mut m = ExpanseStrMap::new();
                let key = vec![b'q'; if cfg!(miri) { 512 } else { 32 * 1024 }];
                m.insert(tk(&key), 7);
                assert_eq!(m.remove(tk(&key)), Some(7));
                assert!(m.is_empty());
                assert_eq!(m.get(tk(&key)), None);
                assert_eq!(m.clear(), 0, "removing the last key freed everything");
            })
            .expect("spawn");
        handle.join().expect("deep-key remove overflowed the stack");
    }

    /// Ordered navigation over the same depth, including the backtracking
    /// paths — a query whose descent runs the full chain and then finds
    /// nothing at-or-after, so it has to unwind every level it pushed.
    /// The recursive version overflowed here even though `Drop` and
    /// `remove` had already been made iterative.
    #[test]
    fn very_long_keys_navigate_without_overflowing_the_stack() {
        let handle = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let mut m = ExpanseStrMap::new();
                let deep = vec![b'm'; if cfg!(miri) { 512 } else { 64 * 1024 }];
                // A sibling diverging only in the very last chunk, so the
                // whole chain is shared and backtracking is forced to the
                // bottom before it can resolve.
                let mut sibling = deep.clone();
                *sibling.last_mut().expect("non-empty") = b'n';
                m.insert(tk(&deep), 1);
                m.insert(tk(&sibling), 2);

                assert_eq!(m.first().map(|(k, _)| k), Some(deep.clone()));
                assert_eq!(m.last().map(|(k, _)| k), Some(sibling.clone()));
                assert_eq!(m.get(tk(&deep)), Some(1));
                assert_eq!(m.get(tk(&sibling)), Some(2));
                assert_eq!(
                    m.next_at_or_after(tk(&deep)).map(|(k, _)| k),
                    Some(deep.clone())
                );
                assert_eq!(
                    m.next_after(tk(&deep)).map(|(k, _)| k),
                    Some(sibling.clone())
                );
                assert_eq!(
                    m.prev_before(tk(&sibling)).map(|(k, _)| k),
                    Some(deep.clone())
                );

                // Past the end: descends the full chain, finds nothing at
                // or after, and unwinds every recorded level.
                let mut past = deep.clone();
                *past.last_mut().expect("non-empty") = b'z';
                assert_eq!(m.next_at_or_after(tk(&past)), None);
                // Mirror: before the beginning.
                let mut before = deep.clone();
                *before.last_mut().expect("non-empty") = b'a';
                assert_eq!(m.prev_before(tk(&before)), None);
            })
            .expect("spawn");
        handle
            .join()
            .expect("deep-key navigation overflowed the stack");
    }

    use super::*;
    use std::collections::BTreeMap;

    struct XorShift(u64);
    impl XorShift {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
    }

    fn keygen(rng: &mut XorShift) -> Vec<u8> {
        const PREFIXES: [&[u8]; 4] = [b"", b"user:profile:", b"a/very/long/shared/path/", b"k"];
        let p = PREFIXES[(rng.next() % 4) as usize];
        let len = (rng.next() % 20) as usize;
        let mut k = p.to_vec();
        for _ in 0..len {
            k.push((rng.next() % 255 + 1) as u8); // 1..=255: NUL-free
        }
        k
    }

    /// Phase 7 (issue #219 Phase 2): deferred-mode round trip —
    /// single-threaded and Miri-clean. Every disposal path (suffix split,
    /// suffix removal, empty-node pruning, root removal, whole-tree clear)
    /// routes unlinked allocations through the epoch collector, and
    /// everything drains without leaks or double frees.
    #[test]
    #[cfg(feature = "std")]
    fn deferred_strmap_dispose_round_trip() {
        use crate::occ::Collector;
        use core_alloc::sync::Arc;

        let collector = Arc::new(Collector::new());
        let mut m = ExpanseStrMap::new();
        // Deferral must precede every allocation (`defer_to` requires an
        // empty map — slab-carved memory must never reach the collector).
        m.defer_to(Arc::clone(&collector));
        m.insert(tk(b"pre-existing:alpha"), 1);
        m.insert(tk(b"pre-existing:beta"), 2);

        // Suffix creation, then a split (disposes the old suffix).
        m.insert(tk(b"shared-prefix-01:aaaa"), 10);
        m.insert(tk(b"shared-prefix-01:bbbb"), 11);
        // In-place value update on a suffix leaf (no disposal).
        assert_eq!(m.insert(tk(b"shared-prefix-01:aaaa"), 12), Some(10));
        assert_eq!(m.get(tk(b"shared-prefix-01:aaaa")), Some(12));
        // ins_slot split path.
        let slot = m.ins_slot(tk(b"shared-prefix-01:aaXX"));
        // SAFETY: slot valid until next mutation.
        unsafe { slot.as_ptr().write(13) };
        assert_eq!(m.get(tk(b"shared-prefix-01:aaXX")), Some(13));

        // Suffix removal + emptied-node pruning back up the chain.
        assert_eq!(m.remove(tk(b"shared-prefix-01:aaXX")), Some(13));
        assert_eq!(m.remove(tk(b"shared-prefix-01:bbbb")), Some(11));
        assert_eq!(m.remove(tk(b"shared-prefix-01:aaaa")), Some(12));
        assert_eq!(m.get(tk(b"pre-existing:alpha")), Some(1));

        // Whole-tree disposal (dispose_tree), then root removal via the
        // last-key path.
        assert_eq!(m.len(), 2);
        assert!(m.clear() > 0);
        m.insert(tk(b"solo"), 42);
        assert_eq!(m.remove(tk(b"solo")), Some(42));
        assert!(m.is_empty());

        // Grace-period advances free the retired chain; drop drains the rest.
        collector.try_advance();
        collector.try_advance();
        collector.try_advance();
        drop(m);
        drop(collector);
    }

    /// #363 Step A regression guard, re-argued for #929's cover word: a
    /// sub-trie node is the map engine core **plus one OCC version word**
    /// and nothing else — no embedded allocator, no per-node insert-path
    /// cache. Re-embedding either (the pre-#363 layout was ~700 bytes)
    /// fails here before it shows up as a descent-locality regression.
    ///
    /// The bound is **derived, not a literal** (AGENTS.md §2.1.6): the
    /// cover is a `u32` at offset 0 and `MapCore` aligns to 8, so the word
    /// and its padding cost exactly one `align_of::<MapCore>()` prefix. If
    /// `MapCore` ever aligns differently this tracks it instead of decaying
    /// into a stale constant.
    ///
    /// The `<= 64` half is the one #363 cared about and it still holds with
    /// room to spare: a node stays inside one cache line, so a descent hop
    /// touches one line for the cover and the map root together — which is
    /// why the word heads the struct.
    #[test]
    fn str_node_is_the_map_core_plus_its_cover_word() {
        assert_eq!(
            size_of::<StrNode>(),
            size_of::<crate::map::MapCore>() + align_of::<crate::map::MapCore>(),
            "StrNode must be the map core plus exactly the cover word and \
             its alignment padding; got {} for a {}-byte core",
            size_of::<StrNode>(),
            size_of::<crate::map::MapCore>()
        );
        assert!(
            size_of::<StrNode>() <= 64,
            "StrNode grew past one cache line: {}",
            size_of::<StrNode>()
        );
    }

    /// #929 groundwork: the cover word heads the node, so the OCC protocol
    /// reaches it from a bare `*mut StrNode` with no field arithmetic.
    /// `offset_of!` is already asserted at compile time; this pins the
    /// consequence the protocol actually relies on — that the address the
    /// map hands out *is* the node address.
    #[test]
    fn deferred_str_node_cover_word_heads_the_node() {
        let mut m = ExpanseStrMap::new();
        assert!(
            m.root_cover_addr().is_null(),
            "an empty map has no root node and so no root cover word"
        );
        m.insert(tk("alpha"), 1);
        let root = m.root.as_deref().expect("root present after an insert");
        assert_eq!(
            m.root_cover_addr().cast::<StrNode>(),
            core::ptr::from_ref(root),
            "the cover word must sit at offset 0 of the node"
        );
    }

    /// #929: the plain path's instantiation of the twins touches no cover
    /// word. On a map that was never deferred, every cover stays at its
    /// initial 0 across inserts, replaces, splits and removes; only the
    /// deferred twin and the optimistic path move them
    /// (`sync_strmap_writes_move_the_per_node_cover`,
    /// `deferred_olc_*`). #985 pinned this for both modes; the deferred
    /// half is now the behaviour change it anticipated.
    #[test]
    fn str_node_cover_words_are_untouched_unshared() {
        let mut m = ExpanseStrMap::new();
        // Shared prefixes force continuation entries, suffix splits and
        // child nodes — T2, T3, T4 and T5 of METHODOLOGY §17.2.1.
        let keys: [&[u8]; 6] = [
            b"prefix_aaaaaaaa_one",
            b"prefix_aaaaaaaa_two",
            b"prefix_bbbbbbbb_one",
            b"prefix_aaaaaaaa",
            b"short",
            b"prefix_aaaaaaaa_one_longer_still",
        ];
        for (i, k) in keys.iter().enumerate() {
            m.insert(tk(*k), i as u64);
        }
        // T3: an in-place value replace over a live entry.
        m.insert(tk(b"prefix_aaaaaaaa_one"), 99);
        // T7/T8/T9: removals, including one that prunes an emptied child.
        m.remove(tk(b"prefix_bbbbbbbb_one"));
        m.remove(tk(b"short"));

        let mut unbumped = 0usize;
        let mut stack: Vec<*const StrNode> = match m.root.as_deref() {
            Some(r) => vec![core::ptr::from_ref(r)],
            None => Vec::new(),
        };
        assert!(!stack.is_empty(), "the fixture must leave a populated tree");
        while let Some(p) = stack.pop() {
            // SAFETY: `p` is the root or a continuation child of a node
            // already visited, so it is a live node this map owns.
            let node = unsafe { &*p };
            assert_eq!(
                node.cover, 0,
                "a cover word moved on a map that was never deferred: the \
                 plain twin must not touch it"
            );
            unbumped += 1;
            for (k, v) in node.map.iter() {
                if !is_terminal(k) && !is_suffix_ptr(v) {
                    stack.push(unpack_child(v));
                }
            }
        }
        assert!(
            unbumped >= 2,
            "the fixture must build a multi-node meta-trie; saw {unbumped}"
        );
    }

    /// #363 Step A: `mem_used` (shared-allocator bytes + shell walk) and
    /// the `clear()` return value agree byte-exactly, and both go to zero.
    #[test]
    fn mem_used_matches_clear_accounting() {
        let mut m = ExpanseStrMap::new();
        let n = if cfg!(miri) { 60 } else { 500 };
        for i in 0..n {
            let k = format!("/api/v2/tenants/{:04}/resources/item-{:06}", i % 37, i);
            m.insert(tk(k.as_bytes()), i);
        }
        let used = m.mem_used() as u64;
        assert!(used > 0);
        assert_eq!(m.clear(), used, "clear() must release exactly mem_used()");
        assert_eq!(m.mem_used(), 0);
    }

    #[test]
    fn model_differential() {
        let ops = if cfg!(miri) { 50 } else { 4000 };
        let mut rng = XorShift(0x571A_5EED_1234 | 1);
        let mut map = ExpanseStrMap::new();
        let mut model: BTreeMap<Vec<u8>, u64> = BTreeMap::new();
        for _ in 0..ops {
            let k = keygen(&mut rng);
            match rng.next() % 4 {
                0 | 1 => {
                    let v = rng.next();
                    assert_eq!(
                        map.insert(tk(&k), v),
                        model.insert(k.clone(), v),
                        "ins {k:?}"
                    );
                }
                2 => assert_eq!(map.remove(tk(&k)), model.remove(&k), "rm {k:?}"),
                _ => assert_eq!(map.get(tk(&k)), model.get(&k).copied(), "get {k:?}"),
            }
            assert_eq!(map.len(), model.len() as u64);
        }
        // Ordered sweep both directions.
        let mut cursor = map.first();
        for (mk, mv) in &model {
            let (k, slot) = cursor.expect("sweep entry");
            assert_eq!(&k, mk, "sweep key");
            // SAFETY: slot valid until next mutation; none happens here.
            assert_eq!(unsafe { *slot.as_ptr() }, *mv, "sweep value");
            cursor = map.next_after(tk(&k));
        }
        assert!(cursor.is_none());
        let mut cursor = map.last();
        for (mk, mv) in model.iter().rev() {
            let (k, slot) = cursor.expect("rev sweep entry");
            assert_eq!(&k, mk, "rev sweep key");
            // SAFETY: as above.
            assert_eq!(unsafe { *slot.as_ptr() }, *mv, "rev sweep value");
            cursor = map.prev_before(tk(&k));
        }
        assert!(cursor.is_none());
        // Point navigation probes.
        for _ in 0..if cfg!(miri) { 30 } else { 400 } {
            let k = keygen(&mut rng);
            assert_eq!(
                map.next_at_or_after(tk(&k)).map(|e| e.0),
                model.range(k.clone()..).next().map(|(mk, _)| mk.clone()),
                "next>= {k:?}"
            );
            assert_eq!(
                map.prev_at_or_before(tk(&k)).map(|e| e.0),
                model
                    .range(..=k.clone())
                    .next_back()
                    .map(|(mk, _)| mk.clone()),
                "prev<= {k:?}"
            );
        }
        // Drain.
        let keys: Vec<Vec<u8>> = model.keys().cloned().collect();
        for k in keys {
            assert_eq!(map.remove(tk(&k)), model.remove(&k));
        }
        assert!(map.is_empty());
    }

    #[test]
    fn edge_keys_and_slots() {
        let mut map = ExpanseStrMap::new();
        // Empty string, chunk-boundary lengths, shared prefixes.
        for (i, k) in [
            b"".as_slice(),
            b"a",
            b"abcdefgh",         // exactly one chunk
            b"abcdefghi",        // crosses into a second chunk
            b"abcdefghabcdefgh", // two full chunks
            b"abcdefgg",
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(map.insert(tk(k), i as u64 + 10), None, "{k:?}");
        }
        assert_eq!(map.get(tk(b"")), Some(10));
        assert_eq!(map.get(tk(b"abcdefgh")), Some(12));
        assert_eq!(map.get(tk(b"abcdefg")), None);
        // ins_slot keeps existing values and writes through.
        let slot = map.ins_slot(tk(b"abcdefgh"));
        // SAFETY: slot valid until next mutation.
        unsafe {
            assert_eq!(*slot.as_ptr(), 12);
            slot.as_ptr().write(99);
        }
        assert_eq!(map.get(tk(b"abcdefgh")), Some(99));
        // Ordering across boundary shapes.
        let (first, _) = map.first().unwrap();
        assert_eq!(first, b"");
        assert_eq!(map.next_after(tk(b"")).unwrap().0, b"a");
        assert_eq!(map.next_after(tk(b"abcdefgg")).unwrap().0, b"abcdefgh");
        assert_eq!(
            map.next_after(tk(b"abcdefgh")).unwrap().0,
            b"abcdefghabcdefgh"
        );
        assert_eq!(map.prev_before(tk(b"abcdefgh")).unwrap().0, b"abcdefgg");
        assert_eq!(map.last().unwrap().0, b"abcdefghi");
        let freed = map.clear();
        assert!(freed > 0);
        assert!(map.is_empty());
    }

    #[test]
    fn test_cross_chunk_tail_collapse_split_and_memory() {
        let mut map = ExpanseStrMap::new();
        // Insert a 64-byte key. With tail collapse, this creates 1 StrNode + 1 StrSuffix.
        let key1 = b"org.apache.hadoop.fs.azurebfs.services.AbfsClientTestFixture";
        map.insert(tk(key1), 100);
        assert_eq!(map.get(tk(key1)), Some(100));
        assert_eq!(map.len(), 1);

        // Insert a second key sharing a long prefix (35 bytes).
        let key2 = b"org.apache.hadoop.fs.azurebfs.services.AbfsRestOperation";
        map.insert(tk(key2), 200);
        assert_eq!(map.get(tk(key1)), Some(100));
        assert_eq!(map.get(tk(key2)), Some(200));
        assert_eq!(map.len(), 2);

        // Insert a third key diverging early (at byte 4).
        let key3 = b"org.eclipse.jetty.server.Server";
        map.insert(tk(key3), 300);
        assert_eq!(map.get(tk(key1)), Some(100));
        assert_eq!(map.get(tk(key2)), Some(200));
        assert_eq!(map.get(tk(key3)), Some(300));
        assert_eq!(map.len(), 3);

        // Verify sorted navigation across compressed paths:
        let (k1, s1) = map.first().unwrap();
        assert_eq!(k1, key1);
        // SAFETY: slot is valid until next mutation.
        unsafe { assert_eq!(*s1.as_ptr(), 100) };

        let (k2, s2) = map.next_after(tk(key1)).unwrap();
        assert_eq!(k2, key2);
        // SAFETY: slot is valid until next mutation.
        unsafe { assert_eq!(*s2.as_ptr(), 200) };

        let (k3, s3) = map.next_after(tk(key2)).unwrap();
        assert_eq!(k3, key3);
        // SAFETY: slot is valid until next mutation.
        unsafe { assert_eq!(*s3.as_ptr(), 300) };

        assert_eq!(map.next_after(tk(key3)), None);

        // Remove the split key:
        assert_eq!(map.remove(tk(key2)), Some(200));
        assert_eq!(map.get(tk(key2)), None);
        assert_eq!(map.get(tk(key1)), Some(100));
        assert_eq!(map.get(tk(key3)), Some(300));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_ins_slot_model_and_invariants() {
        let mut map = ExpanseStrMap::new();
        let mut model: BTreeMap<Vec<u8>, u64> = BTreeMap::new();

        // 1. Boundary key checks: empty key, exact 8-byte, exact 16-byte keys.
        let empty_key = b"";
        let slot_empty = map.ins_slot(tk(empty_key));
        // SAFETY: slot is valid until next mutation.
        unsafe {
            assert_eq!(*slot_empty.as_ptr(), 0);
            *slot_empty.as_ptr() = 42;
        }
        model.insert(empty_key.to_vec(), 42);
        assert_eq!(map.len(), model.len() as u64);
        assert_eq!(map.get(tk(empty_key)), Some(42));

        // Tightest boundary pair: 7-byte terminal ("foobarb") vs 8-byte non-terminal ("foobarba").
        // "foobarb" packs into chunk "foobarb\0" (with one zero byte).
        // "foobarba" packs into chunk "foobarba" (no zero bytes).
        // They differ only in the final byte; directly verifies chunk-collision impossibility.
        let k7 = b"foobarb";
        let k8 = b"foobarba";
        let s7 = map.ins_slot(tk(k7));
        // SAFETY: slot is valid until next mutation.
        unsafe {
            assert_eq!(*s7.as_ptr(), 0);
            *s7.as_ptr() = 70;
        }
        model.insert(k7.to_vec(), 70);
        assert_eq!(map.len(), model.len() as u64);

        let s8 = map.ins_slot(tk(k8));
        // SAFETY: slot is valid until next mutation.
        unsafe {
            assert_eq!(*s8.as_ptr(), 0);
            *s8.as_ptr() = 80;
        }
        model.insert(k8.to_vec(), 80);
        assert_eq!(map.len(), model.len() as u64);
        assert_eq!(map.get(tk(k7)), Some(70));
        assert_eq!(map.get(tk(k8)), Some(80));

        // Exact 16-byte key (2 full non-terminal chunks, empty suffix).
        let k16 = b"12345678abcdefgh";
        let s16 = map.ins_slot(tk(k16));
        // SAFETY: slot is valid until next mutation.
        unsafe {
            assert_eq!(*s16.as_ptr(), 0);
            *s16.as_ptr() = 160;
        }
        model.insert(k16.to_vec(), 160);
        assert_eq!(map.len(), model.len() as u64);
        assert_eq!(map.get(tk(k16)), Some(160));

        // 2. Terminal key with existing value 0 (must NOT be treated as absent).
        let term_zero_key = b"term0";
        // First ins_slot initializes to 0. We leave value as 0.
        let s_tz = map.ins_slot(tk(term_zero_key));
        // SAFETY: slot is valid until next mutation.
        unsafe {
            assert_eq!(*s_tz.as_ptr(), 0);
        }
        model.insert(term_zero_key.to_vec(), 0);
        assert_eq!(map.len(), model.len() as u64);
        // Second ins_slot on the same terminal key holding 0:
        // Must return slot to existing value 0 and NOT increment map.len().
        let s_tz2 = map.ins_slot(tk(term_zero_key));
        // SAFETY: slot is valid until next mutation.
        unsafe {
            assert_eq!(*s_tz2.as_ptr(), 0);
        }
        assert_eq!(map.len(), model.len() as u64);

        // 3. Forced suffix splits across shared prefixes (lengths 1..32).
        let base_prefix = b"prefix_split_test_shared_base_0123456789";
        for split_len in 1..=32 {
            let mut k = base_prefix[..split_len].to_vec();
            k.extend_from_slice(b"_branch_a");
            let s_a = map.ins_slot(tk(&k));
            // SAFETY: slot is valid until next mutation.
            unsafe {
                assert_eq!(*s_a.as_ptr(), 0);
                *s_a.as_ptr() = split_len as u64 * 10;
            }
            model.insert(k.clone(), split_len as u64 * 10);
            assert_eq!(map.len(), model.len() as u64);

            let mut k_b = base_prefix[..split_len].to_vec();
            k_b.extend_from_slice(b"_branch_b");
            let s_b = map.ins_slot(tk(&k_b));
            // SAFETY: slot is valid until next mutation.
            unsafe {
                assert_eq!(*s_b.as_ptr(), 0);
                *s_b.as_ptr() = split_len as u64 * 10 + 1;
            }
            model.insert(k_b.clone(), split_len as u64 * 10 + 1);
            assert_eq!(map.len(), model.len() as u64);

            assert_eq!(map.get(tk(&k)), Some(split_len as u64 * 10));
            assert_eq!(map.get(tk(&k_b)), Some(split_len as u64 * 10 + 1));
        }

        // 4. Random operations against model oracle (scaled for Miri).
        let ops = if cfg!(miri) { 50 } else { 4000 };
        let mut rng = XorShift(0x813_57A0_B123_4567 | 1);
        for _ in 0..ops {
            let k = keygen(&mut rng);
            let action = rng.next() % 5;
            match action {
                0..=2 => {
                    // ins_slot
                    let already_present = model.contains_key(&k);
                    let slot = map.ins_slot(tk(&k));
                    // SAFETY: slot is valid until next mutation.
                    let old_v = unsafe { *slot.as_ptr() };
                    if already_present {
                        let expected = *model.get(&k).unwrap();
                        assert_eq!(
                            old_v, expected,
                            "existing value must be preserved for {k:?}"
                        );
                    } else {
                        assert_eq!(old_v, 0, "new slot must be initialized to 0 for {k:?}");
                    }
                    // Write new value through the slot.
                    let new_val = rng.next();
                    // SAFETY: slot is valid until next mutation.
                    unsafe { *slot.as_ptr() = new_val };
                    model.insert(k.clone(), new_val);
                    assert_eq!(map.len(), model.len() as u64, "len mismatch after ins_slot");
                }
                3 => {
                    // get check
                    let got = map.get(tk(&k));
                    let want = model.get(&k).copied();
                    assert_eq!(got, want, "get mismatch for {k:?}");
                }
                _ => {
                    // remove
                    let got = map.remove(tk(&k));
                    let want = model.remove(&k);
                    assert_eq!(got, want, "remove mismatch for {k:?}");
                    assert_eq!(map.len(), model.len() as u64, "len mismatch after remove");
                }
            }
        }

        // Final verification: all entries in model match map.
        assert_eq!(map.len(), model.len() as u64);
        for (k, v) in &model {
            assert_eq!(map.get(tk(k)), Some(*v), "final map mismatch for {k:?}");
        }
    }
    /// #929: the `SyncExpanseStrMap` write path driven single-threaded, so
    /// Miri runs it (the `deferred` prefix is in the Tier-1 filter). Keys
    /// of at most seven bytes are terminal at the first chunk, so every
    /// mutation is T1 or T7 on the meta-trie root's sub-map — under the
    /// root cover taken as a lock while that sub-map is a root leaf, then
    /// through the engine's OLC bodies once it is a tree, and back — and
    /// the last removal empties the root, which is T10: a `RootGrowth`
    /// prune the serialised path takes.
    #[cfg(feature = "std")]
    #[test]
    fn deferred_olc_terminal_keys_in_leaf_and_tree_state() {
        use crate::set::ROOT_LEAF_CAP;
        use crate::sync::SyncExpanseStrMap;
        let m = SyncExpanseStrMap::new();
        let n = ROOT_LEAF_CAP * 3;
        let keys: Vec<Vec<u8>> = (0..n).map(|i| format!("k{i:05}").into_bytes()).collect();
        for (i, k) in keys.iter().enumerate() {
            assert_eq!(m.insert(tk(k), i as u64), None, "fresh insert of {k:?}");
        }
        assert_eq!(m.len(), n as u64);
        m.with_locked(|inner| {
            let root = inner.root.as_deref().expect("root present");
            assert!(
                root.map.root_is_tree(),
                "past ROOT_LEAF_CAP the root's sub-map is a tree"
            );
            assert!(
                root.cover.is_multiple_of(2),
                "cover even between operations"
            );
            for (i, k) in keys.iter().enumerate() {
                assert_eq!(inner.get(tk(k)), Some(i as u64));
            }
        });
        // T1 in tree state: a replace over a present key.
        for (i, k) in keys.iter().enumerate() {
            assert_eq!(
                m.insert(tk(k), 1000 + i as u64),
                Some(i as u64),
                "replace of {k:?}"
            );
        }
        // T7 on every other key while the sub-map is a tree.
        for k in keys.iter().step_by(2) {
            assert!(m.remove(tk(k)).is_some(), "remove of {k:?}");
        }
        assert_eq!(m.len(), (n - n.div_ceil(2)) as u64);
        for (i, k) in keys.iter().enumerate() {
            let want = if i.is_multiple_of(2) {
                None
            } else {
                Some(1000 + i as u64)
            };
            assert_eq!(m.get(tk(k)), want, "after the even removals, {k:?}");
        }
        // The rest, down through the root leaf and out (T10).
        for k in keys.iter().skip(1).step_by(2) {
            assert!(m.remove(tk(k)).is_some(), "remove of {k:?}");
        }
        assert_eq!(m.len(), 0);
        m.with_locked(|inner| {
            assert!(
                inner.root.is_none(),
                "T10: the emptied root was taken by the exclusive prune"
            );
            assert_eq!(inner.mem_used(), 0);
        });
        assert_eq!(m.get(tk(&keys[0])), None);
    }

    /// #929: the engine's optimistic inserts leave a sub-map's own
    /// population stale (`MapCore::tree_pop`; the wrapper counts in its
    /// sharded counter instead) and the node dirty, and the exclusive path
    /// restores it from a census fold before it reads it. That count is
    /// what the engine condenses a tree back to a root leaf from, so a
    /// stale one would size that leaf wrong — the failure the flag exists
    /// to prevent. Red when `resync` is deleted from the deferred twin.
    /// Under `ablation-str-serial-writers` no optimistic insert runs, so
    /// nothing is ever dirty and the test has nothing to show.
    #[cfg(all(feature = "std", not(feature = "ablation-str-serial-writers")))]
    #[test]
    fn deferred_olc_dirty_sub_map_is_resynced_before_the_exclusive_path_reads_it() {
        use crate::set::ROOT_LEAF_CAP;
        use crate::sync::SyncExpanseStrMap;
        let m = SyncExpanseStrMap::new();
        let n = ROOT_LEAF_CAP + 10;
        let keys: Vec<Vec<u8>> = (0..n).map(|i| format!("d{i:04}").into_bytes()).collect();
        for (i, k) in keys.iter().enumerate() {
            m.insert(tk(k), i as u64);
        }
        m.with_locked(|inner| {
            let root = inner.root.as_deref().expect("root present");
            assert!(root.map.root_is_tree());
            assert_eq!(
                root.dirty, 1,
                "an optimistic insert into a tree marks the node dirty"
            );
            assert!(
                root.map.len() < n as u64,
                "the sub-map's own count is stale until an exclusive re-sync: {} of {n}",
                root.map.len()
            );
        });
        // Exclusive removals down to a root-leaf population: the deferred
        // twin re-syncs first, and the engine condenses from the exact count.
        let keep = ROOT_LEAF_CAP - 5;
        m.with_locked_mut(|inner| {
            for k in keys.iter().skip(keep) {
                assert!(inner.remove(tk(k)).is_some());
            }
        });
        m.with_locked(|inner| {
            let root = inner.root.as_deref().expect("root present");
            assert_eq!(root.dirty, 0, "the re-sync cleared the flag");
            assert_eq!(root.map.len(), keep as u64);
            assert!(
                !root.map.root_is_tree(),
                "condensed to a root leaf from the exact count"
            );
            for (i, k) in keys.iter().enumerate().take(keep) {
                assert_eq!(inner.get(tk(k)), Some(i as u64));
            }
            assert_eq!(inner.len(), keep as u64);
        });
        assert_eq!(m.len(), keep as u64);
    }

    /// #929: the continuation transitions with the meta-trie root's
    /// sub-map in leaf state — T2 (a suffix published under the root
    /// cover), T4 (a split, twice, down to a terminal), T5, T3 (the
    /// in-place value replace), T8 (suffix removals) and T9 (the emptied
    /// children pruned under their own and their parent's cover), ending
    /// in T10 through the exclusive prune.
    #[cfg(feature = "std")]
    #[test]
    fn deferred_olc_continuation_transitions_in_leaf_state() {
        use crate::sync::SyncExpanseStrMap;
        let m = SyncExpanseStrMap::new();
        let one = b"prefix_aaaaaaaa_one";
        let two = b"prefix_aaaaaaaa_two";
        let three = b"prefix_bbbbbbbb_one";
        let short = b"prefix_aaaaaaaa";
        // T11 through the fallback, then T2 at the root.
        assert_eq!(m.insert(tk(one), 1), None);
        // T4 at the root and again one level down, then T1 in the grandchild.
        assert_eq!(m.insert(tk(two), 2), None);
        // T3 in the grandchild.
        assert_eq!(m.insert(tk(one), 11), Some(1));
        // T2 at the root beside the child; T1 in the child.
        assert_eq!(m.insert(tk(three), 3), None);
        assert_eq!(m.insert(tk(short), 4), None);
        assert_eq!(m.len(), 4);
        for (k, v) in [(&one[..], 11u64), (two, 2), (three, 3), (short, 4)] {
            assert_eq!(m.get(tk(k)), Some(v), "{k:?}");
        }
        // T7 and T8 in the grandchild empty it: T9 prunes it from the child.
        assert_eq!(m.remove(tk(one)), Some(11));
        assert_eq!(m.remove(tk(two)), Some(2));
        assert_eq!(m.len(), 2);
        assert_eq!(m.get(tk(short)), Some(4));
        assert_eq!(m.get(tk(three)), Some(3));
        assert_eq!(m.get(tk(one)), None);
        // The child empties: pruned from the root, which keeps `three`.
        assert_eq!(m.remove(tk(short)), Some(4));
        assert_eq!(m.get(tk(three)), Some(3));
        // The root empties: T10, the exclusive prune takes it.
        assert_eq!(m.remove(tk(three)), Some(3));
        assert_eq!(m.len(), 0);
        m.with_locked(|inner| {
            assert!(inner.root.is_none(), "T10 through the exclusive prune");
            assert_eq!(inner.mem_used(), 0);
        });
        assert_eq!(m.insert(tk(one), 5), None);
        assert_eq!(m.get(tk(one)), Some(5));
    }

    /// #929: the same continuation transitions with the root's sub-map in
    /// tree state, so the entry stores go through the engine's OLC bodies:
    /// T2 with the insert-if-absent mode, T4's publish as an engine
    /// replace under the cover lock, T8 as an engine remove under it, and
    /// T9 with a tree-state parent, where the child's entry leaves the
    /// parent through the engine — and, near the end, through the
    /// engine's own fallbacks and the exclusive prune.
    #[cfg(feature = "std")]
    #[test]
    fn deferred_olc_continuation_transitions_in_tree_state() {
        use crate::set::ROOT_LEAF_CAP;
        use crate::sync::SyncExpanseStrMap;
        let m = SyncExpanseStrMap::new();
        let n = ROOT_LEAF_CAP * 2;
        let a: Vec<Vec<u8>> = (0..n)
            .map(|i| format!("p{i:07}-suffix-{i:03}").into_bytes())
            .collect();
        let b: Vec<Vec<u8>> = (0..n)
            .map(|i| format!("p{i:07}-other-{i:03}").into_bytes())
            .collect();
        for (i, k) in a.iter().enumerate() {
            assert_eq!(m.insert(tk(k), i as u64), None);
        }
        m.with_locked(|inner| {
            assert!(inner.root.as_deref().expect("root").map.root_is_tree());
        });
        // Each `b` key diverges from its `a` twin past the first chunk: a
        // split in tree state, then a leaf-state insert in the child.
        for (i, k) in b.iter().enumerate() {
            assert_eq!(m.insert(tk(k), 100 + i as u64), None);
        }
        assert_eq!(m.len(), 2 * n as u64);
        for (i, k) in a.iter().enumerate() {
            assert_eq!(m.get(tk(k)), Some(i as u64), "{k:?}");
            assert_eq!(m.get(tk(&b[i])), Some(100 + i as u64), "{:?}", b[i]);
            assert_eq!(
                m.insert(tk(k), 200 + i as u64),
                Some(i as u64),
                "T3 on {k:?}"
            );
        }
        for (i, k) in b.iter().enumerate() {
            assert_eq!(m.remove(tk(k)), Some(100 + i as u64), "T8 on {k:?}");
            assert_eq!(
                m.get(tk(&a[i])),
                Some(200 + i as u64),
                "{:?} survives",
                a[i]
            );
        }
        assert_eq!(m.len(), n as u64);
        // Emptying every child prunes it from the tree-state root.
        for (i, k) in a.iter().enumerate() {
            assert_eq!(m.remove(tk(k)), Some(200 + i as u64), "T8 then T9 on {k:?}");
        }
        assert_eq!(m.len(), 0);
        m.with_locked(|inner| {
            assert!(inner.root.is_none(), "the emptied root was taken");
            assert_eq!(inner.mem_used(), 0);
        });
    }
}
