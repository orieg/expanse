//! Phase 7: concurrent wrappers — one writer, many lock-free readers.
//!
//! [`SyncExpanseSet`] / [`SyncExpanseMap`] / [`SyncExpanseBlobMap`] wrap the
//! single-threaded structures with the `occ` protocol:
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

use crate::blobmap::{ArenaError, CompactionStats, ExpanseBlobMap};
use crate::bytesmap::ExpanseBytesMap;
use crate::leaf;
use crate::map::ExpanseMap;
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1, LeafBitmapL};
use crate::occ::{Collector, Pin, Reader, SeqVersion};
use crate::set::ExpanseSet;
use crate::slot::{SlotTag, ValueSlot};
use crate::strmap::ExpanseStrMap;
use crate::types::{EdgeTag, EdgeType, Key, digit};
use core::cell::UnsafeCell;
use std::hash::{BuildHasher, RandomState};
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
pub(crate) struct Retry;

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
pub(crate) unsafe fn walk_validated<const MAP: bool>(
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
            // Root leaf: `pop` sorted u64 keys at the base, then (map
            // flavor) the value area at `map::leaf_values_offset(pop)`.
            // That offset is class-based, NOT `pop` — always ask
            // `map::leaf_values_offset` rather than recomputing it here.
            // Covered by the tree version throughout.
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
            // SAFETY: the value area begins at the shared class-based
            // offset; `lo < pop` so the slot is in bounds.
            let v = unsafe {
                ptr.add(crate::map::leaf_values_offset(pop))
                    .cast::<u64>()
                    .add(lo)
                    .read()
            };
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
                    // Validate BEFORE indexing the subarray, not after:
                    // `rank` comes from the bitmap and `sub`/its length
                    // from the pointer array, which a concurrent writer
                    // updates separately. An unvalidated pair can pick a
                    // rank from the new bitmap against the old, shorter
                    // allocation — an out-of-bounds read that a later
                    // check cannot undo. (EBR keeps a retired subarray
                    // mapped, so a *stale* pointer is safe to read; a
                    // stale pointer with a fresh rank is not.)
                    // SAFETY: live version field (EBR).
                    if !unsafe { crate::occ::node_validate(vp, nsnap) } {
                        return Err(Retry);
                    }
                    // SAFETY: bitmap/subarray pair validated consistent
                    // just above → the subarray holds at least `rank + 1`
                    // EBR-live edges.
                    edge = unsafe { sub.add(rank).read() };
                    // The edge copy itself must also be covered: re-check
                    // before the next iteration dereferences it.
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
                let payload: [u8; 16] = if MAP {
                    let mut p = [0u8; 16];
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
    /// Wraps `inner`, handing every allocation source `attach` names over to
    /// a fresh epoch collector (deferred reclamation).
    fn new(inner: T, attach: impl FnOnce(&T, &Arc<Collector>)) -> Self {
        let collector = Arc::new(Collector::new());
        attach(&inner, &collector);
        Self::with_collector(inner, collector)
    }

    /// Wraps `inner` around an existing collector — for construction paths
    /// that must defer allocators *while building* `inner` (a populated
    /// structure is shared by rebuilding it through pre-deferred
    /// allocators; see `NodeAlloc::defer_to`).
    fn with_collector(inner: T, collector: Arc<Collector>) -> Self {
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

    /// Validated population read: samples the version, copies the root
    /// snapshot by value (no heap dereference, so no pin is needed), and
    /// retries until the snapshot validates — one shared definition for
    /// every wrapper's `len()`. Falls back to `locked` after bounded
    /// retries.
    fn validated_len(
        &self,
        root_of: impl Fn(&T) -> RootSnapshot,
        locked: impl FnOnce(&T) -> u64,
    ) -> u64 {
        for _ in 0..MAX_RETRIES {
            let snap = self.version.sample();
            // SAFETY: by-value snapshot; validated before use.
            let root = root_of(unsafe { &*self.inner.get() });
            if self.version.validate(snap) {
                return match root {
                    RootSnapshot::Empty => 0,
                    RootSnapshot::Leaf { pop, .. } => pop as u64,
                    RootSnapshot::Tree { pop, .. } => pop,
                };
            }
        }
        self.read_locked(locked)
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
            shared: Shared::new(ExpanseSet::new(), |s, c| {
                s.occ_root().1.defer_to(Arc::clone(c))
            }),
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

    /// Removes every key from the set.
    pub fn clear(&self) {
        self.shared.write(|s| s.clear());
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
        self.shared
            .validated_len(|s| s.occ_root().0, ExpanseSet::len)
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
            shared: Shared::new(ExpanseMap::new(), |m, c| {
                m.occ_root().1.defer_to(Arc::clone(c))
            }),
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

    /// Removes every key-value pair from the map.
    pub fn clear(&self) {
        self.shared.write(|m| m.clear());
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
        self.shared
            .validated_len(|m| m.occ_root().0, ExpanseMap::len)
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

/// A blob map shareable across threads (issue #219 Phase 1): one writer at a
/// time (internally serialized), lock-free validated readers with epoch-pinned
/// zero-copy payload borrows. See the module docs for the protocol and its
/// trade-offs.
///
/// On top of the [`SyncExpanseMap`] protocol over the index trie, the blob
/// map's arena participates in reclamation:
///
/// - The index walk yields a validated 64-bit [`ValueSlot`]. **Inline**
///   payloads (≤ 7 bytes) decode by value from that word — zero slab reads,
///   covered entirely by the trie validation.
/// - **Arena** payloads resolve through an RCU-published chunk table
///   (readers never touch the arena's internal chunk vector): the reader
///   bounds-checks against the pinned table, re-validates the tree version,
///   and only then hands out a zero-copy borrow. Arena records are never
///   rewritten in place, and compaction retires dead chunks through the
///   epoch [`Collector`], so a validated borrow stays byte-stable for the
///   life of the guard's pin.
/// - Structural reads that need multi-field consistency (`mem_used`,
///   `scan_filtered`, iteration) go through [`Self::with_locked`].
pub struct SyncExpanseBlobMap {
    shared: Shared<ExpanseBlobMap>,
}

impl Default for SyncExpanseBlobMap {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncExpanseBlobMap {
    /// Creates an empty concurrent blob map with default arena chunk slabs.
    #[must_use]
    pub fn new() -> Self {
        Self::from_map(ExpanseBlobMap::new())
    }

    /// Creates an empty concurrent blob map with a custom arena chunk size
    /// (clamped as by [`ExpanseBlobMap::with_chunk_size`]).
    #[must_use]
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        Self::from_map(ExpanseBlobMap::with_chunk_size(chunk_size))
    }

    fn from_map(mut map: ExpanseBlobMap) -> Self {
        let collector = Arc::new(Collector::new());
        // A populated single-threaded index holds slab-carved node memory
        // that must never be retired to the collector (see
        // `NodeAlloc::defer_to`): rebuild the index through a pre-deferred
        // allocator (a no-op-cheap copy for an empty map). Arena chunks are
        // whole allocations and defer in place.
        map.rebuild_index_deferred(&collector);
        map.arena().defer_to(Arc::clone(&collector));
        Self {
            shared: Shared::with_collector(map, collector),
        }
    }

    /// Inserts `key → data` with 24-bit hot metadata; serializes with other
    /// writers. Semantics as [`ExpanseBlobMap::insert`] (inline payloads
    /// ignore `hot_meta`).
    pub fn insert(&self, key: Key, data: &[u8], hot_meta: u32) -> Result<(), ArenaError> {
        self.shared.write(|m| m.insert(key, data, hot_meta))
    }

    /// Removes `key`; returns `true` if it was present.
    pub fn remove(&self, key: Key) -> bool {
        self.shared.write(|m| m.remove(key))
    }

    /// Runs arena garbage collection and compaction. Dead chunks are retired
    /// through the epoch collector, so concurrent pinned readers keep reading
    /// their (relocated-from) payload bytes safely.
    pub fn compact(&self) -> Result<CompactionStats, ArenaError> {
        self.shared.write(ExpanseBlobMap::compact)
    }

    /// Removes every entry and retires all arena chunks.
    pub fn clear(&self) {
        self.shared.write(ExpanseBlobMap::clear);
    }

    /// Registers a reader handle for this thread's lookups.
    #[must_use]
    pub fn reader(&self) -> BlobReader<'_> {
        BlobReader {
            map: self,
            reader: self.shared.collector.register(),
        }
    }

    /// One-shot owned-copy lookup (registers a throwaway reader; use
    /// [`Self::reader`] + [`BlobReader::pin`] in hot loops for zero-copy).
    #[must_use]
    pub fn get(&self, key: Key) -> Option<(Vec<u8>, u32)> {
        self.reader().get(key)
    }

    /// One-shot metadata lookup — never touches payload memory (registers a
    /// throwaway reader; use [`Self::reader`] in hot loops).
    #[must_use]
    pub fn get_meta(&self, key: Key) -> Option<u32> {
        self.reader().get_meta(key)
    }

    /// One-shot membership test (registers a throwaway reader; use
    /// [`Self::reader`] in hot loops).
    #[must_use]
    pub fn contains_key(&self, key: Key) -> bool {
        self.reader().contains(key)
    }

    /// Number of entries (validated read).
    #[must_use]
    pub fn len(&self) -> u64 {
        self.shared
            .validated_len(|m| m.index().occ_root().0, ExpanseBlobMap::len)
    }

    /// True when no entries are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Total heap memory used by the index and arena (consistent read under
    /// the writer lock).
    ///
    /// Counts what the structure currently owns: chunks already retired to
    /// the epoch collector (by a compaction or [`Self::clear`]) but not yet
    /// reclaimed — e.g. while a reader guard pins the epoch — are no longer
    /// included even though their allocations are still resident.
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.shared.read_locked(ExpanseBlobMap::mem_used)
    }

    /// Runs `f` over the map with all writers excluded — the escape hatch to
    /// the full single-threaded read API (`scan_filtered`, iteration over
    /// [`ExpanseBlobMap::index`], persistence, …).
    pub fn with_locked<R>(&self, f: impl FnOnce(&ExpanseBlobMap) -> R) -> R {
        self.shared.read_locked(f)
    }
}

/// Wraps an already-populated single-threaded blob map (e.g. one loaded via
/// [`ExpanseBlobMap::load_from_file`]) for concurrent sharing.
///
/// # Panics
///
/// Panics if the map's index or arena was already deferred to a different
/// collector (i.e. the map was previously shared).
impl From<ExpanseBlobMap> for SyncExpanseBlobMap {
    fn from(map: ExpanseBlobMap) -> Self {
        Self::from_map(map)
    }
}

/// A per-thread reader handle for [`SyncExpanseBlobMap`].
pub struct BlobReader<'a> {
    map: &'a SyncExpanseBlobMap,
    reader: Reader,
}

/// The blob map's hot-metadata semantics, applied to a validated slot word:
/// `ArenaMeta` slots report their 24-bit field, inline slots report `0`, and
/// non-payload tags read as absent. Reads nothing but the word — both
/// `get_meta` paths (lock-free and locked fallback) share it so they cannot
/// disagree.
fn blob_slot_meta(raw: u64) -> Option<u32> {
    let slot = ValueSlot::from_raw(raw);
    let tag = slot.tag();
    if tag == SlotTag::ArenaMeta {
        Some(slot.arena_meta_meta())
    } else {
        tag.is_inline().then_some(0)
    }
}

impl BlobReader<'_> {
    /// Pins the current epoch: payload borrows obtained through the returned
    /// guard stay valid (and byte-stable) until the guard drops. Keep guards
    /// short-lived — a pinned epoch defers all reclamation (retired nodes and
    /// arena chunks accumulate until the pin is released).
    ///
    /// Takes `&mut self` because a reader holds a single epoch slot: pins
    /// from one reader must never overlap (dropping any of them would unpin
    /// the others — see [`Reader::pin`]), and the exclusive borrow makes an
    /// overlap a compile error. Register a second reader for overlapping
    /// guards.
    #[must_use]
    pub fn pin(&mut self) -> BlobReadGuard<'_> {
        BlobReadGuard {
            map: self.map,
            _pin: self.reader.pin(),
        }
    }

    /// Lock-free owned-copy lookup (pins only for the duration of the call).
    #[must_use]
    pub fn get(&mut self, key: Key) -> Option<(Vec<u8>, u32)> {
        let guard = self.pin();
        guard
            .get(key)
            .map(|(view, meta)| (view.as_bytes().to_vec(), meta))
    }

    /// Bounded lock-free validated slot-word lookup shared by the word-level
    /// reads; `Err(Retry)` after retry exhaustion (the caller then falls
    /// back under the writer lock).
    fn lookup_slot(&mut self, key: Key) -> Result<Option<u64>, Retry> {
        let shared = &self.map.shared;
        for _ in 0..MAX_RETRIES {
            let _pin = self.reader.pin();
            let snap = shared.version.sample();
            // SAFETY: pinned + freshly sampled version; the walk validates
            // every load (see `walk_validated`).
            let root = unsafe { (*shared.inner.get()).index().occ_root().0 };
            // SAFETY: same pin + snapshot contract as the line above.
            if let Ok(found) = unsafe { walk_validated::<true>(root, key, &shared.version, snap) } {
                return Ok(found);
            }
        }
        Err(Retry)
    }

    /// Lock-free metadata lookup: the validated slot word alone answers it —
    /// no payload cache line is touched (inline payloads report `0`, as in
    /// [`ExpanseBlobMap::get`]; non-payload slots return `None`). Because
    /// the payload is never resolved, a dangling arena locator (possible
    /// only in a corrupted image) still reports its stored metadata.
    #[must_use]
    pub fn get_meta(&mut self, key: Key) -> Option<u32> {
        match self.lookup_slot(key) {
            Ok(found) => found.and_then(blob_slot_meta),
            Err(Retry) => self
                .map
                .shared
                .read_locked(|m| m.index().get(key).and_then(blob_slot_meta)),
        }
    }

    /// Lock-free membership test.
    #[must_use]
    pub fn contains(&mut self, key: Key) -> bool {
        match self.lookup_slot(key) {
            Ok(found) => found.is_some(),
            Err(Retry) => self.map.shared.read_locked(|m| m.contains_key(key)),
        }
    }
}

/// An epoch-pinned read guard for [`SyncExpanseBlobMap`] — the
/// `SyncBlobReaderGuard` of issue #219: while it lives, nothing the arena or
/// index retires is freed, so the [`SyncBlobView`] borrows it hands out stay
/// valid across concurrent writes and compactions.
pub struct BlobReadGuard<'g> {
    map: &'g SyncExpanseBlobMap,
    _pin: Pin<'g>,
}

impl BlobReadGuard<'_> {
    /// Lock-free validated lookup. Inline payloads are decoded by value from
    /// the validated slot word; arena payloads are zero-copy borrows of
    /// epoch-pinned slab bytes. Falls back to an owned copy under the writer
    /// lock after bounded retries.
    #[must_use]
    pub fn get(&self, key: Key) -> Option<(SyncBlobView<'_>, u32)> {
        let shared = &self.map.shared;
        for _ in 0..MAX_RETRIES {
            let snap = shared.version.sample();
            // SAFETY: the guard's pin predates this sample; the walk
            // validates every load (see `walk_validated`).
            let root = unsafe { (*shared.inner.get()).index().occ_root().0 };
            // SAFETY: same pin + snapshot contract as the line above.
            let Ok(found) = (unsafe { walk_validated::<true>(root, key, &shared.version, snap) })
            else {
                continue;
            };
            // The walk validated this result: absent stays absent, and a
            // present slot word is the value the key held at `snap`.
            let raw = found?;
            let slot = ValueSlot::from_raw(raw);
            let tag = slot.tag();
            if tag.is_inline() {
                let (buf, len) = slot.inline_payload();
                return Some((
                    SyncBlobView::Inline {
                        buf,
                        len: len as u8,
                    },
                    0,
                ));
            }
            if tag != SlotTag::ArenaMeta {
                // Mirrors `ExpanseBlobMap::get`: non-payload tags read as
                // absent (already validated by the walk).
                return None;
            }
            let meta = slot.arena_meta_meta();
            // SAFETY: single atomic load of the published table pointer; the
            // racy `&` borrow of the arena struct is confined to that load
            // (documented module-level seqlock caveat).
            let table = unsafe { (*shared.inner.get()).arena().reader_table() };
            // SAFETY: the guard's pin predates the table load, so the table
            // and every chunk it references are EBR-live.
            let resolved =
                unsafe { crate::blobmap::resolve_meta_in_table(table, slot.arena_meta_locator()) };
            match resolved {
                Some((ptr, len)) => {
                    if shared.version.validate(snap) {
                        // No writer overlapped: the resolution used the
                        // table consistent with `snap`, so `ptr..ptr+len` is
                        // the record's live payload. Arena records are never
                        // rewritten in place and retired chunks stay mapped
                        // under this guard's pin, so the borrow is
                        // byte-stable for the guard's lifetime.
                        // SAFETY: in-bounds of an EBR-live chunk (see
                        // `resolve_meta_in_table`), immutable while pinned.
                        let bytes = unsafe { core::slice::from_raw_parts(ptr, len) };
                        return Some((SyncBlobView::Arena(bytes), meta));
                    }
                }
                None => {
                    if shared.version.validate(snap) {
                        // Validated dangling locator — mirrors the
                        // single-threaded `get` returning `None`.
                        return None;
                    }
                }
            }
        }
        shared.read_locked(|m| {
            m.get(key)
                .map(|(view, meta)| (SyncBlobView::Owned(view.as_bytes().to_vec()), meta))
        })
    }
}

/// A validated payload view obtained through a [`BlobReadGuard`].
///
/// `Inline` payloads are decoded **by value** from the validated slot word
/// (they are at most 7 bytes; copying beats exposing racy leaf-slot memory).
/// `Arena` payloads borrow the epoch-pinned slab bytes zero-copy. `Owned` is
/// the bounded-retry fallback, copied under the writer lock.
#[derive(Clone, Debug)]
pub enum SyncBlobView<'g> {
    /// Inline payload decoded from the value-slot word.
    Inline {
        /// Payload bytes; only the first `len` are meaningful.
        buf: [u8; 7],
        /// Meaningful prefix length of `buf` (≤ 7).
        len: u8,
    },
    /// Zero-copy borrow of an epoch-pinned arena record.
    Arena(&'g [u8]),
    /// Owned copy taken under the writer lock (bounded-retry fallback).
    Owned(Vec<u8>),
}

impl SyncBlobView<'_> {
    /// The payload bytes.
    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            SyncBlobView::Inline { buf, len } => &buf[..*len as usize],
            SyncBlobView::Arena(bytes) => bytes,
            SyncBlobView::Owned(bytes) => bytes,
        }
    }

    /// Payload length in bytes.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.as_bytes().len()
    }

    /// True for a zero-length payload.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }

    /// True when the payload was stored inline in the value slot.
    #[inline]
    #[must_use]
    pub fn is_inline(&self) -> bool {
        matches!(self, SyncBlobView::Inline { .. })
    }

    /// True when the payload is a zero-copy arena borrow.
    #[inline]
    #[must_use]
    pub fn is_arena(&self) -> bool {
        matches!(self, SyncBlobView::Arena(_))
    }
}

impl core::ops::Deref for SyncBlobView<'_> {
    type Target = [u8];
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_bytes()
    }
}

impl AsRef<[u8]> for SyncBlobView<'_> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl PartialEq<[u8]> for SyncBlobView<'_> {
    #[inline]
    fn eq(&self, other: &[u8]) -> bool {
        self.as_bytes() == other
    }
}

impl<'g> PartialEq<SyncBlobView<'g>> for [u8] {
    #[inline]
    fn eq(&self, other: &SyncBlobView<'g>) -> bool {
        self == other.as_bytes()
    }
}

/// A string map shareable across threads (issue #219 Phase 2): one writer
/// at a time (internally serialized), lock-free validated readers for
/// point lookups. See the module docs for the protocol and its trade-offs.
///
/// On top of the [`SyncExpanseMap`] protocol, a lookup cascades across the
/// meta-trie's sub-maps (one hop per 8 key bytes): every hop's walk
/// validates hand-over-hand against the one shared tree version, so the
/// whole multi-hop path is consistent with a single snapshot, and every
/// node/suffix free routes through the epoch [`Collector`]
/// (`ExpanseStrMap::defer_to`), so a pinned reader never dereferences
/// freed memory. Long keys mean more hops per attempt; the bounded-retry
/// fallback to the writer lock caps starvation under write storms.
///
/// Ordered navigation and prefix scans take `&mut ExpanseStrMap` in the
/// single-threaded API (they return writable slots), so they are reachable
/// only through [`Self::with_locked_mut`].
pub struct SyncExpanseStrMap {
    shared: Shared<ExpanseStrMap>,
}

impl Default for SyncExpanseStrMap {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncExpanseStrMap {
    /// Creates an empty concurrent string map.
    #[must_use]
    pub fn new() -> Self {
        Self::from_map(ExpanseStrMap::new())
    }

    fn from_map(mut src: ExpanseStrMap) -> Self {
        let collector = Arc::new(Collector::new());
        let mut map = ExpanseStrMap::new();
        map.defer_to(Arc::clone(&collector));
        // A populated map's sub-trie allocators hold slab-carved node
        // memory that must never be retired to the collector (see
        // `NodeAlloc::defer_to`): rebuild entry-by-entry through the
        // pre-deferred map (a no-op for the `new()` path). The sweep is
        // O(n · depth) with one key allocation per entry — a wrap-once
        // construction cost, acceptable for its startup use case.
        let mut cursor = src.first();
        while let Some((key, slot)) = cursor {
            // SAFETY: the slot is valid until `src`'s next mutation; only
            // navigation happens between here and the next hop.
            map.insert(&key, unsafe { *slot.as_ptr() });
            cursor = src.next_after(&key);
        }
        Self {
            shared: Shared::with_collector(map, collector),
        }
    }

    /// Inserts `key → val`; returns the replaced value, if any. Serializes
    /// with other writers. Keys are NUL-free byte strings.
    pub fn insert(&self, key: &[u8], val: u64) -> Option<u64> {
        self.shared.write(|m| m.insert(key, val))
    }

    /// Removes `key`; returns its value, if present.
    pub fn remove(&self, key: &[u8]) -> Option<u64> {
        self.shared.write(|m| m.remove(key))
    }

    /// Removes every entry; returns the heap bytes released.
    pub fn clear(&self) -> u64 {
        self.shared.write(ExpanseStrMap::clear)
    }

    /// Registers a reader handle for this thread's lookups.
    #[must_use]
    pub fn reader(&self) -> StrReader<'_> {
        StrReader {
            map: self,
            reader: self.shared.collector.register(),
        }
    }

    /// One-shot lookup (registers a throwaway reader; use [`Self::reader`]
    /// in hot loops).
    #[must_use]
    pub fn get(&self, key: &[u8]) -> Option<u64> {
        self.reader().get(key)
    }

    /// One-shot membership test (registers a throwaway reader; use
    /// [`Self::reader`] in hot loops).
    #[must_use]
    pub fn contains_key(&self, key: &[u8]) -> bool {
        self.get(key).is_some()
    }

    /// Number of keys (validated read).
    #[must_use]
    pub fn len(&self) -> u64 {
        for _ in 0..MAX_RETRIES {
            let snap = self.shared.version.sample();
            // SAFETY: single-word racy copy; validated before use.
            let pop = unsafe { (*self.shared.inner.get()).len() };
            if self.shared.version.validate(snap) {
                return pop;
            }
        }
        self.shared.read_locked(ExpanseStrMap::len)
    }

    /// True when no keys are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Runs `f` over the map with all writers excluded — the escape hatch
    /// to the single-threaded `&self` read API.
    pub fn with_locked<R>(&self, f: impl FnOnce(&ExpanseStrMap) -> R) -> R {
        self.shared.read_locked(f)
    }

    /// Runs `f` with exclusive access under the writer lock and version
    /// bracket — the escape hatch to ordered navigation and prefix scans
    /// (`next_at_or_after`, `prev_at_or_before`, `first`/`last`, …), which
    /// take `&mut self` because they return writable value slots. Slots
    /// obtained inside must not escape `f`.
    pub fn with_locked_mut<R>(&self, f: impl FnOnce(&mut ExpanseStrMap) -> R) -> R {
        self.shared.write(f)
    }
}

/// Wraps an already-populated single-threaded string map for concurrent
/// sharing (every existing sub-trie is switched to deferred reclamation).
impl From<ExpanseStrMap> for SyncExpanseStrMap {
    fn from(map: ExpanseStrMap) -> Self {
        Self::from_map(map)
    }
}

/// A per-thread reader handle for [`SyncExpanseStrMap`].
pub struct StrReader<'a> {
    map: &'a SyncExpanseStrMap,
    reader: Reader,
}

impl StrReader<'_> {
    /// Lock-free lookup: a bounded, validated cascade across the sub-tries
    /// (one hop per 8 key bytes), falling back to the writer lock after
    /// bounded retries.
    #[must_use]
    pub fn get(&self, key: &[u8]) -> Option<u64> {
        let shared = &self.map.shared;
        for _ in 0..MAX_RETRIES {
            let _pin = self.reader.pin();
            let snap = shared.version.sample();
            // SAFETY: pinned + freshly sampled version; every hop of the
            // cascade validates its loads (see `ExpanseStrMap::get_validated`).
            let attempt =
                unsafe { (*shared.inner.get()).get_validated(key, &shared.version, snap) };
            if let Ok(r) = attempt {
                return r;
            }
        }
        shared.read_locked(|m| m.get(key))
    }

    /// Lock-free membership test.
    #[must_use]
    pub fn contains(&self, key: &[u8]) -> bool {
        self.get(key).is_some()
    }
}

/// An **unordered** byte-string map shareable across threads (issue
/// #362 — the JudyHS member completing the Sync* family): one writer at
/// a time (internally serialized), lock-free validated readers. See the
/// module docs for the protocol and its trade-offs.
///
/// A lookup is one 64-bit hash, a single validated hand-over-hand walk
/// over the hash trie, and one byte-exact comparison against the
/// collision bucket — the flat competitor class for unordered point
/// lookups (`DashMap` et al.), unlike the multi-hop ordered
/// [`SyncExpanseStrMap`]. Collision buckets are write-once after
/// publication: structural changes publish a replacement bucket and
/// retire the old one (shell, entry buffer, and key buffers) through
/// the epoch [`Collector`]; only value words mutate in place, covered
/// by the reader's final tree-version validation.
///
/// The hasher is shared untouched between the writer and every reader
/// (hashing goes through `&self` concurrently), hence the `Sync` bound.
pub struct SyncExpanseBytesMap<S: BuildHasher + Send + Sync = RandomState> {
    shared: Shared<ExpanseBytesMap<S>>,
}

impl Default for SyncExpanseBytesMap<RandomState> {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncExpanseBytesMap<RandomState> {
    /// Creates an empty concurrent map with a freshly seeded hasher.
    #[must_use]
    pub fn new() -> Self {
        Self::with_hasher(RandomState::new())
    }
}

impl<S: BuildHasher + Send + Sync> SyncExpanseBytesMap<S> {
    /// Creates an empty concurrent map using `hasher`.
    #[must_use]
    pub fn with_hasher(hasher: S) -> Self {
        let collector = Arc::new(Collector::new());
        let map = ExpanseBytesMap::with_hasher(hasher);
        // Fresh map: deferral precedes every allocation.
        map.defer_to(Arc::clone(&collector));
        Self {
            shared: Shared::with_collector(map, collector),
        }
    }

    /// Inserts `key → val`; returns the replaced value, if any.
    /// Serializes with other writers.
    pub fn insert(&self, key: &[u8], val: u64) -> Option<u64> {
        self.shared.write(|m| m.insert(key, val))
    }

    /// Removes `key`; returns its value, if present.
    pub fn remove(&self, key: &[u8]) -> Option<u64> {
        self.shared.write(|m| m.remove(key))
    }

    /// Removes every key and releases all memory.
    pub fn clear(&self) {
        self.shared.write(ExpanseBytesMap::clear)
    }

    /// Registers a reader handle for this thread's lookups.
    #[must_use]
    pub fn reader(&self) -> BytesReader<'_, S> {
        BytesReader {
            map: self,
            reader: self.shared.collector.register(),
        }
    }

    /// One-shot lookup (registers a throwaway reader; use
    /// [`Self::reader`] in hot loops).
    #[must_use]
    pub fn get(&self, key: &[u8]) -> Option<u64> {
        self.reader().get(key)
    }

    /// One-shot membership test (registers a throwaway reader; use
    /// [`Self::reader`] in hot loops).
    #[must_use]
    pub fn contains_key(&self, key: &[u8]) -> bool {
        self.get(key).is_some()
    }

    /// Number of keys (validated read; the entry count, not the bucket
    /// count).
    #[must_use]
    pub fn len(&self) -> u64 {
        for _ in 0..MAX_RETRIES {
            let snap = self.shared.version.sample();
            // SAFETY: single-word racy copy; validated before use.
            let pop = unsafe { (*self.shared.inner.get()).len() };
            if self.shared.version.validate(snap) {
                return pop;
            }
        }
        self.shared.read_locked(ExpanseBytesMap::len)
    }

    /// True when no keys are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Heap bytes used (consistent read under the writer lock).
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.shared.read_locked(ExpanseBytesMap::mem_used)
    }

    /// Runs `f` over the map with all writers excluded — the escape
    /// hatch to the single-threaded `&self` read API ([`ExpanseBytesMap::for_each`], …).
    pub fn with_locked<R>(&self, f: impl FnOnce(&ExpanseBytesMap<S>) -> R) -> R {
        self.shared.read_locked(f)
    }

    /// Runs `f` with exclusive access under the writer lock and version
    /// bracket — the escape hatch to the compat slot API
    /// ([`ExpanseBytesMap::ins_slot`] / [`ExpanseBytesMap::get_value_slot`],
    /// which take `&mut self` because they return writable value slots).
    /// Slots obtained inside must not escape `f`.
    pub fn with_locked_mut<R>(&self, f: impl FnOnce(&mut ExpanseBytesMap<S>) -> R) -> R {
        self.shared.write(f)
    }
}

/// Wraps an already-populated single-threaded map for concurrent
/// sharing. The entries are rebuilt through a pre-deferred map with a
/// fresh `S::default()` hasher (hash values are internal, so reseeding
/// is invisible): a populated map's hash trie holds slab-carved node
/// memory that must never be retired to the collector (see
/// `NodeAlloc::defer_to`).
impl<S: BuildHasher + Send + Sync + Default> From<ExpanseBytesMap<S>> for SyncExpanseBytesMap<S> {
    fn from(src: ExpanseBytesMap<S>) -> Self {
        let collector = Arc::new(Collector::new());
        let mut map = ExpanseBytesMap::with_hasher(S::default());
        map.defer_to(Arc::clone(&collector));
        // Entry-by-entry sweep: O(n) with one rehash per entry — a
        // wrap-once construction cost (see `SyncExpanseStrMap`).
        src.for_each(|key, val| {
            map.insert(key, val);
        });
        Self {
            shared: Shared::with_collector(map, collector),
        }
    }
}

/// A per-thread reader handle for [`SyncExpanseBytesMap`].
pub struct BytesReader<'a, S: BuildHasher + Send + Sync = RandomState> {
    map: &'a SyncExpanseBytesMap<S>,
    reader: Reader,
}

impl<S: BuildHasher + Send + Sync> BytesReader<'_, S> {
    /// Lock-free lookup: one bounded, validated hash-trie walk plus a
    /// byte-exact bucket comparison, falling back to the writer lock
    /// after bounded retries.
    #[must_use]
    pub fn get(&self, key: &[u8]) -> Option<u64> {
        let shared = &self.map.shared;
        for _ in 0..MAX_RETRIES {
            let _pin = self.reader.pin();
            let snap = shared.version.sample();
            // SAFETY: pinned + freshly sampled version; every load is
            // validated (see `ExpanseBytesMap::get_validated`).
            let attempt =
                unsafe { (*shared.inner.get()).get_validated(key, &shared.version, snap) };
            if let Ok(r) = attempt {
                return r;
            }
        }
        shared.read_locked(|m| m.get(key))
    }

    /// Lock-free membership test.
    #[must_use]
    pub fn contains(&self, key: &[u8]) -> bool {
        self.get(key).is_some()
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

    /// Deterministic guard for the root-leaf layout the concurrent read
    /// path shares with `map`.
    ///
    /// This existed as a *concurrency* failure first: when capacity
    /// classes arrived, `map` moved the value area to a class-based
    /// offset and `sync` kept computing it from the population, so a
    /// reader returned a neighbouring key's value. It reproduced only on
    /// aarch64, under churn, as a "torn value" — an expensive way to
    /// learn about an off-by-one. Every population below the promotion
    /// cap is checked here on one thread, so the same drift fails fast
    /// and unambiguously next time.
    #[test]
    fn sync_reader_matches_root_leaf_layout_at_every_population() {
        for pop in 1..crate::set::ROOT_LEAF_CAP {
            let m = SyncExpanseMap::new();
            for i in 0..pop as u64 {
                // Keys spread out so ordering is unambiguous.
                m.insert(i * 0x1001, !(i * 0x1001));
            }
            assert_eq!(m.len(), pop as u64, "population {pop}");
            let rd = m.reader();
            for i in 0..pop as u64 {
                let k = i * 0x1001;
                assert_eq!(
                    rd.get(k),
                    Some(!k),
                    "population {pop}: value for key {k:#x} came from the wrong slot"
                );
            }
            // A key that is absent must stay absent at every population.
            assert_eq!(rd.get(0x7FFF_FFFF), None, "population {pop}");
        }
    }

    #[test]
    fn single_thread_agrees_with_model() {
        let m = SyncExpanseMap::new();
        let mut model = BTreeMap::new();
        let mut rng = XorShift(0x77);
        for _ in 0..if cfg!(miri) { 100 } else { 4000 } {
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

    /// Deterministic NUL-free key for index `idx`: prefix classes exercise
    /// shared-prefix routing, every 8th key carries a 96-byte chain so the
    /// cascade runs many hops, and the tail bytes are a keyed PRNG stream.
    fn str_key_of(idx: u64) -> Vec<u8> {
        const PREFIXES: [&[u8]; 4] = [
            b"",
            b"user:profile:",
            b"a/very/long/shared/api/path/v2/tenants/",
            b"k",
        ];
        let mut k = PREFIXES[(idx % 4) as usize].to_vec();
        if idx % 8 == 0 {
            k.extend_from_slice(&[b'd'; 96]);
        }
        let mut x = idx | 1;
        for _ in 0..(idx % 24) {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            k.push(((x >> 56) as u8).max(1));
        }
        k
    }

    /// FNV-1a over the full key: a misresolved, truncated, or torn key
    /// lookup cannot accidentally return the right value.
    fn str_val_of(key: &[u8]) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for &b in key {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
        h
    }

    #[test]
    fn sync_str_single_thread_agrees_with_model() {
        let m = SyncExpanseStrMap::new();
        let mut model: BTreeMap<Vec<u8>, u64> = BTreeMap::new();
        let mut rng = XorShift(0x99);
        for i in 0..4000u64 {
            let k = str_key_of(rng.next() % 512);
            match rng.next() % 3 {
                0 => {
                    let v = str_val_of(&k);
                    assert_eq!(m.insert(&k, v), model.insert(k.clone(), v), "ins {k:?}");
                }
                1 => assert_eq!(m.remove(&k), model.remove(&k), "rm {k:?}"),
                _ => assert_eq!(m.get(&k), model.get(&k).copied(), "get {k:?}"),
            }
            if i % 1000 == 999 {
                m.clear();
                model.clear();
            }
            assert_eq!(m.len(), model.len() as u64);
        }
        // Navigation still works through the locked escape hatch.
        m.with_locked_mut(|inner| {
            let mut cursor = inner.first();
            for (mk, mv) in &model {
                let (k, slot) = cursor.expect("sweep entry");
                assert_eq!(&k, mk);
                // SAFETY: slot valid until the next mutation; none happens.
                assert_eq!(unsafe { *slot.as_ptr() }, *mv);
                cursor = inner.next_after(&k);
            }
            assert!(cursor.is_none());
        });
    }

    /// Phase-2 gate (issue #219): readers hammer the multi-hop cascade
    /// while the writer churns inserts/removes — suffix splits, in-place
    /// value updates, node pruning — and periodically clears the whole
    /// tree (retiring entire subtrees under active readers). Every
    /// observed value must be the key's full-key hash.
    #[test]
    fn concurrent_str_readers_under_churn() {
        let m = Arc::new(SyncExpanseStrMap::new());
        let stop = Arc::new(AtomicBool::new(false));

        let readers: Vec<_> = (0..3)
            .map(|i| {
                let m = Arc::clone(&m);
                let stop = Arc::clone(&stop);
                std::thread::spawn(move || {
                    let rd = m.reader();
                    let mut rng = XorShift(0x5000 + i);
                    let mut hits = 0u64;
                    while !stop.load(Ordering::Relaxed) {
                        let k = str_key_of(rng.next() % 512);
                        if let Some(v) = rd.get(&k) {
                            assert_eq!(v, str_val_of(&k), "torn value for {k:?}");
                            hits += 1;
                        }
                    }
                    hits
                })
            })
            .collect();

        let mut rng = XorShift(0xC0FE);
        let mut model = BTreeMap::new();
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_millis(300) {
            for _ in 0..2000 {
                let k = str_key_of(rng.next() % 512);
                if rng.next() % 2 == 0 {
                    m.insert(&k, str_val_of(&k));
                    model.insert(k, ());
                } else {
                    m.remove(&k);
                    model.remove(&k);
                }
            }
            // Tear the whole tree down under the readers, then rebuild
            // some of it — the dispose_tree/EBR path under fire.
            m.clear();
            model.clear();
            for idx in 0..64 {
                let k = str_key_of(idx);
                m.insert(&k, str_val_of(&k));
                model.insert(k, ());
            }
        }
        stop.store(true, Ordering::Relaxed);
        let mut total_hits = 0;
        for r in readers {
            total_hits += r.join().expect("reader panicked");
        }
        assert!(total_hits > 0, "readers observed nothing");

        let rd = m.reader();
        for k in model.keys() {
            assert_eq!(rd.get(k), Some(str_val_of(k)), "model key {k:?}");
        }
        assert_eq!(m.len(), model.len() as u64);
    }

    /// A reader must observe one of the values a key actually held while
    /// the writer flips it via the in-place suffix value update, across a
    /// suffix split forced mid-run.
    #[test]
    fn concurrent_str_overwrite_is_atomic() {
        let m = Arc::new(SyncExpanseStrMap::new());
        let key = b"tenant:0000000042:routing-table-entry".to_vec();
        // Diverges after a shared 24-byte prefix: inserting it forces the
        // suffix split path (publish child + retire old suffix).
        let sibling = b"tenant:0000000042:quota-counters".to_vec();
        let (a, b) = (0x1111_2222_3333_4444u64, 0xAAAA_BBBB_CCCC_DDDDu64);
        m.insert(&key, a);
        let stop = Arc::new(AtomicBool::new(false));
        let readers: Vec<_> = (0..3)
            .map(|_| {
                let m = Arc::clone(&m);
                let stop = Arc::clone(&stop);
                let key = key.clone();
                std::thread::spawn(move || {
                    let rd = m.reader();
                    while !stop.load(Ordering::Relaxed) {
                        let v = rd.get(&key).expect("key always present");
                        assert!(v == a || v == b, "torn overwrite state: {v:#x}");
                    }
                })
            })
            .collect();

        let start = std::time::Instant::now();
        let mut flip = false;
        let mut sibling_in = false;
        while start.elapsed() < std::time::Duration::from_millis(250) {
            for _ in 0..500 {
                flip = !flip;
                m.insert(&key, if flip { b } else { a });
            }
            sibling_in = !sibling_in;
            if sibling_in {
                m.insert(&sibling, 7);
            } else {
                m.remove(&sibling);
            }
        }
        stop.store(true, Ordering::Relaxed);
        for r in readers {
            r.join().expect("reader panicked");
        }
    }

    /// Deep keys through the concurrent wrapper: the deferred teardown
    /// (`dispose_tree`) must stay iterative — run on a small stack so a
    /// regression to recursion fails loudly.
    #[test]
    fn sync_str_deep_keys_dispose_iteratively() {
        let handle = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let m = SyncExpanseStrMap::new();
                let key = vec![b'k'; 64 * 1024];
                let mut sibling = key.clone();
                *sibling.last_mut().expect("non-empty") = b'z';
                assert_eq!(m.insert(&key, 1), None);
                assert_eq!(m.insert(&sibling, 2), None);
                assert_eq!(m.get(&key), Some(1));
                assert_eq!(m.get(&sibling), Some(2));
                m.clear(); // deferred subtree disposal, 8k+ nodes deep
                assert!(m.is_empty());
                drop(m); // collector drains the retired chain
            })
            .expect("spawn");
        handle
            .join()
            .expect("deferred deep-key disposal overflowed the stack");
    }

    /// Regression guard for the slab-migration hazard: wrapping a
    /// **populated** single-threaded blob map must rebuild its index
    /// through a pre-deferred allocator — attaching the original
    /// (slab-carved) allocator to the collector corrupts the heap when
    /// retired blocks are later freed individually.
    #[test]
    fn sync_blob_wraps_populated_map() {
        let mut plain = ExpanseBlobMap::with_chunk_size(4096);
        for k in 0..300u64 {
            plain
                .insert(k, &blob_payload_of(k), blob_meta_of(k))
                .unwrap();
        }
        let m = SyncExpanseBlobMap::from(plain);
        let mut rd = m.reader();
        for k in 0..300u64 {
            let guard = rd.pin();
            let (view, meta) = guard.get(k).expect("wrapped key present");
            assert_eq!(view.as_bytes(), &blob_payload_of(k)[..]);
            assert_eq!(meta, blob_expected_meta(k));
        }
        // Mutations, compaction and teardown run against the rebuilt,
        // fully deferred structure.
        for k in 0..150u64 {
            m.remove(k);
        }
        m.compact().unwrap();
        m.insert(1000, &blob_payload_of(1000), blob_meta_of(1000))
            .unwrap();
        assert_eq!(m.len(), 151);
        m.clear();
        drop(m);
    }

    /// The string-map twin of `sync_blob_wraps_populated_map`.
    #[test]
    fn sync_str_wraps_populated_map() {
        let mut plain = ExpanseStrMap::new();
        for idx in 0..300u64 {
            let k = str_key_of(idx);
            plain.insert(&k, str_val_of(&k));
        }
        let expected = plain.len();
        let m = SyncExpanseStrMap::from(plain);
        assert_eq!(m.len(), expected);
        let rd = m.reader();
        for idx in 0..300u64 {
            let k = str_key_of(idx);
            assert_eq!(rd.get(&k), Some(str_val_of(&k)), "wrapped key {k:?}");
        }
        for idx in 0..150u64 {
            m.remove(&str_key_of(idx));
        }
        m.insert(b"post-wrap", 7);
        assert_eq!(m.get(b"post-wrap"), Some(7));
        m.clear();
        drop(m);
    }

    /// Deterministic payload derived from a key: lengths sweep the inline
    /// (< 8 bytes) and arena regimes, and the bytes are a keyed PRNG stream
    /// so a torn or misresolved read cannot accidentally match.
    fn blob_payload_of(k: u64) -> Vec<u8> {
        let len = (k.wrapping_mul(7919) % 160) as usize;
        let mut v = Vec::with_capacity(len);
        let mut x = k ^ 0xD1B5_4A32_D192_ED03;
        for _ in 0..len {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            v.push((x >> 56) as u8);
        }
        v
    }

    fn blob_meta_of(k: u64) -> u32 {
        (k as u32).wrapping_mul(0x9E37_79B9) & ValueSlot::ARENA_META_MAX
    }

    /// Inline payloads carry no metadata field and read back as 0.
    fn blob_expected_meta(k: u64) -> u32 {
        if blob_payload_of(k).len() <= 7 {
            0
        } else {
            blob_meta_of(k)
        }
    }

    #[test]
    fn sync_blob_single_thread_agrees_with_model() {
        let m = SyncExpanseBlobMap::with_chunk_size(4096);
        let mut model: BTreeMap<u64, Vec<u8>> = BTreeMap::new();
        let mut rng = XorShift(0x88);
        for i in 0..4000u64 {
            let k = rng.next() % 2048;
            match rng.next() % 3 {
                0 => {
                    m.insert(k, &blob_payload_of(k), blob_meta_of(k)).unwrap();
                    model.insert(k, blob_payload_of(k));
                }
                1 => assert_eq!(m.remove(k), model.remove(&k).is_some()),
                _ => {
                    let got = m.get(k);
                    let want = model.get(&k);
                    match (&got, want) {
                        (None, None) => {}
                        (Some((bytes, meta)), Some(want)) => {
                            assert_eq!(bytes, want);
                            assert_eq!(*meta, blob_expected_meta(k));
                            assert_eq!(m.get_meta(k), Some(blob_expected_meta(k)));
                            assert!(m.contains_key(k));
                        }
                        _ => panic!("mismatch for {k}: {got:?} vs {want:?}"),
                    }
                }
            }
            if i % 500 == 499 {
                m.compact().unwrap();
            }
            assert_eq!(m.len(), model.len() as u64);
        }
        m.with_locked(|inner| inner.index().validate());
    }

    /// The issue #219 gate for epoch-pinned chunk retirement: a payload view
    /// taken before a compaction must keep reading the original (retired but
    /// EBR-live, never rewritten) bytes after the compaction relocated the
    /// record and dropped its chunk.
    #[test]
    fn sync_blob_guard_view_survives_compaction() {
        let m = SyncExpanseBlobMap::with_chunk_size(4096);
        let payload: Vec<u8> = (0..200u32).map(|i| (i * 31) as u8).collect();
        m.insert(1, &payload, 42).unwrap();
        // Garbage records so the compaction genuinely relocates into fresh
        // chunks and retires several old ones.
        for k in 2..40 {
            m.insert(k, &[0xEE; 300], 0).unwrap();
        }
        for k in 2..40 {
            assert!(m.remove(k));
        }
        let mut rd = m.reader();
        let guard = rd.pin();
        let (view, meta) = guard.get(1).expect("present");
        assert!(view.is_arena());
        assert_eq!(meta, 42);
        let stats = m.compact().unwrap();
        assert!(stats.live_records_moved >= 1);
        assert!(stats.chunks_before > stats.chunks_after);
        // The pinned borrow still reads the retired chunk's bytes.
        assert_eq!(view.as_bytes(), &payload[..]);
        drop(guard);
        // A fresh read resolves the relocated record through the new table.
        let guard = rd.pin();
        let (view, meta) = guard.get(1).expect("present after compact");
        assert!(view.is_arena());
        assert_eq!(view.as_bytes(), &payload[..]);
        assert_eq!(meta, 42);
    }

    /// Phase-1 gate for `SyncExpanseBlobMap` (issue #219): readers hammer
    /// pinned zero-copy lookups while the writer churns inserts/removes and
    /// periodically compacts the arena (retiring chunks). Every observed
    /// payload must be exactly the key's derived payload — a torn read, a
    /// stale chunk table, or a misresolved locator produces a mismatch.
    #[test]
    fn concurrent_blob_readers_under_churn() {
        let m = Arc::new(SyncExpanseBlobMap::with_chunk_size(4096));
        let stop = Arc::new(AtomicBool::new(false));
        let key_of = |r: &mut XorShift| {
            let base = [0u64, 0x11_2233_4400, 0xFFFF_FF00_0000][(r.next() % 3) as usize];
            base + r.next() % 512
        };

        let readers: Vec<_> = (0..3)
            .map(|i| {
                let m = Arc::clone(&m);
                let stop = Arc::clone(&stop);
                std::thread::spawn(move || {
                    let mut rd = m.reader();
                    let mut rng = XorShift(0x3000 + i);
                    let mut hits = 0u64;
                    while !stop.load(Ordering::Relaxed) {
                        let k = key_of(&mut rng);
                        let guard = rd.pin();
                        if let Some((view, meta)) = guard.get(k) {
                            assert_eq!(
                                view.as_bytes(),
                                &blob_payload_of(k)[..],
                                "torn payload for {k:#x}"
                            );
                            assert_eq!(meta, blob_expected_meta(k), "torn metadata for {k:#x}");
                            hits += 1;
                        }
                    }
                    hits
                })
            })
            .collect();

        let mut rng = XorShift(0xACE1);
        let mut model = BTreeMap::new();
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_millis(300) {
            for _ in 0..2000 {
                let k = key_of(&mut rng);
                if rng.next() % 2 == 0 {
                    m.insert(k, &blob_payload_of(k), blob_meta_of(k)).unwrap();
                    model.insert(k, ());
                } else {
                    m.remove(k);
                    model.remove(&k);
                }
            }
            m.compact().expect("compaction under churn");
        }
        stop.store(true, Ordering::Relaxed);
        let mut total_hits = 0;
        for r in readers {
            total_hits += r.join().expect("reader panicked");
        }
        assert!(total_hits > 0, "readers observed nothing");

        // Final state agrees with the model through the pinned read path.
        let mut rd = m.reader();
        let guard = rd.pin();
        for &k in model.keys() {
            let (view, meta) = guard.get(k).expect("model key present");
            assert_eq!(view.as_bytes(), &blob_payload_of(k)[..]);
            assert_eq!(meta, blob_expected_meta(k));
        }
        drop(guard);
        m.with_locked(|inner| {
            inner.index().validate();
            assert_eq!(inner.len(), model.len() as u64);
        });
    }

    /// A reader must observe one of the payload states a key actually held —
    /// never a mix — while the writer flips it between two arena payloads and
    /// an inline one (the slot word alternates between inline-encoded and
    /// arena-locator forms) and compacts in between.
    #[test]
    fn concurrent_blob_overwrite_is_atomic() {
        let m = Arc::new(SyncExpanseBlobMap::with_chunk_size(4096));
        let a = vec![0xAAu8; 96];
        let b = vec![0xBBu8; 160];
        let c = vec![0xCCu8; 5]; // inline: metadata reads as 0
        let key = 0x1234_5678u64;
        m.insert(key, &a, 1).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let readers: Vec<_> = (0..3)
            .map(|_| {
                let m = Arc::clone(&m);
                let stop = Arc::clone(&stop);
                let (a, b, c) = (a.clone(), b.clone(), c.clone());
                std::thread::spawn(move || {
                    let mut rd = m.reader();
                    while !stop.load(Ordering::Relaxed) {
                        let guard = rd.pin();
                        let (view, meta) = guard.get(key).expect("key always present");
                        let bytes = view.as_bytes();
                        assert!(
                            (bytes == &a[..] && meta == 1)
                                || (bytes == &b[..] && meta == 2)
                                || (bytes == &c[..] && meta == 0),
                            "mixed/torn overwrite state: len={} meta={meta}",
                            bytes.len()
                        );
                    }
                })
            })
            .collect();

        let start = std::time::Instant::now();
        let mut state = 0u8;
        while start.elapsed() < std::time::Duration::from_millis(250) {
            for _ in 0..500 {
                state = (state + 1) % 3;
                match state {
                    0 => m.insert(key, &a, 1).unwrap(),
                    1 => m.insert(key, &b, 2).unwrap(),
                    _ => m.insert(key, &c, 3).unwrap(),
                }
            }
            m.compact().unwrap();
        }
        stop.store(true, Ordering::Relaxed);
        for r in readers {
            r.join().expect("reader panicked");
        }
    }

    /// Every key hashes identically, so the whole map is one collision
    /// bucket: the concurrent bucket-replacement paths (append, remove,
    /// removed-key retirement) all run on it.
    #[derive(Default)]
    struct Degenerate;
    impl std::hash::Hasher for Degenerate {
        fn finish(&self) -> u64 {
            0x42
        }
        fn write(&mut self, _: &[u8]) {}
    }
    impl BuildHasher for Degenerate {
        type Hasher = Degenerate;
        fn build_hasher(&self) -> Degenerate {
            Degenerate
        }
    }

    #[test]
    fn sync_bytes_single_thread_agrees_with_model() {
        let m = SyncExpanseBytesMap::new();
        let mut model: std::collections::HashMap<Vec<u8>, u64> = std::collections::HashMap::new();
        let mut rng = XorShift(0xB17E);
        for i in 0..4000u64 {
            let k = str_key_of(rng.next() % 512);
            match rng.next() % 3 {
                0 => {
                    let v = str_val_of(&k);
                    assert_eq!(m.insert(&k, v), model.insert(k.clone(), v), "ins {k:?}");
                }
                1 => assert_eq!(m.remove(&k), model.remove(&k), "rm {k:?}"),
                _ => assert_eq!(m.get(&k), model.get(&k).copied(), "get {k:?}"),
            }
            if i % 1000 == 999 {
                m.clear();
                model.clear();
            }
            assert_eq!(m.len(), model.len() as u64);
        }
        // Unordered iteration through the locked escape hatch.
        let mut seen = 0u64;
        m.with_locked(|inner| {
            inner.for_each(|k, v| {
                assert_eq!(model.get(k).copied(), Some(v), "iter {k:?}");
                seen += 1;
            });
        });
        assert_eq!(seen, model.len() as u64);
        // The compat slot API through the exclusive escape hatch.
        m.with_locked_mut(|inner| {
            let slot = inner.ins_slot(b"slot-key");
            // SAFETY: slot valid until the next mutation; none happens.
            unsafe { slot.as_ptr().write(77) };
        });
        assert_eq!(m.get(b"slot-key"), Some(77));
    }

    /// The issue #362 gate: readers hammer lock-free point lookups while
    /// the writer churns inserts/removes and periodically clears the
    /// whole map (retiring every bucket under active readers). Every
    /// observed value must be the key's full-key FNV-1a hash — a
    /// misresolved bucket, torn word, or stale key comparison cannot
    /// accidentally pass.
    #[test]
    fn concurrent_bytes_readers_under_churn() {
        let m = Arc::new(SyncExpanseBytesMap::new());
        let stop = Arc::new(AtomicBool::new(false));

        let readers: Vec<_> = (0..3)
            .map(|i| {
                let m = Arc::clone(&m);
                let stop = Arc::clone(&stop);
                std::thread::spawn(move || {
                    let rd = m.reader();
                    let mut rng = XorShift(0x7000 + i);
                    let mut hits = 0u64;
                    while !stop.load(Ordering::Relaxed) {
                        let k = str_key_of(rng.next() % 512);
                        if let Some(v) = rd.get(&k) {
                            assert_eq!(v, str_val_of(&k), "torn value for {k:?}");
                            hits += 1;
                        }
                    }
                    hits
                })
            })
            .collect();

        let mut rng = XorShift(0xFACE);
        let mut model = BTreeMap::new();
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_millis(300) {
            for _ in 0..2000 {
                let k = str_key_of(rng.next() % 512);
                if rng.next() % 2 == 0 {
                    m.insert(&k, str_val_of(&k));
                    model.insert(k, ());
                } else {
                    m.remove(&k);
                    model.remove(&k);
                }
            }
            // Retire every bucket + the whole trie under the readers,
            // then rebuild some of it.
            m.clear();
            model.clear();
            for idx in 0..64 {
                let k = str_key_of(idx);
                m.insert(&k, str_val_of(&k));
                model.insert(k, ());
            }
        }
        stop.store(true, Ordering::Relaxed);
        let mut total_hits = 0;
        for r in readers {
            total_hits += r.join().expect("reader panicked");
        }
        assert!(total_hits > 0, "readers observed nothing");

        let rd = m.reader();
        for k in model.keys() {
            assert_eq!(rd.get(k), Some(str_val_of(k)), "model key {k:?}");
        }
        assert_eq!(m.len(), model.len() as u64);
    }

    /// Overwrite atomicity on the collision-bucket paths: with the
    /// degenerate hasher every key shares one bucket, so the writer's
    /// same-key value flips (in-place word updates) race the structural
    /// bucket replacements caused by churning a third key in and out.
    /// Readers must only ever observe complete states: the flipped key
    /// holds one of its two values, and a stable sibling entry in the
    /// same bucket never tears.
    #[test]
    fn concurrent_bytes_overwrite_is_atomic() {
        let m = Arc::new(SyncExpanseBytesMap::with_hasher(Degenerate));
        let key = b"flipping-key".to_vec();
        let stable = b"stable-sibling".to_vec();
        let churn = b"churn-key".to_vec();
        let (a, b) = (0x1111_2222_3333_4444u64, 0xAAAA_BBBB_CCCC_DDDDu64);
        let stable_val = 0x5757_5757_5757_5757u64;
        m.insert(&key, a);
        m.insert(&stable, stable_val);
        let stop = Arc::new(AtomicBool::new(false));
        let readers: Vec<_> = (0..3)
            .map(|_| {
                let m = Arc::clone(&m);
                let stop = Arc::clone(&stop);
                let (key, stable) = (key.clone(), stable.clone());
                std::thread::spawn(move || {
                    let rd = m.reader();
                    while !stop.load(Ordering::Relaxed) {
                        let v = rd.get(&key).expect("flipped key always present");
                        assert!(v == a || v == b, "torn overwrite state: {v:#x}");
                        let s = rd.get(&stable).expect("stable key always present");
                        assert_eq!(s, stable_val, "stable sibling torn: {s:#x}");
                    }
                })
            })
            .collect();

        let start = std::time::Instant::now();
        let mut flip = false;
        let mut churn_in = false;
        while start.elapsed() < std::time::Duration::from_millis(250) {
            for _ in 0..500 {
                flip = !flip;
                m.insert(&key, if flip { b } else { a });
            }
            // Structural bucket replacement under the readers.
            churn_in = !churn_in;
            if churn_in {
                m.insert(&churn, 7);
            } else {
                m.remove(&churn);
            }
        }
        stop.store(true, Ordering::Relaxed);
        for r in readers {
            r.join().expect("reader panicked");
        }
    }

    /// The bytes-map twin of `sync_{blob,str}_wraps_populated_map`:
    /// wrapping a populated single-threaded map must rebuild it through
    /// a pre-deferred one — attaching the original (slab-carved) hash
    /// trie to the collector corrupts the heap when retired blocks are
    /// later freed individually.
    #[test]
    fn sync_bytes_wraps_populated_map() {
        let mut plain = ExpanseBytesMap::new();
        for idx in 0..300u64 {
            let k = str_key_of(idx);
            plain.insert(&k, str_val_of(&k));
        }
        let expected = plain.len();
        let m = SyncExpanseBytesMap::from(plain);
        assert_eq!(m.len(), expected);
        let rd = m.reader();
        for idx in 0..300u64 {
            let k = str_key_of(idx);
            assert_eq!(rd.get(&k), Some(str_val_of(&k)), "wrapped key {k:?}");
        }
        for idx in 0..150u64 {
            m.remove(&str_key_of(idx));
        }
        m.insert(b"post-wrap", 7);
        assert_eq!(m.get(b"post-wrap"), Some(7));
        m.clear();
        drop(m);
    }
}
