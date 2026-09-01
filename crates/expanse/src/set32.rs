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
//! optimistic-read structure. The concurrent wrapper described in the RFC is not
//! yet implemented for the 32-bit engine.

use core::default::Default;
use core::fmt;
use core::option::Option;

use core_alloc::vec::Vec;

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

    /// Bulk-build a 32-bit set from an ascending iterator of keys (issue #348).
    ///
    /// A convenience bulk-load entry mirroring [`crate::set::ExpanseSet::from_sorted_iter`].
    /// Sorted input is loaded in ascending order (so the insert cursor stays on
    /// the hot path); any out-of-order input is sorted and deduplicated first,
    /// so the result is always correct. Duplicate keys are collapsed.
    ///
    /// Note: the 64-bit `ExpanseSet` emits the trie bottom-up in one pass
    /// (direct emission); the 32-bit twin builds via the shared trie32 insert
    /// engine — the 32-bit trie has no structural set-algebra kernel to emit
    /// from, so a dedicated direct-emission builder is deferred with the 32-bit
    /// materialization work.
    #[must_use]
    pub fn from_sorted_iter<I: IntoIterator<Item = Key32>>(iter: I) -> Self {
        let mut keys: Vec<Key32> = iter.into_iter().collect();
        if !keys.is_empty() && !keys.windows(2).all(|w| w[0] < w[1]) {
            keys.sort_unstable();
            keys.dedup();
        }
        let mut set = Self::new();
        for k in keys {
            set.insert(k);
        }
        set
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

/// Native set algebra (issue #339), the 32-bit twin of the `ExpanseSet`
/// kernels. The 32-bit engine is a distinct arena/handle trie (`trie32`) that
/// does not share the 64-bit `Edge` geometry the structural word-parallel
/// kernel walks, so these compose the result from an ordered merge of the two
/// tries via `first`/`next` — correct and `O(|A| + |B|)` in ordered steps.
/// The cardinality variants materialize nothing; the union/difference/xor
/// counts derive from `intersection_len` and the two populations.
impl ExpanseSet32 {
    /// Number of keys present in **both** sets (`|A ∩ B|`), via a lockstep
    /// merge of the two ordered key streams. No result is materialized.
    #[must_use]
    pub fn intersection_len(&self, other: &ExpanseSet32) -> usize {
        let (mut a, mut b) = (self.first(), other.first());
        let mut count = 0usize;
        while let (Some(x), Some(y)) = (a, b) {
            if x == y {
                count += 1;
                a = self.next(x);
                b = other.next(y);
            } else if x < y {
                a = self.next(x);
            } else {
                b = other.next(y);
            }
        }
        count
    }

    /// Number of keys in the union (`|A ∪ B| = |A| + |B| − |A ∩ B|`).
    #[must_use]
    pub fn union_len(&self, other: &ExpanseSet32) -> usize {
        self.len() + other.len() - self.intersection_len(other)
    }

    /// Number of keys in the difference (`|A \ B| = |A| − |A ∩ B|`).
    #[must_use]
    pub fn difference_len(&self, other: &ExpanseSet32) -> usize {
        self.len() - self.intersection_len(other)
    }

    /// Number of keys in the symmetric difference
    /// (`|A △ B| = |A| + |B| − 2·|A ∩ B|`).
    #[must_use]
    pub fn symmetric_difference_len(&self, other: &ExpanseSet32) -> usize {
        self.len() + other.len() - 2 * self.intersection_len(other)
    }

    /// The set of keys present in **both** sets (`A ∩ B`).
    #[must_use]
    pub fn intersection(&self, other: &ExpanseSet32) -> ExpanseSet32 {
        let mut out = ExpanseSet32::new();
        let (mut a, mut b) = (self.first(), other.first());
        while let (Some(x), Some(y)) = (a, b) {
            if x == y {
                out.insert(x);
                a = self.next(x);
                b = other.next(y);
            } else if x < y {
                a = self.next(x);
            } else {
                b = other.next(y);
            }
        }
        out
    }

    /// The set of keys present in **either** set (`A ∪ B`).
    #[must_use]
    pub fn union(&self, other: &ExpanseSet32) -> ExpanseSet32 {
        let mut out = ExpanseSet32::new();
        let (mut a, mut b) = (self.first(), other.first());
        loop {
            match (a, b) {
                (Some(x), Some(y)) => {
                    if x < y {
                        out.insert(x);
                        a = self.next(x);
                    } else if x > y {
                        out.insert(y);
                        b = other.next(y);
                    } else {
                        out.insert(x);
                        a = self.next(x);
                        b = other.next(y);
                    }
                }
                (Some(x), None) => {
                    out.insert(x);
                    a = self.next(x);
                }
                (None, Some(y)) => {
                    out.insert(y);
                    b = other.next(y);
                }
                (None, None) => break,
            }
        }
        out
    }

    /// The set of keys in `self` but not `other` (`A \ B`).
    #[must_use]
    pub fn difference(&self, other: &ExpanseSet32) -> ExpanseSet32 {
        let mut out = ExpanseSet32::new();
        let (mut a, mut b) = (self.first(), other.first());
        loop {
            match (a, b) {
                (Some(x), Some(y)) => {
                    if x < y {
                        out.insert(x);
                        a = self.next(x);
                    } else if x > y {
                        b = other.next(y);
                    } else {
                        a = self.next(x);
                        b = other.next(y);
                    }
                }
                (Some(x), None) => {
                    out.insert(x);
                    a = self.next(x);
                }
                (None, _) => break,
            }
        }
        out
    }

    /// The set of keys in exactly one of the two sets (`A △ B`).
    #[must_use]
    pub fn symmetric_difference(&self, other: &ExpanseSet32) -> ExpanseSet32 {
        let mut out = ExpanseSet32::new();
        let (mut a, mut b) = (self.first(), other.first());
        loop {
            match (a, b) {
                (Some(x), Some(y)) => {
                    if x < y {
                        out.insert(x);
                        a = self.next(x);
                    } else if x > y {
                        out.insert(y);
                        b = other.next(y);
                    } else {
                        a = self.next(x);
                        b = other.next(y);
                    }
                }
                (Some(x), None) => {
                    out.insert(x);
                    a = self.next(x);
                }
                (None, Some(y)) => {
                    out.insert(y);
                    b = other.next(y);
                }
                (None, None) => break,
            }
        }
        out
    }
}

impl core::ops::BitAnd for &ExpanseSet32 {
    type Output = ExpanseSet32;
    /// `A & B` — the intersection ([`ExpanseSet32::intersection`]).
    fn bitand(self, rhs: &ExpanseSet32) -> ExpanseSet32 {
        self.intersection(rhs)
    }
}

impl core::ops::BitOr for &ExpanseSet32 {
    type Output = ExpanseSet32;
    /// `A | B` — the union ([`ExpanseSet32::union`]).
    fn bitor(self, rhs: &ExpanseSet32) -> ExpanseSet32 {
        self.union(rhs)
    }
}

impl core::ops::Sub for &ExpanseSet32 {
    type Output = ExpanseSet32;
    /// `A - B` — the difference ([`ExpanseSet32::difference`]).
    fn sub(self, rhs: &ExpanseSet32) -> ExpanseSet32 {
        self.difference(rhs)
    }
}

impl core::ops::BitXor for &ExpanseSet32 {
    type Output = ExpanseSet32;
    /// `A ^ B` — the symmetric difference
    /// ([`ExpanseSet32::symmetric_difference`]).
    fn bitxor(self, rhs: &ExpanseSet32) -> ExpanseSet32 {
        self.symmetric_difference(rhs)
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
    fn from_sorted_iter_matches_insert() {
        let mut state = 0x1234_5678u32;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        for &n in &[0usize, 1, 5, 40, 300, 5000] {
            let mut model = BTreeSet::new();
            while model.len() < n {
                model.insert(next() & 0x0003_FFFF);
            }
            let keys: Vec<u32> = model.iter().copied().collect();

            let built = ExpanseSet32::from_sorted_iter(keys.iter().copied());
            assert_eq!(built.len(), n);
            let got: Vec<u32> = built.iter().collect();
            assert_eq!(got, keys, "sorted build contents n={n}");

            // Unsorted / duplicate input is corrected.
            let mut raw = keys.clone();
            raw.extend(keys.iter().rev().copied());
            let from_raw = ExpanseSet32::from_sorted_iter(raw);
            assert_eq!(from_raw.iter().collect::<Vec<_>>(), keys, "unsorted n={n}");
        }
    }

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

    // ---- Set algebra (issue #339) ----

    fn algebra_run(seed: u64, key_mask: u32) {
        let mut rng = XorShift::new(seed);
        let n = if cfg!(miri) { 40 } else { 2000 };
        let mut a = ExpanseSet32::new();
        let mut ma: BTreeSet<u32> = BTreeSet::new();
        let mut b = ExpanseSet32::new();
        let mut mb: BTreeSet<u32> = BTreeSet::new();
        for _ in 0..n {
            let ka = rng.next() & key_mask;
            a.insert(ka);
            ma.insert(ka);
            let kb = rng.next() & key_mask;
            b.insert(kb);
            mb.insert(kb);
        }

        let inter = ma.intersection(&mb).count();
        assert_eq!(a.intersection_len(&b), inter, "intersection_len");
        assert_eq!(b.intersection_len(&a), inter, "intersection_len rev");
        assert_eq!(a.union_len(&b), ma.union(&mb).count(), "union_len");
        assert_eq!(
            a.difference_len(&b),
            ma.difference(&mb).count(),
            "difference_len"
        );
        assert_eq!(
            a.symmetric_difference_len(&b),
            ma.symmetric_difference(&mb).count(),
            "symmetric_difference_len"
        );

        let collect = |s: &ExpanseSet32| -> Vec<u32> {
            let mut out = Vec::new();
            let mut cur = s.first();
            while let Some(k) = cur {
                out.push(k);
                cur = s.next(k);
            }
            out
        };
        assert_eq!(
            collect(&(&a & &b)),
            ma.intersection(&mb).copied().collect::<Vec<_>>(),
            "intersection"
        );
        assert_eq!(
            collect(&(&a | &b)),
            ma.union(&mb).copied().collect::<Vec<_>>(),
            "union"
        );
        assert_eq!(
            collect(&(&a - &b)),
            ma.difference(&mb).copied().collect::<Vec<_>>(),
            "difference"
        );
        assert_eq!(
            collect(&(&a ^ &b)),
            ma.symmetric_difference(&mb).copied().collect::<Vec<_>>(),
            "symmetric_difference"
        );
    }

    /// A workload toggling one digit across the `BranchL6_32` -> `BranchB32`
    /// boundary must not rebuild the node on every operation.
    ///
    /// With `BranchB32` added, the 32-bit branch ladder is:
    /// `BranchL2_32` (32 B) -> `BranchL6_32` (64 B) -> `BranchB32` (96 B) -> `BranchU32` (2080 B).
    ///
    /// `BranchL6_32` promotes to `BranchB32` on the 7th child (capacity 6).
    /// `BranchB32` uses Band 1 hysteresis: demotes to `BranchL6_32` at <= 5 children.
    #[test]
    fn branch_l6_to_b_boundary_toggle_keeps_node_form() {
        let key = |b2: u32, low: u32| 0x0100_0000 | (b2 << 16) | low;
        let mut s = ExpanseSet32::new();
        // Six distinct branch digits, 5 keys each: a populated BranchL6_32.
        for b2 in 1..=6u32 {
            for low in 0..5u32 {
                s.insert(key(b2, low));
            }
        }
        let at_six = s.mem_used();

        // The 7th digit exceeds L6's capacity and promotes to BranchB32.
        s.insert(key(7, 0));
        let at_seven = s.mem_used();
        assert!(
            at_seven > at_six,
            "expected promotion to BranchB32; {at_six} -> {at_seven}"
        );

        // Removing it leaves 6 digits. With Band 1 hysteresis, demotion only happens
        // at <= 5 digits, so it stays as BranchB32 (96 B header vs 64 B L6 header).
        assert!(s.remove(key(7, 0)));
        let after = s.mem_used();
        assert!(
            after > at_six,
            "node should keep BranchB32 form at 6 digits (demotes at <= 5); at_six={at_six}, after={after}"
        );

        // Removing down to 5 digits triggers demotion back to BranchL6_32.
        for low in 0..5u32 {
            s.remove(key(6, low));
        }
        let at_five = s.mem_used();
        assert!(
            at_five < after,
            "expected demotion to BranchL6_32 at 5 digits; {after} -> {at_five}"
        );
    }

    /// A workload toggling digits across the `BranchB32` -> `BranchU32`
    /// boundary must not rebuild the 2080-byte node on every operation.
    ///
    /// `BranchB32` promotes to `BranchU32` when child count > 192.
    /// `BranchU32` uses Band 2 hysteresis: demotes to `BranchB32` at <= 190 children.
    #[test]
    fn branch_b_to_u_boundary_toggle_keeps_node_form() {
        let key = |b2: u32, low: u32| 0x0100_0000 | (b2 << 16) | low;
        let mut s = ExpanseSet32::new();
        // 192 distinct branch digits, 5 keys each: a populated BranchB32.
        for b2 in 1..=192u32 {
            for low in 0..5u32 {
                s.insert(key(b2, low));
            }
        }
        let at_192 = s.mem_used();

        // 193rd digit promotes to BranchU32 (2080 B vs 96 B + 192*8 B = 1632 B).
        s.insert(key(193, 0));
        let at_193 = s.mem_used();
        assert!(
            at_193 > at_192 + 400,
            "expected promotion to BranchU32 (2080 B); {at_192} -> {at_193}"
        );

        // Removing 193rd digit leaves 192 digits. With Band 2 hysteresis,
        // it stays as BranchU32 (demotes at <= 190).
        assert!(s.remove(key(193, 0)));
        let after_193_removed = s.mem_used();
        assert_eq!(
            after_193_removed, at_193,
            "node should keep BranchU32 form (2080 B fixed) at 192 digits"
        );

        // Removing 192nd digit leaves 191 digits: still BranchU32.
        for low in 0..5u32 {
            s.remove(key(192, low));
        }
        let at_191 = s.mem_used();
        assert!(
            at_191 > at_192,
            "node should keep BranchU32 form at 191 digits"
        );

        // Removing 191st digit leaves 190 digits: demotes to BranchB32!
        for low in 0..5u32 {
            s.remove(key(191, low));
        }
        let at_190 = s.mem_used();
        assert!(
            at_190 < at_191 - 400,
            "node should demote to BranchB32 at <= 190 digits; {at_191} -> {at_190}"
        );
    }
    #[test]
    fn algebra_matches_btreeset() {
        for seed in [7u64, 42, 1234] {
            algebra_run(seed, 0x0000_03FF); // dense/clustered overlap
            algebra_run(seed, 0x000F_FFFF); // midrange
            algebra_run(seed, 0xFFFF_FFFF); // sparse full spread
        }
        // Edge cases: empty, disjoint, identical.
        let empty = ExpanseSet32::new();
        let mut one = ExpanseSet32::new();
        one.insert(5);
        assert_eq!(empty.intersection_len(&one), 0);
        assert_eq!(one.union_len(&empty), 1);
        assert_eq!(one.difference_len(&empty), 1);
        assert_eq!((&one ^ &empty).len(), 1);
        assert_eq!((&one & &one).len(), 1);
    }
}
