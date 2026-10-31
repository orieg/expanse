//! `BTreeSet`/`BTreeMap` adapters with the `ExpanseSet`/`ExpanseMap` method surface.
//!
//! `WasmExpanseSet` and `WasmExpanseMap` take 64-bit keys. On a 64-bit target
//! (wasm64) they wrap the engine's `ExpanseSet`/`ExpanseMap`; on a 32-bit target
//! (wasm32) the 64-bit engine is not built and they fall back to the standard
//! library's ordered collections. Each wrapper names its backing type through
//! one `cfg`-selected alias, and these adapters give the fallback the same
//! method names and signatures as the engine type, so every wrapper method is
//! written once for both widths.
//!
//! The adapters compile on every target so the native test suite can check
//! them against the engine types; only 32-bit builds construct them.

#![cfg_attr(target_pointer_width = "64", allow(dead_code))]

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound::{Excluded, Unbounded};
use std::ops::RangeInclusive;

/// A `BTreeSet<u64>` behind the `ExpanseSet` method surface the wrapper uses.
#[derive(Default)]
pub(crate) struct BTreeSet64(BTreeSet<u64>);

impl BTreeSet64 {
    pub(crate) fn new() -> Self {
        Self(BTreeSet::new())
    }

    pub(crate) fn insert(&mut self, key: u64) -> bool {
        self.0.insert(key)
    }

    pub(crate) fn remove(&mut self, key: u64) -> bool {
        self.0.remove(&key)
    }

    pub(crate) fn contains(&self, key: u64) -> bool {
        self.0.contains(&key)
    }

    pub(crate) fn len(&self) -> u64 {
        self.0.len() as u64
    }

    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }

    pub(crate) fn first(&self) -> Option<u64> {
        self.0.iter().next().copied()
    }

    pub(crate) fn last(&self) -> Option<u64> {
        self.0.iter().next_back().copied()
    }

    pub(crate) fn next_after(&self, key: u64) -> Option<u64> {
        self.0.range((Excluded(key), Unbounded)).next().copied()
    }

    pub(crate) fn prev_before(&self, key: u64) -> Option<u64> {
        self.0
            .range((Unbounded, Excluded(key)))
            .next_back()
            .copied()
    }

    pub(crate) fn count_below(&self, key: u64) -> u64 {
        self.0.range(..key).count() as u64
    }

    pub(crate) fn by_count(&self, n: u64) -> Option<u64> {
        self.0.iter().nth(n as usize).copied()
    }

    pub(crate) fn count_range(&self, range: RangeInclusive<u64>) -> u64 {
        self.0.range(range).count() as u64
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = u64> + '_ {
        self.0.iter().copied()
    }
}

/// A `BTreeMap<u64, u64>` behind the `ExpanseMap` method surface the wrapper uses.
#[derive(Default)]
pub(crate) struct BTreeMap64(BTreeMap<u64, u64>);

impl BTreeMap64 {
    pub(crate) fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub(crate) fn insert(&mut self, key: u64, value: u64) -> Option<u64> {
        self.0.insert(key, value)
    }

    pub(crate) fn get(&self, key: u64) -> Option<u64> {
        self.0.get(&key).copied()
    }

    pub(crate) fn remove(&mut self, key: u64) -> Option<u64> {
        self.0.remove(&key)
    }

    pub(crate) fn contains_key(&self, key: u64) -> bool {
        self.0.contains_key(&key)
    }

    pub(crate) fn len(&self) -> u64 {
        self.0.len() as u64
    }

    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }

    pub(crate) fn first(&self) -> Option<(u64, u64)> {
        self.0.iter().next().map(|(&k, &v)| (k, v))
    }

    pub(crate) fn last(&self) -> Option<(u64, u64)> {
        self.0.iter().next_back().map(|(&k, &v)| (k, v))
    }

    pub(crate) fn next_after(&self, key: u64) -> Option<(u64, u64)> {
        self.0
            .range((Excluded(key), Unbounded))
            .next()
            .map(|(&k, &v)| (k, v))
    }

    pub(crate) fn prev_before(&self, key: u64) -> Option<(u64, u64)> {
        self.0
            .range((Unbounded, Excluded(key)))
            .next_back()
            .map(|(&k, &v)| (k, v))
    }
}

#[cfg(all(test, target_pointer_width = "64"))]
mod tests {
    //! The adapters must answer every query exactly as the engine types do,
    //! since wasm32 builds run on them in place of the engine.

    use super::{BTreeMap64, BTreeSet64};
    use expanse_trie::map::ExpanseMap;
    use expanse_trie::set::ExpanseSet;

    /// Keys spread across the 64-bit space, with neighbours and both extremes.
    fn keys() -> Vec<u64> {
        let mut keys: Vec<u64> = (0..2_000u64)
            .map(|i| i.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (i % 40))
            .collect();
        keys.extend([0, 1, 2, u64::MAX - 1, u64::MAX, 1 << 32, (1 << 32) + 1]);
        keys
    }

    /// Probes: every key, its neighbours, and values between them.
    fn probes(keys: &[u64]) -> Vec<u64> {
        let mut p = Vec::new();
        for &k in keys {
            p.extend([k, k.wrapping_add(1), k.wrapping_sub(1), k / 2]);
        }
        p
    }

    #[test]
    fn btree_set64_matches_expanse_set() {
        let keys = keys();
        let mut a = ExpanseSet::new();
        let mut b = BTreeSet64::new();
        for &k in &keys {
            assert_eq!(a.insert(k), b.insert(k), "insert {k}");
        }
        for &k in keys.iter().step_by(7) {
            assert_eq!(a.remove(k), b.remove(k), "remove {k}");
        }
        assert_eq!(a.len(), b.len());
        assert_eq!(a.first(), b.first());
        assert_eq!(a.last(), b.last());
        assert_eq!(a.iter().collect::<Vec<_>>(), b.iter().collect::<Vec<_>>());
        for &p in &probes(&keys) {
            assert_eq!(a.contains(p), b.contains(p), "contains {p}");
            assert_eq!(a.next_after(p), b.next_after(p), "next_after {p}");
            assert_eq!(a.prev_before(p), b.prev_before(p), "prev_before {p}");
            assert_eq!(a.count_below(p), b.count_below(p), "count_below {p}");
            assert_eq!(
                a.count_range(p / 3..=p),
                b.count_range(p / 3..=p),
                "count_range {p}"
            );
        }
        for n in [0, 1, 2, a.len() / 2, a.len() - 1, a.len(), a.len() + 5] {
            assert_eq!(a.by_count(n), b.by_count(n), "by_count {n}");
        }
        a.clear();
        b.clear();
        assert_eq!(a.len(), b.len());
        assert_eq!(a.first(), b.first());
    }

    #[test]
    fn btree_map64_matches_expanse_map() {
        let keys = keys();
        let mut a = ExpanseMap::new();
        let mut b = BTreeMap64::new();
        for (i, &k) in keys.iter().enumerate() {
            let v = (i as u64).wrapping_mul(31);
            assert_eq!(a.insert(k, v), b.insert(k, v), "insert {k}");
        }
        for &k in keys.iter().step_by(5) {
            assert_eq!(a.insert(k, k), b.insert(k, k), "replace {k}");
        }
        for &k in keys.iter().step_by(7) {
            assert_eq!(a.remove(k), b.remove(k), "remove {k}");
        }
        assert_eq!(a.len(), b.len());
        assert_eq!(a.first(), b.first());
        assert_eq!(a.last(), b.last());
        for &p in &probes(&keys) {
            assert_eq!(a.get(p), b.get(p), "get {p}");
            assert_eq!(a.contains_key(p), b.contains_key(p), "contains_key {p}");
            assert_eq!(a.next_after(p), b.next_after(p), "next_after {p}");
            assert_eq!(a.prev_before(p), b.prev_before(p), "prev_before {p}");
        }
        a.clear();
        b.clear();
        assert_eq!(a.len(), b.len());
        assert_eq!(a.last(), b.last());
    }
}
