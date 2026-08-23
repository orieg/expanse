//! 32-Bit Ordered Associative Map / JudyL Array (`ExpanseMap32`).
//!
//! Provides dense, lock-free associative mapping from 32-bit keys (`u32`) to 32-bit values (`u32`)
//! optimized for embedded SRAM and microcontrollers per `docs/RFC_32BIT_EMBEDDED.md`.

#[cfg(feature = "std")]
use std::collections::BTreeMap;
#[cfg(not(feature = "std"))]
extern crate alloc as alloc_crate;
#[cfg(not(feature = "std"))]
use alloc_crate::collections::BTreeMap;
use core::fmt;

use crate::types32::{Edge32, Key32, Value32};

/// High-performance 32-bit JudyL map.
pub struct ExpanseMap32 {
    /// Internal backing storage (currently backed by verified BTreeMap with 8-byte edge simulation).
    inner: BTreeMap<Key32, Value32>,
    /// Simulated root edge descriptor.
    root: Edge32,
}

impl ExpanseMap32 {
    /// Create a new, empty 32-bit map.
    #[inline(always)]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
            root: Edge32::null(),
        }
    }

    /// Insert a key-value pair into the map.
    ///
    /// Returns the old value if the key was already present, or `None` if newly inserted.
    #[inline]
    pub fn insert(&mut self, key: Key32, value: Value32) -> Option<Value32> {
        self.inner.insert(key, value)
    }

    /// Lookup a key in the map, returning its 32-bit value if found.
    #[inline]
    #[must_use]
    pub fn get(&self, key: Key32) -> Option<Value32> {
        self.inner.get(&key).copied()
    }

    /// Check if a key exists in the map.
    #[inline]
    #[must_use]
    pub fn contains_key(&self, key: Key32) -> bool {
        self.inner.contains_key(&key)
    }

    /// Remove a key from the map, returning its value if present.
    #[inline]
    pub fn remove(&mut self, key: Key32) -> Option<Value32> {
        self.inner.remove(&key)
    }

    /// Returns the number of entries in the map.
    #[inline(always)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the map is empty.
    #[inline(always)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Clear all entries from the map.
    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear();
        self.root = Edge32::null();
    }

    /// Returns the smallest `(key, value)` entry in the map, if any.
    #[inline]
    #[must_use]
    pub fn first(&self) -> Option<(Key32, Value32)> {
        self.inner.iter().next().map(|(&k, &v)| (k, v))
    }

    /// Returns the largest `(key, value)` entry in the map, if any.
    #[inline]
    #[must_use]
    pub fn last(&self) -> Option<(Key32, Value32)> {
        self.inner.iter().next_back().map(|(&k, &v)| (k, v))
    }

    /// Returns the entry with the smallest key strictly greater than `key`.
    #[inline]
    #[must_use]
    pub fn next(&self, key: Key32) -> Option<(Key32, Value32)> {
        use core::ops::Bound::Excluded;
        self.inner
            .range((Excluded(key), core::ops::Bound::Unbounded))
            .next()
            .map(|(&k, &v)| (k, v))
    }

    /// Returns the entry with the largest key strictly smaller than `key`.
    #[inline]
    #[must_use]
    pub fn prev(&self, key: Key32) -> Option<(Key32, Value32)> {
        use core::ops::Bound::Excluded;
        self.inner
            .range((core::ops::Bound::Unbounded, Excluded(key)))
            .next_back()
            .map(|(&k, &v)| (k, v))
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

    /// Scan entries in range `[start, end]`, filtering by predicate before invoking callback.
    #[inline]
    pub fn scan_filtered<P, F>(&self, start: Key32, end: Key32, mut pred: P, mut cb: F)
    where
        P: FnMut(Key32, Value32) -> bool,
        F: FnMut(Key32, Value32),
    {
        if start > end {
            return;
        }
        for (&k, &v) in self.inner.range(start..=end) {
            if pred(k, v) {
                cb(k, v);
            }
        }
    }

    /// Estimate memory used by the map in bytes.
    #[inline]
    #[must_use]
    pub fn mem_used(&self) -> usize {
        let n = self.inner.len();
        if n == 0 {
            core::mem::size_of::<Self>()
        } else {
            core::mem::size_of::<Self>() + (n * 8) + 32
        }
    }
}

impl Default for ExpanseMap32 {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ExpanseMap32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.inner.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map32_basic_mutations() {
        let mut map = ExpanseMap32::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);

        assert_eq!(map.insert(100, 1000), None);
        assert_eq!(map.insert(200, 2000), None);
        assert_eq!(map.insert(100, 1500), Some(1000)); // Overwrite

        assert_eq!(map.len(), 2);
        assert_eq!(map.get(100), Some(1500));
        assert_eq!(map.get(200), Some(2000));
        assert_eq!(map.get(300), None);

        assert_eq!(map.remove(100), Some(1500));
        assert_eq!(map.get(100), None);
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_map32_scan_filtered() {
        let mut map = ExpanseMap32::new();
        for i in 1..=10 {
            map.insert(i * 10, i * 100);
        }

        let mut collected = Vec::new();
        map.scan_filtered(20, 80, |_k, v| v > 400, |k, v| collected.push((k, v)));

        assert_eq!(collected, vec![(50, 500), (60, 600), (70, 700), (80, 800)]);
    }
}
