//! 32-Bit Integer Set / Judy1 Array (`ExpanseSet32`).
//!
//! A real 256-ary digital trie over 32-bit keys (`u32`), decoding one byte
//! per level across four levels (`L4 -> L1`), specialised for embedded SRAM
//! and microcontrollers per `docs/design/32-bit-embedded.md`. Small subexpanses
//! are packed *immediately* inside the 8-byte edge (zero heap); denser ones
//! use packed linear leaves, a 256-bit bitmap leaf, and linear/uncompressed
//! branches (see [`crate::trie32`]).
//!
//! This type is single-threaded: it holds no synchronisation and is not a
//! lock-free structure. The concurrent wrapper described in the RFC is not
//! yet implemented for the 32-bit engine.

use core::default::Default;
use core::fmt;
use core::option::Option;

use crate::trie32::{self, Arena};
use crate::types32::{Edge32, Key32};

/// An ordered set of 32-bit integer keys backed by a digital trie.
pub struct ExpanseSet32 {
    /// Node arena backing the trie (byte-exact memory accounting).
    alloc: Arena,
    /// Root edge of the trie (level 4). May itself hold immediate keys.
    root: Edge32,
    /// Number of keys currently present.
    len: usize,
}

impl ExpanseSet32 {
    /// Create a new, empty 32-bit set.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            alloc: Arena::new(),
            root: Edge32::null(),
            len: 0,
        }
    }

    /// Insert a 32-bit key into the set.
    ///
    /// Returns `true` if the key was newly inserted, `false` if it already
    /// existed.
    #[inline]
    pub fn insert(&mut self, key: Key32) -> bool {
        let inserted = trie32::set_insert(&mut self.alloc, &mut self.root, 4, key);
        if inserted {
            self.len += 1;
        }
        inserted
    }

    /// Test if a 32-bit key is present in the set.
    #[inline]
    #[must_use]
    pub fn contains(&self, key: Key32) -> bool {
        trie32::set_contains(&self.alloc, &self.root, 4, key)
    }

    /// Query membership for a batch of 32-bit keys, writing boolean presence flags into `out`.
    #[inline]
    pub fn contains_batch(&self, keys: &[Key32], out: &mut [bool]) -> usize {
        assert_eq!(
            keys.len(),
            out.len(),
            "keys and out slices must have equal length"
        );
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

    /// Remove a 32-bit key from the set.
    ///
    /// Returns `true` if the key was present, `false` otherwise.
    #[inline]
    pub fn remove(&mut self, key: Key32) -> bool {
        let removed = trie32::set_remove(&mut self.alloc, &mut self.root, 4, key);
        if removed {
            self.len -= 1;
        }
        removed
    }

    /// Returns the number of keys present in the set.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if the set contains zero keys.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Clear all keys from the set, releasing all node allocations.
    #[inline]
    pub fn clear(&mut self) {
        self.alloc = Arena::new();
        self.root = Edge32::null();
        self.len = 0;
    }

    /// Returns the smallest key in the set, if any.
    #[inline]
    #[must_use]
    pub fn first(&self) -> Option<Key32> {
        trie32::first(&self.alloc, &self.root, 4)
    }

    /// Returns the largest key in the set, if any.
    #[inline]
    #[must_use]
    pub fn last(&self) -> Option<Key32> {
        trie32::last(&self.alloc, &self.root, 4)
    }

    /// Returns the smallest key in the set strictly greater than `key`.
    #[inline]
    #[must_use]
    pub fn next(&self, key: Key32) -> Option<Key32> {
        trie32::next(&self.alloc, &self.root, 4, key)
    }

    /// Returns the largest key in the set strictly smaller than `key`.
    #[inline]
    #[must_use]
    pub fn prev(&self, key: Key32) -> Option<Key32> {
        trie32::prev(&self.alloc, &self.root, 4, key)
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

    /// Real bytes of node/leaf storage held by this set. Zero when empty;
    /// immediate (in-edge) keys cost nothing. Mirrors the 64-bit
    /// `ExpanseSet::mem_used` accounting.
    #[inline]
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.alloc.bytes_in_use()
    }

    /// Smallest key `>= bound`, if any.
    #[inline]
    fn first_ge(&self, bound: Key32) -> Option<Key32> {
        if bound == 0 {
            self.first()
        } else {
            self.next(bound - 1)
        }
    }

    /// Largest key `<= bound`, if any.
    #[inline]
    fn last_le(&self, bound: Key32) -> Option<Key32> {
        if bound == Key32::MAX {
            self.last()
        } else {
            self.prev(bound + 1)
        }
    }

    /// Double-ended iterator over all keys in ascending order.
    ///
    /// Cursor-based (`first`/`next` forward, `last`/`prev` backward); the two
    /// ends share `[lo, hi]` bounds so interleaved calls never cross.
    #[inline]
    #[must_use]
    pub fn iter(&self) -> SetIter32<'_> {
        SetIter32 {
            set: self,
            lo: 0,
            hi: Key32::MAX,
            done: self.is_empty(),
        }
    }

    /// Double-ended iterator over keys in the inclusive range `[start, end]`.
    #[inline]
    #[must_use]
    pub fn range(&self, range: core::ops::RangeInclusive<Key32>) -> SetIter32<'_> {
        let (start, end) = (*range.start(), *range.end());
        SetIter32 {
            set: self,
            lo: start,
            hi: end,
            done: start > end || self.is_empty(),
        }
    }

    /// Descending iterator over all keys.
    #[inline]
    #[must_use]
    pub fn iter_rev(&self) -> SetRangeRev32<'_> {
        SetRangeRev32 {
            set: self,
            lo: 0,
            hi: Key32::MAX,
            done: self.is_empty(),
        }
    }

    /// Descending iterator over keys in the inclusive range `[start, end]`.
    #[inline]
    #[must_use]
    pub fn range_rev(&self, range: core::ops::RangeInclusive<Key32>) -> SetRangeRev32<'_> {
        let (start, end) = (*range.start(), *range.end());
        SetRangeRev32 {
            set: self,
            lo: start,
            hi: end,
            done: start > end || self.is_empty(),
        }
    }
}

/// Double-ended key iterator over an [`ExpanseSet32`] (ascending by default).
pub struct SetIter32<'a> {
    set: &'a ExpanseSet32,
    lo: Key32,
    hi: Key32,
    done: bool,
}

impl Iterator for SetIter32<'_> {
    type Item = Key32;

    #[inline]
    fn next(&mut self) -> Option<Key32> {
        if self.done {
            return None;
        }
        let k = self.set.first_ge(self.lo)?;
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

impl DoubleEndedIterator for SetIter32<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Key32> {
        if self.done {
            return None;
        }
        let k = self.set.last_le(self.hi)?;
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

/// Descending key iterator over an [`ExpanseSet32`] range.
pub struct SetRangeRev32<'a> {
    set: &'a ExpanseSet32,
    lo: Key32,
    hi: Key32,
    done: bool,
}

impl Iterator for SetRangeRev32<'_> {
    type Item = Key32;

    #[inline]
    fn next(&mut self) -> Option<Key32> {
        if self.done {
            return None;
        }
        let k = self.set.last_le(self.hi)?;
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

impl DoubleEndedIterator for SetRangeRev32<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Key32> {
        if self.done {
            return None;
        }
        let k = self.set.first_ge(self.lo)?;
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

impl<'a> IntoIterator for &'a ExpanseSet32 {
    type Item = Key32;
    type IntoIter = SetIter32<'a>;

    #[inline]
    fn into_iter(self) -> SetIter32<'a> {
        self.iter()
    }
}

impl Default for ExpanseSet32 {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ExpanseSet32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_set();
        let mut cur = self.first();
        while let Some(k) = cur {
            dbg.entry(&k);
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
    use alloc_crate::collections::BTreeSet;
    #[cfg(not(feature = "std"))]
    use alloc_crate::vec::Vec;
    #[cfg(feature = "std")]
    use std::collections::BTreeSet;

    #[test]
    fn basic_mutations() {
        let mut set = ExpanseSet32::new();
        assert!(set.is_empty());
        assert_eq!(set.mem_used(), 0);

        assert!(set.insert(100));
        assert!(set.insert(200));
        assert!(set.insert(300));
        assert!(!set.insert(100));

        assert_eq!(set.len(), 3);
        assert!(set.contains(100));
        assert!(set.contains(200));
        assert!(set.contains(300));
        assert!(!set.contains(400));

        assert!(set.remove(200));
        assert!(!set.contains(200));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn navigation() {
        let mut set = ExpanseSet32::new();
        for &k in &[10u32, 50, 100, 250, 500] {
            set.insert(k);
        }
        assert_eq!(set.first(), Some(10));
        assert_eq!(set.last(), Some(500));
        assert_eq!(set.next(10), Some(50));
        assert_eq!(set.next(50), Some(100));
        assert_eq!(set.next(500), None);
        assert_eq!(set.prev(500), Some(250));
        assert_eq!(set.prev(100), Some(50));
        assert_eq!(set.prev(10), None);
        assert_eq!(set.count_range(50, 250), 3);
    }

    #[test]
    fn boundaries() {
        let mut set = ExpanseSet32::new();
        for &k in &[0u32, 1, u32::MAX, u32::MAX - 1, 0x00FF_FF00, 0x0100_0000] {
            assert!(set.insert(k));
        }
        assert_eq!(set.first(), Some(0));
        assert_eq!(set.last(), Some(u32::MAX));
        assert_eq!(set.next(0), Some(1));
        assert_eq!(set.prev(u32::MAX), Some(u32::MAX - 1));
        assert!(set.contains(u32::MAX));
        assert!(set.remove(u32::MAX));
        assert_eq!(set.last(), Some(u32::MAX - 1));
    }

    // XorShift RNG (mirrors the 64-bit map tests) — deterministic, no deps.
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
        let mut set = ExpanseSet32::new();
        let mut model: BTreeSet<u32> = BTreeSet::new();

        for _ in 0..ops {
            let k = rng.next() & key_mask;
            match rng.next() % 5 {
                0 | 1 => assert_eq!(set.insert(k), model.insert(k), "insert {k:#x}"),
                2 => assert_eq!(set.remove(k), model.remove(&k), "remove {k:#x}"),
                _ => assert_eq!(set.contains(k), model.contains(&k), "contains {k:#x}"),
            }
            assert_eq!(set.len(), model.len());
        }

        // Ordering + navigation agreement.
        let expected: Vec<u32> = model.iter().copied().collect();
        let mut got = Vec::new();
        let mut cur = set.first();
        while let Some(k) = cur {
            got.push(k);
            cur = set.next(k);
        }
        assert_eq!(got, expected, "forward ordering seed={seed}");

        assert_eq!(set.first(), expected.first().copied());
        assert_eq!(set.last(), expected.last().copied());
        for &k in &expected {
            assert_eq!(
                set.next(k),
                expected.iter().copied().find(|&x| x > k),
                "next({k:#x})"
            );
            assert_eq!(
                set.prev(k),
                expected.iter().rev().copied().find(|&x| x < k),
                "prev({k:#x})"
            );
        }
        // A handful of range counts.
        for &(lo, hi) in &[
            (0u32, key_mask),
            (0, key_mask / 2),
            (key_mask / 4, key_mask / 2),
        ] {
            let exp = expected.iter().filter(|&&x| x >= lo && x <= hi).count();
            assert_eq!(set.count_range(lo, hi), exp, "count_range({lo:#x},{hi:#x})");
        }

        // Iterator agreement (forward, reverse, ranges) vs BTreeSet.
        assert_eq!(set.iter().collect::<Vec<_>>(), expected, "iter seed={seed}");
        let rev: Vec<u32> = expected.iter().rev().copied().collect();
        assert_eq!(
            set.iter_rev().collect::<Vec<_>>(),
            rev,
            "iter_rev seed={seed}"
        );
        assert_eq!(
            set.iter().rev().collect::<Vec<_>>(),
            rev,
            "iter().rev() seed={seed}"
        );
        for &(lo, hi) in &[
            (0u32, key_mask),
            (0, key_mask / 2),
            (key_mask / 4, key_mask / 2),
            (key_mask / 2, key_mask / 2),
        ] {
            let fwd: Vec<u32> = expected
                .iter()
                .copied()
                .filter(|&x| x >= lo && x <= hi)
                .collect();
            let bwd: Vec<u32> = fwd.iter().rev().copied().collect();
            assert_eq!(
                set.range(lo..=hi).collect::<Vec<_>>(),
                fwd,
                "range({lo:#x},{hi:#x})"
            );
            assert_eq!(
                set.range_rev(lo..=hi).collect::<Vec<_>>(),
                bwd,
                "range_rev({lo:#x},{hi:#x})"
            );
            assert_eq!(
                set.range(lo..=hi).rev().collect::<Vec<_>>(),
                bwd,
                "range({lo:#x},{hi:#x}).rev()"
            );
        }
        // Interleaved next()/next_back(): the two ends must never cross.
        {
            let mut it = set.iter();
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

        // Drain: every key removable once, no bytes left behind.
        for &k in &expected {
            assert!(set.remove(k), "drain {k:#x}");
        }
        assert!(set.is_empty());
        assert_eq!(set.mem_used(), 0, "leak after drain seed={seed}");
    }

    #[cfg(not(miri))]
    const OPS: usize = 20_000;
    #[cfg(miri)]
    const OPS: usize = 400;

    #[test]
    fn differential_dense() {
        for seed in [1u64, 2, 3] {
            differential(seed, OPS, 0x0000_03FF); // clustered low bytes
        }
    }

    #[test]
    fn differential_sparse() {
        for seed in [11u64, 22, 33] {
            differential(seed, OPS, 0xFFFF_FFFF); // full 32-bit spread
        }
    }

    #[test]
    fn differential_midrange() {
        for seed in [101u64, 202] {
            differential(seed, OPS, 0x000F_FFFF);
        }
    }
}
