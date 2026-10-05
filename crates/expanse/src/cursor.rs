//! Stateful ordered cursors with `advance_to` for skip-scans (issue #340).
//!
//! A [`SetCursor`] / [`MapCursor`] keeps the trie descent path it reached on
//! the previous step — the edge stack plus a leaf position, the zero-allocation
//! stack iterator from #245 / #343 — and, on [`SetCursor::advance_to`], reuses
//! it: a target inside the current leaf is a leaf-local search, a target under
//! a near ancestor re-descends only the levels it crosses, and only a target
//! beyond the whole current path re-descends from the root. This is the
//! primitive WAND / block-max skip-scan and merge-join want, where the
//! stateless [`crate::set::ExpanseSet::next_at_or_after`] pays a full root
//! re-descent every call. See docs/ALGORITHMS.md §3.5.
//!
//! Cursors borrow their container immutably for their whole lifetime, so the
//! trie cannot be mutated (and the held raw pointers cannot dangle) while a
//! cursor is live. Targets are expected non-decreasing across calls (monotone
//! skip-scan); a target at or below the current key leaves the cursor put and
//! never rewinds it.

use crate::iter::RawIter;
use crate::node::Edge;
use crate::types::{Key, Value};

/// Shared engine for the set and map cursors. `top` is the trie root edge, or
/// [`Edge::NULL`] for a flat root-leaf / empty container (seeks then stay
/// entirely leaf-local). `front` is the current position, peeked one step
/// ahead of the underlying [`RawIter`].
pub(crate) struct RawCursor<const MAP: bool> {
    raw: RawIter<MAP>,
    top: Edge,
    front: Option<(Key, u64)>,
}

impl<const MAP: bool> RawCursor<MAP> {
    #[inline]
    pub(crate) fn new(mut raw: RawIter<MAP>, top: Edge) -> Self {
        let front = raw.next();
        Self { raw, top, front }
    }

    /// An empty cursor, for storage that is re-seeded in place later.
    #[inline]
    pub(crate) fn empty() -> Self {
        Self {
            raw: RawIter::new(),
            top: Edge::NULL,
            front: None,
        }
    }

    /// Re-seeds the cursor in place: `seed` re-initialises the iterator
    /// (one of `RawIter`'s `reset_*`), `top` is the new trie root edge, and
    /// the cursor peeks its first entry — the state `new` builds, without
    /// constructing and moving a cursor (#1096).
    #[inline]
    pub(crate) fn reset(&mut self, top: Edge, seed: impl FnOnce(&mut RawIter<MAP>)) {
        seed(&mut self.raw);
        self.top = top;
        self.front = self.raw.next();
    }

    #[inline]
    pub(crate) fn current(&self) -> Option<(Key, u64)> {
        self.front
    }

    #[inline]
    pub(crate) fn next(&mut self) -> Option<(Key, u64)> {
        let cur = self.front;
        self.front = self.raw.next();
        cur
    }

    #[inline]
    pub(crate) fn advance_to(&mut self, target: Key) -> Option<(Key, u64)> {
        match self.front {
            // Already at or past `target`: monotone no-op, never rewind.
            Some((k, _)) if k >= target => self.front,
            Some(_) => {
                // SAFETY: `raw` is a live forward cursor over the trie rooted
                // at `top` (or a root leaf), kept valid by the container borrow
                // that this cursor holds for its whole lifetime.
                unsafe { self.raw.seek_forward(&self.top, target) };
                self.front = self.raw.next();
                self.front
            }
            None => None,
        }
    }
}

/// A stateful, forward-only ordered cursor over an [`crate::set::ExpanseSet`],
/// built for monotone skip-scans (WAND / block-max, merge-joins).
///
/// [`advance_to`](Self::advance_to) returns the smallest key `>= target` at or
/// after the cursor's current position, reusing the descent path from the
/// previous step. Construct with
/// [`ExpanseSet::cursor`](crate::set::ExpanseSet::cursor) or
/// [`cursor_from`](crate::set::ExpanseSet::cursor_from).
pub struct SetCursor<'a> {
    inner: RawCursor<false>,
    _set: core::marker::PhantomData<&'a crate::set::ExpanseSet>,
}

impl<'a> SetCursor<'a> {
    #[inline]
    pub(crate) fn new(raw: RawIter<false>, top: Edge) -> Self {
        Self {
            inner: RawCursor::new(raw, top),
            _set: core::marker::PhantomData,
        }
    }

    /// The key at the cursor's current position, or `None` past the end.
    #[inline]
    #[must_use]
    pub fn current(&self) -> Option<Key> {
        self.inner.current().map(|(k, _)| k)
    }

    /// Advances to and returns the smallest key `>= target` that is `>=` the
    /// cursor's current position; `None` once the set is exhausted.
    ///
    /// Targets are expected non-decreasing across calls; a `target` at or below
    /// the current key returns the current key without moving.
    #[inline]
    pub fn advance_to(&mut self, target: Key) -> Option<Key> {
        self.inner.advance_to(target).map(|(k, _)| k)
    }

    /// Returns the current key and advances one step; `None` past the end.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<Key> {
        self.inner.next().map(|(k, _)| k)
    }
}

/// A stateful, forward-only ordered cursor over an [`crate::map::ExpanseMap`],
/// yielding `(key, value)`. See [`SetCursor`] for the skip-scan contract.
///
/// Construct with [`ExpanseMap::cursor`](crate::map::ExpanseMap::cursor) or
/// [`cursor_from`](crate::map::ExpanseMap::cursor_from).
pub struct MapCursor<'a> {
    inner: RawCursor<true>,
    _map: core::marker::PhantomData<&'a crate::map::ExpanseMap>,
}

impl<'a> MapCursor<'a> {
    #[inline]
    pub(crate) fn new(raw: RawIter<true>, top: Edge) -> Self {
        Self {
            inner: RawCursor::new(raw, top),
            _map: core::marker::PhantomData,
        }
    }

    /// The `(key, value)` at the cursor's current position, or `None` past the
    /// end.
    #[inline]
    #[must_use]
    pub fn current(&self) -> Option<(Key, Value)> {
        self.inner.current()
    }

    /// Advances to and returns the entry with the smallest key `>= target` that
    /// is `>=` the cursor's current position; `None` once the map is exhausted.
    #[inline]
    pub fn advance_to(&mut self, target: Key) -> Option<(Key, Value)> {
        self.inner.advance_to(target)
    }

    /// Returns the current entry and advances one step; `None` past the end.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<(Key, Value)> {
        self.inner.next()
    }
}

/// A stateful, forward-only ordered cursor over a bounded range of an [`crate::map::ExpanseMap`],
/// yielding `(key, value)`. See [`SetCursor`] for the skip-scan contract.
///
/// Construct with [`ExpanseMap::range_cursor`](crate::map::ExpanseMap::range_cursor).
pub struct MapRangeCursor<'a> {
    cur: Option<MapCursor<'a>>,
    end: Key,
}

impl<'a> MapRangeCursor<'a> {
    #[inline]
    pub(crate) fn new(cur: MapCursor<'a>, end: Key) -> Self {
        Self {
            cur: Some(cur),
            end,
        }
    }

    #[inline]
    pub(crate) fn empty(end: Key) -> Self {
        Self { cur: None, end }
    }

    /// The upper inclusive bound for this cursor.
    #[inline]
    #[must_use]
    pub fn end_bound(&self) -> Key {
        self.end
    }

    /// The `(key, value)` at the cursor's current position, or `None` past the
    /// end.
    #[inline]
    #[must_use]
    pub fn current(&self) -> Option<(Key, Value)> {
        match self.cur.as_ref()?.current() {
            Some((k, v)) if k <= self.end => Some((k, v)),
            _ => None,
        }
    }

    /// Advances to and returns the entry with the smallest key `>= target` that
    /// is `>=` the cursor's current position and `<= end_bound()`; `None` once
    /// the map or range is exhausted.
    #[inline]
    pub fn advance_to(&mut self, target: Key) -> Option<(Key, Value)> {
        if target > self.end {
            // Nothing at or after `target` is in range: the cursor is done.
            self.cur = None;
            return None;
        }
        let cur = self.cur.as_mut()?;
        match cur.advance_to(target) {
            Some((k, v)) if k <= self.end => Some((k, v)),
            _ => None,
        }
    }

    /// Returns the current entry and advances one step; `None` past the end.
    #[inline]
    #[allow(clippy::should_implement_trait)] // discipline:allow(suppression-delta): clippy::should_implement_trait inherent cursor next
    pub fn next(&mut self) -> Option<(Key, Value)> {
        let cur = self.cur.as_mut()?;
        match cur.current() {
            Some((k, _)) if k <= self.end => cur.next(),
            _ => None,
        }
    }
}

impl<'a> Iterator for MapRangeCursor<'a> {
    type Item = (Key, Value);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        MapRangeCursor::next(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::map::ExpanseMap;
    use crate::set::ExpanseSet;
    use crate::types::Key;
    use std::collections::{BTreeMap, BTreeSet};

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

    /// A `BTreeSet`-backed reference cursor with exactly the semantics
    /// [`super::RawCursor`] promises: `front` is the current position, peeked
    /// one step ahead; `advance_to` never rewinds below it.
    struct RefCursor {
        sorted: Vec<Key>,
        idx: usize,
    }
    impl RefCursor {
        fn from_start(sorted: Vec<Key>) -> Self {
            Self { sorted, idx: 0 }
        }
        fn from_key(sorted: Vec<Key>, start: Key) -> Self {
            let idx = sorted.partition_point(|&k| k < start);
            Self { sorted, idx }
        }
        fn current(&self) -> Option<Key> {
            self.sorted.get(self.idx).copied()
        }
        fn next(&mut self) -> Option<Key> {
            let r = self.sorted.get(self.idx).copied();
            if r.is_some() {
                self.idx += 1;
            }
            r
        }
        fn advance_to(&mut self, target: Key) -> Option<Key> {
            match self.current() {
                Some(k) if k >= target => Some(k),
                Some(_) => {
                    self.idx = self.sorted.partition_point(|&k| k < target);
                    self.current()
                }
                None => None,
            }
        }
    }

    fn build(keys: &[Key]) -> (ExpanseSet, ExpanseMap, Vec<Key>, BTreeMap<Key, u64>) {
        let mut set = ExpanseSet::new();
        let mut map = ExpanseMap::new();
        let mut bset = BTreeSet::new();
        let mut bmap = BTreeMap::new();
        for &k in keys {
            let v = k.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5555;
            set.insert(k);
            map.insert(k, v);
            bset.insert(k);
            bmap.insert(k, v);
        }
        let sorted: Vec<Key> = bset.into_iter().collect();
        (set, map, sorted, bmap)
    }

    /// Drives set + map cursors against the reference over a scripted target
    /// stream (monotone or not), interleaving `next` per `also_next`.
    fn drive(
        set: &ExpanseSet,
        map: &ExpanseMap,
        bmap: &BTreeMap<Key, u64>,
        sorted: &[Key],
        start: Option<Key>,
        targets: &[Key],
        also_next: bool,
    ) {
        let (mut sc, mut mc, mut rc) = match start {
            None => (
                set.cursor(),
                map.cursor(),
                RefCursor::from_start(sorted.to_vec()),
            ),
            Some(s) => (
                set.cursor_from(s),
                map.cursor_from(s),
                RefCursor::from_key(sorted.to_vec(), s),
            ),
        };

        assert_eq!(sc.current(), rc.current(), "initial current (set)");
        assert_eq!(mc.current().map(|(k, _)| k), rc.current(), "initial (map)");

        for (i, &t) in targets.iter().enumerate() {
            let expect = rc.advance_to(t);
            let got_set = sc.advance_to(t);
            assert_eq!(got_set, expect, "advance_to({t:#x}) at step {i} (set)");
            let got_map = mc.advance_to(t);
            assert_eq!(
                got_map.map(|(k, _)| k),
                expect,
                "advance_to({t:#x}) at step {i} (map key)"
            );
            if let Some((k, v)) = got_map {
                assert_eq!(Some(&v), bmap.get(&k), "value for {k:#x}");
            }
            // current() must agree after each advance.
            assert_eq!(sc.current(), rc.current(), "current after advance step {i}");
            assert_eq!(mc.current().map(|(k, _)| k), rc.current());

            if also_next && i % 3 == 0 {
                let e = rc.next();
                assert_eq!(sc.next(), e, "next after advance step {i} (set)");
                assert_eq!(mc.next().map(|(k, _)| k), e, "next (map) step {i}");
            }
        }
    }

    /// Builds a diverse probe set: every key ±1, midpoints, boundaries.
    fn probes(sorted: &[Key]) -> Vec<Key> {
        let mut p = vec![0u64, 1, u64::MAX, u64::MAX - 1];
        for &k in sorted {
            p.push(k);
            p.push(k.saturating_sub(1));
            p.push(k.saturating_add(1));
        }
        p.sort_unstable();
        p.dedup();
        p
    }

    /// Exhaustive over every distribution: full monotone sweep of all probes
    /// (targets inside current leaf / sibling / distant / past-end / equal /
    /// repeated), plus starts at first / mid / last / beyond.
    fn check_distribution(keys: &[Key]) {
        let (set, map, sorted, bmap) = build(keys);
        let pr = probes(&sorted);

        // 1. Monotone full sweep from the start, no interleaved next.
        drive(&set, &map, &bmap, &sorted, None, &pr, false);
        // 2. Same, interleaving next().
        drive(&set, &map, &bmap, &sorted, None, &pr, true);

        // 3. Repeated-equal and equal-to-current targets: hit each probe twice.
        let mut repeated = Vec::new();
        for &t in &pr {
            repeated.push(t);
            repeated.push(t);
        }
        drive(&set, &map, &bmap, &sorted, None, &repeated, true);

        // 4. Starts at first / mid / last / beyond-end.
        if let (Some(&first), Some(&last)) = (sorted.first(), sorted.last()) {
            let mid = sorted[sorted.len() / 2];
            for &s in &[first, mid, last, last.saturating_add(1), 0, u64::MAX] {
                let tail: Vec<Key> = pr.iter().copied().filter(|&t| t >= s).collect();
                drive(&set, &map, &bmap, &sorted, Some(s), &tail, true);
                drive(&set, &map, &bmap, &sorted, Some(s), &pr, false);
            }
        }
    }

    /// A compact structural smoke test sized for Miri: one build spanning
    /// immediates, linear leaves, a dense bitmap-leaf/full-expanse run, and
    /// multi-level branches, driven by a short monotone + a few backward
    /// targets so every `seek_forward` leaf/branch arm and the ascend/root
    /// fallbacks execute under the interpreter without the probe explosion of
    /// [`check_distribution`].
    #[test]
    fn seek_smoke_all_node_types() {
        let mut keys: Vec<Key> = Vec::new();
        keys.extend(0u64..40); // immediates → linear leaves
        keys.extend((0u64..256).map(|i| 0xAABB_CC00 | i)); // bitmap leaf / full expanse
        keys.extend([1u64 << 40, 3u64 << 40, 0x1234_5678_9ABC_DEF0, u64::MAX]); // deep branches
        let (set, map, sorted, bmap) = build(&keys);

        // Bounded probe stream (each key ±1 and the extremes), monotone — every
        // seek_forward leaf/branch arm plus the ascend and root fallbacks run,
        // small enough for the Miri interpreter.
        let pr = probes(&sorted);
        drive(&set, &map, &bmap, &sorted, None, &pr, true);
        drive(&set, &map, &bmap, &sorted, Some(0xAABB_CC80), &pr, true);
    }

    #[test]
    fn empty_and_singleton() {
        check_distribution(&[]);
        check_distribution(&[0]);
        check_distribution(&[u64::MAX]);
        check_distribution(&[0x1234_5678]);
        check_distribution(&[0, u64::MAX]);
    }

    #[test]
    fn immediate_and_small() {
        check_distribution(&[10, 20, 30]);
        check_distribution(&(0u64..15).collect::<Vec<_>>());
        check_distribution(&(0u64..31).collect::<Vec<_>>()); // root-leaf cap
        check_distribution(&(0u64..40).collect::<Vec<_>>()); // just past → tree
    }

    #[test]
    fn dense_byte_run() {
        // 0..=255 under one prefix: immediate → leaf1 → bitmap → full expanse.
        check_distribution(&(0u64..=255).collect::<Vec<_>>());
        let base = 0xAABB_CCDD_EE00u64;
        check_distribution(&(0u64..256).map(|i| base | i).collect::<Vec<_>>());
    }

    #[test]
    fn linear_and_bitmap_leaves() {
        check_distribution(&(100u64..180).collect::<Vec<_>>());
        check_distribution(&(1000u64..2000).step_by(3).collect::<Vec<_>>());
    }

    #[test]
    fn clustered_multi_expanse() {
        let mut keys = Vec::new();
        for base in [0u64, 0xDEAD_0000, 0xFFFF_FFFF_FF00, 0x1234_5678_9ABC_0000] {
            for i in 0..200u64 {
                keys.push(base.wrapping_add(i));
            }
        }
        check_distribution(&keys);
    }

    #[test]
    fn sparse_single_key_immediates() {
        // Every leaf a single-key immediate (bytes 0..5 zero); distant skips.
        check_distribution(&(0u64..400).map(|i| i << 40).collect::<Vec<_>>());
    }

    #[test]
    fn boundary_keys() {
        check_distribution(&[
            0,
            1,
            255,
            256,
            257,
            65535,
            65536,
            1 << 24,
            1 << 32,
            (1 << 32) - 1,
            1 << 48,
            u64::MAX - 1,
            u64::MAX,
        ]);
    }

    #[test]
    fn random_and_zipfian() {
        // Every round drives the same cursor paths over a fresh random
        // population; the repetition buys distribution variety, not new
        // paths. Under Miri all twelve rounds were the largest single item of
        // the cursor shard in the lane's first measured run (docs/CI.md §5,
        // Tier 3), so the interpreter runs two and the native test job keeps
        // all twelve on every PR.
        const ROUNDS: usize = if cfg!(miri) { 2 } else { 12 };
        let mut rng = XorShift(0xC0FF_EE12_3456_789A);
        for _ in 0..ROUNDS {
            // Full-width random.
            let n = 300 + (rng.next() % 400) as usize;
            let keys: Vec<Key> = (0..n).map(|_| rng.next()).collect();
            check_distribution(&keys);
            // Zipfian-ish: many keys crowded into a small low range, a few far.
            let zipf: Vec<Key> = (0..n)
                .map(|_| {
                    let r = rng.next();
                    match r % 10 {
                        0 => r,                       // rare: full-width
                        1 | 2 => (r % 100_000) << 20, // uncommon: mid
                        _ => r % 1000,                // common: crowded low
                    }
                })
                .collect();
            check_distribution(&zipf);
        }
    }

    #[test]
    fn monotone_stream_deep_skips() {
        // A large sparse set with a monotone target walk that skips whole
        // subtrees each step — the WAND skip-scan shape.
        let mut rng = XorShift(0x1357_9BDF_2468_ACE0);
        let keys: Vec<Key> = (0..5000).map(|_| rng.next()).collect();
        let (set, map, sorted, bmap) = build(&keys);

        // Monotone targets striding forward by random jumps.
        let mut targets = Vec::new();
        let mut t = 0u64;
        while t < u64::MAX {
            targets.push(t);
            let step = rng.next() % (u64::MAX / 200 + 1);
            match t.checked_add(step) {
                Some(nt) => t = nt,
                None => break,
            }
        }
        drive(&set, &map, &bmap, &sorted, None, &targets, false);
        drive(&set, &map, &bmap, &sorted, None, &targets, true);

        // Also from a mid start.
        drive(
            &set,
            &map,
            &bmap,
            &sorted,
            Some(sorted[sorted.len() / 2]),
            &targets,
            true,
        );
    }

    #[test]
    fn interleave_next_and_advance() {
        let keys: Vec<Key> = (0u64..1000).map(|i| i * 37).collect();
        let (set, map, sorted, _bmap) = build(&keys);
        let mut sc = set.cursor();
        let mut mc = map.cursor();
        let mut rc = RefCursor::from_start(sorted.clone());
        // Alternate next / advance_to with growing targets.
        for i in 0..500u64 {
            if i % 2 == 0 {
                let e = rc.next();
                assert_eq!(sc.next(), e);
                assert_eq!(mc.next().map(|(k, _)| k), e);
            } else {
                let t = i * 71;
                let e = rc.advance_to(t);
                assert_eq!(sc.advance_to(t), e);
                assert_eq!(mc.advance_to(t).map(|(k, _)| k), e);
            }
        }
    }

    #[test]
    fn test_map_range_cursor_boundary_empty_map() {
        let map = ExpanseMap::new();
        // Unbounded
        let mut cur = map.range_cursor(..);
        assert_eq!(cur.end_bound(), u64::MAX);
        assert_eq!(cur.current(), None);
        assert_eq!(cur.next(), None);
        assert_eq!(cur.advance_to(10), None);
        assert_eq!(map.range_cursor(..).collect::<Vec<_>>(), vec![]);

        // Inclusive range
        let mut cur = map.range_cursor(10..=20);
        assert_eq!(cur.end_bound(), 20);
        assert_eq!(cur.current(), None);
        assert_eq!(cur.next(), None);
        assert_eq!(cur.advance_to(15), None);

        // Extremal
        assert_eq!(map.range_cursor(0..=0).collect::<Vec<_>>(), vec![]);
        assert_eq!(
            map.range_cursor(u64::MAX..=u64::MAX).collect::<Vec<_>>(),
            vec![]
        );
        assert_eq!(map.range_cursor(0..=u64::MAX).collect::<Vec<_>>(), vec![]);

        // Inverted
        let (lo, hi) = (50, 10);
        assert_eq!(map.range_cursor(lo..=hi).collect::<Vec<_>>(), vec![]);
    }

    #[test]
    fn test_map_range_cursor_boundary_lo_greater_than_hi() {
        let mut map = ExpanseMap::new();
        for k in [10u64, 20, 30, 40, 50] {
            map.insert(k, k * 10);
        }

        let (lo, hi) = (50, 10);
        let mut cur = map.range_cursor(lo..=hi);
        assert_eq!(cur.end_bound(), 10);
        assert_eq!(cur.current(), None);
        assert_eq!(cur.next(), None);
        assert_eq!(cur.advance_to(10), None);
        assert_eq!(cur.advance_to(30), None);
        assert_eq!(map.range_cursor(lo..=hi).collect::<Vec<_>>(), vec![]);

        let (lo, hi) = (100, 50);
        let mut cur = map.range_cursor(lo..hi);
        assert_eq!(cur.end_bound(), 49);
        assert_eq!(cur.current(), None);
        assert_eq!(cur.next(), None);
        assert_eq!(map.range_cursor(lo..hi).collect::<Vec<_>>(), vec![]);
    }

    #[test]
    fn test_map_range_cursor_boundary_lo_equals_hi_present() {
        let mut map = ExpanseMap::new();
        for k in [10u64, 20, 30, 40, 50] {
            map.insert(k, k * 10);
        }

        // Test lo == hi present (20..=20)
        let mut cur = map.range_cursor(20..=20);
        assert_eq!(cur.end_bound(), 20);
        assert_eq!(cur.current(), Some((20, 200)));
        assert_eq!(cur.next(), Some((20, 200)));
        assert_eq!(cur.next(), None);
        assert_eq!(cur.current(), None);

        // Multiple iterates via collect
        assert_eq!(
            map.range_cursor(20..=20).collect::<Vec<_>>(),
            vec![(20, 200)]
        );
        assert_eq!(
            map.range_cursor(10..=10).collect::<Vec<_>>(),
            vec![(10, 100)]
        );
        assert_eq!(
            map.range_cursor(50..=50).collect::<Vec<_>>(),
            vec![(50, 500)]
        );
    }

    #[test]
    fn test_map_range_cursor_boundary_lo_equals_hi_absent() {
        let mut map = ExpanseMap::new();
        for k in [10u64, 20, 30, 40, 50] {
            map.insert(k, k * 10);
        }

        // Absent key: 15..=15
        let mut cur = map.range_cursor(15..=15);
        assert_eq!(cur.end_bound(), 15);
        assert_eq!(cur.current(), None);
        assert_eq!(cur.next(), None);
        assert_eq!(cur.advance_to(15), None);
        assert_eq!(map.range_cursor(15..=15).collect::<Vec<_>>(), vec![]);

        // Absent key before first: 5..=5
        assert_eq!(map.range_cursor(5..=5).collect::<Vec<_>>(), vec![]);

        // Absent key after last: 55..=55
        assert_eq!(map.range_cursor(55..=55).collect::<Vec<_>>(), vec![]);
    }

    #[test]
    fn test_map_range_cursor_boundary_between_keys() {
        let mut map = ExpanseMap::new();
        for k in [10u64, 20, 30, 40, 50] {
            map.insert(k, k * 10);
        }

        // Between keys with nothing present in range
        let mut cur = map.range_cursor(12..=18);
        assert_eq!(cur.end_bound(), 18);
        assert_eq!(cur.current(), None);
        assert_eq!(cur.next(), None);
        assert_eq!(map.range_cursor(12..=18).collect::<Vec<_>>(), vec![]);

        // Between keys with keys in range: 15..=35 -> [20, 30]
        let mut cur = map.range_cursor(15..=35);
        assert_eq!(cur.end_bound(), 35);
        assert_eq!(cur.current(), Some((20, 200)));
        assert_eq!(cur.next(), Some((20, 200)));
        assert_eq!(cur.current(), Some((30, 300)));
        assert_eq!(cur.next(), Some((30, 300)));
        assert_eq!(cur.current(), None);
        assert_eq!(cur.next(), None);
        assert_eq!(
            map.range_cursor(15..=35).collect::<Vec<_>>(),
            vec![(20, 200), (30, 300)]
        );

        // Half-open: 15..30 -> only [20]
        assert_eq!(
            map.range_cursor(15..30).collect::<Vec<_>>(),
            vec![(20, 200)]
        );
    }

    #[test]
    fn test_map_range_cursor_boundary_extremal_keys() {
        let mut map = ExpanseMap::new();
        map.insert(0, 100);
        map.insert(1, 101);
        map.insert(u64::MAX - 1, 998);
        map.insert(u64::MAX, 999);

        // 0..=0
        assert_eq!(map.range_cursor(0..=0).collect::<Vec<_>>(), vec![(0, 100)]);

        // MAX..=MAX
        assert_eq!(
            map.range_cursor(u64::MAX..=u64::MAX).collect::<Vec<_>>(),
            vec![(u64::MAX, 999)]
        );

        // 0..=MAX
        assert_eq!(
            map.range_cursor(0..=u64::MAX).collect::<Vec<_>>(),
            vec![(0, 100), (1, 101), (u64::MAX - 1, 998), (u64::MAX, 999)]
        );

        // Bound::Excluded(0) at end
        use core::ops::Bound;
        assert_eq!(
            map.range_cursor((Bound::Unbounded, Bound::Excluded(0)))
                .collect::<Vec<_>>(),
            vec![]
        );

        // Bound::Excluded(u64::MAX) at start
        assert_eq!(
            map.range_cursor((Bound::Excluded(u64::MAX), Bound::Unbounded))
                .collect::<Vec<_>>(),
            vec![]
        );

        // Advance to MAX from middle
        let mut cur = map.range_cursor(0..=u64::MAX);
        assert_eq!(cur.advance_to(u64::MAX), Some((u64::MAX, 999)));
        assert_eq!(cur.next(), Some((u64::MAX, 999)));
        assert_eq!(cur.next(), None);
    }

    #[test]
    fn test_map_range_cursor_differential_btreemap() {
        use std::collections::BTreeMap;
        let mut map = ExpanseMap::new();
        let mut btree = BTreeMap::new();

        // Populate varied distributions:
        // 1. Small cluster
        for i in 0..30u64 {
            map.insert(i, i * 3);
            btree.insert(i, i * 3);
        }
        // 2. Dense byte run (LeafB1)
        for i in 0..256u64 {
            let k = 0xAABB_0000 | i;
            map.insert(k, k ^ 0x55);
            btree.insert(k, k ^ 0x55);
        }
        // 3. Sparse deep branches
        for &k in &[
            1u64 << 40,
            3u64 << 40,
            0x1234_5678_9ABC_DEF0,
            u64::MAX - 10,
            u64::MAX,
        ] {
            map.insert(k, k ^ 0xAA);
            btree.insert(k, k ^ 0xAA);
        }

        // Test various ranges
        let test_ranges: Vec<(core::ops::Bound<u64>, core::ops::Bound<u64>)> = vec![
            (core::ops::Bound::Unbounded, core::ops::Bound::Unbounded),
            (
                core::ops::Bound::Included(0),
                core::ops::Bound::Included(10),
            ),
            (
                core::ops::Bound::Included(5),
                core::ops::Bound::Excluded(25),
            ),
            (
                core::ops::Bound::Included(0xAABB_0010),
                core::ops::Bound::Included(0xAABB_0080),
            ),
            (
                core::ops::Bound::Excluded(0xAABB_0010),
                core::ops::Bound::Excluded(0xAABB_0080),
            ),
            (
                core::ops::Bound::Included(1u64 << 40),
                core::ops::Bound::Included(u64::MAX),
            ),
            (
                core::ops::Bound::Included(500),
                core::ops::Bound::Included(600),
            ), // gap
            (
                core::ops::Bound::Included(100),
                core::ops::Bound::Included(50),
            ), // inverted
            (
                core::ops::Bound::Included(u64::MAX),
                core::ops::Bound::Included(u64::MAX),
            ),
        ];

        for bounds in test_ranges {
            let exp: Vec<(u64, u64)> = map.range_cursor(bounds).collect();
            let bt: Vec<(u64, u64)> = match (bounds.0, bounds.1) {
                (core::ops::Bound::Included(lo), core::ops::Bound::Included(hi)) if lo > hi => {
                    vec![]
                }
                (core::ops::Bound::Included(lo), core::ops::Bound::Excluded(hi)) if lo >= hi => {
                    vec![]
                }
                (core::ops::Bound::Excluded(lo), core::ops::Bound::Included(hi)) if lo >= hi => {
                    vec![]
                }
                (core::ops::Bound::Excluded(lo), core::ops::Bound::Excluded(hi))
                    if lo >= hi.saturating_sub(1) =>
                {
                    vec![]
                }
                _ => btree.range(bounds).map(|(&k, &v)| (k, v)).collect(),
            };
            assert_eq!(exp, bt, "Mismatch for bounds {:?}", bounds);
        }
    }

    #[test]
    fn test_map_range_cursor_iterator_trait() {
        let mut map = ExpanseMap::new();
        map.insert(1, 10);
        map.insert(2, 20);
        map.insert(3, 30);

        // Test standard Iterator methods: map, filter, fold
        let sum: u64 = map.range_cursor(..).map(|(_, v)| v).sum();
        assert_eq!(sum, 60);

        let keys: Vec<u64> = map.range_cursor(2..=3).map(|(k, _)| k).collect();
        assert_eq!(keys, vec![2, 3]);

        // for-in loop consumption
        let mut count = 0;
        for (k, v) in map.range_cursor(1..=2) {
            assert_eq!(v, k * 10);
            count += 1;
        }
        assert_eq!(count, 2);
    }

    #[test]
    fn test_map_range_cursor_advance_to_past_end_exhausts() {
        let mut map = crate::map::ExpanseMap::new();
        for k in 1..=100u64 {
            map.insert(k, k as _);
        }
        let mut cur = map.range_cursor(1..=50);
        assert_eq!(cur.next().map(|e| e.0), Some(1));
        assert_eq!(cur.advance_to(51), None);
        assert_eq!(cur.current(), None);
        assert_eq!(cur.next(), None);
        assert_eq!(cur.advance_to(10), None);

        // A seek that lands past the end agrees with one that starts past it.
        let mut cur = map.range_cursor(1..50);
        assert_eq!(cur.advance_to(50), None);
        assert_eq!(cur.current(), None);
        assert_eq!(cur.next(), None);
    }
}
