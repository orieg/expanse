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
//! This type is single-threaded: it holds no synchronisation and is not an
//! optimistic-read structure. The single-writer/many-reader wrapper is
//! [`crate::sync32`] — an opt-in, fixed-capacity surface layered on top;
//! this type's expanse-proportional memory behaviour is unchanged.

use core::default::Default;
use core::fmt;
use core::option::Option;

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

    /// A fixed-capacity, deferred-reclamation instance for the concurrent
    /// wrapper (`sync32`). Not public: rigid preallocation trades away the
    /// §2.1 expanse-proportional memory invariant, so it is opt-in through
    /// the concurrent surface only, where the trade is declared.
    #[must_use]
    pub(crate) fn with_fixed_arena(node_cap: usize, pending_cap: usize) -> Self {
        Self {
            alloc: Arena::with_capacity(node_cap, pending_cap),
            root: Edge32::null(),
            len: 0,
        }
    }

    /// The node arena (optimistic read path).
    #[inline]
    pub(crate) fn arena(&self) -> &Arena {
        &self.alloc
    }

    /// Mutable arena access (writer-side reclamation).
    #[inline]
    pub(crate) fn arena_mut(&mut self) -> &mut Arena {
        &mut self.alloc
    }

    /// A racy copy of the root edge; the caller must validate its version
    /// word before acting on the copy.
    #[inline]
    pub(crate) fn root_edge(&self) -> Edge32 {
        self.root
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

    /// Look up a batch of 32-bit keys simultaneously, writing results into `out`.
    #[inline]
    pub fn get_batch(&self, keys: &[Key32], out: &mut [Option<Value32>]) {
        assert_eq!(
            keys.len(),
            out.len(),
            "keys and out slices must have equal length"
        );
        for (k, o) in keys.iter().zip(out.iter_mut()) {
            *o = self.get(*k);
        }
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

    /// Removes every entry whose key lies in `range`, calling `f(key, value)`
    /// for each removed entry in ascending key order, and returns how many
    /// were removed.
    ///
    /// One descent to the range plus one structural fix-up per touched
    /// node, where a `first`/`remove` loop pays two full descents per
    /// entry — the TTL-eviction shape (#578). The callback keeps this
    /// allocation-free for callers that need the removed values back (a
    /// slab index to recycle, say).
    pub fn remove_range<F: FnMut(Key32, Value32)>(
        &mut self,
        range: core::ops::RangeInclusive<Key32>,
        mut f: F,
    ) -> usize {
        let (lo, hi) = (*range.start(), *range.end());
        if lo > hi {
            return 0;
        }
        let n = trie32::map_remove_range(&mut self.alloc, &mut self.root, 4, 0, lo, hi, &mut f);
        self.len -= n;
        n
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
        trie32::first_entry(&self.alloc, &self.root, 4)
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

    /// Smallest entry with key `>= bound`, if any.
    #[inline]
    #[must_use]
    pub fn next_at_or_after(&self, bound: Key32) -> Option<(Key32, Value32)> {
        self.first_ge(bound)
    }

    /// Smallest entry with key `> bound`, if any.
    #[inline]
    #[must_use]
    pub fn next_after(&self, bound: Key32) -> Option<(Key32, Value32)> {
        self.next(bound)
    }

    /// Largest entry with key `<= bound`, if any.
    #[inline]
    #[must_use]
    pub fn prev_at_or_before(&self, bound: Key32) -> Option<(Key32, Value32)> {
        self.last_le(bound)
    }

    /// Largest entry with key `< bound`, if any.
    #[inline]
    #[must_use]
    pub fn prev_before(&self, bound: Key32) -> Option<(Key32, Value32)> {
        self.prev(bound)
    }

    /// Smallest entry with key `>= bound`, if any.
    #[inline]
    fn first_ge(&self, bound: Key32) -> Option<(Key32, Value32)> {
        if bound == 0 {
            self.first()
        } else {
            self.next(bound - 1)
        }
    }

    /// Largest entry with key `<= bound`, if any.
    #[inline]
    fn last_le(&self, bound: Key32) -> Option<(Key32, Value32)> {
        if bound == Key32::MAX {
            self.last()
        } else {
            self.prev(bound + 1)
        }
    }

    /// Double-ended iterator over all entries in ascending key order.
    ///
    /// Cursor-based (`first`/`next` forward, `last`/`prev` backward); the two
    /// ends share `[lo, hi]` bounds so interleaved calls never cross.
    #[inline]
    #[must_use]
    pub fn iter(&self) -> MapIter32<'_> {
        MapIter32 {
            map: self,
            lo: 0,
            hi: Key32::MAX,
            done: self.is_empty(),
        }
    }

    /// Double-ended iterator over entries in the inclusive range `[start, end]`.
    #[inline]
    #[must_use]
    pub fn range(&self, range: core::ops::RangeInclusive<Key32>) -> MapIter32<'_> {
        let (start, end) = (*range.start(), *range.end());
        MapIter32 {
            map: self,
            lo: start,
            hi: end,
            done: start > end || self.is_empty(),
        }
    }

    /// Descending iterator over all entries.
    #[inline]
    #[must_use]
    pub fn iter_rev(&self) -> MapRangeRev32<'_> {
        MapRangeRev32 {
            map: self,
            lo: 0,
            hi: Key32::MAX,
            done: self.is_empty(),
        }
    }

    /// Descending iterator over entries in the inclusive range `[start, end]`.
    #[inline]
    #[must_use]
    pub fn range_rev(&self, range: core::ops::RangeInclusive<Key32>) -> MapRangeRev32<'_> {
        let (start, end) = (*range.start(), *range.end());
        MapRangeRev32 {
            map: self,
            lo: start,
            hi: end,
            done: start > end || self.is_empty(),
        }
    }
}

/// Double-ended entry iterator over an [`ExpanseMap32`] (ascending by default).
pub struct MapIter32<'a> {
    map: &'a ExpanseMap32,
    lo: Key32,
    hi: Key32,
    done: bool,
}

impl Iterator for MapIter32<'_> {
    type Item = (Key32, Value32);

    #[inline]
    fn next(&mut self) -> Option<(Key32, Value32)> {
        if self.done {
            return None;
        }
        let (k, v) = self.map.first_ge(self.lo)?;
        if k > self.hi {
            self.done = true;
            return None;
        }
        if k == self.hi {
            self.done = true;
        } else {
            self.lo = k + 1;
        }
        Some((k, v))
    }
}

impl DoubleEndedIterator for MapIter32<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<(Key32, Value32)> {
        if self.done {
            return None;
        }
        let (k, v) = self.map.last_le(self.hi)?;
        if k < self.lo {
            self.done = true;
            return None;
        }
        if k == self.lo {
            self.done = true;
        } else {
            self.hi = k - 1;
        }
        Some((k, v))
    }
}

/// Descending entry iterator over an [`ExpanseMap32`] range.
pub struct MapRangeRev32<'a> {
    map: &'a ExpanseMap32,
    lo: Key32,
    hi: Key32,
    done: bool,
}

impl Iterator for MapRangeRev32<'_> {
    type Item = (Key32, Value32);

    #[inline]
    fn next(&mut self) -> Option<(Key32, Value32)> {
        if self.done {
            return None;
        }
        let (k, v) = self.map.last_le(self.hi)?;
        if k < self.lo {
            self.done = true;
            return None;
        }
        if k == self.lo {
            self.done = true;
        } else {
            self.hi = k - 1;
        }
        Some((k, v))
    }
}

impl DoubleEndedIterator for MapRangeRev32<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<(Key32, Value32)> {
        if self.done {
            return None;
        }
        let (k, v) = self.map.first_ge(self.lo)?;
        if k > self.hi {
            self.done = true;
            return None;
        }
        if k == self.hi {
            self.done = true;
        } else {
            self.lo = k + 1;
        }
        Some((k, v))
    }
}

impl<'a> IntoIterator for &'a ExpanseMap32 {
    type Item = (Key32, Value32);
    type IntoIter = MapIter32<'a>;

    #[inline]
    fn into_iter(self) -> MapIter32<'a> {
        self.iter()
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

        // Iterator agreement (forward, reverse, ranges) vs BTreeMap.
        assert_eq!(map.iter().collect::<Vec<_>>(), expected, "iter seed={seed}");
        let rev: Vec<(u32, u32)> = expected.iter().rev().copied().collect();
        assert_eq!(
            map.iter_rev().collect::<Vec<_>>(),
            rev,
            "iter_rev seed={seed}"
        );
        assert_eq!(
            map.iter().rev().collect::<Vec<_>>(),
            rev,
            "iter().rev() seed={seed}"
        );
        for &(lo, hi) in &[
            (0u32, key_mask),
            (0, key_mask / 2),
            (key_mask / 4, key_mask / 2),
            (key_mask / 2, key_mask / 2),
        ] {
            let fwd: Vec<(u32, u32)> = expected
                .iter()
                .copied()
                .filter(|&(k, _)| k >= lo && k <= hi)
                .collect();
            let bwd: Vec<(u32, u32)> = fwd.iter().rev().copied().collect();
            assert_eq!(
                map.range(lo..=hi).collect::<Vec<_>>(),
                fwd,
                "range({lo:#x},{hi:#x})"
            );
            assert_eq!(
                map.range_rev(lo..=hi).collect::<Vec<_>>(),
                bwd,
                "range_rev({lo:#x},{hi:#x})"
            );
            assert_eq!(
                map.range(lo..=hi).rev().collect::<Vec<_>>(),
                bwd,
                "range({lo:#x},{hi:#x}).rev()"
            );
        }
        // Interleaved next()/next_back(): the two ends must never cross.
        {
            let mut it = map.iter();
            let mut lo = 0usize;
            let mut hi = expected.len();
            let mut front = true;
            while lo < hi {
                if front {
                    assert_eq!(it.next(), Some(expected[lo]), "interleave front");
                    lo += 1;
                } else {
                    hi -= 1;
                    assert_eq!(it.next_back(), Some(expected[hi]), "interleave back");
                }
                front = !front;
            }
            assert_eq!(it.next(), None);
            assert_eq!(it.next_back(), None);
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

    /// A workload toggling entries across the `MapLeaf(1)` -> `LeafBitmapL_32`
    /// boundary must not rebuild the node on every single operation.
    ///
    /// `MapLeaf(1)` promotes to `LeafBitmapL_32` on the 65th entry (`MAP_BITMAP_ENTER_32 = 64`).
    /// `LeafBitmapL_32` uses Band 16 hysteresis: demotes to `MapLeaf(1)` at <= 48 entries (`MAP_BITMAP_LEAVE_32 = 48`).
    #[test]
    fn map_bitmap_leaf_boundary_toggle_keeps_node_form() {
        let key = |rem: u32| 0x0100_0000 | rem;
        let mut map = ExpanseMap32::new();

        // 64 keys sharing a prefix: a populated 1-byte MapLeaf.
        for rem in 0..64u32 {
            map.insert(key(rem), rem * 10);
        }
        let at_64 = map.mem_used();

        // 65th key exceeds MapLeaf(1) capacity and promotes to LeafBitmapL_32 (MapBitmap).
        map.insert(key(64), 640);
        let at_65 = map.mem_used();
        assert!(
            at_65 > at_64,
            "expected promotion to LeafBitmapL_32 (MapBitmap); {at_64} -> {at_65}"
        );

        // Removing 65th key leaves 64 entries. With Band 16 hysteresis,
        // it stays as LeafBitmapL_32 (demotes only at <= 48).
        assert_eq!(map.remove(key(64)), Some(640));
        let after_64_left = map.mem_used();
        assert!(
            after_64_left > at_64,
            "node should keep LeafBitmapL_32 form at 64 entries (demotes at <= 48); at_64={at_64}, after={after_64_left}"
        );

        // Remove down to 49 entries: still LeafBitmapL_32.
        for rem in 49..64u32 {
            assert!(map.remove(key(rem)).is_some());
        }
        let at_49 = map.mem_used();
        assert!(
            at_49 >= 96,
            "node should keep LeafBitmapL_32 header at 49 entries"
        );

        // Removing the 49th entry leaves 48 entries: demotes to MapLeaf(1)!
        assert!(map.remove(key(48)).is_some());
        let at_48 = map.mem_used();
        assert!(
            at_48 < at_49,
            "expected demotion to MapLeaf(1) at <= 48 entries; {at_49} -> {at_48}"
        );
    }

    /// `remove_range` against a `BTreeMap` model over shapes that reach every
    /// node kind (dense runs for level-1 bitmaps, sparse keys for immediates
    /// and linear leaves, wide spreads for the branch flavours), with random
    /// inclusive ranges; also pins `first()` to the model after every step.
    #[test]
    fn remove_range_matches_btreemap_model() {
        let mut state = 0x2F6E_2B1Fu32;
        let mut lcg = move || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            state
        };
        for round in 0..40u32 {
            let mut map = ExpanseMap32::new();
            let mut model: BTreeMap<u32, u32> = BTreeMap::new();
            let n = 200 + (lcg() % 3_000) as usize;
            for i in 0..n {
                let k = match lcg() % 4 {
                    0 => lcg() % 512,                      // dense low bytes -> bitmaps
                    1 => (lcg() % 64) << 24 | (lcg() % 8), // sparse high digits -> immediates/leaves
                    2 => 0x0100_0000 + lcg() % 100_000,    // one populous subtree -> branches
                    _ => lcg(),                            // anywhere
                };
                let v = (i as u32) ^ round;
                assert_eq!(map.insert(k, v), model.insert(k, v));
            }
            let mut steps = 0;
            while !model.is_empty() && steps < 64 {
                steps += 1;
                assert_eq!(map.first(), model.iter().next().map(|(&k, &v)| (k, v)));
                let (lo, hi) = match lcg() % 3 {
                    0 => (0, lcg() % 1_024), // prefix (the eviction shape)
                    1 => {
                        let a = lcg();
                        (a, a.saturating_add(lcg() % 0x0100_0000))
                    }
                    _ => {
                        let a = lcg();
                        (a.min(a ^ 0xFF), a.max(a ^ 0xFF))
                    }
                };
                let mut seen = Vec::new();
                let n = map.remove_range(lo..=hi, |k, v| seen.push((k, v)));
                let expected: Vec<(u32, u32)> =
                    model.range(lo..=hi).map(|(&k, &v)| (k, v)).collect();
                assert_eq!(
                    seen, expected,
                    "round {round} step {steps} range {lo:#x}..={hi:#x}"
                );
                assert_eq!(n, expected.len());
                model.retain(|&k, _| k < lo || k > hi);
                assert_eq!(map.len(), model.len());
                for (&k, &v) in &model {
                    assert_eq!(
                        map.get(k),
                        Some(v),
                        "survivor {k:#x} after range {lo:#x}..={hi:#x}"
                    );
                }
                assert!(map.iter().eq(model.iter().map(|(&k, &v)| (k, v))));
            }
            // Drain whatever is left in one call and check nothing leaked.
            let left = model.len();
            assert_eq!(map.remove_range(0..=u32::MAX, |_, _| {}), left);
            assert!(map.is_empty());
            assert_eq!(
                map.mem_used(),
                ExpanseMap32::new().mem_used(),
                "byte leak after drain"
            );
        }
    }

    #[test]
    fn remove_range_empty_and_inverted_ranges_are_noops() {
        let mut map = ExpanseMap32::new();
        for k in [1u32, 5, 9, 0xFFFF_FFFF] {
            map.insert(k, k);
        }
        assert_eq!(
            map.remove_range(6..=8, |_, _| panic!("nothing in range")),
            0
        );
        let (lo, hi) = (9u32, 5u32);
        assert_eq!(map.remove_range(lo..=hi, |_, _| panic!("inverted")), 0);
        assert_eq!(map.len(), 4);
        let mut got = Vec::new();
        assert_eq!(
            map.remove_range(0xFFFF_FFFF..=0xFFFF_FFFF, |k, _| got.push(k)),
            1
        );
        assert_eq!(got, [0xFFFF_FFFF]);
        assert_eq!(map.last(), Some((9, 9)));
    }

    /// Regression for the batched bitmap arm (#578, caught by the 32-bit C
    /// test in CI): a level-1 map bitmap stores `pop0 = count - 1`, so
    /// removing a whole expanse must not underflow it, and the free /
    /// demote thresholds are in terms of the remaining count.
    #[test]
    fn remove_range_clears_a_whole_map_bitmap_expanse() {
        let empty = ExpanseMap32::new().mem_used();
        // > MAP_BITMAP_ENTER keys inside one 256-key expanse -> map bitmap.
        let mut map = ExpanseMap32::new();
        for k in 0..100u32 {
            map.insert(0x0101_0000 + k, k);
        }
        let mut n = 0;
        assert_eq!(
            map.remove_range(0x0101_0000..=0x0101_00FF, |_, _| n += 1),
            100
        );
        assert_eq!(n, 100);
        assert!(map.is_empty());
        assert_eq!(
            map.mem_used(),
            empty,
            "node leak after clearing the expanse"
        );

        // Partial removal down to exactly one survivor must demote, not free.
        let mut map = ExpanseMap32::new();
        for k in 0..100u32 {
            map.insert(0x0101_0000 + k, k);
        }
        assert_eq!(map.remove_range(0x0101_0001..=0x0101_00FF, |_, _| {}), 99);
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(0x0101_0000), Some(0));
        assert_eq!(map.first(), Some((0x0101_0000, 0)));
        assert_eq!(map.remove(0x0101_0000), Some(0));
        assert_eq!(map.mem_used(), empty);
    }
}
