//! Stateful ordered cursors for the 32-bit twins (issue #340).
//!
//! [`SetCursor32`] / [`MapCursor32`] present the same `current` / `advance_to`
//! / `next` skip-scan surface as the 64-bit [`crate::cursor`] cursors, so
//! callers can be written once against either width.
//!
//! Sequential stepping is path-reusing: `next` runs on the 32-bit trie's
//! stack iterator (`trie32::RawIter32`, #614), which holds the descent path
//! across calls, so a step is a leaf index bump or one child lookup at a
//! branch already in hand rather than an O(depth) root descent.
//!
//! A **skip** still re-descends. `advance_to` short-circuits a `target` at or
//! below the current position (the monotone no-op), and otherwise abandons the
//! held path and re-seeks from the root through the stateless
//! [`ExpanseSet32::next`](crate::set32::ExpanseSet32::next) — where the 64-bit
//! [`crate::cursor`] cursors reuse the path they already hold. Teaching
//! `RawIter32` to advance an existing path to a target is the remaining half
//! of the 64-bit parity.
//!
//! On 32-bit targets these types are also re-exported at the crate root as the
//! unsuffixed `SetCursor` / `MapCursor` (mirroring the `ExpanseSet32` →
//! `ExpanseSet` re-point), so downstream code names the same cursor types on
//! either width.

use crate::map32::ExpanseMap32;
use crate::set32::ExpanseSet32;
use crate::types32::{Key32, Value32};

/// A stateful, forward-only ordered cursor over an [`ExpanseSet32`].
///
/// See the [module docs](self): `next` reuses the held descent path, a
/// forward `advance_to` re-descends from the root.
pub struct SetCursor32<'a> {
    set: &'a ExpanseSet32,
    front: Option<Key32>,
    /// The held descent path, positioned to yield the key after `front`.
    /// Built on the first step and dropped by a skip, which re-seeks.
    walk: Option<crate::trie32::RawIter32<'a>>,
}

impl<'a> SetCursor32<'a> {
    #[inline]
    pub(crate) fn new(set: &'a ExpanseSet32, front: Option<Key32>) -> Self {
        Self {
            set,
            front,
            walk: None,
        }
    }

    /// Smallest key `>= bound`, via the stateless primitives.
    #[inline]
    fn seek(set: &ExpanseSet32, bound: Key32) -> Option<Key32> {
        match bound.checked_sub(1) {
            Some(b) => set.next(b),
            None => set.first(),
        }
    }

    /// The key at the cursor's current position, or `None` past the end.
    #[inline]
    #[must_use]
    pub fn current(&self) -> Option<Key32> {
        self.front
    }

    /// Advances to and returns the smallest key `>= target` that is `>=` the
    /// cursor's current position; `None` once the set is exhausted. Targets are
    /// expected non-decreasing; a `target` at or below the current key returns
    /// it without moving.
    #[inline]
    pub fn advance_to(&mut self, target: Key32) -> Option<Key32> {
        match self.front {
            Some(k) if k >= target => self.front,
            Some(_) => {
                // A skip abandons the held path: it may land anywhere at or
                // after the target, so the path is re-seeked from the root.
                self.walk = None;
                self.front = Self::seek(self.set, target);
                self.front
            }
            None => None,
        }
    }

    /// Returns the current key and advances one step; `None` past the end.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<Key32> {
        let cur = self.front?;
        let Some(from) = cur.checked_add(1) else {
            self.front = None;
            self.walk = None;
            return Some(cur);
        };
        let set = self.set;
        let walk = self.walk.get_or_insert_with(|| {
            crate::trie32::RawIter32::new(set.arena(), set.root_edge(), 4, from)
        });
        self.front = walk.next().map(|(k, _)| k);
        Some(cur)
    }
}

/// A stateful, forward-only ordered cursor over an [`ExpanseMap32`], yielding
/// `(key, value)`. See [`SetCursor32`] and the [module docs](self).
pub struct MapCursor32<'a> {
    map: &'a ExpanseMap32,
    front: Option<(Key32, Value32)>,
    /// The held descent path, positioned to yield the entry after `front`.
    /// Built on the first step and dropped by a skip, which re-seeks.
    walk: Option<crate::trie32::RawIter32<'a>>,
}

impl<'a> MapCursor32<'a> {
    #[inline]
    pub(crate) fn new(map: &'a ExpanseMap32, front: Option<(Key32, Value32)>) -> Self {
        Self {
            map,
            front,
            walk: None,
        }
    }

    #[inline]
    fn seek(map: &ExpanseMap32, bound: Key32) -> Option<(Key32, Value32)> {
        match bound.checked_sub(1) {
            Some(b) => map.next(b),
            None => map.first(),
        }
    }

    /// The `(key, value)` at the cursor's current position, or `None`.
    #[inline]
    #[must_use]
    pub fn current(&self) -> Option<(Key32, Value32)> {
        self.front
    }

    /// Advances to and returns the entry with the smallest key `>= target` that
    /// is `>=` the cursor's current position; `None` once exhausted.
    #[inline]
    pub fn advance_to(&mut self, target: Key32) -> Option<(Key32, Value32)> {
        match self.front {
            Some((k, _)) if k >= target => self.front,
            Some(_) => {
                // A skip abandons the held path: it may land anywhere at or
                // after the target, so the path is re-seeked from the root.
                self.walk = None;
                self.front = Self::seek(self.map, target);
                self.front
            }
            None => None,
        }
    }

    /// Returns the current entry and advances one step; `None` past the end.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<(Key32, Value32)> {
        let cur = self.front?;
        let Some(from) = cur.0.checked_add(1) else {
            self.front = None;
            self.walk = None;
            return Some(cur);
        };
        let map = self.map;
        let walk = self.walk.get_or_insert_with(|| {
            crate::trie32::RawIter32::new(map.arena(), map.root_edge(), 4, from)
        });
        self.front = walk.next();
        Some(cur)
    }
}

impl ExpanseSet32 {
    /// Creates a stateful forward [`SetCursor32`] positioned before the first
    /// key. See [`crate::cursor32`] for the 32-bit skip-scan contract (correct,
    /// but not path-reusing).
    #[inline]
    #[must_use]
    pub fn cursor(&self) -> SetCursor32<'_> {
        SetCursor32::new(self, self.first())
    }

    /// Creates a [`SetCursor32`] positioned at the smallest key `>= start`.
    #[inline]
    #[must_use]
    pub fn cursor_from(&self, start: Key32) -> SetCursor32<'_> {
        let front = SetCursor32::seek(self, start);
        SetCursor32::new(self, front)
    }
}

impl ExpanseMap32 {
    /// Creates a stateful forward [`MapCursor32`] positioned before the first
    /// entry. See [`crate::cursor32`] for the 32-bit skip-scan contract.
    #[inline]
    #[must_use]
    pub fn cursor(&self) -> MapCursor32<'_> {
        MapCursor32::new(self, self.first())
    }

    /// Creates a [`MapCursor32`] positioned at the smallest key `>= start`.
    #[inline]
    #[must_use]
    pub fn cursor_from(&self, start: Key32) -> MapCursor32<'_> {
        let front = MapCursor32::seek(self, start);
        MapCursor32::new(self, front)
    }
}

#[cfg(test)]
mod tests {
    use crate::map32::ExpanseMap32;
    use crate::set32::ExpanseSet32;
    use crate::types32::Key32;
    use std::collections::{BTreeMap, BTreeSet};

    fn check(keys: &[Key32]) {
        let mut set = ExpanseSet32::new();
        let mut map = ExpanseMap32::new();
        let mut model = BTreeSet::new();
        let mut bmap = BTreeMap::new();
        for &k in keys {
            let v = k.wrapping_mul(2654435761) ^ 0x5555;
            set.insert(k);
            map.insert(k, v);
            model.insert(k);
            bmap.insert(k, v);
        }
        let sorted: Vec<Key32> = model.iter().copied().collect();

        // Probe stream: each key ±1 and extremes.
        let mut pr = vec![0u32, 1, u32::MAX, u32::MAX - 1];
        for &k in &sorted {
            pr.push(k);
            pr.push(k.saturating_sub(1));
            pr.push(k.saturating_add(1));
        }
        pr.sort_unstable();
        pr.dedup();

        for &start in &[
            None,
            sorted.first().copied(),
            sorted.last().copied(),
            Some(0),
        ] {
            let mut idx = match start {
                Some(s) => sorted.partition_point(|&k| k < s),
                None => 0,
            };
            let (mut sc, mut mc) = match start {
                Some(s) => (set.cursor_from(s), map.cursor_from(s)),
                None => (set.cursor(), map.cursor()),
            };
            assert_eq!(sc.current(), sorted.get(idx).copied());
            assert_eq!(mc.current().map(|(k, _)| k), sorted.get(idx).copied());
            for (i, &t) in pr.iter().enumerate() {
                let expect = match sorted.get(idx).copied() {
                    Some(k) if k >= t => Some(k),
                    Some(_) => {
                        idx = sorted.partition_point(|&k| k < t);
                        sorted.get(idx).copied()
                    }
                    None => None,
                };
                assert_eq!(sc.advance_to(t), expect, "advance {t} @ {i}");
                let gm = mc.advance_to(t);
                assert_eq!(gm.map(|(k, _)| k), expect);
                if let Some((k, v)) = gm {
                    assert_eq!(Some(&v), bmap.get(&k));
                }
                if i % 3 == 0 {
                    let e = sorted.get(idx).copied();
                    if e.is_some() {
                        idx += 1;
                    }
                    assert_eq!(sc.next(), e);
                    assert_eq!(mc.next().map(|(k, _)| k), e);
                }
            }
        }
    }

    #[test]
    fn cursor32_matches_model() {
        check(&[]);
        check(&[42]);
        check(&[10, 20, 30]);
        check(&(0u32..256).collect::<Vec<_>>());
        check(&(0u32..500).map(|i| i * 37).collect::<Vec<_>>());
        check(&(0u32..300).map(|i| i << 20).collect::<Vec<_>>());
        check(&[
            0,
            1,
            255,
            256,
            65535,
            65536,
            1 << 24,
            u32::MAX - 1,
            u32::MAX,
        ]);
        let mut s = 0x1234_5678u32;
        let rand: Vec<Key32> = (0..2000)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                s
            })
            .collect();
        check(&rand);
    }
}
