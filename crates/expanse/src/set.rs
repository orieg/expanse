//! Phase 6: `ExpanseSet`, the public set-flavor tree (compat: Judy1).
//!
//! Root organization follows the original's economy: populations up to
//! [`ROOT_LEAF_CAP`] live in a **root leaf** — one sorted array of full
//! 8-byte keys — and only past that does a real level-8 trie exist (whose
//! total population this struct tracks, the original's JPM role). v1
//! simplification: the tree never shrinks back into a root leaf; deleting
//! the last key returns the set to `Empty`.

use crate::alloc::NodeAlloc;
use crate::get;
use crate::mutate;
use crate::node::Edge;
use crate::types::Key;
use core::ptr::NonNull;

/// Maximum population held in the root leaf before a real trie is built.
pub const ROOT_LEAF_CAP: usize = 31;

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
}

impl ExpanseSet {
    /// Creates an empty set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: Root::Empty,
            alloc: NodeAlloc::new(),
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
                let keys = self.alloc.alloc_bytes(8);
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
                let Err(at) = slice.binary_search(&key) else {
                    return false;
                };
                if pop < ROOT_LEAF_CAP {
                    let new = self.alloc.alloc_bytes(8 * (pop + 1));
                    // SAFETY: copy `pop` keys around the insertion point
                    // into the fresh (pop + 1)-slot allocation.
                    unsafe {
                        let dst = new.as_ptr().cast::<u64>();
                        dst.copy_from_nonoverlapping(slice.as_ptr(), at);
                        dst.add(at).write(key);
                        dst.add(at + 1)
                            .copy_from_nonoverlapping(slice.as_ptr().add(at), pop - at);
                        self.alloc.free_bytes(keys, 8 * pop);
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
                        let ins = unsafe { mutate::insert(&self.alloc, &mut top, k, 8) };
                        debug_assert!(ins);
                    }
                    // SAFETY: same trie.
                    let ins = unsafe { mutate::insert(&self.alloc, &mut top, key, 8) };
                    debug_assert!(ins);
                    // SAFETY: old root leaf no longer referenced.
                    unsafe { self.alloc.free_bytes(keys, 8 * pop) };
                    self.root = Root::Tree {
                        top,
                        pop: pop as u64 + 1,
                    };
                }
                true
            }
            Root::Tree { top, pop } => {
                // SAFETY: trie maintained/owned by this set's engine.
                let inserted = unsafe { mutate::insert(&self.alloc, top, key, 8) };
                if inserted {
                    *pop += 1;
                }
                inserted
            }
        }
    }

    /// Removes `key`; returns `true` if it was present.
    pub fn remove(&mut self, key: Key) -> bool {
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
                    unsafe { self.alloc.free_bytes(keys, 8) };
                    self.root = Root::Empty;
                } else {
                    let new = self.alloc.alloc_bytes(8 * (pop - 1));
                    // SAFETY: copy the surviving keys into the smaller
                    // allocation.
                    unsafe {
                        let dst = new.as_ptr().cast::<u64>();
                        dst.copy_from_nonoverlapping(slice.as_ptr(), at);
                        dst.add(at)
                            .copy_from_nonoverlapping(slice.as_ptr().add(at + 1), pop - 1 - at);
                        self.alloc.free_bytes(keys, 8 * pop);
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
                let removed = unsafe { mutate::remove(&self.alloc, top, key, 8) };
                if removed {
                    *pop -= 1;
                    if *pop == 0 {
                        debug_assert!(top.is_null());
                        self.root = Root::Empty;
                    }
                }
                removed
            }
        }
    }

    /// Removes every key.
    pub fn clear(&mut self) {
        match &mut self.root {
            Root::Empty => {}
            Root::Leaf { keys, pop } => {
                // SAFETY: freeing the root leaf exactly once.
                unsafe { self.alloc.free_bytes(*keys, 8 * *pop) };
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
        match &self.root {
            Root::Empty => {}
            Root::Leaf { pop, .. } => {
                assert!(
                    *pop >= 1 && *pop <= ROOT_LEAF_CAP,
                    "root leaf pop out of range"
                );
                let keys = self.root_leaf_keys();
                assert!(keys.windows(2).all(|w| w[0] < w[1]), "root leaf unsorted");
            }
            Root::Tree { top, pop } => {
                assert!(!top.is_null(), "tree root with null top");
                // SAFETY: trie maintained/owned by this set's engine.
                let counted = unsafe { mutate::validate_subtree::<false>(top, 8) };
                assert_eq!(counted, *pop, "total population disagrees with tree");
            }
        }
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
}
