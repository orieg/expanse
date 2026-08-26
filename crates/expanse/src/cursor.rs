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
        let mut rng = XorShift(0xC0FF_EE12_3456_789A);
        for _ in 0..12 {
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
}
