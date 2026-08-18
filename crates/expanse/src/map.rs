//! Phase 6b: `ExpanseMap`, the public map-flavor tree (compat: JudyL).
//!
//! Root organization mirrors `ExpanseSet`: populations up to
//! [`crate::set::ROOT_LEAF_CAP`] live in a root leaf — here parallel
//! sorted-key and value arrays in one allocation — before a level-8 trie
//! exists. Same v1 simplification: the tree never shrinks back into a
//! root leaf.

use crate::alloc::NodeAlloc;
use crate::get;
use crate::mutate;
use crate::mutate_map;
use crate::node::Edge;
use crate::set::ROOT_LEAF_CAP;
use crate::types::Key;
use core::ptr::NonNull;

enum Root {
    Empty,
    /// One allocation: `pop` sorted keys, then `pop` values.
    Leaf {
        ptr: NonNull<u8>,
        pop: usize,
    },
    /// A level-8 trie; `pop` is the total population (the JPM role).
    Tree {
        top: Edge,
        pop: u64,
    },
}

/// A sparse, dynamic map from `u64` keys to `u64` values (compat: JudyL).
///
/// Adaptive expanse-partitioned trie: memory stays near-proportional to
/// population across sequential, random, clustered, and sparse key
/// distributions, and lookups run in at most eight digit steps.
pub struct ExpanseMap {
    root: Root,
    alloc: NodeAlloc,
}

const fn leaf_size(pop: usize) -> usize {
    16 * pop
}

impl ExpanseMap {
    /// Creates an empty map.
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: Root::Empty,
            alloc: NodeAlloc::new(),
        }
    }

    /// Number of keys in the map.
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

    /// Heap bytes currently used by the map's nodes and leaves.
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.alloc.bytes_in_use()
    }

    fn leaf_parts(ptr: NonNull<u8>, pop: usize) -> (&'static [u64], *mut u64) {
        // SAFETY: the root leaf holds `pop` keys then `pop` values, both
        // 8-aligned (allocations are cache-line aligned). The lifetime is
        // scoped by callers to the borrow of self.
        unsafe {
            (
                core::slice::from_raw_parts(ptr.as_ptr().cast::<u64>(), pop),
                ptr.as_ptr().cast::<u64>().add(pop),
            )
        }
    }

    /// Returns the value stored for `key`.
    #[must_use]
    pub fn get(&self, key: Key) -> Option<u64> {
        match &self.root {
            Root::Empty => None,
            Root::Leaf { ptr, pop } => {
                let (keys, vals) = Self::leaf_parts(*ptr, *pop);
                let at = keys.binary_search(&key).ok()?;
                // SAFETY: `at < pop` values live behind the keys.
                Some(unsafe { *vals.add(at) })
            }
            // SAFETY: the trie is maintained by the map mutation engine
            // and satisfies the lookup contract.
            Root::Tree { top, .. } => unsafe { get::get_map(top, key, 8) },
        }
    }

    /// Membership test.
    #[must_use]
    pub fn contains_key(&self, key: Key) -> bool {
        self.get(key).is_some()
    }

    /// Inserts `key → val`; returns the replaced value if the key was
    /// already present.
    pub fn insert(&mut self, key: Key, val: u64) -> Option<u64> {
        match &mut self.root {
            Root::Empty => {
                let ptr = self.alloc.alloc_bytes(leaf_size(1));
                // SAFETY: fresh allocation: key slot then value slot.
                unsafe {
                    ptr.as_ptr().cast::<u64>().write(key);
                    ptr.as_ptr().cast::<u64>().add(1).write(val);
                }
                self.root = Root::Leaf { ptr, pop: 1 };
                None
            }
            Root::Leaf { ptr, pop } => {
                let (ptr, pop) = (*ptr, *pop);
                let (keys, vals) = Self::leaf_parts(ptr, pop);
                match keys.binary_search(&key) {
                    Ok(at) => {
                        // SAFETY: in-place value swap.
                        unsafe {
                            let slot = vals.add(at);
                            let old = *slot;
                            slot.write(val);
                            Some(old)
                        }
                    }
                    Err(at) if pop < ROOT_LEAF_CAP => {
                        let new = self.alloc.alloc_bytes(leaf_size(pop + 1));
                        // SAFETY: copy keys and values around the insertion
                        // point into the fresh (pop + 1)-entry leaf.
                        unsafe {
                            let nk = new.as_ptr().cast::<u64>();
                            nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                            nk.add(at).write(key);
                            nk.add(at + 1)
                                .copy_from_nonoverlapping(keys.as_ptr().add(at), pop - at);
                            let nv = nk.add(pop + 1);
                            nv.copy_from_nonoverlapping(vals, at);
                            nv.add(at).write(val);
                            nv.add(at + 1)
                                .copy_from_nonoverlapping(vals.add(at), pop - at);
                            self.alloc.free_bytes(ptr, leaf_size(pop));
                        }
                        self.root = Root::Leaf {
                            ptr: new,
                            pop: pop + 1,
                        };
                        None
                    }
                    Err(_) => {
                        // Root leaf overflow: build the level-8 trie.
                        let mut top = Edge::NULL;
                        for (at, &k) in keys.iter().enumerate() {
                            // SAFETY: trie built and owned by self.alloc;
                            // values read in-bounds.
                            let prev = unsafe {
                                mutate_map::map_insert(&self.alloc, &mut top, k, *vals.add(at), 8)
                            };
                            debug_assert!(prev.is_none());
                        }
                        // SAFETY: same trie.
                        let prev =
                            unsafe { mutate_map::map_insert(&self.alloc, &mut top, key, val, 8) };
                        debug_assert!(prev.is_none());
                        // SAFETY: old root leaf no longer referenced.
                        unsafe { self.alloc.free_bytes(ptr, leaf_size(pop)) };
                        self.root = Root::Tree {
                            top,
                            pop: pop as u64 + 1,
                        };
                        None
                    }
                }
            }
            Root::Tree { top, pop } => {
                // SAFETY: trie maintained/owned by this map's engine.
                let prev = unsafe { mutate_map::map_insert(&self.alloc, top, key, val, 8) };
                if prev.is_none() {
                    *pop += 1;
                }
                prev
            }
        }
    }

    /// Removes `key`; returns its value if it was present.
    pub fn remove(&mut self, key: Key) -> Option<u64> {
        match &mut self.root {
            Root::Empty => None,
            Root::Leaf { ptr, pop } => {
                let (ptr, pop) = (*ptr, *pop);
                let (keys, vals) = Self::leaf_parts(ptr, pop);
                let at = keys.binary_search(&key).ok()?;
                // SAFETY: in-bounds value read.
                let old = unsafe { *vals.add(at) };
                if pop == 1 {
                    // SAFETY: last entry removed; free the leaf.
                    unsafe { self.alloc.free_bytes(ptr, leaf_size(1)) };
                    self.root = Root::Empty;
                } else {
                    let new = self.alloc.alloc_bytes(leaf_size(pop - 1));
                    // SAFETY: copy the surviving keys/values into the
                    // smaller leaf.
                    unsafe {
                        let nk = new.as_ptr().cast::<u64>();
                        nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                        nk.add(at)
                            .copy_from_nonoverlapping(keys.as_ptr().add(at + 1), pop - 1 - at);
                        let nv = nk.add(pop - 1);
                        nv.copy_from_nonoverlapping(vals, at);
                        nv.add(at)
                            .copy_from_nonoverlapping(vals.add(at + 1), pop - 1 - at);
                        self.alloc.free_bytes(ptr, leaf_size(pop));
                    }
                    self.root = Root::Leaf {
                        ptr: new,
                        pop: pop - 1,
                    };
                }
                Some(old)
            }
            Root::Tree { top, pop } => {
                // SAFETY: trie maintained/owned by this map's engine.
                let old = unsafe { mutate_map::map_remove(&self.alloc, top, key, 8) };
                if old.is_some() {
                    *pop -= 1;
                    if *pop == 0 {
                        debug_assert!(top.is_null());
                        self.root = Root::Empty;
                    }
                }
                old
            }
        }
    }

    /// Removes every entry.
    pub fn clear(&mut self) {
        match &mut self.root {
            Root::Empty => {}
            Root::Leaf { ptr, pop } => {
                // SAFETY: freeing the root leaf exactly once.
                unsafe { self.alloc.free_bytes(*ptr, leaf_size(*pop)) };
            }
            Root::Tree { top, .. } => {
                // SAFETY: freeing the whole owned trie exactly once.
                unsafe { mutate::free_subtree::<true>(&self.alloc, top) };
            }
        }
        self.root = Root::Empty;
        debug_assert_eq!(self.alloc.bytes_in_use(), 0);
    }

    /// Walks the whole structure, panicking on any violated invariant
    /// (`docs/TESTING.md`, "Structural invariant validator").
    pub fn validate(&self) {
        match &self.root {
            Root::Empty => {}
            Root::Leaf { ptr, pop } => {
                assert!(
                    *pop >= 1 && *pop <= ROOT_LEAF_CAP,
                    "root leaf pop out of range"
                );
                let (keys, _) = Self::leaf_parts(*ptr, *pop);
                assert!(keys.windows(2).all(|w| w[0] < w[1]), "root leaf unsorted");
            }
            Root::Tree { top, pop } => {
                assert!(!top.is_null(), "tree root with null top");
                // SAFETY: trie maintained/owned by this map's engine.
                let counted = unsafe { mutate::validate_subtree::<true>(top, 8) };
                assert_eq!(counted, *pop, "total population disagrees with tree");
            }
        }
    }
}

impl Default for ExpanseMap {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ExpanseMap {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
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

    #[cfg(miri)]
    const OPS: usize = 250;
    #[cfg(not(miri))]
    const OPS: usize = 6000;

    fn model_run(seed: u64, gen_key: impl Fn(&mut XorShift) -> u64) {
        let mut rng = XorShift(seed);
        let mut map = ExpanseMap::new();
        let mut model = BTreeMap::new();
        for op in 0..OPS {
            let key = gen_key(&mut rng);
            match rng.next() % 4 {
                0 | 3 => {
                    let val = rng.next();
                    assert_eq!(map.insert(key, val), model.insert(key, val), "ins {key:#x}");
                }
                1 => assert_eq!(map.remove(key), model.remove(&key), "rem {key:#x}"),
                _ => assert_eq!(map.get(key), model.get(&key).copied(), "get {key:#x}"),
            }
            assert_eq!(map.len(), model.len() as u64);
            if op % 64 == 0 {
                map.validate();
            }
        }
        map.validate();
        for (&k, &v) in &model {
            assert_eq!(map.get(k), Some(v), "model entry {k:#x}");
        }
        let keys: Vec<u64> = model.keys().copied().collect();
        for k in keys {
            assert_eq!(map.remove(k), model.remove(&k));
        }
        map.validate();
        assert!(map.is_empty());
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    fn model_sequential() {
        model_run(0x1A, |rng| rng.next() % 4096);
    }

    #[test]
    fn model_random_full_width() {
        model_run(0x2B, |rng| rng.next());
    }

    #[test]
    fn model_clustered() {
        let bases = [
            0u64,
            0xDEAD_BEEF_0000,
            0xFFFF_FFFF_FFFF_FF00,
            0x1234_5678_9ABC_0000,
        ];
        model_run(0x3C, move |rng| {
            let base = bases[(rng.next() % 4) as usize];
            base.wrapping_add(rng.next() % 512)
        });
    }

    #[test]
    fn model_sparse_high_bytes() {
        model_run(0x4D, |rng| (rng.next() % 4096) << 48);
    }

    #[test]
    fn ladder_full_climb_and_descend() {
        // 0..=255 with distinct values: immed → linear map leaf → bitmap
        // map leaf on the way up (no full expanse for maps), every
        // hysteresis step down, values intact throughout.
        let stride = if cfg!(miri) { 32 } else { 1 };
        let mut map = ExpanseMap::new();
        for k in 0u64..=255 {
            assert_eq!(map.insert(k, k * 7 + 1), None);
            if k % stride == 0 {
                map.validate();
            }
        }
        assert_eq!(map.len(), 256);
        for k in 0u64..=255 {
            assert_eq!(map.get(k), Some(k * 7 + 1), "value of {k}");
        }
        // Replacement must preserve structure and return the old value.
        assert_eq!(map.insert(100, 9999), Some(701));
        assert_eq!(map.get(100), Some(9999));
        map.validate();
        for k in (0u64..=255).rev() {
            let expected = if k == 100 { 9999 } else { k * 7 + 1 };
            assert_eq!(map.remove(k), Some(expected), "remove {k}");
            if k % stride == 0 {
                map.validate();
            }
        }
        assert!(map.is_empty());
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    fn wide_keys_drain_leaf_to_null() {
        // 7-byte remainders have map_immed_max == 1: leaves must drain to
        // null, never to an empty immediate.
        let mut map = ExpanseMap::new();
        let keys: Vec<u64> = (0..40u64)
            .map(|i| i.wrapping_mul(0x0000_FF12_3456_789B) & ((1 << 56) - 1))
            .collect();
        for (i, &k) in keys.iter().enumerate() {
            assert_eq!(map.insert(k, i as u64), None);
        }
        map.validate();
        for (i, &k) in keys.iter().enumerate() {
            assert_eq!(map.remove(k), Some(i as u64));
        }
        map.validate();
        assert!(map.is_empty());
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    fn wide_fanout_reaches_uncompressed_branch() {
        let mut map = ExpanseMap::new();
        for hi in 0u64..256 {
            for lo in 0u64..16 {
                let k = (hi << 8) | lo;
                assert_eq!(map.insert(k, !k), None);
            }
        }
        map.validate();
        assert_eq!(map.len(), 4096);
        for hi in 8u64..256 {
            for lo in 0u64..16 {
                let k = (hi << 8) | lo;
                assert_eq!(map.remove(k), Some(!k));
            }
            if hi % 64 == 0 {
                map.validate();
            }
        }
        map.validate();
        assert_eq!(map.len(), 8 * 16);
        map.clear();
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    #[should_panic(expected = "branch pop0 disagrees with subtree")]
    fn negative_control_validator_must_fire() {
        let mut map = ExpanseMap::new();
        for k in 0u64..200 {
            map.insert(k * 977, k);
        }
        let Root::Tree { top, .. } = &mut map.root else {
            panic!("expected a tree root");
        };
        // SAFETY: the top is a live BranchL3 (one distinct top digit in
        // this key set); we only rewrite the first child edge's aux bytes.
        unsafe {
            let b = &mut *top.node_ptr().cast::<crate::node::BranchL3>();
            let child = &mut b.edges[0];
            let pop0 = child.pop0(7);
            child.set_pop0(7, pop0 + 1);
        }
        map.validate();
    }
}
