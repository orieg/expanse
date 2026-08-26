//! Shared model code for the Redis ZSET (sorted set) engine benchmark suite
//! (`docs/benchmarks/redis_zset_engine/`, issue #330).
//!
//! Two sorted-set engines are compared head-to-head:
//!
//! * [`ExpanseZSet`] — the Expanse design. A ZSET is `member -> score` plus
//!   score-ordered iteration and rank. That is genuinely **two** access paths,
//!   so this uses **two** `ExpanseMap`s: a composite-key `order` map keyed by
//!   `(score << 32) | member` (the total order Redis uses: score first, member
//!   as the tie-break) for range/rank, and a `members` map keyed by `member`
//!   for `ZSCORE`/`ZREM`/`ZINCRBY`. The "single structure" framing in the issue
//!   is refuted for the full command surface — see `METHODOLOGY.md`. What the
//!   design *does* buy over Redis is (a) both halves are the same compact
//!   cache-conscious trie, and (b) the ordered half answers `ZRANK`/`ZCOUNT`
//!   natively via `count_below`/`count_range` with no separately-maintained
//!   span counters.
//!
//! * [`SkiplistZSet`] — the reference Redis design. A span-augmented skip list
//!   (William Pugh, 1990, plus the standard order-statistic span counters that
//!   give `O(log n)` rank) for ordering, plus a `hashbrown::HashMap` dict for
//!   `O(1)` member lookup. Clean-room from the published skip-list algorithm;
//!   no Redis source consulted. The skip list is arena-backed (index links,
//!   per-node boxed level array) which *understates* a production
//!   pointer-chasing skip list's per-node allocator overhead — the memory
//!   comparison is therefore conservative toward this baseline.
//!
//! Both engines expose the same inherent methods; each bench writes its timed
//! loop once per concrete type (no trait-object dispatch in the measured
//! region). Members and scores are modeled as `u32`; the composite key packs
//! them into the `u64` key space `ExpanseMap` uses.

#![allow(dead_code)]

use expanse_trie::map::ExpanseMap;
use hashbrown::HashMap;

/// Deterministic xorshift64 PRNG. Named seed per bench for bit-reproducibility.
pub struct XorShift64(u64);

impl XorShift64 {
    pub fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x1A2B_3C4D_5E6F_7081
        } else {
            seed
        })
    }
    #[inline]
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    /// Uniform in `[0, n)`.
    #[inline]
    pub fn below(&mut self, n: u32) -> u32 {
        (self.next() % n as u64) as u32
    }
}

/// Median of a set of samples (interleaved-round medians per rule 5).
pub fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n == 0 {
        0.0
    } else if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

/// A Fisher–Yates shuffle over `0..n` using the given RNG. Members are inserted
/// in shuffled order so no engine sees a favorable monotonic build.
pub fn shuffled_members(n: u32, rng: &mut XorShift64) -> Vec<u32> {
    let mut v: Vec<u32> = (0..n).collect();
    for i in (1..v.len()).rev() {
        let j = (rng.next() % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
    v
}

/// Pack `(score, member)` into the composite ordering key: score in the high
/// 32 bits, member in the low 32 bits. Ordering by this `u64` is exactly
/// Redis's `(score, then member)` total order.
#[inline]
pub fn composite(score: u32, member: u32) -> u64 {
    ((score as u64) << 32) | member as u64
}

#[inline]
pub fn comp_member(key: u64) -> u32 {
    key as u32
}

#[inline]
pub fn comp_score(key: u64) -> u32 {
    (key >> 32) as u32
}

// ============================================================================
// ExpanseZSet: composite-key ExpanseMap (order) + member->score ExpanseMap.
// ============================================================================

/// Expanse sorted-set engine: two `ExpanseMap`s (see module docs).
pub struct ExpanseZSet {
    /// `(score << 32) | member  ->  member` — the score-ordered structure.
    order: ExpanseMap,
    /// `member -> score` — the point-lookup structure.
    members: ExpanseMap,
}

impl Default for ExpanseZSet {
    fn default() -> Self {
        Self::new()
    }
}

impl ExpanseZSet {
    pub fn new() -> Self {
        Self {
            order: ExpanseMap::new(),
            members: ExpanseMap::new(),
        }
    }

    pub fn len(&self) -> u64 {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.len() == 0
    }

    /// `ZADD`: insert a new member, or move an existing member to a new score.
    /// A score change is a delete + insert of the composite key (the composite
    /// encoding admits no in-place score move) plus a member-map update.
    /// Returns `true` if the member was newly added.
    #[inline]
    pub fn zadd(&mut self, member: u32, score: u32) -> bool {
        match self.members.get(member as u64) {
            Some(old) => {
                let old = old as u32;
                if old != score {
                    self.order.remove(composite(old, member));
                    self.order.insert(composite(score, member), member as u64);
                    self.members.insert(member as u64, score as u64);
                }
                false
            }
            None => {
                self.members.insert(member as u64, score as u64);
                self.order.insert(composite(score, member), member as u64);
                true
            }
        }
    }

    /// `ZSCORE`: member -> score.
    #[inline]
    pub fn zscore(&self, member: u32) -> Option<u32> {
        self.members.get(member as u64).map(|s| s as u32)
    }

    /// `ZREM`: remove a member. Returns `true` if it was present.
    #[inline]
    pub fn zrem(&mut self, member: u32) -> bool {
        match self.members.remove(member as u64) {
            Some(score) => {
                self.order.remove(composite(score as u32, member));
                true
            }
            None => false,
        }
    }

    /// `ZINCRBY`: add `delta` to a member's score (clamped to the `u32` domain),
    /// inserting the member at `delta` if absent. Returns the new score.
    #[inline]
    pub fn zincrby(&mut self, member: u32, delta: i64) -> u32 {
        let cur = self.zscore(member).unwrap_or(0) as i64;
        let new = (cur + delta).clamp(0, u32::MAX as i64) as u32;
        self.zadd(member, new);
        new
    }

    /// `ZRANK`: 0-based rank of `member` in ascending `(score, member)` order.
    /// Costs one member-map lookup plus one `count_below` — both `O(depth)`.
    #[inline]
    pub fn zrank(&self, member: u32) -> Option<u64> {
        let score = self.members.get(member as u64)? as u32;
        Some(self.order.count_below(composite(score, member)))
    }

    /// `ZCOUNT`: number of members whose score is in `[min, max]`.
    #[inline]
    pub fn zcount(&self, min: u32, max: u32) -> u64 {
        if min > max {
            return 0;
        }
        self.order
            .count_range(composite(min, 0)..=composite(max, u32::MAX))
    }

    /// `ZRANGEBYSCORE` (ascending): invoke `f(member, score)` for each member
    /// with score in `[min, max]`, in ascending order. Returns the count.
    #[inline]
    pub fn zrangebyscore<F: FnMut(u32, u32)>(&self, min: u32, max: u32, mut f: F) -> usize {
        if min > max {
            return 0;
        }
        let mut n = 0;
        for (k, _) in self
            .order
            .range(composite(min, 0)..=composite(max, u32::MAX))
        {
            f(comp_member(k), comp_score(k));
            n += 1;
        }
        n
    }

    /// `ZREVRANGEBYSCORE` (descending): a single amortized descending walk via
    /// the reverse ordered iterator (`range_rev`, #341) — `O(1)` per member
    /// across a leaf. This is the production path; the emulated re-descent is
    /// retained as [`Self::zrevrangebyscore_emulated`] only so the suite can
    /// report emulated vs native vs skiplist per cell.
    #[inline]
    pub fn zrevrangebyscore<F: FnMut(u32, u32)>(&self, min: u32, max: u32, mut f: F) -> usize {
        if min > max {
            return 0;
        }
        let lo = composite(min, 0);
        let hi = composite(max, u32::MAX);
        let mut n = 0;
        for (k, _) in self.order.range_rev(lo..=hi) {
            f(comp_member(k), comp_score(k));
            n += 1;
        }
        n
    }

    /// `ZREVRANGEBYSCORE` (descending), **emulated** arm: descending walk via
    /// repeated `prev_at_or_before`, each step an `O(depth)` re-descent — the
    /// pre-#341 cost, kept as a benchmark-only comparison arm.
    #[inline]
    pub fn zrevrangebyscore_emulated<F: FnMut(u32, u32)>(
        &self,
        min: u32,
        max: u32,
        mut f: F,
    ) -> usize {
        if min > max {
            return 0;
        }
        let lo = composite(min, 0);
        let mut cursor = composite(max, u32::MAX);
        let mut n = 0;
        while let Some((k, _)) = self.order.prev_at_or_before(cursor) {
            if k < lo {
                break;
            }
            f(comp_member(k), comp_score(k));
            n += 1;
            if k == 0 {
                break;
            }
            cursor = k - 1;
        }
        n
    }

    /// `ZRANGE` by rank `[start, stop]` inclusive, 0-based, ascending. Seeds the
    /// cursor with one `by_count` (select) then streams forward.
    #[inline]
    pub fn zrange_by_rank<F: FnMut(u32, u32)>(&self, start: u64, stop: u64, mut f: F) -> usize {
        let len = self.order.len();
        if len == 0 || start >= len || start > stop {
            return 0;
        }
        let stop = stop.min(len - 1);
        let Some((first, _)) = self.order.by_count(start) else {
            return 0;
        };
        let mut n = 0;
        for (k, _) in self.order.range(first..=u64::MAX) {
            f(comp_member(k), comp_score(k));
            n += 1;
            if start + n as u64 > stop {
                break;
            }
        }
        n
    }
}

// ============================================================================
// SkiplistZSet: span-augmented skip list + hashbrown dict (Redis reference).
// ============================================================================

const SKIPLIST_MAX_LEVEL: usize = 32;
/// `P = 1/4`, expressed as a 16-bit threshold for the xorshift level roll.
const SKIPLIST_P_THRESHOLD: u64 = 0xFFFF / 4;
const NIL: u32 = u32::MAX;

#[derive(Clone, Copy)]
struct Lvl {
    /// Index of the next node at this level, or [`NIL`].
    forward: u32,
    /// Number of nodes (at level 0) between this node and `forward`, inclusive
    /// of `forward`. The order-statistic augmentation that makes rank `O(log n)`.
    span: u32,
}

struct SkipNode {
    key: u64,
    /// Previous node at level 0, or [`NIL`] (used for descending range walks).
    backward: u32,
    levels: Box<[Lvl]>,
}

/// A skip list keyed by the composite `(score, member)` `u64`, augmented with
/// per-level spans for `O(log n)` rank. Arena-backed with a free list.
struct SpanSkipList {
    nodes: Vec<SkipNode>,
    free: Vec<u32>,
    /// Highest level currently in use (1-based count of active levels).
    level: usize,
    /// Number of real nodes (excludes the head sentinel at index 0).
    length: u64,
    rng: XorShift64,
}

impl SpanSkipList {
    fn new(seed: u64) -> Self {
        // Node 0 is the head sentinel: full height, key unused.
        let head = SkipNode {
            key: 0,
            backward: NIL,
            levels: vec![
                Lvl {
                    forward: NIL,
                    span: 0
                };
                SKIPLIST_MAX_LEVEL
            ]
            .into_boxed_slice(),
        };
        Self {
            nodes: vec![head],
            free: Vec::new(),
            level: 1,
            length: 0,
            rng: XorShift64::new(seed),
        }
    }

    #[inline]
    fn key_of(&self, idx: u32) -> u64 {
        self.nodes[idx as usize].key
    }
    #[inline]
    fn forward(&self, idx: u32, i: usize) -> u32 {
        self.nodes[idx as usize].levels[i].forward
    }
    #[inline]
    fn span(&self, idx: u32, i: usize) -> u32 {
        self.nodes[idx as usize].levels[i].span
    }

    #[inline]
    fn random_level(&mut self) -> usize {
        let mut lvl = 1;
        while lvl < SKIPLIST_MAX_LEVEL && (self.rng.next() & 0xFFFF) < SKIPLIST_P_THRESHOLD {
            lvl += 1;
        }
        lvl
    }

    fn alloc(&mut self, key: u64, height: usize) -> u32 {
        let levels = vec![
            Lvl {
                forward: NIL,
                span: 0
            };
            height
        ]
        .into_boxed_slice();
        if let Some(idx) = self.free.pop() {
            let n = &mut self.nodes[idx as usize];
            n.key = key;
            n.backward = NIL;
            n.levels = levels;
            idx
        } else {
            let idx = self.nodes.len() as u32;
            self.nodes.push(SkipNode {
                key,
                backward: NIL,
                levels,
            });
            idx
        }
    }

    /// Insert `key` (assumed absent). Returns the new node index.
    #[allow(clippy::needless_range_loop)]
    fn insert(&mut self, key: u64) -> u32 {
        let mut update = [0u32; SKIPLIST_MAX_LEVEL];
        let mut rank = [0u32; SKIPLIST_MAX_LEVEL];
        let mut x = 0u32; // head
        for i in (0..self.level).rev() {
            rank[i] = if i == self.level - 1 { 0 } else { rank[i + 1] };
            while self.forward(x, i) != NIL && self.key_of(self.forward(x, i)) < key {
                rank[i] += self.span(x, i);
                x = self.forward(x, i);
            }
            update[i] = x;
        }

        let lvl = self.random_level();
        if lvl > self.level {
            for (i, up) in update.iter_mut().enumerate().take(lvl).skip(self.level) {
                rank[i] = 0;
                *up = 0; // head
                self.nodes[0].levels[i].span = self.length as u32;
            }
            self.level = lvl;
        }

        let new = self.alloc(key, lvl);
        for i in 0..lvl {
            let up = update[i];
            let up_fwd = self.forward(up, i);
            let up_span = self.span(up, i);
            self.nodes[new as usize].levels[i].forward = up_fwd;
            self.nodes[up as usize].levels[i].forward = new;
            let crossed = rank[0] - rank[i];
            self.nodes[new as usize].levels[i].span = up_span - crossed;
            self.nodes[up as usize].levels[i].span = crossed + 1;
        }
        // Higher levels of `update` that the new node does not reach: +1 span.
        for i in lvl..self.level {
            let up = update[i];
            self.nodes[up as usize].levels[i].span += 1;
        }

        // backward pointer at level 0
        let bw = update[0];
        self.nodes[new as usize].backward = if bw == 0 { NIL } else { bw };
        let fwd0 = self.forward(new, 0);
        if fwd0 != NIL {
            self.nodes[fwd0 as usize].backward = new;
        }
        self.length += 1;
        new
    }

    /// Delete `key` if present. Returns `true` if a node was removed.
    #[allow(clippy::needless_range_loop)]
    fn delete(&mut self, key: u64) -> bool {
        let mut update = [0u32; SKIPLIST_MAX_LEVEL];
        let mut x = 0u32;
        for i in (0..self.level).rev() {
            while self.forward(x, i) != NIL && self.key_of(self.forward(x, i)) < key {
                x = self.forward(x, i);
            }
            update[i] = x;
        }
        let target = self.forward(x, 0);
        if target == NIL || self.key_of(target) != key {
            return false;
        }
        for i in 0..self.level {
            let up = update[i];
            if self.forward(up, i) == target {
                let t_span = self.span(target, i);
                self.nodes[up as usize].levels[i].span += t_span.wrapping_sub(1);
                let t_fwd = self.forward(target, i);
                self.nodes[up as usize].levels[i].forward = t_fwd;
            } else {
                self.nodes[up as usize].levels[i].span -= 1;
            }
        }
        let t_fwd0 = self.forward(target, 0);
        let t_back = self.nodes[target as usize].backward;
        if t_fwd0 != NIL {
            self.nodes[t_fwd0 as usize].backward = t_back;
        }
        while self.level > 1 && self.forward(0, self.level - 1) == NIL {
            self.level -= 1;
        }
        // Release the level array and recycle the slot.
        self.nodes[target as usize].levels = Box::new([]);
        self.free.push(target);
        self.length -= 1;
        true
    }

    /// Number of nodes with key strictly less than `key` (0-based rank).
    fn rank_lt(&self, key: u64) -> u64 {
        let mut x = 0u32;
        let mut rank = 0u64;
        for i in (0..self.level).rev() {
            while self.forward(x, i) != NIL && self.key_of(self.forward(x, i)) < key {
                rank += self.span(x, i) as u64;
                x = self.forward(x, i);
            }
        }
        rank
    }

    /// Number of nodes with key `<= key`.
    fn rank_le(&self, key: u64) -> u64 {
        let mut x = 0u32;
        let mut rank = 0u64;
        for i in (0..self.level).rev() {
            while self.forward(x, i) != NIL && self.key_of(self.forward(x, i)) <= key {
                rank += self.span(x, i) as u64;
                x = self.forward(x, i);
            }
        }
        rank
    }

    /// First node with key `>= key`, or [`NIL`].
    fn first_ge(&self, key: u64) -> u32 {
        let mut x = 0u32;
        for i in (0..self.level).rev() {
            while self.forward(x, i) != NIL && self.key_of(self.forward(x, i)) < key {
                x = self.forward(x, i);
            }
        }
        self.forward(x, 0)
    }

    /// Last node with key `<= key`, or [`NIL`] (head).
    fn last_le(&self, key: u64) -> u32 {
        let mut x = 0u32;
        for i in (0..self.level).rev() {
            while self.forward(x, i) != NIL && self.key_of(self.forward(x, i)) <= key {
                x = self.forward(x, i);
            }
        }
        if x == 0 { NIL } else { x }
    }

    /// Node at 0-based rank `n`, or [`NIL`]. `O(log n)` via span walk.
    fn by_rank(&self, n: u64) -> u32 {
        if n >= self.length {
            return NIL;
        }
        let target = n + 1; // 1-based traversed count
        let mut x = 0u32;
        let mut traversed = 0u64;
        for i in (0..self.level).rev() {
            while self.forward(x, i) != NIL && traversed + self.span(x, i) as u64 <= target {
                traversed += self.span(x, i) as u64;
                x = self.forward(x, i);
            }
            if traversed == target {
                return x;
            }
        }
        NIL
    }
}

/// Redis-style sorted-set engine: span skip list + `member -> score` dict.
pub struct SkiplistZSet {
    list: SpanSkipList,
    dict: HashMap<u32, u32>,
}

impl SkiplistZSet {
    pub fn new(seed: u64) -> Self {
        Self {
            list: SpanSkipList::new(seed),
            dict: HashMap::new(),
        }
    }

    pub fn len(&self) -> u64 {
        self.dict.len() as u64
    }

    pub fn is_empty(&self) -> bool {
        self.dict.is_empty()
    }

    /// `ZADD`: insert or move a member. Score change = skip-list delete + insert
    /// plus a dict update (mirrors Redis's `zsetAdd`). Returns `true` if newly
    /// added.
    #[inline]
    pub fn zadd(&mut self, member: u32, score: u32) -> bool {
        match self.dict.get(&member).copied() {
            Some(old) => {
                if old != score {
                    self.list.delete(composite(old, member));
                    self.list.insert(composite(score, member));
                    self.dict.insert(member, score);
                }
                false
            }
            None => {
                self.dict.insert(member, score);
                self.list.insert(composite(score, member));
                true
            }
        }
    }

    /// `ZSCORE`: O(1) dict lookup — the skip list is not touched.
    #[inline]
    pub fn zscore(&self, member: u32) -> Option<u32> {
        self.dict.get(&member).copied()
    }

    /// `ZREM`: remove a member. Returns `true` if it was present.
    #[inline]
    pub fn zrem(&mut self, member: u32) -> bool {
        match self.dict.remove(&member) {
            Some(score) => {
                self.list.delete(composite(score, member));
                true
            }
            None => false,
        }
    }

    #[inline]
    pub fn zincrby(&mut self, member: u32, delta: i64) -> u32 {
        let cur = self.zscore(member).unwrap_or(0) as i64;
        let new = (cur + delta).clamp(0, u32::MAX as i64) as u32;
        self.zadd(member, new);
        new
    }

    /// `ZRANK`: dict lookup for the score, then `O(log n)` span rank.
    #[inline]
    pub fn zrank(&self, member: u32) -> Option<u64> {
        let score = *self.dict.get(&member)?;
        Some(self.list.rank_lt(composite(score, member)))
    }

    /// `ZCOUNT`: two `O(log n)` span ranks.
    #[inline]
    pub fn zcount(&self, min: u32, max: u32) -> u64 {
        if min > max {
            return 0;
        }
        self.list.rank_le(composite(max, u32::MAX)) - self.list.rank_lt(composite(min, 0))
    }

    /// `ZRANGEBYSCORE` (ascending): find first `>= min`, follow level-0 forward.
    #[inline]
    pub fn zrangebyscore<F: FnMut(u32, u32)>(&self, min: u32, max: u32, mut f: F) -> usize {
        if min > max {
            return 0;
        }
        let hi = composite(max, u32::MAX);
        let mut cur = self.list.first_ge(composite(min, 0));
        let mut n = 0;
        while cur != NIL {
            let k = self.list.key_of(cur);
            if k > hi {
                break;
            }
            f(comp_member(k), comp_score(k));
            n += 1;
            cur = self.list.forward(cur, 0);
        }
        n
    }

    /// `ZREVRANGEBYSCORE` (descending): find last `<= max`, follow backward.
    /// `O(k)` — the skip list's level-0 backward pointers make this cheap, the
    /// cell where the reference is expected to beat Expanse.
    #[inline]
    pub fn zrevrangebyscore<F: FnMut(u32, u32)>(&self, min: u32, max: u32, mut f: F) -> usize {
        if min > max {
            return 0;
        }
        let lo = composite(min, 0);
        let mut cur = self.list.last_le(composite(max, u32::MAX));
        let mut n = 0;
        while cur != NIL {
            let k = self.list.key_of(cur);
            if k < lo {
                break;
            }
            f(comp_member(k), comp_score(k));
            n += 1;
            cur = self.list.nodes[cur as usize].backward;
        }
        n
    }

    /// `ZRANGE` by rank `[start, stop]` inclusive, 0-based, ascending.
    #[inline]
    pub fn zrange_by_rank<F: FnMut(u32, u32)>(&self, start: u64, stop: u64, mut f: F) -> usize {
        let len = self.list.length;
        if len == 0 || start >= len || start > stop {
            return 0;
        }
        let stop = stop.min(len - 1);
        let mut cur = self.list.by_rank(start);
        let mut n = 0u64;
        while cur != NIL && start + n <= stop {
            let k = self.list.key_of(cur);
            f(comp_member(k), comp_score(k));
            n += 1;
            cur = self.list.forward(cur, 0);
        }
        n as usize
    }
}

// ============================================================================
// Correctness cross-check.
// ============================================================================

/// Build a small randomized sorted set in both engines and a brute-force
/// `BTreeSet` oracle, then assert `zrank`/`zscore`/`zcount`/`zrangebyscore`/
/// `zrevrangebyscore`/`zrange_by_rank` agree across all three. Called once at
/// the start of every pillar bench so a broken span-counter or composite
/// encoding fails loudly instead of publishing wrong numbers.
pub fn validate() {
    use std::collections::{BTreeSet, HashMap as StdMap};

    let mut rng = XorShift64::new(0x5A11_DA7E_C0DE_0001);
    let mut exp = ExpanseZSet::new();
    let mut sl = SkiplistZSet::new(0xABCD);
    let mut oracle: BTreeSet<u64> = BTreeSet::new();
    let mut omap: StdMap<u32, u32> = StdMap::new();

    // Adds with in-place score updates.
    for _ in 0..3000 {
        let m = rng.below(600);
        let s = rng.below(60);
        if let Some(&old) = omap.get(&m) {
            oracle.remove(&composite(old, m));
        }
        omap.insert(m, s);
        oracle.insert(composite(s, m));
        exp.zadd(m, s);
        sl.zadd(m, s);
    }
    // Removals.
    for m in 0..300u32 {
        if let Some(&old) = omap.get(&m) {
            oracle.remove(&composite(old, m));
            omap.remove(&m);
        }
        assert_eq!(exp.zrem(m), sl.zrem(m));
    }

    assert_eq!(exp.len(), oracle.len() as u64, "expanse len");
    assert_eq!(sl.len(), oracle.len() as u64, "skiplist len");

    let ordered: Vec<u64> = oracle.iter().copied().collect();
    for (rank, &k) in ordered.iter().enumerate() {
        let m = comp_member(k);
        let s = comp_score(k);
        assert_eq!(exp.zrank(m), Some(rank as u64), "expanse zrank m={m}");
        assert_eq!(sl.zrank(m), Some(rank as u64), "skiplist zrank m={m}");
        assert_eq!(exp.zscore(m), Some(s), "expanse zscore m={m}");
        assert_eq!(sl.zscore(m), Some(s), "skiplist zscore m={m}");
    }

    for _ in 0..300 {
        let a = rng.below(60);
        let b = rng.below(60);
        let (lo, hi) = (a.min(b), a.max(b));
        let want = ordered
            .iter()
            .filter(|&&k| {
                let s = comp_score(k);
                s >= lo && s <= hi
            })
            .count() as u64;
        assert_eq!(exp.zcount(lo, hi), want, "expanse zcount [{lo},{hi}]");
        assert_eq!(sl.zcount(lo, hi), want, "skiplist zcount [{lo},{hi}]");
    }

    let (lo, hi) = (12u32, 34u32);
    let want_fwd: Vec<(u32, u32)> = ordered
        .iter()
        .filter(|&&k| {
            let s = comp_score(k);
            s >= lo && s <= hi
        })
        .map(|&k| (comp_score(k), comp_member(k)))
        .collect();

    let mut e = Vec::new();
    exp.zrangebyscore(lo, hi, |m, s| e.push((s, m)));
    let mut s2 = Vec::new();
    sl.zrangebyscore(lo, hi, |m, s| s2.push((s, m)));
    assert_eq!(e, want_fwd, "expanse zrangebyscore");
    assert_eq!(s2, want_fwd, "skiplist zrangebyscore");

    let mut want_rev = want_fwd.clone();
    want_rev.reverse();
    let mut e = Vec::new();
    exp.zrevrangebyscore(lo, hi, |m, s| e.push((s, m)));
    let mut s2 = Vec::new();
    sl.zrevrangebyscore(lo, hi, |m, s| s2.push((s, m)));
    assert_eq!(e, want_rev, "expanse zrevrangebyscore (native)");
    assert_eq!(s2, want_rev, "skiplist zrevrangebyscore");
    let mut e_emul = Vec::new();
    exp.zrevrangebyscore_emulated(lo, hi, |m, s| e_emul.push((s, m)));
    assert_eq!(e_emul, want_rev, "expanse zrevrangebyscore (emulated)");

    let want_rank: Vec<(u32, u32)> = ordered
        .iter()
        .skip(7)
        .take(40)
        .map(|&k| (comp_score(k), comp_member(k)))
        .collect();
    let mut e = Vec::new();
    exp.zrange_by_rank(7, 46, |m, s| e.push((s, m)));
    let mut s2 = Vec::new();
    sl.zrange_by_rank(7, 46, |m, s| s2.push((s, m)));
    assert_eq!(e, want_rank, "expanse zrange_by_rank");
    assert_eq!(s2, want_rank, "skiplist zrange_by_rank");
}
