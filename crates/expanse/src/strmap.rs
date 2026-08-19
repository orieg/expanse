//! `ExpanseStrMap`: a sorted map from C-style byte strings to `u64`
//! values (compat: JudySL).
//!
//! Structure (clean-room, designed against the documented JudySL
//! *semantics* only): a meta-trie of word-map nodes. Each node is an
//! [`ExpanseMap`] keyed by the string's next **8-byte chunk, packed
//! big-endian**, so the word maps' numeric order *is* byte-lexicographic
//! order and all ordered navigation falls out of the word engine.
//!
//! Keys are NUL-free byte strings (the C surface hands over
//! NUL-terminated strings, so this is the natural domain). A chunk that
//! contains the string's terminating NUL — always the case for the last
//! chunk, including the all-zero chunk of a string whose length is a
//! multiple of 8 — is a **terminal** entry holding the user value; a
//! chunk of 8 non-NUL bytes is a **continuation** entry holding a pointer
//! to the child node. The two are distinguished by the chunk bytes alone
//! (does it contain a zero byte?), so values need no tag bits.
//!
//! v1 note: no single-path compression across chunk levels yet; a long
//! unique suffix costs one node per 8 bytes.

use crate::map::ExpanseMap;
use core::ptr::NonNull;

const CHUNK: usize = 8;

/// One trie level: word map over the next 8-byte chunk.
struct StrNode {
    map: ExpanseMap,
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
    /// `v` must be a continuation value produced by `Box::into_raw` in
    /// this module.
    unsafe fn child_mut<'a>(v: u64) -> &'a mut StrNode {
        // SAFETY: per contract, `v` is a live Box<StrNode> pointer.
        unsafe { &mut *(v as *mut StrNode) }
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
            node = v as *mut StrNode;
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
                // Exact continuation: the answer, if there is one, is
                // deeper. Record where to resume if it is not.
                stack.push((node, target, out.len()));
                out.extend_from_slice(&chunk.to_be_bytes());
                node = v as *mut StrNode;
                off += CHUNK;
                continue;
            }
            // No exact continuation. `cursor` is either the query's own
            // terminal chunk — a valid answer, since a longer query could
            // not have produced a chunk containing a NUL — or the first
            // entry strictly greater. Both are taken the same way.
            if let Some(slot) = n.take_from(cursor, out, true) {
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
                stack.push((node, target, out.len()));
                out.extend_from_slice(&target.to_be_bytes());
                node = v as *mut StrNode;
                off += CHUNK;
                continue;
            }
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
    /// are pruned on the way out.
    /// Iterative: descend recording the path, then prune emptied nodes on
    /// the way back out. Recursing costs a frame per 8 key bytes, which a
    /// long key turns into a stack overflow.
    fn remove(&mut self, key: &[u8], off: usize) -> Option<u64> {
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
            path.push((node, chunk));
            node = v as *mut StrNode;
            off += CHUNK;
        };

        // Unwind: an emptied child is unlinked and freed, which may empty
        // its parent in turn. The first non-empty ancestor stops it —
        // nothing above that can have been emptied by this removal.
        while let Some((parent_ptr, chunk)) = path.pop() {
            // SAFETY: recorded during the descent; still live.
            let parent = unsafe { &mut *parent_ptr };
            let child_v = parent.map.get(chunk).expect("path entry still linked");
            // SAFETY: continuation value, a live child node.
            let empty = unsafe { &*(child_v as *const StrNode) }.map.is_empty();
            if !empty {
                break;
            }
            parent.map.remove(chunk);
            // SAFETY: unlinked above, so this is the last reference.
            drop(unsafe { Box::from_raw(child_v as *mut StrNode) });
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
                    stack.push(v as *const StrNode);
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
                chunks.push(k);
                stack.push(v as *mut StrNode);
            }
        }
        for k in chunks {
            self.map.remove(k);
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
}

impl ExpanseStrMap {
    /// Creates an empty map.
    #[must_use]
    pub fn new() -> Self {
        Self { root: None, pop: 0 }
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

    fn assert_key(key: &[u8]) {
        debug_assert!(!key.contains(&0), "keys are NUL-free byte strings");
    }

    /// Inserts `key → val`; returns the replaced value if present.
    pub fn insert(&mut self, key: &[u8], val: u64) -> Option<u64> {
        Self::assert_key(key);
        let mut node: &mut StrNode = self.root.get_or_insert_with(|| Box::new(StrNode::new()));
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
            let v = match node.map.get(chunk) {
                Some(v) => v,
                None => {
                    let child = Box::into_raw(Box::new(StrNode::new())) as u64;
                    node.map.insert(chunk, child);
                    child
                }
            };
            // SAFETY: continuation values are child pointers.
            node = unsafe { StrNode::child_mut(v) };
            off += CHUNK;
        }
    }

    /// Inserts `key` with value 0 if absent (existing value kept) and
    /// returns a writable pointer to its value slot — the compat
    /// `JudySLIns` contract. Valid until the next structural mutation.
    pub fn ins_slot(&mut self, key: &[u8]) -> NonNull<u64> {
        Self::assert_key(key);
        let mut node: &mut StrNode = self.root.get_or_insert_with(|| Box::new(StrNode::new()));
        let mut off = 0;
        loop {
            let (chunk, terminal) = chunk_at(key, off);
            if terminal {
                if !node.map.contains_key(chunk) {
                    self.pop += 1;
                }
                return node.map.ins_slot(chunk);
            }
            let v = match node.map.get(chunk) {
                Some(v) => v,
                None => {
                    let child = Box::into_raw(Box::new(StrNode::new())) as u64;
                    node.map.insert(chunk, child);
                    child
                }
            };
            // SAFETY: continuation values are child pointers.
            node = unsafe { StrNode::child_mut(v) };
            off += CHUNK;
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
            // SAFETY: continuation values are child pointers.
            node = unsafe { &*(v as *const StrNode) };
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
            // SAFETY: continuation values are child pointers.
            node = unsafe { StrNode::child_mut(v) };
            off += CHUNK;
        }
    }

    /// Removes `key`; returns its value if it was present.
    pub fn remove(&mut self, key: &[u8]) -> Option<u64> {
        Self::assert_key(key);
        let root = self.root.as_deref_mut()?;
        let removed = root.remove(key, 0)?;
        self.pop -= 1;
        if root.map.is_empty() {
            self.root = None;
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
            // Count with a read-only walk, then let ownership drop it.
            Some(root) => root.subtree_bytes(),
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
                let key = vec![b'k'; 64 * 1024];
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
                let key = vec![b'q'; 32 * 1024];
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
                let deep = vec![b'm'; 64 * 1024];
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

    #[test]
    fn model_differential() {
        let ops = if cfg!(miri) { 300 } else { 4000 };
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
}
