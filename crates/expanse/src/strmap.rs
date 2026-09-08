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
use crate::map::MapCore;
#[cfg(feature = "std")]
use crate::occ::Collector;
use core::alloc::Layout;
use core::ptr::NonNull;
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

/// Allocates a suffix leaf holding `bytes` and `value` in one allocation.
fn new_suffix(bytes: &[u8], value: u64) -> *mut StrSuffix {
    let layout = suffix_layout(bytes.len());
    // SAFETY: `layout` has non-zero size — the header alone is 16 bytes.
    let raw = unsafe { core_alloc::alloc::alloc(layout) };
    if raw.is_null() {
        core_alloc::alloc::handle_alloc_error(layout);
    }
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

/// One trie level: a word-map core over the next 8-byte chunk.
///
/// Deliberately just the engine core (issue #363 Step A): the backing
/// allocator is the string map's **single shared [`NodeAlloc`]**, passed
/// in per call, and there is no per-node insert-path cache — which is
/// what shrinks a node from ~700 bytes (embedded allocator + path cache)
/// to the bare root word, so a descent chain stays cache-resident.
/// `StrNode` has no `Drop`: teardown must route through
/// [`dispose_node`]/[`dispose_tree`] with the shared allocator.
struct StrNode {
    map: MapCore,
}

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
fn dispose_suffix(ptr: *mut StrSuffix, defer: DeferHandle<'_>) {
    // SAFETY: the caller unlinked `ptr` and this is the last owner, so the
    // header is still live and `len` — write-once since publication — still
    // describes the allocation `new_suffix` made.
    let layout = suffix_layout(unsafe { (*ptr).len });
    #[cfg(feature = "std")]
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
    #[cfg(not(feature = "std"))]
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
                    dispose_suffix(unpack_suffix(v), defer);
                } else {
                    stack.push(unpack_child(v));
                }
            }
        }
        dispose_node(p, alloc, defer);
    }
}

/// Packs `key[off..]`'s next chunk big-endian; `true` when the chunk is
/// terminal, which is [`is_terminal`]'s definition and no other.
///
/// The flag used to be `rest.len() < CHUNK` -- a *length* rule, where every
/// read of a stored entry uses `is_terminal`'s *content* rule. The two agree
/// on the NUL-free key domain and disagree outside it, and that disagreement
/// was the whole of #794: a key whose NUL fell inside a chunk was written by
/// one rule and read by the other, so a suffix pointer could be published
/// where a later read expected a terminal value, and a stored value could be
/// read where a later descent expected a pointer. `get` and `remove`
/// segfaulted on the second of those in a release build.
///
/// The rules are now one rule, so the discriminant is a total function of the
/// chunk rather than of the key that produced it, and there is nothing left to
/// disagree. This is a deletion, not an added condition: the padding above
/// means `rest.len() < CHUNK` *implies* `is_terminal(chunk)`, so the old flag
/// was the new one plus a false-negative case.
///
/// Out-of-domain keys become defined rather than undefined. They alias: the
/// chunk stops at the first NUL, so `b"ab\0..."` reaches the entry `b"ab"`
/// owns, and the ordered surface reconstructs a truncated key. That is lossy
/// and documented, and `ExpanseStrMap::assert_key` still rejects it in debug
/// builds. It is not memory-unsafe, which is the property that matters here.
fn chunk_at(key: &[u8], off: usize) -> (u64, bool) {
    let rest = &key[off.min(key.len())..];
    let mut c = [0u8; CHUNK];
    let n = rest.len().min(CHUNK);
    c[..n].copy_from_slice(&rest[..n]);
    let chunk = u64::from_be_bytes(c);
    (chunk, is_terminal(chunk))
}

/// The byte content of a terminal chunk (bytes before the NUL).
fn terminal_bytes(chunk: u64) -> impl Iterator<Item = u8> {
    chunk.to_be_bytes().into_iter().take_while(|&b| b != 0)
}

/// True when the chunk contains a NUL byte (terminal entry).
///
/// SWAR haszero rather than `to_be_bytes().contains(&0)`. The three-operation
/// form is branchless and byte-order independent -- it asks whether any lane
/// borrowed, which does not depend on which end the lanes came from.
///
/// No codegen claim is made for the form it replaced: §8.7 wants an
/// `--emit asm` citation for one and there is none. The substitution has not
/// been shown to pay for itself either -- every call site below is on the
/// ordered-navigation and teardown paths, and no benchmark arm covers them
/// (§6's benchmark-arm prerequisite), so its effect is unmeasured.
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

/// Publishes a suffix leaf under `chunk`, refusing to do so at a chunk that
/// [`is_terminal`] will later read as terminal.
///
/// The check and the publication are one operation on purpose. This module
/// decides "terminal" two ways -- [`chunk_at`] by length, `is_terminal` by
/// content -- and they coincide only on the NUL-free key domain. A key whose
/// NUL falls inside a chunk takes the non-terminal path here while every
/// later read treats the entry as terminal, so the ordered surface hands the
/// tagged pointer back as the caller's value slot and the block is leaked
/// where the byte accounting cannot see it (#794).
///
/// It is a release assertion, not a `debug_assert!`. [`ExpanseStrMap::assert_key`]
/// already rejects the key in debug builds, so a debug-only check here would
/// be unreachable; the case this guards is precisely the one no build
/// currently catches.
///
/// It does not close #794. The guard fires only where a *fresh* leaf is
/// published, and the disagreement reaches a wild pointer by two other routes
/// that publish nothing: an out-of-domain key whose first chunk collides with
/// an existing terminal entry makes the descent read the caller's own value
/// word and, when its low bit happens to be set, treat it as a tagged suffix
/// pointer. `get` and `remove` both segfault on that in a release build, and
/// `get_validated` takes the same shape inside the seqlock bracket.
///
/// Its cost is measured and disclosed on the pull request, and is *not*
/// attributed: the arm deltas have opposite signs on the two architectures,
/// which is codegen, not arithmetic. Callgrind counts retired instructions,
/// so the cold panic path contributes nothing to them.
fn publish_suffix(
    node: &mut StrNode,
    alloc: &NodeAlloc,
    chunk: u64,
    suffix: *mut StrSuffix,
) -> Option<u64> {
    debug_assert!(
        !is_terminal(chunk),
        "suffix pointer published at a terminal chunk: keys must be NUL-free"
    );
    node.map.insert_pathless(alloc, chunk, pack_suffix(suffix))
}

impl StrNode {
    fn new() -> Self {
        Self {
            map: MapCore::new(),
        }
    }

    /// # Safety
    ///
    /// `v` must be a continuation child pointer produced by `Box::into_raw`.
    unsafe fn child_mut<'a>(v: u64) -> &'a mut StrNode {
        // SAFETY: per contract, `v` is a live Box<StrNode> pointer.
        unsafe { &mut *unpack_child(v) }
    }

    /// Largest entry in this subtree; appends its key bytes to `out`.
    fn max_entry(&mut self, out: &mut Vec<u8>) -> NonNull<u64> {
        self.extreme_entry(out, false)
    }

    /// The subtree's first (`min`) or last (`!min`) entry.
    ///
    /// Iterative, like every other walk in this module: one frame per 8
    /// key bytes turns a long key into a stack overflow, and this runs
    /// down the deepest chain in the tree by construction. The two
    /// directions differ only in which end of each node they take, so
    /// they share a walk.
    fn extreme_entry(&mut self, out: &mut Vec<u8>, min: bool) -> NonNull<u64> {
        let mut node: *mut StrNode = &raw mut *self;
        loop {
            // SAFETY: `self` on the first turn, then continuation values,
            // which are live child nodes; the descent never revisits one,
            // so the borrow is unique.
            let n = unsafe { &mut *node };
            let (chunk, v) = if min {
                n.map.first().expect("non-empty node")
            } else {
                n.map.last().expect("non-empty node")
            };
            if is_terminal(chunk) {
                out.extend(terminal_bytes(chunk));
                return n.map.value_slot_pathless(chunk).expect("present chunk");
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
        &mut self,
        cursor: Option<(u64, u64)>,
        out: &mut Vec<u8>,
        min: bool,
    ) -> Option<NonNull<u64>> {
        let (chunk, v) = cursor?;
        if is_terminal(chunk) {
            out.extend(terminal_bytes(chunk));
            Some(self.map.value_slot_pathless(chunk).expect("present chunk"))
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
            Some(unsafe { Self::child_mut(v) }.extreme_entry(out, min))
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
        &mut self,
        key: &[u8],
        off: usize,
        out: &mut Vec<u8>,
    ) -> Option<NonNull<u64>> {
        // (node to resume at, its target chunk, `out` length on entry)
        let mut stack: Vec<(*mut StrNode, u64, usize)> = Vec::new();
        let mut node: *mut StrNode = &raw mut *self;
        let mut off = off;

        loop {
            // SAFETY: `self`, then continuation values — all live nodes,
            // uniquely borrowed because the descent never revisits one.
            let n = unsafe { &mut *node };
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
                // SAFETY: recorded during the descent; still live, and
                // the child borrow taken from it has ended.
                let p = unsafe { &mut *parent };
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
        &mut self,
        key: &[u8],
        off: usize,
        exclusive: bool,
        out: &mut Vec<u8>,
    ) -> Option<NonNull<u64>> {
        let mut stack: Vec<(*mut StrNode, u64, usize)> = Vec::new();
        let mut node: *mut StrNode = &raw mut *self;
        let mut off = off;

        loop {
            // SAFETY: as in `next_at_or_after`.
            let n = unsafe { &mut *node };
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
                let p = unsafe { &mut *parent };
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

    /// Removes `key[off..]`; returns the removed value. Empty child nodes
    /// are pruned on the way out (disposal routed through `defer` — see
    /// [`dispose_suffix`]/[`dispose_node`]).
    /// Iterative: descend recording the path, then prune emptied nodes on
    /// the way back out. Recursing costs a frame per 8 key bytes, which a
    /// long key turns into a stack overflow.
    fn remove(
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
                break n.map.remove_pathless(alloc, chunk)?;
            }
            let v = n.map.get(chunk)?;
            if is_suffix_ptr(v) {
                let sfx = unpack_suffix(v);
                let rem = &key[off + CHUNK..];
                // SAFETY: live suffix leaf, raw provenance over the bytes.
                if rem == unsafe { suffix_bytes(sfx) } {
                    n.map.remove_pathless(alloc, chunk);
                    // Read out before disposal: the borrow of the bytes above
                    // has ended, and nothing may reference the block once
                    // `dispose_suffix` has it.
                    // SAFETY: unlinked but still live; last owner.
                    let removed_val = unsafe { (*sfx).value };
                    // Unlinked above; retired when shared.
                    dispose_suffix(sfx, defer);
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
        // nothing above that can have been emptied by this removal.
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
                parent.map.remove_pathless(alloc, chunk);
                // Unlinked above; retired when shared.
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
                        // exactly what `dispose_suffix` will hand back.
                        bytes += suffix_layout(len).size() as u64;
                    } else {
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
struct StrFrame {
    node: *mut StrNode,
    chunk: u64,
    key_len: usize,
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
    /// The path from the root to the entry last emitted. Empty before the
    /// first `next` and after the walk is exhausted.
    stack: Vec<StrFrame>,
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
    /// Held for the borrow, and to keep the root reachable.
    map: &'a mut ExpanseStrMap,
}

impl<'a> StrCursor<'a> {
    fn new(map: &'a mut ExpanseStrMap) -> Self {
        Self {
            stack: Vec::new(),
            key: Vec::new(),
            done: false,
            started: false,
            pending: None,
            map,
        }
    }

    /// Descends from `node` taking the smallest entry at every level until an
    /// entry that *is* a value is reached, pushing a frame per level.
    ///
    /// `entry` is the entry to take at `node`; every level below takes its
    /// `first`. Returns the value slot, or `None` for an empty node, which a
    /// well-formed trie does not contain below the root.
    fn descend(&mut self, mut node: *mut StrNode, mut entry: (u64, u64)) -> Option<NonNull<u64>> {
        loop {
            let (chunk, v) = entry;
            let key_len = self.key.len();
            self.stack.push(StrFrame {
                node,
                chunk,
                key_len,
            });
            if is_terminal(chunk) {
                self.key.extend(terminal_bytes(chunk));
                // SAFETY: `node` is a live node on the path just walked, and
                // the chunk came from its own map, so the slot is present.
                return unsafe { &mut *node }.map.value_slot_pathless(chunk);
            }
            self.key.extend_from_slice(&chunk.to_be_bytes());
            if is_suffix_ptr(v) {
                let sfx = unpack_suffix(v);
                // SAFETY: tagged pointer encodes a live suffix leaf; the raw
                // pointer carries provenance over the inline bytes.
                self.key.extend_from_slice(unsafe { suffix_bytes(sfx) });
                // SAFETY: field-precise pointer to the value word at offset 0.
                return Some(
                    NonNull::new(unsafe { &raw mut (*sfx).value }).expect("non-null value slot"),
                );
            }
            node = unpack_child(v);
            // SAFETY: untagged continuation value, a live child node.
            entry = unsafe { &*node }.map.first()?;
        }
    }

    /// Advances to the next entry in byte-lexicographic order.
    ///
    /// The returned key borrows the cursor's own buffer and is valid until the
    /// next call; copy it if it must outlive that. The slot follows the
    /// surrounding surface's contract and stays valid until the map is
    /// structurally mutated, which the cursor's borrow prevents.
    #[allow(clippy::should_implement_trait)] // lending: the key borrows `self`
    pub fn next(&mut self) -> Option<(&[u8], NonNull<u64>)> {
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
        // Unwind to the nearest level with an unexplored sibling, dropping the
        // key bytes each abandoned level contributed.
        while let Some(frame) = self.stack.pop() {
            self.key.truncate(frame.key_len);
            // SAFETY: recorded during the descent and still live; the borrow
            // taken from it in `descend` has ended.
            let sibling = unsafe { &*frame.node }.map.next_after(frame.chunk);
            if let Some(entry) = sibling {
                let slot = self.descend(frame.node, entry);
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
                    self.stack.push(StrFrame {
                        node,
                        chunk,
                        key_len: self.key.len(),
                    });
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
                // SAFETY: recorded during this descent; still live.
                let p = unsafe { &*frame.node };
                if let Some(entry) = p.map.next_after(frame.chunk) {
                    return self.descend(frame.node, entry);
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

    /// Debug-only check of the NUL-free key domain documented at the top of
    /// this module.
    ///
    /// It is `debug_assert!` rather than `assert!` deliberately: every caller
    /// below is on a Callgrind-gated descent path, and a byte scan per key
    /// would be paid by every correct caller to catch an incorrect one. The
    /// cost belongs at the boundary where an untrusted string actually enters
    /// -- the language bindings, which reject an embedded NUL before the key
    /// reaches here.
    ///
    /// What a release build does with an out-of-domain key, since this
    /// assertion is compiled out of it and the behaviour is not obvious: the
    /// key is stored, counted by [`len`](Self::len), and returned by
    /// [`get`](Self::get). What it is *not* is addressable by the ordered
    /// surface. A trailing NUL is how the encoding terminates a string, so
    /// `next_at_or_after(b"abc\0X")` answers with `"abc"` -- a different and
    /// smaller key than the one asked for.
    ///
    /// For a key whose NUL falls *inside* the first chunk the consequence is
    /// worse than a disagreement, because the module decides "terminal" two
    /// ways that coincide only on this domain: [`chunk_at`] decides by length
    /// (`rest.len() < CHUNK`) and [`is_terminal`] by content (the chunk holds
    /// a zero byte). `b"abc\0defghij"` is eleven bytes, so `insert` takes the
    /// non-terminal path and publishes a [`pack_suffix`] pointer at a chunk
    /// every later `is_terminal` reads as terminal. The ordered surface then
    /// takes its terminal branch over that entry and hands back the word
    /// holding the tagged pointer *as the caller's value slot* -- a `*mut
    /// Word` a JudySL caller is contracted to write through, which corrupts
    /// the edge and leaves the next [`get`] dereferencing a wild address. The
    /// suffix block is leaked in the same breath: `dispose_tree` and
    /// `shell_bytes` skip it under that same `is_terminal` test, so they agree
    /// with each other and the `bytes_in_use() == 0` accounting invariant
    /// still passes. That is the reason the domain is a contract and not a
    /// preference. Note that the publication sites are not the whole of it:
    /// the same disagreement reaches a wild pointer through `get` and
    /// `remove`, which publish nothing, when an out-of-domain key's first
    /// chunk collides with an existing terminal entry and its stored value
    /// has the low bit set. Guarding publication alone does not close it.
    fn assert_key(key: &[u8]) {
        debug_assert!(!key.contains(&0), "keys are NUL-free byte strings");
    }

    /// Splits a suffix entry that diverges from the key being inserted:
    /// builds a child node holding the existing suffix's continuation,
    /// publishes it over the suffix's map entry, and disposes of the old
    /// suffix (retired when shared — a concurrent reader may still hold
    /// it). Returns the raw child for the caller to descend into.
    ///
    /// Reads `old` only through short-lived internal borrows that all end
    /// before the disposal: a borrow passed in as a parameter would be
    /// *protected* for the whole call and still be live when the block is
    /// handed to `dispose_suffix`.
    fn split_suffix(
        node: &mut StrNode,
        chunk: u64,
        old: *mut StrSuffix,
        alloc: &NodeAlloc,
        defer: DeferHandle<'_>,
    ) -> *mut StrNode {
        let mut child = Box::new(StrNode::new());
        // SAFETY: `old` is the live suffix being split, reached as a raw
        // pointer so the projection covers the inline bytes; both borrows
        // end before the disposal below.
        let ((c1, t1), value) = unsafe { (chunk_at(suffix_bytes(old), 0), (*old).value) };
        if t1 {
            child.map.insert_pathless(alloc, c1, value);
        } else {
            // The continuation bytes are copied into the new leaf here, so
            // the borrow of `old` is over before it is disposed of.
            // SAFETY: as above.
            let s1 = unsafe { new_suffix(&suffix_bytes(old)[CHUNK..], value) };
            publish_suffix(&mut child, alloc, c1, s1);
        }
        let child_raw = Box::into_raw(child);
        node.map
            .insert_pathless(alloc, chunk, pack_child(child_raw));
        dispose_suffix(old, defer);
        child_raw
    }

    /// Inserts `key → val`; returns the replaced value if present.
    pub fn insert(&mut self, key: &[u8], val: u64) -> Option<u64> {
        Self::assert_key(key);
        let defer = self.defer_handle();
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
                let prev = node.map.insert_pathless(alloc, chunk, val);
                if prev.is_none() {
                    self.pop += 1;
                }
                return prev;
            }
            match node.map.get(chunk) {
                None => {
                    let suffix = new_suffix(&key[off + CHUNK..], val);
                    publish_suffix(node, alloc, chunk, suffix);
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
                        // the version bracket when shared.
                        // SAFETY: exclusive writer; a racing reader's load
                        // is discarded unless its snapshot validates.
                        return Some(unsafe { core::ptr::replace(&raw mut (*sfx).value, val) });
                    }
                    let child_raw = Self::split_suffix(node, chunk, sfx, alloc, defer.as_ref());
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
    pub fn ins_slot(&mut self, key: &[u8]) -> NonNull<u64> {
        Self::assert_key(key);
        let defer = self.defer_handle();
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
                if !node.map.contains_key(chunk) {
                    self.pop += 1;
                }
                return node.map.ins_slot_pathless(alloc, chunk);
            }
            match node.map.get(chunk) {
                None => {
                    let suffix = new_suffix(&key[off + CHUNK..], 0);
                    publish_suffix(node, alloc, chunk, suffix);
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
                    let child_raw = Self::split_suffix(node, chunk, sfx, alloc, defer.as_ref());
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
    pub fn get(&self, key: &[u8]) -> Option<u64> {
        Self::assert_key(key);
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
    /// Every hop's sub-map walk (`sync::walk_validated`) starts by
    /// validating the shared tree version, so the multi-hop **path
    /// prefix** is consistent with the map state at `snap`; the terminal
    /// hop's value itself is covered hand-over-hand by per-node versions
    /// (exactly [`crate::sync::SyncExpanseMap`]'s read semantics — the
    /// result is a value the key held during the call, linearizable
    /// because sub-tries are never re-parented and unlink always precedes
    /// retirement). A hop that races a writer fails validation and
    /// surfaces as `Retry`. Suffix leaves carry no per-node version, so
    /// that arm re-validates the tree version before returning; their fat
    /// pointer and byte buffer are write-once after publication (splits
    /// publish a replacement and retire the old suffix; only the value
    /// word mutates in place).
    ///
    /// # Safety
    ///
    /// Same contract as `sync::walk_validated`: `snap` must be an even
    /// version sampled from `ver` after this map switched to deferred
    /// reclamation ([`Self::defer_to`]), and the caller must hold an epoch
    /// pin for the whole call — every pointer read under a still-valid
    /// cover then references EBR-live memory.
    #[cfg(feature = "std")]
    pub(crate) unsafe fn get_validated(
        &self,
        key: &[u8],
        ver: &crate::occ::SeqVersion,
        snap: u64,
    ) -> Result<Option<u64>, crate::sync::Retry> {
        use crate::sync::Retry;
        Self::assert_key(key);
        // Racy single-word copy of the root pointer; the first sub-map
        // walk's validation covers it before anything read through it is
        // used (and a stale-but-retired root stays EBR-live under the pin).
        let mut node: *const StrNode = match self.root.as_deref() {
            Some(r) => core::ptr::from_ref(r),
            None => {
                return if ver.validate(snap) {
                    Ok(None)
                } else {
                    Err(Retry)
                };
            }
        };
        let mut off = 0usize;
        loop {
            let (chunk, terminal) = chunk_at(key, off);
            // SAFETY: `node` was validated at `snap` (the root by the walk's
            // first check below; children by the previous hop's validated
            // walk) and is EBR-live under the caller's pin. The possibly
            // racy root-snapshot copy is validated before use.
            let msnap = unsafe { (*node).map.occ_snapshot() };
            // SAFETY: the caller's pin + snapshot contract carries through.
            let found = unsafe { crate::sync::walk_validated::<true>(msnap, chunk, ver, snap) }?;
            if terminal {
                return Ok(found);
            }
            let Some(v) = found else { return Ok(None) };
            if is_suffix_ptr(v) {
                let sfx: *const StrSuffix = unpack_suffix(v);
                // SAFETY: `v` was validated at `snap`, so `sfx` was the
                // published suffix then, and EBR keeps the whole block —
                // header and inline bytes, now one allocation — mapped under
                // the pin. `len` and the bytes are write-once; the value word
                // may race and is validated below before use. Both reads
                // project from the raw pointer: a `&StrSuffix` would not
                // carry provenance over the bytes past the header.
                let (bytes, value) = unsafe { (suffix_bytes(sfx), (*sfx).value) };
                let matched = bytes == &key[off + CHUNK..];
                if !ver.validate(snap) {
                    return Err(Retry);
                }
                return Ok(matched.then_some(value));
            }
            if !ver.validate(snap) {
                return Err(Retry);
            }
            node = unpack_child(v);
            off += CHUNK;
        }
    }

    /// Returns a writable pointer to `key`'s value slot (compat:
    /// `JudySLGet`), or `None` if absent.
    #[must_use]
    pub fn get_value_slot(&mut self, key: &[u8]) -> Option<NonNull<u64>> {
        Self::assert_key(key);
        let mut node = self.root.as_deref_mut()?;
        let mut off = 0;
        loop {
            let (chunk, terminal) = chunk_at(key, off);
            if terminal {
                return node.map.value_slot_pathless(chunk);
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
            node = unsafe { StrNode::child_mut(v) };
            off += CHUNK;
        }
    }

    /// Removes `key`; returns its value if it was present.
    pub fn remove(&mut self, key: &[u8]) -> Option<u64> {
        Self::assert_key(key);
        let defer = self.defer_handle();
        let alloc = &self.alloc;
        let root = self.root.as_deref_mut()?;
        let removed = root.remove(key, 0, alloc, defer.as_ref())?;
        self.pop -= 1;
        if root.map.is_empty() {
            let root_box = self.root.take().expect("root present");
            // Unlinked (the root slot is cleared); retired when shared.
            dispose_node(Box::into_raw(root_box), alloc, defer.as_ref());
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
        StrCursor::new(self)
    }

    /// An ordered cursor positioned at the first key `>= key`, inclusive.
    ///
    /// The seek is the one descent a bounded range scan needs; every
    /// subsequent element is a step along the path it recorded.
    #[must_use]
    pub fn cursor_at_or_after(&mut self, key: &[u8]) -> StrCursor<'_> {
        Self::assert_key(key);
        let mut c = StrCursor::new(self);
        c.pending = c.seek(key);
        c
    }

    /// Smallest entry with key `>= key`: `(key bytes, value slot)`
    /// (compat: `JudySLFirst`).
    pub fn next_at_or_after(&mut self, key: &[u8]) -> Option<(Vec<u8>, NonNull<u64>)> {
        Self::assert_key(key);
        let root = self.root.as_deref_mut()?;
        let mut out = Vec::with_capacity(key.len() + CHUNK);
        let slot = root.next_at_or_after(key, 0, &mut out)?;
        Some((out, slot))
    }

    /// Smallest entry with key `> key` (compat: `JudySLNext`). The
    /// immediate successor of a NUL-free string is itself + `0x01`.
    pub fn next_after(&mut self, key: &[u8]) -> Option<(Vec<u8>, NonNull<u64>)> {
        let mut succ = Vec::with_capacity(key.len() + 1);
        succ.extend_from_slice(key);
        succ.push(1);
        self.next_at_or_after(&succ)
    }

    /// Largest entry with key `<= key` (compat: `JudySLLast`).
    pub fn prev_at_or_before(&mut self, key: &[u8]) -> Option<(Vec<u8>, NonNull<u64>)> {
        Self::assert_key(key);
        let root = self.root.as_deref_mut()?;
        let mut out = Vec::with_capacity(key.len() + CHUNK);
        let slot = root.prev_at_or_before(key, 0, false, &mut out)?;
        Some((out, slot))
    }

    /// Largest entry with key `< key` (compat: `JudySLPrev`).
    pub fn prev_before(&mut self, key: &[u8]) -> Option<(Vec<u8>, NonNull<u64>)> {
        Self::assert_key(key);
        let root = self.root.as_deref_mut()?;
        let mut out = Vec::with_capacity(key.len() + CHUNK);
        let slot = root.prev_at_or_before(key, 0, true, &mut out)?;
        Some((out, slot))
    }

    /// Smallest entry.
    pub fn first(&mut self) -> Option<(Vec<u8>, NonNull<u64>)> {
        self.next_at_or_after(&[])
    }

    /// Largest entry.
    pub fn last(&mut self) -> Option<(Vec<u8>, NonNull<u64>)> {
        let root = self.root.as_deref_mut()?;
        let mut out = Vec::new();
        let slot = root.max_entry(&mut out);
        Some((out, slot))
    }

    /// Removes every entry; returns the heap bytes released (the compat
    /// `JudySLFreeArray` return value).
    pub fn clear(&mut self) -> u64 {
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
}

impl Default for ExpanseStrMap {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ExpanseStrMap {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
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
            m.insert(k, i as u64);
        }

        let mut positional: Vec<(Vec<u8>, u64)> = Vec::new();
        let mut cur = m.first();
        while let Some((k, slot)) = cur {
            // SAFETY: slot is live until the next structural mutation, and
            // this walk performs none.
            positional.push((k.clone(), unsafe { *slot.as_ptr() }));
            cur = m.next_after(&k);
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

    /// Seeking lands where `next_at_or_after` lands, for keys that are
    /// present, absent, shorter and longer than what is stored, and past the
    /// end.
    #[test]
    fn cursor_seek_matches_next_at_or_after() {
        let keys = walk_corpus();
        let mut m = ExpanseStrMap::new();
        for (i, k) in keys.iter().enumerate() {
            m.insert(k, i as u64);
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
            let expected = m.next_at_or_after(&probe).map(|(k, slot)| {
                // SAFETY: no structural mutation between here and the read.
                (k, unsafe { *slot.as_ptr() })
            });
            let mut c = m.cursor_at_or_after(&probe);
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
            m.insert(k, i as u64);
        }
        let start = &keys[keys.len() / 3];
        let expected: Vec<Vec<u8>> = keys.iter().skip(keys.len() / 3).cloned().collect();

        let mut got: Vec<Vec<u8>> = Vec::new();
        let mut c = m.cursor_at_or_after(start);
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
            map.insert(k, k.len() as u64);
        }
        let mut c = map.cursor_at_or_after(b"ap");
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
            m.insert(k, i as u64);
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
        let mut c = m.cursor_at_or_after(b"aaaaaaaaBBBBBBBBcc");
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

    /// The guard that refuses to publish a suffix pointer at a chunk every
    /// later read treats as terminal (#794).
    ///
    /// Driven at `publish_suffix` rather than through `insert`, deliberately.
    /// `assert_key` rejects a NUL-bearing key at the entry of `insert` in any
    /// build with debug assertions on -- which is every build this suite runs
    /// in -- so a test through the public API would panic at the upstream
    /// guard and pass while saying nothing about this one (AGENTS.md §5).
    ///
    /// The check lives inside the function that publishes, so there is no
    /// call site that can drop it: `pack_suffix` is reachable from nowhere
    /// else in the module.
    #[test]
    #[should_panic(expected = "suffix pointer published at a terminal chunk")]
    fn a_suffix_pointer_is_never_published_at_a_terminal_chunk() {
        let mut node = StrNode::new();
        let alloc = NodeAlloc::new();
        // Eight bytes, so `chunk_at` would call this non-terminal, while
        // `is_terminal` reads the embedded NUL and calls it terminal -- the
        // exact disagreement. The pointer is never dereferenced or stored:
        // the assertion fires before `pack_suffix`, so nothing is leaked.
        let chunk = u64::from_be_bytes(*b"abc\0defg");
        publish_suffix(&mut node, &alloc, chunk, NonNull::dangling().as_ptr());
    }

    /// Degenerate shapes: empty map, one key, and a cursor driven past the
    /// end, which must keep returning `None` rather than restarting.
    #[test]
    fn cursor_edges() {
        let mut empty = ExpanseStrMap::new();
        assert!(empty.cursor().next().is_none());
        assert!(empty.cursor_at_or_after(b"anything").next().is_none());

        let mut one = ExpanseStrMap::new();
        one.insert(b"solo", 7);
        let mut c = one.cursor();
        assert_eq!(c.next().map(|(k, _)| k.to_vec()), Some(b"solo".to_vec()));
        assert!(c.next().is_none());
        assert!(c.next().is_none(), "an exhausted cursor restarted");

        let mut past = one.cursor_at_or_after(b"zzz");
        assert!(past.next().is_none());
    }

    /// The value slot the cursor hands out is the map's own, not a copy.
    #[test]
    fn cursor_slots_are_writable_in_place() {
        let mut m = ExpanseStrMap::new();
        for k in [b"alpha".as_slice(), b"beta", b"gamma"] {
            m.insert(k, 0);
        }
        let mut c = m.cursor();
        while let Some((_, slot)) = c.next() {
            // SAFETY: the slot is the map's value word, live for the borrow.
            unsafe { *slot.as_ptr() = 42 };
        }
        for k in [b"alpha".as_slice(), b"beta", b"gamma"] {
            assert_eq!(m.get(k), Some(42), "in-place write through the cursor lost");
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
                assert_eq!(m.insert(&key, 1), None);
                assert_eq!(m.get(&key), Some(1));
                // A second key sharing most of the chain, so teardown has
                // branching to walk rather than one straight line.
                let mut other = key.clone();
                *other.last_mut().expect("non-empty") = b'z';
                assert_eq!(m.insert(&other, 2), None);
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
                m.insert(&key, 7);
                assert_eq!(m.remove(&key), Some(7));
                assert!(m.is_empty());
                assert_eq!(m.get(&key), None);
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
                m.insert(&deep, 1);
                m.insert(&sibling, 2);

                assert_eq!(m.first().map(|(k, _)| k), Some(deep.clone()));
                assert_eq!(m.last().map(|(k, _)| k), Some(sibling.clone()));
                assert_eq!(m.get(&deep), Some(1));
                assert_eq!(m.get(&sibling), Some(2));
                assert_eq!(
                    m.next_at_or_after(&deep).map(|(k, _)| k),
                    Some(deep.clone())
                );
                assert_eq!(m.next_after(&deep).map(|(k, _)| k), Some(sibling.clone()));
                assert_eq!(m.prev_before(&sibling).map(|(k, _)| k), Some(deep.clone()));

                // Past the end: descends the full chain, finds nothing at
                // or after, and unwinds every recorded level.
                let mut past = deep.clone();
                *past.last_mut().expect("non-empty") = b'z';
                assert_eq!(m.next_at_or_after(&past), None);
                // Mirror: before the beginning.
                let mut before = deep.clone();
                *before.last_mut().expect("non-empty") = b'a';
                assert_eq!(m.prev_before(&before), None);
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
        m.insert(b"pre-existing:alpha", 1);
        m.insert(b"pre-existing:beta", 2);

        // Suffix creation, then a split (disposes the old suffix).
        m.insert(b"shared-prefix-01:aaaa", 10);
        m.insert(b"shared-prefix-01:bbbb", 11);
        // In-place value update on a suffix leaf (no disposal).
        assert_eq!(m.insert(b"shared-prefix-01:aaaa", 12), Some(10));
        assert_eq!(m.get(b"shared-prefix-01:aaaa"), Some(12));
        // ins_slot split path.
        let slot = m.ins_slot(b"shared-prefix-01:aaXX");
        // SAFETY: slot valid until next mutation.
        unsafe { slot.as_ptr().write(13) };
        assert_eq!(m.get(b"shared-prefix-01:aaXX"), Some(13));

        // Suffix removal + emptied-node pruning back up the chain.
        assert_eq!(m.remove(b"shared-prefix-01:aaXX"), Some(13));
        assert_eq!(m.remove(b"shared-prefix-01:bbbb"), Some(11));
        assert_eq!(m.remove(b"shared-prefix-01:aaaa"), Some(12));
        assert_eq!(m.get(b"pre-existing:alpha"), Some(1));

        // Whole-tree disposal (dispose_tree), then root removal via the
        // last-key path.
        assert_eq!(m.len(), 2);
        assert!(m.clear() > 0);
        m.insert(b"solo", 42);
        assert_eq!(m.remove(b"solo"), Some(42));
        assert!(m.is_empty());

        // Grace-period advances free the retired chain; drop drains the rest.
        collector.try_advance();
        collector.try_advance();
        collector.try_advance();
        drop(m);
        drop(collector);
    }

    /// #363 Step A regression guard: a sub-trie node is exactly the map
    /// engine core — no embedded allocator, no per-node insert-path
    /// cache. Re-embedding either (the pre-#363 layout was ~700 bytes)
    /// fails here before it shows up as a descent-locality regression.
    #[test]
    fn str_node_is_just_the_map_core() {
        assert_eq!(size_of::<StrNode>(), size_of::<crate::map::MapCore>());
        assert!(
            size_of::<StrNode>() <= 64,
            "StrNode grew: {}",
            size_of::<StrNode>()
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
            m.insert(k.as_bytes(), i);
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
                    assert_eq!(map.insert(&k, v), model.insert(k.clone(), v), "ins {k:?}");
                }
                2 => assert_eq!(map.remove(&k), model.remove(&k), "rm {k:?}"),
                _ => assert_eq!(map.get(&k), model.get(&k).copied(), "get {k:?}"),
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
            cursor = map.next_after(&k);
        }
        assert!(cursor.is_none());
        let mut cursor = map.last();
        for (mk, mv) in model.iter().rev() {
            let (k, slot) = cursor.expect("rev sweep entry");
            assert_eq!(&k, mk, "rev sweep key");
            // SAFETY: as above.
            assert_eq!(unsafe { *slot.as_ptr() }, *mv, "rev sweep value");
            cursor = map.prev_before(&k);
        }
        assert!(cursor.is_none());
        // Point navigation probes.
        for _ in 0..if cfg!(miri) { 30 } else { 400 } {
            let k = keygen(&mut rng);
            assert_eq!(
                map.next_at_or_after(&k).map(|e| e.0),
                model.range(k.clone()..).next().map(|(mk, _)| mk.clone()),
                "next>= {k:?}"
            );
            assert_eq!(
                map.prev_at_or_before(&k).map(|e| e.0),
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
            assert_eq!(map.remove(&k), model.remove(&k));
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
            assert_eq!(map.insert(k, i as u64 + 10), None, "{k:?}");
        }
        assert_eq!(map.get(b""), Some(10));
        assert_eq!(map.get(b"abcdefgh"), Some(12));
        assert_eq!(map.get(b"abcdefg"), None);
        // ins_slot keeps existing values and writes through.
        let slot = map.ins_slot(b"abcdefgh");
        // SAFETY: slot valid until next mutation.
        unsafe {
            assert_eq!(*slot.as_ptr(), 12);
            slot.as_ptr().write(99);
        }
        assert_eq!(map.get(b"abcdefgh"), Some(99));
        // Ordering across boundary shapes.
        let (first, _) = map.first().unwrap();
        assert_eq!(first, b"");
        assert_eq!(map.next_after(b"").unwrap().0, b"a");
        assert_eq!(map.next_after(b"abcdefgg").unwrap().0, b"abcdefgh");
        assert_eq!(map.next_after(b"abcdefgh").unwrap().0, b"abcdefghabcdefgh");
        assert_eq!(map.prev_before(b"abcdefgh").unwrap().0, b"abcdefgg");
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
        map.insert(key1, 100);
        assert_eq!(map.get(key1), Some(100));
        assert_eq!(map.len(), 1);

        // Insert a second key sharing a long prefix (35 bytes).
        let key2 = b"org.apache.hadoop.fs.azurebfs.services.AbfsRestOperation";
        map.insert(key2, 200);
        assert_eq!(map.get(key1), Some(100));
        assert_eq!(map.get(key2), Some(200));
        assert_eq!(map.len(), 2);

        // Insert a third key diverging early (at byte 4).
        let key3 = b"org.eclipse.jetty.server.Server";
        map.insert(key3, 300);
        assert_eq!(map.get(key1), Some(100));
        assert_eq!(map.get(key2), Some(200));
        assert_eq!(map.get(key3), Some(300));
        assert_eq!(map.len(), 3);

        // Verify sorted navigation across compressed paths:
        let (k1, s1) = map.first().unwrap();
        assert_eq!(k1, key1);
        // SAFETY: slot is valid until next mutation.
        unsafe { assert_eq!(*s1.as_ptr(), 100) };

        let (k2, s2) = map.next_after(key1).unwrap();
        assert_eq!(k2, key2);
        // SAFETY: slot is valid until next mutation.
        unsafe { assert_eq!(*s2.as_ptr(), 200) };

        let (k3, s3) = map.next_after(key2).unwrap();
        assert_eq!(k3, key3);
        // SAFETY: slot is valid until next mutation.
        unsafe { assert_eq!(*s3.as_ptr(), 300) };

        assert_eq!(map.next_after(key3), None);

        // Remove the split key:
        assert_eq!(map.remove(key2), Some(200));
        assert_eq!(map.get(key2), None);
        assert_eq!(map.get(key1), Some(100));
        assert_eq!(map.get(key3), Some(300));
        assert_eq!(map.len(), 2);
    }
}
