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
    /// Cached descent to the expanse the last insert terminated in. A plain
    /// field, not a `Cell`: `insert` already takes `&mut self`, and an
    /// interior-mutability wrapper would make the container non-`Sync` and
    /// break the `sync32` bound.
    finger: trie32::Finger32,
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
            finger: trie32::Finger32::new(),
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
            finger: trie32::Finger32::new(),
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
        // The cached path answers a key in the same expanse as the last
        // insert -- the shape a monotonic stream produces -- without the
        // descent. It reports a miss rather than guessing, so a stale or
        // inapplicable finger costs one compare.
        if let Some(old) = trie32::map_insert_via_finger(&mut self.alloc, key, value, &self.finger)
        {
            #[cfg(test)]
            finger_census::hit();
            if old.is_none() {
                self.len += 1;
            }
            return old;
        }
        #[cfg(test)]
        finger_census::miss();
        let old = trie32::map_insert_f(
            &mut self.alloc,
            &mut self.root,
            4,
            key,
            value,
            &mut self.finger,
        );
        self.finger.set_prefix(key >> 8);
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
        self.finger.clear();
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
        self.finger.clear();
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
        self.finger.clear();
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
        trie32::last_entry(&self.alloc, &self.root, 4)
    }

    /// Returns the entry with the smallest key strictly greater than `key`.
    #[inline]
    #[must_use]
    pub fn next(&self, key: Key32) -> Option<(Key32, Value32)> {
        trie32::next_entry(&self.alloc, &self.root, 4, key)
    }

    /// Returns the entry with the largest key strictly smaller than `key`.
    #[inline]
    #[must_use]
    pub fn prev(&self, key: Key32) -> Option<(Key32, Value32)> {
        trie32::prev_entry(&self.alloc, &self.root, 4, key)
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
        self.try_for_each_range(start, end, |k, v| {
            if pred(k, v) {
                cb(k, v);
            }
            true
        });
    }

    /// Walks entries in the inclusive range `[start, end]` in ascending key
    /// order, calling `f` on each until it returns `false`. Returns `false`
    /// when `f` stopped the walk, `true` when the range was exhausted.
    ///
    /// One descent to `start` and then contiguous streaming through the
    /// leaves the range spans, where a `next_after` loop pays a fresh
    /// `O(depth)` descent per key (#614).
    #[inline]
    pub fn try_for_each_range<F>(&self, start: Key32, end: Key32, mut f: F) -> bool
    where
        F: FnMut(Key32, Value32) -> bool,
    {
        if start > end {
            return true;
        }
        trie32::map_for_each_range(&self.alloc, &self.root, 4, start, end, &mut f)
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
            fwd: None,
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
            fwd: None,
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
    /// The forward walk's descent path, built on the first `next` so an
    /// iterator only ever driven from the back never pays for it (#614).
    /// The back end stays descent-based: `next_back` lowers `hi`, and the
    /// forward walk stops at the first key above it, so the two ends still
    /// cannot cross.
    fwd: Option<trie32::RawIter32<'a>>,
}

impl<'a> Iterator for MapIter32<'a> {
    type Item = (Key32, Value32);

    #[inline]
    fn next(&mut self) -> Option<(Key32, Value32)> {
        if self.done {
            return None;
        }
        if self.fwd.is_none() {
            let map = self.map;
            self.fwd = Some(trie32::RawIter32::new(&map.alloc, map.root, 4, self.lo));
        }
        let Some((k, v)) = self.fwd.as_mut().and_then(trie32::RawIter32::next) else {
            self.done = true;
            return None;
        };
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

/// Test-only census of the insert finger: how many inserts the cached path
/// answered and how many took the full descent. Compiled out of every
/// non-test build, so the insert path is untouched (AGENTS.md §6). Per
/// thread, because the test harness runs tests in parallel and a shared
/// counter would count the neighbours' inserts.
#[cfg(test)]
pub(crate) mod finger_census {
    use std::cell::Cell;

    thread_local! {
        static HITS: Cell<u64> = const { Cell::new(0) };
        static MISSES: Cell<u64> = const { Cell::new(0) };
    }

    pub(crate) fn hit() {
        HITS.with(|c| c.set(c.get() + 1));
    }

    pub(crate) fn miss() {
        MISSES.with(|c| c.set(c.get() + 1));
    }

    pub(crate) fn reset() {
        HITS.with(|c| c.set(0));
        MISSES.with(|c| c.set(0));
    }

    /// `(hits, misses)` since the last reset, on this thread.
    pub(crate) fn snapshot() -> (u64, u64) {
        (HITS.with(Cell::get), MISSES.with(Cell::get))
    }
}

#[cfg(test)]
mod tests {

    /// `range` and `try_for_each_range` must visit the same entries in the
    /// same order.
    ///
    /// They are two different walks over one structure: `range` streams a
    /// `LeafCur32` cursor, `try_for_each_range` recurses through
    /// `trie32::map_for_each_range`. Nothing tied them together, and nothing
    /// measured the second — which is how the cursor lost its per-key popcount
    /// while the C ABI path kept one. This is the equivalence any optimisation
    /// to either walk has to preserve.
    #[test]
    fn range_and_for_each_range_agree() {
        for dist in ["sequential", "clustered", "random"] {
            let mut x: u64 = 0x2545_F491_4F6C_DD1D;
            let mut m = ExpanseMap32::new();
            for i in 0..2_000u32 {
                let k = match dist {
                    "random" => {
                        x ^= x << 13;
                        x ^= x >> 7;
                        x ^= x << 17;
                        (x >> 16) as Key32
                    }
                    "clustered" => (i / 8) * 4096 + (i % 8),
                    _ => i,
                };
                m.insert(k, (i * 3) as Value32);
            }
            for (lo, hi) in [
                (0, Key32::MAX / 2),
                (0, Key32::MAX),
                (37, 41),
                (Key32::MAX / 2, Key32::MAX),
            ] {
                let via_cursor: Vec<(Key32, Value32)> = m.range(lo..=hi).collect();
                let mut via_recursion = Vec::new();
                m.try_for_each_range(lo, hi, |k, v| {
                    via_recursion.push((k, v));
                    true
                });
                assert_eq!(
                    via_cursor, via_recursion,
                    "{dist}: the two walks disagree over [{lo}, {hi}]"
                );
            }
            // The callback's `false` must stop the walk, which the cursor has
            // no analogue for and only this path can get wrong.
            let mut seen = 0usize;
            m.try_for_each_range(0, Key32::MAX, |_, _| {
                seen += 1;
                seen < 5
            });
            assert_eq!(seen, 5, "{dist}: a false return did not stop the walk");
        }
    }

    /// The `map32_iterate` / `map32_range` arms exist to measure the ordered
    /// walk, and its bitmap-leaf path is only reached when a distribution is
    /// dense enough to promote past `MAP_BITMAP_ENTER_32`. This pins which of
    /// the three benchmark shapes actually covers it: `sequential` does,
    /// `clustered` and `random` do not, so an optimisation to the bitmap walk
    /// should move the sequential arms and leave the other two flat.
    ///
    /// Mirrors `keys32` in `benches/instructions.rs`. If that generator
    /// changes shape, this fails rather than letting the arm silently stop
    /// covering the path it was added for.
    #[test]
    fn bench_distributions_cover_the_bitmap_walk() {
        fn keys32(dist: &str) -> Vec<Key32> {
            let mut x: u64 = 0x2545_F491_4F6C_DD1D;
            (0..2_000u32)
                .map(|i| match dist {
                    "random" => {
                        x ^= x << 13;
                        x ^= x >> 7;
                        x ^= x << 17;
                        (x >> 16) as Key32
                    }
                    "clustered" => (i / 8) * 4096 + (i % 8),
                    _ => i,
                })
                .collect()
        }
        for (dist, want_bitmaps) in [
            ("sequential", true),
            ("clustered", false),
            ("random", false),
        ] {
            let mut m = ExpanseMap32::new();
            for (i, k) in keys32(dist).into_iter().enumerate() {
                m.insert(k, (i * 3) as Value32);
            }
            let n = m.alloc.map_bitmap_count();
            assert_eq!(
                n > 0,
                want_bitmaps,
                "{dist}: {n} map bitmap leaves, expected {}",
                if want_bitmaps { "at least one" } else { "none" }
            );
            // The walk must still yield every key whatever the leaf form.
            assert_eq!(m.iter().count(), 2_000, "{dist}: walk lost keys");
        }
    }

    use super::*;
    #[cfg(not(feature = "std"))]
    extern crate alloc as alloc_crate;
    #[cfg(not(feature = "std"))]
    use alloc_crate::collections::BTreeMap;
    #[cfg(not(feature = "std"))]
    use alloc_crate::vec::Vec;
    use core::ops::Bound;
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
        // Forty rounds of up to 3,200 keys and 64 model-checked removals each
        // was the single largest item in the nightly Miri lane's first
        // measured run (docs/CI.md §5, Tier 3); every round exercises the same
        // removal paths on a fresh population, so the interpreter runs three
        // and the native test job keeps all forty on every PR.
        const ROUNDS: u32 = if cfg!(miri) { 3 } else { 40 };
        let mut state = 0x2F6E_2B1Fu32;
        let mut lcg = move || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            state
        };
        for round in 0..ROUNDS {
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

    /// The fused single-descent walk (`next_entry` / `prev_entry` /
    /// `last_entry`, #614) must return exactly what the two-descent
    /// composition it replaced returned: the key the key-only primitive
    /// finds, paired with that key's `get` value. Asserted across the node
    /// forms a 32-bit map takes — immediates, linear leaves, bitmap leaves
    /// and every branch class — plus the two saturating bounds where the
    /// fused path returns early.
    #[test]
    fn fused_entry_walk_matches_key_then_get() {
        // Three masks × (4,000 inserts + two full walks + 2,000 probes) is the
        // largest item in the map shard under Miri; the walks and probes
        // scale with the population, so the interpreter runs a quarter.
        const N: usize = if cfg!(miri) { 1_000 } else { 4_000 };
        for &key_mask in &[0x0000_03FFu32, 0x000F_FFFF, 0xFFFF_FFFF] {
            let mut rng = XorShift::new(0xC0FFEE ^ u64::from(key_mask));
            let mut map = ExpanseMap32::new();
            let mut model: BTreeMap<u32, u32> = BTreeMap::new();
            for _ in 0..N {
                let k = rng.next() & key_mask;
                let v = rng.next();
                map.insert(k, v);
                model.insert(k, v);
            }
            // Saturating bounds: nothing is above u32::MAX or below 0.
            assert_eq!(map.next(u32::MAX), None, "next past the top of the space");
            assert_eq!(map.prev(0), None, "prev below the bottom of the space");

            // Forward: every step's value is the one `get` reports.
            let mut cur = map.first();
            while let Some((k, v)) = cur {
                assert_eq!(map.get(k), Some(v), "forward value at {k:#x}");
                assert_eq!(model.get(&k).copied(), Some(v), "forward model at {k:#x}");
                cur = map.next(k);
            }
            // Backward from `last_entry`, the same invariant.
            let mut cur = map.last();
            while let Some((k, v)) = cur {
                assert_eq!(map.get(k), Some(v), "backward value at {k:#x}");
                assert_eq!(model.get(&k).copied(), Some(v), "backward model at {k:#x}");
                cur = map.prev(k);
            }
            // Probe keys that are mostly absent: the fused path must agree
            // with the model's own successor and predecessor.
            for _ in 0..N / 2 {
                let probe = rng.next() & key_mask;
                assert_eq!(
                    map.next(probe),
                    model
                        .range((Bound::Excluded(probe), Bound::Unbounded))
                        .next()
                        .map(|(&k, &v)| (k, v)),
                    "next({probe:#x})"
                );
                assert_eq!(
                    map.prev(probe),
                    model.range(..probe).next_back().map(|(&k, &v)| (k, v)),
                    "prev({probe:#x})"
                );
            }
        }
    }

    /// The streaming range walk (#614) must reproduce the model's range
    /// exactly, and must stop where the callback says — calling it neither
    /// again nor for entries past the stop.
    #[test]
    fn range_walk_matches_model_and_stops_on_request() {
        for &key_mask in &[0x0000_03FFu32, 0x000F_FFFF, 0xFFFF_FFFF] {
            let mut rng = XorShift::new(0xBEEF ^ u64::from(key_mask));
            let mut map = ExpanseMap32::new();
            let mut model: BTreeMap<u32, u32> = BTreeMap::new();
            for _ in 0..4_000 {
                let k = rng.next() & key_mask;
                let v = rng.next();
                map.insert(k, v);
                model.insert(k, v);
            }
            for &(lo, hi) in &[
                (0u32, u32::MAX),
                (0, key_mask / 2),
                (key_mask / 4, key_mask / 2),
                (key_mask / 2, key_mask),
                (key_mask, key_mask),
                // An inverted range walks nothing and reports completion.
                (key_mask, 0),
            ] {
                let expected: Vec<(u32, u32)> = if lo > hi {
                    Vec::new()
                } else {
                    model.range(lo..=hi).map(|(&k, &v)| (k, v)).collect()
                };
                let mut got = Vec::new();
                assert!(
                    map.try_for_each_range(lo, hi, |k, v| {
                        got.push((k, v));
                        true
                    }),
                    "exhausted walk reports completion ({lo:#x},{hi:#x})"
                );
                assert_eq!(got, expected, "range walk ({lo:#x},{hi:#x})");

                // Stopping after n entries yields exactly the first n.
                for n in [1usize, 3, 17] {
                    if expected.len() <= n {
                        continue;
                    }
                    let mut seen = Vec::new();
                    assert!(
                        !map.try_for_each_range(lo, hi, |k, v| {
                            seen.push((k, v));
                            seen.len() < n
                        }),
                        "stopped walk reports the stop ({lo:#x},{hi:#x}) n={n}"
                    );
                    assert_eq!(seen, expected[..n], "stop after {n} ({lo:#x},{hi:#x})");
                }
            }
        }
    }

    /// The finger's hit rate on the monotonic ingest shape the 32-bit
    /// benchmarks and the on-device fixtures use (`1_700_000_000 + i`), so the
    /// amortisation #625 landed is a measured share rather than an assumed one
    /// (#615: "the hit rate is unmeasured").
    ///
    /// The finger arms only inside the `Kind::MapBitmap` arm of
    /// `trie32::map_insert_f`, i.e. for an insert that lands in an *existing*
    /// level-1 bitmap leaf. A monotonic fill therefore takes the full descent
    /// for `MAP_BITMAP_ENTER_32 + 2` inserts of every 256-key expanse: the
    /// `MAP_BITMAP_ENTER_32` inserts before the leaf reaches promotion size,
    /// the promoting insert (which converts the leaf and does not arm), and
    /// the first insert into the new bitmap (which arms). Every later insert
    /// in the expanse hits. Measured at the shipped constants: 7,410 hits and
    /// 2,590 misses over 10,000 keys, a 74.1% hit rate against a 74.9%
    /// ceiling if the promoting insert armed. The two extra misses are one
    /// descent per 128 keys and are not worth a change; the test pins the
    /// count so a regression that disarms the finger inside an expanse is
    /// caught.
    #[test]
    fn finger_hit_rate_on_monotonic_ingest() {
        use crate::types32::MAP_BITMAP_ENTER_32;
        const N: u32 = 10_000;
        const BASE: u32 = 1_700_000_000;
        let mut map = ExpanseMap32::new();
        super::finger_census::reset();
        for i in 0..N {
            map.insert(BASE + i, i);
        }
        let (hits, misses) = super::finger_census::snapshot();
        assert_eq!(hits + misses, u64::from(N));
        // Per 256-key expanse: MAP_BITMAP_ENTER_32 pre-promotion inserts, the
        // promoting insert, and the arming insert all miss. BASE is
        // 256-aligned, so the last expanse is partial and contributes only
        // as many misses as it has inserts, capped at that count.
        let per_expanse = MAP_BITMAP_ENTER_32 as u64 + 2;
        let full = u64::from(N / 256);
        let tail = u64::from(N % 256);
        let expected_misses = full * per_expanse + tail.min(per_expanse);
        let rate = hits as f64 / f64::from(N);
        println!(
            "map32 finger census on monotonic ingest: {hits} hits / {misses} misses over {N} inserts \
             ({rate:.4}); {per_expanse} misses per 256-key expanse with MAP_BITMAP_ENTER_32 = {MAP_BITMAP_ENTER_32}"
        );
        assert_eq!(
            misses, expected_misses,
            "the finger should miss exactly the pre-promotion, promoting and arming inserts of each expanse"
        );
    }
}
