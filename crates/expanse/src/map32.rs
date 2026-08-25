//! 32-Bit Ordered Associative Map / JudyL Array (`ExpanseMap32`).
//!
//! A real 256-ary digital trie mapping 32-bit keys (`u32`) to 32-bit values
//! (`u32`), decoding one byte per level across four levels (`L4 -> L1`),
//! specialised for embedded SRAM and microcontrollers per
//! `docs/design/32-bit-embedded.md`. Small subexpanses pack a single key/value
//! *immediately* inside the 8-byte edge (zero heap); denser ones use packed
//! linear leaves (`[values: u32 * pop][keys: KB * pop]`) and
//! linear/uncompressed branches (see [`crate::trie32`]).
//!
//! This type is single-threaded: it holds no synchronisation and is not a
//! lock-free structure. The concurrent wrapper described in the RFC is not
//! yet implemented for the 32-bit engine.

use core::fmt;

use crate::trie32::{self, Arena};
use crate::types32::{Edge32, Key32, Value32};

/// An ordered map from 32-bit integer keys to 32-bit values, backed by a
/// digital trie.
pub struct ExpanseMap32 {
    /// Node arena backing the trie (byte-exact memory accounting).
    alloc: Arena,
    /// Root edge of the trie (level 4). May itself hold an immediate entry.
    root: Edge32,
    /// Number of entries currently present.
    len: usize,
}

impl ExpanseMap32 {
    /// Create a new, empty 32-bit map.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            alloc: Arena::new(),
            root: Edge32::null(),
            len: 0,
        }
    }

    /// Insert a key-value pair into the map.
    ///
    /// Returns the old value if the key was already present, or `None` if
    /// newly inserted.
    #[inline]
    pub fn insert(&mut self, key: Key32, value: Value32) -> Option<Value32> {
        let old = trie32::map_insert(&mut self.alloc, &mut self.root, 4, key, value);
        if old.is_none() {
            self.len += 1;
        }
        old
    }

    /// Lookup a key in the map, returning its 32-bit value if found.
    #[inline]
    #[must_use]
    pub fn get(&self, key: Key32) -> Option<Value32> {
        trie32::map_get(&self.alloc, &self.root, 4, key)
    }

    /// Check if a key exists in the map.
    #[inline]
    #[must_use]
    pub fn contains_key(&self, key: Key32) -> bool {
        self.get(key).is_some()
    }

    /// Remove a key from the map, returning its value if present.
    #[inline]
    pub fn remove(&mut self, key: Key32) -> Option<Value32> {
        let old = trie32::map_remove(&mut self.alloc, &mut self.root, 4, key);
        if old.is_some() {
            self.len -= 1;
        }
        old
    }

    /// Returns the number of entries in the map.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if the map is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Clear all entries from the map, releasing all node allocations.
    #[inline]
    pub fn clear(&mut self) {
        self.alloc = Arena::new();
        self.root = Edge32::null();
        self.len = 0;
    }

    /// Returns the smallest `(key, value)` entry in the map, if any.
    #[inline]
    #[must_use]
    pub fn first(&self) -> Option<(Key32, Value32)> {
        let k = trie32::first(&self.alloc, &self.root, 4)?;
        Some((k, self.get(k).expect("first key present")))
    }

    /// Returns the largest `(key, value)` entry in the map, if any.
    #[inline]
    #[must_use]
    pub fn last(&self) -> Option<(Key32, Value32)> {
        let k = trie32::last(&self.alloc, &self.root, 4)?;
        Some((k, self.get(k).expect("last key present")))
    }

    /// Returns the entry with the smallest key strictly greater than `key`.
    #[inline]
    #[must_use]
    pub fn next(&self, key: Key32) -> Option<(Key32, Value32)> {
        let k = trie32::next(&self.alloc, &self.root, 4, key)?;
        Some((k, self.get(k).expect("next key present")))
    }

    /// Returns the entry with the largest key strictly smaller than `key`.
    #[inline]
    #[must_use]
    pub fn prev(&self, key: Key32) -> Option<(Key32, Value32)> {
        let k = trie32::prev(&self.alloc, &self.root, 4, key)?;
        Some((k, self.get(k).expect("prev key present")))
    }

    /// Returns the number of keys present in the inclusive range
    /// `[start, end]`.
    #[inline]
    #[must_use]
    pub fn count_range(&self, start: Key32, end: Key32) -> usize {
        if start > end {
            return 0;
        }
        trie32::count_range(&self.alloc, &self.root, 4, start, end)
    }

    /// Scan entries in the inclusive range `[start, end]`, filtering by
    /// predicate before invoking the callback.
    #[inline]
    pub fn scan_filtered<P, F>(&self, start: Key32, end: Key32, mut pred: P, mut cb: F)
    where
        P: FnMut(Key32, Value32) -> bool,
        F: FnMut(Key32, Value32),
    {
        if start > end {
            return;
        }
        trie32::map_for_each_range(&self.alloc, &self.root, 4, start, end, &mut |k, v| {
            if pred(k, v) {
                cb(k, v);
            }
        });
    }

    /// Real bytes of node/leaf storage held by this map. Zero when empty;
    /// immediate (in-edge) entries cost nothing. Mirrors the 64-bit
    /// `ExpanseMap::mem_used` accounting.
    #[inline]
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.alloc.bytes_in_use()
    }
}

impl Default for ExpanseMap32 {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ExpanseMap32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_map();
        let mut cur = self.first();
        while let Some((k, v)) = cur {
            dbg.entry(&k, &v);
            cur = self.next(k);
        }
        dbg.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(feature = "std"))]
    extern crate alloc as alloc_crate;
    #[cfg(not(feature = "std"))]
    use alloc_crate::collections::BTreeMap;
    #[cfg(not(feature = "std"))]
    use alloc_crate::vec::Vec;
    #[cfg(feature = "std")]
    use std::collections::BTreeMap;

    #[test]
    fn basic_mutations() {
        let mut map = ExpanseMap32::new();
        assert!(map.is_empty());
        assert_eq!(map.mem_used(), 0);

        assert_eq!(map.insert(100, 1000), None);
        assert_eq!(map.insert(200, 2000), None);
        assert_eq!(map.insert(100, 1500), Some(1000));

        assert_eq!(map.len(), 2);
        assert_eq!(map.get(100), Some(1500));
        assert_eq!(map.get(200), Some(2000));
        assert_eq!(map.get(300), None);

        assert_eq!(map.remove(100), Some(1500));
        assert_eq!(map.get(100), None);
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn scan_filtered_works() {
        let mut map = ExpanseMap32::new();
        for i in 1..=10u32 {
            map.insert(i * 10, i * 100);
        }
        let mut collected = Vec::new();
        map.scan_filtered(20, 80, |_k, v| v > 400, |k, v| collected.push((k, v)));
        assert_eq!(collected, [(50, 500), (60, 600), (70, 700), (80, 800)]);
    }

    #[test]
    fn boundaries() {
        let mut map = ExpanseMap32::new();
        for &k in &[0u32, 1, u32::MAX, u32::MAX - 1, 0x00FF_FF00] {
            assert_eq!(map.insert(k, k ^ 0xDEAD), None);
        }
        assert_eq!(map.first(), Some((0, 0xDEAD)));
        assert_eq!(map.last(), Some((u32::MAX, u32::MAX ^ 0xDEAD)));
        assert_eq!(map.get(u32::MAX), Some(u32::MAX ^ 0xDEAD));
        assert_eq!(map.remove(u32::MAX), Some(u32::MAX ^ 0xDEAD));
        assert_eq!(map.last(), Some((u32::MAX - 1, (u32::MAX - 1) ^ 0xDEAD)));
    }

    struct XorShift(u64);
    impl XorShift {
        fn new(seed: u64) -> Self {
            Self(seed | 1)
        }
        fn next(&mut self) -> u32 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            (x >> 16) as u32
        }
    }

    fn differential(seed: u64, ops: usize, key_mask: u32) {
        let mut rng = XorShift::new(seed);
        let mut map = ExpanseMap32::new();
        let mut model: BTreeMap<u32, u32> = BTreeMap::new();

        for _ in 0..ops {
            let k = rng.next() & key_mask;
            let v = rng.next();
            match rng.next() % 5 {
                0 | 1 => assert_eq!(map.insert(k, v), model.insert(k, v), "insert {k:#x}"),
                2 => assert_eq!(map.remove(k), model.remove(&k), "remove {k:#x}"),
                _ => assert_eq!(map.get(k), model.get(&k).copied(), "get {k:#x}"),
            }
            assert_eq!(map.len(), model.len());
        }

        let expected: Vec<(u32, u32)> = model.iter().map(|(&k, &v)| (k, v)).collect();
        let mut got = Vec::new();
        let mut cur = map.first();
        while let Some((k, v)) = cur {
            got.push((k, v));
            cur = map.next(k);
        }
        assert_eq!(got, expected, "forward ordering seed={seed}");
        assert_eq!(map.first(), expected.first().copied());
        assert_eq!(map.last(), expected.last().copied());

        for &(lo, hi) in &[(0u32, key_mask), (0, key_mask / 2)] {
            let exp = expected
                .iter()
                .filter(|&&(k, _)| k >= lo && k <= hi)
                .count();
            assert_eq!(map.count_range(lo, hi), exp, "count_range({lo:#x},{hi:#x})");
        }

        for &(k, v) in &expected {
            assert_eq!(map.remove(k), Some(v), "drain {k:#x}");
        }
        assert!(map.is_empty());
        assert_eq!(map.mem_used(), 0, "leak after drain seed={seed}");
    }

    #[cfg(not(miri))]
    const OPS: usize = 20_000;
    #[cfg(miri)]
    const OPS: usize = 400;

    #[test]
    fn differential_dense() {
        for seed in [1u64, 2, 3] {
            differential(seed, OPS, 0x0000_03FF);
        }
    }

    #[test]
    fn differential_sparse() {
        for seed in [11u64, 22, 33] {
            differential(seed, OPS, 0xFFFF_FFFF);
        }
    }

    #[test]
    fn differential_midrange() {
        for seed in [101u64, 202] {
            differential(seed, OPS, 0x000F_FFFF);
        }
    }
}
