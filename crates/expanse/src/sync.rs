//! Phase 7: concurrent wrappers — one writer, many optimistic readers.
//!
//! [`SyncExpanseSet`] / [`SyncExpanseMap`] / [`SyncExpanseBlobMap`] wrap the
//! single-threaded structures with the `occ` protocol:
//!
//! - **Writers** serialize on a mutex. The tree-level [`SeqVersion`] (in
//!   the [`Collector`]) covers the root state: on the set and map the
//!   engine brackets root-state writes itself and ordinary writes never
//!   touch it, while the string / bytes / blob wrappers still bracket the
//!   whole operation with it. Inside the tree every store is bracketed by
//!   the version of the node containing its address (`occ::Cover` — active
//!   only for concurrently shared trees), one frame at a time, so a node's
//!   word is odd only while that node is being written, never for a whole
//!   descent (#568 PR 3). All node frees route through the epoch
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
//! The per-node version words are live protocol state (readers validate
//! against them hand-over-hand); the two bitmap-leaf words are reserved.

use crate::blobmap::{ArenaError, CompactionStats, ExpanseBlobMap};
use crate::bytesmap::ExpanseBytesMap;
use crate::leaf;
use crate::map::ExpanseMap;
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1, LeafBitmapL};
use crate::occ::{Collector, Pin, Reader, SeqVersion};
use crate::set::ExpanseSet;
use crate::slot::{SlotTag, ValueSlot};
use crate::strmap::{ExpanseStrMap, NulFreeStr};
use crate::types::{BRANCH_L3_CAP, BRANCH_L7_CAP, EdgeTag, EdgeType, ImmedType, Key, digit};
use core::cell::UnsafeCell;
use std::hash::{BuildHasher, RandomState};
use std::sync::{Arc, Mutex, MutexGuard};

#[cfg(not(loom))]
type AtomicUsize = core::sync::atomic::AtomicUsize;
#[cfg(loom)]
type AtomicUsize = loom::sync::atomic::AtomicUsize;

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
    },
}

/// A validated read failed because the version moved; restart.
pub(crate) struct Retry;

/// Bounded optimistic restarts before falling back to the writer lock.
const MAX_RETRIES: usize = 64;

/// Writes between epoch-advance attempts (`Collector::try_advance`).
///
/// `try_advance` is documented writer-side and *amortized* — "call once
/// per mutation batch" — but `Shared::write` called it on every single
/// mutation, **inside the writer critical section**. It takes the
/// collector's reader-registry mutex, scans every registered reader slot
/// (a separately allocated cache line each, so the scan bounces lines
/// between cores), then takes a garbage bin mutex and recycles the stale
/// bin. A stack sample of a 16-thread 50/50 write-mix run attributed
/// about as much on-CPU time to `try_advance` as to the trie mutation
/// itself — i.e. it roughly doubled the length of the one section every
/// writer must serialize on.
///
/// Batching it costs only *latency* of reclamation, never safety:
/// advancing less often keeps retired blocks alive strictly longer, and
/// the deferred garbage is bounded by the mutations since the last
/// attempt (`Collector::drain` sweeps the remainder when the wrapper is
/// dropped). 32 sits at the knee — a sweep over 1/4/8/32/128/1024 on a
/// 16-thread mix reached ~85% of the total available gain by 32, with
/// 1024 adding only a few more points for 32x the retained garbage.
#[cfg(not(any(feature = "advance-every-4096", feature = "advance-never")))]
pub(crate) const ADVANCE_EVERY: u64 = 32;
/// Diagnostic: a long advance interval, to measure what the epoch-advance
/// scan costs the critical section (compare against the default).
#[cfg(all(feature = "advance-every-4096", not(feature = "advance-never")))]
pub(crate) const ADVANCE_EVERY: u64 = 4096;
// `advance-never` (diagnostic): the write path never attempts an epoch advance,
// so there is no interval constant at all. Only sound for a workload that retires
// nothing (pure overwrites); with retirements the bins grow without bound.
// Features are additive, so a build that enables both variants gets this one:
// the interval constant is gated out rather than left defined and unused.

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
            // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
            Cover::Node(p, s) => unsafe {
                crate::occ::node_validate(crate::occ::version_cell(*p), *s)
            },
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
                // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
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
            // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
            let v = unsafe {
                ptr.add(crate::map::leaf_values_offset(pop))
                    .cast::<u64>()
                    .add(lo)
                    .read()
            };
            chk!();
            return Ok(Some(v));
        }
        RootSnapshot::Tree { top } => (top, 8),
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
                    let Some(nsnap) =
                        // SAFETY: version cell is within an EBR-live node allocation.
                        (unsafe { crate::occ::node_sample(crate::occ::version_cell(vp)) })
                    else {
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
                        if !unsafe {
                            crate::occ::node_validate(crate::occ::version_cell(vp), nsnap)
                        } {
                            return Err(Retry);
                        }
                        return Ok(None);
                    }
                    let d = digit(key, bl);
                    let Some(slot) = digits[..num].iter().position(|&x| x == d) else {
                        // SAFETY: live version field (EBR).
                        if !unsafe {
                            crate::occ::node_validate(crate::occ::version_cell(vp), nsnap)
                        } {
                            return Err(Retry);
                        }
                        return Ok(None);
                    };
                    // SAFETY: `slot < num <= capacity`; in-bounds read of
                    // the EBR-live node, validated just below.
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    edge = unsafe { edges_base.add(slot).read() };
                    // SAFETY: live version field (EBR).
                    if !unsafe { crate::occ::node_validate(crate::occ::version_cell(vp), nsnap) } {
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
                    let Some(nsnap) =
                        // SAFETY: version cell is within an EBR-live node allocation.
                        (unsafe { crate::occ::node_sample(crate::occ::version_cell(vp)) })
                    else {
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
                        if !unsafe {
                            crate::occ::node_validate(crate::occ::version_cell(vp), nsnap)
                        } {
                            return Err(Retry);
                        }
                        return Ok(None);
                    }
                    if !bit {
                        // SAFETY: live version field (EBR).
                        if !unsafe {
                            crate::occ::node_validate(crate::occ::version_cell(vp), nsnap)
                        } {
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
                    if !unsafe { crate::occ::node_validate(crate::occ::version_cell(vp), nsnap) } {
                        return Err(Retry);
                    }
                    // SAFETY: bitmap/subarray pair validated consistent
                    // just above → the subarray holds at least `rank + 1`
                    // EBR-live edges.
                    // SAFETY: pointer arithmetic and destination buffer bounds are valid under locked parent.
                    edge = unsafe { sub.add(rank).read() };
                    // The edge copy itself must also be covered: re-check
                    // before the next iteration dereferences it.
                    // SAFETY: live version field (EBR).
                    if !unsafe { crate::occ::node_validate(crate::occ::version_cell(vp), nsnap) } {
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
                    let Some(nsnap) =
                        // SAFETY: version cell is within an EBR-live node allocation.
                        (unsafe { crate::occ::node_sample(crate::occ::version_cell(vp)) })
                    else {
                        return Err(Retry);
                    };
                    let d = digit(key, level);
                    // SAFETY: EBR-live BranchU; direct 256-slot index.
                    edge = unsafe { (*node).edges.as_ptr().add(d as usize).read() };
                    // SAFETY: live version field (EBR).
                    if !unsafe { crate::occ::node_validate(crate::occ::version_cell(vp), nsnap) } {
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
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
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
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let found = unsafe { leaf::search(keys, pop, kb, key) };
                    chk!();
                    let Some(slot) = found else {
                        return Ok(None);
                    };
                    if !MAP {
                        return Ok(Some(0));
                    }
                    #[cfg(test)]
                    test_hooks::before_leaf_value();
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
                // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                let v = unsafe { edge.node_ptr().cast::<u64>().add(slot).read() };
                chk!();
                return Ok(Some(v));
            }
        }
    }
}

/// A field on its own cache line when the `lock-padded` diagnostic feature is
/// on; the bare field otherwise. It wraps `Shared`'s writer mutex, writer gate,
/// tree population counter and tree version word, so the feature measures what
/// padding these shared/writer words away from each other and from the wrapper's
/// other writer-private words costs (the version already heads the struct on the
/// readers' line in either configuration).
/// [`layout_report`] says where each field landed.
#[cfg(feature = "lock-padded")]
#[derive(Debug)]
#[repr(align(64))]
pub(crate) struct Line<X>(X);
#[cfg(feature = "lock-padded")]
impl<X> core::ops::Deref for Line<X> {
    type Target = X;
    fn deref(&self) -> &X {
        &self.0
    }
}
#[cfg(feature = "lock-padded")]
impl<X> From<X> for Line<X> {
    fn from(x: X) -> Self {
        Self(x)
    }
}
/// See the `lock-padded` twin: the bare field.
#[cfg(not(feature = "lock-padded"))]
pub(crate) type Line<X> = X;
/// Wraps a field for [`Line`] whichever way the feature resolves.
#[cfg(feature = "lock-padded")]
#[inline]
fn line<X>(x: X) -> Line<X> {
    Line(x)
}
/// See the `lock-padded` twin: the bare field.
#[cfg(not(feature = "lock-padded"))]
#[inline]
fn line<X>(x: X) -> Line<X> {
    x
}

/// What every wrapped engine offers `Shared`: a way to bind the tree-level
/// version word to its allocator(s) once the wrapper is boxed (#568 PR 3).
pub(crate) trait SharedTree {
    /// # Safety
    ///
    /// As `NodeAlloc::bind_tree_word`: `word` outlives every operation on
    /// this tree.
    unsafe fn bind_tree_word(&self, word: *const SeqVersion);

    /// Total entries currently stored.
    fn tree_pop(&self) -> u64;

    /// Sets tree population when synchronizing from concurrent writes.
    fn set_tree_pop(&mut self, _pop: u64) {}

    /// Resets internal path cursors to ensure stale node pointers are not reused.
    fn clear_path(&self) {}
}

impl SharedTree for ExpanseMap {
    unsafe fn bind_tree_word(&self, word: *const SeqVersion) {
        // SAFETY: forwarded contract.
        unsafe { self.occ_root().1.bind_tree_word(word) };
    }

    fn tree_pop(&self) -> u64 {
        self.len()
    }

    fn set_tree_pop(&mut self, pop: u64) {
        ExpanseMap::set_tree_pop(self, pop);
    }

    fn clear_path(&self) {
        ExpanseMap::clear_path(self);
    }
}

impl SharedTree for ExpanseSet {
    unsafe fn bind_tree_word(&self, word: *const SeqVersion) {
        // SAFETY: forwarded contract.
        unsafe { self.occ_root().1.bind_tree_word(word) };
    }

    fn tree_pop(&self) -> u64 {
        self.len()
    }

    fn set_tree_pop(&mut self, pop: u64) {
        ExpanseSet::set_tree_pop(self, pop);
    }

    fn clear_path(&self) {
        ExpanseSet::clear_path(self);
    }
}

impl SharedTree for ExpanseStrMap {
    unsafe fn bind_tree_word(&self, word: *const SeqVersion) {
        // SAFETY: forwarded contract.
        unsafe { ExpanseStrMap::bind_tree_word(self, word) };
    }

    fn tree_pop(&self) -> u64 {
        self.len()
    }
}

impl<S: BuildHasher> SharedTree for ExpanseBytesMap<S> {
    unsafe fn bind_tree_word(&self, word: *const SeqVersion) {
        // SAFETY: forwarded contract.
        unsafe { ExpanseBytesMap::bind_tree_word(self, word) };
    }

    fn tree_pop(&self) -> u64 {
        self.len()
    }
}

impl SharedTree for ExpanseBlobMap {
    unsafe fn bind_tree_word(&self, word: *const SeqVersion) {
        // SAFETY: forwarded contract.
        unsafe { ExpanseBlobMap::bind_tree_word(self, word) };
    }

    fn tree_pop(&self) -> u64 {
        self.len()
    }
}

/// What `Shared::write_root_covered` asks of an engine: whether its root is
/// a level-8 trie right now (read under the writer lock).
pub(crate) trait RootState {
    fn root_is_tree(&self) -> bool;
}

impl RootState for ExpanseMap {
    #[inline(always)]
    fn root_is_tree(&self) -> bool {
        ExpanseMap::root_is_tree(self)
    }
}

impl RootState for ExpanseSet {
    #[inline(always)]
    fn root_is_tree(&self) -> bool {
        ExpanseSet::root_is_tree(self)
    }
}

/// The shared writer/reader state behind every wrapper. `repr(C)` and
/// line-aligned, and always boxed (see [`Shared::new`]): the tree-level
/// version word heads the struct, on the cache line the root snapshot
/// shares, so a reader's sample, root load and validate touch one line and
/// reach the word at a fixed offset from the pointer they already hold; the
/// writer-private words (mutex, holder token, advance tick) sit after
/// `inner`, on a line no reader samples. The engine reaches the word
/// through `NodeAlloc::bind_tree_word`, which is why the block is boxed —
/// its address must not change when the wrapper moves. `layout_report`
/// names the offsets and its test pins the invariant (#568 PR 3).
#[repr(C, align(64))]
struct Shared<T> {
    version: Line<SeqVersion>,
    inner: UnsafeCell<T>,
    tree_pop: Line<core::sync::atomic::AtomicU64>,
    collector: Arc<Collector>,
    write: Line<Mutex<()>>,
    #[cfg(feature = "std")]
    gate: Line<crate::occ::WriterGate>,
    #[cfg(feature = "std")]
    fallback_mutex: Mutex<()>,
    #[cfg(feature = "std")]
    writers: Mutex<Vec<Arc<AtomicUsize>>>,
    /// Token of the thread that last held `write`, for the `Handoffs`
    /// counter. Read and written only under the lock — no coherence traffic
    /// beyond the line it shares (which [`layout_report`] names). Diagnostic
    /// only.
    #[cfg(feature = "occ-stats")]
    last_holder: UnsafeCell<u64>,
    /// Mutations since the last epoch-advance attempt (see
    /// [`ADVANCE_EVERY`]). Read and written only by [`Shared::write`],
    /// which holds `write` — a plain counter, not an atomic, so it adds
    /// no coherence traffic to the critical section.
    // Kept under `advance-never` even though nothing reads it: the field stays so
    // `Shared`'s layout -- and `layout_report()` -- is the same across variants.
    #[cfg_attr(feature = "advance-never", allow(dead_code))]
    advance_tick: UnsafeCell<u64>,
}

// SAFETY: the OCC protocol above is exactly what makes the inner tree
// shareable — writers are serialized by the mutex + version brackets, and
// readers only act on validated, EBR-live data. `advance_tick` is touched
// only by `write`, which holds that same mutex, so it is never aliased.
unsafe impl<T: Send> Send for Shared<T> {}
// SAFETY: as above.
unsafe impl<T: Send> Sync for Shared<T> {}

impl<T> Drop for Shared<T> {
    fn drop(&mut self) {
        self.collector
            .alive
            .store(false, core::sync::atomic::Ordering::Release);
        self.collector.drain();
    }
}

impl<T: SharedTree> Shared<T> {
    /// Wraps `inner`, handing every allocation source `attach` names over to
    /// a fresh epoch collector (deferred reclamation).
    fn new(inner: T, attach: impl FnOnce(&T, &Arc<Collector>)) -> Box<Self> {
        let collector = Arc::new(Collector::new());
        attach(&inner, &collector);
        Self::with_collector(inner, collector)
    }

    /// Wraps `inner` around an existing collector — for construction paths
    /// that must defer allocators *while building* `inner` (a populated
    /// structure is shared by rebuilding it through pre-deferred
    /// allocators; see `NodeAlloc::defer_to`). Boxed, then the tree word is
    /// bound to `inner`'s allocators at its final address.
    fn with_collector(inner: T, collector: Arc<Collector>) -> Box<Self> {
        let initial_pop = inner.tree_pop();
        let shared = Box::new(Self {
            version: line(SeqVersion::new()),
            inner: UnsafeCell::new(inner),
            tree_pop: line(core::sync::atomic::AtomicU64::new(initial_pop)),
            collector,
            write: line(Mutex::new(())),
            #[cfg(feature = "std")]
            gate: line(crate::occ::WriterGate::new()),
            #[cfg(feature = "std")]
            fallback_mutex: Mutex::new(()),
            #[cfg(feature = "std")]
            writers: Mutex::new(Vec::new()),
            #[cfg(feature = "occ-stats")]
            last_holder: UnsafeCell::new(0),
            advance_tick: UnsafeCell::new(0),
        });
        // SAFETY: the word and the tree live in this one heap block, which
        // the wrapper owns and never opens; `inner` drops before `version`
        // (field order) and nothing hands the tree out.
        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
        unsafe {
            shared
                .inner_ref()
                .bind_tree_word(core::ptr::from_ref(shared.version()))
        };
        shared
    }
}

impl<T: SharedTree> Shared<T> {
    /// The tree-level version word.
    #[inline(always)]
    fn version(&self) -> &SeqVersion {
        &self.version
    }

    /// The wrapped engine, for construction-time calls that need no lock.
    #[inline(always)]
    fn inner_ref(&self) -> &T {
        // SAFETY: a shared borrow of the engine; callers use it only where
        // no writer can be running (construction).
        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
        unsafe { &*self.inner.get() }
    }

    #[cfg(feature = "std")]
    pub(crate) fn enter_writer(&self) -> Option<crate::occ::WriterGuard<'_>> {
        use std::cell::RefCell;
        std::thread_local! {
            static REGISTERED: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
            static SLOT: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
        }
        let slot = SLOT.with(Arc::clone);
        let key = self.gate.id();
        let needs_reg = REGISTERED.with(|reg| {
            let mut r = reg.borrow_mut();
            if r.contains(&key) {
                false
            } else {
                if r.len() >= 128 {
                    r.remove(0);
                }
                r.push(key);
                true
            }
        });
        if needs_reg {
            let mut writers = self.writers.lock().expect("writers poisoned");
            writers.retain(|s| Arc::strong_count(s) > 1);
            if !writers.iter().any(|s| Arc::ptr_eq(s, &slot)) {
                writers.push(Arc::clone(&slot));
            }
        }
        // SAFETY: slot is allocated in a thread-local Arc and remains pinned for the duration of enter_writer.
        let slot_ref = unsafe { &*(&*slot as *const AtomicUsize) };
        self.gate.enter_writer(slot_ref)
    }

    #[cfg(feature = "std")]
    pub(crate) fn quiesce_writers(&self) {
        self.gate.close();
        let slots = {
            let mut writers = self.writers.lock().expect("writers poisoned");
            writers.retain(|s| Arc::strong_count(s) > 1);
            writers.clone()
        };
        for slot in &slots {
            while slot.load(core::sync::atomic::Ordering::Relaxed) != 0 {
                core::hint::spin_loop();
                #[cfg(loom)]
                loom::thread::yield_now();
            }
        }
    }

    #[cfg(feature = "std")]
    #[inline(always)]
    pub(crate) fn reopen_gate(&self) {
        self.gate.open();
    }

    #[cfg(feature = "std")]
    pub(crate) fn enter_writer_blocking(&self) -> crate::occ::WriterGuard<'_> {
        loop {
            if let Some(guard) = self.enter_writer() {
                return guard;
            }
            core::hint::spin_loop();
            #[cfg(loom)]
            loom::thread::yield_now();
        }
    }

    /// Executes `f` while holding a thread-local EBR reader pin registered with this
    /// tree's collector. Hoists `Collector::register()` so subsequent mutations on this
    /// thread take zero mutexes and perform zero heap allocations.
    #[cfg(feature = "std")]
    pub(crate) fn with_writer_pin<R>(&self, f: impl FnOnce() -> R) -> R {
        use std::cell::RefCell;
        std::thread_local! {
            static WRITER_READERS: RefCell<Vec<(usize, crate::occ::Reader)>> = const { RefCell::new(Vec::new()) };
        }
        let key = Arc::as_ptr(&self.collector) as usize;
        WRITER_READERS.with(|cell| {
            let mut vec = cell.borrow_mut();
            let idx = if let Some(pos) = vec.iter().position(|(k, _)| *k == key) {
                // True LRU on hit: promote accessed entry to the back (MRU) if not already there.
                // Does NOT run `retain` on fast-path hits (N2).
                if pos < vec.len() - 1 {
                    let item = vec.remove(pos);
                    vec.push(item);
                    vec.len() - 1
                } else {
                    pos
                }
            } else {
                // Cache miss: prune dead collectors whose Shared tree has been dropped (M4/N1).
                vec.retain(|(_, r)| !r.is_orphan());
                if vec.len() >= 16 {
                    vec.remove(0); // Evict LRU entry
                }
                let r = self.collector.register();
                vec.push((key, r));
                vec.len() - 1
            };
            let _pin = vec[idx].1.pin();
            f()
        })
    }

    /// Runs one mutation under the writer lock and version bracket, and
    /// attempts an epoch advance once every [`ADVANCE_EVERY`] mutations.
    ///
    /// The tick lives behind the writer mutex (a plain `Cell` read and
    /// write, no atomic traffic): this is the one place that mutates it
    /// and the lock is already held.
    fn write<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        crate::occ_stats::bump(crate::occ_stats::Stat::WriteOps);
        let _g = self.write.lock().expect("writer lock poisoned");
        #[cfg(feature = "occ-stats")]
        {
            // SAFETY: the writer mutex serializes this word.
            let holder = unsafe { &mut *self.last_holder.get() };
            let me = thread_token();
            if *holder != me {
                if *holder != 0 {
                    crate::occ_stats::bump(crate::occ_stats::Stat::Handoffs);
                }
                *holder = me;
            }
        }
        self.version().begin();
        #[cfg(debug_assertions)]
        crate::alloc::bracket_stack::enter(self.tree_cover_addr());
        crate::occ_stats::op_begin();
        // SAFETY: the writer mutex makes this the only mutable borrow.
        let inner = unsafe { &mut *self.inner.get() };
        inner.set_tree_pop(self.tree_pop.load(core::sync::atomic::Ordering::Relaxed));
        let r = f(inner);
        inner.clear_path();
        self.tree_pop
            .store(inner.tree_pop(), core::sync::atomic::Ordering::Relaxed);
        crate::occ_stats::op_end();
        #[cfg(debug_assertions)]
        crate::alloc::bracket_stack::leave(self.tree_cover_addr());
        self.version().end();
        #[cfg(not(feature = "advance-never"))]
        {
            // SAFETY: as above — the writer mutex serializes this counter.
            let tick = unsafe { &mut *self.advance_tick.get() };
            *tick += 1;
            if *tick >= ADVANCE_EVERY {
                *tick = 0;
                self.collector.try_advance();
            }
        }
        r
    }

    /// One mutation under the writer lock, bracketed with the tree-level
    /// word only while the root is not a tree (#568 PR 3). In root-leaf
    /// state every store is a root-state write — the leaf in place, its
    /// reallocation, the promotion to a tree — so the wrapper holds the word
    /// for the whole operation, exactly as `write` does. In tree state an
    /// ordinary insert or remove never touches the word: the engine brackets
    /// every store with the version of the node that holds it, and the two
    /// root-state changes a remove can make (to empty, or a condense back to
    /// a root leaf) bracket themselves. The unshared path pays nothing for
    /// this: the decision is one branch here, on the shared path only.
    fn write_root_covered<R>(&self, f: impl FnOnce(&mut T) -> R) -> R
    where
        T: RootState,
    {
        crate::occ_stats::bump(crate::occ_stats::Stat::WriteOps);
        #[cfg(feature = "std")]
        let _fallback = self.fallback_mutex.lock().expect("fallback mutex poisoned");
        #[cfg(feature = "std")]
        self.quiesce_writers();
        let _g = self.write.lock().expect("writer lock poisoned");
        #[cfg(feature = "occ-stats")]
        {
            // SAFETY: the writer mutex serializes this word.
            let holder = unsafe { &mut *self.last_holder.get() };
            let me = thread_token();
            if *holder != me {
                if *holder != 0 {
                    crate::occ_stats::bump(crate::occ_stats::Stat::Handoffs);
                }
                *holder = me;
            }
        }
        crate::occ_stats::op_begin();
        // SAFETY: the writer mutex makes this the only mutable borrow.
        let inner = unsafe { &mut *self.inner.get() };
        inner.set_tree_pop(self.tree_pop.load(core::sync::atomic::Ordering::Relaxed));
        // Read under the lock: the root state is the writer's to change.
        let r = if inner.root_is_tree() {
            f(inner)
        } else {
            self.version().begin();
            #[cfg(debug_assertions)]
            crate::alloc::bracket_stack::enter(self.tree_cover_addr());
            let r = f(inner);
            #[cfg(debug_assertions)]
            crate::alloc::bracket_stack::leave(self.tree_cover_addr());
            self.version().end();
            r
        };
        inner.clear_path();
        self.tree_pop
            .store(inner.tree_pop(), core::sync::atomic::Ordering::Relaxed);
        crate::occ_stats::op_end();
        #[cfg(not(feature = "advance-never"))]
        {
            // SAFETY: as in `write` — the writer mutex serializes this counter.
            let tick = unsafe { &mut *self.advance_tick.get() };
            *tick += 1;
            if *tick >= ADVANCE_EVERY {
                *tick = 0;
                self.collector.try_advance();
            }
        }
        drop(_g);
        #[cfg(feature = "std")]
        self.reopen_gate();
        r
    }

    /// The tree cover's sentinel on the debug bracket stack.
    #[cfg(debug_assertions)]
    fn tree_cover_addr(&self) -> *const u32 {
        core::ptr::from_ref(self.version()).cast::<u32>()
    }

    /// Consistent fallback read under the writer lock.
    fn read_locked<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        // Every route to the writer mutex passes through here — the
        // retry-exhaustion fallbacks and the unconditional `with_locked` /
        // `len` / `mem_used` paths alike. Counting at this chokepoint rather
        // than at each caller is what stops the instrument drifting.
        crate::occ_stats::bump(crate::occ_stats::Stat::LockedReads);
        #[cfg(feature = "std")]
        let _fallback = self.fallback_mutex.lock().expect("fallback mutex poisoned");
        #[cfg(feature = "std")]
        self.quiesce_writers();
        let _g: MutexGuard<'_, ()> = self.write.lock().expect("writer lock poisoned");
        // SAFETY: the writer mutex and WriterGate quiescence exclude all concurrent
        // readers and writers, so creating a temporary unique reference to flush
        // acceleration path cursors (`inner.clear_path()`) does not alias any concurrent access.
        let inner = unsafe { &mut *self.inner.get() };
        inner.clear_path();
        inner.set_tree_pop(self.tree_pop.load(core::sync::atomic::Ordering::Relaxed));
        let res = f(inner);
        inner.clear_path();
        drop(_g);
        #[cfg(feature = "std")]
        self.reopen_gate();
        res
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
            let snap = self.version().sample();
            // SAFETY: by-value snapshot; validated before use.
            let root = root_of(unsafe { &*self.inner.get() });
            if self.version().validate(snap) {
                return match root {
                    RootSnapshot::Empty => 0,
                    RootSnapshot::Leaf { pop, .. } => pop as u64,
                    RootSnapshot::Tree { .. } => {
                        self.tree_pop.load(core::sync::atomic::Ordering::Relaxed)
                    }
                };
            }
        }
        crate::occ_stats::bump(crate::occ_stats::Stat::ReadFallbacks);
        self.read_locked(locked)
    }
}

#[cfg(feature = "std")]
enum OlcOutcome<T> {
    Done(T),
    Retry,
    Fallback(FallbackCause),
}

/// Why an OLC mutation gave up and took the serialized root-covered path.
///
/// Phase 0 of #568 exists to measure this composition rather than assume it:
/// the plan's premise is that linear-leaf capacity expansion dominates, and
/// the committed artifacts already argue against it (the set and map arms
/// differ 2.3x in fallback share on identical key counts, and the map arm
/// retires ~2 blocks per fallback, which is a cascade rather than one leaf
/// growth). Every `OlcOutcome::Fallback` carries one of these, so the shares
/// sum to `Stat::LockFallbacks` and an attribution that does not add up is
/// visible instead of residual (AGENTS.md 8.1).
#[cfg(feature = "std")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FallbackCause {
    /// A linear leaf crossed a `leaf::cap_class` boundary, or a bitmap leaf
    /// crossed its own population threshold, and had to be reallocated.
    CapExpansion,
    /// An immediate slot ran out of packed capacity and must become a heap
    /// leaf, or an empty branch slot must become an immediate (#568).
    ImmediateConversion,
    /// A branch structural mutation: a full linear branch, a new digit in a
    /// bitmap subexpanse, or a prefix split from a failed skip decode.
    BranchSplit,
    /// A root-state transition, or a terminal directly below the root whose
    /// mutation is itself a root-state write (`anc_depth == 0`).
    RootGrowth,
    /// Contention, not structure: the retry budget was exhausted or the
    /// writer gate closed under a quiescing peer. Not removable by any
    /// leaf-sizing or node-layout change.
    Contention,
    /// The descent met an edge tag the OLC path does not decode. Expected
    /// ~0; a non-zero share means the walk has a hole.
    UnknownTag,
}

#[cfg(feature = "std")]
impl FallbackCause {
    /// The counter this cause bumps.
    #[inline]
    fn stat(self) -> crate::occ_stats::Stat {
        use crate::occ_stats::Stat;
        match self {
            Self::CapExpansion => Stat::FallbackCapExpansion,
            Self::ImmediateConversion => Stat::FallbackImmediateConversion,
            Self::BranchSplit => Stat::FallbackBranchSplit,
            Self::RootGrowth => Stat::FallbackRootGrowth,
            Self::Contention => Stat::FallbackContention,
            Self::UnknownTag => Stat::FallbackUnknownTag,
        }
    }
}

#[cfg(feature = "std")]
#[derive(Clone, Copy)]
struct AncestorFrame {
    node: *mut u8,
    edge_type: EdgeType,
    version_ptr: *const u32,
    version_snap: u32,
    child_level: u8,
    digit: u8,
}

#[cfg(feature = "std")]
#[inline(always)]
unsafe fn bump_edge_pop0(edge: *mut Edge, slot_level: u8, delta: i64) {
    // SAFETY: caller guarantees edge is an aligned, live pointer to an Edge inside a locked node.
    let tag = match unsafe { (*edge).tag() } {
        Some(t) => t,
        None => return,
    };
    let pop0_level = match tag {
        EdgeTag::Structural(EdgeType::Null) | EdgeTag::Immed(_) => return,
        EdgeTag::Structural(t) if t.leaf_key_bytes().is_some() => {
            t.leaf_key_bytes().expect("leaf tag")
        }
        EdgeTag::Structural(EdgeType::LeafB1) => 1,
        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7 | EdgeType::BranchB)) => {
            // SAFETY: edge is a valid structural branch edge.
            unsafe { crate::mutate::branch_form_level(&*edge, t, slot_level) }
        }
        EdgeTag::Structural(_) => slot_level,
    };
    if pop0_level <= 7 {
        // SAFETY: pop0_level is bounded and edge is live.
        unsafe {
            crate::mutate::bump_pop0(edge, pop0_level, delta);
        }
    }
}

#[cfg(feature = "std")]
#[inline(always)]
fn version_try_lock_timed(cell: &crate::occ::VersionCell) -> Result<(u32, u64), u32> {
    #[cfg(feature = "occ-stats")]
    let t0 = crate::occ_stats::cycles_now();
    #[cfg(not(feature = "occ-stats"))]
    let t0 = 0;
    let old_v = crate::occ::version_try_lock(cell)?;
    Ok((old_v, t0))
}

#[cfg(feature = "std")]
#[inline(always)]
fn version_try_lock_expect_timed(
    cell: &crate::occ::VersionCell,
    expected: u32,
) -> Result<(u32, u64), u32> {
    #[cfg(feature = "occ-stats")]
    let t0 = crate::occ_stats::cycles_now();
    #[cfg(not(feature = "occ-stats"))]
    let t0 = 0;
    let old_v = crate::occ::version_try_lock_expect(cell, expected)?;
    Ok((old_v, t0))
}

#[cfg(feature = "std")]
#[inline(always)]
fn version_unlock_timed(cell: &crate::occ::VersionCell, old_v: u32, modified: bool, _t0: u64) {
    crate::occ::version_unlock(cell, old_v, modified);
    #[cfg(feature = "occ-stats")]
    crate::occ_stats::bump_by(
        crate::occ_stats::Stat::LockHoldCycles,
        crate::occ_stats::cycles_now().wrapping_sub(_t0),
    );
}

#[cfg(feature = "std")]
#[inline]
unsafe fn bump_ancestor_pop0(
    node: *mut u8,
    edge_type: EdgeType,
    child_level: u8,
    d: u8,
    delta: i64,
) {
    let vp: *const u32 = match edge_type {
        // SAFETY: node pointer is EBR-live and validated by parent version check.
        EdgeType::BranchL3 => unsafe { &raw const (*node.cast::<BranchL3>()).hdr.version },
        // SAFETY: node pointer is EBR-live and validated by parent version check.
        EdgeType::BranchL7 => unsafe { &raw const (*node.cast::<BranchL7>()).hdr.version },
        // SAFETY: node pointer is EBR-live and validated by parent version check.
        EdgeType::BranchB => unsafe { &raw const (*node.cast::<BranchB>()).version },
        // SAFETY: node pointer is EBR-live and validated by parent version check.
        EdgeType::BranchU => unsafe { &raw const (*node.cast::<BranchU>()).version },
        _ => return,
    };
    // SAFETY: caller guarantees node is an EBR-live branch node.
    let cell = unsafe { crate::occ::version_cell(vp) };
    loop {
        match version_try_lock_timed(cell) {
            Ok((old_v, lock_t0)) => {
                // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                unsafe {
                    match edge_type {
                        EdgeType::BranchL3 => {
                            let b = node.cast::<BranchL3>();
                            if let Some(slot) = (*b).hdr.find(d) {
                                bump_edge_pop0(&raw mut (*b).edges[slot], child_level, delta);
                            }
                        }
                        EdgeType::BranchL7 => {
                            let b = node.cast::<BranchL7>();
                            if let Some(slot) = (*b).hdr.find(d) {
                                bump_edge_pop0(&raw mut (*b).edges[slot], child_level, delta);
                            }
                        }
                        EdgeType::BranchB => {
                            let b = node.cast::<BranchB>();
                            if (*b).bitmap.test(d) {
                                let rank = (*b).bitmap.subexpanse_rank(d) as usize;
                                let sub = (*b).subarrays[(d >> 5) as usize];
                                if !sub.is_null() {
                                    bump_edge_pop0(sub.add(rank), child_level, delta);
                                }
                            }
                        }
                        EdgeType::BranchU => {
                            let b = node.cast::<BranchU>();
                            bump_edge_pop0(&raw mut (*b).edges[d as usize], child_level, delta);
                        }
                        _ => {}
                    }
                    version_unlock_timed(cell, old_v, true, lock_t0);
                }
                break;
            }
            Err(cur) => {
                crate::occ_stats::bump(crate::occ_stats::Stat::LockSpins);
                debug_assert!(
                    (cur & crate::occ::OBSOLETE) == 0,
                    "ancestor node became obsolete during bump_ancestor_pop0: structural restructuring must re-descend"
                );
                if (cur & crate::occ::OBSOLETE) != 0 {
                    break;
                }
                core::hint::spin_loop();
                #[cfg(loom)]
                loom::thread::yield_now();
            }
        }
    }
}

/// A set shareable across threads: one writer at a time (internally
/// serialized), validated optimistic readers. See the module docs for the
/// protocol and its trade-offs.
pub struct SyncExpanseSet {
    shared: Box<Shared<ExpanseSet>>,
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
        let shared = Shared::new(ExpanseSet::new(), |s, c| {
            s.clear_path();
            s.occ_root().1.defer_to(Arc::clone(c));
        });
        // The word is bound at its final address; the engine may now cover
        // the root state itself.
        shared.inner_ref().occ_root().1.cover_root();
        Self { shared }
    }

    /// Inserts `key`; returns `true` if it was absent. Uses multi-writer
    /// optimistic lock coupling (Stage B) when the root is a tree, falling
    /// back to the serialized writer lock for root-state transitions.
    pub fn insert(&self, key: Key) -> bool {
        crate::occ_stats::bump(crate::occ_stats::Stat::Inserts);
        #[cfg(feature = "std")]
        {
            if !self.shared.inner_ref().root_is_tree() {
                crate::occ_stats::bump(crate::occ_stats::Stat::LockFallbacks);
                crate::occ_stats::bump(FallbackCause::RootGrowth.stat());
                return self.shared.write_root_covered(|s| s.insert(key));
            }

            let _guard = self.shared.enter_writer_blocking();
            let res = self.shared.with_writer_pin(|| {
                crate::occ_stats::bump(crate::occ_stats::Stat::WriteOps);
                crate::occ_stats::op_begin();

                let mut cause = FallbackCause::Contention;
                for _ in 0..MAX_RETRIES {
                    if self.shared.gate.is_closed() {
                        break;
                    }
                    match self.olc_insert_set(key) {
                        OlcOutcome::Done(ins) => {
                            if ins {
                                self.shared
                                    .tree_pop
                                    .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                            }
                            self.shared.collector.tick_advance();
                            crate::occ_stats::op_end();
                            return Ok(ins);
                        }
                        OlcOutcome::Retry => {
                            crate::occ_stats::bump(crate::occ_stats::Stat::LockRestarts);
                            core::hint::spin_loop();
                            #[cfg(loom)]
                            loom::thread::yield_now();
                        }
                        OlcOutcome::Fallback(c) => {
                            cause = c;
                            break;
                        }
                    }
                }
                crate::occ_stats::op_end();
                crate::occ_stats::bump(crate::occ_stats::Stat::LockFallbacks);
                crate::occ_stats::bump(cause.stat());
                Err(cause)
            });
            drop(_guard);
            match res {
                Ok(ins) => ins,
                Err(_) => self.shared.write_root_covered(|s| s.insert(key)),
            }
        }
        #[cfg(not(feature = "std"))]
        {
            self.shared.write_root_covered(|s| s.insert(key))
        }
    }

    /// Removes `key`; returns `true` if it was present.
    pub fn remove(&self, key: Key) -> bool {
        #[cfg(feature = "std")]
        {
            if !self.shared.inner_ref().root_is_tree() {
                crate::occ_stats::bump(crate::occ_stats::Stat::LockFallbacks);
                crate::occ_stats::bump(FallbackCause::RootGrowth.stat());
                return self.shared.write_root_covered(|s| s.remove(key));
            }

            let _guard = self.shared.enter_writer_blocking();
            let res = self.shared.with_writer_pin(|| {
                crate::occ_stats::bump(crate::occ_stats::Stat::WriteOps);
                crate::occ_stats::op_begin();

                let mut cause = FallbackCause::Contention;
                for _ in 0..MAX_RETRIES {
                    if self.shared.gate.is_closed() {
                        break;
                    }
                    match self.olc_remove_set(key) {
                        OlcOutcome::Done(rem) => {
                            if rem {
                                self.shared
                                    .tree_pop
                                    .fetch_sub(1, core::sync::atomic::Ordering::Relaxed);
                            }
                            self.shared.collector.tick_advance();
                            crate::occ_stats::op_end();
                            return Ok(rem);
                        }
                        OlcOutcome::Retry => {
                            crate::occ_stats::bump(crate::occ_stats::Stat::LockRestarts);
                            core::hint::spin_loop();
                            #[cfg(loom)]
                            loom::thread::yield_now();
                        }
                        OlcOutcome::Fallback(c) => {
                            cause = c;
                            break;
                        }
                    }
                }
                crate::occ_stats::op_end();
                crate::occ_stats::bump(crate::occ_stats::Stat::LockFallbacks);
                crate::occ_stats::bump(cause.stat());
                Err(cause)
            });
            drop(_guard);
            match res {
                Ok(rem) => rem,
                Err(_) => self.shared.write_root_covered(|s| s.remove(key)),
            }
        }
        #[cfg(not(feature = "std"))]
        {
            self.shared.write_root_covered(|s| s.remove(key))
        }
    }

    #[cfg(feature = "std")]
    fn olc_insert_set(&self, key: Key) -> OlcOutcome<bool> {
        let v_snap = self.shared.version().sample();
        if (v_snap & 1) != 0 {
            return OlcOutcome::Retry;
        }
        // SAFETY: top_ptr obtained without taking &mut on inner.
        let top_ptr = unsafe { (*self.shared.inner.get()).root_top_ptr() };
        if top_ptr.is_null() {
            return OlcOutcome::Fallback(FallbackCause::RootGrowth);
        }
        let mut ancestors: [AncestorFrame; 8] = [AncestorFrame {
            node: core::ptr::null_mut(),
            edge_type: EdgeType::Null,
            version_ptr: core::ptr::null(),
            version_snap: 0,
            child_level: 0,
            digit: 0,
        }; 8];
        let mut anc_depth = 0;
        // SAFETY: top_ptr points to the root edge under verified EBR-live snapshot.
        let mut edge = unsafe { top_ptr.read() };
        let mut edge_ptr = top_ptr;
        let mut level = 8u8;

        loop {
            let tag = edge.tag().expect("valid edge tag");
            match tag {
                EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
                    let is_l3 = matches!(t, EdgeType::BranchL3);
                    let node = edge.node_ptr();
                    let vp = if is_l3 {
                        // SAFETY: node pointer is EBR-live and validated by parent version check.
                        unsafe { &raw const (*node.cast::<BranchL3>()).hdr.version }
                    } else {
                        // SAFETY: node pointer is EBR-live and validated by parent version check.
                        unsafe { &raw const (*node.cast::<BranchL7>()).hdr.version }
                    };
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let Some(nsnap) =
                        (unsafe { crate::occ::node_sample(crate::occ::version_cell(vp)) })
                    else {
                        return OlcOutcome::Retry;
                    };
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let (bl, num, digits) = unsafe {
                        if is_l3 {
                            let b = node.cast::<BranchL3>();
                            ((*b).hdr.level, (*b).hdr.num as usize, (*b).hdr.digits)
                        } else {
                            let b = node.cast::<BranchL7>();
                            ((*b).hdr.level, (*b).hdr.num as usize, (*b).hdr.digits)
                        }
                    };
                    if !(2..=level).contains(&bl) || num > if is_l3 { 3 } else { 7 } {
                        return OlcOutcome::Retry;
                    }
                    if bl < level && !crate::get::decode_matches(&edge, key, bl, level) {
                        return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                    }
                    let d = digit(key, bl);
                    let slot_opt = digits[..num].iter().position(|&x| x == d);
                    // SAFETY: version cell is within an EBR-live node allocation.
                    if !unsafe { crate::occ::node_validate(crate::occ::version_cell(vp), nsnap) } {
                        return OlcOutcome::Retry;
                    }
                    if let Some(slot) = slot_opt {
                        ancestors[anc_depth] = AncestorFrame {
                            node,
                            edge_type: t,
                            version_ptr: vp,
                            version_snap: nsnap,
                            child_level: bl - 1,
                            digit: d,
                        };
                        anc_depth += 1;
                        edge_ptr = if is_l3 {
                            // SAFETY: node pointer is EBR-live and validated by parent version check.
                            unsafe { &raw mut (*node.cast::<BranchL3>()).edges[slot] }
                        } else {
                            // SAFETY: node pointer is EBR-live and validated by parent version check.
                            unsafe { &raw mut (*node.cast::<BranchL7>()).edges[slot] }
                        };
                        // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                        edge = unsafe { edge_ptr.read() };
                        level = bl - 1;
                        continue;
                    }
                    let cap = if is_l3 { BRANCH_L3_CAP } else { BRANCH_L7_CAP };
                    if num < cap {
                        // SAFETY: version cell is within an EBR-live node allocation.
                        let Ok((old_v, lock_t0)) = (unsafe {
                            version_try_lock_expect_timed(crate::occ::version_cell(vp), nsnap)
                        }) else {
                            return OlcOutcome::Retry;
                        };
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        let cur_num = unsafe {
                            if is_l3 {
                                (*node.cast::<BranchL3>()).hdr.num as usize
                            } else {
                                (*node.cast::<BranchL7>()).hdr.num as usize
                            }
                        };
                        if cur_num >= cap {
                            // SAFETY: version cell is within an EBR-live node allocation.
                            unsafe {
                                version_unlock_timed(
                                    crate::occ::version_cell(vp),
                                    old_v,
                                    false,
                                    lock_t0,
                                );
                            }
                            return OlcOutcome::Retry;
                        }
                        let new_edge =
                            Edge::new_immed_single_set(bl - 1, crate::mutate::key_low(key, bl - 1));
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        unsafe {
                            if is_l3 {
                                let b = node.cast::<BranchL3>();
                                let slot = crate::mutate::linear_insert_slot_l3(
                                    &mut (*b).hdr.digits,
                                    &mut (*b).edges,
                                    cur_num,
                                    d,
                                );
                                (*b).edges[slot] = new_edge;
                                (*b).hdr.num += 1;
                                (*b).hdr.add_presence(d);
                            } else {
                                let b = node.cast::<BranchL7>();
                                let slot = crate::mutate::linear_insert_slot(
                                    &mut (*b).hdr.digits,
                                    &mut (*b).edges,
                                    cur_num,
                                    d,
                                );
                                (*b).edges[slot] = new_edge;
                                (*b).hdr.num += 1;
                                (*b).hdr.add_presence(d);
                            }
                            version_unlock_timed(
                                crate::occ::version_cell(vp),
                                old_v,
                                true,
                                lock_t0,
                            );
                            for i in (0..anc_depth).rev() {
                                let a = &ancestors[i];
                                bump_ancestor_pop0(a.node, a.edge_type, a.child_level, a.digit, 1);
                            }
                        }
                        return OlcOutcome::Done(true);
                    }
                    return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                }

                EdgeTag::Structural(EdgeType::BranchB) => {
                    let node = edge.node_ptr().cast::<BranchB>();
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let vp = unsafe { &raw const (*node).version };
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let Some(nsnap) =
                        (unsafe { crate::occ::node_sample(crate::occ::version_cell(vp)) })
                    else {
                        return OlcOutcome::Retry;
                    };
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let (bl, bit, rank, sub) = unsafe {
                        let bl = (*node).level;
                        if !(2..=level).contains(&bl) {
                            return OlcOutcome::Retry;
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
                        return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                    }
                    if !bit || sub.is_null() {
                        return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                    }
                    // SAFETY: version cell is within an EBR-live node allocation.
                    if !unsafe { crate::occ::node_validate(crate::occ::version_cell(vp), nsnap) } {
                        return OlcOutcome::Retry;
                    }
                    ancestors[anc_depth] = AncestorFrame {
                        node: node.cast(),
                        edge_type: EdgeType::BranchB,
                        version_ptr: vp,
                        version_snap: nsnap,
                        child_level: bl - 1,
                        digit: digit(key, bl),
                    };
                    anc_depth += 1;
                    // SAFETY: pointer arithmetic and destination buffer bounds are valid under locked parent.
                    edge_ptr = unsafe { sub.add(rank) };
                    // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                    edge = unsafe { edge_ptr.read() };
                    level = bl - 1;
                    continue;
                }

                EdgeTag::Structural(EdgeType::BranchU) => {
                    let node = edge.node_ptr().cast::<BranchU>();
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let vp = unsafe { &raw const (*node).version };
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let Some(nsnap) =
                        (unsafe { crate::occ::node_sample(crate::occ::version_cell(vp)) })
                    else {
                        return OlcOutcome::Retry;
                    };
                    let d = digit(key, level);
                    // SAFETY: version cell is within an EBR-live node allocation.
                    if !unsafe { crate::occ::node_validate(crate::occ::version_cell(vp), nsnap) } {
                        return OlcOutcome::Retry;
                    }
                    ancestors[anc_depth] = AncestorFrame {
                        node: node.cast(),
                        edge_type: EdgeType::BranchU,
                        version_ptr: vp,
                        version_snap: nsnap,
                        child_level: level - 1,
                        digit: d,
                    };
                    anc_depth += 1;
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    edge_ptr = unsafe { &raw mut (*node).edges[d as usize] };
                    // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                    edge = unsafe { edge_ptr.read() };
                    level -= 1;
                    continue;
                }

                EdgeTag::Structural(EdgeType::LeafB1) => {
                    if anc_depth == 0 {
                        return OlcOutcome::Fallback(FallbackCause::RootGrowth);
                    }
                    if level > 1 && !crate::get::decode_matches(&edge, key, 1, level) {
                        return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                    }
                    let parent = ancestors[anc_depth - 1];
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let p_cell = unsafe { crate::occ::version_cell(parent.version_ptr) };
                    let node = edge.node_ptr().cast::<LeafBitmap1>();
                    let d = (key & 0xFF) as u8;
                    // SAFETY: node is an EBR-live bitmap node and parent is validated/locked.
                    let bit = unsafe { (*node).bitmap.test(d) };
                    if bit {
                        if !crate::occ::node_validate(p_cell, parent.version_snap) {
                            return OlcOutcome::Retry;
                        }
                        return OlcOutcome::Done(false);
                    }
                    let pop0 = edge.pop0(1) as usize;
                    if pop0 >= 254 {
                        return OlcOutcome::Fallback(FallbackCause::CapExpansion);
                    }
                    let Ok((old_v, lock_t0)) =
                        version_try_lock_expect_timed(p_cell, parent.version_snap)
                    else {
                        return OlcOutcome::Retry;
                    };
                    // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                    if !unsafe { edge_ptr.read() }.bits_eq(&edge) {
                        version_unlock_timed(p_cell, old_v, false, lock_t0);
                        return OlcOutcome::Retry;
                    }
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    unsafe {
                        if (*node).bitmap.set(d) {
                            (*edge_ptr).set_pop0(1, (pop0 + 1) as u64);
                            version_unlock_timed(p_cell, old_v, true, lock_t0);
                            for i in (0..anc_depth.saturating_sub(1)).rev() {
                                let a = &ancestors[i];
                                bump_ancestor_pop0(a.node, a.edge_type, a.child_level, a.digit, 1);
                            }
                            return OlcOutcome::Done(true);
                        } else {
                            version_unlock_timed(p_cell, old_v, false, lock_t0);
                            return OlcOutcome::Done(false);
                        }
                    }
                }

                EdgeTag::Structural(EdgeType::FullExpanse) => {
                    // Linearizability: A FullExpanse node contains all 256 keys in its
                    // subexpanse, so `key` is guaranteed to already be present. Because
                    // this terminal was observed at a valid point in time under this
                    // descent, the insert linearizes at this instant as a no-op returning false.
                    return OlcOutcome::Done(false);
                }

                EdgeTag::Structural(EdgeType::Null) => {
                    return OlcOutcome::Fallback(FallbackCause::ImmediateConversion);
                }

                EdgeTag::Structural(
                    t @ (EdgeType::Leaf1
                    | EdgeType::Leaf2
                    | EdgeType::Leaf3
                    | EdgeType::Leaf4
                    | EdgeType::Leaf5
                    | EdgeType::Leaf6
                    | EdgeType::Leaf7),
                ) => {
                    if anc_depth == 0 {
                        return OlcOutcome::Fallback(FallbackCause::RootGrowth);
                    }
                    let parent = ancestors[anc_depth - 1];
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let p_cell = unsafe { crate::occ::version_cell(parent.version_ptr) };
                    let kb = t.leaf_key_bytes().expect("leaf tag") as usize;
                    let pop = edge.pop0(kb as u8) as usize + 1;
                    if kb < level as usize
                        && !crate::get::decode_matches(&edge, key, kb as u8, level)
                    {
                        return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                    }
                    let k = crate::mutate::key_low(key, kb as u8);
                    let keys_ptr = edge.node_ptr();
                    let mut found = false;
                    let mut at = pop;
                    for i in 0..pop {
                        // SAFETY: pointer and index are within bounds of valid leaf/immediate allocation.
                        let existing = unsafe { crate::mutate::read_packed(keys_ptr, i, kb) };
                        if existing == k {
                            found = true;
                            break;
                        } else if existing > k && at == pop {
                            at = i;
                        }
                    }
                    if found {
                        if !crate::occ::node_validate(p_cell, parent.version_snap) {
                            return OlcOutcome::Retry;
                        }
                        return OlcOutcome::Done(false);
                    }
                    let cap = if kb == 1 {
                        crate::mutate::LEAF1_CAP
                    } else {
                        crate::mutate::LEAF_CAP
                    };
                    if pop < cap && crate::leaf::cap_class(pop + 1) == crate::leaf::cap_class(pop) {
                        let Ok((old_v, lock_t0)) =
                            version_try_lock_expect_timed(p_cell, parent.version_snap)
                        else {
                            return OlcOutcome::Retry;
                        };
                        // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                        if !unsafe { edge_ptr.read() }.bits_eq(&edge) {
                            version_unlock_timed(p_cell, old_v, false, lock_t0);
                            return OlcOutcome::Retry;
                        }
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        unsafe {
                            core::ptr::copy(
                                keys_ptr.add(at * kb),
                                keys_ptr.add((at + 1) * kb),
                                (pop - at) * kb,
                            );
                            crate::mutate::write_packed(keys_ptr, at, kb, k);
                            (*edge_ptr).set_pop0(kb as u8, pop as u64);
                            version_unlock_timed(p_cell, old_v, true, lock_t0);
                            for i in (0..anc_depth.saturating_sub(1)).rev() {
                                let a = &ancestors[i];
                                bump_ancestor_pop0(a.node, a.edge_type, a.child_level, a.digit, 1);
                            }
                        }
                        return OlcOutcome::Done(true);
                    }
                    return OlcOutcome::Fallback(FallbackCause::CapExpansion);
                }

                EdgeTag::Immed(im) => {
                    if anc_depth == 0 {
                        return OlcOutcome::Fallback(FallbackCause::RootGrowth);
                    }
                    let kb = im.key_bytes();
                    if level > kb && !crate::get::decode_matches(&edge, key, kb, level) {
                        return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                    }
                    let parent = ancestors[anc_depth - 1];
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let p_cell = unsafe { crate::occ::version_cell(parent.version_ptr) };
                    let k = crate::mutate::key_low(key, kb);
                    let n = im.key_count() as usize;
                    let kb_usize = kb as usize;
                    let payload = edge.imm_payload();
                    let mut found = false;
                    for i in 0..n {
                        // SAFETY: pointer and index are within bounds of valid leaf/immediate allocation.
                        if unsafe { crate::mutate::read_packed(payload.as_ptr(), i, kb_usize) } == k
                        {
                            found = true;
                            break;
                        }
                    }
                    if found {
                        if !crate::occ::node_validate(p_cell, parent.version_snap) {
                            return OlcOutcome::Retry;
                        }
                        return OlcOutcome::Done(false);
                    }
                    if n == 1 && crate::types::ImmedType::max_count(kb) >= 2 {
                        let Ok((old_v, lock_t0)) =
                            version_try_lock_expect_timed(p_cell, parent.version_snap)
                        else {
                            return OlcOutcome::Retry;
                        };
                        // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                        if !unsafe { edge_ptr.read() }.bits_eq(&edge) {
                            version_unlock_timed(p_cell, old_v, false, lock_t0);
                            return OlcOutcome::Retry;
                        }
                        // SAFETY: pointer and index are within bounds of valid leaf/immediate allocation.
                        let existing_k =
                            unsafe { crate::mutate::read_packed(payload.as_ptr(), 0, kb_usize) };
                        let (k0, k1) = if k < existing_k {
                            (k, existing_k)
                        } else {
                            (existing_k, k)
                        };
                        let mut new_payload = [0u8; 16];
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        unsafe {
                            crate::mutate::write_packed(new_payload.as_mut_ptr(), 0, kb_usize, k0);
                            crate::mutate::write_packed(new_payload.as_mut_ptr(), 1, kb_usize, k1);
                        }
                        let mut w0 = [0u8; 8];
                        w0.copy_from_slice(&new_payload[..8]);
                        let mut aux = [0u8; 7];
                        aux.copy_from_slice(&new_payload[8..15]);
                        let new_im = crate::types::ImmedType::new(kb, 2).expect("capacity");
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        unsafe {
                            (*edge_ptr).set_imm_bytes(w0);
                            (*edge_ptr).set_aux_bytes(aux);
                            (*edge_ptr).set_tag(new_im.as_u8());
                            version_unlock_timed(p_cell, old_v, true, lock_t0);
                            for i in (0..anc_depth.saturating_sub(1)).rev() {
                                let a = &ancestors[i];
                                bump_ancestor_pop0(a.node, a.edge_type, a.child_level, a.digit, 1);
                            }
                        }
                        return OlcOutcome::Done(true);
                    }
                    return OlcOutcome::Fallback(FallbackCause::ImmediateConversion);
                }

                #[allow(unreachable_patterns)]
                _ => return OlcOutcome::Fallback(FallbackCause::UnknownTag),
            }
        }
    }

    #[cfg(feature = "std")]
    fn olc_remove_set(&self, key: Key) -> OlcOutcome<bool> {
        let v_snap = self.shared.version().sample();
        if (v_snap & 1) != 0 {
            return OlcOutcome::Retry;
        }
        // SAFETY: top_ptr obtained without taking &mut on inner.
        let top_ptr = unsafe { (*self.shared.inner.get()).root_top_ptr() };
        if top_ptr.is_null() {
            return OlcOutcome::Fallback(FallbackCause::RootGrowth);
        }
        let mut ancestors: [AncestorFrame; 8] = [AncestorFrame {
            node: core::ptr::null_mut(),
            edge_type: EdgeType::Null,
            version_ptr: core::ptr::null(),
            version_snap: 0,
            child_level: 0,
            digit: 0,
        }; 8];
        let mut anc_depth = 0;
        // SAFETY: top_ptr points to the root edge under verified EBR-live snapshot.
        let mut edge = unsafe { top_ptr.read() };
        let mut edge_ptr = top_ptr;
        let mut level = 8u8;

        loop {
            let tag = edge.tag().expect("valid edge tag");
            match tag {
                EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
                    let is_l3 = matches!(t, EdgeType::BranchL3);
                    let node = edge.node_ptr();
                    let vp = if is_l3 {
                        // SAFETY: node pointer is EBR-live and validated by parent version check.
                        unsafe { &raw const (*node.cast::<BranchL3>()).hdr.version }
                    } else {
                        // SAFETY: node pointer is EBR-live and validated by parent version check.
                        unsafe { &raw const (*node.cast::<BranchL7>()).hdr.version }
                    };
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let Some(nsnap) =
                        (unsafe { crate::occ::node_sample(crate::occ::version_cell(vp)) })
                    else {
                        return OlcOutcome::Retry;
                    };
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let (bl, num, digits) = unsafe {
                        if is_l3 {
                            let b = node.cast::<BranchL3>();
                            ((*b).hdr.level, (*b).hdr.num as usize, (*b).hdr.digits)
                        } else {
                            let b = node.cast::<BranchL7>();
                            ((*b).hdr.level, (*b).hdr.num as usize, (*b).hdr.digits)
                        }
                    };
                    if !(2..=level).contains(&bl) || num > if is_l3 { 3 } else { 7 } {
                        return OlcOutcome::Retry;
                    }
                    if bl < level && !crate::get::decode_matches(&edge, key, bl, level) {
                        return OlcOutcome::Done(false);
                    }
                    let d = digit(key, bl);
                    let slot_opt = digits[..num].iter().position(|&x| x == d);
                    // SAFETY: version cell is within an EBR-live node allocation.
                    if !unsafe { crate::occ::node_validate(crate::occ::version_cell(vp), nsnap) } {
                        return OlcOutcome::Retry;
                    }
                    if let Some(slot) = slot_opt {
                        ancestors[anc_depth] = AncestorFrame {
                            node,
                            edge_type: t,
                            version_ptr: vp,
                            version_snap: nsnap,
                            child_level: bl - 1,
                            digit: d,
                        };
                        anc_depth += 1;
                        edge_ptr = if is_l3 {
                            // SAFETY: node pointer is EBR-live and validated by parent version check.
                            unsafe { &raw mut (*node.cast::<BranchL3>()).edges[slot] }
                        } else {
                            // SAFETY: node pointer is EBR-live and validated by parent version check.
                            unsafe { &raw mut (*node.cast::<BranchL7>()).edges[slot] }
                        };
                        // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                        edge = unsafe { edge_ptr.read() };
                        level = bl - 1;
                        continue;
                    }
                    return OlcOutcome::Done(false);
                }

                EdgeTag::Structural(EdgeType::BranchB) => {
                    let node = edge.node_ptr().cast::<BranchB>();
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let vp = unsafe { &raw const (*node).version };
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let Some(nsnap) =
                        (unsafe { crate::occ::node_sample(crate::occ::version_cell(vp)) })
                    else {
                        return OlcOutcome::Retry;
                    };
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let (bl, bit, rank, sub) = unsafe {
                        let bl = (*node).level;
                        if !(2..=level).contains(&bl) {
                            return OlcOutcome::Retry;
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
                        return OlcOutcome::Done(false);
                    }
                    if !bit || sub.is_null() {
                        return OlcOutcome::Done(false);
                    }
                    // SAFETY: version cell is within an EBR-live node allocation.
                    if !unsafe { crate::occ::node_validate(crate::occ::version_cell(vp), nsnap) } {
                        return OlcOutcome::Retry;
                    }
                    ancestors[anc_depth] = AncestorFrame {
                        node: node.cast(),
                        edge_type: EdgeType::BranchB,
                        version_ptr: vp,
                        version_snap: nsnap,
                        child_level: bl - 1,
                        digit: digit(key, bl),
                    };
                    anc_depth += 1;
                    // SAFETY: pointer arithmetic and destination buffer bounds are valid under locked parent.
                    edge_ptr = unsafe { sub.add(rank) };
                    // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                    edge = unsafe { edge_ptr.read() };
                    level = bl - 1;
                    continue;
                }

                EdgeTag::Structural(EdgeType::BranchU) => {
                    let node = edge.node_ptr().cast::<BranchU>();
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let vp = unsafe { &raw const (*node).version };
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let Some(nsnap) =
                        (unsafe { crate::occ::node_sample(crate::occ::version_cell(vp)) })
                    else {
                        return OlcOutcome::Retry;
                    };
                    let d = digit(key, level);
                    // SAFETY: version cell is within an EBR-live node allocation.
                    if !unsafe { crate::occ::node_validate(crate::occ::version_cell(vp), nsnap) } {
                        return OlcOutcome::Retry;
                    }
                    ancestors[anc_depth] = AncestorFrame {
                        node: node.cast(),
                        edge_type: EdgeType::BranchU,
                        version_ptr: vp,
                        version_snap: nsnap,
                        child_level: level - 1,
                        digit: d,
                    };
                    anc_depth += 1;
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    edge_ptr = unsafe { &raw mut (*node).edges[d as usize] };
                    // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                    edge = unsafe { edge_ptr.read() };
                    level -= 1;
                    continue;
                }

                EdgeTag::Structural(EdgeType::LeafB1) => {
                    if anc_depth == 0 {
                        return OlcOutcome::Fallback(FallbackCause::RootGrowth);
                    }
                    if level > 1 && !crate::get::decode_matches(&edge, key, 1, level) {
                        return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                    }
                    let parent = ancestors[anc_depth - 1];
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let p_cell = unsafe { crate::occ::version_cell(parent.version_ptr) };
                    let node = edge.node_ptr().cast::<LeafBitmap1>();
                    let d = (key & 0xFF) as u8;
                    // SAFETY: node is an EBR-live bitmap node and parent is validated/locked.
                    let bit = unsafe { (*node).bitmap.test(d) };
                    if !bit {
                        if !crate::occ::node_validate(p_cell, parent.version_snap) {
                            return OlcOutcome::Retry;
                        }
                        return OlcOutcome::Done(false);
                    }
                    let pop0 = edge.pop0(1) as usize;
                    if pop0 == 0 || pop0 <= 32 {
                        return OlcOutcome::Fallback(FallbackCause::CapExpansion);
                    }
                    let Ok((old_v, lock_t0)) =
                        version_try_lock_expect_timed(p_cell, parent.version_snap)
                    else {
                        return OlcOutcome::Retry;
                    };
                    // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                    if !unsafe { edge_ptr.read() }.bits_eq(&edge) {
                        version_unlock_timed(p_cell, old_v, false, lock_t0);
                        return OlcOutcome::Retry;
                    }
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    unsafe {
                        (*node).bitmap.clear(d);
                        (*edge_ptr).set_pop0(1, (pop0 - 1) as u64);
                        version_unlock_timed(p_cell, old_v, true, lock_t0);
                        for i in (0..anc_depth.saturating_sub(1)).rev() {
                            let a = &ancestors[i];
                            bump_ancestor_pop0(a.node, a.edge_type, a.child_level, a.digit, -1);
                        }
                        return OlcOutcome::Done(true);
                    }
                }

                EdgeTag::Structural(
                    t @ (EdgeType::Leaf1
                    | EdgeType::Leaf2
                    | EdgeType::Leaf3
                    | EdgeType::Leaf4
                    | EdgeType::Leaf5
                    | EdgeType::Leaf6
                    | EdgeType::Leaf7),
                ) => {
                    if anc_depth == 0 {
                        return OlcOutcome::Fallback(FallbackCause::RootGrowth);
                    }
                    let parent = ancestors[anc_depth - 1];
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let p_cell = unsafe { crate::occ::version_cell(parent.version_ptr) };
                    let kb = t.leaf_key_bytes().expect("leaf tag") as usize;
                    let pop = edge.pop0(kb as u8) as usize + 1;
                    if kb < level as usize
                        && !crate::get::decode_matches(&edge, key, kb as u8, level)
                    {
                        return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                    }
                    let k = crate::mutate::key_low(key, kb as u8);
                    let keys_ptr = edge.node_ptr();
                    let mut found = None;
                    for i in 0..pop {
                        // SAFETY: pointer and index are within bounds of valid leaf/immediate allocation.
                        if unsafe { crate::mutate::read_packed(keys_ptr, i, kb) } == k {
                            found = Some(i);
                            break;
                        }
                    }
                    let Some(pos) = found else {
                        if !crate::occ::node_validate(p_cell, parent.version_snap) {
                            return OlcOutcome::Retry;
                        }
                        return OlcOutcome::Done(false);
                    };
                    if pop > 2 && crate::leaf::cap_class(pop - 1) == crate::leaf::cap_class(pop) {
                        let Ok((old_v, lock_t0)) =
                            version_try_lock_expect_timed(p_cell, parent.version_snap)
                        else {
                            return OlcOutcome::Retry;
                        };
                        // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                        if !unsafe { edge_ptr.read() }.bits_eq(&edge) {
                            version_unlock_timed(p_cell, old_v, false, lock_t0);
                            return OlcOutcome::Retry;
                        }
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        unsafe {
                            core::ptr::copy(
                                keys_ptr.add((pos + 1) * kb),
                                keys_ptr.add(pos * kb),
                                (pop - 1 - pos) * kb,
                            );
                            (*edge_ptr).set_pop0(kb as u8, (pop - 2) as u64);
                            version_unlock_timed(p_cell, old_v, true, lock_t0);
                            for i in (0..anc_depth.saturating_sub(1)).rev() {
                                let a = &ancestors[i];
                                bump_ancestor_pop0(a.node, a.edge_type, a.child_level, a.digit, -1);
                            }
                        }
                        return OlcOutcome::Done(true);
                    }
                    return OlcOutcome::Fallback(FallbackCause::CapExpansion);
                }

                EdgeTag::Structural(EdgeType::Null) => {
                    if anc_depth > 0 {
                        let parent = ancestors[anc_depth - 1];
                        // SAFETY: version cell is within an EBR-live node allocation.
                        let p_cell = unsafe { crate::occ::version_cell(parent.version_ptr) };
                        if !crate::occ::node_validate(p_cell, parent.version_snap) {
                            return OlcOutcome::Retry;
                        }
                    }
                    return OlcOutcome::Done(false);
                }

                #[allow(unreachable_patterns)]
                _ => return OlcOutcome::Fallback(FallbackCause::UnknownTag),
            }
        }
    }

    /// Removes every key from the set.
    pub fn clear(&self) {
        self.shared.write_root_covered(|s| {
            s.clear();
            self.shared
                .tree_pop
                .store(0, core::sync::atomic::Ordering::Relaxed);
        });
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
    /// Optimistic membership test.
    #[must_use]
    pub fn contains(&self, key: Key) -> bool {
        let shared = &self.set.shared;
        crate::occ_stats::bump(crate::occ_stats::Stat::ReadOps);
        for _ in 0..MAX_RETRIES {
            crate::occ_stats::bump(crate::occ_stats::Stat::ReadAttempts);
            let _pin = self.reader.pin();
            let snap = shared.version().sample();
            // SAFETY: pinned + freshly sampled version; the walk
            // validates every load (see `walk_validated`).
            // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
            let root = unsafe { (*shared.inner.get()).occ_root().0 };
            // SAFETY: same pin + snapshot contract as the line above.
            let walked = unsafe { walk_validated::<false>(root, key, shared.version(), snap) };
            if let Ok(r) = walked {
                return r.is_some();
            }
        }
        crate::occ_stats::bump(crate::occ_stats::Stat::ReadFallbacks);
        shared.read_locked(|s| s.contains(key))
    }
}

/// A map shareable across threads: one writer at a time (internally
/// serialized), validated optimistic readers. See the module docs.
pub struct SyncExpanseMap {
    shared: Box<Shared<ExpanseMap>>,
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
        let shared = Shared::new(ExpanseMap::new(), |m, c| {
            m.clear_path();
            m.occ_root().1.defer_to(Arc::clone(c));
        });
        // The word is bound at its final address; the engine may now cover
        // the root state itself.
        shared.inner_ref().occ_root().1.cover_root();
        Self { shared }
    }

    /// Inserts `key → val`; returns the replaced value, if any. Uses
    /// multi-writer optimistic lock coupling (Stage B) when the root is a
    /// tree, falling back to the serialized writer lock for root-state transitions.
    pub fn insert(&self, key: Key, val: u64) -> Option<u64> {
        crate::occ_stats::bump(crate::occ_stats::Stat::Inserts);
        #[cfg(feature = "std")]
        {
            if !self.shared.inner_ref().root_is_tree() {
                crate::occ_stats::bump(crate::occ_stats::Stat::LockFallbacks);
                crate::occ_stats::bump(FallbackCause::RootGrowth.stat());
                return self.shared.write_root_covered(|m| m.insert(key, val));
            }

            let _guard = self.shared.enter_writer_blocking();
            let res = self.shared.with_writer_pin(|| {
                crate::occ_stats::bump(crate::occ_stats::Stat::WriteOps);
                crate::occ_stats::op_begin();

                let mut cause = FallbackCause::Contention;
                for _ in 0..MAX_RETRIES {
                    if self.shared.gate.is_closed() {
                        break;
                    }
                    match self.olc_insert_map(key, val) {
                        OlcOutcome::Done(prev) => {
                            if prev.is_none() {
                                self.shared
                                    .tree_pop
                                    .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                            }
                            self.shared.collector.tick_advance();
                            crate::occ_stats::op_end();
                            return Ok(prev);
                        }
                        OlcOutcome::Retry => {
                            crate::occ_stats::bump(crate::occ_stats::Stat::LockRestarts);
                            core::hint::spin_loop();
                            #[cfg(loom)]
                            loom::thread::yield_now();
                        }
                        OlcOutcome::Fallback(c) => {
                            cause = c;
                            break;
                        }
                    }
                }
                crate::occ_stats::op_end();
                crate::occ_stats::bump(crate::occ_stats::Stat::LockFallbacks);
                crate::occ_stats::bump(cause.stat());
                Err(cause)
            });
            drop(_guard);
            match res {
                Ok(prev) => prev,
                Err(_) => self.shared.write_root_covered(|m| m.insert(key, val)),
            }
        }
        #[cfg(not(feature = "std"))]
        {
            self.shared.write_root_covered(|m| m.insert(key, val))
        }
    }

    /// Removes `key`; returns its value, if present. Uses multi-writer
    /// optimistic lock coupling (Stage B) when the root is a tree, falling
    /// back to the serialized writer lock for root-state transitions.
    pub fn remove(&self, key: Key) -> Option<u64> {
        #[cfg(feature = "std")]
        {
            if !self.shared.inner_ref().root_is_tree() {
                crate::occ_stats::bump(crate::occ_stats::Stat::LockFallbacks);
                crate::occ_stats::bump(FallbackCause::RootGrowth.stat());
                return self.shared.write_root_covered(|m| m.remove(key));
            }

            let _guard = self.shared.enter_writer_blocking();
            let res = self.shared.with_writer_pin(|| {
                crate::occ_stats::bump(crate::occ_stats::Stat::WriteOps);
                crate::occ_stats::op_begin();

                let mut cause = FallbackCause::Contention;
                for _ in 0..MAX_RETRIES {
                    if self.shared.gate.is_closed() {
                        break;
                    }
                    match self.olc_remove_map(key) {
                        OlcOutcome::Done(prev) => {
                            if prev.is_some() {
                                self.shared
                                    .tree_pop
                                    .fetch_sub(1, core::sync::atomic::Ordering::Relaxed);
                            }
                            self.shared.collector.tick_advance();
                            crate::occ_stats::op_end();
                            return Ok(prev);
                        }
                        OlcOutcome::Retry => {
                            crate::occ_stats::bump(crate::occ_stats::Stat::LockRestarts);
                            core::hint::spin_loop();
                            #[cfg(loom)]
                            loom::thread::yield_now();
                        }
                        OlcOutcome::Fallback(c) => {
                            cause = c;
                            break;
                        }
                    }
                }
                crate::occ_stats::op_end();
                crate::occ_stats::bump(crate::occ_stats::Stat::LockFallbacks);
                crate::occ_stats::bump(cause.stat());
                Err(cause)
            });
            drop(_guard);
            match res {
                Ok(prev) => prev,
                Err(_) => self.shared.write_root_covered(|m| m.remove(key)),
            }
        }
        #[cfg(not(feature = "std"))]
        {
            self.shared.write_root_covered(|m| m.remove(key))
        }
    }

    #[cfg(feature = "std")]
    fn olc_insert_map(&self, key: Key, val: u64) -> OlcOutcome<Option<u64>> {
        let v_snap = self.shared.version().sample();
        if (v_snap & 1) != 0 {
            return OlcOutcome::Retry;
        }
        // SAFETY: top_ptr obtained without taking &mut on inner.
        let top_ptr = unsafe { (*self.shared.inner.get()).root_top_ptr() };
        if top_ptr.is_null() {
            return OlcOutcome::Fallback(FallbackCause::RootGrowth);
        }
        let mut ancestors: [AncestorFrame; 8] = [AncestorFrame {
            node: core::ptr::null_mut(),
            edge_type: EdgeType::Null,
            version_ptr: core::ptr::null(),
            version_snap: 0,
            child_level: 0,
            digit: 0,
        }; 8];
        let mut anc_depth = 0;
        // SAFETY: top_ptr points to the root edge under verified EBR-live snapshot.
        let mut edge = unsafe { top_ptr.read() };
        let mut edge_ptr = top_ptr;
        let mut level = 8u8;

        loop {
            let tag = edge.tag().expect("valid edge tag");
            match tag {
                EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
                    let is_l3 = matches!(t, EdgeType::BranchL3);
                    let node = edge.node_ptr();
                    let vp = if is_l3 {
                        // SAFETY: node pointer is EBR-live and validated by parent version check.
                        unsafe { &raw const (*node.cast::<BranchL3>()).hdr.version }
                    } else {
                        // SAFETY: node pointer is EBR-live and validated by parent version check.
                        unsafe { &raw const (*node.cast::<BranchL7>()).hdr.version }
                    };
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let Some(nsnap) =
                        (unsafe { crate::occ::node_sample(crate::occ::version_cell(vp)) })
                    else {
                        return OlcOutcome::Retry;
                    };
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let (bl, num, slot_opt) = unsafe {
                        if is_l3 {
                            let b = node.cast::<BranchL3>();
                            let bl = (*b).hdr.level;
                            let num = (*b).hdr.num as usize;
                            (bl, num, (*b).hdr.find(digit(key, bl)))
                        } else {
                            let b = node.cast::<BranchL7>();
                            let bl = (*b).hdr.level;
                            let num = (*b).hdr.num as usize;
                            (bl, num, (*b).hdr.find(digit(key, bl)))
                        }
                    };
                    if !(2..=level).contains(&bl) || num > if is_l3 { 3 } else { 7 } {
                        return OlcOutcome::Retry;
                    }
                    if bl < level && !crate::get::decode_matches(&edge, key, bl, level) {
                        return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                    }
                    let d = digit(key, bl);
                    // SAFETY: version cell is within an EBR-live node allocation.
                    if !crate::occ::node_validate(unsafe { crate::occ::version_cell(vp) }, nsnap) {
                        return OlcOutcome::Retry;
                    }
                    if let Some(slot) = slot_opt {
                        ancestors[anc_depth] = AncestorFrame {
                            node,
                            edge_type: t,
                            version_ptr: vp,
                            version_snap: nsnap,
                            child_level: bl - 1,
                            digit: d,
                        };
                        anc_depth += 1;
                        edge_ptr = if is_l3 {
                            // SAFETY: node pointer is EBR-live and validated by parent version check.
                            unsafe { &raw mut (*node.cast::<BranchL3>()).edges[slot] }
                        } else {
                            // SAFETY: node pointer is EBR-live and validated by parent version check.
                            unsafe { &raw mut (*node.cast::<BranchL7>()).edges[slot] }
                        };
                        // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                        edge = unsafe { edge_ptr.read() };
                        level = bl - 1;
                        continue;
                    }
                    let cap = if is_l3 { BRANCH_L3_CAP } else { BRANCH_L7_CAP };
                    if num < cap {
                        // SAFETY: version cell is within an EBR-live node allocation.
                        let Ok((old_v, lock_t0)) = version_try_lock_expect_timed(
                            unsafe { crate::occ::version_cell(vp) },
                            nsnap,
                        ) else {
                            return OlcOutcome::Retry;
                        };
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        let cur_num = unsafe {
                            if is_l3 {
                                (*node.cast::<BranchL3>()).hdr.num as usize
                            } else {
                                (*node.cast::<BranchL7>()).hdr.num as usize
                            }
                        };
                        if cur_num >= cap {
                            // SAFETY: version cell is within an EBR-live node allocation.
                            version_unlock_timed(
                                unsafe { crate::occ::version_cell(vp) },
                                old_v,
                                false,
                                lock_t0,
                            );
                            return OlcOutcome::Retry;
                        }
                        let new_edge = Edge::new_immed_single_map(
                            bl - 1,
                            crate::mutate::key_low(key, bl - 1),
                            val,
                        );
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        unsafe {
                            if is_l3 {
                                let b = node.cast::<BranchL3>();
                                let slot = crate::mutate::linear_insert_slot_l3(
                                    &mut (*b).hdr.digits,
                                    &mut (*b).edges,
                                    cur_num,
                                    d,
                                );
                                (*b).edges[slot] = new_edge;
                                (*b).hdr.num += 1;
                                (*b).hdr.add_presence(d);
                            } else {
                                let b = node.cast::<BranchL7>();
                                let slot = crate::mutate::linear_insert_slot(
                                    &mut (*b).hdr.digits,
                                    &mut (*b).edges,
                                    cur_num,
                                    d,
                                );
                                (*b).edges[slot] = new_edge;
                                (*b).hdr.num += 1;
                                (*b).hdr.add_presence(d);
                            }
                            version_unlock_timed(
                                crate::occ::version_cell(vp),
                                old_v,
                                true,
                                lock_t0,
                            );
                            for i in (0..anc_depth).rev() {
                                let a = &ancestors[i];
                                bump_ancestor_pop0(a.node, a.edge_type, a.child_level, a.digit, 1);
                            }
                        }
                        return OlcOutcome::Done(None);
                    }
                    return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                }

                EdgeTag::Structural(EdgeType::BranchB) => {
                    let node = edge.node_ptr().cast::<BranchB>();
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let vp = unsafe { &raw const (*node).version };
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let Some(nsnap) =
                        (unsafe { crate::occ::node_sample(crate::occ::version_cell(vp)) })
                    else {
                        return OlcOutcome::Retry;
                    };
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let (bl, bit, rank, sub) = unsafe {
                        let bl = (*node).level;
                        if !(2..=level).contains(&bl) {
                            return OlcOutcome::Retry;
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
                        return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                    }
                    if !bit || sub.is_null() {
                        return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                    }
                    // SAFETY: version cell is within an EBR-live node allocation.
                    if !crate::occ::node_validate(unsafe { crate::occ::version_cell(vp) }, nsnap) {
                        return OlcOutcome::Retry;
                    }
                    ancestors[anc_depth] = AncestorFrame {
                        node: node.cast(),
                        edge_type: EdgeType::BranchB,
                        version_ptr: vp,
                        version_snap: nsnap,
                        child_level: bl - 1,
                        digit: digit(key, bl),
                    };
                    anc_depth += 1;
                    // SAFETY: pointer arithmetic and destination buffer bounds are valid under locked parent.
                    edge_ptr = unsafe { sub.add(rank) };
                    // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                    edge = unsafe { edge_ptr.read() };
                    level = bl - 1;
                    continue;
                }

                EdgeTag::Structural(EdgeType::BranchU) => {
                    let node = edge.node_ptr().cast::<BranchU>();
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let vp = unsafe { &raw const (*node).version };
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let Some(nsnap) =
                        (unsafe { crate::occ::node_sample(crate::occ::version_cell(vp)) })
                    else {
                        return OlcOutcome::Retry;
                    };
                    let d = digit(key, level);
                    // SAFETY: version cell is within an EBR-live node allocation.
                    if !crate::occ::node_validate(unsafe { crate::occ::version_cell(vp) }, nsnap) {
                        return OlcOutcome::Retry;
                    }
                    ancestors[anc_depth] = AncestorFrame {
                        node: node.cast(),
                        edge_type: EdgeType::BranchU,
                        version_ptr: vp,
                        version_snap: nsnap,
                        child_level: level - 1,
                        digit: d,
                    };
                    anc_depth += 1;
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    edge_ptr = unsafe { &raw mut (*node).edges[d as usize] };
                    // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                    edge = unsafe { edge_ptr.read() };
                    level -= 1;
                    continue;
                }

                EdgeTag::Structural(EdgeType::LeafB1) => {
                    if anc_depth == 0 {
                        return OlcOutcome::Fallback(FallbackCause::RootGrowth);
                    }
                    if level > 1 && !crate::get::decode_matches(&edge, key, 1, level) {
                        return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                    }
                    let parent = ancestors[anc_depth - 1];
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let p_cell = unsafe { crate::occ::version_cell(parent.version_ptr) };
                    let node = edge.node_ptr().cast::<LeafBitmapL>();
                    let d = (key & 0xFF) as u8;
                    let sub = (d >> 5) as usize;
                    // SAFETY: node is an EBR-live bitmap node and parent is validated/locked.
                    let tested = unsafe { (*node).bitmap.test_and_subexpanse_rank_with_sub(d) };
                    if let Some((_, rank)) = tested {
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        let vals = unsafe { (*node).values[sub] };
                        if vals.is_null() {
                            return OlcOutcome::Retry;
                        }
                        if !crate::occ::node_validate(p_cell, parent.version_snap) {
                            return OlcOutcome::Retry;
                        }
                        let Ok((old_v, lock_t0)) =
                            version_try_lock_expect_timed(p_cell, parent.version_snap)
                        else {
                            return OlcOutcome::Retry;
                        };
                        // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                        if !unsafe { edge_ptr.read() }.bits_eq(&edge) {
                            version_unlock_timed(p_cell, old_v, false, lock_t0);
                            return OlcOutcome::Retry;
                        }
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        unsafe {
                            let slot = (*node).values[sub].add(rank);
                            let old = slot.read();
                            slot.write(val);
                            version_unlock_timed(p_cell, old_v, true, lock_t0);
                            return OlcOutcome::Done(Some(old));
                        }
                    }
                    // SAFETY: node is an EBR-live bitmap node and parent is validated/locked.
                    let old_n = unsafe { (*node).bitmap.subexpanse_count(sub) as usize };
                    // SAFETY: node is an EBR-live bitmap node and parent is validated/locked.
                    let rank = unsafe { (*node).bitmap.subexpanse_rank(d) as usize };
                    let pop0 = edge.pop0(1) as usize;
                    if pop0 >= 254 {
                        return OlcOutcome::Fallback(FallbackCause::CapExpansion);
                    }
                    if old_n > 0
                        && crate::leaf::cap_class(old_n + 1) == crate::leaf::cap_class(old_n)
                    {
                        let Ok((old_v, lock_t0)) =
                            version_try_lock_expect_timed(p_cell, parent.version_snap)
                        else {
                            return OlcOutcome::Retry;
                        };
                        // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                        if !unsafe { edge_ptr.read() }.bits_eq(&edge) {
                            version_unlock_timed(p_cell, old_v, false, lock_t0);
                            return OlcOutcome::Retry;
                        }
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        unsafe {
                            let arr = (*node).values[sub];
                            core::ptr::copy(arr.add(rank), arr.add(rank + 1), old_n - rank);
                            arr.add(rank).write(val);
                            (*node).bitmap.set(d);
                            (*edge_ptr).set_pop0(1, (pop0 + 1) as u64);
                            version_unlock_timed(p_cell, old_v, true, lock_t0);
                            for i in (0..anc_depth.saturating_sub(1)).rev() {
                                let a = &ancestors[i];
                                bump_ancestor_pop0(a.node, a.edge_type, a.child_level, a.digit, 1);
                            }
                        }
                        return OlcOutcome::Done(None);
                    }
                    return OlcOutcome::Fallback(FallbackCause::CapExpansion);
                }

                EdgeTag::Structural(
                    t @ (EdgeType::Leaf1
                    | EdgeType::Leaf2
                    | EdgeType::Leaf3
                    | EdgeType::Leaf4
                    | EdgeType::Leaf5
                    | EdgeType::Leaf6
                    | EdgeType::Leaf7),
                ) => {
                    if anc_depth == 0 {
                        return OlcOutcome::Fallback(FallbackCause::RootGrowth);
                    }
                    let parent = ancestors[anc_depth - 1];
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let p_cell = unsafe { crate::occ::version_cell(parent.version_ptr) };
                    let kb = t.leaf_key_bytes().expect("leaf tag") as usize;
                    let pop = edge.pop0(kb as u8) as usize + 1;
                    if kb < level as usize
                        && !crate::get::decode_matches(&edge, key, kb as u8, level)
                    {
                        return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                    }
                    let k = crate::mutate::key_low(key, kb as u8);
                    let base = edge.node_ptr();
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let keys_ptr = unsafe { base.add(crate::leaf::map_keys_offset(pop)) };
                    let mut found = None;
                    let mut at = pop;
                    for i in 0..pop {
                        // SAFETY: pointer and index are within bounds of valid leaf/immediate allocation.
                        let existing = unsafe { crate::mutate::read_packed(keys_ptr, i, kb) };
                        if existing == k {
                            found = Some(i);
                            break;
                        } else if existing > k && at == pop {
                            at = i;
                        }
                    }
                    if let Some(pos) = found {
                        let Ok((old_v, lock_t0)) =
                            version_try_lock_expect_timed(p_cell, parent.version_snap)
                        else {
                            return OlcOutcome::Retry;
                        };
                        // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                        if !unsafe { edge_ptr.read() }.bits_eq(&edge) {
                            version_unlock_timed(p_cell, old_v, false, lock_t0);
                            return OlcOutcome::Retry;
                        }
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        unsafe {
                            let slot = base.cast::<u64>().add(pos);
                            let old = slot.read();
                            slot.write(val);
                            version_unlock_timed(p_cell, old_v, true, lock_t0);
                            return OlcOutcome::Done(Some(old));
                        }
                    }
                    let cap = if kb == 1 {
                        crate::mutate::LEAF1_CAP
                    } else {
                        crate::mutate::LEAF_CAP
                    };
                    if pop < cap && crate::leaf::cap_class(pop + 1) == crate::leaf::cap_class(pop) {
                        let Ok((old_v, lock_t0)) =
                            version_try_lock_expect_timed(p_cell, parent.version_snap)
                        else {
                            return OlcOutcome::Retry;
                        };
                        // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                        if !unsafe { edge_ptr.read() }.bits_eq(&edge) {
                            version_unlock_timed(p_cell, old_v, false, lock_t0);
                            return OlcOutcome::Retry;
                        }
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        unsafe {
                            crate::leaf::map_insert_at(base, kb as u8, pop, at, k, val);
                            (*edge_ptr).set_pop0(kb as u8, pop as u64);
                            version_unlock_timed(p_cell, old_v, true, lock_t0);
                            for i in (0..anc_depth.saturating_sub(1)).rev() {
                                let a = &ancestors[i];
                                bump_ancestor_pop0(a.node, a.edge_type, a.child_level, a.digit, 1);
                            }
                        }
                        return OlcOutcome::Done(None);
                    }
                    return OlcOutcome::Fallback(FallbackCause::CapExpansion);
                }

                EdgeTag::Immed(im) => {
                    if anc_depth == 0 {
                        return OlcOutcome::Fallback(FallbackCause::RootGrowth);
                    }
                    let parent = ancestors[anc_depth - 1];
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let p_cell = unsafe { crate::occ::version_cell(parent.version_ptr) };
                    let kb = im.key_bytes();
                    if level > kb && !crate::get::decode_matches(&edge, key, kb, level) {
                        return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                    }
                    let k = crate::mutate::key_low(key, kb);
                    let n = im.key_count() as usize;
                    let kb_usize = kb as usize;
                    if n == 1 {
                        let mask = if kb >= 8 {
                            u64::MAX
                        } else {
                            (1u64 << (kb * 8)) - 1
                        };
                        let existing_k = edge.aux_word() & mask;
                        if existing_k == k {
                            let Ok((old_v, lock_t0)) =
                                version_try_lock_expect_timed(p_cell, parent.version_snap)
                            else {
                                return OlcOutcome::Retry;
                            };
                            // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                            if !unsafe { edge_ptr.read() }.bits_eq(&edge) {
                                version_unlock_timed(p_cell, old_v, false, lock_t0);
                                return OlcOutcome::Retry;
                            }
                            // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                            unsafe {
                                let old = (*edge_ptr).word0();
                                (*edge_ptr).set_imm_bytes(val.to_le_bytes());
                                version_unlock_timed(p_cell, old_v, true, lock_t0);
                                return OlcOutcome::Done(Some(old));
                            }
                        }
                        return OlcOutcome::Fallback(FallbackCause::ImmediateConversion);
                    }
                    // SAFETY: aux_bytes slice is within immediate edge descriptor.
                    let pos =
                        (unsafe { crate::leaf::locate(edge.aux_bytes().as_ptr(), n, kb, k) }).ok();
                    if let Some(p) = pos {
                        let Ok((old_v, lock_t0)) =
                            version_try_lock_expect_timed(p_cell, parent.version_snap)
                        else {
                            return OlcOutcome::Retry;
                        };
                        // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                        if !unsafe { edge_ptr.read() }.bits_eq(&edge) {
                            version_unlock_timed(p_cell, old_v, false, lock_t0);
                            return OlcOutcome::Retry;
                        }
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        unsafe {
                            let vals = edge.node_ptr().cast::<u64>();
                            let slot = vals.add(p);
                            let old = slot.read();
                            slot.write(val);
                            version_unlock_timed(p_cell, old_v, true, lock_t0);
                            return OlcOutcome::Done(Some(old));
                        }
                    }
                    if n < crate::mutate::map_immed_max(kb)
                        && crate::leaf::cap_class(n + 1) == crate::leaf::cap_class(n)
                    {
                        let Ok((old_v, lock_t0)) =
                            version_try_lock_expect_timed(p_cell, parent.version_snap)
                        else {
                            return OlcOutcome::Retry;
                        };
                        // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                        if !unsafe { edge_ptr.read() }.bits_eq(&edge) {
                            version_unlock_timed(p_cell, old_v, false, lock_t0);
                            return OlcOutcome::Retry;
                        }
                        // SAFETY: pointer and index are within bounds of valid leaf/immediate allocation.
                        let ins_pos = match unsafe {
                            crate::leaf::locate(edge.aux_bytes().as_ptr(), n, kb, k)
                        } {
                            Ok(_) => {
                                version_unlock_timed(p_cell, old_v, false, lock_t0);
                                return OlcOutcome::Retry;
                            }
                            Err(p) => p,
                        };
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        unsafe {
                            let old_vals = edge.node_ptr().cast::<u64>();
                            if ins_pos < n {
                                core::ptr::copy(
                                    old_vals.add(ins_pos),
                                    old_vals.add(ins_pos + 1),
                                    n - ins_pos,
                                );
                            }
                            old_vals.add(ins_pos).write(val);
                            let mut new_aux = *(*edge_ptr).aux_bytes();
                            if ins_pos < n {
                                new_aux.copy_within(
                                    ins_pos * kb_usize..n * kb_usize,
                                    (ins_pos + 1) * kb_usize,
                                );
                            }
                            crate::mutate::write_packed(new_aux.as_mut_ptr(), ins_pos, kb_usize, k);
                            let new_im =
                                ImmedType::new(kb, (n + 1) as u8).expect("immediate capacity");
                            (*edge_ptr).set_aux_bytes(new_aux);
                            (*edge_ptr).set_tag(new_im.as_u8());
                            version_unlock_timed(p_cell, old_v, true, lock_t0);
                            for i in (0..anc_depth.saturating_sub(1)).rev() {
                                let a = &ancestors[i];
                                bump_ancestor_pop0(a.node, a.edge_type, a.child_level, a.digit, 1);
                            }
                        }
                        return OlcOutcome::Done(None);
                    }
                    return OlcOutcome::Fallback(FallbackCause::ImmediateConversion);
                }

                EdgeTag::Structural(EdgeType::Null) => {
                    return OlcOutcome::Fallback(FallbackCause::ImmediateConversion);
                }

                #[allow(unreachable_patterns)]
                _ => return OlcOutcome::Fallback(FallbackCause::UnknownTag),
            }
        }
    }

    #[cfg(feature = "std")]
    fn olc_remove_map(&self, key: Key) -> OlcOutcome<Option<u64>> {
        let v_snap = self.shared.version().sample();
        if (v_snap & 1) != 0 {
            return OlcOutcome::Retry;
        }
        // SAFETY: top_ptr obtained without taking &mut on inner.
        let top_ptr = unsafe { (*self.shared.inner.get()).root_top_ptr() };
        if top_ptr.is_null() {
            return OlcOutcome::Fallback(FallbackCause::RootGrowth);
        }
        let mut ancestors: [AncestorFrame; 8] = [AncestorFrame {
            node: core::ptr::null_mut(),
            edge_type: EdgeType::Null,
            version_ptr: core::ptr::null(),
            version_snap: 0,
            child_level: 0,
            digit: 0,
        }; 8];
        let mut anc_depth = 0;
        // SAFETY: top_ptr points to the root edge under verified EBR-live snapshot.
        let mut edge = unsafe { top_ptr.read() };
        let mut edge_ptr = top_ptr;
        let mut level = 8u8;

        loop {
            let tag = edge.tag().expect("valid edge tag");
            match tag {
                EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
                    let is_l3 = matches!(t, EdgeType::BranchL3);
                    let node = edge.node_ptr();
                    let vp = if is_l3 {
                        // SAFETY: node pointer is EBR-live and validated by parent version check.
                        unsafe { &raw const (*node.cast::<BranchL3>()).hdr.version }
                    } else {
                        // SAFETY: node pointer is EBR-live and validated by parent version check.
                        unsafe { &raw const (*node.cast::<BranchL7>()).hdr.version }
                    };
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let Some(nsnap) =
                        (unsafe { crate::occ::node_sample(crate::occ::version_cell(vp)) })
                    else {
                        return OlcOutcome::Retry;
                    };
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let (bl, num, slot_opt) = unsafe {
                        if is_l3 {
                            let b = node.cast::<BranchL3>();
                            let bl = (*b).hdr.level;
                            let num = (*b).hdr.num as usize;
                            (bl, num, (*b).hdr.find(digit(key, bl)))
                        } else {
                            let b = node.cast::<BranchL7>();
                            let bl = (*b).hdr.level;
                            let num = (*b).hdr.num as usize;
                            (bl, num, (*b).hdr.find(digit(key, bl)))
                        }
                    };
                    if !(2..=level).contains(&bl) || num > if is_l3 { 3 } else { 7 } {
                        return OlcOutcome::Retry;
                    }
                    if bl < level && !crate::get::decode_matches(&edge, key, bl, level) {
                        return OlcOutcome::Done(None);
                    }
                    let d = digit(key, bl);
                    // SAFETY: version cell is within an EBR-live node allocation.
                    if !crate::occ::node_validate(unsafe { crate::occ::version_cell(vp) }, nsnap) {
                        return OlcOutcome::Retry;
                    }
                    if let Some(slot) = slot_opt {
                        ancestors[anc_depth] = AncestorFrame {
                            node,
                            edge_type: t,
                            version_ptr: vp,
                            version_snap: nsnap,
                            child_level: bl - 1,
                            digit: d,
                        };
                        anc_depth += 1;
                        edge_ptr = if is_l3 {
                            // SAFETY: node pointer is EBR-live and validated by parent version check.
                            unsafe { &raw mut (*node.cast::<BranchL3>()).edges[slot] }
                        } else {
                            // SAFETY: node pointer is EBR-live and validated by parent version check.
                            unsafe { &raw mut (*node.cast::<BranchL7>()).edges[slot] }
                        };
                        // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                        edge = unsafe { edge_ptr.read() };
                        level = bl - 1;
                        continue;
                    }
                    return OlcOutcome::Done(None);
                }

                EdgeTag::Structural(EdgeType::BranchB) => {
                    let node = edge.node_ptr().cast::<BranchB>();
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let vp = unsafe { &raw const (*node).version };
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let Some(nsnap) =
                        (unsafe { crate::occ::node_sample(crate::occ::version_cell(vp)) })
                    else {
                        return OlcOutcome::Retry;
                    };
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let (bl, bit, rank, sub) = unsafe {
                        let bl = (*node).level;
                        if !(2..=level).contains(&bl) {
                            return OlcOutcome::Retry;
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
                        return OlcOutcome::Done(None);
                    }
                    if !bit || sub.is_null() {
                        return OlcOutcome::Done(None);
                    }
                    // SAFETY: version cell is within an EBR-live node allocation.
                    if !crate::occ::node_validate(unsafe { crate::occ::version_cell(vp) }, nsnap) {
                        return OlcOutcome::Retry;
                    }
                    ancestors[anc_depth] = AncestorFrame {
                        node: node.cast(),
                        edge_type: EdgeType::BranchB,
                        version_ptr: vp,
                        version_snap: nsnap,
                        child_level: bl - 1,
                        digit: digit(key, bl),
                    };
                    anc_depth += 1;
                    // SAFETY: pointer arithmetic and destination buffer bounds are valid under locked parent.
                    edge_ptr = unsafe { sub.add(rank) };
                    // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                    edge = unsafe { edge_ptr.read() };
                    level = bl - 1;
                    continue;
                }

                EdgeTag::Structural(EdgeType::BranchU) => {
                    let node = edge.node_ptr().cast::<BranchU>();
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let vp = unsafe { &raw const (*node).version };
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let Some(nsnap) =
                        (unsafe { crate::occ::node_sample(crate::occ::version_cell(vp)) })
                    else {
                        return OlcOutcome::Retry;
                    };
                    let d = digit(key, level);
                    // SAFETY: version cell is within an EBR-live node allocation.
                    if !crate::occ::node_validate(unsafe { crate::occ::version_cell(vp) }, nsnap) {
                        return OlcOutcome::Retry;
                    }
                    ancestors[anc_depth] = AncestorFrame {
                        node: node.cast(),
                        edge_type: EdgeType::BranchU,
                        version_ptr: vp,
                        version_snap: nsnap,
                        child_level: level - 1,
                        digit: d,
                    };
                    anc_depth += 1;
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    edge_ptr = unsafe { &raw mut (*node).edges[d as usize] };
                    // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                    edge = unsafe { edge_ptr.read() };
                    level -= 1;
                    continue;
                }

                EdgeTag::Structural(EdgeType::LeafB1) => {
                    if anc_depth == 0 {
                        return OlcOutcome::Fallback(FallbackCause::RootGrowth);
                    }
                    if level > 1 && !crate::get::decode_matches(&edge, key, 1, level) {
                        return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                    }
                    let parent = ancestors[anc_depth - 1];
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let p_cell = unsafe { crate::occ::version_cell(parent.version_ptr) };
                    let node = edge.node_ptr().cast::<LeafBitmapL>();
                    let d = (key & 0xFF) as u8;
                    let sub = (d >> 5) as usize;
                    // SAFETY: node is an EBR-live bitmap node and parent is validated/locked.
                    let Some((_, rank)) =
                        (unsafe { (*node).bitmap.test_and_subexpanse_rank_with_sub(d) })
                    else {
                        if !crate::occ::node_validate(p_cell, parent.version_snap) {
                            return OlcOutcome::Retry;
                        }
                        return OlcOutcome::Done(None);
                    };
                    let pop0 = edge.pop0(1) as usize;
                    if pop0 == 0 || pop0 <= 32 {
                        return OlcOutcome::Fallback(FallbackCause::CapExpansion);
                    }
                    // SAFETY: node is an EBR-live bitmap node and parent is validated/locked.
                    let old_n = unsafe { (*node).bitmap.subexpanse_count(sub) as usize };
                    if old_n > 1
                        && crate::leaf::cap_class(old_n - 1) == crate::leaf::cap_class(old_n)
                    {
                        let Ok((old_v, lock_t0)) =
                            version_try_lock_expect_timed(p_cell, parent.version_snap)
                        else {
                            return OlcOutcome::Retry;
                        };
                        // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                        if !unsafe { edge_ptr.read() }.bits_eq(&edge) {
                            version_unlock_timed(p_cell, old_v, false, lock_t0);
                            return OlcOutcome::Retry;
                        }
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        unsafe {
                            let arr = (*node).values[sub];
                            let old = arr.add(rank).read();
                            core::ptr::copy(arr.add(rank + 1), arr.add(rank), old_n - 1 - rank);
                            (*node).bitmap.clear(d);
                            (*edge_ptr).set_pop0(1, (pop0 - 1) as u64);
                            version_unlock_timed(p_cell, old_v, true, lock_t0);
                            for i in (0..anc_depth.saturating_sub(1)).rev() {
                                let a = &ancestors[i];
                                bump_ancestor_pop0(a.node, a.edge_type, a.child_level, a.digit, -1);
                            }
                            return OlcOutcome::Done(Some(old));
                        }
                    }
                    return OlcOutcome::Fallback(FallbackCause::CapExpansion);
                }

                EdgeTag::Structural(
                    t @ (EdgeType::Leaf1
                    | EdgeType::Leaf2
                    | EdgeType::Leaf3
                    | EdgeType::Leaf4
                    | EdgeType::Leaf5
                    | EdgeType::Leaf6
                    | EdgeType::Leaf7),
                ) => {
                    if anc_depth == 0 {
                        return OlcOutcome::Fallback(FallbackCause::RootGrowth);
                    }
                    let parent = ancestors[anc_depth - 1];
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let p_cell = unsafe { crate::occ::version_cell(parent.version_ptr) };
                    let kb = t.leaf_key_bytes().expect("leaf tag") as usize;
                    let pop = edge.pop0(kb as u8) as usize + 1;
                    if kb < level as usize
                        && !crate::get::decode_matches(&edge, key, kb as u8, level)
                    {
                        return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                    }
                    let k = crate::mutate::key_low(key, kb as u8);
                    let base = edge.node_ptr();
                    // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                    let keys_ptr = unsafe { base.add(crate::leaf::map_keys_offset(pop)) };
                    let mut found = None;
                    for i in 0..pop {
                        // SAFETY: pointer and index are within bounds of valid leaf/immediate allocation.
                        if unsafe { crate::mutate::read_packed(keys_ptr, i, kb) } == k {
                            found = Some(i);
                            break;
                        }
                    }
                    let Some(pos) = found else {
                        if !crate::occ::node_validate(p_cell, parent.version_snap) {
                            return OlcOutcome::Retry;
                        }
                        return OlcOutcome::Done(None);
                    };
                    if pop > 2
                        && crate::leaf::cap_class(pop - 1) == crate::leaf::cap_class(pop)
                        && pop > crate::mutate::map_immed_max(kb as u8)
                    {
                        let Ok((old_v, lock_t0)) =
                            version_try_lock_expect_timed(p_cell, parent.version_snap)
                        else {
                            return OlcOutcome::Retry;
                        };
                        // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                        if !unsafe { edge_ptr.read() }.bits_eq(&edge) {
                            version_unlock_timed(p_cell, old_v, false, lock_t0);
                            return OlcOutcome::Retry;
                        }
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        unsafe {
                            let old = base.cast::<u64>().add(pos).read();
                            crate::leaf::map_remove_at(base, kb as u8, pop, pos);
                            (*edge_ptr).set_pop0(kb as u8, (pop - 2) as u64);
                            version_unlock_timed(p_cell, old_v, true, lock_t0);
                            for i in (0..anc_depth.saturating_sub(1)).rev() {
                                let a = &ancestors[i];
                                bump_ancestor_pop0(a.node, a.edge_type, a.child_level, a.digit, -1);
                            }
                            return OlcOutcome::Done(Some(old));
                        }
                    }
                    return OlcOutcome::Fallback(FallbackCause::CapExpansion);
                }

                EdgeTag::Immed(im) => {
                    if anc_depth == 0 {
                        return OlcOutcome::Fallback(FallbackCause::RootGrowth);
                    }
                    let parent = ancestors[anc_depth - 1];
                    // SAFETY: version cell is within an EBR-live node allocation.
                    let p_cell = unsafe { crate::occ::version_cell(parent.version_ptr) };
                    let kb = im.key_bytes();
                    if level > kb && !crate::get::decode_matches(&edge, key, kb, level) {
                        return OlcOutcome::Fallback(FallbackCause::BranchSplit);
                    }
                    let k = crate::mutate::key_low(key, kb);
                    let n = im.key_count() as usize;
                    let kb_usize = kb as usize;
                    if n == 1 {
                        let mask = if kb >= 8 {
                            u64::MAX
                        } else {
                            (1u64 << (kb * 8)) - 1
                        };
                        let existing_k = edge.aux_word() & mask;
                        if existing_k == k {
                            let Ok((old_v, lock_t0)) =
                                version_try_lock_expect_timed(p_cell, parent.version_snap)
                            else {
                                return OlcOutcome::Retry;
                            };
                            // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                            if !unsafe { edge_ptr.read() }.bits_eq(&edge) {
                                version_unlock_timed(p_cell, old_v, false, lock_t0);
                                return OlcOutcome::Retry;
                            }
                            // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                            unsafe {
                                let old = (*edge_ptr).word0();
                                (*edge_ptr) = Edge::NULL;
                                version_unlock_timed(p_cell, old_v, true, lock_t0);
                                for i in (0..anc_depth.saturating_sub(1)).rev() {
                                    let a = &ancestors[i];
                                    bump_ancestor_pop0(
                                        a.node,
                                        a.edge_type,
                                        a.child_level,
                                        a.digit,
                                        -1,
                                    );
                                }
                                return OlcOutcome::Done(Some(old));
                            }
                        }
                        if !crate::occ::node_validate(p_cell, parent.version_snap) {
                            return OlcOutcome::Retry;
                        }
                        return OlcOutcome::Done(None);
                    }
                    // SAFETY: aux_bytes slice is within immediate edge descriptor.
                    let pos =
                        (unsafe { crate::leaf::locate(edge.aux_bytes().as_ptr(), n, kb, k) }).ok();
                    let Some(p) = pos else {
                        if !crate::occ::node_validate(p_cell, parent.version_snap) {
                            return OlcOutcome::Retry;
                        }
                        return OlcOutcome::Done(None);
                    };
                    if n > 2 && crate::leaf::cap_class(n - 1) == crate::leaf::cap_class(n) {
                        let Ok((old_v, lock_t0)) =
                            version_try_lock_expect_timed(p_cell, parent.version_snap)
                        else {
                            return OlcOutcome::Retry;
                        };
                        // SAFETY: edge_ptr points to an EBR-live edge inside a validated ancestor node or root.
                        if !unsafe { edge_ptr.read() }.bits_eq(&edge) {
                            version_unlock_timed(p_cell, old_v, false, lock_t0);
                            return OlcOutcome::Retry;
                        }
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        unsafe {
                            let vals = edge.node_ptr().cast::<u64>();
                            let old = vals.add(p).read();
                            if p + 1 < n {
                                core::ptr::copy(vals.add(p + 1), vals.add(p), n - 1 - p);
                            }
                            let mut new_aux = *(*edge_ptr).aux_bytes();
                            new_aux.copy_within((p + 1) * kb_usize..n * kb_usize, p * kb_usize);
                            new_aux[((n - 1) * kb_usize)..].fill(0);
                            let new_im =
                                ImmedType::new(kb, (n - 1) as u8).expect("immediate capacity");
                            (*edge_ptr).set_aux_bytes(new_aux);
                            (*edge_ptr).set_tag(new_im.as_u8());
                            version_unlock_timed(p_cell, old_v, true, lock_t0);
                            for i in (0..anc_depth.saturating_sub(1)).rev() {
                                let a = &ancestors[i];
                                bump_ancestor_pop0(a.node, a.edge_type, a.child_level, a.digit, -1);
                            }
                            return OlcOutcome::Done(Some(old));
                        }
                    }
                    return OlcOutcome::Fallback(FallbackCause::ImmediateConversion);
                }

                EdgeTag::Structural(EdgeType::Null) => {
                    if anc_depth > 0 {
                        let parent = ancestors[anc_depth - 1];
                        // SAFETY: version cell is within an EBR-live node allocation.
                        let p_cell = unsafe { crate::occ::version_cell(parent.version_ptr) };
                        if !crate::occ::node_validate(p_cell, parent.version_snap) {
                            return OlcOutcome::Retry;
                        }
                    }
                    return OlcOutcome::Done(None);
                }

                #[allow(unreachable_patterns)]
                _ => return OlcOutcome::Fallback(FallbackCause::UnknownTag),
            }
        }
    }

    /// Removes every key-value pair from the map.
    pub fn clear(&self) {
        self.shared.write_root_covered(|m| {
            m.clear();
            self.shared
                .tree_pop
                .store(0, core::sync::atomic::Ordering::Relaxed);
        });
    }

    /// Registers a reader handle for this thread's lookups.
    #[must_use]
    pub fn reader(&self) -> MapReader<'_> {
        MapReader {
            map: self,
            reader: self.shared.collector.register(),
        }
    }

    /// A reader that owns its handle on the map, for callers that cannot hold
    /// a borrow — a `#[pyclass]`, a thread-local, anything outliving the call.
    ///
    /// Registers once, so a loop through it pays none of the per-lookup
    /// registry locking that [`Self::get`] does (#554). One reader owns one
    /// epoch slot and its pins are not reentrant, so give each thread its own
    /// rather than sharing one.
    #[must_use]
    pub fn owned_reader(self: &Arc<Self>) -> OwnedMapReader {
        OwnedMapReader {
            map: Arc::clone(self),
            reader: self.shared.collector.register(),
        }
    }

    /// Registers a reader that holds no reference to this map.
    ///
    /// Use this where the reader is cached somewhere whose lifetime is not the
    /// map's -- a per-thread cache, say -- and pass the map back in at lookup
    /// time. See [`DetachedMapReader`].
    #[must_use]
    pub fn detached_reader(&self) -> DetachedMapReader {
        DetachedMapReader {
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

/// The validated walk shared by [`MapReader`] and [`OwnedMapReader`].
///
/// Split out so the borrowing and owning readers cannot drift: both are the
/// same protocol, differing only in how they hold the map (#554).
fn map_get_with(map: &SyncExpanseMap, reader: &Reader, key: Key) -> Option<u64> {
    let shared = &map.shared;
    crate::occ_stats::bump(crate::occ_stats::Stat::ReadOps);
    for _ in 0..MAX_RETRIES {
        crate::occ_stats::bump(crate::occ_stats::Stat::ReadAttempts);
        let _pin = reader.pin();
        let snap = shared.version().sample();
        // SAFETY: pinned + freshly sampled version; the walk validates every
        // load (see `walk_validated`).
        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
        let root = unsafe { (*shared.inner.get()).occ_root().0 };
        // SAFETY: same pin + snapshot contract as the line above.
        let walked = unsafe { walk_validated::<true>(root, key, shared.version(), snap) };
        if let Ok(r) = walked {
            return r;
        }
    }
    crate::occ_stats::bump(crate::occ_stats::Stat::ReadFallbacks);
    shared.read_locked(|m| m.get(key))
}

/// A reader that **owns** its map handle instead of borrowing it.
///
/// [`MapReader`] borrows, which makes it impossible to cache: a `#[pyclass]`,
/// a thread-local, or any struct outliving the call cannot hold it. Without a
/// cacheable reader every binding lookup went through the one-shot
/// [`SyncExpanseMap::get`], which registers a throwaway reader — two
/// acquisitions of the collector's registry mutex per lookup, plus an O(n)
/// `retain` on drop. Concurrent Python readers serialised on that and scaled
/// to 0.02x at 16 threads, worse than a GIL-bound `dict` (#554).
///
/// One [`Reader`] owns a single epoch slot and its pins are not reentrant, so
/// an `OwnedMapReader` must not be shared between threads: give each thread
/// its own.
pub struct OwnedMapReader {
    map: Arc<SyncExpanseMap>,
    reader: Reader,
}

impl OwnedMapReader {
    /// Optimistic lookup, without the per-call registry lock `get` pays.
    #[must_use]
    pub fn get(&self, key: Key) -> Option<u64> {
        map_get_with(&self.map, &self.reader, key)
    }
}

/// A reader that owns neither its map nor a reference to it.
///
/// [`OwnedMapReader`] holds an `Arc<SyncExpanseMap>` so it can be cached
/// without a borrow. That is the right shape when the cache's lifetime is the
/// reader's, and the wrong one when the cache outlives the map: a per-thread
/// cache keyed by map keeps every map a thread ever read from alive for the
/// life of that thread, and nothing in the map's own drop can evict it.
///
/// This variant holds only the epoch slot, so caching one says nothing about
/// how long the map lives. The caller supplies the map at lookup time, which it
/// necessarily has anyway — it is the thing being read.
///
/// The same non-reentrancy rule applies: one [`Reader`] owns a single epoch
/// slot, so a `DetachedMapReader` must not be shared between threads.
pub struct DetachedMapReader {
    reader: Reader,
}

impl DetachedMapReader {
    /// Optimistic lookup against `map`, without the per-call registry lock
    /// [`SyncExpanseMap::get`] pays.
    ///
    /// `map` must be the map this reader was registered against; reading a
    /// different map through it would validate against the wrong collector.
    #[must_use]
    pub fn get(&self, map: &SyncExpanseMap, key: Key) -> Option<u64> {
        map_get_with(map, &self.reader, key)
    }
}

impl MapReader<'_> {
    /// Optimistic lookup.
    #[must_use]
    pub fn get(&self, key: Key) -> Option<u64> {
        map_get_with(self.map, &self.reader, key)
    }
}

/// A blob map shareable across threads (issue #219 Phase 1): one writer at a
/// time (internally serialized), validated optimistic readers with epoch-pinned
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
    shared: Box<Shared<ExpanseBlobMap>>,
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
        crate::occ_stats::bump(crate::occ_stats::Stat::Inserts);
        self.shared.write(|m| {
            let old = m.len();
            let r = m.insert(key, data, hot_meta);
            if r.is_ok() && m.len() > old {
                self.shared
                    .tree_pop
                    .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
            }
            r
        })
    }

    /// Removes `key`; returns `true` if it was present.
    pub fn remove(&self, key: Key) -> bool {
        self.shared.write(|m| {
            let r = m.remove(key);
            if r {
                self.shared
                    .tree_pop
                    .fetch_sub(1, core::sync::atomic::Ordering::Relaxed);
            }
            r
        })
    }

    /// Runs arena garbage collection and compaction. Dead chunks are retired
    /// through the epoch collector, so concurrent pinned readers keep reading
    /// their (relocated-from) payload bytes safely.
    pub fn compact(&self) -> Result<CompactionStats, ArenaError> {
        self.shared.write(ExpanseBlobMap::compact)
    }

    /// Removes every entry and retires all arena chunks.
    pub fn clear(&self) {
        self.shared.write(|m| {
            m.clear();
            self.shared
                .tree_pop
                .store(0, core::sync::atomic::Ordering::Relaxed);
        });
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
/// `get_meta` paths (optimistic and locked fallback) share it so they cannot
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
    /// guard stay valid (and byte-stable) until the guard drops.
    ///
    /// # Reclamation Deferral Contract & Lifetime Discipline
    ///
    /// Holding a [`BlobReadGuard`] defers epoch reclamation **tree-wide** — both
    /// for arena chunk slabs and for the underlying radix index trie nodes.
    /// While the guard is held, concurrent writers continue retiring superseded
    /// trie nodes and compacted chunks, but `Collector::try_advance` cannot
    /// advance past the pinned epoch and frees nothing. Under write churn,
    /// retained garbage accumulates proportionally to mutation volume.
    ///
    /// **Best Practice:** Keep guards strictly short-lived within tight lexical
    /// scopes. For long-lived or asynchronous storage across I/O boundaries,
    /// prefer [`Self::get`] (which copies to an owned [`Vec<u8>`] and unpins
    /// immediately) or copy the payload via [`SyncBlobView::as_bytes`].
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

    /// Optimistic owned-copy lookup (pins only for the duration of the call).
    #[must_use]
    pub fn get(&mut self, key: Key) -> Option<(Vec<u8>, u32)> {
        let guard = self.pin();
        guard
            .get(key)
            .map(|(view, meta)| (view.as_bytes().to_vec(), meta))
    }

    /// Bounded validated optimistic slot-word lookup shared by the word-level
    /// reads; `Err(Retry)` after retry exhaustion (the caller then falls
    /// back under the writer lock).
    fn lookup_slot(&mut self, key: Key) -> Result<Option<u64>, Retry> {
        let shared = &self.map.shared;
        crate::occ_stats::bump(crate::occ_stats::Stat::ReadOps);
        for _ in 0..MAX_RETRIES {
            crate::occ_stats::bump(crate::occ_stats::Stat::ReadAttempts);
            let _pin = self.reader.pin();
            let snap = shared.version().sample();
            // SAFETY: pinned + freshly sampled version; the walk validates
            // every load (see `walk_validated`).
            // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
            let root = unsafe { (*shared.inner.get()).index().occ_root().0 };
            // SAFETY: same pin + snapshot contract as the line above.
            let walked = unsafe { walk_validated::<true>(root, key, shared.version(), snap) };
            if let Ok(found) = walked {
                return Ok(found);
            }
        }
        Err(Retry)
    }

    /// Optimistic metadata lookup: the validated slot word alone answers it —
    /// no payload cache line is touched (inline payloads report `0`, as in
    /// [`ExpanseBlobMap::get`]; non-payload slots return `None`). Because
    /// the payload is never resolved, a dangling arena locator (possible
    /// only in a corrupted image) still reports its stored metadata.
    #[must_use]
    pub fn get_meta(&mut self, key: Key) -> Option<u32> {
        match self.lookup_slot(key) {
            Ok(found) => found.and_then(blob_slot_meta),
            Err(Retry) => {
                crate::occ_stats::bump(crate::occ_stats::Stat::ReadFallbacks);
                self.map
                    .shared
                    .read_locked(|m| m.index().get(key).and_then(blob_slot_meta))
            }
        }
    }

    /// Optimistic membership test.
    #[must_use]
    pub fn contains(&mut self, key: Key) -> bool {
        match self.lookup_slot(key) {
            Ok(found) => found.is_some(),
            Err(Retry) => {
                crate::occ_stats::bump(crate::occ_stats::Stat::ReadFallbacks);
                self.map.shared.read_locked(|m| m.contains_key(key))
            }
        }
    }
}

/// An epoch-pinned read guard for [`SyncExpanseBlobMap`] — the
/// `SyncBlobReaderGuard` of issue #219: while it lives, nothing the arena or
/// index retires is freed, so the [`SyncBlobView`] borrows it hands out stay
/// valid across concurrent writes and compactions.
///
/// # Reclamation Deferral & Memory Growth
///
/// Because reclamation is epoch-based (EBR), holding this guard freezes epoch
/// advances for the **entire map** (both arena chunk slabs and index trie nodes).
/// Unreclaimed garbage accumulates across writer mutations until this guard drops.
///
/// To avoid unbounded memory retention under write churn, do not hold this guard
/// across slow caller operations, async yields, or network I/O. Use [`BlobReader::get`]
/// or copy the payload view via `view.as_bytes().to_vec()` when data must outlive
/// the immediate read scope.
pub struct BlobReadGuard<'g> {
    map: &'g SyncExpanseBlobMap,
    _pin: Pin<'g>,
}

impl BlobReadGuard<'_> {
    /// Validated optimistic lookup. Inline payloads are decoded by value from
    /// the validated slot word; arena payloads are zero-copy borrows of
    /// epoch-pinned slab bytes. Falls back to an owned copy under the writer
    /// lock after bounded retries.
    #[must_use]
    pub fn get(&self, key: Key) -> Option<(SyncBlobView<'_>, u32)> {
        let shared = &self.map.shared;
        crate::occ_stats::bump(crate::occ_stats::Stat::ReadOps);
        for _ in 0..MAX_RETRIES {
            crate::occ_stats::bump(crate::occ_stats::Stat::ReadAttempts);
            let snap = shared.version().sample();
            // SAFETY: the guard's pin predates this sample; the walk
            // validates every load (see `walk_validated`).
            // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
            let root = unsafe { (*shared.inner.get()).index().occ_root().0 };
            // SAFETY: same pin + snapshot contract as the line above.
            let Ok(found) = (unsafe { walk_validated::<true>(root, key, shared.version(), snap) })
            else {
                continue;
            };
            // The walk validated this result: absent stays absent, and a
            // present slot word is the value the key held at `snap`.
            let raw = found?;
            let slot = ValueSlot::from_raw(raw);
            let tag = slot.tag();
            if tag.is_raw_inline() {
                let (raw_buf, len) = slot.inline_payload();
                let mut buf = [0u8; 16];
                buf[..7].copy_from_slice(&raw_buf);
                return Some((
                    SyncBlobView::Inline {
                        buf,
                        len: len as u8,
                    },
                    0,
                ));
            }
            if tag.is_compressed_inline() {
                let mut buf = [0u8; 16];
                if let Some(len) = crate::codec::decompress_inline(slot, &mut buf) {
                    return Some((
                        SyncBlobView::Inline {
                            buf,
                            len: len as u8,
                        },
                        0,
                    ));
                }
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
            // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
            let table = unsafe { (*shared.inner.get()).arena().reader_table() };
            // SAFETY: the guard's pin predates the table load, so the table
            // and every chunk it references are EBR-live.
            let resolved =
                // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                unsafe { crate::blobmap::resolve_meta_in_table(table, slot.arena_meta_locator()) };
            match resolved {
                Some((ptr, len)) => {
                    if shared.version().validate(snap) {
                        // No writer overlapped: the resolution used the
                        // table consistent with `snap`, so `ptr..ptr+len` is
                        // the record's live payload. Arena records are never
                        // rewritten in place and retired chunks stay mapped
                        // under this guard's pin, so the borrow is
                        // byte-stable for the guard's lifetime.
                        // SAFETY: in-bounds of an EBR-live chunk (see
                        // `resolve_meta_in_table`), immutable while pinned.
                        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                        let bytes = unsafe { core::slice::from_raw_parts(ptr, len) };
                        return Some((SyncBlobView::Arena(bytes), meta));
                    }
                }
                None => {
                    if shared.version().validate(snap) {
                        // Validated dangling locator — mirrors the
                        // single-threaded `get` returning `None`.
                        return None;
                    }
                }
            }
        }
        crate::occ_stats::bump(crate::occ_stats::Stat::ReadFallbacks);
        shared.read_locked(|m| {
            m.get(key)
                .map(|(view, meta)| (SyncBlobView::Owned(view.as_bytes().to_vec()), meta))
        })
    }
}

/// A validated payload view obtained through a [`BlobReadGuard`].
///
/// `Inline` payloads are decoded **by value** from the validated slot word
/// (they are at most 7 raw bytes or 8-14 compressed bytes; copying beats exposing racy leaf-slot memory).
/// `Arena` payloads borrow the epoch-pinned slab bytes zero-copy. `Owned` is
/// the bounded-retry fallback, copied under the writer lock.
#[derive(Clone, Debug)]
pub enum SyncBlobView<'g> {
    /// Inline payload decoded from the value-slot word.
    Inline {
        /// Payload bytes; only the first `len` are meaningful.
        buf: [u8; 16],
        /// Meaningful prefix length of `buf` (≤ 16).
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
/// at a time (internally serialized), validated optimistic readers for
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
    shared: Box<Shared<ExpanseStrMap>>,
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
            // SAFETY: `key` came out of a `StrMap` walk, so it is already in
            // the NUL-free domain by construction.
            // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
            let k = unsafe { NulFreeStr::new_unchecked(&key) };
            // SAFETY: the slot is valid until `src`'s next mutation; only
            // navigation happens between here and the next hop.
            // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
            let v = unsafe { *slot.as_ptr() };
            map.insert(k, v);
            cursor = src.next_after(k);
        }
        Self {
            shared: Shared::with_collector(map, collector),
        }
    }

    /// Inserts `key → val`; returns the replaced value, if any. Serializes
    /// with other writers. Keys are NUL-free byte strings.
    pub fn insert(&self, key: &NulFreeStr, val: u64) -> Option<u64> {
        crate::occ_stats::bump(crate::occ_stats::Stat::Inserts);
        self.shared.write(|m| m.insert(key, val))
    }

    /// Removes `key`; returns its value, if present.
    pub fn remove(&self, key: &NulFreeStr) -> Option<u64> {
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
    pub fn get(&self, key: &NulFreeStr) -> Option<u64> {
        self.reader().get(key)
    }

    /// One-shot membership test (registers a throwaway reader; use
    /// [`Self::reader`] in hot loops).
    #[must_use]
    pub fn contains_key(&self, key: &NulFreeStr) -> bool {
        self.get(key).is_some()
    }

    /// Number of keys (validated read).
    #[must_use]
    pub fn len(&self) -> u64 {
        for _ in 0..MAX_RETRIES {
            let snap = self.shared.version().sample();
            // SAFETY: single-word racy copy; validated before use.
            let pop = unsafe { (*self.shared.inner.get()).len() };
            if self.shared.version().validate(snap) {
                return pop;
            }
        }
        // Mirrors `Shared::validated_len`: retry exhaustion here is a
        // fallback, not one of the unconditionally-locked routes, so it must
        // be counted as one or `locked_reads - read_fallbacks` misreports the
        // unconditional share.
        crate::occ_stats::bump(crate::occ_stats::Stat::ReadFallbacks);
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
    /// Optimistic lookup: a bounded, validated cascade across the sub-tries
    /// (one hop per 8 key bytes), falling back to the writer lock after
    /// bounded retries.
    #[must_use]
    pub fn get(&self, key: &NulFreeStr) -> Option<u64> {
        let shared = &self.map.shared;
        crate::occ_stats::bump(crate::occ_stats::Stat::ReadOps);
        for _ in 0..MAX_RETRIES {
            crate::occ_stats::bump(crate::occ_stats::Stat::ReadAttempts);
            let _pin = self.reader.pin();
            let snap = shared.version().sample();
            // SAFETY: pinned + freshly sampled version; every hop of the
            // cascade validates its loads (see `ExpanseStrMap::get_validated`).
            let attempt =
                // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                unsafe { (*shared.inner.get()).get_validated(key, shared.version(), snap) };
            if let Ok(r) = attempt {
                return r;
            }
        }
        crate::occ_stats::bump(crate::occ_stats::Stat::ReadFallbacks);
        shared.read_locked(|m| m.get(key))
    }

    /// Optimistic membership test.
    #[must_use]
    pub fn contains(&self, key: &NulFreeStr) -> bool {
        self.get(key).is_some()
    }
}

/// An **unordered** byte-string map shareable across threads (issue
/// #362 — the JudyHS member completing the Sync* family): one writer at
/// a time (internally serialized), validated optimistic readers. See the
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
    shared: Box<Shared<ExpanseBytesMap<S>>>,
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
        crate::occ_stats::bump(crate::occ_stats::Stat::Inserts);
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
            let snap = self.shared.version().sample();
            // SAFETY: single-word racy copy; validated before use.
            let pop = unsafe { (*self.shared.inner.get()).len() };
            if self.shared.version().validate(snap) {
                return pop;
            }
        }
        // As in `SyncExpanseStrMap::len`: a retry-exhaustion fallback, and
        // counted as one.
        crate::occ_stats::bump(crate::occ_stats::Stat::ReadFallbacks);
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
    /// Optimistic lookup: one bounded, validated hash-trie walk plus a
    /// byte-exact bucket comparison, falling back to the writer lock
    /// after bounded retries.
    #[must_use]
    pub fn get(&self, key: &[u8]) -> Option<u64> {
        let shared = &self.map.shared;
        crate::occ_stats::bump(crate::occ_stats::Stat::ReadOps);
        for _ in 0..MAX_RETRIES {
            crate::occ_stats::bump(crate::occ_stats::Stat::ReadAttempts);
            let _pin = self.reader.pin();
            let snap = shared.version().sample();
            // SAFETY: pinned + freshly sampled version; every load is
            // validated (see `ExpanseBytesMap::get_validated`).
            let attempt =
                // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
                unsafe { (*shared.inner.get()).get_validated(key, shared.version(), snap) };
            if let Ok(r) = attempt {
                return r;
            }
        }
        crate::occ_stats::bump(crate::occ_stats::Stat::ReadFallbacks);
        shared.read_locked(|m| m.get(key))
    }

    /// Optimistic membership test.
    #[must_use]
    pub fn contains(&self, key: &[u8]) -> bool {
        self.get(key).is_some()
    }
}

#[cfg(all(test, not(miri)))]
mod tests {
    /// A tree whose root state the engine covers, with no wrapper: defers
    /// to a fresh collector and binds `word` (the wrapper would own both;
    /// here the test declares the word before the tree so it outlives it).
    #[allow(dead_code)]
    pub(super) fn cover_root_for_test(alloc: &crate::alloc::NodeAlloc, word: &SeqVersion) {
        alloc.defer_to(Arc::new(crate::occ::Collector::new()));
        // SAFETY: the caller declared `word` before the tree, so it drops
        // after it.
        // SAFETY: node, edge, and version pointers are valid and EBR-live under the OLC protocol.
        unsafe { alloc.bind_tree_word(core::ptr::from_ref(word)) };
        alloc.cover_root();
    }

    /// Wraps a test key. These are literals and generated keys the tests know
    /// are in-domain; a NUL in one is a bug in the test, so panicking is right.
    fn tk<B: AsRef<[u8]> + ?Sized>(bytes: &B) -> &NulFreeStr {
        NulFreeStr::new(bytes.as_ref()).expect("test key contains a NUL")
    }

    /// A `DetachedMapReader` must give the same answers as the owned reader
    /// and the one-shot `get`, register exactly once, and — the property it
    /// exists for — hold no strong reference to the map.
    ///
    /// A per-thread cache keyed by map is the natural place to put a cached
    /// reader, and an `OwnedMapReader` in one keeps every map a thread has
    /// read from alive for the life of that thread: the map cannot drop
    /// because the cache entry exists, and the map's own drop cannot evict it.
    /// The strong-count assertion is what makes that structural rather than a
    /// convention, so it is checked here and not in the binding that depends
    /// on it.
    #[test]
    fn detached_reader_answers_correctly_without_retaining_the_map() {
        let map = Arc::new(SyncExpanseMap::new());
        for k in 0..500u64 {
            map.insert(k, k * 3);
        }

        let strong_before = Arc::strong_count(&map);
        let before = map.shared.collector.registered_readers();
        let reader = map.detached_reader();
        assert_eq!(
            map.shared.collector.registered_readers(),
            before + 1,
            "constructing a detached reader registers exactly one slot"
        );
        assert_eq!(
            Arc::strong_count(&map),
            strong_before,
            "a detached reader must not take a strong reference to its map — \
             that is the whole difference from OwnedMapReader, and a cache \
             holding one would pin the map for the life of the cache"
        );

        for k in 0..500u64 {
            assert_eq!(
                reader.get(&map, k),
                map.get(k),
                "detached reader disagrees at {k}"
            );
        }
        assert_eq!(reader.get(&map, 9_999), None, "absent key must miss");

        let regs_before_loop = map.shared.collector.registrations();
        for k in 0..500u64 {
            assert_eq!(reader.get(&map, k), Some(k * 3));
        }
        assert_eq!(
            map.shared.collector.registrations(),
            regs_before_loop,
            "lookups through a detached reader must not call register()"
        );

        // The map drops while the reader is still alive. An owned reader could
        // not be dropped after its map; a detached one has nothing to dangle.
        drop(map);
        drop(reader);
    }

    /// An `OwnedMapReader` must agree with the one-shot `get` it replaces, and
    /// must register exactly once however many lookups go through it — that
    /// second property is the point of #554, where per-call registration
    /// serialised concurrent Python readers to 0.02x at 16 threads.
    #[test]
    fn owned_reader_matches_one_shot_get_and_registers_once() {
        let map = Arc::new(SyncExpanseMap::new());
        for k in 0..500u64 {
            map.insert(k, k * 3);
        }

        let before = map.shared.collector.registered_readers();
        let reader = map.owned_reader();
        assert_eq!(
            map.shared.collector.registered_readers(),
            before + 1,
            "constructing an owned reader registers exactly one slot"
        );

        // Correctness first. `map.get` is the one-shot form and registers per
        // call by design, so the registration baseline is taken *after* this.
        for k in 0..500u64 {
            assert_eq!(reader.get(k), map.get(k), "owned reader disagrees at {k}");
        }
        assert_eq!(reader.get(9_999), None, "absent key must miss");

        let regs_before_loop = map.shared.collector.registrations();
        for k in 0..500u64 {
            assert_eq!(reader.get(k), Some(k * 3));
        }
        // Registry *size* cannot catch a regression here: a one-shot `get`
        // registers and deregisters inside the call, so the size is back to
        // where it started by the time this runs. Counting `register` calls is
        // what distinguishes a cached reader from a per-lookup one.
        assert_eq!(
            map.shared.collector.registrations(),
            regs_before_loop,
            "lookups through an owned reader must not call register() — that \
             per-call registration is the contention #554 removed"
        );

        drop(reader);
        assert_eq!(
            map.shared.collector.registered_readers(),
            before,
            "dropping the owned reader deregisters its slot"
        );
    }
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

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

    /// `Shared::write` batches epoch-advance attempts one per
    /// [`ADVANCE_EVERY`] mutations — it must batch them (that is the
    /// point: `try_advance` runs inside the writer critical section and
    /// roughly doubled its length) and it must not *stop* making them
    /// (that would defer reclamation without bound).
    ///
    /// With no readers registered, every attempt succeeds, so the epoch
    /// delta counts the attempts exactly.
    // Loops on the advance interval; there is no finite interval under `advance-never`.
    #[cfg(not(feature = "advance-never"))]
    #[test]
    fn write_batches_epoch_advances_without_dropping_them() {
        let collector = Arc::new(Collector::new());
        let shared = Shared::with_collector(ExpanseMap::new(), Arc::clone(&collector));
        let start = collector.epoch_now();

        let writes = ADVANCE_EVERY * 4;
        for i in 1..=writes {
            shared.write(|m| m.insert(i, !i));
        }

        let advances = (collector.epoch_now() - start) as u64;
        assert_eq!(
            advances, 4,
            "expected one advance per {ADVANCE_EVERY} writes over {writes} writes"
        );
        assert!(
            advances < writes,
            "advances must be batched, not one per write"
        );
        // The batching is a reclamation-latency change only: the tree is
        // still exactly what the mutations said it is.
        assert_eq!(shared.read_locked(ExpanseMap::len), writes);
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
        // Published so the writer can wait for a reader to observe a stable
        // map instead of racing one; see the bounded wait below.
        let observed = Arc::new(AtomicU64::new(0));
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
                let observed = Arc::clone(&observed);
                std::thread::spawn(move || {
                    let rd = m.reader();
                    let mut rng = XorShift(0x1000 + i);
                    let mut hits = 0u64;
                    while !stop.load(Ordering::Relaxed) {
                        let k = key_of(&mut rng);
                        if let Some(v) = rd.get(k) {
                            assert_eq!(v, val_of(k), "wrong-slot value for {k:#x}");
                            hits += 1;
                            observed.fetch_add(1, Ordering::Relaxed);
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
                if rng.next().is_multiple_of(2) {
                    m.insert(k, val_of(k));
                    model.insert(k, val_of(k));
                } else {
                    m.remove(k);
                    model.remove(&k);
                }
            }
        }
        // Wait for a reader to observe the map, rather than asserting that one
        // happened to during the churn. The map is left populated by the loop
        // above, so a reader that cannot see it here has a real problem -- an
        // unhandled tag decoding to `None` is exactly the §2.3 hazard this test
        // exists for. Racing for it made the assertion a scheduling outcome:
        // it fired on a loaded Windows runner with nothing wrong (§8.4 -- a
        // hard assertion belongs on a deterministic invariant, and "a reader
        // got scheduled inside a populated window" is not one).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while observed.load(Ordering::Relaxed) == 0 && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        stop.store(true, Ordering::Relaxed);
        let mut total_hits = 0;
        for r in readers {
            total_hits += r.join().expect("reader panicked");
        }
        assert!(
            total_hits > 0,
            "no reader observed the map within 10s while it was populated"
        );

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
                if rng.next().is_multiple_of(2) {
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
        if idx.is_multiple_of(8) {
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
                    assert_eq!(m.insert(tk(&k), v), model.insert(k.clone(), v), "ins {k:?}");
                }
                1 => assert_eq!(m.remove(tk(&k)), model.remove(&k), "rm {k:?}"),
                _ => assert_eq!(m.get(tk(&k)), model.get(&k).copied(), "get {k:?}"),
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
                cursor = inner.next_after(tk(&k));
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
        // Published so the writer can wait for a reader to observe a stable
        // map instead of racing one; see the bounded wait below.
        let observed = Arc::new(AtomicU64::new(0));

        let readers: Vec<_> = (0..3)
            .map(|i| {
                let m = Arc::clone(&m);
                let stop = Arc::clone(&stop);
                let observed = Arc::clone(&observed);
                std::thread::spawn(move || {
                    let rd = m.reader();
                    let mut rng = XorShift(0x5000 + i);
                    let mut hits = 0u64;
                    while !stop.load(Ordering::Relaxed) {
                        let k = str_key_of(rng.next() % 512);
                        if let Some(v) = rd.get(tk(&k)) {
                            assert_eq!(v, str_val_of(&k), "torn value for {k:?}");
                            hits += 1;
                            observed.fetch_add(1, Ordering::Relaxed);
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
                if rng.next().is_multiple_of(2) {
                    m.insert(tk(&k), str_val_of(&k));
                    model.insert(k, ());
                } else {
                    m.remove(tk(&k));
                    model.remove(&k);
                }
            }
            // Tear the whole tree down under the readers, then rebuild
            // some of it — the dispose_tree/EBR path under fire.
            m.clear();
            model.clear();
            for idx in 0..64 {
                let k = str_key_of(idx);
                m.insert(tk(&k), str_val_of(&k));
                model.insert(k, ());
            }
        }
        // Wait for a reader to observe the map, rather than asserting that one
        // happened to during the churn. The map is left populated by the loop
        // above, so a reader that cannot see it here has a real problem -- an
        // unhandled tag decoding to `None` is exactly the §2.3 hazard this test
        // exists for. Racing for it made the assertion a scheduling outcome:
        // it fired on a loaded Windows runner with nothing wrong (§8.4 -- a
        // hard assertion belongs on a deterministic invariant, and "a reader
        // got scheduled inside a populated window" is not one).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while observed.load(Ordering::Relaxed) == 0 && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        stop.store(true, Ordering::Relaxed);
        let mut total_hits = 0;
        for r in readers {
            total_hits += r.join().expect("reader panicked");
        }
        assert!(
            total_hits > 0,
            "no reader observed the map within 10s while it was populated"
        );

        let rd = m.reader();
        for k in model.keys() {
            assert_eq!(rd.get(tk(k)), Some(str_val_of(k)), "model key {k:?}");
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
        m.insert(tk(&key), a);
        let stop = Arc::new(AtomicBool::new(false));
        let readers: Vec<_> = (0..3)
            .map(|_| {
                let m = Arc::clone(&m);
                let stop = Arc::clone(&stop);
                let key = key.clone();
                std::thread::spawn(move || {
                    let rd = m.reader();
                    while !stop.load(Ordering::Relaxed) {
                        let v = rd.get(tk(&key)).expect("key always present");
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
                m.insert(tk(&key), if flip { b } else { a });
            }
            sibling_in = !sibling_in;
            if sibling_in {
                m.insert(tk(&sibling), 7);
            } else {
                m.remove(tk(&sibling));
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
                assert_eq!(m.insert(tk(&key), 1), None);
                assert_eq!(m.insert(tk(&sibling), 2), None);
                assert_eq!(m.get(tk(&key)), Some(1));
                assert_eq!(m.get(tk(&sibling)), Some(2));
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
            plain.insert(tk(&k), str_val_of(&k));
        }
        let expected = plain.len();
        let m = SyncExpanseStrMap::from(plain);
        assert_eq!(m.len(), expected);
        let rd = m.reader();
        for idx in 0..300u64 {
            let k = str_key_of(idx);
            assert_eq!(rd.get(tk(&k)), Some(str_val_of(&k)), "wrapped key {k:?}");
        }
        for idx in 0..150u64 {
            m.remove(tk(&str_key_of(idx)));
        }
        m.insert(tk(b"post-wrap"), 7);
        assert_eq!(m.get(tk(b"post-wrap")), Some(7));
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
        // Published so the writer can wait for a reader to observe a stable
        // map instead of racing one; see the bounded wait below.
        let observed = Arc::new(AtomicU64::new(0));
        let key_of = |r: &mut XorShift| {
            let base = [0u64, 0x11_2233_4400, 0xFFFF_FF00_0000][(r.next() % 3) as usize];
            base + r.next() % 512
        };

        let readers: Vec<_> = (0..3)
            .map(|i| {
                let m = Arc::clone(&m);
                let stop = Arc::clone(&stop);
                let observed = Arc::clone(&observed);
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
                            observed.fetch_add(1, Ordering::Relaxed);
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
                if rng.next().is_multiple_of(2) {
                    m.insert(k, &blob_payload_of(k), blob_meta_of(k)).unwrap();
                    model.insert(k, ());
                } else {
                    m.remove(k);
                    model.remove(&k);
                }
            }
            m.compact().expect("compaction under churn");
        }
        // Wait for a reader to observe the map, rather than asserting that one
        // happened to during the churn. The map is left populated by the loop
        // above, so a reader that cannot see it here has a real problem -- an
        // unhandled tag decoding to `None` is exactly the §2.3 hazard this test
        // exists for. Racing for it made the assertion a scheduling outcome:
        // it fired on a loaded Windows runner with nothing wrong (§8.4 -- a
        // hard assertion belongs on a deterministic invariant, and "a reader
        // got scheduled inside a populated window" is not one).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while observed.load(Ordering::Relaxed) == 0 && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        stop.store(true, Ordering::Relaxed);
        let mut total_hits = 0;
        for r in readers {
            total_hits += r.join().expect("reader panicked");
        }
        assert!(
            total_hits > 0,
            "no reader observed the map within 10s while it was populated"
        );

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

    /// The issue #362 gate: readers hammer optimistic point lookups while
    /// the writer churns inserts/removes and periodically clears the
    /// whole map (retiring every bucket under active readers). Every
    /// observed value must be the key's full-key FNV-1a hash — a
    /// misresolved bucket, torn word, or stale key comparison cannot
    /// accidentally pass.
    #[test]
    fn concurrent_bytes_readers_under_churn() {
        let m = Arc::new(SyncExpanseBytesMap::new());
        let stop = Arc::new(AtomicBool::new(false));
        // Published so the writer can wait for a reader to observe a stable
        // map instead of racing one; see the bounded wait below.
        let observed = Arc::new(AtomicU64::new(0));

        let readers: Vec<_> = (0..3)
            .map(|i| {
                let m = Arc::clone(&m);
                let stop = Arc::clone(&stop);
                let observed = Arc::clone(&observed);
                std::thread::spawn(move || {
                    let rd = m.reader();
                    let mut rng = XorShift(0x7000 + i);
                    let mut hits = 0u64;
                    while !stop.load(Ordering::Relaxed) {
                        let k = str_key_of(rng.next() % 512);
                        if let Some(v) = rd.get(&k) {
                            assert_eq!(v, str_val_of(&k), "torn value for {k:?}");
                            hits += 1;
                            observed.fetch_add(1, Ordering::Relaxed);
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
                if rng.next().is_multiple_of(2) {
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
        // Wait for a reader to observe the map, rather than asserting that one
        // happened to during the churn. The map is left populated by the loop
        // above, so a reader that cannot see it here has a real problem -- an
        // unhandled tag decoding to `None` is exactly the §2.3 hazard this test
        // exists for. Racing for it made the assertion a scheduling outcome:
        // it fired on a loaded Windows runner with nothing wrong (§8.4 -- a
        // hard assertion belongs on a deterministic invariant, and "a reader
        // got scheduled inside a populated window" is not one).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while observed.load(Ordering::Relaxed) == 0 && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        stop.store(true, Ordering::Relaxed);
        let mut total_hits = 0;
        for r in readers {
            total_hits += r.join().expect("reader panicked");
        }
        assert!(
            total_hits > 0,
            "no reader observed the map within 10s while it was populated"
        );

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

    /// The insert-path bypass is compiled out on a shared tree (#477, #479,
    /// #568 PR 3 — AGENTS.md §2.1.5, a compile-time path rather than a
    /// runtime check): a warm path cache matching the next key's prefix is
    /// ignored, the engine brackets its own stores, and the cache is left
    /// cold. With no bracket open around the call, the old bypass would
    /// have stored into the leaf unbracketed.
    #[test]
    #[cfg(debug_assertions)]
    fn shared_map_insert_leaves_the_path_cache_cold() {
        let word = SeqVersion::new();
        let mut map = ExpanseMap::new();
        // The map/set wrapper's mode: the engine covers the root state, so
        // no bracket is open around any call here.
        cover_root_for_test(map.occ_root().1, &word);
        for k in 0..40u64 {
            map.insert(k, k * 10);
        }
        assert_eq!(map.insert(50, 500), None);
        assert_eq!(map.get(50), Some(500));
        // Sequential keys are the bypass's own case; on a shared tree the
        // engine leaves the cache cold, so the bypass can never fire.
        let path = map.path_mut();
        assert_eq!(path.depth, 0, "the shared engine records no path");
        assert!(
            path.leaf.is_null() && path.leaf1.is_null(),
            "no terminal cursor"
        );
        assert_eq!(path.prefix, u64::MAX, "no warm prefix");
        assert!(crate::alloc::bracket_stack::open().is_empty());
    }

    /// Set twin of the test above.
    #[test]
    #[cfg(debug_assertions)]
    fn shared_set_insert_leaves_the_path_cache_cold() {
        let word = SeqVersion::new();
        let mut set = ExpanseSet::new();
        // The map/set wrapper's mode: the engine covers the root state, so
        // no bracket is open around any call here.
        cover_root_for_test(set.occ_root().1, &word);
        for k in 0..40u64 {
            set.insert(k);
        }
        assert!(set.insert(50));
        assert!(set.contains(50));
        let path = set.path_mut();
        assert_eq!(path.depth, 0, "the shared engine records no path");
        assert!(
            path.leaf.is_null() && path.leaf1.is_null(),
            "no terminal cursor"
        );
        assert_eq!(path.prefix, u64::MAX, "no warm prefix");
        assert!(crate::alloc::bracket_stack::open().is_empty());
    }

    /// Positive companion: when properly bracketed, map mutations covered
    /// by an active bracket succeed quietly.
    #[test]
    #[cfg(debug_assertions)]
    fn bracketed_map_insert_is_quiet_when_covered() {
        let mut map = ExpanseMap::new();
        map.occ_root()
            .1
            .defer_to(Arc::new(crate::occ::Collector::new()));

        map.occ_root().1.bracket_enter_any();
        for k in 0..40u64 {
            map.insert(k, k * 10);
        }
        map.path_mut().prefix = 0;
        map.insert(50, 500);
        map.occ_root().1.bracket_leave_any();
    }

    /// Positive companion: set twin of the test above.
    #[test]
    #[cfg(debug_assertions)]
    fn bracketed_set_insert_is_quiet_when_covered() {
        let mut set = ExpanseSet::new();
        set.occ_root()
            .1
            .defer_to(Arc::new(crate::occ::Collector::new()));

        set.occ_root().1.bracket_enter_any();
        for k in 0..40u64 {
            set.insert(k);
        }
        set.path_mut().prefix = 0;
        set.insert(50);
        set.occ_root().1.bracket_leave_any();
    }

    /// A long-held `BlobReadGuard` holds an epoch `Pin` that stalls `Collector::try_advance`
    /// for every concurrent writer, causing retired nodes and chunk memory to accumulate
    /// as retained garbage.
    ///
    /// Discriminates the stall by comparing against an identical unheld arm:
    /// 1. Both maps receive identical initial populations and identical overwrite write churn.
    /// 2. Under a held `BlobReadGuard`, epoch advances are refused and retained backlog grows.
    /// 3. Without a held guard, epoch advances succeed every `ADVANCE_EVERY` writes, keeping backlog bounded.
    /// 4. `retained_held > retained_unheld` strictly holds.
    /// 5. Once the guard is dropped and advances resume, `retained_held` drains back to the baseline.
    // Assumes the default epoch-advance interval: with `advance-every-4096` or
    // `advance-never` no advance happens inside this test, so held and unheld
    // retain the same garbage -- which is what those variants exist to show.
    #[cfg(not(any(feature = "advance-every-4096", feature = "advance-never")))]
    #[test]
    fn blob_read_guard_stalls_reclamation_and_tracks_retained_garbage() {
        crate::occ_stats::reset();
        let payload = vec![0xABu8; 128];
        let new_payload = vec![0xCDu8; 256];

        // 1. Arm with long-held BlobReadGuard
        let map_held = SyncExpanseBlobMap::with_chunk_size(4096);
        for k in 0..100u64 {
            map_held.insert(k, &payload, 1).unwrap();
        }
        let mut rd_held = map_held.reader();
        let guard = rd_held.pin();
        let (view, meta) = guard.get(10).expect("key present");
        assert_eq!(view.as_bytes(), &payload[..]);
        assert_eq!(meta, 1);

        for k in 0..200u64 {
            map_held.insert(k, &new_payload, 2).unwrap();
        }
        let retained_held = map_held.shared.collector.retained_bytes();

        // 2. Twin comparison baseline: identical population and churn, but NO held guard
        let map_unheld = SyncExpanseBlobMap::with_chunk_size(4096);
        for k in 0..100u64 {
            map_unheld.insert(k, &payload, 1).unwrap();
        }
        for k in 0..200u64 {
            map_unheld.insert(k, &new_payload, 2).unwrap();
        }
        let retained_unheld = map_unheld.shared.collector.retained_bytes();

        // Discriminates the stall: held guard must retain strictly more than unheld
        assert!(
            retained_held > retained_unheld,
            "held guard must retain strictly more garbage than unheld: held {retained_held}, unheld {retained_unheld}"
        );

        // 3. Drop the guard: advances on map_held can now drain the backlog
        drop(guard);
        for k in 200..300u64 {
            map_held.insert(k, &payload, 3).unwrap();
        }
        let retained_after_drop = map_held.shared.collector.retained_bytes();
        assert!(
            retained_after_drop < retained_held,
            "retained garbage must drop after releasing guard: before {retained_held}, after {retained_after_drop}"
        );
    }

    #[test]
    fn multi_writer_orphan_reader_pruning() {
        let map = Box::new(SyncExpanseMap::new());
        let col = Arc::clone(&map.shared.collector);
        let reader = col.register();
        assert!(!reader.is_orphan());

        // Multiple reader handles simulating multiple worker thread TLS caches.
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let r = col.register();
                std::thread::spawn(move || {
                    {
                        let _pin = r.pin();
                    }
                    r
                })
            })
            .collect();
        let readers: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // Strong count is > 1 because multiple threads held handles.
        assert!(!reader.is_orphan());
        for r in &readers {
            assert!(!r.is_orphan());
        }

        // Drop the owning tree:
        drop(map);

        // Under N1 (liveness flag), dropping the tree immediately marks all readers as orphan,
        // even though multiple thread-local handles still hold Arc<Collector> (strong_count > 1).
        assert!(reader.is_orphan());
        for r in &readers {
            assert!(r.is_orphan());
        }
    }
}

/// A small integer naming the calling thread, for the `Handoffs` counter.
/// Allocated once per thread from a global counter; 0 is never issued.
#[cfg(feature = "occ-stats")]
fn thread_token() -> u64 {
    use core::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    std::thread_local! {
        static TOKEN: u64 = NEXT.fetch_add(1, Ordering::Relaxed);
    }
    TOKEN.with(|t| *t)
}

/// Byte offsets of every `Shared` field, for a benchmark that wants to know
/// which of them share a cache line before attributing a cost to line
/// sharing. One row per (wrapper, field): the wrapper's type name, the field
/// name, and the field's byte offset within `Shared`; a final `size_of` row
/// per wrapper carries the struct's size. `Shared` is `repr(Rust)`, so the
/// order is whatever the compiler chose, and it is chosen per instantiation:
/// the offsets are reported for each concrete wrapped type rather than for a
/// placeholder, because a `Shared<()>` need not be laid out like a
/// `Shared<ExpanseMap>`. This is the only way to know the layout.
#[cfg(feature = "occ-stats")]
#[must_use]
pub fn layout_report() -> Vec<(&'static str, &'static str, usize)> {
    fn rows<T>(wrapper: &'static str, out: &mut Vec<(&'static str, &'static str, usize)>) {
        out.extend([
            (wrapper, "inner", core::mem::offset_of!(Shared<T>, inner)),
            (
                wrapper,
                "version",
                core::mem::offset_of!(Shared<T>, version),
            ),
            (
                wrapper,
                "tree_pop",
                core::mem::offset_of!(Shared<T>, tree_pop),
            ),
            (wrapper, "write", core::mem::offset_of!(Shared<T>, write)),
            (
                wrapper,
                "collector",
                core::mem::offset_of!(Shared<T>, collector),
            ),
            (wrapper, "gate", core::mem::offset_of!(Shared<T>, gate)),
            (
                wrapper,
                "fallback_mutex",
                core::mem::offset_of!(Shared<T>, fallback_mutex),
            ),
            (
                wrapper,
                "writers",
                core::mem::offset_of!(Shared<T>, writers),
            ),
            (
                wrapper,
                "last_holder",
                core::mem::offset_of!(Shared<T>, last_holder),
            ),
            (
                wrapper,
                "advance_tick",
                core::mem::offset_of!(Shared<T>, advance_tick),
            ),
            (wrapper, "size_of", core::mem::size_of::<Shared<T>>()),
        ]);
    }
    let mut out = Vec::with_capacity(5 * LAYOUT_ROWS);
    rows::<ExpanseSet>("SyncExpanseSet", &mut out);
    rows::<ExpanseMap>("SyncExpanseMap", &mut out);
    rows::<ExpanseBlobMap>("SyncExpanseBlobMap", &mut out);
    rows::<ExpanseStrMap>("SyncExpanseStrMap", &mut out);
    rows::<ExpanseBytesMap>("SyncExpanseBytesMap", &mut out);
    out
}

/// Rows per wrapper in [`layout_report`]: the ten fields and the size row.
#[cfg(feature = "occ-stats")]
const LAYOUT_ROWS: usize = 11;

#[cfg(all(test, feature = "occ-stats"))]
mod diagnostics_tests {
    use super::*;
    use crate::occ_stats::{Stat, snapshot};

    #[test]
    fn handoffs_count_holder_changes_not_acquisitions() {
        // Strict alternation: A writes, hands a baton to B, B writes, hands it
        // back. Every acquisition changes the holder, so the counter must rise
        // by at least ROUNDS regardless of how the scheduler interleaves the
        // two threads. (The counters are process-global and other tests write
        // concurrently, so only lower bounds are sound here; an "exactly zero
        // for one thread" assertion is not.)
        const ROUNDS: u64 = 200;
        let m = std::sync::Arc::new(SyncExpanseMap::new());
        let (to_b, from_a) = std::sync::mpsc::channel::<u64>();
        let (to_a, from_b) = std::sync::mpsc::channel::<u64>();
        let before = snapshot()[Stat::Handoffs as usize];
        let mb = std::sync::Arc::clone(&m);
        let b = std::thread::spawn(move || {
            for k in from_a {
                mb.insert(1_000_000 + k, k);
                if to_a.send(k).is_err() {
                    break;
                }
            }
        });
        for k in 0..ROUNDS {
            m.insert(k, k);
            to_b.send(k).unwrap();
            from_b.recv().unwrap();
        }
        drop(to_b);
        b.join().unwrap();
        let handoffs = snapshot()[Stat::Handoffs as usize] - before;
        assert!(
            handoffs >= ROUNDS,
            "strict alternation over {ROUNDS} rounds must hand the lock over at least {ROUNDS} times; counted {handoffs}"
        );
    }

    #[test]
    fn retire_and_free_are_counted_on_remove() {
        let m = SyncExpanseMap::new();
        for k in 0..50_000u64 {
            m.insert(k.wrapping_mul(0x9E37_79B9_7F4A_7C15), k);
        }
        let r0 = snapshot()[Stat::Retired as usize];
        for k in 0..50_000u64 {
            m.remove(k.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        }
        assert!(
            snapshot()[Stat::Retired as usize] > r0,
            "removes must retire blocks"
        );
        let f0 = snapshot()[Stat::FreedRaw as usize];
        drop(m);
        assert!(
            snapshot()[Stat::FreedRaw as usize] > f0,
            "dropping the map frees retired blocks"
        );
    }

    #[test]
    fn layout_report_names_every_field_of_every_wrapper() {
        const FIELDS: [&str; 10] = [
            "inner",
            "version",
            "tree_pop",
            "write",
            "collector",
            "gate",
            "fallback_mutex",
            "writers",
            "last_holder",
            "advance_tick",
        ];
        const WRAPPERS: [&str; 5] = [
            "SyncExpanseSet",
            "SyncExpanseMap",
            "SyncExpanseBlobMap",
            "SyncExpanseStrMap",
            "SyncExpanseBytesMap",
        ];
        let r = layout_report();
        assert_eq!(r.len(), WRAPPERS.len() * LAYOUT_ROWS);
        for (i, wrapper) in WRAPPERS.iter().enumerate() {
            let rows = &r[i * LAYOUT_ROWS..(i + 1) * LAYOUT_ROWS];
            assert!(
                rows.iter().all(|(w, _, _)| w == wrapper),
                "{wrapper}: rows grouped"
            );
            let names: Vec<&str> = rows.iter().map(|(_, n, _)| *n).collect();
            assert_eq!(&names[..FIELDS.len()], &FIELDS, "{wrapper}: every field");
            assert_eq!(names[FIELDS.len()], "size_of");
            let size = rows[FIELDS.len()].2;
            let offs: std::collections::BTreeSet<usize> =
                rows[..FIELDS.len()].iter().map(|(_, _, o)| *o).collect();
            assert_eq!(offs.len(), FIELDS.len(), "{wrapper}: distinct offsets");
            assert!(
                offs.iter().all(|&o| o < size),
                "{wrapper}: every offset inside the struct"
            );
            let off = |name: &str| rows.iter().find(|(_, n, _)| *n == name).unwrap().2;
            // The readers' line: the word heads the block, the root
            // snapshot follows it; the writer's private words are on
            // another line (#568 PR 3).
            assert_eq!(
                off("version"),
                0,
                "{wrapper}: the tree word heads the block"
            );
            assert_eq!(
                off("inner"),
                core::mem::size_of::<Line<SeqVersion>>(),
                "{wrapper}: the root snapshot follows the word"
            );
            for w in ["write", "last_holder", "advance_tick"] {
                assert!(
                    off(w) / 64 != off("version") / 64,
                    "{wrapper}: {w} shares no line with the tree word"
                );
            }
            #[cfg(feature = "lock-padded")]
            {
                assert_eq!(off("tree_pop") % 64, 0, "{wrapper}: tree_pop line-aligned");
                assert_eq!(off("write") % 64, 0, "{wrapper}: write line-aligned");
            }
        }
    }
}

/// Test-only pause points inside the validated walk, so a test can hold a
/// reader at a precise step while the writer moves the structure under it.
/// Armed per thread: only the thread that armed itself parks, so tests in
/// the same binary never catch each other's readers.
#[cfg(test)]
pub(crate) mod test_hooks {
    use std::cell::Cell;
    use std::sync::{Arc, Barrier};

    /// Two rendezvous: `parked` fires when the reader has reached the hook,
    /// `release` lets it continue.
    pub(crate) struct Gate {
        pub(crate) parked: Barrier,
        pub(crate) release: Barrier,
    }

    impl Gate {
        pub(crate) fn new() -> Arc<Self> {
            Arc::new(Self {
                parked: Barrier::new(2),
                release: Barrier::new(2),
            })
        }
    }

    thread_local! {
        static ARMED: Cell<Option<Arc<Gate>>> = const { Cell::new(None) };
    }

    /// The next time *this thread's* walk reaches a linear-leaf value load,
    /// it stops at `gate` (once).
    pub(crate) fn arm_current_thread(gate: Arc<Gate>) {
        ARMED.with(|c| c.set(Some(gate)));
    }

    /// Between finding a key in a linear map leaf and loading its value.
    #[inline(always)]
    pub(crate) fn before_leaf_value() {
        if let Some(g) = ARMED.with(Cell::take) {
            g.parked.wait();
            g.release.wait();
        }
    }
}

/// A reader whose cover node is replaced must restart, not trust the dead
/// node's frozen version (#568 plan PR 1).
#[cfg(all(test, not(miri)))]
mod obsolete_tests {
    use super::*;
    use crate::node::BranchL3;
    use crate::types::{EdgeTag, EdgeType};

    /// The reader is held between finding its key in the leaf and loading
    /// the value; the writer then promotes the reader's cover (an L3 at
    /// capacity, retired by the promotion) and shifts that leaf in place
    /// under the replacement. Without the obsolete mark the reader's
    /// validation passes and it returns the value that shifted into its
    /// slot; with it, the reader restarts and answers correctly.
    #[test]
    fn reader_restarts_after_its_cover_is_replaced() {
        let map = SyncExpanseMap::new();
        // Seventeen 1-byte keys under each of three level-2 digits: a full
        // BranchL3 at level 2 whose children are linear leaves of 17 keys
        // (class 20, so an 18th shifts in place). 51 keys > ROOT_LEAF_CAP.
        for d in [0x10u64, 0x20, 0x30] {
            for i in 0..17u64 {
                let k = (d << 8) | (1 + 2 * i);
                map.insert(k, !k);
            }
        }
        // Preconditions (AGENTS.md §5 negative-control discrimination): the
        // shape must actually be the one the interleaving needs.
        let probe = 0x1021u64;
        // SAFETY: no writer is running; the snapshot is read for its shape.
        let RootSnapshot::Tree { top } = (unsafe { (*map.shared.inner.get()).occ_root().0 }) else {
            panic!("root must be a tree")
        };
        // The keys share bytes 7..2, so the path from the top is a chain of
        // single-digit linear branches down to the full level-2 L3; walk it.
        let mut edge = top;
        // SAFETY: live nodes, single-threaded here.
        let c = unsafe {
            loop {
                assert_eq!(
                    edge.tag(),
                    Some(EdgeTag::Structural(EdgeType::BranchL3)),
                    "every node on the path must be an L3"
                );
                let b = &*edge.node_ptr().cast::<BranchL3>();
                if b.hdr.level == 2 {
                    break b;
                }
                assert_eq!(b.hdr.num, 1, "chain node above level 2 has one digit");
                edge = b.edges[0];
            }
        };
        let (level, num, leaf_tag, leaf_pop) = {
            let slot = c.hdr.digits[..c.hdr.num as usize]
                .iter()
                .position(|&d| d == 0x10)
                .expect("digit 0x10 present");
            let e = c.edges[slot];
            (c.hdr.level, c.hdr.num, e.tag(), e.pop0(1) + 1)
        };
        assert_eq!((level, num), (2, 3), "L3 at level 2, full");
        assert_eq!(
            leaf_tag,
            Some(EdgeTag::Structural(EdgeType::Leaf1)),
            "linear leaf under 0x10"
        );
        assert_eq!(leaf_pop, 17);

        let gate = test_hooks::Gate::new();
        let value = std::thread::scope(|sc| {
            let g = Arc::clone(&gate);
            let map = &map;
            let reader = sc.spawn(move || {
                test_hooks::arm_current_thread(g);
                let rd = map.reader();
                rd.get(probe)
            });
            gate.parked.wait();
            // Op 1: a fourth digit promotes the L3 to an L7 and retires it —
            // the reader's cover is now a dead node.
            map.insert(0x4001, !0x4001);
            // Op 2: the smallest key into the 0x10 leaf, shifting keys and
            // values right by one in place, under the replacement's bracket.
            map.insert(0x1000, !0x1000);
            gate.release.wait();
            reader.join().expect("reader thread")
        });
        assert_eq!(
            value,
            Some(!probe),
            "the reader must restart, not read the shifted slot"
        );
    }

    #[test]
    #[cfg(feature = "occ-stats")]
    fn olc_stats_tracked() {
        crate::occ_stats::reset();
        let set = SyncExpanseSet::new();
        // Fallback bumped when inserting into empty/immediate root.
        set.insert(42);
        let snap0 = crate::occ_stats::snapshot();
        assert!(snap0[crate::occ_stats::Stat::LockFallbacks as usize] >= 1);

        // Prefill map so root becomes a tree with leaves.
        let map = SyncExpanseMap::new();
        for k in 0..500u64 {
            map.insert(k * 100, k);
        }
        let before_hold =
            crate::occ_stats::snapshot()[crate::occ_stats::Stat::LockHoldCycles as usize];
        // Updating existing keys in-place under OLC locks the leaf/parent.
        for k in 0..500u64 {
            map.insert(k * 100, k + 1);
        }
        let after_hold =
            crate::occ_stats::snapshot()[crate::occ_stats::Stat::LockHoldCycles as usize];
        assert!(
            after_hold > before_hold,
            "LockHoldCycles must advance under OLC node locks (before {before_hold}, after {after_hold})"
        );
    }
}
