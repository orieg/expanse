//! Phase 6: `ExpanseSet`, the public set-flavor tree (compat: Judy1).
//!
//! Root organization follows the original's economy: populations up to
//! [`ROOT_LEAF_CAP`] live in a **root leaf** — one sorted array of full
//! 8-byte keys — and only past that does a real level-8 trie exist (whose
//! total population this struct tracks, the original's JPM role). The
//! tree condenses back into a root leaf when its population falls one
//! below the promotion boundary (1-index hysteresis, as everywhere).

use crate::alloc::NodeAlloc;
use crate::get;
use crate::mutate;
use crate::node::Edge;
use crate::sync::RootSnapshot;
use crate::types::Key;
use crate::validate::ExpanseStats;
use core::ptr::NonNull;

/// Maximum population held in the root leaf before a real trie is built.
pub const ROOT_LEAF_CAP: usize = 31;

/// Allocation size of a root leaf holding `pop` keys. Class-sized (like
/// the trie's linear leaves) so consecutive inserts and deletes shift in
/// place instead of reallocating on every operation.
fn root_leaf_size(pop: usize) -> usize {
    8 * crate::leaf::cap_class(pop)
}

#[derive(Clone, Copy)]
enum Root {
    Empty,
    /// Sorted full-width keys, `pop` of them, in one allocation.
    Leaf {
        keys: NonNull<u8>,
        pop: usize,
    },
    /// A level-8 trie; `pop` is the total population (the JPM role).
    Tree {
        top: Edge,
        pop: u64,
    },
}

/// A sparse, dynamic set of `u64` keys (compat: Judy1).
///
/// Adaptive expanse-partitioned trie: memory stays near-proportional to
/// population across sequential, random, clustered, and sparse key
/// distributions, and membership tests run in at most eight digit steps.
pub struct ExpanseSet {
    root: Root,
    alloc: NodeAlloc,
    path: core::cell::UnsafeCell<crate::mutate::InsertPath>,
}

// SAFETY: the tree exclusively owns every allocation reachable from its
// root (raw pointers are the only reason the impl is not automatic);
// moving it to another thread moves that ownership wholesale. It is
// deliberately NOT `Sync` — shared access goes through `SyncExpanseSet`.
unsafe impl Send for ExpanseSet {}

impl ExpanseSet {
    /// Creates an empty set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: Root::Empty,
            alloc: NodeAlloc::new(),
            path: core::cell::UnsafeCell::new(crate::mutate::InsertPath::empty()),
        }
    }

    #[inline(always)]
    fn flush_path(&self) {
        // SAFETY: path is an internal cursor whose state is flushed through UnsafeCell.
        unsafe {
            (*self.path.get()).flush();
        }
    }

    /// Number of keys in the set.
    #[must_use]
    pub fn len(&self) -> u64 {
        match &self.root {
            Root::Empty => 0,
            Root::Leaf { pop, .. } => *pop as u64,
            Root::Tree { pop, .. } => *pop,
        }
    }

    /// True when no keys are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Heap bytes currently used by the set's nodes and leaves.
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.alloc.bytes_in_use()
    }

    /// Cumulative node/leaf allocations made by this container since it
    /// was created (diagnostics; see `tests/no_heap_churn.rs`, which
    /// subtracts these from the process-wide count to isolate
    /// incidental scratch allocation).
    #[must_use]
    pub fn total_node_allocs(&self) -> usize {
        self.alloc.total_allocs()
    }

    fn root_leaf_keys(&self) -> &[u64] {
        match &self.root {
            Root::Leaf { keys, pop } => {
                // SAFETY: the root leaf holds `pop` u64 keys, 8-aligned
                // (allocations are cache-line aligned).
                unsafe { core::slice::from_raw_parts(keys.as_ptr().cast::<u64>(), *pop) }
            }
            _ => &[],
        }
    }

    /// Membership test.
    #[inline(always)]
    #[must_use]
    pub fn contains(&self, key: Key) -> bool {
        match &self.root {
            Root::Empty => false,
            Root::Leaf { keys, pop } => {
                let pop = *pop;
                let kptr = keys.as_ptr().cast::<u64>();
                if pop <= 4 {
                    // SAFETY: root leaf holds `pop` valid u64 keys.
                    unsafe {
                        if pop >= 1 && *kptr == key {
                            return true;
                        }
                        if pop >= 2 && *kptr.add(1) == key {
                            return true;
                        }
                        if pop >= 3 && *kptr.add(2) == key {
                            return true;
                        }
                        if pop >= 4 && *kptr.add(3) == key {
                            return true;
                        }
                    }
                    false
                } else if pop <= 8 {
                    // SAFETY: root leaf holds `pop` valid u64 keys.
                    unsafe {
                        if *kptr == key
                            || *kptr.add(1) == key
                            || *kptr.add(2) == key
                            || *kptr.add(3) == key
                        {
                            return true;
                        }
                        if pop >= 5 && *kptr.add(4) == key {
                            return true;
                        }
                        if pop >= 6 && *kptr.add(5) == key {
                            return true;
                        }
                        if pop >= 7 && *kptr.add(6) == key {
                            return true;
                        }
                        if pop >= 8 && *kptr.add(7) == key {
                            return true;
                        }
                    }
                    false
                } else {
                    self.root_leaf_keys().binary_search(&key).is_ok()
                }
            }
            // SAFETY: the trie is maintained by the mutation engine and satisfies the lookup contract.
            Root::Tree { top, .. } => unsafe { get::test_set(top, key, 8) },
        }
    }

    /// Performs batched membership queries for a slice of `keys`, writing boolean presence
    /// flags into `out`. Returns the number of found keys.
    ///
    /// When the root is a multi-level digital trie, `contains_batch` interleaves key descents
    /// across CPU Line Fill Buffers in chunks of 8 keys and issues software prefetch hints
    /// on branch nodes to overlap DRAM memory latency.
    #[inline]
    pub fn contains_batch(&self, keys: &[Key], out: &mut [bool]) -> usize {
        assert_eq!(
            keys.len(),
            out.len(),
            "keys and out slices must have equal length"
        );
        if keys.is_empty() {
            return 0;
        }
        match &self.root {
            Root::Empty => {
                out.fill(false);
                0
            }
            Root::Leaf { .. } => {
                let mut count = 0;
                for (k, o) in keys.iter().zip(out.iter_mut()) {
                    let hit = self.contains(*k);
                    *o = hit;
                    if hit {
                        count += 1;
                    }
                }
                count
            }
            Root::Tree { top, .. } => {
                // SAFETY: tree satisfies lookup invariants.
                unsafe {
                    get::test_set_batch(top, keys, out, 8);
                }
                out.iter().filter(|&&b| b).count()
            }
        }
    }

    /// Membership test returning `c_int` (1 or 0).
    #[inline(always)]
    #[must_use]
    pub fn contains_c_int(&self, key: Key) -> core::ffi::c_int {
        self.contains(key) as core::ffi::c_int
    }

    /// Inserts `key`; returns 1 if newly inserted, 0 if already present.
    #[inline(always)]
    pub fn insert_c_int(&mut self, key: Key) -> core::ffi::c_int {
        self.insert(key) as core::ffi::c_int
    }

    /// Inserts `key`; returns `true` if it was newly inserted.
    #[inline(always)]
    pub fn insert(&mut self, key: Key) -> bool {
        match &mut self.root {
            Root::Empty => {
                let keys = self.alloc.alloc_bytes(root_leaf_size(1));
                // SAFETY: fresh 8-byte allocation, cache-line aligned.
                unsafe { keys.as_ptr().cast::<u64>().write(key) };
                self.root = Root::Leaf { keys, pop: 1 };
                true
            }
            Root::Leaf { keys, pop } => {
                let (keys, pop) = (*keys, *pop);
                // SAFETY: root leaf holds `pop` keys.
                let slice =
                    unsafe { core::slice::from_raw_parts(keys.as_ptr().cast::<u64>(), pop) };
                let at = if pop > 0 {
                    // SAFETY: pop > 0 guarantees slot pop - 1 is in-bounds.
                    let last = unsafe { *keys.as_ptr().cast::<u64>().add(pop - 1) };
                    if key > last {
                        pop
                    } else if key == last {
                        return false;
                    } else {
                        let Err(at) = slice.binary_search(&key) else {
                            return false;
                        };
                        at
                    }
                } else {
                    0
                };
                if pop < ROOT_LEAF_CAP {
                    if root_leaf_size(pop + 1) == root_leaf_size(pop) {
                        // Spare class capacity: shift in place, no
                        // allocation. Without this every insert into a
                        // small array — i.e. every array a C caller keeps
                        // under 32 entries — paid a malloc, a full copy
                        // and a free (issue #1).
                        // SAFETY: the allocation holds the same class of
                        // slots for `pop + 1` as for `pop`.
                        unsafe {
                            let base = keys.as_ptr().cast::<u64>();
                            core::ptr::copy(base.add(at), base.add(at + 1), pop - at);
                            base.add(at).write(key);
                        }
                        self.root = Root::Leaf { keys, pop: pop + 1 };
                        return true;
                    }
                    let new = self.alloc.alloc_bytes(root_leaf_size(pop + 1));
                    // SAFETY: copy `pop` keys around the insertion point
                    // into the fresh (pop + 1)-slot allocation.
                    unsafe {
                        let dst = new.as_ptr().cast::<u64>();
                        dst.copy_from_nonoverlapping(slice.as_ptr(), at);
                        dst.add(at).write(key);
                        dst.add(at + 1)
                            .copy_from_nonoverlapping(slice.as_ptr().add(at), pop - at);
                        self.alloc.free_bytes(keys, root_leaf_size(pop));
                    }
                    self.root = Root::Leaf {
                        keys: new,
                        pop: pop + 1,
                    };
                } else {
                    // Root leaf overflow: build the level-8 trie.
                    let mut top = Edge::NULL;
                    for &k in slice {
                        // SAFETY: trie built and owned by self.alloc.
                        let ins = unsafe { mutate::insert_dyn(&self.alloc, &mut top, k, 8) };
                        debug_assert!(ins);
                    }
                    // SAFETY: same trie; populate path for subsequent sequential/clustered inserts.
                    let ins = unsafe {
                        mutate::insert_path_dyn(&self.alloc, &mut top, key, 8, self.path.get_mut())
                    };
                    debug_assert!(ins);
                    // SAFETY: old root leaf no longer referenced.
                    unsafe { self.alloc.free_bytes(keys, root_leaf_size(pop)) };
                    self.root = Root::Tree {
                        top,
                        pop: pop as u64 + 1,
                    };
                }
                true
            }
            Root::Tree { top, pop } => {
                let prefix = key >> 8;
                let path = self.path.get_mut();
                if path.prefix == prefix {
                    if !path.leaf.is_null() {
                        let d = (key & 0xFF) as u8;
                        // SAFETY: path holds valid live LeafBitmap1 pointer.
                        let leaf = unsafe { &mut *path.leaf };
                        if leaf.bitmap.set(d) {
                            path.pending_pop += 1;
                            path.terminal_pop += 1;
                            // SAFETY: keep terminal edge pop0 up to date.
                            unsafe {
                                (*path.edges[0]).set_pop0(1, (path.terminal_pop - 1) as u64);
                            }
                            if path.terminal_pop == 256 {
                                // SAFETY: terminal edge is valid and rewritten to FullExpanse.
                                unsafe {
                                    path.flush();
                                    let ptr = core::ptr::NonNull::new(leaf);
                                    self.alloc.free_node(ptr.expect("leaf ptr"));
                                    let terminal_edge = &mut *path.edges[0];
                                    *terminal_edge = Edge::NULL;
                                    terminal_edge
                                        .set_tag(crate::types::EdgeType::FullExpanse.as_u8());
                                    terminal_edge.set_pop0(1, 255);
                                    path.clear();
                                }
                            }
                            *pop += 1;
                            return true;
                        } else {
                            return false;
                        }
                    } else if !path.leaf1.is_null() {
                        let d = (key & 0xFF) as u8;
                        let cur_pop = path.terminal_pop as usize;
                        // SAFETY: cur_pop >= 1 when leaf1 is active, so cur_pop - 1 is in bounds.
                        let last = unsafe { *path.leaf1.add(cur_pop - 1) };
                        if d > last {
                            if cur_pop < crate::mutate::LEAF1_CAP
                                && crate::leaf::cap_class(cur_pop + 1)
                                    == crate::leaf::cap_class(cur_pop)
                            {
                                // SAFETY: spare class capacity in the live Leaf1 allocation.
                                unsafe {
                                    *path.leaf1.add(cur_pop) = d;
                                    (*path.edges[0]).set_pop0(1, cur_pop as u64);
                                }
                                path.terminal_pop += 1;
                                path.pending_pop += 1;
                                *pop += 1;
                                return true;
                            }
                        } else if d == last {
                            return false;
                        }
                    }
                }
                path.clear();
                // SAFETY: trie maintained/owned by this set's engine.
                let inserted = unsafe { mutate::insert_path_dyn(&self.alloc, top, key, 8, path) };
                if inserted {
                    *pop += 1;
                }
                inserted
            }
        }
    }

    /// Phase 7 (occ): a by-value snapshot of the root state plus the
    /// allocation handle, for the validated concurrent read walk. The
    /// snapshot may be torn mid-mutation; `sync` validates before use.
    pub(crate) fn occ_root(&self) -> (RootSnapshot, &NodeAlloc) {
        let snap = match self.root {
            Root::Empty => RootSnapshot::Empty,
            Root::Leaf { keys, pop } => RootSnapshot::Leaf {
                ptr: keys.as_ptr(),
                pop,
            },
            Root::Tree { top, pop } => RootSnapshot::Tree { top, pop },
        };
        (snap, &self.alloc)
    }

    /// Removes `key`; returns `true` if it was present.
    pub fn remove(&mut self, key: Key) -> bool {
        self.path.get_mut().clear();
        match &mut self.root {
            Root::Empty => false,
            Root::Leaf { keys, pop } => {
                let (keys, pop) = (*keys, *pop);
                // SAFETY: root leaf holds `pop` keys.
                let slice =
                    unsafe { core::slice::from_raw_parts(keys.as_ptr().cast::<u64>(), pop) };
                let Ok(at) = slice.binary_search(&key) else {
                    return false;
                };
                if pop == 1 {
                    // SAFETY: last key removed; free the leaf.
                    unsafe { self.alloc.free_bytes(keys, root_leaf_size(1)) };
                    self.root = Root::Empty;
                } else if crate::leaf::cap_class(pop - 1) == crate::leaf::cap_class(pop) {
                    // Fast path: capacity class unchanged — shift surviving keys in-place.
                    // SAFETY: in-place shift inside class-sized buffer.
                    unsafe {
                        let ptr = keys.as_ptr().cast::<u64>();
                        core::ptr::copy(ptr.add(at + 1), ptr.add(at), pop - 1 - at);
                    }
                    self.root = Root::Leaf { keys, pop: pop - 1 };
                } else {
                    let new = self.alloc.alloc_bytes(root_leaf_size(pop - 1));
                    // SAFETY: copy the surviving keys into the smaller
                    // allocation.
                    unsafe {
                        let dst = new.as_ptr().cast::<u64>();
                        dst.copy_from_nonoverlapping(slice.as_ptr(), at);
                        dst.add(at)
                            .copy_from_nonoverlapping(slice.as_ptr().add(at + 1), pop - 1 - at);
                        self.alloc.free_bytes(keys, root_leaf_size(pop));
                    }
                    self.root = Root::Leaf {
                        keys: new,
                        pop: pop - 1,
                    };
                }
                true
            }
            Root::Tree { top, pop } => {
                // SAFETY: trie maintained/owned by this set's engine.
                let removed = unsafe { mutate::remove_dyn(&self.alloc, top, key, 8) };
                if removed {
                    *pop -= 1;
                    if *pop == 0 {
                        debug_assert!(top.is_null());
                        self.root = Root::Empty;
                    } else if *pop < ROOT_LEAF_CAP as u64 {
                        // Hysteresis: condense back to a root leaf one
                        // index below the promotion boundary (promote at
                        // CAP + 1, condense at CAP - 1; CAP is stable in
                        // both forms).
                        self.condense_to_root_leaf();
                    }
                }
                removed
            }
        }
    }

    /// Removes every key.
    pub fn clear(&mut self) {
        self.path.get_mut().clear();
        match &mut self.root {
            Root::Empty => {}
            Root::Leaf { keys, pop } => {
                // SAFETY: freeing the root leaf exactly once.
                unsafe { self.alloc.free_bytes(*keys, root_leaf_size(*pop)) };
            }
            Root::Tree { top, .. } => {
                // SAFETY: freeing the whole owned trie exactly once.
                unsafe { mutate::free_subtree::<false>(&self.alloc, top) };
            }
        }
        self.root = Root::Empty;
        debug_assert_eq!(self.alloc.bytes_in_use(), 0);
    }

    /// Walks the whole structure, panicking on any violated invariant
    /// (`docs/TESTING.md`, "Structural invariant validator"). Debug/test
    /// aid; cost is a full tree walk.
    pub fn validate(&self) {
        if let Err(err) = self.validate_defensive() {
            panic!("{err}");
        }
    }

    /// Defensive trie structure validator that does not panic.
    ///
    /// Returns `Ok(())` if the trie invariants are fully met, or `Err(reason)`
    /// indicating what structural corruption was detected.
    pub fn validate_defensive(&self) -> Result<(), String> {
        match &self.root {
            Root::Empty => Ok(()),
            Root::Leaf { pop, .. } => {
                if *pop < 1 || *pop > ROOT_LEAF_CAP {
                    return Err(format!("root leaf pop {pop} out of range"));
                }
                let keys = self.root_leaf_keys();
                if !keys.windows(2).all(|w| w[0] < w[1]) {
                    return Err("root leaf keys unsorted".into());
                }
                Ok(())
            }
            Root::Tree { top, pop } => {
                if top.is_null() {
                    return Err("tree root with null top".into());
                }
                self.flush_path();
                let mut stats = ExpanseStats::default();
                let counted =
                    crate::validate::expanse_validate_and_stats::<false>(top, 8, &mut stats, 0)?;
                if counted != *pop {
                    return Err(format!(
                        "total population {pop} disagrees with tree {counted}"
                    ));
                }
                Ok(())
            }
        }
    }

    /// Gathers structural statistics of the trie.
    #[must_use]
    pub fn stats(&self) -> ExpanseStats {
        let mut stats = ExpanseStats::default();
        match &self.root {
            Root::Empty => {}
            Root::Leaf { pop, .. } => {
                stats.depth_histogram[0] = 1;
                stats.leaf_pop_histogram[*pop] = 1;
                stats.node_counts.leaf_linear = 1;
            }
            Root::Tree { top, .. } => {
                self.flush_path();
                let _ = crate::validate::expanse_validate_and_stats::<false>(top, 8, &mut stats, 0);
            }
        }
        stats
    }
}

impl ExpanseSet {
    /// Smallest key in the set.
    #[must_use]
    pub fn first(&self) -> Option<u64> {
        self.next_at_or_after(0)
    }

    /// Largest key in the set.
    #[must_use]
    pub fn last(&self) -> Option<u64> {
        self.prev_at_or_before(u64::MAX)
    }

    /// Smallest key `>= key` (compat: `Judy1First`).
    #[must_use]
    pub fn next_at_or_after(&self, key: Key) -> Option<u64> {
        match &self.root {
            Root::Empty => None,
            Root::Leaf { .. } => {
                let keys = self.root_leaf_keys();
                keys.get(keys.partition_point(|&k| k < key)).copied()
            }
            Root::Tree { top, .. } => {
                // SAFETY: trie maintained/owned by this set's engine.
                unsafe { crate::nav::next::<false>(top, key, 8) }.map(|e| e.0)
            }
        }
    }

    /// Smallest key `> key` (compat: `Judy1Next`).
    #[must_use]
    pub fn next_after(&self, key: Key) -> Option<u64> {
        self.next_at_or_after(key.checked_add(1)?)
    }

    /// Largest key `<= key` (compat: `Judy1Last`).
    #[must_use]
    pub fn prev_at_or_before(&self, key: Key) -> Option<u64> {
        match &self.root {
            Root::Empty => None,
            Root::Leaf { .. } => {
                let keys = self.root_leaf_keys();
                let i = keys.partition_point(|&k| k <= key).checked_sub(1)?;
                Some(keys[i])
            }
            Root::Tree { top, .. } => {
                // SAFETY: trie maintained/owned by this set's engine.
                unsafe { crate::nav::prev::<false>(top, key, 8) }.map(|e| e.0)
            }
        }
    }

    /// Largest key `< key` (compat: `Judy1Prev`).
    #[must_use]
    pub fn prev_before(&self, key: Key) -> Option<u64> {
        self.prev_at_or_before(key.checked_sub(1)?)
    }

    /// Number of keys strictly below `key` (rank).
    #[must_use]
    pub fn count_below(&self, key: Key) -> u64 {
        self.flush_path();
        match &self.root {
            Root::Empty => 0,
            Root::Leaf { .. } => self.root_leaf_keys().partition_point(|&k| k < key) as u64,
            // SAFETY: trie maintained/owned by this set's engine.
            Root::Tree { top, .. } => unsafe { crate::nav::count_below::<false>(top, key, 8) },
        }
    }

    /// Number of keys in the inclusive range (compat: `Judy1Count`).
    #[must_use]
    pub fn count_range(&self, range: core::ops::RangeInclusive<u64>) -> u64 {
        let (a, b) = (*range.start(), *range.end());
        if a > b {
            return 0;
        }
        self.count_below(b) + u64::from(self.contains(b)) - self.count_below(a)
    }

    /// The key with `n` keys below it — 0-based select (compat:
    /// `Judy1ByCount`, which is 1-based).
    #[must_use]
    pub fn by_count(&self, n: u64) -> Option<u64> {
        if n >= self.len() {
            return None;
        }
        self.flush_path();
        match &self.root {
            Root::Empty => None,
            Root::Leaf { .. } => self.root_leaf_keys().get(n as usize).copied(),
            // SAFETY: trie maintained/owned by this set's engine; n is
            // below the population.
            Root::Tree { top, .. } => Some(unsafe { crate::nav::by_count::<false>(top, n, 8) }.0),
        }
    }

    /// Builds a forward (ascending) raw cursor over all keys.
    fn iter_fwd_raw(&self) -> crate::iter::RawIter<false> {
        match &self.root {
            Root::Empty => crate::iter::RawIter::new(),
            Root::Leaf { pop, .. } => {
                let keys_ptr = self.root_leaf_keys().as_ptr();
                crate::iter::RawIter::from_root_leaf(keys_ptr, core::ptr::null(), *pop)
            }
            // SAFETY: tree maintained by set engine per invariants.
            Root::Tree { top, .. } => unsafe { crate::iter::RawIter::from_tree(top) },
        }
    }

    /// Builds a forward (ascending) raw cursor over keys `>= start`.
    fn range_fwd_raw(&self, start: Key) -> crate::iter::RawIter<false> {
        match &self.root {
            Root::Empty => crate::iter::RawIter::new(),
            Root::Leaf { pop, .. } => {
                let keys_ptr = self.root_leaf_keys().as_ptr();
                // SAFETY: root leaf contains valid keys array of length pop.
                unsafe {
                    crate::iter::RawIter::from_root_leaf_range(
                        keys_ptr,
                        core::ptr::null(),
                        *pop,
                        start,
                    )
                }
            }
            // SAFETY: tree maintained by set engine per invariants.
            Root::Tree { top, .. } => unsafe { crate::iter::RawIter::from_tree_range(top, start) },
        }
    }

    /// The trie root edge for cursor seeks, or `Edge::NULL` when the root is a
    /// flat leaf / empty (cursor seeks then stay leaf-local).
    #[inline]
    fn cursor_top(&self) -> Edge {
        match &self.root {
            Root::Tree { top, .. } => *top,
            _ => Edge::NULL,
        }
    }

    /// Creates a stateful forward [`SetCursor`](crate::cursor::SetCursor) for
    /// monotone skip-scans (WAND / block-max, merge-joins), positioned before
    /// the first key.
    ///
    /// Unlike the stateless [`next_at_or_after`](Self::next_at_or_after), which
    /// re-descends from the root on every call, the cursor keeps its descent
    /// path and re-descends only from the deepest ancestor whose expanse still
    /// covers the next target (issue #340; docs/ALGORITHMS.md §3.5).
    #[must_use]
    pub fn cursor(&self) -> crate::cursor::SetCursor<'_> {
        crate::cursor::SetCursor::new(self.iter_fwd_raw(), self.cursor_top())
    }

    /// Creates a [`SetCursor`](crate::cursor::SetCursor) positioned at the
    /// smallest key `>= start`.
    #[must_use]
    pub fn cursor_from(&self, start: Key) -> crate::cursor::SetCursor<'_> {
        crate::cursor::SetCursor::new(self.range_fwd_raw(start), self.cursor_top())
    }

    /// Ascending iterator over the keys.
    #[must_use]
    pub fn iter(&self) -> SetIter<'_> {
        SetIter {
            _set: core::marker::PhantomData,
            raw: self.iter_fwd_raw(),
        }
    }

    /// Returns an iterator over keys in the inclusive range `[start, end]`.
    #[must_use]
    pub fn range(&self, range: core::ops::RangeInclusive<Key>) -> SetRange<'_> {
        let (start, end) = (*range.start(), *range.end());
        let raw = if start > end {
            crate::iter::RawIter::new()
        } else {
            self.range_fwd_raw(start)
        };
        SetRange {
            _set: core::marker::PhantomData,
            raw,
            end,
        }
    }

    /// Builds a reverse (descending) raw cursor over all keys.
    fn iter_rev_raw(&self) -> crate::iter::RawIter<false> {
        match &self.root {
            Root::Empty => crate::iter::RawIter::new(),
            Root::Leaf { pop, .. } => {
                let keys_ptr = self.root_leaf_keys().as_ptr();
                crate::iter::RawIter::from_root_leaf_rev(keys_ptr, core::ptr::null(), *pop)
            }
            // SAFETY: tree maintained by set engine per invariants.
            Root::Tree { top, .. } => unsafe { crate::iter::RawIter::from_tree_rev(top) },
        }
    }

    /// Builds a reverse (descending) raw cursor over keys `<= end`.
    fn range_rev_raw(&self, end: Key) -> crate::iter::RawIter<false> {
        match &self.root {
            Root::Empty => crate::iter::RawIter::new(),
            Root::Leaf { pop, .. } => {
                let keys_ptr = self.root_leaf_keys().as_ptr();
                // SAFETY: root leaf contains valid keys array of length pop.
                unsafe {
                    crate::iter::RawIter::from_root_leaf_range_rev(
                        keys_ptr,
                        core::ptr::null(),
                        *pop,
                        end,
                    )
                }
            }
            // SAFETY: tree maintained by set engine per invariants.
            Root::Tree { top, .. } => unsafe {
                crate::iter::RawIter::from_tree_range_rev(top, end)
            },
        }
    }

    /// Descending (double-ended) iterator over the keys.
    #[must_use]
    pub fn iter_rev(&self) -> SetIterRev<'_> {
        SetIterRev {
            set: self,
            raw: self.iter_rev_raw(),
            front: None,
            lo: 0,
            hi: Key::MAX,
            done: self.is_empty(),
        }
    }

    /// Returns a descending (double-ended) iterator over keys in the inclusive
    /// range `[start, end]`.
    #[must_use]
    pub fn range_rev(&self, range: core::ops::RangeInclusive<Key>) -> SetRangeRev<'_> {
        let (start, end) = (*range.start(), *range.end());
        let (raw, done) = if start > end {
            (crate::iter::RawIter::new(), true)
        } else {
            (self.range_rev_raw(end), false)
        };
        SetRangeRev {
            set: self,
            raw,
            front: None,
            lo: start,
            hi: end,
            done,
        }
    }
}

/// Ascending key iterator over an [`ExpanseSet`].
///
/// Forward-only by design: the deterministic-Callgrind zero-regression gate
/// (`AGENTS.md` §5) forbids adding any per-element work to this hot path, so
/// reverse iteration lives on the dedicated [`SetIterRev`] / [`SetRangeRev`]
/// types (`iter_rev` / `range_rev`), which are themselves double-ended.
pub struct SetIter<'a> {
    _set: core::marker::PhantomData<&'a ExpanseSet>,
    raw: crate::iter::RawIter<false>,
}

impl Iterator for SetIter<'_> {
    type Item = u64;

    #[inline(always)]
    fn next(&mut self) -> Option<u64> {
        self.raw.next().map(|(k, _)| k)
    }
}

/// Ascending key iterator over a range in an [`ExpanseSet`].
pub struct SetRange<'a> {
    _set: core::marker::PhantomData<&'a ExpanseSet>,
    raw: crate::iter::RawIter<false>,
    end: Key,
}

impl Iterator for SetRange<'_> {
    type Item = u64;

    #[inline(always)]
    fn next(&mut self) -> Option<u64> {
        let (k, _) = self.raw.next()?;
        if k > self.end {
            return None;
        }
        Some(k)
    }
}

/// Double-ended key iterator over an [`ExpanseSet`], descending by default.
///
/// `next` streams descending from the largest key; `next_back` streams
/// ascending from the smallest. The two ends share an inclusive `[lo, hi]`
/// window so interleaved calls never cross: `next` lowers `hi`, `next_back`
/// raises `lo`. The ascending cursor is built lazily.
pub struct SetIterRev<'a> {
    set: &'a ExpanseSet,
    raw: crate::iter::RawIter<false>,
    front: Option<crate::iter::RawIter<false>>,
    lo: Key,
    hi: Key,
    done: bool,
}

impl Iterator for SetIterRev<'_> {
    type Item = u64;

    #[inline]
    fn next(&mut self) -> Option<u64> {
        if self.done {
            return None;
        }
        let (k, _) = self.raw.next_back()?;
        if k < self.lo {
            self.done = true;
            return None;
        }
        if k == self.lo {
            self.done = true;
        } else {
            self.hi = k - 1;
        }
        Some(k)
    }
}

impl DoubleEndedIterator for SetIterRev<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<u64> {
        if self.done {
            return None;
        }
        if self.front.is_none() {
            self.front = Some(self.set.iter_fwd_raw());
        }
        let (k, _) = self.front.as_mut().unwrap().next()?;
        if k > self.hi {
            self.done = true;
            return None;
        }
        if k == self.hi {
            self.done = true;
        } else {
            self.lo = k + 1;
        }
        Some(k)
    }
}

/// Double-ended key iterator over a range in an [`ExpanseSet`], descending by
/// default. See [`SetIterRev`] for the shared-window discipline.
pub struct SetRangeRev<'a> {
    set: &'a ExpanseSet,
    raw: crate::iter::RawIter<false>,
    front: Option<crate::iter::RawIter<false>>,
    lo: Key,
    hi: Key,
    done: bool,
}

impl Iterator for SetRangeRev<'_> {
    type Item = u64;

    #[inline]
    fn next(&mut self) -> Option<u64> {
        if self.done {
            return None;
        }
        let (k, _) = self.raw.next_back()?;
        if k < self.lo {
            self.done = true;
            return None;
        }
        if k == self.lo {
            self.done = true;
        } else {
            self.hi = k - 1;
        }
        Some(k)
    }
}

impl DoubleEndedIterator for SetRangeRev<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<u64> {
        if self.done {
            return None;
        }
        if self.front.is_none() {
            // At first `next_back`, `lo` still equals the original range start.
            self.front = Some(self.set.range_fwd_raw(self.lo));
        }
        let (k, _) = self.front.as_mut().unwrap().next()?;
        if k > self.hi {
            self.done = true;
            return None;
        }
        if k == self.hi {
            self.done = true;
        } else {
            self.lo = k + 1;
        }
        Some(k)
    }
}

impl<'a> IntoIterator for &'a ExpanseSet {
    type Item = u64;
    type IntoIter = SetIter<'a>;

    fn into_iter(self) -> SetIter<'a> {
        self.iter()
    }
}

impl FromIterator<Key> for ExpanseSet {
    fn from_iter<I: IntoIterator<Item = Key>>(iter: I) -> Self {
        let mut set = Self::new();
        set.extend(iter);
        set
    }
}

impl Extend<Key> for ExpanseSet {
    fn extend<I: IntoIterator<Item = Key>>(&mut self, iter: I) {
        for k in iter {
            self.insert(k);
        }
    }
}

impl ExpanseSet {
    /// Bulk-builds a set from an ascending iterator of keys, emitting the trie
    /// bottom-up in one pass (issue #348) rather than inserting key by key.
    ///
    /// The fast path expects **strictly ascending** input; equal adjacent keys
    /// are collapsed. Any out-of-order input is sorted and deduplicated first,
    /// so the result is always correct, but callers get the direct-emission
    /// speed only when the input is already sorted (e.g. a posting list, an
    /// image key stream, or the output of a set-algebra merge).
    ///
    /// The result is content-equivalent to the insert path's tree, canonical,
    /// and never less compact (the builder may pick the more-compact
    /// `FullExpanse` where an ascending insert leaves a full bitmap leaf), so it
    /// composes with every later `insert`/`remove`/query unchanged.
    #[must_use]
    pub fn from_sorted_iter<I: IntoIterator<Item = Key>>(iter: I) -> Self {
        let mut keys: Vec<u64> = iter.into_iter().collect();
        // Fast path leaves an already-sorted, deduplicated stream untouched.
        if !keys.is_empty() {
            let sorted = keys.windows(2).all(|w| w[0] < w[1]);
            if !sorted {
                keys.sort_unstable();
                keys.dedup();
            }
        }
        Self::from_sorted_keys(keys)
    }

    /// Builds a set from a `Vec` of strictly ascending, distinct keys, choosing
    /// the root-leaf or trie representation by population exactly as the insert
    /// ladder would. Consumes the vector.
    pub(crate) fn from_sorted_keys(keys: Vec<u64>) -> Self {
        debug_assert!(
            keys.windows(2).all(|w| w[0] < w[1]),
            "keys must be ascending"
        );
        let mut out = Self::new();
        let n = keys.len();
        if n == 0 {
            return out;
        }
        if n <= ROOT_LEAF_CAP {
            let leaf = out.alloc.alloc_bytes(root_leaf_size(n));
            // SAFETY: `leaf` holds `n` u64 slots (class-sized); write each key.
            unsafe {
                let dst = leaf.as_ptr().cast::<u64>();
                for (i, &k) in keys.iter().enumerate() {
                    dst.add(i).write(k);
                }
            }
            out.root = Root::Leaf { keys: leaf, pop: n };
            return out;
        }
        // SAFETY: `keys` is sorted/distinct; `out.alloc` owns the built trie.
        let top = unsafe { crate::algebra_build::build_subtree(&out.alloc, &keys, 8) };
        out.root = Root::Tree { top, pop: n as u64 };
        out
    }
}

impl ExpanseSet {
    /// Rebuilds the flat sorted root leaf from a small tree (the shrink
    /// twin of the root-leaf → trie promotion).
    fn condense_to_root_leaf(&mut self) {
        self.path.get_mut().clear();
        let Root::Tree { top, pop } = &mut self.root else {
            unreachable!("condense outside tree state")
        };
        let n = *pop as usize;
        debug_assert!((1..ROOT_LEAF_CAP).contains(&n));
        let leaf = self.alloc.alloc_bytes(root_leaf_size(n));
        let mut written = 0usize;
        let mut from = Some(0u64);
        // SAFETY: engine-maintained trie per this type's invariants.
        while let Some((k, _)) = from.and_then(|f| unsafe { crate::nav::next::<false>(top, f, 8) })
        {
            debug_assert!(written < n);
            // SAFETY: in-bounds write of the fresh n-key leaf.
            unsafe { leaf.as_ptr().cast::<u64>().add(written).write(k) };
            written += 1;
            from = k.checked_add(1);
        }
        debug_assert_eq!(written, n);
        // SAFETY: whole trie owned by this set; freed exactly once.
        unsafe { mutate::free_subtree::<false>(&self.alloc, top) };
        self.root = Root::Leaf { keys: leaf, pop: n };
    }
}

/// Counts the keys common to two ascending, deduplicated `u64` slices via a
/// lockstep merge (the small root-leaf intersection path).
fn sorted_intersection_count(a: &[u64], b: &[u64]) -> u64 {
    let (mut i, mut j) = (0usize, 0usize);
    let mut count = 0u64;
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            core::cmp::Ordering::Equal => {
                count += 1;
                i += 1;
                j += 1;
            }
            core::cmp::Ordering::Less => i += 1,
            core::cmp::Ordering::Greater => j += 1,
        }
    }
    count
}

/// Native set algebra (issue #339). The cardinality variants are computed
/// structurally over both tries (`crate::algebra`, `docs/ALGORITHMS.md`);
/// only [`Self::intersection_len`] walks the structure — the other three
/// cardinalities derive from it and the two populations, all `O(1)` given the
/// walk. The result-materializing variants merge the two ordered iterators.
impl ExpanseSet {
    /// Number of keys present in **both** sets (`|A ∩ B|`), computed by
    /// descending both tries in lockstep: whole subtrees absent on one side
    /// are skipped, whole subtrees under a full expanse are counted from the
    /// sibling's `pop0` in `O(1)`, and aligned bitmap leaves are `AND`-ed
    /// word-parallel with `popcnt`. No result is materialized.
    #[must_use]
    pub fn intersection_len(&self, other: &ExpanseSet) -> u64 {
        match (&self.root, &other.root) {
            (Root::Empty, _) | (_, Root::Empty) => 0,
            (Root::Leaf { .. }, Root::Leaf { .. }) => {
                sorted_intersection_count(self.root_leaf_keys(), other.root_leaf_keys())
            }
            (Root::Leaf { .. }, Root::Tree { .. }) => self
                .root_leaf_keys()
                .iter()
                .filter(|&&k| other.contains(k))
                .count() as u64,
            (Root::Tree { .. }, Root::Leaf { .. }) => other
                .root_leaf_keys()
                .iter()
                .filter(|&&k| self.contains(k))
                .count() as u64,
            (Root::Tree { top: ta, .. }, Root::Tree { top: tb, .. }) => {
                // Deferred ancestor pops must be settled before the walk reads
                // `pop0` (the full-expanse shortcut); both cursors are flushed
                // exactly as `validate`/`count_below` do.
                self.flush_path();
                other.flush_path();
                // Pass the smaller side second so the lockstep probes it first
                // and a sparse operand short-circuits each digit.
                let (big, small) = if self.len() >= other.len() {
                    (ta, tb)
                } else {
                    (tb, ta)
                };
                // SAFETY: both trees are live and maintained by the set engine,
                // rooted at level 8.
                unsafe { crate::algebra::intersection_len(big, small, 8) }
            }
        }
    }

    /// Number of keys in the union (`|A ∪ B| = |A| + |B| − |A ∩ B|`), without
    /// materializing the result.
    #[must_use]
    pub fn union_len(&self, other: &ExpanseSet) -> u64 {
        self.len() + other.len() - self.intersection_len(other)
    }

    /// Number of keys in the difference (`|A \ B| = |A| − |A ∩ B|`), without
    /// materializing the result.
    #[must_use]
    pub fn difference_len(&self, other: &ExpanseSet) -> u64 {
        self.len() - self.intersection_len(other)
    }

    /// Number of keys in the symmetric difference
    /// (`|A △ B| = |A| + |B| − 2·|A ∩ B|`), without materializing the result.
    #[must_use]
    pub fn symmetric_difference_len(&self, other: &ExpanseSet) -> u64 {
        self.len() + other.len() - 2 * self.intersection_len(other)
    }

    /// The set of keys present in **both** sets (`A ∩ B`).
    #[must_use]
    pub fn intersection(&self, other: &ExpanseSet) -> ExpanseSet {
        self.materialize_op(other, crate::algebra_build::Op::And)
    }

    /// The set of keys present in **either** set (`A ∪ B`).
    #[must_use]
    pub fn union(&self, other: &ExpanseSet) -> ExpanseSet {
        self.materialize_op(other, crate::algebra_build::Op::Or)
    }

    /// The set of keys in `self` but not `other` (`A \ B`).
    #[must_use]
    pub fn difference(&self, other: &ExpanseSet) -> ExpanseSet {
        self.materialize_op(other, crate::algebra_build::Op::Diff)
    }

    /// The set of keys in exactly one of the two sets (`A △ B`).
    #[must_use]
    pub fn symmetric_difference(&self, other: &ExpanseSet) -> ExpanseSet {
        self.materialize_op(other, crate::algebra_build::Op::Xor)
    }

    /// Direct-emission set algebra (issue #348): when both operands are level-8
    /// tries the result is emitted structurally by the lockstep walk
    /// ([`crate::algebra_build::materialize`]) — bitmap leaves combined word-parallel,
    /// full expanses resolved without enumeration, branches assembled
    /// bottom-up — instead of re-inserting each surviving key. Any other root
    /// combination (empty / small root leaf) merges the two ordered streams and
    /// bulk-builds the result, which is already cheap.
    fn materialize_op(&self, other: &ExpanseSet, op: crate::algebra_build::Op) -> ExpanseSet {
        if let (Root::Tree { top: ta, .. }, Root::Tree { top: tb, .. }) = (&self.root, &other.root)
        {
            // Deferred ancestor pops must be settled before the walk reads
            // `pop0` (the full-expanse shortcut / structural clone).
            self.flush_path();
            other.flush_path();
            let mut out = ExpanseSet::new();
            // SAFETY: both tries are live, engine-maintained, rooted at level 8;
            // the result is built in `out.alloc`.
            let built = unsafe { crate::algebra_build::materialize(&out.alloc, ta, tb, 8, op) };
            let Some(top) = built else {
                return out;
            };
            // SAFETY: `top` is a live level-8 branch built just now.
            let pop = unsafe { crate::algebra_build::tree_pop(&top) };
            if pop as usize <= ROOT_LEAF_CAP {
                // A tiny result belongs in a root leaf, not a trie: drain the
                // built subtree in order, then free it.
                out.condense_built_tree(top, pop as usize);
            } else {
                out.root = Root::Tree { top, pop };
            }
            return out;
        }
        // Empty / root-leaf operands: merge the ordered streams and bulk-build.
        ExpanseSet::from_sorted_keys(merge_sorted(self, other, op))
    }

    /// Installs a freshly-materialized `top` (a level-8 trie of `n` keys, with
    /// `n <= ROOT_LEAF_CAP`) as a root leaf, draining it in order and freeing
    /// the trie. `self` is a fresh set whose allocator owns `top`.
    fn condense_built_tree(&mut self, top: Edge, n: usize) {
        debug_assert!(n <= ROOT_LEAF_CAP);
        if n == 0 {
            let mut t = top;
            // SAFETY: `top` is an empty live subtree owned by this allocator.
            unsafe { mutate::free_subtree::<false>(&self.alloc, &mut t) };
            self.root = Root::Empty;
            return;
        }
        let leaf = self.alloc.alloc_bytes(root_leaf_size(n));
        let mut written = 0usize;
        let mut from = Some(0u64);
        // SAFETY: `top` is a live, engine-maintained level-8 trie.
        while let Some((k, _)) = from.and_then(|f| unsafe { crate::nav::next::<false>(&top, f, 8) })
        {
            debug_assert!(written < n);
            // SAFETY: in-bounds write of the fresh n-key leaf.
            unsafe { leaf.as_ptr().cast::<u64>().add(written).write(k) };
            written += 1;
            from = k.checked_add(1);
        }
        debug_assert_eq!(written, n);
        let mut t = top;
        // SAFETY: the built trie is owned by this allocator; freed exactly once.
        unsafe { mutate::free_subtree::<false>(&self.alloc, &mut t) };
        self.root = Root::Leaf { keys: leaf, pop: n };
    }
}

/// Merges the two sets' ordered key streams under `op` into a sorted, distinct
/// key vector (the root-leaf / empty-operand materialization path).
fn merge_sorted(a: &ExpanseSet, b: &ExpanseSet, op: crate::algebra_build::Op) -> Vec<u64> {
    use crate::algebra_build::Op;
    let mut out = Vec::new();
    let (mut ia, mut ib) = (a.iter(), b.iter());
    let (mut x, mut y) = (ia.next(), ib.next());
    loop {
        let (in_a, in_b, v) = match (x, y) {
            (Some(xv), Some(yv)) => match xv.cmp(&yv) {
                core::cmp::Ordering::Less => {
                    x = ia.next();
                    (true, false, xv)
                }
                core::cmp::Ordering::Greater => {
                    y = ib.next();
                    (false, true, yv)
                }
                core::cmp::Ordering::Equal => {
                    x = ia.next();
                    y = ib.next();
                    (true, true, xv)
                }
            },
            (Some(xv), None) => {
                x = ia.next();
                (true, false, xv)
            }
            (None, Some(yv)) => {
                y = ib.next();
                (false, true, yv)
            }
            (None, None) => break,
        };
        let keep = match op {
            Op::And => in_a && in_b,
            Op::Or => true,
            Op::Diff => in_a && !in_b,
            Op::Xor => in_a != in_b,
        };
        if keep {
            out.push(v);
        }
    }
    out
}

impl core::ops::BitAnd for &ExpanseSet {
    type Output = ExpanseSet;
    /// `A & B` — the intersection ([`ExpanseSet::intersection`]).
    fn bitand(self, rhs: &ExpanseSet) -> ExpanseSet {
        self.intersection(rhs)
    }
}

impl core::ops::BitOr for &ExpanseSet {
    type Output = ExpanseSet;
    /// `A | B` — the union ([`ExpanseSet::union`]).
    fn bitor(self, rhs: &ExpanseSet) -> ExpanseSet {
        self.union(rhs)
    }
}

impl core::ops::Sub for &ExpanseSet {
    type Output = ExpanseSet;
    /// `A - B` — the difference ([`ExpanseSet::difference`]).
    fn sub(self, rhs: &ExpanseSet) -> ExpanseSet {
        self.difference(rhs)
    }
}

impl core::ops::BitXor for &ExpanseSet {
    type Output = ExpanseSet;
    /// `A ^ B` — the symmetric difference
    /// ([`ExpanseSet::symmetric_difference`]).
    fn bitxor(self, rhs: &ExpanseSet) -> ExpanseSet {
        self.symmetric_difference(rhs)
    }
}

impl Default for ExpanseSet {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ExpanseSet {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

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

    #[cfg(miri)]
    const OPS: usize = 250;
    #[cfg(not(miri))]
    const OPS: usize = 6000;

    /// `from_sorted_iter` must produce a tree content-equivalent to the insert
    /// path's — identical contents, canonical, and never less compact
    /// (`mem_used() <=` insert; the builder may pick the more-compact
    /// `FullExpanse` where ascending insert leaves a full bitmap leaf) — and it
    /// must pass the invariants validator.
    #[test]
    fn from_sorted_iter_matches_insert() {
        type Gen = fn(&mut XorShift, usize) -> Vec<u64>;
        let distributions: &[(&str, Gen)] = &[
            ("dense", |_r, n| (0..n as u64).collect()),
            ("clustered", |r, n| {
                let mut v = Vec::new();
                let mut base = 0u64;
                while v.len() < n {
                    base += (r.next() % 4096) + 1;
                    let run = 1 + (r.next() % 64) as usize;
                    for j in 0..run {
                        v.push(base + j as u64);
                    }
                }
                v.truncate(n);
                v
            }),
            ("sparse", |r, n| {
                let mut s = BTreeSet::new();
                while s.len() < n {
                    s.insert(r.next());
                }
                s.into_iter().collect()
            }),
            ("byte-skewed", |r, n| {
                let mut s = BTreeSet::new();
                while s.len() < n {
                    s.insert(r.next() & 0x0003_03FF_00FF_03FF);
                }
                s.into_iter().collect()
            }),
        ];
        for &(name, genf) in distributions {
            for &n in &[
                0usize, 1, 5, 15, 16, 25, 26, 31, 32, 33, 200, 300, 2000, 20_000,
            ] {
                let mut rng = XorShift(0x51ED_0000 ^ n as u64);
                let mut keys = genf(&mut rng, n);
                keys.sort_unstable();
                keys.dedup();

                let built = ExpanseSet::from_sorted_iter(keys.iter().copied());
                built
                    .validate_defensive()
                    .unwrap_or_else(|e| panic!("{name} n={n}: builder invalid: {e}"));

                let mut inserted = ExpanseSet::new();
                for &k in &keys {
                    inserted.insert(k);
                }

                assert_eq!(built.len(), keys.len() as u64, "{name} n={n}: len");
                assert_eq!(
                    built.iter().collect::<Vec<_>>(),
                    keys,
                    "{name} n={n}: contents"
                );
                // The builder emits the same forms the insert ladder converges
                // to, except it may pick the more-compact `FullExpanse` where
                // an ascending insert (which fills a still-skipped bitmap leaf
                // before the parent branch exists) leaves a `LeafBitmap1(256)`.
                // Both are valid and equal in content; the bulk build is never
                // *less* compact than insert.
                assert!(
                    built.mem_used() <= inserted.mem_used(),
                    "{name} n={n}: builder less compact than insert ({} > {})",
                    built.mem_used(),
                    inserted.mem_used()
                );

                // Built tree must remain mutable and correct.
                let mut b2 = ExpanseSet::from_sorted_iter(keys.iter().copied());
                if let Some(&first) = keys.first() {
                    assert!(!b2.insert(first), "re-insert of existing key");
                    assert!(b2.remove(first), "remove existing key");
                    assert!(!b2.contains(first), "removed key still present");
                    b2.validate_defensive()
                        .unwrap_or_else(|e| panic!("{name} n={n}: post-mutation invalid: {e}"));
                }
            }
        }
    }

    /// Direct-emission materialization must match `BTreeSet` for all four ops
    /// across the issue's input categories — including full-expanse-heavy and
    /// self-ops — with every result passing the invariants validator.
    #[test]
    fn materialize_differential() {
        fn set_of(v: &[u64]) -> ExpanseSet {
            let mut s = ExpanseSet::new();
            for &k in v {
                s.insert(k);
            }
            s
        }
        fn check(a: &[u64], b: &[u64], label: &str) {
            let (sa, sb) = (set_of(a), set_of(b));
            let (ma, mb): (BTreeSet<u64>, BTreeSet<u64>) =
                (a.iter().copied().collect(), b.iter().copied().collect());
            let cases: &[(ExpanseSet, Vec<u64>, &str)] = &[
                (
                    sa.intersection(&sb),
                    ma.intersection(&mb).copied().collect(),
                    "and",
                ),
                (sa.union(&sb), ma.union(&mb).copied().collect(), "or"),
                (
                    sa.difference(&sb),
                    ma.difference(&mb).copied().collect(),
                    "diff",
                ),
                (
                    sa.symmetric_difference(&sb),
                    ma.symmetric_difference(&mb).copied().collect(),
                    "xor",
                ),
            ];
            for (got, want, op) in cases {
                got.validate_defensive()
                    .unwrap_or_else(|e| panic!("{label}/{op}: invalid result: {e}"));
                assert_eq!(
                    got.iter().collect::<Vec<_>>(),
                    *want,
                    "{label}/{op}: contents"
                );
                assert_eq!(got.len(), want.len() as u64, "{label}/{op}: len");
            }
        }

        let mut rng = XorShift(0xA1B2_C3D4);
        // dense contiguous ranges → full level-1 and level-2 expanses.
        let dense_a: Vec<u64> = (0..70_000u64).collect();
        let dense_b: Vec<u64> = (30_000..100_000u64).collect();
        check(&dense_a, &dense_b, "dense-overlap");
        check(&dense_a, &dense_a, "dense-self");
        check(
            &(0..65_536u64).collect::<Vec<_>>(),
            &[0, 65_535, 65_536, 200_000],
            "full-l2-vs-sparse",
        );
        check(
            &(0..256u64).collect::<Vec<_>>(),
            &(128..384u64).collect::<Vec<_>>(),
            "full-l1-overlap",
        );

        // clustered runs.
        let clustered = |rng: &mut XorShift, n: usize| -> Vec<u64> {
            let mut v = Vec::new();
            let mut base = 0u64;
            while v.len() < n {
                base += (rng.next() % 8192) + 1;
                for j in 0..(1 + rng.next() % 200) {
                    v.push(base + j);
                }
            }
            v.truncate(n);
            v.sort_unstable();
            v.dedup();
            v
        };
        let ca = clustered(&mut rng, 40_000);
        let cb = clustered(&mut rng, 40_000);
        check(&ca, &cb, "clustered");

        // sparse random (Tree × Tree, little overlap → small AND result).
        let sparse = |rng: &mut XorShift, n: usize| -> Vec<u64> {
            let mut s = BTreeSet::new();
            while s.len() < n {
                s.insert(rng.next());
            }
            s.into_iter().collect()
        };
        let ra = sparse(&mut rng, 5_000);
        let rb = sparse(&mut rng, 5_000);
        check(&ra, &rb, "sparse");

        // zipfian / skewed: many small keys, few large.
        let zipf = |rng: &mut XorShift, n: usize| -> Vec<u64> {
            let mut s = BTreeSet::new();
            while s.len() < n {
                let r = rng.next();
                let k = if r.is_multiple_of(4) { r } else { r % 4096 };
                s.insert(k);
            }
            s.into_iter().collect()
        };
        let za = zipf(&mut rng, 3_000);
        let zb = zipf(&mut rng, 3_000);
        check(&za, &zb, "zipfian");

        // Root-combination coverage: empty, single, small leaf, and big tree.
        let big = sparse(&mut rng, 2_000);
        check(&[], &[], "empty-empty");
        check(&[], &big, "empty-tree");
        check(&big, &[], "tree-empty");
        check(&[42], &big, "single-tree");
        check(&[1, 2, 3], &[3, 4, 5], "leaf-leaf");
        check(&big, &big, "tree-self");
    }

    /// Materialization with real `FullExpanse` operands. The builder emits a
    /// level-1 `FullExpanse` for a fully-populated final byte under a branch, so
    /// `from_sorted_iter` over contiguous blocks yields operands with actual
    /// `FullExpanse` edges — the only way to reach `algebra_build::complement`
    /// (the `full \ B` / `A △ full` arms). Each op is checked against `BTreeSet`
    /// and the result is invariant-validated.
    #[test]
    fn materialize_full_expanse_operands() {
        // 0..512 = two full level-1 expanses under a level-2 branch → level-1
        // FullExpanse edges (verified: builder emits FullExpanse only at level 1
        // under a branch). Also mix a partial block so not every child is full.
        let a_keys: Vec<u64> = (0..512u64).chain(1024..1100).collect();
        let a = ExpanseSet::from_sorted_iter(a_keys.iter().copied());
        a.validate_defensive().unwrap();
        // Confirm the operand actually contains FullExpanse edges (else this test
        // would not exercise the intended path).
        assert!(
            a.stats().node_counts.full_expanse >= 2,
            "operand should carry level-1 FullExpanse edges: {:?}",
            a.stats().node_counts
        );

        let others: &[(&str, Vec<u64>)] = &[
            ("overlap-block", (100..300).chain(1050..1060).collect()),
            ("full-vs-full", (0..512).collect()),
            ("sparse", vec![5, 200, 511, 1024, 1099, 999_999]),
            ("empty", vec![]),
            ("superset", (0..2000).collect()),
        ];
        let ma: BTreeSet<u64> = a_keys.iter().copied().collect();
        for (label, b_keys) in others {
            let b = ExpanseSet::from_sorted_iter(b_keys.iter().copied());
            let mb: BTreeSet<u64> = b_keys.iter().copied().collect();
            let cases: &[(ExpanseSet, Vec<u64>, &str)] = &[
                (
                    a.intersection(&b),
                    ma.intersection(&mb).copied().collect(),
                    "and",
                ),
                (a.union(&b), ma.union(&mb).copied().collect(), "or"),
                (
                    a.difference(&b),
                    ma.difference(&mb).copied().collect(),
                    "diff",
                ),
                (
                    a.symmetric_difference(&b),
                    ma.symmetric_difference(&mb).copied().collect(),
                    "xor",
                ),
            ];
            for (got, want, op) in cases {
                got.validate_defensive()
                    .unwrap_or_else(|e| panic!("{label}/{op}: invalid: {e}"));
                assert_eq!(got.iter().collect::<Vec<_>>(), *want, "{label}/{op}");
            }
        }
    }

    /// Unsorted / duplicate input is corrected, not trusted.
    #[test]
    fn from_sorted_iter_tolerates_unsorted() {
        let raw = [9u64, 3, 3, 7, 1, 1, 9, 100, 50, 50];
        let built = ExpanseSet::from_sorted_iter(raw);
        built.validate_defensive().unwrap();
        assert_eq!(built.iter().collect::<Vec<_>>(), vec![1, 3, 7, 9, 50, 100]);
    }

    /// Runs an op sequence differentially against `BTreeSet`, validating
    /// invariants as it goes, and drains the set at the end.
    fn model_run(seed: u64, gen_key: impl Fn(&mut XorShift) -> u64) {
        let mut rng = XorShift(seed);
        let mut set = ExpanseSet::new();
        let mut model = BTreeSet::new();
        for op in 0..OPS {
            let key = gen_key(&mut rng);
            match rng.next() % 3 {
                0 => assert_eq!(set.insert(key), model.insert(key), "insert {key:#x}"),
                1 => assert_eq!(set.remove(key), model.remove(&key), "remove {key:#x}"),
                _ => assert_eq!(set.contains(key), model.contains(&key), "contains {key:#x}"),
            }
            assert_eq!(set.len(), model.len() as u64);
            if op % 64 == 0 {
                set.validate();
            }
        }
        set.validate();
        // Membership sweep over everything the model holds plus misses.
        for &k in &model {
            assert!(set.contains(k), "model key missing {k:#x}");
        }
        // Drain and confirm all memory returns.
        for &k in &model {
            assert!(set.remove(k));
        }
        set.validate();
        assert!(set.is_empty());
        assert_eq!(set.mem_used(), 0);
    }

    #[test]
    fn model_sequential() {
        model_run(0xA1, |rng| rng.next() % 4096);
    }

    #[test]
    fn model_random_full_width() {
        model_run(0xB2, |rng| rng.next());
    }

    #[test]
    fn model_clustered() {
        // Dense runs at a handful of random 64-bit bases.
        let bases = [
            0u64,
            0xDEAD_BEEF_0000,
            0xFFFF_FFFF_FFFF_FF00,
            0x1234_5678_9ABC_0000,
        ];
        model_run(0xC3, move |rng| {
            let base = bases[(rng.next() % 4) as usize];
            base.wrapping_add(rng.next() % 512)
        });
    }

    #[test]
    fn model_sparse_high_bytes() {
        // Keys differing only in high bytes: one key per deep subexpanse.
        model_run(0xD4, |rng| (rng.next() % 4096) << 48);
    }

    #[test]
    fn model_boundary_keys() {
        model_run(0xE5, |rng| {
            let picks = [
                0u64,
                1,
                255,
                256,
                u64::MAX,
                u64::MAX - 1,
                1 << 32,
                (1 << 32) - 1,
            ];
            match rng.next() % 3 {
                0 => picks[(rng.next() % 8) as usize],
                1 => 1u64 << (rng.next() % 64),
                _ => rng.next() % 300,
            }
        });
    }

    #[test]
    fn ladder_full_climb_and_descend() {
        // 0..=255 under one byte: exercises immed → leaf1 → bitmap leaf →
        // full expanse on the way up and every hysteresis step down.
        // Full validation per op is an O(n) walk; under Miri's interpreter
        // that dominates the whole suite, so sample it there.
        let stride = if cfg!(miri) { 32 } else { 1 };
        let mut set = ExpanseSet::new();
        for k in 0u64..=255 {
            assert!(set.insert(k), "insert {k}");
            if k % stride == 0 {
                set.validate();
            }
        }
        assert_eq!(set.len(), 256);
        for k in 0u64..=255 {
            assert!(set.contains(k));
        }
        assert!(!set.contains(256));
        // Delete out of a full expanse (forces materialization), then all.
        for k in (0u64..=255).rev() {
            assert!(set.remove(k), "remove {k}");
            if k % stride == 0 {
                set.validate();
            }
        }
        assert!(set.is_empty());
        assert_eq!(set.mem_used(), 0);
    }

    #[test]
    fn wide_fanout_reaches_uncompressed_branch() {
        // 4096 keys spread over 256 second-level digits × 16 low digits:
        // the level-2 branch climbs L3 → L7 → B → U.
        let mut set = ExpanseSet::new();
        let mut model = BTreeSet::new();
        for hi in 0u64..256 {
            for lo in 0u64..16 {
                let k = (hi << 8) | lo;
                assert!(set.insert(k));
                model.insert(k);
            }
        }
        set.validate();
        assert_eq!(set.len(), 4096);
        for &k in &model {
            assert!(set.contains(k));
        }
        // Remove most digits to walk back down U → B → L7 → L3.
        for hi in 8u64..256 {
            for lo in 0u64..16 {
                assert!(set.remove((hi << 8) | lo));
            }
            if hi % 32 == 0 {
                set.validate();
            }
        }
        set.validate();
        assert_eq!(set.len(), 8 * 16);
        for hi in 0u64..8 {
            for lo in 0u64..16 {
                assert!(set.contains((hi << 8) | lo));
            }
        }
        set.clear();
        assert_eq!(set.mem_used(), 0);
    }

    #[test]
    fn boundary_thrash_does_not_churn() {
        // Alternate insert/delete across each conversion boundary; the
        // 1-index hysteresis band must keep every op correct (and the
        // validator holds the band's floors/ceilings throughout).
        let mut set = ExpanseSet::new();
        // Sit exactly at the immediate→leaf boundary for 1-byte remainders
        // (15 keys), then oscillate the 16th.
        for k in 0u64..47 {
            set.insert(k * 3);
        }
        for _ in 0..64 {
            assert!(set.insert(1000));
            set.validate();
            assert!(set.remove(1000));
            set.validate();
        }
        set.clear();
        assert_eq!(set.mem_used(), 0);
    }

    #[test]
    fn root_leaf_to_tree_transition() {
        let mut set = ExpanseSet::new();
        for k in 0..=ROOT_LEAF_CAP as u64 {
            assert!(set.insert(k * 0x0101_0101_0101));
            set.validate();
        }
        assert_eq!(set.len(), ROOT_LEAF_CAP as u64 + 1);
        for k in 0..=ROOT_LEAF_CAP as u64 {
            assert!(set.contains(k * 0x0101_0101_0101));
        }
        set.clear();
        assert_eq!(set.mem_used(), 0);
    }

    fn nav_differential(set: &ExpanseSet, model: &BTreeSet<u64>, probes: &BTreeSet<u64>) {
        assert_eq!(set.first(), model.first().copied());
        assert_eq!(set.last(), model.last().copied());
        assert!(set.iter().eq(model.iter().copied()), "iterator order");
        for &k in probes {
            assert_eq!(
                set.next_at_or_after(k),
                model.range(k..).next().copied(),
                "next>={k:#x}"
            );
            assert_eq!(
                set.prev_at_or_before(k),
                model.range(..=k).next_back().copied(),
                "prev<={k:#x}"
            );
            if k < u64::MAX {
                assert_eq!(set.next_after(k), model.range(k + 1..).next().copied());
            }
            if k > 0 {
                assert_eq!(set.prev_before(k), model.range(..k).next_back().copied());
            }
            assert_eq!(
                set.count_below(k),
                model.range(..k).count() as u64,
                "rank {k:#x}"
            );
        }
        for n in 0..model.len().min(64) as u64 {
            assert_eq!(set.by_count(n), model.iter().nth(n as usize).copied());
        }
        assert_eq!(set.by_count(model.len() as u64), None);
        let mut ps = probes.iter();
        while let (Some(&a), Some(&b)) = (ps.next(), ps.next()) {
            let (a, b) = (a.min(b), a.max(b));
            assert_eq!(
                set.count_range(a..=b),
                model.range(a..=b).count() as u64,
                "count {a:#x}..={b:#x}"
            );
        }
    }

    #[test]
    fn navigation_matches_model() {
        let n_rand = if cfg!(miri) { 30 } else { 1500 };
        let mut rng = XorShift(0xFACE_0FF5_1234_5678);
        let mut set = ExpanseSet::new();
        let mut model = BTreeSet::new();
        // Mixed distributions: dense byte run (bitmap leaf → full expanse),
        // sequential, random, clustered, sparse-high, boundary.
        for k in 0u64..=255 {
            set.insert(k);
            model.insert(k);
        }
        for k in 0u64..40 {
            set.insert(0x4000 + k);
            model.insert(0x4000 + k);
        }
        for _ in 0..n_rand {
            let k = match rng.next() % 3 {
                0 => rng.next(),
                1 => 0xAA_BB00_0000 + (rng.next() % 300),
                _ => (rng.next() % 2048) << 48,
            };
            set.insert(k);
            model.insert(k);
        }
        for k in [0u64, 1, u64::MAX, u64::MAX - 1] {
            set.insert(k);
            model.insert(k);
        }
        set.validate();
        let mut probes: BTreeSet<u64> = if cfg!(miri) {
            model.iter().copied().step_by(16).collect()
        } else {
            model.iter().copied().collect()
        };
        for &k in model.iter().take(if cfg!(miri) { 10 } else { 200 }) {
            probes.insert(k.wrapping_add(1));
            probes.insert(k.wrapping_sub(1));
        }
        for _ in 0..if cfg!(miri) { 10 } else { 400 } {
            probes.insert(rng.next());
        }
        nav_differential(&set, &model, &probes);
        set.clear();
    }

    #[test]
    fn navigation_root_leaf_and_empty() {
        let set = ExpanseSet::new();
        assert_eq!(set.first(), None);
        assert_eq!(set.by_count(0), None);
        assert_eq!(set.count_range(0..=u64::MAX), 0);
        assert!(set.iter().next().is_none());

        let mut set = ExpanseSet::new();
        let mut model = BTreeSet::new();
        for k in [5u64, 100, 7, 0, u64::MAX, 1 << 40] {
            set.insert(k);
            model.insert(k);
        }
        let probes: BTreeSet<u64> = (0..64u64).map(|i| i * 0x0404_0404_0404).collect();
        nav_differential(&set, &model, &probes);
    }

    #[test]
    fn model_prefix_runs() {
        // Dense 256-aligned runs at random bases: the narrow-pointer
        // synthesis path (skip-carrying bitmap leaves) under the full
        // op mix.
        model_run(0xF6, |rng| {
            let base = (rng.next() & !0xFF) >> (rng.next() % 3 * 8) << (rng.next() % 3 * 8);
            (base & !0xFF) | (rng.next() % 256)
        });
    }

    #[test]
    fn narrow_pointer_lifecycle() {
        let mut set = ExpanseSet::new();
        let mut model = BTreeSet::new();
        let base = 0xAABB_CCDD_EEFF_0000u64;
        // Fill one 256-run: cascades into a skip-carrying bitmap leaf
        // instead of a 6-node single-child branch chain.
        for i in 0..256u64 {
            assert!(set.insert(base | i));
            model.insert(base | i);
        }
        set.validate();
        // The measured point of the feature: the whole cluster (plus the
        // top-level path) must cost far less than the ~448 bytes a
        // branch chain + bitmap leaf used to take.
        assert!(
            set.mem_used() <= 192,
            "cluster should collapse to one skip edge, used {}",
            set.mem_used()
        );
        for i in 0..256u64 {
            assert!(set.contains(base | i));
        }
        assert!(!set.contains(base ^ (1 << 40)));
        assert_eq!(set.count_range(base..=base | 0xFF), 256);
        assert_eq!(set.by_count(10), Some(base | 10));
        assert_eq!(set.next_at_or_after(base | 0x80), Some(base | 0x80));
        assert_eq!(set.prev_before(base), None);

        // Diverge inside the skipped prefix at several depths: each
        // insert splits the narrow pointer along its divergence path.
        for (i, div) in [1u64 << 16, 1 << 24, 1 << 40, 1 << 55]
            .into_iter()
            .enumerate()
        {
            let k = (base ^ div) | i as u64;
            assert!(set.insert(k), "diverging insert {k:#x}");
            model.insert(k);
            set.validate();
        }
        for &k in &model {
            assert!(set.contains(k), "{k:#x} after splits");
        }
        assert!(set.iter().eq(model.iter().copied()));

        // Drain the cluster through every shrink conversion (bitmap →
        // linear leaf with decode → immediate absorbing the decode).
        for i in (2..256u64).rev() {
            assert!(set.remove(base | i));
            model.remove(&(base | i));
            if i % 16 == 0 {
                set.validate();
            }
        }
        assert!(set.iter().eq(model.iter().copied()));
        set.clear();
        assert_eq!(set.mem_used(), 0);
    }

    #[test]
    fn branch_skip_clusters() {
        let mut set = ExpanseSet::new();
        let mut model = BTreeSet::new();
        let bases = [0x1122_3344_5566_0000u64, 0x99AA_BBCC_DDEE_0000u64];
        let count = if cfg!(miri) { 64u64 } else { 512u64 };
        for &base in &bases {
            for i in 0..count {
                assert!(set.insert(base | i));
                model.insert(base | i);
            }
            set.validate();
        }
        // Two 512-key clusters: per cluster one skip branch (level 2) and
        // two full-expanse children, plus the shared top branch — far
        // below the ~448 bytes/cluster a per-level chain used to take.
        assert!(
            set.mem_used() <= 320,
            "clusters should collapse to skip branches, used {}",
            set.mem_used()
        );
        assert!(set.iter().eq(model.iter().copied()));
        // Ordered navigation across a skipping branch, from inside and
        // outside the skipped prefix.
        assert_eq!(
            set.next_at_or_after(bases[0] | (count / 2)),
            Some(bases[0] | (count / 2))
        );
        assert_eq!(set.next_at_or_after(bases[0] | count), Some(bases[1]));
        assert_eq!(set.prev_before(bases[1]), Some(bases[0] | (count - 1)));
        assert_eq!(set.count_range(bases[0]..=bases[0] | (count - 1)), count);
        assert_eq!(set.by_count(count), Some(bases[1]));
        // Diverge inside the skipped span between the two byte levels.
        let split = bases[0] ^ (0x31 << 24);
        assert!(set.insert(split));
        model.insert(split);
        set.validate();
        assert!(set.iter().eq(model.iter().copied()));
        // Drain one cluster through every downgrade.
        for i in (0..count).rev() {
            assert!(set.remove(bases[0] | i));
            model.remove(&(bases[0] | i));
            if i % 16 == 0 {
                set.validate();
                assert!(set.iter().eq(model.iter().copied()));
            }
        }
        set.clear();
        assert_eq!(set.mem_used(), 0);
    }

    #[test]
    fn root_condenses_and_regrows() {
        let mut set = ExpanseSet::new();
        let mut model = BTreeSet::new();
        for k in 0u64..120 {
            set.insert(k * 0x0101_0101);
            model.insert(k * 0x0101_0101);
        }
        let stride = if cfg!(miri) { 10 } else { 1 };
        // Shrink through the condensation boundary and keep probing.
        for k in (25u64..120).rev() {
            assert!(set.remove(k * 0x0101_0101));
            model.remove(&(k * 0x0101_0101));
            if k % stride == 0 {
                set.validate();
                assert_eq!(set.len(), model.len() as u64);
                assert!(set.iter().eq(model.iter().copied()), "at {k}");
            }
        }
        assert_eq!(set.count_range(0..=u64::MAX), 25);
        // Regrow across the promotion boundary again.
        for k in 200u64..250 {
            assert!(set.insert(k << 32));
            model.insert(k << 32);
        }
        set.validate();
        assert!(set.iter().eq(model.iter().copied()));
        set.clear();
        assert_eq!(set.mem_used(), 0);
    }

    #[test]
    #[should_panic(expected = "branch pop0 disagrees with subtree")]
    fn negative_control_validator_must_fire() {
        // docs/TESTING.md: an assertion that has never fired is not known
        // to work. Corrupt one pop0 and require the validator to abort.
        let mut set = ExpanseSet::new();
        for k in 0u64..200 {
            set.insert(k * 977);
        }
        let Root::Tree { top, .. } = &mut set.root else {
            panic!("expected a tree root");
        };
        // Corrupt: bump the top branch's first child subtree count.
        // SAFETY: the top is a live BranchL3 (one distinct top digit in
        // this key set); we only rewrite the first child edge's aux bytes.
        unsafe {
            let b = &mut *top.node_ptr().cast::<crate::node::BranchL3>();
            let child = &mut b.edges[0];
            let pop0 = child.pop0(7);
            child.set_pop0(7, pop0 + 1);
        }
        set.validate();
    }

    #[test]
    fn dense_leaf_structure_assertion() {
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
        let mut set = ExpanseSet::new();
        let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
        let num_runs = 50;
        for _ in 0..num_runs {
            let prefix = rng.next() & !0xFF;
            for j in 0..32 {
                set.insert(prefix | j);
            }
        }
        set.validate();
        let stats = set.stats();
        // Assert that we have linear leaves of population 32, and NO bitmap leaves!
        assert!(stats.leaf_pop_histogram[32] >= num_runs);
        assert_eq!(stats.node_counts.leaf_bitmap, 0);
        assert!(stats.node_counts.leaf_linear >= num_runs);
    }

    #[test]
    fn test_deferred_ancestor_pop_clustered_and_boundary_flush() {
        let mut set = ExpanseSet::new();
        // Insert multiple 200-key clusters into distinct 256-key expanses
        for cluster in 0..10u64 {
            let prefix = (cluster + 1) << 16;
            for i in 0..200u64 {
                assert!(set.insert(prefix | i));
            }
        }
        assert_eq!(set.len(), 2000);
        // Verify membership during and after deferred flushing
        for cluster in 0..10u64 {
            let prefix = (cluster + 1) << 16;
            for i in 0..200u64 {
                assert!(set.contains(prefix | i));
            }
        }
        // Validation performs full recursive pop0 check at all tree levels
        set.validate();
        assert_eq!(set.len(), 2000);
    }

    #[test]
    fn test_sequential_linear_leaf_cursor_bypass_and_upgrade() {
        let mut set = ExpanseSet::new();
        // Push 1000 sequential items across many linear leaves and bitmap upgrades
        for i in 0..1000u64 {
            assert!(set.insert(i));
            assert!(set.contains(i));
            assert_eq!(set.len(), i + 1);
        }
        set.validate();
        for i in 0..1000u64 {
            assert!(set.contains(i));
        }
        for i in 0..1000u64 {
            assert!(set.remove(i));
            assert!(!set.contains(i));
        }
        set.validate();
        assert!(set.is_empty());
        assert_eq!(set.mem_used(), 0);
    }

    #[test]
    fn test_from_iterator_and_extend() {
        let keys: Vec<u64> = (0..500u64).map(|i| i * 17).collect();
        let set: ExpanseSet = keys.iter().copied().collect();
        assert_eq!(set.len(), 500);
        set.validate();
        for &k in &keys {
            assert!(set.contains(k));
        }

        let mut extended = ExpanseSet::new();
        extended.extend(keys.iter().copied());
        assert_eq!(extended.len(), 500);
        extended.validate();
        for &k in &keys {
            assert!(extended.contains(k));
        }
    }

    #[test]
    fn test_fuzz_set_ops_crash_case() {
        let mut set = ExpanseSet::new();
        let k1 = 4268070197446520623u64;
        let k2 = 0xAABB_CCDD_EE00 + 29555u64;
        let k_c = 0x1122_3344_0000 + 37265u64;
        assert!(set.insert(k1));
        assert!(!set.contains(k_c));
        assert!(!set.contains(k_c));
        assert!(set.insert(k2));
        set.validate();
        assert!(set.contains(k1));
        assert!(set.contains(k2));
        assert!(set.remove(k1));
        assert!(set.remove(k2));
        assert!(set.is_empty());
    }

    #[test]
    fn test_contains_batch_against_single_contains() {
        let mut set = ExpanseSet::new();
        let mut all_keys = Vec::new();

        // 1. Empty set batch test
        let mut out = [true; 8];
        assert_eq!(set.contains_batch(&[1, 2, 3, 4, 5, 6, 7, 8], &mut out), 0);
        assert_eq!(out, [false; 8]);

        // 2. Populate diverse keys
        for i in 0..10_000u64 {
            let k = if i % 3 == 0 {
                i
            } else if i % 3 == 1 {
                i * 1000
            } else {
                i.wrapping_mul(0x1122_3344_5566_7788)
            };
            set.insert(k);
            all_keys.push(k);
        }

        let test_batch_sizes = [0, 1, 2, 7, 8, 9, 15, 16, 64, 100, 1000];
        for &size in &test_batch_sizes {
            if size > all_keys.len() {
                continue;
            }
            let query_keys: Vec<u64> = all_keys[..size]
                .iter()
                .enumerate()
                .map(|(idx, &k)| if idx % 3 == 0 { k + 1 } else { k })
                .collect();

            let mut batch_out = vec![false; size];
            let found_count = set.contains_batch(&query_keys, &mut batch_out);

            let mut expected_found = 0;
            for (idx, &k) in query_keys.iter().enumerate() {
                let single = set.contains(k);
                assert_eq!(batch_out[idx], single, "Mismatch for set key {}", k);
                if single {
                    expected_found += 1;
                }
            }
            assert_eq!(found_count, expected_found);
        }
    }

    // ---- Set algebra (issue #339) ----

    fn set_of(keys: &[u64]) -> ExpanseSet {
        let mut s = ExpanseSet::new();
        for &k in keys {
            s.insert(k);
        }
        s
    }

    /// Checks every native set-algebra kernel (cardinality + materializing +
    /// operators) against `BTreeSet`, and validates the invariants of every
    /// materialized result.
    fn check_algebra(a_keys: &[u64], b_keys: &[u64]) {
        let a = set_of(a_keys);
        let b = set_of(b_keys);
        let ma: BTreeSet<u64> = a_keys.iter().copied().collect();
        let mb: BTreeSet<u64> = b_keys.iter().copied().collect();

        let inter: u64 = ma.intersection(&mb).count() as u64;
        let uni: u64 = ma.union(&mb).count() as u64;
        let diff: u64 = ma.difference(&mb).count() as u64;
        let sym: u64 = ma.symmetric_difference(&mb).count() as u64;

        // Cardinality kernels (both directions; intersection/union/xor are
        // symmetric, difference is not).
        assert_eq!(a.intersection_len(&b), inter, "intersection_len");
        assert_eq!(b.intersection_len(&a), inter, "intersection_len rev");
        assert_eq!(a.union_len(&b), uni, "union_len");
        assert_eq!(b.union_len(&a), uni, "union_len rev");
        assert_eq!(a.difference_len(&b), diff, "difference_len");
        assert_eq!(b.difference_len(&a), mb.difference(&ma).count() as u64);
        assert_eq!(
            a.symmetric_difference_len(&b),
            sym,
            "symmetric_difference_len"
        );

        // Materializing ops + operator sugar; validate each result.
        let checks: [(ExpanseSet, Vec<u64>, &str); 4] = [
            (
                &a & &b,
                ma.intersection(&mb).copied().collect(),
                "intersection",
            ),
            (&a | &b, ma.union(&mb).copied().collect(), "union"),
            (&a - &b, ma.difference(&mb).copied().collect(), "difference"),
            (
                &a ^ &b,
                ma.symmetric_difference(&mb).copied().collect(),
                "symmetric_difference",
            ),
        ];
        for (got, expected, name) in checks {
            got.validate();
            assert_eq!(got.len(), expected.len() as u64, "{name} len");
            assert!(got.iter().eq(expected.iter().copied()), "{name} contents");
        }
    }

    fn build_dist(kind: u8, n: usize, seed: u64) -> Vec<u64> {
        let mut rng = XorShift(0x5EED_0000 ^ seed);
        let mut v: Vec<u64> = match kind {
            // dense contiguous run, offset by seed so pairs half-overlap
            0 => {
                let start = (seed % 4) * (n as u64 / 2).max(1);
                (start..start + n as u64).collect()
            }
            // clustered: bursts of 128 contiguous at random bases
            1 => {
                let universe = (n as u64 * 2).max(2);
                let mut out = Vec::new();
                while out.len() < n {
                    let base = (rng.next() % (universe / 128).max(1)) * 128;
                    for j in 0..128 {
                        out.push(base + j);
                    }
                }
                out.truncate(n);
                out
            }
            // sparse: uniform random over a wide universe
            2 => (0..n).map(|_| rng.next() % (n as u64 * 4).max(2)).collect(),
            // power-law-ish: concentrate on low IDs
            3 => (0..n)
                .map(|_| {
                    let r = rng.next() % 1000;
                    if r < 800 {
                        rng.next() % 256
                    } else {
                        rng.next() % (n as u64 * 4).max(2)
                    }
                })
                .collect(),
            // shard layout: (tenant << 40) | doc
            _ => {
                let mut out = Vec::new();
                for t in 0..16u64 {
                    for d in 0..(n as u64).div_ceil(16) {
                        out.push((t << 40) | d);
                    }
                }
                out.truncate(n);
                out
            }
        };
        v.sort_unstable();
        v.dedup();
        v
    }

    #[test]
    fn algebra_edge_cases() {
        check_algebra(&[], &[]);
        check_algebra(&[1], &[]);
        check_algebra(&[], &[1]);
        check_algebra(&[1], &[1]);
        check_algebra(&[1], &[2]);
        check_algebra(&[0, u64::MAX], &[0, u64::MAX]);
        check_algebra(&[0, 1, 2, 3], &[2, 3, 4, 5]);
        // Root-leaf vs tree (one side small, other promoted past 31).
        let big: Vec<u64> = (0..200u64).map(|k| k * 7).collect();
        check_algebra(&[0, 700, 1400], &big);
        check_algebra(&big, &[0, 700, 1400]);
    }

    #[test]
    fn algebra_distributions() {
        let sizes = if cfg!(miri) {
            &[64usize][..]
        } else {
            &[300usize, 4096][..]
        };
        for kind in 0u8..=4 {
            for &n in sizes {
                let a = build_dist(kind, n, 0);
                let b = build_dist(kind, n, 1);
                check_algebra(&a, &b);
            }
        }
    }

    #[test]
    fn algebra_skewed_sizes() {
        // |B| = |A| / 1000: the subtree-skipping case. A dense, B a sparse
        // sample drawn from A's universe (guaranteed partial overlap).
        let n = if cfg!(miri) { 256usize } else { 20_000 };
        for kind in 0u8..=2 {
            let a = build_dist(kind, n, 0);
            let mut rng = XorShift(0xB10B_0007);
            let nb = (n / 1000).max(4);
            let mut b: Vec<u64> = (0..nb)
                .map(|_| {
                    if rng.next().is_multiple_of(2) && !a.is_empty() {
                        a[(rng.next() as usize) % a.len()] // a hit
                    } else {
                        rng.next() % (n as u64 * 4).max(2) // maybe a miss
                    }
                })
                .collect();
            b.sort_unstable();
            b.dedup();
            check_algebra(&a, &b);
        }
    }

    #[test]
    fn algebra_disjoint_high_expanses() {
        // Sets whose keys live under different top-byte expanses: the lockstep
        // must skip the entire non-overlapping subtrees (intersection empty).
        let a: Vec<u64> = (0..500u64).collect();
        let b: Vec<u64> = (0..500u64).map(|k| (1u64 << 56) | k).collect();
        check_algebra(&a, &b);
        assert_eq!(set_of(&a).intersection_len(&set_of(&b)), 0);
    }
}
