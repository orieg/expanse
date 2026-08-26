//! `ExpanseStrMap`: a sorted map from C-style byte strings to `u64`
//! values (compat: JudySL).
//!
//! Structure (clean-room, designed against the documented JudySL
//! *semantics* only): a meta-trie of word-map nodes with cross-chunk
//! path compression ([Issue #84 Item 1](https://github.com/orieg/expanse/issues/84)).
//! Each branch node is an [`ExpanseMap`] keyed by the string's next
//! **8-byte chunk, packed big-endian**, so the word maps' numeric order
//! *is* byte-lexicographic order and all ordered navigation falls out of
//! the word engine.
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

use crate::map::ExpanseMap;
use crate::occ::Collector;
use core::ptr::NonNull;
use std::sync::{Arc, OnceLock};

const CHUNK: usize = 8;
const TAG_SUFFIX: u64 = 1;

/// Leaf suffix: stores the unbranched remainder of a string key and its value.
struct StrSuffix {
    suffix: Box<[u8]>,
    value: u64,
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

/// One trie level: word map over the next 8-byte chunk.
struct StrNode {
    map: ExpanseMap,
}

/// Phase 7 (issue #219 Phase 2): hooks a freshly created sub-trie node's
/// allocator to the epoch collector when the map is concurrently shared
/// (no-op otherwise). Called before the node is published, so every
/// mutation a reader could race is bracketed and every free it could
/// observe is deferred.
fn attach_node(node: &StrNode, defer: Option<&Arc<Collector>>) {
    if let Some(c) = defer {
        node.map.occ_root().1.defer_to(Arc::clone(c));
    }
}

/// Disposes an unlinked suffix: dropped immediately when not shared,
/// retired through the epoch collector when it is — a reader that
/// validated the tagged pointer at an earlier snapshot may still be
/// reading the (write-once) shell and byte buffer under its pin.
fn dispose_suffix(ptr: *mut StrSuffix, defer: Option<&Arc<Collector>>) {
    match defer {
        None => {
            // SAFETY: caller unlinked `ptr`; this is the last reference.
            drop(unsafe { Box::from_raw(ptr) });
        }
        Some(c) => {
            // Retire the byte buffer and the shell raw (no `Drop` runs; the
            // collector frees plain memory after the grace period). The
            // buffer's owning `Box` is moved out **by value** so its
            // original provenance travels to the collector — a pointer
            // merely borrowed out of the shell would not carry deallocation
            // rights (Miri rejects the `dealloc`). An empty suffix's
            // `Box<[u8]>` owns no allocation — nothing to retire for it.
            // SAFETY: unlinked; this is the last owner. The shell's
            // `suffix` field is never read again — the shell itself is
            // retired below without running `Drop` (concurrent readers
            // still see the write-once bytes until the grace period ends).
            let boxed: Box<[u8]> = unsafe { core::ptr::read(&raw const (*ptr).suffix) };
            let len = boxed.len();
            if len > 0 {
                let buf = Box::into_raw(boxed).cast::<u8>();
                c.retire(NonNull::new(buf).expect("non-null suffix buffer"), len, 1);
            } else {
                core::mem::forget(boxed);
            }
            c.retire(
                NonNull::new(ptr.cast::<u8>()).expect("non-null suffix"),
                size_of::<StrSuffix>(),
                align_of::<StrSuffix>(),
            );
        }
    }
}

/// Disposes one unlinked node whose continuation children the caller
/// handles separately — pruning disposes an already-empty child, while
/// [`dispose_tree`] queues/disposes each node's children itself before
/// calling this (the map's continuation entries are plain words; nothing
/// here follows them).
fn dispose_node(ptr: *mut StrNode, defer: Option<&Arc<Collector>>) {
    match defer {
        None => {
            // SAFETY: caller unlinked `ptr`; this is the last reference.
            drop(unsafe { Box::from_raw(ptr) });
        }
        Some(c) => {
            // Drop the interior map in place — its frees route through its
            // deferred `NodeAlloc`, so retired trie nodes stay mapped for
            // pinned readers — then retire the shell raw without running
            // `StrNode::drop` (the map field is already dropped, and any
            // continuation words it held are the caller's to dispose).
            // SAFETY: unlinked, writer-exclusive; the map field is dropped
            // exactly once here and never touched again (the shell memory
            // stays mapped for pinned readers, whose validation rejects
            // whatever they read from it).
            unsafe { core::ptr::drop_in_place(&raw mut (*ptr).map) };
            c.retire(
                NonNull::new(ptr.cast::<u8>()).expect("non-null node"),
                size_of::<StrNode>(),
                align_of::<StrNode>(),
            );
        }
    }
}

/// Disposes a whole unlinked subtree. Iterative like every other walk in
/// this module — one frame per 8 key bytes would overflow the stack on
/// exactly the deep chains the iterative `Drop` exists to survive.
fn dispose_tree(root: *mut StrNode, defer: Option<&Arc<Collector>>) {
    match defer {
        None => {
            // SAFETY: unlinked subtree; `StrNode::drop` tears it down
            // iteratively.
            drop(unsafe { Box::from_raw(root) });
        }
        Some(_) => {
            let mut stack: Vec<*mut StrNode> = vec![root];
            while let Some(p) = stack.pop() {
                // Queue children and dispose suffixes while iterating (the
                // continuation words are plain values in the map — neither
                // disposal touches the map being iterated, and dropping the
                // map below does not follow them).
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
                dispose_node(p, defer);
            }
        }
    }
}

/// Packs `key[off..]`'s next chunk big-endian; `true` when the chunk
/// contains the terminating NUL (i.e. fewer than 8 bytes remain).
fn chunk_at(key: &[u8], off: usize) -> (u64, bool) {
    let rest = &key[off.min(key.len())..];
    let mut c = [0u8; CHUNK];
    let n = rest.len().min(CHUNK);
    c[..n].copy_from_slice(&rest[..n]);
    (u64::from_be_bytes(c), rest.len() < CHUNK)
}

/// The byte content of a terminal chunk (bytes before the NUL).
fn terminal_bytes(chunk: u64) -> impl Iterator<Item = u8> {
    chunk.to_be_bytes().into_iter().take_while(|&b| b != 0)
}

/// True when the chunk contains a NUL byte (terminal entry).
fn is_terminal(chunk: u64) -> bool {
    chunk.to_be_bytes().contains(&0)
}

impl StrNode {
    fn new() -> Self {
        Self {
            map: ExpanseMap::new(),
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
                return n.map.get_value_slot(chunk).expect("present chunk");
            }
            out.extend_from_slice(&chunk.to_be_bytes());
            if is_suffix_ptr(v) {
                // SAFETY: tagged pointer encodes a live Box<StrSuffix>.
                let suffix = unsafe { &mut *unpack_suffix(v) };
                out.extend_from_slice(&suffix.suffix);
                return NonNull::new(&raw mut suffix.value).expect("non-null value slot");
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
            Some(self.map.get_value_slot(chunk).expect("present chunk"))
        } else if is_suffix_ptr(v) {
            // SAFETY: tagged pointer encodes a live Box<StrSuffix>.
            let suffix = unsafe { &mut *unpack_suffix(v) };
            out.extend_from_slice(&chunk.to_be_bytes());
            out.extend_from_slice(&suffix.suffix);
            Some(NonNull::new(&raw mut suffix.value).expect("non-null value slot"))
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
                    // SAFETY: tagged pointer encodes a live Box<StrSuffix>.
                    let suffix = unsafe { &mut *unpack_suffix(v) };
                    let rem = &key[off.min(key.len()) + CHUNK.min(key.len().saturating_sub(off))..];
                    if rem <= &suffix.suffix[..] {
                        out.extend_from_slice(&chunk.to_be_bytes());
                        out.extend_from_slice(&suffix.suffix);
                        return Some(
                            NonNull::new(&raw mut suffix.value).expect("non-null value slot"),
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
                    // SAFETY: tagged pointer encodes a live Box<StrSuffix>.
                    let suffix = unsafe { &mut *unpack_suffix(v) };
                    let rem = &key[off.min(key.len()) + CHUNK.min(key.len().saturating_sub(off))..];
                    let cmp = rem.cmp(&suffix.suffix[..]);
                    let match_ok = if exclusive {
                        cmp == core::cmp::Ordering::Greater
                    } else {
                        cmp != core::cmp::Ordering::Less
                    };
                    if match_ok {
                        out.extend_from_slice(&target.to_be_bytes());
                        out.extend_from_slice(&suffix.suffix);
                        return Some(
                            NonNull::new(&raw mut suffix.value).expect("non-null value slot"),
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
    fn remove(&mut self, key: &[u8], off: usize, defer: Option<&Arc<Collector>>) -> Option<u64> {
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
                break n.map.remove(chunk)?;
            }
            let v = n.map.get(chunk)?;
            if is_suffix_ptr(v) {
                // SAFETY: tagged pointer encodes a live Box<StrSuffix>.
                let suffix = unsafe { &*unpack_suffix(v) };
                let rem = &key[off + CHUNK..];
                if rem == &suffix.suffix[..] {
                    n.map.remove(chunk);
                    let removed_val = suffix.value;
                    // Unlinked above; retired when shared.
                    dispose_suffix(unpack_suffix(v), defer);
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
                parent.map.remove(chunk);
                // Unlinked above; retired when shared.
                dispose_node(unpack_child(child_v), defer);
            }
        }
        Some(removed)
    }

    /// Heap bytes used by this subtree (read-only accounting walk).
    ///
    /// Iterative for the same reason as `Drop`: `clear()` calls this, so
    /// recursing would overflow the stack on exactly the deep chains the
    /// iterative destructor exists to survive.
    fn subtree_bytes(&self) -> u64 {
        let mut bytes = 0u64;
        let mut stack: Vec<*const StrNode> = vec![core::ptr::from_ref(self)];
        while let Some(p) = stack.pop() {
            // SAFETY: `self` plus continuation values, all live nodes.
            let node = unsafe { &*p };
            bytes += node.map.mem_used() as u64 + size_of::<Self>() as u64;
            for (k, v) in node.map.iter() {
                if !is_terminal(k) {
                    if is_suffix_ptr(v) {
                        // SAFETY: tagged pointer encodes a live Box<StrSuffix>.
                        let suffix = unsafe { &*unpack_suffix(v) };
                        bytes += size_of::<StrSuffix>() as u64 + suffix.suffix.len() as u64;
                    } else {
                        stack.push(unpack_child(v));
                    }
                }
            }
        }
        bytes
    }
}

impl StrNode {
    /// Moves this node's continuation children onto `stack`, leaving the
    /// node childless. Terminal entries (values) are left alone.
    ///
    /// Clearing as we go is what makes the iterative teardown correct: a
    /// node whose children have been taken has nothing left for its own
    /// `Drop` to descend into.
    fn take_children(&mut self, stack: &mut Vec<*mut StrNode>) {
        let mut chunks = Vec::new();
        for (k, v) in self.map.iter() {
            if !is_terminal(k) {
                chunks.push((k, v));
            }
        }
        for (k, v) in chunks {
            self.map.remove(k);
            if is_suffix_ptr(v) {
                // SAFETY: unlinked from map, unique ownership reclaimed.
                drop(unsafe { Box::from_raw(unpack_suffix(v)) });
            } else {
                stack.push(unpack_child(v));
            }
        }
    }
}

impl Drop for StrNode {
    fn drop(&mut self) {
        // Iterative, with an explicit worklist. Recursing here costs one
        // stack frame per 8 bytes of key depth, so a long key (this map
        // imposes no length limit, matching the original) can overflow
        // the stack **while freeing** — and a stack overflow aborts the
        // process with no recovery path. Cleanup is the worst possible
        // place to have that failure mode.
        let mut stack: Vec<*mut StrNode> = Vec::new();
        self.take_children(&mut stack);
        while let Some(p) = stack.pop() {
            // SAFETY: sole owner at drop time; each pointer reaches the
            // stack exactly once, because taking a node's children also
            // unlinks them from its map.
            let mut node = unsafe { Box::from_raw(p) };
            node.take_children(&mut stack);
            // No continuation entries left, so this recurses no further.
            drop(node);
        }
    }
}

/// A sorted map from NUL-free byte strings to `u64` values (compat:
/// JudySL). Iteration order is byte-lexicographic.
pub struct ExpanseStrMap {
    root: Option<Box<StrNode>>,
    pop: u64,
    /// Phase 7 (issue #219 Phase 2): when set, unlinked nodes/suffixes are
    /// retired through the collector instead of freed (concurrent readers
    /// may still hold pointers into them), and every sub-trie's `NodeAlloc`
    /// is deferred at creation so its interior frees and mutation brackets
    /// participate too.
    deferred: OnceLock<Arc<Collector>>,
}

impl ExpanseStrMap {
    /// Creates an empty map.
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: None,
            pop: 0,
            deferred: OnceLock::new(),
        }
    }

    /// Switches this map to deferred reclamation through `collector`,
    /// permanently (the Phase 7 `sync` wrapper calls this once at
    /// construction): every sub-trie created from here on is attached at
    /// creation, before it is published. Idempotent for the same
    /// collector; a second call with a different collector panics.
    ///
    /// Requires an **empty** map: a populated map's sub-trie allocators
    /// hold slab-carved node memory, which must never be retired to the
    /// collector (see `NodeAlloc::defer_to`). The `sync` wrapper shares a
    /// populated map by rebuilding it through a pre-deferred one.
    ///
    /// `pub(crate)` deliberately — only the `sync` wrapper drives a
    /// collector's epochs (see `BlobArena::defer_to` for the rationale).
    pub(crate) fn defer_to(&self, collector: Arc<Collector>) {
        assert!(
            self.root.is_none(),
            "ExpanseStrMap::defer_to requires an empty map; rebuild a \
             populated map through a pre-deferred one instead"
        );
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

    /// Heap bytes used by the map (read-only accounting walk).
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.root
            .as_deref()
            .map_or(0, |r| r.subtree_bytes() as usize)
    }

    fn assert_key(key: &[u8]) {
        debug_assert!(!key.contains(&0), "keys are NUL-free byte strings");
    }

    /// Splits a suffix entry that diverges from the key being inserted:
    /// builds a child node holding the existing suffix's continuation,
    /// publishes it over the suffix's map entry, and disposes of the old
    /// suffix (retired when shared — a concurrent reader may still hold
    /// it). Returns the raw child for the caller to descend into.
    ///
    /// Reads `old` only through short-lived internal borrows: a borrow
    /// passed in as a parameter would be *protected* for the whole call
    /// and conflict with the disposal's move-out of the byte buffer
    /// (Miri rejects it).
    fn split_suffix(
        node: &mut StrNode,
        chunk: u64,
        old: *mut StrSuffix,
        defer: Option<&Arc<Collector>>,
    ) -> *mut StrNode {
        let mut child = Box::new(StrNode::new());
        attach_node(&child, defer);
        // SAFETY: `old` is the live suffix being split; every borrow here
        // ends before the disposal below.
        let (c1, t1, value) = unsafe {
            let s = &*old;
            let (c1, t1) = chunk_at(&s.suffix, 0);
            (c1, t1, s.value)
        };
        if t1 {
            child.map.insert(c1, value);
        } else {
            // SAFETY: as above — the continuation bytes are copied out
            // before the old suffix is disposed of.
            let rem1: Box<[u8]> = unsafe { (&(*old).suffix)[CHUNK..].into() };
            let s1 = Box::into_raw(Box::new(StrSuffix {
                suffix: rem1,
                value,
            }));
            child.map.insert(c1, pack_suffix(s1));
        }
        let child_raw = Box::into_raw(child);
        node.map.insert(chunk, pack_child(child_raw));
        dispose_suffix(old, defer);
        child_raw
    }

    /// Inserts `key → val`; returns the replaced value if present.
    pub fn insert(&mut self, key: &[u8], val: u64) -> Option<u64> {
        Self::assert_key(key);
        let defer = self.deferred.get().cloned();
        // Field-level borrows on purpose: `node` must borrow only
        // `self.root` so `self.pop` stays reachable in the loop.
        if self.root.is_none() {
            let node = Box::new(StrNode::new());
            attach_node(&node, defer.as_ref());
            self.root = Some(node);
        }
        let mut node: &mut StrNode = self.root.as_deref_mut().expect("root just ensured");
        let mut off = 0;
        loop {
            let (chunk, terminal) = chunk_at(key, off);
            if terminal {
                let prev = node.map.insert(chunk, val);
                if prev.is_none() {
                    self.pop += 1;
                }
                return prev;
            }
            match node.map.get(chunk) {
                None => {
                    let rem = &key[off + CHUNK..];
                    let suffix = Box::into_raw(Box::new(StrSuffix {
                        suffix: rem.into(),
                        value: val,
                    }));
                    node.map.insert(chunk, pack_suffix(suffix));
                    self.pop += 1;
                    return None;
                }
                Some(v) if is_suffix_ptr(v) => {
                    let sfx = unpack_suffix(v);
                    let rem = &key[off + CHUNK..];
                    // SAFETY: tagged pointer encodes a live Box<StrSuffix>;
                    // short-lived shared borrow of the write-once bytes.
                    if unsafe { rem == &(&(*sfx).suffix)[..] } {
                        // In-place value update, field-precise (no `&mut`
                        // over the shell whose write-once fields concurrent
                        // readers load): only the value word mutates, under
                        // the version bracket when shared.
                        // SAFETY: exclusive writer; a racing reader's load
                        // is discarded unless its snapshot validates.
                        return Some(unsafe { core::ptr::replace(&raw mut (*sfx).value, val) });
                    }
                    let child_raw = Self::split_suffix(node, chunk, sfx, defer.as_ref());
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
        let defer = self.deferred.get().cloned();
        // Field-level borrows on purpose: `node` must borrow only
        // `self.root` so `self.pop` stays reachable in the loop.
        if self.root.is_none() {
            let node = Box::new(StrNode::new());
            attach_node(&node, defer.as_ref());
            self.root = Some(node);
        }
        let mut node: &mut StrNode = self.root.as_deref_mut().expect("root just ensured");
        let mut off = 0;
        loop {
            let (chunk, terminal) = chunk_at(key, off);
            if terminal {
                if !node.map.contains_key(chunk) {
                    self.pop += 1;
                }
                return node.map.ins_slot(chunk);
            }
            match node.map.get(chunk) {
                None => {
                    let rem = &key[off + CHUNK..];
                    let suffix = Box::into_raw(Box::new(StrSuffix {
                        suffix: rem.into(),
                        value: 0,
                    }));
                    node.map.insert(chunk, pack_suffix(suffix));
                    self.pop += 1;
                    // SAFETY: suffix is a live, uniquely owned pointer allocated above.
                    return unsafe { NonNull::new_unchecked(&raw mut (*suffix).value) };
                }
                Some(v) if is_suffix_ptr(v) => {
                    let sfx = unpack_suffix(v);
                    let rem = &key[off + CHUNK..];
                    // SAFETY: tagged pointer encodes a live Box<StrSuffix>;
                    // short-lived shared borrow of the write-once bytes.
                    if unsafe { rem == &(&(*sfx).suffix)[..] } {
                        // SAFETY: field-precise pointer to the value word.
                        return NonNull::new(unsafe { &raw mut (*sfx).value })
                            .expect("non-null value slot");
                    }
                    // Divergence: publish a child over the suffix entry,
                    // then dispose of the old suffix (see `split_suffix`).
                    let child_raw = Self::split_suffix(node, chunk, sfx, defer.as_ref());
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
                // SAFETY: tagged pointer encodes a live Box<StrSuffix>.
                let suffix = unsafe { &*unpack_suffix(v) };
                let rem = &key[off + CHUNK..];
                if rem == &suffix.suffix[..] {
                    return Some(suffix.value);
                }
                return None;
            }
            // SAFETY: untagged child pointer to a live StrNode.
            node = unsafe { &*unpack_child(v) };
            off += CHUNK;
        }
    }

    /// Phase 7 (issue #219 Phase 2): one bounded, validated lock-free
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
            let msnap = unsafe { (*node).map.occ_root().0 };
            // SAFETY: the caller's pin + snapshot contract carries through.
            let found = unsafe { crate::sync::walk_validated::<true>(msnap, chunk, ver, snap) }?;
            if terminal {
                return Ok(found);
            }
            let Some(v) = found else { return Ok(None) };
            if is_suffix_ptr(v) {
                let sfx: *const StrSuffix = unpack_suffix(v);
                // SAFETY: `v` was validated at `snap`, so `sfx` was the
                // published suffix then; EBR keeps it (and its buffer)
                // mapped under the pin. Fat pointer and bytes are
                // write-once; the value word may race and is validated
                // below before use.
                let (bytes, value) = unsafe {
                    let s = &*sfx;
                    (&s.suffix[..], s.value)
                };
                let matched = bytes == &key[off + CHUNK..];
                if !ver.validate(snap) {
                    return Err(Retry);
                }
                return Ok(matched.then_some(value));
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
                return node.map.get_value_slot(chunk);
            }
            let v = node.map.get(chunk)?;
            if is_suffix_ptr(v) {
                // SAFETY: tagged pointer encodes a live Box<StrSuffix>.
                let suffix = unsafe { &mut *unpack_suffix(v) };
                let rem = &key[off + CHUNK..];
                if rem == &suffix.suffix[..] {
                    return Some(NonNull::new(&raw mut suffix.value).expect("non-null value slot"));
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
        let defer = self.deferred.get().cloned();
        let root = self.root.as_deref_mut()?;
        let removed = root.remove(key, 0, defer.as_ref())?;
        self.pop -= 1;
        if root.map.is_empty() {
            let root_box = self.root.take().expect("root present");
            // Unlinked (the root slot is cleared); retired when shared.
            dispose_node(Box::into_raw(root_box), defer.as_ref());
        }
        Some(removed)
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
                // Count with a read-only walk, then dispose of the subtree
                // (retired through the collector when shared).
                let bytes = root.subtree_bytes();
                dispose_tree(Box::into_raw(root), self.deferred.get());
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
    fn deferred_strmap_dispose_round_trip() {
        use crate::occ::Collector;
        use std::sync::Arc;

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
    fn test_cross_chunk_path_compression_split_and_memory() {
        let mut map = ExpanseStrMap::new();
        // Insert a 64-byte key. With path compression, this creates 1 StrNode + 1 StrSuffix.
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
