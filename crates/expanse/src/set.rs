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
    path: crate::mutate::InsertPath,
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
            path: crate::mutate::InsertPath::empty(),
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
    #[must_use]
    pub fn contains(&self, key: Key) -> bool {
        match &self.root {
            Root::Empty => false,
            Root::Leaf { .. } => self.root_leaf_keys().binary_search(&key).is_ok(),
            // SAFETY: the trie is maintained by the mutation engine and
            // satisfies the lookup contract.
            Root::Tree { top, .. } => unsafe { get::test_set(top, key, 8) },
        }
    }

    /// Inserts `key`; returns `true` if it was newly inserted.
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
                        mutate::insert_path_dyn(&self.alloc, &mut top, key, 8, &mut self.path)
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
                if self.path.prefix == prefix && !self.path.leaf.is_null() {
                    let d = (key & 0xFF) as u8;
                    // SAFETY: path holds valid live LeafBitmap1 pointer.
                    let leaf = unsafe { &mut *self.path.leaf };
                    if leaf.bitmap.set(d) {
                        self.path.pending_pop += 1;
                        // SAFETY: path.depth >= 1 and edges[0] is the live leaf edge.
                        let terminal_pop = unsafe { (*self.path.edges[0]).pop0(1) } as usize
                            + 1
                            + self.path.pending_pop;
                        if terminal_pop == 256 {
                            // SAFETY: terminal edge is valid and rewritten to FullExpanse.
                            unsafe {
                                self.path.flush();
                                let ptr = core::ptr::NonNull::new(leaf);
                                self.alloc.free_node(ptr.expect("leaf ptr"));
                                let terminal_edge = &mut *self.path.edges[0];
                                *terminal_edge = Edge::NULL;
                                terminal_edge.set_tag(crate::types::EdgeType::FullExpanse.as_u8());
                                terminal_edge.set_pop0(1, 255);
                                self.path.clear();
                            }
                        }
                        *pop += 1;
                        return true;
                    } else {
                        return false;
                    }
                }
                self.path.clear();
                // SAFETY: trie maintained/owned by this set's engine.
                let inserted =
                    unsafe { mutate::insert_path_dyn(&self.alloc, top, key, 8, &mut self.path) };
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
        self.path.clear();
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
        self.path.clear();
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
                // SAFETY: flushing pending population before validating invariant counts.
                unsafe {
                    let mut_self = (self as *const Self as *mut Self).as_mut().unwrap();
                    mut_self.path.flush();
                }
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
                // SAFETY: flushing pending population before gathering stats.
                unsafe {
                    let mut_self = (self as *const Self as *mut Self).as_mut().unwrap();
                    mut_self.path.flush();
                }
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
        // SAFETY: flushing pending population before rank traversal.
        unsafe {
            let mut_self = (self as *const Self as *mut Self).as_mut().unwrap();
            mut_self.path.flush();
        }
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
        // SAFETY: flushing pending population before select traversal.
        unsafe {
            let mut_self = (self as *const Self as *mut Self).as_mut().unwrap();
            mut_self.path.flush();
        }
        match &self.root {
            Root::Empty => None,
            Root::Leaf { .. } => self.root_leaf_keys().get(n as usize).copied(),
            // SAFETY: trie maintained/owned by this set's engine; n is
            // below the population.
            Root::Tree { top, .. } => Some(unsafe { crate::nav::by_count::<false>(top, n, 8) }.0),
        }
    }

    /// Ascending iterator over the keys.
    #[must_use]
    pub fn iter(&self) -> SetIter<'_> {
        SetIter {
            set: self,
            from: Some(0),
        }
    }
}

/// Ascending key iterator over an [`ExpanseSet`].
pub struct SetIter<'a> {
    set: &'a ExpanseSet,
    from: Option<u64>,
}

impl Iterator for SetIter<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        let Some(found) = self.set.next_at_or_after(self.from?) else {
            self.from = None;
            return None;
        };
        self.from = found.checked_add(1);
        Some(found)
    }
}

impl<'a> IntoIterator for &'a ExpanseSet {
    type Item = u64;
    type IntoIter = SetIter<'a>;

    fn into_iter(self) -> SetIter<'a> {
        self.iter()
    }
}

impl ExpanseSet {
    /// Rebuilds the flat sorted root leaf from a small tree (the shrink
    /// twin of the root-leaf → trie promotion).
    fn condense_to_root_leaf(&mut self) {
        self.path.clear();
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
        let n_rand = if cfg!(miri) { 100 } else { 1500 };
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
        let mut probes: BTreeSet<u64> = model.iter().copied().collect();
        for &k in model.iter().take(200) {
            probes.insert(k.wrapping_add(1));
            probes.insert(k.wrapping_sub(1));
        }
        if cfg!(miri) {
            let sampled: Vec<u64> = probes.iter().copied().step_by(8).collect();
            probes = sampled.into_iter().collect();
        }
        for _ in 0..if cfg!(miri) { 40 } else { 400 } {
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
        // Clusters spanning two key bytes (divergence level 2): the leaf
        // cascade must place one skipping branch at level 2 instead of a
        // single-child chain down from the slot level.
        let mut set = ExpanseSet::new();
        let mut model = BTreeSet::new();
        let bases = [0x1122_3344_5566_0000u64, 0x99AA_BBCC_DDEE_0000u64];
        for &base in &bases {
            for i in 0..512u64 {
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
            set.next_at_or_after(bases[0] | 0x17F),
            Some(bases[0] | 0x17F)
        );
        assert_eq!(set.next_at_or_after(bases[0] | 0x200), Some(bases[1]));
        assert_eq!(set.prev_before(bases[1]), Some(bases[0] | 0x1FF));
        assert_eq!(set.count_range(bases[0]..=bases[0] | 0x1FF), 512);
        assert_eq!(set.by_count(512), Some(bases[1]));
        // Diverge inside the skipped span between the two byte levels.
        let split = bases[0] ^ (0x31 << 24);
        assert!(set.insert(split));
        model.insert(split);
        set.validate();
        assert!(set.iter().eq(model.iter().copied()));
        // Drain one cluster through every downgrade.
        for i in (0..512u64).rev() {
            assert!(set.remove(bases[0] | i));
            model.remove(&(bases[0] | i));
            if i % 64 == 0 {
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
        // Shrink through the condensation boundary and keep probing.
        for k in (25u64..120).rev() {
            assert!(set.remove(k * 0x0101_0101));
            model.remove(&(k * 0x0101_0101));
            set.validate();
            assert_eq!(set.len(), model.len() as u64);
            assert!(set.iter().eq(model.iter().copied()), "at {k}");
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
}
