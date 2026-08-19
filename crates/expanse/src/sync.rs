//! Phase 7: concurrent wrappers — one writer, many lock-free readers.
//!
//! [`SyncExpanseSet`] / [`SyncExpanseMap`] wrap the single-threaded trees
//! with the `occ` protocol:
//!
//! - **Writers** serialize on a mutex; the tree-level [`SeqVersion`]
//!   brackets each operation (covering the root state), and the engine
//!   brackets every branch node's in-place mutation region with that
//!   node's own version (`occ::version_begin_if` — active only for
//!   concurrently shared trees). All node frees route through the epoch
//!   [`Collector`] (the tree's `NodeAlloc` is switched to deferred
//!   reclamation at construction), so a reader never dereferences freed
//!   memory.
//! - **Readers** pin the current epoch, validate the root snapshot
//!   against the tree version, then walk **hand-over-hand**: each branch
//!   node is sampled even before its fields are read and re-validated
//!   before anything loaded from it is dereferenced; terminal payloads
//!   are validated against their parent's version. Any failure restarts
//!   the walk; after a bounded number of restarts the reader falls back
//!   to the writer mutex (guaranteed progress under a write storm).
//!
//! ## Memory-model caveat (deliberate, documented)
//!
//! Between `sample` and a failed `validate`, a reader may perform plain
//! loads that race with the writer's plain stores — the classic seqlock
//! pattern (Linux kernel seqlocks; Judy's own published OCC design).
//! Those racy loads are never *used*: every value is discarded unless the
//! subsequent validation proves no writer overlapped. This is undefined
//! behavior under a strict reading of the C++/Rust memory model (hence
//! not Miri/loom-checkable end-to-end; loom covers the `occ` protocol
//! pieces, and the thread stress tests cover the whole); it is the
//! industry-standard trade until Rust grows blessed tearable atomics.
//! The per-node version slots reserved in the node headers are the
//! planned contention refinement, not a correctness requirement.

use crate::alloc::NodeAlloc;
use crate::leaf;
use crate::map::ExpanseMap;
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1, LeafBitmapL};
use crate::occ::{Collector, Reader, SeqVersion};
use crate::set::ExpanseSet;
use crate::types::{EdgeTag, EdgeType, Key, digit};
use core::cell::UnsafeCell;
use std::sync::{Arc, Mutex, MutexGuard};

/// By-value snapshot of a tree's root state (possibly torn — the reader
/// validates before acting on it).
#[derive(Clone, Copy)]
pub(crate) enum RootSnapshot {
    /// No keys.
    Empty,
    /// Root leaf: `pop` entries at `ptr` (set: sorted `u64` keys; map:
    /// keys then values).
    Leaf {
        /// Base of the root-leaf allocation.
        ptr: *const u8,
        /// Entry count.
        pop: usize,
    },
    /// A level-8 trie.
    Tree {
        /// The top edge (by value).
        top: Edge,
        /// Total population.
        pop: u64,
    },
}

/// A validated read failed because the version moved; restart.
struct Retry;

/// Bounded optimistic restarts before falling back to the writer lock.
const MAX_RETRIES: usize = 64;

/// The reader's cover for the bytes it just loaded: the tree-level
/// version for the root state, then — hand-over-hand — the version of
/// the node each subsequent edge was loaded from (Phase 7 per-node OCC:
/// the writer brackets every node's in-place mutations, child slots and
/// the recursion beneath them included, with that node's version).
enum Cover<'a> {
    Tree(&'a SeqVersion, u64),
    Node(*const u32, u32),
}

impl Cover<'_> {
    #[inline]
    fn ok(&self) -> bool {
        match self {
            Cover::Tree(v, s) => v.validate(*s),
            // SAFETY: the node whose version this is stays EBR-live for
            // the duration of the reader's pin.
            Cover::Node(p, s) => unsafe { crate::occ::node_validate(*p, *s) },
        }
    }
}

/// One validated step-by-step lookup over a (possibly mutating) tree.
///
/// Hand-over-hand: the root snapshot is validated against the tree
/// version; each branch node is then read under its own version (sampled
/// even before the reads, re-validated after), which also covers the
/// terminal payloads of its children. Any failure restarts the walk.
///
/// # Safety
///
/// `snap` must be an even version sampled from `ver` after the tree's
/// `NodeAlloc` switched to deferred reclamation, and the caller must hold
/// an epoch pin for the whole call: every pointer loaded under a
/// still-valid cover then references EBR-live memory.
unsafe fn walk_validated<const MAP: bool>(
    root: RootSnapshot,
    key: Key,
    ver: &SeqVersion,
    snap: u64,
) -> Result<Option<u64>, Retry> {
    let mut cover = Cover::Tree(ver, snap);
    macro_rules! chk {
        () => {
            if !cover.ok() {
                return Err(Retry);
            }
        };
    }
    // The root snapshot itself was copied before the first validation.
    chk!();
    let (mut edge, mut level): (Edge, u8) = match root {
        RootSnapshot::Empty => return Ok(None),
        RootSnapshot::Leaf { ptr, pop } => {
            // Root leaf: `pop` sorted u64 keys at the base (map: values
            // follow the keys). Covered by the tree version throughout.
            let keys = ptr.cast::<u64>();
            let (mut lo, mut hi) = (0usize, pop);
            while lo < hi {
                let mid = (lo + hi) / 2;
                // SAFETY: `mid < pop` and the allocation is EBR-live;
                // the loaded value is validated before use.
                let k = unsafe { keys.add(mid).read() };
                chk!();
                if k < key {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            if lo >= pop {
                return Ok(None);
            }
            // SAFETY: in-bounds read of the EBR-live root leaf.
            let found = unsafe { keys.add(lo).read() } == key;
            chk!();
            if !found {
                return Ok(None);
            }
            if !MAP {
                return Ok(Some(0));
            }
            // SAFETY: map root leaves store `pop` values behind the keys.
            let v = unsafe { keys.add(pop + lo).read() };
            chk!();
            return Ok(Some(v));
        }
        RootSnapshot::Tree { top, .. } => (top, 8),
    };

    loop {
        let Some(tag) = edge.tag() else {
            // A torn edge can hold any byte; only a concurrent mutation
            // produces an invalid tag.
            return Err(Retry);
        };
        match tag {
            EdgeTag::Structural(t) => match t {
                EdgeType::Null => {
                    chk!();
                    return Ok(None);
                }

                EdgeType::BranchL3 | EdgeType::BranchL7 => {
                    // The edge copy was validated when it was loaded; its
                    // node pointer is EBR-live.
                    let node = edge.node_ptr();
                    let is_l3 = matches!(t, EdgeType::BranchL3);
                    let vp: *const u32 = if is_l3 {
                        // SAFETY: EBR-live node; field projection only.
                        unsafe { &raw const (*node.cast::<BranchL3>()).hdr.version }
                    } else {
                        // SAFETY: as above.
                        unsafe { &raw const (*node.cast::<BranchL7>()).hdr.version }
                    };
                    // SAFETY: live version field (EBR).
                    let Some(nsnap) = (unsafe { crate::occ::node_sample(vp) }) else {
                        return Err(Retry);
                    };
                    // SAFETY: EBR-live branch node; loads validated below.
                    let (bl, num, digits, edges_base) = unsafe {
                        if is_l3 {
                            let b = node.cast::<BranchL3>();
                            (
                                (*b).hdr.level,
                                (*b).hdr.num as usize,
                                (*b).hdr.digits,
                                (*b).edges.as_ptr(),
                            )
                        } else {
                            let b = node.cast::<BranchL7>();
                            (
                                (*b).hdr.level,
                                (*b).hdr.num as usize,
                                (*b).hdr.digits,
                                (*b).edges.as_ptr(),
                            )
                        }
                    };
                    if !(2..=level).contains(&bl) || num > if is_l3 { 3 } else { 7 } {
                        return Err(Retry);
                    }
                    if bl < level && !crate::get::decode_matches(&edge, key, bl, level) {
                        // SAFETY: live version field (EBR).
                        if !unsafe { crate::occ::node_validate(vp, nsnap) } {
                            return Err(Retry);
                        }
                        return Ok(None);
                    }
                    let d = digit(key, bl);
                    let Some(slot) = digits[..num].iter().position(|&x| x == d) else {
                        // SAFETY: live version field (EBR).
                        if !unsafe { crate::occ::node_validate(vp, nsnap) } {
                            return Err(Retry);
                        }
                        return Ok(None);
                    };
                    // SAFETY: `slot < num <= capacity`; in-bounds read of
                    // the EBR-live node, validated just below.
                    edge = unsafe { edges_base.add(slot).read() };
                    // SAFETY: live version field (EBR).
                    if !unsafe { crate::occ::node_validate(vp, nsnap) } {
                        return Err(Retry);
                    }
                    cover = Cover::Node(vp, nsnap);
                    level = bl - 1;
                }

                EdgeType::BranchB => {
                    let node = edge.node_ptr().cast::<BranchB>();
                    // SAFETY: EBR-live node; field projection only.
                    let vp: *const u32 = unsafe { &raw const (*node).version };
                    // SAFETY: live version field (EBR).
                    let Some(nsnap) = (unsafe { crate::occ::node_sample(vp) }) else {
                        return Err(Retry);
                    };
                    // SAFETY: EBR-live BranchB; loads validated below.
                    let (bl, bit, rank, sub) = unsafe {
                        let bl = (*node).level;
                        if !(2..=level).contains(&bl) {
                            return Err(Retry);
                        }
                        let d = digit(key, bl);
                        (
                            bl,
                            (*node).bitmap.test(d),
                            (*node).bitmap.subexpanse_rank(d) as usize,
                            (*node).subarrays[(d >> 5) as usize],
                        )
                    };
                    if bl < level && !crate::get::decode_matches(&edge, key, bl, level) {
                        // SAFETY: live version field (EBR).
                        if !unsafe { crate::occ::node_validate(vp, nsnap) } {
                            return Err(Retry);
                        }
                        return Ok(None);
                    }
                    if !bit {
                        // SAFETY: live version field (EBR).
                        if !unsafe { crate::occ::node_validate(vp, nsnap) } {
                            return Err(Retry);
                        }
                        return Ok(None);
                    }
                    if sub.is_null() {
                        return Err(Retry);
                    }
                    // SAFETY: bit set + validated below → subarray holds
                    // at least `rank + 1` EBR-live edges.
                    edge = unsafe { sub.add(rank).read() };
                    // SAFETY: live version field (EBR).
                    if !unsafe { crate::occ::node_validate(vp, nsnap) } {
                        return Err(Retry);
                    }
                    cover = Cover::Node(vp, nsnap);
                    level = bl - 1;
                }

                EdgeType::BranchU => {
                    let node = edge.node_ptr().cast::<BranchU>();
                    // SAFETY: EBR-live node; field projection only.
                    let vp: *const u32 = unsafe { &raw const (*node).version };
                    // SAFETY: live version field (EBR).
                    let Some(nsnap) = (unsafe { crate::occ::node_sample(vp) }) else {
                        return Err(Retry);
                    };
                    let d = digit(key, level);
                    // SAFETY: EBR-live BranchU; direct 256-slot index.
                    edge = unsafe { (*node).edges.as_ptr().add(d as usize).read() };
                    // SAFETY: live version field (EBR).
                    if !unsafe { crate::occ::node_validate(vp, nsnap) } {
                        return Err(Retry);
                    }
                    cover = Cover::Node(vp, nsnap);
                    level -= 1;
                }

                EdgeType::LeafB1 => {
                    if level > 1 && !crate::get::decode_matches(&edge, key, 1, level) {
                        chk!();
                        return Ok(None);
                    }
                    let d = digit(key, 1);
                    if MAP {
                        let node = edge.node_ptr().cast::<LeafBitmapL>();
                        // SAFETY: EBR-live LeafBitmapL; loads validated
                        // (against the parent's cover) before the value
                        // subarray is dereferenced.
                        let (bit, rank, vals) = unsafe {
                            (
                                (*node).bitmap.test(d),
                                (*node).bitmap.subexpanse_rank(d) as usize,
                                (*node).values[(d >> 5) as usize],
                            )
                        };
                        chk!();
                        if !bit {
                            return Ok(None);
                        }
                        if vals.is_null() {
                            return Err(Retry);
                        }
                        // SAFETY: bit set + validated → `rank + 1` values.
                        let v = unsafe { vals.add(rank).read() };
                        chk!();
                        return Ok(Some(v));
                    }
                    let node = edge.node_ptr().cast::<LeafBitmap1>();
                    // SAFETY: EBR-live LeafBitmap1.
                    let bit = unsafe { (*node).bitmap.test(d) };
                    chk!();
                    return Ok(bit.then_some(0));
                }

                EdgeType::Leaf1
                | EdgeType::Leaf2
                | EdgeType::Leaf3
                | EdgeType::Leaf4
                | EdgeType::Leaf5
                | EdgeType::Leaf6
                | EdgeType::Leaf7 => {
                    let kb = t.leaf_key_bytes().expect("linear-leaf tag");
                    if kb > level {
                        return Err(Retry);
                    }
                    if !crate::get::decode_matches(&edge, key, kb, level) {
                        chk!();
                        return Ok(None);
                    }
                    let pop = edge.pop0(kb) as usize + 1;
                    let base = edge.node_ptr();
                    let keys = if MAP {
                        base.wrapping_add(leaf::map_keys_offset(pop))
                    } else {
                        base
                    };
                    // SAFETY: EBR-live leaf of (validated) `pop` keys;
                    // the slot is validated before the value read.
                    let found = unsafe { leaf::search(keys, pop, kb, key) };
                    chk!();
                    let Some(slot) = found else {
                        return Ok(None);
                    };
                    if !MAP {
                        return Ok(Some(0));
                    }
                    // SAFETY: `slot < pop` values at the leaf base.
                    let v = unsafe { base.cast::<u64>().add(slot).read() };
                    chk!();
                    return Ok(Some(v));
                }

                EdgeType::FullExpanse => {
                    if MAP {
                        return Err(Retry);
                    }
                    chk!();
                    return Ok(Some(0));
                }
            },

            EdgeTag::Immed(im) => {
                if im.key_bytes() != level {
                    return Err(Retry);
                }
                let kb = im.key_bytes() as usize;
                let n = im.key_count() as usize;
                let needle = &key.to_le_bytes()[..kb];
                let payload: [u8; 15] = if MAP {
                    let mut p = [0u8; 15];
                    p[..7].copy_from_slice(edge.aux_bytes());
                    p
                } else {
                    edge.imm_payload()
                };
                let mut slot = None;
                for i in 0..n {
                    if &payload[i * kb..(i + 1) * kb] == needle {
                        slot = Some(i);
                        break;
                    }
                }
                chk!();
                let Some(slot) = slot else {
                    return Ok(None);
                };
                if !MAP {
                    return Ok(Some(0));
                }
                if n == 1 {
                    return Ok(Some(u64::from_le_bytes(edge.imm_bytes())));
                }
                // SAFETY: multi-key map immediates store an EBR-live
                // array of `n` values in word 0 (validated tag + count).
                let v = unsafe { edge.node_ptr().cast::<u64>().add(slot).read() };
                chk!();
                return Ok(Some(v));
            }
        }
    }
}

/// The shared writer/reader state behind both wrappers.
struct Shared<T> {
    inner: UnsafeCell<T>,
    version: SeqVersion,
    write: Mutex<()>,
    collector: Arc<Collector>,
}

// SAFETY: the OCC protocol above is exactly what makes the inner tree
// shareable — writers are serialized by the mutex + version brackets, and
// readers only act on validated, EBR-live data.
unsafe impl<T: Send> Send for Shared<T> {}
// SAFETY: as above.
unsafe impl<T: Send> Sync for Shared<T> {}

impl<T> Shared<T> {
    fn new(inner: T, alloc_of: impl FnOnce(&T) -> &NodeAlloc) -> Self {
        let collector = Arc::new(Collector::new());
        alloc_of(&inner).defer_to(Arc::clone(&collector));
        Self {
            inner: UnsafeCell::new(inner),
            version: SeqVersion::new(),
            write: Mutex::new(()),
            collector,
        }
    }

    /// Runs one mutation under the writer lock and version bracket.
    fn write<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let _g = self.write.lock().expect("writer lock poisoned");
        self.version.begin();
        // SAFETY: the writer mutex makes this the only mutable borrow.
        let r = f(unsafe { &mut *self.inner.get() });
        self.version.end();
        self.collector.try_advance();
        r
    }

    /// Consistent fallback read under the writer lock.
    fn read_locked<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let _g: MutexGuard<'_, ()> = self.write.lock().expect("writer lock poisoned");
        // SAFETY: the writer mutex excludes all mutation.
        f(unsafe { &*self.inner.get() })
    }
}

/// A set shareable across threads: one writer at a time (internally
/// serialized), lock-free validated readers. See the module docs for the
/// protocol and its trade-offs.
pub struct SyncExpanseSet {
    shared: Shared<ExpanseSet>,
}

impl Default for SyncExpanseSet {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncExpanseSet {
    /// Creates an empty concurrent set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            shared: Shared::new(ExpanseSet::new(), |s| s.occ_root().1),
        }
    }

    /// Inserts `key`; returns `true` if it was absent. Serializes with
    /// other writers.
    pub fn insert(&self, key: Key) -> bool {
        self.shared.write(|s| s.insert(key))
    }

    /// Removes `key`; returns `true` if it was present.
    pub fn remove(&self, key: Key) -> bool {
        self.shared.write(|s| s.remove(key))
    }

    /// Registers a reader handle for this thread's lookups.
    #[must_use]
    pub fn reader(&self) -> SetReader<'_> {
        SetReader {
            set: self,
            reader: self.shared.collector.register(),
        }
    }

    /// One-shot membership test (registers a throwaway reader; use
    /// [`Self::reader`] in hot loops).
    #[must_use]
    pub fn contains(&self, key: Key) -> bool {
        self.reader().contains(key)
    }

    /// Number of keys (validated read).
    #[must_use]
    pub fn len(&self) -> u64 {
        for _ in 0..MAX_RETRIES {
            let snap = self.shared.version.sample();
            // SAFETY: by-value snapshot; validated before use.
            let (root, _) = unsafe { (*self.shared.inner.get()).occ_root() };
            if self.shared.version.validate(snap) {
                return match root {
                    RootSnapshot::Empty => 0,
                    RootSnapshot::Leaf { pop, .. } => pop as u64,
                    RootSnapshot::Tree { pop, .. } => pop,
                };
            }
        }
        self.shared.read_locked(super::set::ExpanseSet::len)
    }

    /// True when no keys are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Runs `f` over the tree with all writers excluded — the escape
    /// hatch to the full single-threaded read API (iteration, ranges,
    /// `count_range`, …).
    pub fn with_locked<R>(&self, f: impl FnOnce(&ExpanseSet) -> R) -> R {
        self.shared.read_locked(f)
    }
}

/// A per-thread reader handle for [`SyncExpanseSet`].
pub struct SetReader<'a> {
    set: &'a SyncExpanseSet,
    reader: Reader,
}

impl SetReader<'_> {
    /// Lock-free membership test.
    #[must_use]
    pub fn contains(&self, key: Key) -> bool {
        let shared = &self.set.shared;
        for _ in 0..MAX_RETRIES {
            let _pin = self.reader.pin();
            let snap = shared.version.sample();
            // SAFETY: pinned + freshly sampled version; the walk
            // validates every load (see `walk_validated`).
            let root = unsafe { (*shared.inner.get()).occ_root().0 };
            // SAFETY: same pin + snapshot contract as the line above.
            if let Ok(r) = unsafe { walk_validated::<false>(root, key, &shared.version, snap) } {
                return r.is_some();
            }
        }
        shared.read_locked(|s| s.contains(key))
    }
}

/// A map shareable across threads: one writer at a time (internally
/// serialized), lock-free validated readers. See the module docs.
pub struct SyncExpanseMap {
    shared: Shared<ExpanseMap>,
}

impl Default for SyncExpanseMap {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncExpanseMap {
    /// Creates an empty concurrent map.
    #[must_use]
    pub fn new() -> Self {
        Self {
            shared: Shared::new(ExpanseMap::new(), |m| m.occ_root().1),
        }
    }

    /// Inserts `key → val`; returns the replaced value, if any.
    pub fn insert(&self, key: Key, val: u64) -> Option<u64> {
        self.shared.write(|m| m.insert(key, val))
    }

    /// Removes `key`; returns its value, if present.
    pub fn remove(&self, key: Key) -> Option<u64> {
        self.shared.write(|m| m.remove(key))
    }

    /// Registers a reader handle for this thread's lookups.
    #[must_use]
    pub fn reader(&self) -> MapReader<'_> {
        MapReader {
            map: self,
            reader: self.shared.collector.register(),
        }
    }

    /// One-shot lookup (registers a throwaway reader; use
    /// [`Self::reader`] in hot loops).
    #[must_use]
    pub fn get(&self, key: Key) -> Option<u64> {
        self.reader().get(key)
    }

    /// Number of keys (validated read).
    #[must_use]
    pub fn len(&self) -> u64 {
        for _ in 0..MAX_RETRIES {
            let snap = self.shared.version.sample();
            // SAFETY: by-value snapshot; validated before use.
            let (root, _) = unsafe { (*self.shared.inner.get()).occ_root() };
            if self.shared.version.validate(snap) {
                return match root {
                    RootSnapshot::Empty => 0,
                    RootSnapshot::Leaf { pop, .. } => pop as u64,
                    RootSnapshot::Tree { pop, .. } => pop,
                };
            }
        }
        self.shared.read_locked(super::map::ExpanseMap::len)
    }

    /// True when no keys are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Runs `f` over the tree with all writers excluded — the escape
    /// hatch to the full single-threaded read API.
    pub fn with_locked<R>(&self, f: impl FnOnce(&ExpanseMap) -> R) -> R {
        self.shared.read_locked(f)
    }
}

/// A per-thread reader handle for [`SyncExpanseMap`].
pub struct MapReader<'a> {
    map: &'a SyncExpanseMap,
    reader: Reader,
}

impl MapReader<'_> {
    /// Lock-free lookup.
    #[must_use]
    pub fn get(&self, key: Key) -> Option<u64> {
        let shared = &self.map.shared;
        for _ in 0..MAX_RETRIES {
            let _pin = self.reader.pin();
            let snap = shared.version.sample();
            // SAFETY: pinned + freshly sampled version; the walk
            // validates every load (see `walk_validated`).
            let root = unsafe { (*shared.inner.get()).occ_root().0 };
            // SAFETY: same pin + snapshot contract as the line above.
            if let Ok(r) = unsafe { walk_validated::<true>(root, key, &shared.version, snap) } {
                return r;
            }
        }
        shared.read_locked(|m| m.get(key))
    }
}

#[cfg(all(test, not(miri)))]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, Ordering};

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

    #[test]
    fn single_thread_agrees_with_model() {
        let m = SyncExpanseMap::new();
        let mut model = BTreeMap::new();
        let mut rng = XorShift(0x77);
        for _ in 0..4000 {
            let k = rng.next() % 8192;
            match rng.next() % 3 {
                0 => {
                    let v = rng.next();
                    assert_eq!(m.insert(k, v), model.insert(k, v));
                }
                1 => assert_eq!(m.remove(k), model.remove(&k)),
                _ => assert_eq!(m.get(k), model.get(&k).copied()),
            }
            assert_eq!(m.len(), model.len() as u64);
        }
        m.with_locked(|inner| inner.validate());
    }

    /// The Phase 7 gate: reader threads hammer lookups while the writer
    /// churns inserts/removes. Every read must be a value the key held
    /// at *some* point (here: a function of the key), never garbage —
    /// and nothing may crash (EBR keeps retired nodes alive for pinned
    /// readers).
    #[test]
    fn concurrent_readers_under_churn() {
        let m = Arc::new(SyncExpanseMap::new());
        let stop = Arc::new(AtomicBool::new(false));
        let val_of = |k: u64| !k ^ 0xABCD;

        // Clustered keys force cascades, skips, and downgrades.
        let key_of = |r: &mut XorShift| {
            let base = [0u64, 0x11_2233_4400, 0xFFFF_FF00_0000][(r.next() % 3) as usize];
            base + r.next() % 512
        };

        let readers: Vec<_> = (0..3)
            .map(|i| {
                let m = Arc::clone(&m);
                let stop = Arc::clone(&stop);
                std::thread::spawn(move || {
                    let rd = m.reader();
                    let mut rng = XorShift(0x1000 + i);
                    let mut hits = 0u64;
                    while !stop.load(Ordering::Relaxed) {
                        let k = key_of(&mut rng);
                        if let Some(v) = rd.get(k) {
                            assert_eq!(v, val_of(k), "torn value for {k:#x}");
                            hits += 1;
                        }
                    }
                    hits
                })
            })
            .collect();

        let mut rng = XorShift(0x9E37);
        let mut model = BTreeMap::new();
        // Churn for a wall-clock floor so the readers genuinely overlap
        // the writer (a fixed op count finishes before threads spin up
        // in release builds).
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_millis(300) {
            for _ in 0..10_000 {
                let k = key_of(&mut rng);
                if rng.next() % 2 == 0 {
                    m.insert(k, val_of(k));
                    model.insert(k, val_of(k));
                } else {
                    m.remove(k);
                    model.remove(&k);
                }
            }
        }
        stop.store(true, Ordering::Relaxed);
        let mut total_hits = 0;
        for r in readers {
            total_hits += r.join().expect("reader panicked");
        }
        // The churn keys stay populated ~half the time; a zero hit count
        // would mean the readers never actually raced the writer.
        assert!(total_hits > 0, "readers observed nothing");

        // Final state agrees with the model, via both read paths.
        let rd = m.reader();
        for (&k, &v) in &model {
            assert_eq!(rd.get(k), Some(v));
        }
        m.with_locked(|inner| {
            inner.validate();
            assert_eq!(inner.len(), model.len() as u64);
        });
    }

    #[test]
    fn concurrent_set_readers_under_churn() {
        let s = Arc::new(SyncExpanseSet::new());
        let stop = Arc::new(AtomicBool::new(false));

        let readers: Vec<_> = (0..2)
            .map(|i| {
                let s = Arc::clone(&s);
                let stop = Arc::clone(&stop);
                std::thread::spawn(move || {
                    let rd = s.reader();
                    let mut rng = XorShift(0x2000 + i);
                    while !stop.load(Ordering::Relaxed) {
                        // Presence flaps under churn; the assertion is
                        // "no crash, no hang, sane returns".
                        let _ = rd.contains(rng.next() % 4096);
                    }
                })
            })
            .collect();

        let mut rng = XorShift(0xBEEF);
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_millis(200) {
            for _ in 0..10_000 {
                let k = rng.next() % 4096;
                if rng.next() % 2 == 0 {
                    s.insert(k);
                } else {
                    s.remove(k);
                }
            }
        }
        stop.store(true, Ordering::Relaxed);
        for r in readers {
            r.join().expect("reader panicked");
        }
        s.with_locked(|inner| inner.validate());
    }
}
