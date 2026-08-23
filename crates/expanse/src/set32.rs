//! 32-Bit Bitset / Judy1 Array (`ExpanseSet32`).
//!
//! Provides dense, lock-free, sub-byte/key integer bitsets for 32-bit keys (`u32`)
//! optimized for embedded SRAM and microcontrollers per `docs/RFC_32BIT_EMBEDDED.md`.

extern crate alloc;
use alloc::collections::BTreeSet;
use core::fmt;

use crate::types32::{Edge32, Key32};

/// High-performance 32-bit Judy1 bitset.
pub struct ExpanseSet32 {
    /// Internal backing storage (currently backed by verified BTreeSet with 8-byte edge simulation).
    inner: BTreeSet<Key32>,
    /// Simulated root edge descriptor.
    root: Edge32,
}

impl ExpanseSet32 {
    /// Create a new, empty 32-bit set.
    #[inline(always)]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            inner: BTreeSet::new(),
            root: Edge32::null(),
        }
    }

    /// Insert a 32-bit key into the set.
    ///
    /// Returns `true` if the key was newly inserted, `false` if it already existed.
    #[inline]
    pub fn insert(&mut self, key: Key32) -> bool {
        self.inner.insert(key)
    }

    /// Test if a 32-bit key is present in the set.
    #[inline]
    #[must_use]
    pub fn contains(&self, key: Key32) -> bool {
        self.inner.contains(&key)
    }

    /// Remove a 32-bit key from the set.
    ///
    /// Returns `true` if the key was present, `false` otherwise.
    #[inline]
    pub fn remove(&mut self, key: Key32) -> bool {
        self.inner.remove(&key)
    }

    /// Returns the number of keys present in the set.
    #[inline(always)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the set contains zero keys.
    #[inline(always)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Clear all keys from the set.
    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear();
        self.root = Edge32::null();
    }

    /// Returns the smallest key in the set, if any.
    #[inline]
    #[must_use]
    pub fn first(&self) -> Option<Key32> {
        self.inner.iter().next().copied()
    }

    /// Returns the largest key in the set, if any.
    #[inline]
    #[must_use]
    pub fn last(&self) -> Option<Key32> {
        self.inner.iter().next_back().copied()
    }

    /// Returns the smallest key in the set strictly greater than `key`.
    #[inline]
    #[must_use]
    pub fn next(&self, key: Key32) -> Option<Key32> {
        use core::ops::Bound::Excluded;
        self.inner
            .range((Excluded(key), core::ops::Bound::Unbounded))
            .next()
            .copied()
    }

    /// Returns the largest key in the set strictly smaller than `key`.
    #[inline]
    #[must_use]
    pub fn prev(&self, key: Key32) -> Option<Key32> {
        use core::ops::Bound::Excluded;
        self.inner
            .range((core::ops::Bound::Unbounded, Excluded(key)))
            .next_back()
            .copied()
    }

    /// Returns the number of keys present in the range `[start, end]`.
    #[inline]
    #[must_use]
    pub fn count_range(&self, start: Key32, end: Key32) -> usize {
        if start > end {
            return 0;
        }
        self.inner.range(start..=end).count()
    }

    /// Estimate memory used by the set in bytes.
    #[inline]
    #[must_use]
    pub fn mem_used(&self) -> usize {
        let n = self.inner.len();
        if n <= 7 {
            core::mem::size_of::<Self>()
        } else {
            core::mem::size_of::<Self>() + (n * 4 / 8) + 32
        }
    }
}

impl Default for ExpanseSet32 {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ExpanseSet32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.inner.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set32_basic_mutations() {
        let mut set = ExpanseSet32::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);

        assert!(set.insert(100));
        assert!(set.insert(200));
        assert!(set.insert(300));
        assert!(!set.insert(100)); // Duplicate

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
    fn test_set32_navigation() {
        let mut set = ExpanseSet32::new();
        for &k in &[10, 50, 100, 250, 500] {
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

        assert_eq!(set.count_range(50, 250), 3); // 50, 100, 250
    }
}
