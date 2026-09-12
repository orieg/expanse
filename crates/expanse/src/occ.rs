//! Phase 7: optimistic concurrency primitives — the seqlock version word
//! readers validate against, and epoch-based reclamation (EBR) so a
//! reader never dereferences a freed node.
//!
//! The concurrent wrappers (`SyncExpanseSet`/`SyncExpanseMap` in `sync`)
//! combine a **tree-level** [`SeqVersion`] (in the [`Collector`]; readers
//! validate their root snapshot against it) with **per-node** versions in
//! the branch headers. The mutation engine brackets every store by the
//! version of the node that *contains* the stored address — the parent's
//! word for a slot, an immediate, or a leaf / subarray payload; the tree
//! word for the root state — through [`Cover`] (active only for
//! concurrently shared trees). Where the engine covers the root, a branch
//! child's frame is entered with its parent's word closed, so a word is
//! odd only while one frame stores into that node, never for a whole
//! descent; where the wrapper holds the tree word for the whole operation
//! (the string, bytes and blob wrappers) the engine's nested mode keeps a
//! node's word odd across the descent beneath it, the protocol those
//! readers are built for (`Cover::nest_begin`). Readers validate hand-over-hand
//! with `node_sample`/`node_validate`. Measured motivation and effect in
//! `docs/BENCHMARKING.md` (concurrent read scaling) and
//! `docs/benchmarks/concurrency/`.
//!
//! Under `--cfg loom` the atomics and sync types swap to loom's, and the
//! `loom_` tests model-check writer/reader/reclamation interleavings.

#[cfg(all(not(loom), feature = "std"))]
use core::sync::atomic::{AtomicBool, AtomicUsize};
#[cfg(not(loom))]
use core::sync::atomic::{AtomicU64, Ordering, fence};
#[cfg(all(not(loom), feature = "std"))]
use std::sync::Mutex;

#[cfg(loom)]
use loom::sync::Mutex;
#[cfg(loom)]
use loom::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering, fence};

#[cfg(feature = "std")]
use core::alloc::Layout;
#[cfg(feature = "std")]
use core::ptr::NonNull;
#[cfg(feature = "std")]
use core_alloc::alloc::dealloc;
#[cfg(feature = "std")]
use core_alloc::sync::Arc;
#[cfg(feature = "std")]
use core_alloc::vec::Vec;

/// A seqlock word: even = stable, odd = mutation in progress.
///
/// One writer at a time (the wrappers enforce this with a mutex); any
/// number of readers.
#[derive(Debug, Default)]
pub struct SeqVersion(AtomicU64);

impl SeqVersion {
    /// A fresh, even (stable) version.
    #[must_use]
    pub fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    /// Writer: marks a mutation in progress (even → odd). The release
    /// fence orders this store before every data write the bracket
    /// covers — a reader that observes any covered write also observes
    /// the odd version (Boehm, "Can seqlocks get along with programming
    /// language memory models?", MSPC 2012; loom found the interleaving
    /// a plain release store here misses).
    pub fn begin(&self) {
        let v = self.0.load(Ordering::Relaxed);
        debug_assert!(v.is_multiple_of(2), "nested or unpaired begin");
        self.0.store(v + 1, Ordering::Relaxed);
        fence(Ordering::Release);
    }

    /// Writer: marks the mutation complete (odd → even), publishing all
    /// writes made since [`Self::begin`].
    pub fn end(&self) {
        let v = self.0.load(Ordering::Relaxed);
        debug_assert!(v % 2 == 1, "end without begin");
        self.0.store(v + 1, Ordering::Release);
    }

    /// Attempts to acquire an exclusive write lock on the tree version (even → odd).
    ///
    /// Returns `Ok(old_even_version)` on success, or `Err(current_version)` if locked or changed.
    ///
    /// The CAS is strong: this is a single attempt, not a retry loop, so a
    /// spurious weak-CAS failure would report an uncontended word as held.
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn try_lock(&self) -> Result<u64, u64> {
        let cur = self.0.load(Ordering::Relaxed);
        if !cur.is_multiple_of(2) {
            return Err(cur);
        }
        self.0
            .compare_exchange(cur, cur + 1, Ordering::Acquire, Ordering::Relaxed)
    }

    /// Releases the exclusive write lock on the tree version.
    ///
    /// If `modified` is true, advances to `old_v + 2` (`Release`).
    /// If `modified` is false, restores `old_v` so readers do not needlessly retry.
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn unlock(&self, old_v: u64, modified: bool) {
        let cur = self.0.load(Ordering::Relaxed);
        debug_assert!(cur % 2 == 1, "unlock without lock");
        let next = if modified {
            old_v.wrapping_add(2)
        } else {
            old_v
        };
        self.0.store(next, Ordering::Release);
    }

    /// Reader: samples the version, spinning past in-progress (odd)
    /// states. Pair with [`Self::validate`].
    #[must_use]
    pub fn sample(&self) -> u64 {
        // Diagnostic build only: the tick at the first odd observation, so
        // the wait is charged as a duration (`SampleSpinCycles`) and not
        // just as a spin count.
        #[cfg(feature = "occ-stats")]
        let mut wait_from: u64 = 0;
        loop {
            let v = self.0.load(Ordering::Acquire);
            if v.is_multiple_of(2) {
                #[cfg(feature = "occ-stats")]
                if wait_from != 0 {
                    crate::occ_stats::bump_by(
                        crate::occ_stats::Stat::SampleSpinCycles,
                        crate::occ_stats::cycles_now().wrapping_sub(wait_from),
                    );
                }
                return v;
            }
            #[cfg(feature = "occ-stats")]
            if wait_from == 0 {
                wait_from = crate::occ_stats::cycles_now() | 1;
            }
            crate::occ_stats::bump(crate::occ_stats::Stat::SampleSpins);
            core::hint::spin_loop();
            #[cfg(loom)]
            loom::thread::yield_now();
        }
    }

    /// Reader: true when no mutation began since `snapshot` was taken —
    /// everything read in between is consistent. The acquire fence
    /// orders the caller's preceding data loads before this re-read (an
    /// acquire *load* alone orders only what follows it).
    #[must_use]
    pub fn validate(&self, snapshot: u64) -> bool {
        fence(Ordering::Acquire);
        self.0.load(Ordering::Relaxed) == snapshot
    }
}

/// Which version word brackets a store (#568 PR 3): the tree-level
/// [`SeqVersion`] for root state — the `Root` variant, the root leaf, the
/// top edge — or the `u32` of the branch node whose allocation contains the
/// stored address. The rule the reader protocol relies on: **every store to
/// address `x` happens inside the bracket of `node(x)`**, where `node(x)` is
/// the branch containing `x`, or, for a leaf, immediate or subarray payload,
/// the branch whose slot points at it. A frame receives the cover of the
/// node holding its incoming edge and passes its own node's version down.
#[derive(Clone, Copy)]
pub(crate) enum Cover {
    /// The tree-level word (root state).
    Tree,
    /// A branch node's version field.
    Node(*mut u32),
}

impl Cover {
    /// Opens the bracket when `OCC`. In the nested mode (`NESTED`, the
    /// trees whose wrapper holds the tree word for the whole operation) the
    /// frame's own stores are already under the word its parent opened
    /// around the descent (`nest_begin`), so this is a no-op there.
    #[inline(always)]
    pub(crate) fn begin_if<const OCC: bool, const NESTED: bool>(self, a: &crate::alloc::NodeAlloc) {
        if NESTED {
            return;
        }
        match self {
            Cover::Tree => tree_begin_if::<OCC>(a),
            // SAFETY: a live node's version field, per the engine's contract
            // that a `Cover::Node` names the node containing the edge.
            Cover::Node(p) => unsafe { version_begin_if_ptr::<OCC>(a, p) },
        }
    }

    /// The address the debug bracket stack records for this cover.
    #[cfg(debug_assertions)]
    #[inline(always)]
    pub(crate) fn addr(self, a: &crate::alloc::NodeAlloc) -> *const u32 {
        match self {
            #[cfg(feature = "std")]
            Cover::Tree => a.tree_cover_addr(),
            #[cfg(not(feature = "std"))]
            Cover::Tree => {
                let _ = a;
                core::ptr::null()
            }
            Cover::Node(p) => p.cast_const(),
        }
    }

    /// Release-build twin of [`Self::addr`]: the asserts it feeds compile
    /// out, so this is never called.
    #[cfg(not(debug_assertions))]
    #[inline(always)]
    pub(crate) fn addr(self, _a: &crate::alloc::NodeAlloc) -> *const u32 {
        match self {
            Cover::Tree => core::ptr::null(),
            Cover::Node(p) => p.cast_const(),
        }
    }

    /// Nested mode only: opens this node's word before the descent into a
    /// child and holds it until [`Self::nest_end`], so the whole recursion
    /// beneath the node runs under its word — the protocol the string,
    /// bytes and blob wrappers keep, whose readers wait on the tree word
    /// anyway and for whom a per-node brief bracket only turns a wait into
    /// a restart of the whole multi-trie walk (measured, `docs/benchmarks/
    /// concurrency/README.md` §8). A no-op on the tree cover (the wrapper
    /// holds that word) and in the brief-bracket mode.
    #[inline(always)]
    pub(crate) fn nest_begin<const OCC: bool, const NESTED: bool>(
        self,
        a: &crate::alloc::NodeAlloc,
    ) {
        if OCC && NESTED {
            let Cover::Node(p) = self else {
                return;
            };
            // SAFETY: a live node's version field, per the engine's contract
            // that a `Cover::Node` names the node containing the edge.
            unsafe { version_begin_if_ptr::<true>(a, p) }
        }
    }

    /// Closes the word opened by [`Self::nest_begin`].
    #[inline(always)]
    pub(crate) fn nest_end<const OCC: bool, const NESTED: bool>(self, a: &crate::alloc::NodeAlloc) {
        if OCC && NESTED {
            let Cover::Node(p) = self else {
                return;
            };
            // SAFETY: a live node's version field, per the engine's contract
            // that a `Cover::Node` names the node containing the edge.
            unsafe { version_end_if_ptr::<true>(a, p) }
        }
    }

    /// Closes the bracket when `OCC` (a no-op in the nested mode, as
    /// [`Self::begin_if`]).
    #[inline(always)]
    pub(crate) fn end_if<const OCC: bool, const NESTED: bool>(self, a: &crate::alloc::NodeAlloc) {
        if NESTED {
            return;
        }
        match self {
            Cover::Tree => tree_end_if::<OCC>(a),
            // SAFETY: as in `begin_if`.
            Cover::Node(p) => unsafe { version_end_if_ptr::<OCC>(a, p) },
        }
    }
}

/// Opens a node's bracket when `OCC` (`version_begin` on the word `v`),
/// through a raw pointer: the `OCC = true` engine holds no `&mut` to node
/// memory. Also records the word on the debug bracket stack.
///
/// # Safety
///
/// `v` must point at a live branch node's version field owned by the
/// calling writer, with no live `&mut` to it.
#[inline(always)]
pub(crate) unsafe fn version_begin_if_ptr<const OCC: bool>(
    a: &crate::alloc::NodeAlloc,
    v: *mut u32,
) {
    if OCC {
        debug_assert!(a.occ_enabled(), "OCC=true on a non-shared tree");
        // SAFETY: forwarded contract.
        version_begin(unsafe { version_cell(v) });
    }
    #[cfg(debug_assertions)]
    a.bracket_enter(v.cast_const());
    let _ = a;
}

/// Closes the bracket opened by [`version_begin_if_ptr`].
///
/// # Safety
///
/// As [`version_begin_if_ptr`].
#[inline(always)]
pub(crate) unsafe fn version_end_if_ptr<const OCC: bool>(a: &crate::alloc::NodeAlloc, v: *mut u32) {
    if OCC {
        // SAFETY: forwarded contract.
        version_end(unsafe { version_cell(v) });
    }
    #[cfg(debug_assertions)]
    a.bracket_leave(v.cast_const());
    let _ = a;
}

/// Opens the tree-level bracket for a root-state write, when `OCC` and when
/// the engine (not the wrapper) covers root state for this tree
/// (`NodeAlloc::engine_covers_root`). The map and set wrappers hand root
/// coverage to the engine so ordinary writes never touch the tree word; the
/// string, bytes and blob wrappers keep bracketing whole operations in
/// `Shared::write`, and there this is a no-op so the word is never opened
/// twice.
#[inline(always)]
pub(crate) fn tree_begin_if<const OCC: bool>(a: &crate::alloc::NodeAlloc) {
    #[cfg(feature = "std")]
    if OCC && a.engine_covers_root() {
        a.tree_version().begin();
    }
    #[cfg(all(debug_assertions, feature = "std"))]
    if OCC && a.engine_covers_root() {
        a.bracket_enter(a.tree_cover_addr());
    }
    let _ = a;
}

/// Closes the tree-level bracket opened by [`tree_begin_if`].
#[inline(always)]
pub(crate) fn tree_end_if<const OCC: bool>(a: &crate::alloc::NodeAlloc) {
    #[cfg(all(debug_assertions, feature = "std"))]
    if OCC && a.engine_covers_root() {
        a.bracket_leave(a.tree_cover_addr());
    }
    #[cfg(feature = "std")]
    if OCC && a.engine_covers_root() {
        a.tree_version().end();
    }
    let _ = a;
}

/// The node version word, as the OCC protocol addresses it.
///
/// `AtomicU32` normally; loom's under `--cfg loom`, which is what lets
/// `loom_hand_over_hand_node_bracket_safety` call [`version_begin`],
/// [`version_end`], [`node_sample`] and [`node_validate`] instead of
/// re-implementing them (#756). A test that re-implements the protocol cannot
/// fail when the protocol loses a fence.
///
/// Storing through this rather than `write_volatile` also removes a formal
/// data race: the writer's non-atomic volatile store raced the reader's atomic
/// load, which is UB in both the C++ and Rust models however well it behaved.
/// `AtomicU32::store(Relaxed)` lowers to the same instruction.
#[cfg(not(loom))]
pub(crate) type VersionCell = core::sync::atomic::AtomicU32;
/// See the `not(loom)` twin.
#[cfg(loom)]
pub(crate) type VersionCell = loom::sync::atomic::AtomicU32;

/// Views a node's version field as a [`VersionCell`].
///
/// # Safety
///
/// `ptr` must point at a live node's version field (EBR-pinned). The field is
/// only ever accessed through this view, so the `AtomicU32` aliasing is sound:
/// `AtomicU32` has the same size and alignment as `u32`.
#[cfg(not(loom))]
#[inline(always)]
pub(crate) unsafe fn version_cell<'a>(ptr: *const u32) -> &'a VersionCell {
    // SAFETY: caller guarantees a live, aligned version field; `AtomicU32` is
    // layout-compatible with `u32` and this is the only access path to it.
    unsafe { &*ptr.cast::<VersionCell>() }
}

/// Under loom this is unreachable and says so rather than casting.
///
/// loom's atomics carry model state and are **not** layout-compatible with raw
/// memory, so there is no honest cast to make here. The whole crate compiles
/// under `--cfg loom` but the loom job runs only `loom_` tests, which build
/// their own cells; nothing reaches this.
///
/// # Safety
///
/// Never call this under `--cfg loom`.
#[cfg(loom)]
#[inline(always)]
pub(crate) unsafe fn version_cell<'a>(_ptr: *const u32) -> &'a VersionCell {
    panic!(
        "version_cell is not loom-modelled: loom atomics are not layout-compatible \
         with raw node memory. The loom tests construct their own VersionCell."
    )
}

/// Version-word flag of a branch node that has been replaced or removed:
/// odd, so it reads as "mutation in progress" forever, with the top bit
/// set so an obsolete word is distinguishable from a live bracket in a
/// debugger. Readers already treat any odd version as `Retry`.
pub(crate) const OBSOLETE: u32 = 0x8000_0000;

/// Writer: marks a node that is about to become unreachable — its parent's
/// slot is being rewritten to point elsewhere and the node is about to be
/// retired — as [`OBSOLETE`].
///
/// Why this exists: a rebuild copies the old header, version included, into
/// the replacement and retires the old node, and EBR keeps the old node
/// mapped for every pinned reader. A reader that validated the parent,
/// loaded the edge to the old node and was descheduled resumes with a
/// cover whose version nobody will ever bump again; a later mutation of a
/// leaf both old and new node point at, bracketed by the *new* node, is
/// then invisible to that reader's validation. Marking the old node odd
/// before the slot changes and before retirement closes that window: the
/// reader's next sample or validate of it fails and the walk restarts from
/// the root (Leis et al., DaMoN 2016, `writeUnlockObsolete`).
///
/// Same construction as [`version_begin`]: the store, then a release
/// fence, so a reader whose acquire-fenced re-read of this word returns the
/// old even value cannot have observed any store the writer makes after
/// the fence. Taking a `&VersionCell` is what lets
/// `loom_obsolete_mark_covers_replaced_node` call it.
///
/// Must be called **outside** the node's own bracket: `version_end` on an
/// obsolete word would make it even again.
#[inline]
pub(crate) fn version_obsolete(v: &VersionCell) {
    let cur = v.load(Ordering::Relaxed);
    debug_assert!(cur & OBSOLETE == 0, "node marked obsolete twice");
    debug_assert!(
        cur.is_multiple_of(2),
        "node marked obsolete inside its own bracket"
    );
    v.store(OBSOLETE | cur | 1, Ordering::Relaxed);
    fence(Ordering::Release);
}

/// Engine boundary for [`version_obsolete`]: compiled out on unshared trees
/// (`OCC = false`) like every other bracket.
///
/// # Safety
///
/// `v` must point at the version field of a live branch node owned by the
/// calling writer, with no live `&mut` or `&` to that field.
#[inline]
pub(crate) unsafe fn version_obsolete_if<const OCC: bool>(v: *mut u32) {
    if OCC {
        // SAFETY: forwarded contract; `version_cell` carries the liveness
        // obligation.
        version_obsolete(unsafe { version_cell(v) });
    }
}

/// Writer: marks a node mutation in progress (even → odd, then a release
/// fence so the odd version is visible before any covered write).
#[inline]
pub(crate) fn version_begin(v: &VersionCell) {
    let cur = v.load(Ordering::Relaxed);
    debug_assert!(cur.is_multiple_of(2), "nested node write bracket");
    v.store(cur + 1, Ordering::Relaxed);
    fence(Ordering::Release);
}

/// Writer: marks the node mutation complete (odd → even; the release
/// fence orders every covered write before the even version).
#[inline]
pub(crate) fn version_end(v: &VersionCell) {
    let cur = v.load(Ordering::Relaxed);
    debug_assert!(cur % 2 == 1, "version_end without begin");
    fence(Ordering::Release);
    v.store(cur + 1, Ordering::Relaxed);
}

/// Attempts to acquire an exclusive write lock on a node's version word via atomic CAS.
///
/// Follows the OLC protocol specified in `docs/ARCHITECTURE.md` §4.2:
/// If `v` is even and not [`OBSOLETE`], attempts to transition `even -> even + 1`
/// using a strong `compare_exchange` (a single attempt, so a spurious weak-CAS
/// failure would misreport an unlocked node as held).
///
/// On success, executes an acquire fence to ensure subsequent node reads and writes
/// do not reorder prior to lock acquisition.
///
/// Returns `Ok(even_version)` on successful lock acquisition, or `Err(current_version)`
/// if the lock was already held, obsolete, or if the CAS failed.
#[allow(dead_code)]
#[inline]
pub(crate) fn version_try_lock(v: &VersionCell) -> Result<u32, u32> {
    let cur = v.load(Ordering::Relaxed);
    if !cur.is_multiple_of(2) || (cur & OBSOLETE != 0) {
        return Err(cur);
    }
    v.compare_exchange(cur, cur + 1, Ordering::Acquire, Ordering::Relaxed)
}

/// Attempts to acquire an exclusive write lock on a node's version word, verifying
/// that the current version matches `expected` (the OLC snapshot taken during descent).
///
/// This implements the canonical `lockVersionOrRestart` primitive (Leis et al., DaMoN 2016).
/// It succeeds ONLY if `v == expected`. If another writer modified or locked the node in the
/// meantime, the CAS fails, protecting against concurrent shifts and subarray reallocations.
///
/// Returns `Ok(expected)` on success, or `Err(current_version)` if the version changed,
/// was odd, obsolete, or if CAS failed.
#[cfg_attr(not(feature = "std"), allow(dead_code))]
#[inline]
pub(crate) fn version_try_lock_expect(v: &VersionCell, expected: u32) -> Result<u32, u32> {
    if !expected.is_multiple_of(2) || (expected & OBSOLETE != 0) {
        return Err(expected);
    }
    v.compare_exchange(expected, expected + 1, Ordering::Acquire, Ordering::Relaxed)
}

/// Marks an actively locked node as [`OBSOLETE`].
///
/// Must be called while holding the lock (version is odd). Sets `OBSOLETE | 1`
/// with release ordering so that when `NodeLock` drops, `version_unlock` will
/// preserve the obsolete state rather than restoring an even version.
#[allow(dead_code)]
#[inline]
pub(crate) fn version_obsolete_locked(v: &VersionCell) {
    let cur = v.load(Ordering::Relaxed);
    debug_assert!(
        cur % 2 == 1,
        "node must be locked when calling version_obsolete_locked"
    );
    v.store(OBSOLETE | cur, Ordering::Release);
}

/// Releases an exclusive write lock on a node's version word.
///
/// Follows the OLC protocol specified in `docs/ARCHITECTURE.md` §4.2:
/// - If the node was marked [`OBSOLETE`], the obsolete bit and odd parity are preserved.
/// - If `modified` is true, advances the version to `old_v + 2`, invalidating any
///   concurrent optimistic reader that sampled `old_v`.
/// - If `modified` is false, restores `old_v`, preventing spurious reader retries
///   on nodes that were locked during speculative descent or split sizing but left unmodified.
#[allow(dead_code)]
#[inline]
pub(crate) fn version_unlock(v: &VersionCell, old_v: u32, modified: bool) {
    let cur = v.load(Ordering::Relaxed);
    debug_assert!(cur % 2 == 1, "version_unlock without lock");
    // If marked OBSOLETE while locked, preserve the OBSOLETE state.
    if cur & OBSOLETE != 0 {
        return;
    }
    let next = if modified {
        old_v.wrapping_add(2)
    } else {
        old_v
    };
    v.store(next, Ordering::Release);
}

/// Engine boundary for [`version_try_lock`]: tries to lock `v` when `OCC = true`.
///
/// # Safety
///
/// `v` must point to a live branch node's version field.
#[allow(dead_code)]
#[inline(always)]
pub(crate) unsafe fn version_try_lock_if_ptr<const OCC: bool>(v: *mut u32) -> Result<u32, u32> {
    if OCC {
        // SAFETY: forwarded contract; `version_cell` carries liveness obligation.
        version_try_lock(unsafe { version_cell(v) })
    } else {
        Ok(0)
    }
}

/// Engine boundary for [`version_unlock`]: unlocks `v` when `OCC = true`.
///
/// # Safety
///
/// `v` must point to a live branch node's version field previously locked.
#[allow(dead_code)]
#[inline(always)]
pub(crate) unsafe fn version_unlock_if_ptr<const OCC: bool>(
    v: *mut u32,
    old_v: u32,
    modified: bool,
) {
    if OCC {
        // SAFETY: forwarded contract; `version_cell` carries liveness obligation.
        version_unlock(unsafe { version_cell(v) }, old_v, modified);
    }
}

/// An RAII guard representing exclusive write access to a branch node `N`.
///
/// Acquired via [`version_try_lock`]. On drop, automatically unlocks the node
/// via [`version_unlock`] in LIFO order.
///
/// - Defaults to `modified = true` (fail-closed): if a writer panics or returns
///   early without explicit cleanup, the version advances to invalidate readers.
/// - If marked obsolete via [`mark_obsolete`](Self::mark_obsolete), or if dropped
///   during panic unwinding (`std::thread::panicking()`), the node is poisoned as
///   [`OBSOLETE`]. Any reader traversing through this poisoned node will repeatedly
///   fail validation until reaching `MAX_RETRIES`, falling back to the writer lock
///   (which panics on poison), preventing silent reads of torn or corrupted state.
/// - If unmodified, callers can explicitly call [`abort_unmodified`](Self::abort_unmodified)
///   to restore the previous version (`old_v`). Restoring `old_v` is sound because
///   no store occurred: readers that sampled `old_v` before the lock was held validate
///   correctly, and readers that sampled while odd retry.
///
/// # Invariant & Safety
///
/// `NodeLock` strictly dereferences to raw `*mut N`, NEVER `&mut N`, ensuring
/// that Stacked Borrows and Tree Borrows invariants are preserved while concurrent
/// readers sample and load fields of `N`.
#[cfg(feature = "std")]
#[allow(dead_code)]
pub(crate) struct NodeLock<'a, N> {
    ptr: *mut N,
    version: &'a VersionCell,
    old_v: u32,
    modified: core::cell::Cell<bool>,
    obsolete: core::cell::Cell<bool>,
}

#[cfg(feature = "std")]
#[allow(dead_code)]
impl<'a, N> NodeLock<'a, N> {
    /// Attempts to lock `node` using its `VersionCell`.
    ///
    /// # Safety
    ///
    /// `ptr` must point to a live node `N` whose version field is `v`.
    /// The caller must ensure that `ptr` remains valid for the lifetime `'a`.
    #[inline]
    pub(crate) unsafe fn try_lock(ptr: *mut N, v: &'a VersionCell) -> Result<Self, u32> {
        let old_v = version_try_lock(v)?;
        Ok(Self {
            ptr,
            version: v,
            old_v,
            modified: core::cell::Cell::new(true),
            obsolete: core::cell::Cell::new(false),
        })
    }

    /// Constructs a `NodeLock` for an already-acquired lock.
    ///
    /// # Safety
    ///
    /// `v` must have been successfully locked via `version_try_lock` returning `old_v`.
    #[inline]
    pub(crate) unsafe fn from_locked(ptr: *mut N, v: &'a VersionCell, old_v: u32) -> Self {
        Self {
            ptr,
            version: v,
            old_v,
            modified: core::cell::Cell::new(true),
            obsolete: core::cell::Cell::new(false),
        }
    }

    /// Marks the node as modified under this lock.
    #[inline(always)]
    pub(crate) fn mark_modified(&self) {
        self.modified.set(true);
    }

    /// Declares that no modifications were performed on the node under this lock.
    ///
    /// Reverts version advancement on drop, preventing spurious reader retries.
    #[inline(always)]
    pub(crate) fn abort_unmodified(&self) {
        self.modified.set(false);
    }

    /// Marks the node as [`OBSOLETE`].
    ///
    /// When dropped, the node's version word will retain `OBSOLETE | 1`, preventing
    /// concurrent and future readers from validating through this node.
    #[inline(always)]
    pub(crate) fn mark_obsolete(&self) {
        self.obsolete.set(true);
        version_obsolete_locked(self.version);
    }

    /// Returns whether the node was marked modified.
    #[inline(always)]
    pub(crate) fn is_modified(&self) -> bool {
        self.modified.get()
    }

    /// Returns whether the node was marked obsolete.
    #[inline(always)]
    pub(crate) fn is_obsolete(&self) -> bool {
        self.obsolete.get()
    }

    /// Returns the raw pointer to the node.
    #[inline(always)]
    pub(crate) fn as_ptr(&self) -> *mut N {
        self.ptr
    }

    /// Returns the version at the time of lock acquisition.
    #[inline(always)]
    pub(crate) fn old_version(&self) -> u32 {
        self.old_v
    }

    /// Returns the reference to the underlying `VersionCell`.
    #[inline(always)]
    pub(crate) fn version_cell(&self) -> &'a VersionCell {
        self.version
    }
}

#[cfg(feature = "std")]
impl<N> core::ops::Deref for NodeLock<'_, N> {
    type Target = *mut N;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.ptr
    }
}

#[cfg(feature = "std")]
impl<N> Drop for NodeLock<'_, N> {
    #[inline]
    fn drop(&mut self) {
        if self.obsolete.get() || std::thread::panicking() {
            version_obsolete_locked(self.version);
        } else {
            version_unlock(self.version, self.old_v, self.modified.get());
        }
    }
}

/// A descriptor of an acquired lock along a root-to-leaf path.
#[cfg(feature = "std")]
#[derive(Debug)]
pub(crate) enum LockedTarget<'a> {
    Tree(&'a SeqVersion, u64, bool),
    Node(&'a VersionCell, u32, bool, bool),
}

/// A bounded lock set tracking locks acquired top-down along a root-to-leaf path.
///
/// Upholds Invariants S6 (Acyclicity) and S8 (Acquire/Release pairing):
/// - Locks are acquired strictly top-down along the path.
/// - If any acquisition fails, all previously held locks in the set are dropped
///   in reverse (LIFO) order, restoring their unmodified versions, and returning `Err`.
/// - On drop or [`Self::unlock_all`], held locks are released in LIFO order.
///   Modified nodes advance by 2 (`old_v + 2`), unmodified nodes restore `old_v`,
///   and obsolete nodes preserve [`OBSOLETE`].
#[cfg(feature = "std")]
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct LockSet<'a> {
    entries: [Option<LockedTarget<'a>>; 4],
    len: usize,
}

#[cfg(feature = "std")]
#[allow(dead_code)]
impl<'a> LockSet<'a> {
    /// Creates an empty lock set.
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            entries: [None, None, None, None],
            len: 0,
        }
    }

    /// Number of locks currently held.
    #[inline(always)]
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// Whether any locks are currently held.
    #[inline(always)]
    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Attempts to lock the tree-level version word and records it on success.
    ///
    /// On failure, drops all currently held locks in LIFO order (unmodified)
    /// and returns `Err(current_v)`.
    pub(crate) fn try_lock_tree(&mut self, v: &'a SeqVersion) -> Result<usize, u64> {
        debug_assert!(self.len < self.entries.len(), "lock set capacity exceeded");
        match v.try_lock() {
            Ok(old_v) => {
                let idx = self.len;
                self.entries[idx] = Some(LockedTarget::Tree(v, old_v, false));
                self.len += 1;
                Ok(idx)
            }
            Err(cur) => {
                self.unlock_all();
                Err(cur)
            }
        }
    }

    /// Attempts to lock a node's version word and records it on success.
    ///
    /// On failure, drops all currently held locks in LIFO order (unmodified)
    /// and returns `Err(current_v)`.
    pub(crate) fn try_lock_node(&mut self, cell: &'a VersionCell) -> Result<usize, u32> {
        debug_assert!(self.len < self.entries.len(), "lock set capacity exceeded");
        match version_try_lock(cell) {
            Ok(old_v) => {
                let idx = self.len;
                self.entries[idx] = Some(LockedTarget::Node(cell, old_v, false, false));
                self.len += 1;
                Ok(idx)
            }
            Err(cur) => {
                self.unlock_all();
                Err(cur)
            }
        }
    }

    /// Marks the lock at `idx` as modified.
    #[inline]
    pub(crate) fn mark_modified(&mut self, idx: usize) {
        debug_assert!(idx < self.len);
        match &mut self.entries[idx] {
            Some(LockedTarget::Tree(_, _, modified)) => *modified = true,
            Some(LockedTarget::Node(_, _, modified, _)) => *modified = true,
            None => unreachable!(),
        }
    }

    /// Marks the lock at `idx` as obsolete.
    #[inline]
    pub(crate) fn mark_obsolete(&mut self, idx: usize) {
        debug_assert!(idx < self.len);
        match &mut self.entries[idx] {
            Some(LockedTarget::Node(cell, _, modified, obsolete)) => {
                *modified = true;
                *obsolete = true;
                version_obsolete_locked(cell);
            }
            _ => panic!("cannot mark tree word obsolete"),
        }
    }

    /// Releases all held locks in LIFO order.
    pub(crate) fn unlock_all(&mut self) {
        while self.len > 0 {
            self.len -= 1;
            if let Some(target) = self.entries[self.len].take() {
                match target {
                    LockedTarget::Tree(v, old_v, modified) => {
                        v.unlock(old_v, modified);
                    }
                    LockedTarget::Node(cell, old_v, modified, obsolete) => {
                        if obsolete || std::thread::panicking() {
                            version_obsolete_locked(cell);
                        } else {
                            version_unlock(cell, old_v, modified);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(feature = "std")]
impl Drop for LockSet<'_> {
    fn drop(&mut self) {
        self.unlock_all();
    }
}

/// An RAII guard representing an active writer operation within [`WriterGate`].
///
/// On drop or panic unwinding, automatically clears the in-flight writer status,
/// preventing quiescence deadlocks.
#[cfg(feature = "std")]
#[allow(dead_code)]
pub(crate) struct WriterGuard<'a> {
    gate: &'a WriterGate,
    in_flight: &'a AtomicUsize,
    slot_id: usize,
}

#[cfg(feature = "std")]
#[allow(dead_code)]
impl<'a> WriterGuard<'a> {
    #[inline(always)]
    pub(crate) fn slot_id(&self) -> usize {
        self.slot_id
    }
}

#[cfg(feature = "std")]
#[allow(dead_code)]
impl<'a> Drop for WriterGuard<'a> {
    #[inline]
    fn drop(&mut self) {
        self.gate.exit_writer(self.in_flight);
    }
}

/// Coordinates writer quiescence for reader fallback and exclusive operations (`with_locked`).
///
/// Follows the Dekker-style fence pairing protocol specified in `docs/ARCHITECTURE.md` §4.2:
/// - Writers publish an in-flight status flag, execute `fence(Ordering::SeqCst)`, and re-check
///   whether `WriterGate` is closed.
/// - Quiescence coordinators (reader fallback or `with_locked`) close `WriterGate`, execute
///   `fence(Ordering::SeqCst)`, and wait for all registered in-flight writer slots to drain to 0.
#[cfg(all(not(loom), feature = "std"))]
static NEXT_GATE_ID: AtomicU64 = AtomicU64::new(1);

// The same counter per model iteration under loom, not per process. A gate's id
// keys the per-thread writer-slot cache in `sync::Shared::enter_writer`, and
// that cache restarts with each model iteration: an id carried across
// iterations would make a fresh tree's first writer hit the previous
// iteration's cached slot and never call `WriterTable::allocate_slot`.
// (A doc comment on a macro invocation is an `unused_doc_comments` warning.)
#[cfg(all(loom, feature = "std"))]
loom::lazy_static! {
    static ref NEXT_GATE_ID: AtomicU64 = AtomicU64::new(1);
}

#[cfg(feature = "std")]
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct WriterGate {
    closed: AtomicBool,
    id: u64,
}

#[cfg(feature = "std")]
#[allow(dead_code)]
impl WriterGate {
    /// Creates a fresh, open gate.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            closed: AtomicBool::new(false),
            id: NEXT_GATE_ID.fetch_add(1, Ordering::Relaxed),
        }
    }

    /// Unique instance identifier for this gate.
    #[inline(always)]
    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    /// Returns whether the gate is closed (quiescence in progress).
    #[inline(always)]
    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    /// Closes the gate and executes a `SeqCst` fence to initiate quiescence.
    #[inline]
    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
        fence(Ordering::SeqCst);
    }

    /// Re-opens the gate after quiescence completes.
    #[inline]
    pub(crate) fn open(&self) {
        self.closed.store(false, Ordering::Release);
    }

    /// Writer op entry: verifies the gate is open, publishes in-flight status,
    /// executes `fence(Ordering::SeqCst)`, and re-verifies that the gate is not closed.
    ///
    /// `in_flight` counts the writers inside the slot rather than flagging
    /// one: a tree's writer table hashes threads onto taken slots once all
    /// `MAX_WRITER_SLOTS` are allocated, so two live writers can share a
    /// word, and a store of 0 by either would drain it under the other.
    /// The Dekker pairing with [`Self::close`] stays fence-to-fence, the
    /// form loom models: it treats `SeqCst` accesses as `AcqRel` and
    /// supports only `fence(SeqCst)`.
    ///
    /// Returns `Some(WriterGuard)` if entry succeeded, or `None` if the gate is closed.
    #[inline]
    pub(crate) fn enter_writer<'a>(
        &'a self,
        in_flight: &'a AtomicUsize,
        slot_id: usize,
    ) -> Option<WriterGuard<'a>> {
        if self.is_closed() {
            return None;
        }
        in_flight.fetch_add(1, Ordering::Relaxed);
        fence(Ordering::SeqCst);
        if self.is_closed() {
            in_flight.fetch_sub(1, Ordering::Relaxed);
            None
        } else {
            Some(WriterGuard {
                gate: self,
                in_flight,
                slot_id,
            })
        }
    }

    /// Writer op exit: withdraws this writer from the slot's in-flight count.
    #[inline]
    pub(crate) fn exit_writer(&self, in_flight: &AtomicUsize) {
        in_flight.fetch_sub(1, Ordering::Release);
    }

    /// Spins until `in_flight` reads zero: the drain half of quiescence,
    /// run on each allocated writer slot after [`Self::close`].
    ///
    /// The load is `Acquire`, pairing with the `Release` in
    /// [`Self::exit_writer`]: the caller goes on to read, without taking any
    /// lock the writer released, the nodes an optimistic writer stored to,
    /// and this edge is what orders those stores before the reads.
    #[inline]
    pub(crate) fn wait_drained(in_flight: &AtomicUsize) {
        while in_flight.load(Ordering::Acquire) != 0 {
            core::hint::spin_loop();
            #[cfg(loom)]
            loom::thread::yield_now();
        }
    }
}

#[cfg(feature = "std")]
impl Default for WriterGate {
    fn default() -> Self {
        Self::new()
    }
}

/// Marker trait for trie engines that support multi-writer Optimistic Lock Coupling.
///
/// # Safety
///
/// Implementors must uphold the multi-writer OLC protocol invariants specified in
/// `docs/ARCHITECTURE.md` §4.2:
///
/// 1. **Per-Node Write Locking (S5)**: Any shared mutation to a branch node or its child
///    slots must occur under an acquired `NodeLock` (via `version_try_lock`).
/// 2. **Acyclic Lock Acquisition (S6)**: Locks along root-to-leaf paths must be acquired
///    strictly top-down. A writer must never request an ancestor lock while holding a
///    descendant lock.
/// 3. **Release-Acquire Visibility (S8)**: New child nodes and leaf payloads must be
///    published using release-acquire ordering via `version_unlock`.
/// 4. **Obsolete Node Retirement (S3)**: Any node replaced by reorganization must be marked
///    [`OBSOLETE`] before its incoming slot in the parent is rewritten.
/// 5. **Bottom-Up Population Convergence (S7)**: Ancestor `pop0` counts must be updated
///    via isolated bottom-up locks conforming to Rule (a) and Rule (b).
#[cfg(feature = "std")]
#[allow(dead_code)]
pub(crate) unsafe trait OlcEngine: Send {}

/// Reader: loads a node's seqlock version word, returning `None` if
/// torn (odd). Node versions are `u32` (half-words; see Phase 7 node
/// layouts).
///
/// Returns `Some(even)` on a consistent snapshot; `None` if a write is in
/// progress (odd).
///
/// Taking a `&VersionCell` rather than a raw pointer is what makes this
/// callable from the loom model; the EBR-liveness obligation this used to
/// carry now sits on [`version_cell`], which is where the pointer is.
#[cfg(feature = "std")]
pub(crate) fn node_sample(v: &VersionCell) -> Option<u32> {
    let v = v.load(Ordering::Acquire);
    v.is_multiple_of(2).then_some(v)
}

/// Reader: true when the node version still equals `snap` (the acquire
/// fence orders the caller's preceding loads before the re-read).
///
/// Same contract as [`node_sample`].
#[cfg(feature = "std")]
pub(crate) fn node_validate(v: &VersionCell, snap: u32) -> bool {
    fence(Ordering::Acquire);
    v.load(Ordering::Relaxed) == snap
}

/// Number of epoch garbage bins. A retired node becomes freeable once
/// the global epoch has advanced twice past its retirement epoch: every
/// reader pinned at retirement time has since unpinned.
#[cfg(feature = "std")]
const BINS: usize = 3;

/// A reader-registration slot: the epoch the reader is pinned at, or
/// [`INACTIVE`].
#[cfg(feature = "std")]
type Slot = AtomicUsize;

#[cfg(feature = "std")]
const INACTIVE: usize = usize::MAX;

#[cfg(feature = "std")]
#[derive(Debug)]
struct Garbage {
    ptr: NonNull<u8>,
    bytes: usize,
    /// Alignment the allocation was made with. Travels with the pointer
    /// because the collector frees it later and elsewhere; a `dealloc`
    /// layout mismatch is UB, not a leak.
    align: usize,
    /// Stripe of the writer that retired it (`ablation-striped-freelist`):
    /// the reclaimed block goes back to that writer's freelists.
    #[cfg(feature = "ablation-striped-freelist")]
    slot: usize,
}

// SAFETY: a retired allocation is exclusively owned by the collector —
// no live reference remains once its grace period elapses.
#[cfg(feature = "std")]
unsafe impl Send for Garbage {}

#[cfg(feature = "std")]
use crate::alloc::{CLASS_SPECS, FreeBlock, NUM_CLASSES, class_for};

#[cfg(feature = "std")]
#[derive(Debug)]
struct FreeListHead(*mut FreeBlock);

#[cfg(feature = "std")]
// SAFETY: Access to the raw pointer in FreeListHead is synchronized by a Mutex.
unsafe impl Send for FreeListHead {}

#[cfg(feature = "lock-padded")]
#[derive(Debug)]
#[repr(align(64))]
#[allow(dead_code)]
pub(crate) struct Line<X>(pub(crate) X);

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

#[cfg(not(feature = "lock-padded"))]
#[allow(dead_code)]
pub(crate) type Line<X> = X;

#[cfg(feature = "lock-padded")]
#[inline]
#[allow(dead_code)]
pub(crate) fn line<X>(x: X) -> Line<X> {
    Line(x)
}

/// Maximum number of concurrent writer slots tracked for sharded state,
/// tree population, gate quiescence, and epoch bin striping.
///
/// Under Loom, scaled to 2 to model cross-stripe interleavings within Loom's
/// coroutine stack. It is also what makes the writer table's hashed-slot path
/// — two live writers publishing through one in-flight word — reachable from
/// `sync::Shared::enter_writer`: a model runs at most five threads, so 64
/// slots could never be exhausted there
/// (`sync::loom_tests::loom_shared_enter_writer_quiescence`).
#[allow(dead_code)]
#[cfg(not(loom))]
pub(crate) const MAX_WRITER_SLOTS: usize = 64;
#[allow(dead_code)]
#[cfg(loom)]
pub(crate) const MAX_WRITER_SLOTS: usize = 2;

/// One writer stripe of one epoch bin (`ablation-striped-epoch`).
#[cfg(all(feature = "std", feature = "ablation-striped-epoch"))]
#[derive(Debug)]
#[repr(align(64))]
struct PaddedBin {
    garbage: Mutex<Vec<Garbage>>,
    /// Set when garbage is pushed and cleared when it is taken, both under
    /// `garbage`'s lock; read without it. It lets an advance skip empty
    /// stripes with a load instead of a lock, since an advance runs on the
    /// write path. It is a hint: a stale read only delays a stripe to the
    /// bin's next drain, and nothing is freed on its strength.
    nonempty: AtomicBool,
}

#[cfg(all(feature = "std", feature = "ablation-striped-epoch"))]
impl PaddedBin {
    fn new() -> Self {
        Self {
            garbage: Mutex::new(Vec::new()),
            nonempty: AtomicBool::new(false),
        }
    }

    /// Takes everything queued in this stripe.
    fn take(&self) -> Vec<Garbage> {
        let mut guard = self.garbage.lock().expect("garbage bin poisoned");
        self.nonempty.store(false, Ordering::Relaxed);
        core::mem::take(&mut *guard)
    }
}

#[cfg(all(feature = "std", feature = "ablation-striped-epoch"))]
#[derive(Debug)]
#[repr(align(64))]
struct PaddedRetained(AtomicUsize);

/// One writer stripe's size-class freelists (`ablation-striped-freelist`).
/// A stripe's classes share lines only with each other, never with another
/// stripe's.
#[cfg(all(feature = "std", feature = "ablation-striped-freelist"))]
#[derive(Debug)]
#[repr(align(64))]
struct PaddedFreelists([Mutex<FreeListHead>; NUM_CLASSES]);

// The stripe index the diagnostic ablations shard by. It is per thread, not
// per `WriterGate` slot: it is assigned round-robin on a thread's first call,
// so threads created in succession take distinct stripes until
// `MAX_WRITER_SLOTS` of them exist. Two threads that share a stripe share its
// lock or counter, which stays correct; only the ablation's separation is
// lost. Under Loom an unset stripe is 0: a process-wide counter would hand
// out different stripes on different model iterations, and a test that
// wants distinct stripes sets them with `set_writer_slot`.
#[cfg(all(
    feature = "std",
    not(loom),
    any(
        feature = "ablation-sharded-alloc",
        feature = "ablation-striped-epoch",
        feature = "ablation-striped-freelist"
    )
))]
std::thread_local! {
    static STRIPE: core::cell::Cell<usize> = const { core::cell::Cell::new(usize::MAX) };
}

#[cfg(all(
    feature = "std",
    loom,
    any(
        feature = "ablation-sharded-alloc",
        feature = "ablation-striped-epoch",
        feature = "ablation-striped-freelist"
    )
))]
loom::thread_local! {
    static STRIPE: core::cell::Cell<usize> = core::cell::Cell::new(usize::MAX);
}

/// The calling thread's ablation stripe, in `0..MAX_WRITER_SLOTS`.
#[cfg(all(
    feature = "std",
    any(
        feature = "ablation-sharded-alloc",
        feature = "ablation-striped-epoch",
        feature = "ablation-striped-freelist"
    )
))]
#[inline]
pub(crate) fn writer_slot() -> usize {
    STRIPE.with(|s| {
        let v = s.get();
        if v < MAX_WRITER_SLOTS {
            return v;
        }
        #[cfg(not(loom))]
        let v = {
            use core::sync::atomic::{AtomicUsize, Ordering};
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            NEXT.fetch_add(1, Ordering::Relaxed) % MAX_WRITER_SLOTS
        };
        #[cfg(loom)]
        let v = 0;
        s.set(v);
        v
    })
}

/// Pins the calling thread to `slot`, so a test can place work on a chosen
/// stripe.
#[cfg(all(
    test,
    feature = "std",
    any(
        feature = "ablation-sharded-alloc",
        feature = "ablation-striped-epoch",
        feature = "ablation-striped-freelist"
    )
))]
pub(crate) fn set_writer_slot(slot: usize) {
    assert!(slot < MAX_WRITER_SLOTS, "stripe {slot} out of range");
    STRIPE.with(|s| s.set(slot));
}

// Without `std` there are no threads to separate, and the sharded
// allocator accounting (the one ablation that builds there) uses shard 0.
#[cfg(all(not(feature = "std"), feature = "ablation-sharded-alloc"))]
#[inline(always)]
pub(crate) fn writer_slot() -> usize {
    0
}

#[cfg(not(feature = "lock-padded"))]
#[inline]
#[allow(dead_code)]
pub(crate) fn line<X>(x: X) -> Line<X> {
    x
}

/// Epoch-based reclamation for one tree: readers pin the current epoch
/// around each walk; retired allocations wait two epoch advances before
/// they are freed, so a pinned reader can never observe freed memory.
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct Collector {
    epoch: Line<AtomicUsize>,
    advancing: AtomicBool,
    pub(crate) alive: AtomicBool,
    op_count: Line<AtomicUsize>,
    readers: Mutex<Vec<Arc<Slot>>>,
    #[cfg(not(feature = "ablation-striped-epoch"))]
    bins: [Mutex<Vec<Garbage>>; BINS],
    #[cfg(feature = "ablation-striped-epoch")]
    bins: [[PaddedBin; MAX_WRITER_SLOTS]; BINS],
    #[cfg(not(feature = "ablation-striped-freelist"))]
    freelists: [Mutex<FreeListHead>; NUM_CLASSES],
    #[cfg(feature = "ablation-striped-freelist")]
    freelists: [PaddedFreelists; MAX_WRITER_SLOTS],
    #[cfg(not(feature = "ablation-striped-epoch"))]
    retained_bytes: AtomicUsize,
    #[cfg(feature = "ablation-striped-epoch")]
    retained_bytes: [PaddedRetained; MAX_WRITER_SLOTS],
    #[cfg(test)]
    registrations: core::sync::atomic::AtomicU64,
}

#[cfg(feature = "std")]
impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "std")]
impl Collector {
    /// A fresh collector at epoch 0 with no registered readers.
    #[must_use]
    pub fn new() -> Self {
        Self {
            epoch: line(AtomicUsize::new(0)),
            advancing: AtomicBool::new(false),
            alive: AtomicBool::new(true),
            op_count: line(AtomicUsize::new(0)),
            readers: Mutex::new(Vec::new()),
            #[cfg(not(feature = "ablation-striped-epoch"))]
            bins: [
                Mutex::new(Vec::new()),
                Mutex::new(Vec::new()),
                Mutex::new(Vec::new()),
            ],
            #[cfg(feature = "ablation-striped-epoch")]
            bins: core::array::from_fn(|_| core::array::from_fn(|_| PaddedBin::new())),
            #[cfg(not(feature = "ablation-striped-freelist"))]
            freelists: core::array::from_fn(|_| Mutex::new(FreeListHead(core::ptr::null_mut()))),
            #[cfg(feature = "ablation-striped-freelist")]
            freelists: core::array::from_fn(|_| {
                PaddedFreelists(core::array::from_fn(|_| {
                    Mutex::new(FreeListHead(core::ptr::null_mut()))
                }))
            }),
            #[cfg(not(feature = "ablation-striped-epoch"))]
            retained_bytes: AtomicUsize::new(0),
            #[cfg(feature = "ablation-striped-epoch")]
            retained_bytes: core::array::from_fn(|_| PaddedRetained(AtomicUsize::new(0))),
            #[cfg(test)]
            registrations: core::sync::atomic::AtomicU64::new(0),
        }
    }

    /// The freelist an allocation of `class` pops from: the one shared list,
    /// or the calling writer's own under `ablation-striped-freelist`.
    #[inline(always)]
    fn alloc_freelist(&self, class: usize) -> &Mutex<FreeListHead> {
        #[cfg(not(feature = "ablation-striped-freelist"))]
        {
            &self.freelists[class]
        }
        #[cfg(feature = "ablation-striped-freelist")]
        {
            &self.freelists[writer_slot()].0[class]
        }
    }

    /// The freelist a reclaimed block of `class` is pushed to: the one shared
    /// list, or the retiring writer's own under `ablation-striped-freelist`.
    #[inline(always)]
    fn reclaim_freelist(&self, class: usize, g: &Garbage) -> &Mutex<FreeListHead> {
        #[cfg(not(feature = "ablation-striped-freelist"))]
        {
            let _ = g;
            &self.freelists[class]
        }
        #[cfg(feature = "ablation-striped-freelist")]
        {
            &self.freelists[g.slot].0[class]
        }
    }

    /// Pops a reclaimed block from this collector's size-class freelist.
    #[inline(always)]
    pub(crate) fn pop_freelist(&self, class: usize) -> *mut u8 {
        let mut head = self
            .alloc_freelist(class)
            .lock()
            .expect("freelist poisoned");
        let block = head.0;
        if block.is_null() {
            return core::ptr::null_mut();
        }
        // SAFETY: block points to a valid FreeBlock in this size class.
        head.0 = unsafe { (*block).next };
        drop(head);
        let bytes = CLASS_SPECS[class].0;
        // SAFETY: zero out the reused memory before returning.
        unsafe { core::ptr::write_bytes(block.cast::<u8>(), 0, bytes) };
        block.cast::<u8>()
    }

    /// Registers a reader; the returned handle pins/unpins cheaply.
    #[must_use]
    pub fn register(self: &Arc<Self>) -> Reader {
        #[cfg(test)]
        self.registrations.fetch_add(1, Ordering::Relaxed);
        let slot = Arc::new(Slot::new(INACTIVE));
        self.readers
            .lock()
            .expect("reader registry poisoned")
            .push(Arc::clone(&slot));
        Reader {
            collector: Arc::clone(self),
            slot,
        }
    }

    /// Queues an allocation for deferred freeing (writer side).
    pub fn retire(&self, ptr: NonNull<u8>, bytes: usize, align: usize) {
        crate::occ_stats::bump(crate::occ_stats::Stat::Retired);
        // S4 store-buffer pairing with `try_advance` / `Reader::pin` (ARCHITECTURE.md §4.2):
        // the retire-side epoch load is preceded by a SeqCst fence, ensuring that a retirer
        // has a happens-before with an advance and readers pinned at the next epoch.
        fence(Ordering::SeqCst);
        let e = self.epoch.load(Ordering::Relaxed);
        let g = Garbage {
            ptr,
            bytes,
            align,
            #[cfg(feature = "ablation-striped-freelist")]
            slot: writer_slot(),
        };
        #[cfg(not(feature = "ablation-striped-epoch"))]
        {
            self.bins[e % BINS]
                .lock()
                .expect("garbage bin poisoned")
                .push(g);
            self.retained_bytes.fetch_add(bytes, Ordering::Relaxed);
        }
        #[cfg(feature = "ablation-striped-epoch")]
        {
            let slot = writer_slot();
            let stripe = &self.bins[e % BINS][slot];
            let mut garbage = stripe.garbage.lock().expect("garbage bin poisoned");
            // Counted before the push: an advance may take the block as soon
            // as the lock drops, and its per-stripe subtraction must not come
            // first, or the stripe's counter wraps.
            self.retained_bytes[slot]
                .0
                .fetch_add(bytes, Ordering::Relaxed);
            garbage.push(g);
            stripe.nonempty.store(true, Ordering::Relaxed);
        }
        crate::occ_stats::record_retire(bytes);
    }

    /// Attempts one epoch advance: succeeds when every pinned reader has
    /// caught up to the current epoch, then frees the bin two epochs
    /// back. Writer-side, amortized (call once per mutation batch).
    ///
    /// Protected by an advancer try-lock (`advancing`) and CAS on `self.epoch`
    /// to guarantee that concurrent callers cannot run simultaneously or roll the epoch backwards.
    pub fn try_advance(&self) {
        crate::occ_stats::bump(crate::occ_stats::Stat::AdvanceCalls);
        if self
            .advancing
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        struct AdvancingGuard<'a>(&'a AtomicBool);
        impl Drop for AdvancingGuard<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _guard = AdvancingGuard(&self.advancing);

        let e = self.epoch.load(Ordering::Relaxed);
        // Store-buffer pairing with `Reader::pin` (its slot store /
        // epoch load run against our epoch store / slot loads): the
        // SeqCst fences on both sides guarantee at least one of them
        // observes the other, so a pin this scan misses has itself seen
        // the current epoch. Loom's model found the interleaving plain
        // SeqCst accesses (which loom models weakly) let through.
        fence(Ordering::SeqCst);
        {
            let readers = self.readers.lock().expect("reader registry poisoned");
            for slot in readers.iter() {
                // Acquire: a reader's release-unpin (its walk's loads
                // complete) must happen-before we free what it read.
                let s = slot.load(Ordering::Acquire);
                if s != INACTIVE && s != e {
                    return; // a reader still runs in an older epoch
                }
            }
        }
        if self
            .epoch
            .compare_exchange(e, e + 1, Ordering::SeqCst, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        crate::occ_stats::bump(crate::occ_stats::Stat::AdvanceOk);
        // Everything retired at epoch e - 1 predates every possible pin
        // in epochs e and e + 1: no live reader can hold it.
        #[cfg(not(feature = "ablation-striped-epoch"))]
        {
            let stale = core::mem::take(
                &mut *self.bins[(e + BINS - 1) % BINS]
                    .lock()
                    .expect("garbage bin poisoned"),
            );
            let mut freed_bytes = 0;
            for g in stale {
                freed_bytes += g.bytes;
                if let Some(class) = class_for(g.bytes, g.align) {
                    let block = g.ptr.as_ptr().cast::<FreeBlock>();
                    let mut head = self
                        .reclaim_freelist(class, &g)
                        .lock()
                        .expect("freelist poisoned");
                    // SAFETY: block was retired by a well-aligned allocation
                    // matching this size class, and grace period elapsed.
                    unsafe {
                        (*block).next = head.0;
                    }
                    head.0 = block;
                } else {
                    free_raw(g.ptr, g.bytes, g.align);
                }
            }
            self.retained_bytes
                .fetch_sub(freed_bytes, Ordering::Relaxed);
            crate::occ_stats::record_reclaim(freed_bytes);
        }
        #[cfg(feature = "ablation-striped-epoch")]
        {
            let stale_bin = (e + BINS - 1) % BINS;
            for slot in 0..MAX_WRITER_SLOTS {
                let stripe = &self.bins[stale_bin][slot];
                // A stripe whose flag reads clear is skipped without its
                // lock; one being filled right now keeps its garbage until
                // this bin's next drain.
                if !stripe.nonempty.load(Ordering::Relaxed) {
                    continue;
                }
                let stale = stripe.take();
                let mut freed_bytes = 0;
                for g in stale {
                    freed_bytes += g.bytes;
                    if let Some(class) = class_for(g.bytes, g.align) {
                        let block = g.ptr.as_ptr().cast::<FreeBlock>();
                        let mut head = self
                            .reclaim_freelist(class, &g)
                            .lock()
                            .expect("freelist poisoned");
                        // SAFETY: block was retired by a well-aligned allocation
                        // matching this size class, and grace period elapsed.
                        unsafe {
                            (*block).next = head.0;
                        }
                        head.0 = block;
                    } else {
                        free_raw(g.ptr, g.bytes, g.align);
                    }
                }
                self.retained_bytes[slot]
                    .0
                    .fetch_sub(freed_bytes, Ordering::Relaxed);
                crate::occ_stats::record_reclaim(freed_bytes);
            }
        }
    }

    /// Records one mutation operation and triggers `try_advance()` if `ADVANCE_EVERY` operations have elapsed.
    #[inline]
    pub(crate) fn tick_advance(&self) {
        #[cfg(not(feature = "advance-never"))]
        {
            if self
                .op_count
                .fetch_add(1, Ordering::Relaxed)
                .is_multiple_of(crate::sync::ADVANCE_EVERY as usize)
            {
                self.try_advance();
            }
        }
    }

    /// Total bytes currently queued across this collector's garbage bins.
    ///
    /// Measures unreclaimed garbage backlog queued in epoch bins awaiting
    /// reclamation, not total allocated heap footprint (reclaimed node
    /// blocks transition to collector size-class freelists for reuse).
    #[must_use]
    pub fn retained_bytes(&self) -> usize {
        #[cfg(not(feature = "ablation-striped-epoch"))]
        {
            self.retained_bytes.load(Ordering::Relaxed)
        }
        #[cfg(feature = "ablation-striped-epoch")]
        {
            let mut sum: usize = 0;
            for s in &self.retained_bytes {
                sum += s.0.load(Ordering::Relaxed);
            }
            sum
        }
    }

    /// Number of registered reader slots. Test-only observability: the #554
    /// regression guard asserts a cached reader does not grow this per lookup.
    #[cfg(test)]
    pub(crate) fn registered_readers(&self) -> usize {
        self.readers.lock().expect("reader registry poisoned").len()
    }

    /// Cumulative `register` calls. Registry *size* cannot catch a one-shot
    /// lookup — it registers and deregisters inside the call, so the size is
    /// unchanged by the time a test looks. Counting the calls is what actually
    /// distinguishes a cached reader from a per-lookup one (#554).
    #[cfg(test)]
    pub(crate) fn registrations(&self) -> u64 {
        self.registrations.load(Ordering::Relaxed)
    }

    /// Current epoch. Test-only observability, so `sync`'s write-batching
    /// policy can assert how often it actually advances.
    // Its only caller — `sync::tests` — is `#[cfg(not(miri))]`, so under
    // Miri this is legitimately unreferenced.
    #[cfg(test)]
    #[cfg_attr(miri, allow(dead_code))]
    #[cfg_attr(feature = "advance-never", allow(dead_code))]
    pub(crate) fn epoch_now(&self) -> usize {
        self.epoch.load(Ordering::Relaxed)
    }

    /// Frees everything still queued in garbage bins and size-class freelists.
    /// Only sound once no reader can be pinned (the owning wrapper calls this
    /// on drop, when exclusive ownership proves that).
    pub(crate) fn drain(&self) {
        #[cfg(not(feature = "ablation-striped-epoch"))]
        {
            for bin in &self.bins {
                let stale = core::mem::take(&mut *bin.lock().expect("garbage bin poisoned"));
                let mut freed_bytes = 0;
                for g in stale {
                    freed_bytes += g.bytes;
                    free_raw(g.ptr, g.bytes, g.align);
                }
                self.retained_bytes
                    .fetch_sub(freed_bytes, Ordering::Relaxed);
                crate::occ_stats::record_reclaim(freed_bytes);
            }
        }
        #[cfg(feature = "ablation-striped-epoch")]
        {
            for b in 0..BINS {
                for slot in 0..MAX_WRITER_SLOTS {
                    // Every stripe, whatever its flag says: nothing may
                    // outlive the collector.
                    let stale = self.bins[b][slot].take();
                    if stale.is_empty() {
                        continue;
                    }
                    let mut freed_bytes = 0;
                    for g in stale {
                        freed_bytes += g.bytes;
                        free_raw(g.ptr, g.bytes, g.align);
                    }
                    self.retained_bytes[slot]
                        .0
                        .fetch_sub(freed_bytes, Ordering::Relaxed);
                    crate::occ_stats::record_reclaim(freed_bytes);
                }
            }
        }
        #[cfg(not(feature = "ablation-striped-freelist"))]
        let rows = core::iter::once(&self.freelists);
        #[cfg(feature = "ablation-striped-freelist")]
        let rows = self.freelists.iter().map(|stripe| &stripe.0);
        for row in rows {
            for (class, &(bytes, align)) in CLASS_SPECS.iter().enumerate() {
                let mut head = row[class].lock().expect("freelist poisoned");
                let mut cur = head.0;
                head.0 = core::ptr::null_mut();
                drop(head);
                let layout = Layout::from_size_align(bytes, align).expect("valid node layout");
                while !cur.is_null() {
                    // SAFETY: cur was allocated with `layout`.
                    let next = unsafe { (*cur).next };
                    // SAFETY: deallocating unreferenced freelist block with its original layout.
                    unsafe { dealloc(cur.cast::<u8>(), layout) };
                    cur = next;
                }
            }
        }
    }
}

#[cfg(feature = "std")]
impl Drop for Collector {
    fn drop(&mut self) {
        // Frees queued garbage bins and size-class freelists.
        self.drain();
    }
}

#[cfg(feature = "std")]
fn free_raw(ptr: NonNull<u8>, bytes: usize, align: usize) {
    crate::occ_stats::bump(crate::occ_stats::Stat::FreedRaw);
    let layout = Layout::from_size_align(bytes, align).expect("valid retired layout");
    // SAFETY: retired allocations come from `NodeAlloc` (same
    // size/alignment contract) and are freed exactly once, after their
    // grace period.
    unsafe { dealloc(ptr.as_ptr(), layout) };
}

/// A registered reader handle. Cheap to pin/unpin per operation; not
/// `Sync` — one per reading thread.
#[cfg(feature = "std")]
pub struct Reader {
    pub(crate) collector: Arc<Collector>,
    slot: Arc<Slot>,
}

#[cfg(feature = "std")]
impl Reader {
    /// Returns true if this reader handle's collector has been marked dead by the owning tree.
    #[inline]
    pub(crate) fn is_orphan(&self) -> bool {
        !self.collector.alive.load(Ordering::Acquire)
    }
    /// Pins the current epoch for the duration of the returned guard:
    /// nothing retired from here on is freed while the guard lives.
    ///
    /// Pins from one `Reader` must never overlap: the reader has a single
    /// epoch slot, so dropping *any* [`Pin`] unpins the reader entirely —
    /// an outstanding sibling pin would silently lose its protection.
    /// Wrappers that expose long-lived guards must rule the overlap out
    /// statically (`sync::BlobReader::pin` takes `&mut self` for exactly
    /// this reason); for overlapping pins, register a second reader.
    #[must_use]
    pub fn pin(&self) -> Pin<'_> {
        // Loop until the epoch is stable across the pin store, with a
        // SeqCst fence between the store and the re-read: this is the
        // store-buffer pairing with `Collector::try_advance` (see there)
        // — either the writer's scan sees this pin, or this re-read sees
        // the writer's new epoch and the loop re-pins.
        let mut e = self.collector.epoch.load(Ordering::Relaxed);
        loop {
            self.slot.store(e, Ordering::Relaxed);
            fence(Ordering::SeqCst);
            let e2 = self.collector.epoch.load(Ordering::Relaxed);
            if e2 == e {
                break;
            }
            e = e2;
        }
        Pin { slot: &self.slot }
    }
}

#[cfg(feature = "std")]
impl Drop for Reader {
    fn drop(&mut self) {
        // Deregister: drop the slot from the registry so a departed
        // thread cannot stall epoch advances.
        let mut readers = self
            .collector
            .readers
            .lock()
            .expect("reader registry poisoned");
        readers.retain(|s| !Arc::ptr_eq(s, &self.slot));
    }
}

/// An active pin; dropping it unpins the reader.
#[cfg(feature = "std")]
pub struct Pin<'a> {
    slot: &'a Slot,
}

#[cfg(feature = "std")]
impl Drop for Pin<'_> {
    fn drop(&mut self) {
        self.slot.store(INACTIVE, Ordering::Release);
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::*;
    use std::alloc::alloc_zeroed;

    /// Retired blocks must be freed at the alignment they were made
    /// with, so the test allocates and retires at one constant.
    const TEST_ALIGN: usize = 16;

    fn alloc_test_block(bytes: usize) -> NonNull<u8> {
        let layout = Layout::from_size_align(bytes, TEST_ALIGN).unwrap();
        // SAFETY: nonzero-size layout.
        NonNull::new(unsafe { alloc_zeroed(layout) }).unwrap()
    }

    #[test]
    fn seq_version_protocol() {
        let v = SeqVersion::new();
        let s = v.sample();
        assert!(v.validate(s));
        v.begin();
        assert!(!v.validate(s));
        v.end();
        let s2 = v.sample();
        assert_ne!(s, s2);
        assert!(v.validate(s2));
    }

    #[test]
    fn epochs_defer_frees_until_readers_leave() {
        let c = Arc::new(Collector::new());
        let reader = c.register();
        let pin = reader.pin();
        c.retire(alloc_test_block(64), 64, TEST_ALIGN);
        // A reader pinned at the current epoch permits one advance (it
        // only stalls once it lags), but its pin still precedes the
        // retirement's grace period: the block retired at its epoch
        // cannot be freed until the pin is gone.
        c.try_advance(); // advances; pinned reader now lags one epoch
        c.try_advance(); // refused: the lagging pin stalls this one
        drop(pin);
        c.try_advance(); // advances; frees the block retired above
        c.try_advance(); // advances; empty bin
        drop(reader);
        c.drain();
    }

    #[test]
    fn departed_reader_does_not_stall() {
        let c = Arc::new(Collector::new());
        let r1 = c.register();
        let _pin_forever = r1.pin();
        let epoch_before = c.epoch.load(Ordering::Relaxed);
        // A pinned reader at the current epoch does NOT stall advances
        // (it pins the epoch it saw; only lagging readers stall).
        c.try_advance();
        assert_eq!(c.epoch.load(Ordering::Relaxed), epoch_before + 1);
        // Now r1 lags one epoch behind and stalls further advances...
        c.try_advance();
        assert_eq!(c.epoch.load(Ordering::Relaxed), epoch_before + 1);
        // ...until its thread departs entirely.
        drop(_pin_forever);
        drop(r1);
        c.try_advance();
        assert_eq!(c.epoch.load(Ordering::Relaxed), epoch_before + 2);
    }

    #[test]
    fn collector_retained_bytes_accounting() {
        crate::occ_stats::reset();
        let c = Arc::new(Collector::new());
        assert_eq!(c.retained_bytes(), 0);

        let reader = c.register();
        let pin = reader.pin();

        c.retire(alloc_test_block(64), 64, TEST_ALIGN);
        c.retire(alloc_test_block(128), 128, TEST_ALIGN);
        assert_eq!(c.retained_bytes(), 192);

        // One advance is allowed while pinned at epoch 0 (advances to epoch 1)
        c.try_advance();
        assert_eq!(c.retained_bytes(), 192);

        // Subsequent advance refused because reader is lagging at epoch 0
        c.retire(alloc_test_block(256), 256, TEST_ALIGN);
        assert_eq!(c.retained_bytes(), 448);
        c.try_advance();
        assert_eq!(c.retained_bytes(), 448);

        // Release reader pin: advances can now proceed and reclaim stale bins
        drop(pin);
        c.try_advance(); // advances to epoch 2, reclaims epoch 0 blocks (64 + 128 = 192)
        assert_eq!(c.retained_bytes(), 256);
        c.try_advance(); // advances to epoch 3, reclaims epoch 1 blocks (256)
        assert_eq!(c.retained_bytes(), 0);

        drop(reader);
        c.drain();
    }

    #[test]
    fn node_lock_and_unlock_modified() {
        let cell = VersionCell::new(0);
        let mut dummy = 1234u64;
        {
            // SAFETY: dummy is a live stack variable.
            let lock = unsafe { NodeLock::try_lock(&raw mut dummy, &cell) }.expect("lock acquired");
            assert_eq!(*lock, &raw mut dummy);
            assert_eq!(lock.old_version(), 0);
            assert!(lock.is_modified()); // Fail closed: modified by default
            // cell is odd while locked
            assert_eq!(cell.load(Ordering::Relaxed), 1);
        }
        // Dropped with modified = true: advanced to old_v + 2 = 2
        assert_eq!(cell.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn node_lock_and_unlock_unmodified() {
        let cell = VersionCell::new(4);
        let mut dummy = 5678u64;
        {
            // SAFETY: dummy is a live stack variable.
            let lock = unsafe { NodeLock::try_lock(&raw mut dummy, &cell) }.expect("lock acquired");
            assert_eq!(lock.old_version(), 4);
            assert_eq!(cell.load(Ordering::Relaxed), 5);
            // Explicitly abort unmodified: restores old_v = 4
            lock.abort_unmodified();
            assert!(!lock.is_modified());
        }
        assert_eq!(cell.load(Ordering::Relaxed), 4);
    }

    #[test]
    fn node_lock_and_unlock_obsolete() {
        let cell = VersionCell::new(0);
        let mut dummy = 1234u64;
        {
            // SAFETY: dummy is a live stack variable.
            let lock = unsafe { NodeLock::try_lock(&raw mut dummy, &cell) }.expect("lock acquired");
            lock.mark_obsolete();
            assert!(lock.is_obsolete());
            assert_eq!(cell.load(Ordering::Relaxed), OBSOLETE | 1);
        }
        // Dropped with obsolete = true: retains OBSOLETE | 1
        assert_eq!(cell.load(Ordering::Relaxed), OBSOLETE | 1);
        // Future try_lock fails due to OBSOLETE
        assert!(version_try_lock(&cell).is_err());
    }

    #[test]
    fn version_try_lock_expect_semantics() {
        let cell = VersionCell::new(4);

        // 1. Success on matching even version: locks cell (4 -> 5)
        assert_eq!(version_try_lock_expect(&cell, 4), Ok(4));
        assert_eq!(cell.load(Ordering::Relaxed), 5);

        // Unlock back to 6 (modified)
        version_unlock(&cell, 4, true);
        assert_eq!(cell.load(Ordering::Relaxed), 6);

        // 2. Failure on stale snapshot (expected 4, but current is 6)
        assert_eq!(version_try_lock_expect(&cell, 4), Err(6));
        // Cell remains unlocked and unmodified
        assert_eq!(cell.load(Ordering::Relaxed), 6);

        // 3. Failure on odd snapshot (expected 5)
        assert_eq!(version_try_lock_expect(&cell, 5), Err(5));

        // 4. Failure on obsolete snapshot
        assert_eq!(
            version_try_lock_expect(&cell, 6 | OBSOLETE),
            Err(6 | OBSOLETE)
        );
    }

    #[test]
    fn version_try_lock_expect_discriminates_stale_advancement() {
        let cell = VersionCell::new(4);
        let snapshot = 4;

        // Simulate concurrent modification: node advances 4 -> 5 -> 6
        cell.store(6, Ordering::Release);

        // Under old un-anchored try_lock:
        // Plain try_lock succeeds despite stale snapshot because 6 is even!
        let old_behavior_lock = version_try_lock(&cell);
        assert!(
            old_behavior_lock.is_ok(),
            "plain try_lock cannot detect stale snapshot"
        );
        version_unlock(&cell, 6, false); // restore to 6

        // Under version_try_lock_expect with snapshot = 4:
        // Must fail with Err(6) because the cell advanced past the snapshot!
        let anchored_lock = version_try_lock_expect(&cell, snapshot);
        assert_eq!(
            anchored_lock,
            Err(6),
            "version_try_lock_expect MUST reject lock on stale snapshot"
        );
    }

    #[test]
    fn writer_gate_protocol() {
        let gate = WriterGate::new();
        let in_flight = AtomicUsize::new(0);
        assert!(!gate.is_closed());

        // Normal entry when open
        {
            let guard = gate.enter_writer(&in_flight, 0);
            assert!(guard.is_some());
            assert_eq!(in_flight.load(Ordering::Relaxed), 1);
        }
        assert_eq!(in_flight.load(Ordering::Relaxed), 0);

        // Quiescence closure
        gate.close();
        assert!(gate.is_closed());
        // Entry fails when closed
        assert!(gate.enter_writer(&in_flight, 0).is_none());
        assert_eq!(in_flight.load(Ordering::Relaxed), 0);

        gate.open();
        assert!(!gate.is_closed());
        {
            let guard = gate.enter_writer(&in_flight, 0);
            assert!(guard.is_some());
            assert_eq!(in_flight.load(Ordering::Relaxed), 1);
        }
        assert_eq!(in_flight.load(Ordering::Relaxed), 0);
    }

    /// Two writers publishing through one in-flight word, as they do once a
    /// tree's writer table has allocated all its slots and hashes further
    /// threads onto taken ones. Quiescence reads the word as drained when it
    /// is zero, so neither one writer's exit nor another's back-out from a
    /// closed gate may zero it while a guard is still held.
    #[test]
    fn writer_gate_shared_slot_stays_in_flight_until_last_exit() {
        let gate = WriterGate::new();
        let in_flight = AtomicUsize::new(0);

        let a = gate.enter_writer(&in_flight, 7).expect("gate is open");
        let b = gate.enter_writer(&in_flight, 7).expect("gate is open");
        drop(a);
        assert_ne!(
            in_flight.load(Ordering::Relaxed),
            0,
            "the first exit drained a slot another writer still holds"
        );
        drop(b);
        assert_eq!(in_flight.load(Ordering::Relaxed), 0);

        let c = gate.enter_writer(&in_flight, 7).expect("gate is open");
        gate.close();
        assert!(gate.enter_writer(&in_flight, 7).is_none());
        assert_ne!(
            in_flight.load(Ordering::Relaxed),
            0,
            "a back-out from the closed gate drained a slot another writer still holds"
        );
        drop(c);
        assert_eq!(in_flight.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn lock_set_normal_and_abort_protocol() {
        let v_tree = SeqVersion::new();
        let v1 = VersionCell::new(0);
        let v2 = VersionCell::new(0);

        // 1. Normal acquisition and modified release
        {
            let mut ls = LockSet::new();
            let t_idx = ls.try_lock_tree(&v_tree).expect("lock tree");
            let n1_idx = ls.try_lock_node(&v1).expect("lock v1");
            let _n2_idx = ls.try_lock_node(&v2).expect("lock v2");

            ls.mark_modified(t_idx);
            ls.mark_modified(n1_idx);
            // n2 remains unmodified
            assert_eq!(ls.len(), 3);
            ls.unlock_all();
        }
        // Tree advanced to 2
        assert_eq!(v_tree.0.load(Ordering::Relaxed), 2);
        // v1 advanced to 2 (modified)
        assert_eq!(v1.load(Ordering::Relaxed), 2);
        // v2 remained at 0 (unmodified restore)
        assert_eq!(v2.load(Ordering::Relaxed), 0);

        // 2. Abort on second lock acquisition drops previous locks unmodified
        {
            let mut ls = LockSet::new();
            let _t_idx = ls.try_lock_tree(&v_tree).expect("lock tree");
            let _n1_idx = ls.try_lock_node(&v1).expect("lock v1");

            // Lock v2 externally so try_lock_node fails
            let _ext = version_try_lock(&v2).expect("external lock");

            // try_lock_node on v2 must fail and unlock v_tree and v1 unmodified
            assert!(ls.try_lock_node(&v2).is_err());
            assert_eq!(ls.len(), 0);

            // Unlock external lock
            version_unlock(&v2, 0, false);
        }
        // Tree restored to 2 (unmodified)
        assert_eq!(v_tree.0.load(Ordering::Relaxed), 2);
        // v1 restored to 2 (unmodified)
        assert_eq!(v1.load(Ordering::Relaxed), 2);
    }

    /// Retires one block on every stripe. Each lands in its own stripe, a
    /// pin holds all of them, and the advances after the unpin reclaim all
    /// of them: a retire that ignored the stripe, or an advance that skipped
    /// one, leaves a stripe's bytes behind.
    #[test]
    #[cfg(feature = "ablation-striped-epoch")]
    fn ablation_striped_epoch_reclaims_every_stripe() {
        let c = Arc::new(Collector::new());
        let reader = c.register();
        let pin = reader.pin();
        let layout = Layout::from_size_align(64, 16).unwrap();
        for slot in 0..MAX_WRITER_SLOTS {
            set_writer_slot(slot);
            // SAFETY: non-zero size and a valid power-of-two alignment.
            let ptr = NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) }).unwrap();
            c.retire(ptr, 64, 16);
            assert_eq!(c.retained_bytes[slot].0.load(Ordering::Relaxed), 64);
        }
        let all = 64 * MAX_WRITER_SLOTS;
        assert_eq!(c.retained_bytes(), all);

        c.try_advance();
        c.try_advance();
        assert_eq!(
            c.retained_bytes(),
            all,
            "a pinned reader must hold every stripe"
        );

        drop(pin);
        c.try_advance();
        c.try_advance();
        assert_eq!(
            c.retained_bytes(),
            0,
            "an advance must reclaim every stripe"
        );
    }

    /// `drain` frees every stripe of every bin, not just the ones an advance
    /// would reach.
    #[test]
    #[cfg(feature = "ablation-striped-epoch")]
    fn ablation_striped_epoch_drain_empties_every_stripe() {
        let c = Collector::new();
        let layout = Layout::from_size_align(64, 16).unwrap();
        for slot in 0..MAX_WRITER_SLOTS {
            set_writer_slot(slot);
            // SAFETY: non-zero size and a valid power-of-two alignment.
            let ptr = NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) }).unwrap();
            c.retire(ptr, 64, 16);
        }
        assert_eq!(c.retained_bytes(), 64 * MAX_WRITER_SLOTS);
        c.drain();
        assert_eq!(c.retained_bytes(), 0);
    }

    /// A reclaimed block goes back to the freelist of the stripe that
    /// retired it, and only an allocation on that stripe pops it.
    #[test]
    #[cfg(feature = "ablation-striped-freelist")]
    fn ablation_striped_freelist_routes_by_stripe() {
        let c = Collector::new();
        let class = 1;
        let (bytes, align) = CLASS_SPECS[class];
        assert_eq!(class_for(bytes, align), Some(class));
        let layout = Layout::from_size_align(bytes, align).unwrap();
        let home = MAX_WRITER_SLOTS - 1;

        set_writer_slot(home);
        // SAFETY: non-zero size and a valid power-of-two alignment.
        let ptr = NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) }).unwrap();
        c.retire(ptr, bytes, align);
        // No reader is registered, so these advances reclaim the block.
        for _ in 0..BINS {
            c.try_advance();
        }
        assert_eq!(c.retained_bytes(), 0);

        set_writer_slot(0);
        assert!(
            c.pop_freelist(class).is_null(),
            "another stripe must not see the block"
        );
        set_writer_slot(home);
        let got = c.pop_freelist(class);
        assert_eq!(
            got,
            ptr.as_ptr(),
            "the retiring stripe must get its block back"
        );
        // SAFETY: `got` was allocated with `layout` and the test now owns it.
        unsafe { dealloc(got, layout) };
    }
}

/// Residual coverage (AGENTS.md §5) of the striped-epoch ablation. Loom runs
/// with `MAX_WRITER_SLOTS = 2` and checks two writers on stripes 0 and 1
/// racing a third thread that advances the epoch: under a pin, and without
/// one, so that a retire can land in the bin an advance is draining. What
/// Loom does not reach — all 64 stripes, and the striped bins inside a real
/// tree — is covered by the `ablation_` unit tests in `occ::tests` (also run
/// under Miri) and by `tests/test_concurrency_ablations.rs` (also run under
/// ASan), which are deterministic about stripes, not about schedules.
#[cfg(all(test, loom))]
mod loom_tests {
    use super::*;
    /// The EBR safety property on observable state: while a reader is
    /// pinned at epoch `e`, the writer can advance at most once (to
    /// `e + 1`) — so a bin retired at `e` (freed only by the advance
    /// *from* `e + 1`) can never be freed under that pin.
    #[test]
    fn loom_pin_blocks_second_advance() {
        loom::model(|| {
            let c = Arc::new(Collector::new());
            let reader = c.register();

            let cw = Arc::clone(&c);
            let writer = loom::thread::spawn(move || {
                cw.try_advance();
                cw.try_advance();
            });

            {
                let _pin = reader.pin();
                let pinned_at = reader.slot.load(Ordering::SeqCst);
                let now = c.epoch.load(Ordering::SeqCst);
                assert!(
                    now <= pinned_at + 1,
                    "epoch advanced twice past a live pin: pinned {pinned_at}, now {now}"
                );
            }

            writer.join().unwrap();
            drop(reader);
        });
    }

    /// Seqlock: a reader that validates successfully saw either the
    /// old or the new value, never a torn intermediate.
    #[test]
    fn loom_seqlock_no_torn_reads() {
        loom::model(|| {
            let v = Arc::new(SeqVersion::new());
            let data = Arc::new((AtomicU64::new(1), AtomicU64::new(2)));

            let vw = Arc::clone(&v);
            let dw = Arc::clone(&data);
            let writer = loom::thread::spawn(move || {
                vw.begin();
                dw.0.store(10, Ordering::Relaxed);
                dw.1.store(20, Ordering::Relaxed);
                vw.end();
            });

            let s = v.sample();
            let a = data.0.load(Ordering::Relaxed);
            let b = data.1.load(Ordering::Relaxed);
            if v.validate(s) {
                assert!(
                    (a, b) == (1, 2) || (a, b) == (10, 20),
                    "validated read must be consistent"
                );
            }
            writer.join().unwrap();
        });
    }

    /// Hand-over-hand OCC: verifies that when a writer brackets a node mutation
    /// (`SeqVersion::begin`/`end` at root and node version begin/end on the parent node),
    /// a reader narrowing its cover to the parent node version never observes an
    /// inconsistent intermediate slot/value state upon successful validation.
    #[test]
    fn loom_hand_over_hand_node_bracket_safety() {
        loom::model(|| {
            let tree_v = Arc::new(SeqVersion::new());
            let node_v = Arc::new(VersionCell::new(0));
            // Leaf payload: key and value slots
            let key_slot = Arc::new(AtomicU64::new(0xFD));
            let val_slot = Arc::new(AtomicU64::new(100));

            let tv_w = Arc::clone(&tree_v);
            let nv_w = Arc::clone(&node_v);
            let k_w = Arc::clone(&key_slot);
            let v_w = Arc::clone(&val_slot);

            let writer = loom::thread::spawn(move || {
                // The production bracket, called — not re-implemented. This is
                // the whole point of #756: the previous version of this test
                // open-coded `store(Release)` plus a fence on the write side
                // and `fence(Acquire)` plus `load(Acquire)` on the read side,
                // which is neither what the engine does nor sensitive to it
                // losing a fence. Deleting `fence(Ordering::Release)` from
                // `version_begin` or `fence(Ordering::Acquire)` from
                // `node_validate` left the old test green; each deletion fails
                // this one on the assertion below.
                tv_w.begin();
                version_begin(&nv_w);

                // Mutate leaf payload (shift/insert neighbouring key 0xF1 -> 200)
                k_w.store(0xF1, Ordering::Relaxed);
                v_w.store(200, Ordering::Relaxed);

                version_end(&nv_w);
                tv_w.end();
            });

            // Reader: samples tree version, descends to parent node, validates
            let ts = tree_v.sample();
            if let Some(ns) = node_sample(&node_v) {
                // Cover narrowed to parent node: read leaf key and value
                let k = key_slot.load(Ordering::Relaxed);
                let v = val_slot.load(Ordering::Relaxed);
                if node_validate(&node_v, ns) {
                    // Successful validation: must be consistent
                    assert!(
                        (k == 0xFD && v == 100) || (k == 0xF1 && v == 200),
                        "hand-over-hand reader saw inconsistent leaf state: key {k:#x}, val {v}"
                    );
                }
            } else {
                // Cover retained at tree root: read leaf key and value
                let k = key_slot.load(Ordering::Relaxed);
                let v = val_slot.load(Ordering::Relaxed);
                if tree_v.validate(ts) {
                    assert!(
                        (k == 0xFD && v == 100) || (k == 0xF1 && v == 200),
                        "root-covered reader saw inconsistent leaf state: key {k:#x}, val {v}"
                    );
                }
            }
            writer.join().unwrap();
        });
    }

    /// A replaced node must be marked obsolete before its slot changes:
    /// otherwise a reader whose cover is the *old* node validates a leaf
    /// the writer mutates under the *new* node's bracket. The writer here
    /// promotes child C to C′ (copying C's version, as the engine does),
    /// then mutates the shared leaf D under C′. The reader validated the
    /// parent N and loaded the edge to C before the promotion; with
    /// `OBSOLETE` marking its next sample or validate of C fails and it
    /// restarts. Delete the `mark` store and this model finds the torn
    /// read. Models the protocol shape with loom atomics (the production
    /// version word is a plain `u32` until #756).
    #[test]
    fn loom_obsolete_mark_covers_replaced_node() {
        loom::model(|| {
            let n_v = Arc::new(VersionCell::new(0));
            let c_v = Arc::new(VersionCell::new(0));
            let c2_v = Arc::new(VersionCell::new(0));
            // The slot in N: 0 = C, 1 = C′.
            let slot = Arc::new(AtomicU64::new(0));
            let d_key = Arc::new(AtomicU64::new(0xA0));
            let d_val = Arc::new(AtomicU64::new(1));

            let (nw, cw, c2w, sw, kw, vw) = (
                Arc::clone(&n_v),
                Arc::clone(&c_v),
                Arc::clone(&c2_v),
                Arc::clone(&slot),
                Arc::clone(&d_key),
                Arc::clone(&d_val),
            );
            let writer = loom::thread::spawn(move || {
                // Op 1, under N's bracket: promote C → C′. C′ starts from
                // C's version (the header copy the engine makes); C is
                // marked obsolete before N's slot is rewritten and before
                // C is retired — the store under test.
                version_begin(&nw);
                c2w.store(cw.load(Ordering::Relaxed), Ordering::Relaxed);
                version_obsolete(&cw);
                sw.store(1, Ordering::Relaxed);
                version_end(&nw);
                // Op 2: mutate leaf D — which C and C′ both point at —
                // under C′'s bracket (and N's, as the engine nests them).
                version_begin(&nw);
                version_begin(&c2w);
                kw.store(0xB0, Ordering::Relaxed);
                vw.store(2, Ordering::Relaxed);
                version_end(&c2w);
                version_end(&nw);
            });

            // Reader: sample N, load the slot, validate N, move the cover to
            // whichever child the slot named, read D, validate that child.
            if let Some(ns) = node_sample(&n_v) {
                let which = slot.load(Ordering::Relaxed);
                if node_validate(&n_v, ns) {
                    let child = if which == 0 { &c_v } else { &c2_v };
                    if let Some(cs) = node_sample(child) {
                        let k = d_key.load(Ordering::Relaxed);
                        let v = d_val.load(Ordering::Relaxed);
                        if node_validate(child, cs) {
                            assert!(
                                (k == 0xA0 && v == 1) || (k == 0xB0 && v == 2),
                                "validated read through a replaced node is torn: key {k:#x}, val {v}"
                            );
                        }
                    }
                }
            }
            writer.join().unwrap();
        });
    }

    /// S5: Two concurrent writers attempt to acquire a lock on the same node `N` via `try_lock()`.
    /// At no point do both writers enter the critical section simultaneously.
    #[test]
    fn loom_multi_writer_mutual_exclusion() {
        loom::model(|| {
            let node_v = Arc::new(VersionCell::new(0));
            let in_crit = Arc::new(AtomicUsize::new(0));

            let (nv1, ic1) = (Arc::clone(&node_v), Arc::clone(&in_crit));
            let w1 = loom::thread::spawn(move || {
                if let Ok(v) = version_try_lock(&nv1) {
                    let prev = ic1.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(
                        prev, 0,
                        "mutual exclusion violated: concurrent writers inside critical section"
                    );
                    ic1.fetch_sub(1, Ordering::SeqCst);
                    version_unlock(&nv1, v, true);
                }
            });

            let (nv2, ic2) = (Arc::clone(&node_v), Arc::clone(&in_crit));
            let w2 = loom::thread::spawn(move || {
                if let Ok(v) = version_try_lock(&nv2) {
                    let prev = ic2.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(
                        prev, 0,
                        "mutual exclusion violated: concurrent writers inside critical section"
                    );
                    ic2.fetch_sub(1, Ordering::SeqCst);
                    version_unlock(&nv2, v, true);
                }
            });

            w1.join().unwrap();
            w2.join().unwrap();
        });
    }

    /// S8: Writer 1 locks node, stores data into child payload with Relaxed store, unlocks;
    /// Writer 2 locks node, reads payload. Writer 2 observes all stores made by Writer 1.
    #[test]
    fn loom_multi_writer_fence_pairing() {
        loom::model(|| {
            let node_v = Arc::new(VersionCell::new(0));
            let payload = Arc::new(AtomicU64::new(0));

            let (nv1, p1) = (Arc::clone(&node_v), Arc::clone(&payload));
            let w1 = loom::thread::spawn(move || {
                if let Ok(v) = version_try_lock(&nv1) {
                    p1.store(42, Ordering::Relaxed);
                    version_unlock(&nv1, v, true);
                }
            });

            let (nv2, p2) = (Arc::clone(&node_v), Arc::clone(&payload));
            let w2 = loom::thread::spawn(move || {
                if let Ok(v) = version_try_lock(&nv2) {
                    let val = p2.load(Ordering::Relaxed);
                    if v == 2 {
                        assert_eq!(val, 42, "Writer 2 saw stale payload under Acquire lock");
                    }
                    version_unlock(&nv2, v, false);
                }
            });

            w1.join().unwrap();
            w2.join().unwrap();
        });
    }

    /// S7: Two concurrent writers perform disjoint leaf inserts beneath common ancestor `A`,
    /// then execute bottom-up `pop0` bumps under `A`'s node lock. Final `pop0` equals initial `pop0 + 2`.
    ///
    /// NOTE: This model exercises lock-discipline convergence for `pop0` bumps using an `AtomicU64`
    /// with relaxed load/store. In the production engine, `pop0` lives in a plain edge word whose
    /// stores are synchronized by holding the containing node's `NodeLock` (via release-acquire pairing
    /// on node version words).
    #[test]
    fn loom_multi_writer_pop0_convergence() {
        loom::model(|| {
            let node_v = Arc::new(VersionCell::new(0));
            let pop0 = Arc::new(AtomicU64::new(0));

            let (nv1, p1) = (Arc::clone(&node_v), Arc::clone(&pop0));
            let w1 = loom::thread::spawn(move || {
                loop {
                    if let Ok(v) = version_try_lock(&nv1) {
                        let cur = p1.load(Ordering::Relaxed);
                        p1.store(cur + 1, Ordering::Relaxed);
                        version_unlock(&nv1, v, true);
                        break;
                    }
                    loom::thread::yield_now();
                }
            });

            let (nv2, p2) = (Arc::clone(&node_v), Arc::clone(&pop0));
            let w2 = loom::thread::spawn(move || {
                loop {
                    if let Ok(v) = version_try_lock(&nv2) {
                        let cur = p2.load(Ordering::Relaxed);
                        p2.store(cur + 1, Ordering::Relaxed);
                        version_unlock(&nv2, v, true);
                        break;
                    }
                    loom::thread::yield_now();
                }
            });

            w1.join().unwrap();
            w2.join().unwrap();
            assert_eq!(pop0.load(Ordering::Relaxed), 2, "pop0 updates lost");
        });
    }

    /// L5: Two writers execute mutating loops; one coordinator invokes quiescence (`with_locked`).
    /// While coordinator executes in quiescence, zero writer stores are in-flight.
    #[test]
    fn loom_with_locked_quiescence() {
        loom::model(|| {
            let gate = Arc::new(WriterGate::new());
            let w1_inflight = Arc::new(AtomicUsize::new(0));
            let w2_inflight = Arc::new(AtomicUsize::new(0));
            let in_quiescence = Arc::new(AtomicBool::new(false));
            let stores_during_quiescence = Arc::new(AtomicUsize::new(0));

            let (g1, if1, q1, sq1) = (
                Arc::clone(&gate),
                Arc::clone(&w1_inflight),
                Arc::clone(&in_quiescence),
                Arc::clone(&stores_during_quiescence),
            );
            let w1 = loom::thread::spawn(move || {
                for _ in 0..2 {
                    if let Some(_guard) = g1.enter_writer(&if1, 0) {
                        if q1.load(Ordering::Relaxed) {
                            sq1.fetch_add(1, Ordering::SeqCst);
                        }
                        break;
                    }
                    loom::thread::yield_now();
                }
            });

            let (g2, if2, q2, sq2) = (
                Arc::clone(&gate),
                Arc::clone(&w2_inflight),
                Arc::clone(&in_quiescence),
                Arc::clone(&stores_during_quiescence),
            );
            let w2 = loom::thread::spawn(move || {
                for _ in 0..2 {
                    if let Some(_guard) = g2.enter_writer(&if2, 1) {
                        if q2.load(Ordering::Relaxed) {
                            sq2.fetch_add(1, Ordering::SeqCst);
                        }
                        break;
                    }
                    loom::thread::yield_now();
                }
            });

            // Coordinator (with_locked / fallback reader)
            gate.close();
            while w1_inflight.load(Ordering::Relaxed) != 0
                || w2_inflight.load(Ordering::Relaxed) != 0
            {
                loom::thread::yield_now();
            }

            in_quiescence.store(true, Ordering::Relaxed);
            assert_eq!(
                stores_during_quiescence.load(Ordering::SeqCst),
                0,
                "writer mutated during quiescence window"
            );
            in_quiescence.store(false, Ordering::Relaxed);
            gate.open();

            w1.join().unwrap();
            w2.join().unwrap();
        });
    }

    /// A writer stores to data it owns while its guard is held; the
    /// coordinator closes the gate, drains the writer's slot with the
    /// production [`WriterGate::wait_drained`], then reads the data, as a
    /// serialized fallback reads the nodes an optimistic writer just wrote.
    /// The fallback takes no lock the writer released, so the drain is the
    /// only edge that can order the two: loom reports a causality violation
    /// on the cell unless the drain acquires the writer's exit.
    #[test]
    fn loom_quiesce_drain_acquires_writer_exit() {
        loom::model(|| {
            let gate = Arc::new(WriterGate::new());
            let in_flight = Arc::new(AtomicUsize::new(0));
            let data = Arc::new(loom::cell::UnsafeCell::new(0usize));

            let (g, f, d) = (Arc::clone(&gate), Arc::clone(&in_flight), Arc::clone(&data));
            let w = loom::thread::spawn(move || {
                if let Some(_guard) = g.enter_writer(&f, 0) {
                    // SAFETY: loom's cell checks this access; the gate
                    // protocol is what the test puts under that check.
                    d.with_mut(|p| unsafe { *p += 1 });
                }
            });

            gate.close();
            WriterGate::wait_drained(&in_flight);
            // SAFETY: as above.
            let seen = data.with(|p| unsafe { *p });
            assert!(seen <= 1);
            w.join().unwrap();
            gate.open();
        });
    }

    /// Two writers publish through ONE in-flight word, as they do once a
    /// tree's writer table has allocated every slot and hashes later threads
    /// onto taken ones (and as every writer does under loom, where the
    /// per-thread slot cache is a `std::thread_local!` all loom threads
    /// share). Each writes its own cell under its guard; the coordinator
    /// closes, drains the shared word, and reads both. If one writer's exit
    /// can drain the word while the other is still in flight, loom reports a
    /// causality violation on that writer's cell.
    #[test]
    fn loom_shared_slot_quiescence() {
        loom::model(|| {
            let gate = Arc::new(WriterGate::new());
            let in_flight = Arc::new(AtomicUsize::new(0));
            let cells = Arc::new([
                loom::cell::UnsafeCell::new(0usize),
                loom::cell::UnsafeCell::new(0usize),
            ]);

            let writers: Vec<_> = (0..2)
                .map(|i| {
                    let (g, f, c) = (
                        Arc::clone(&gate),
                        Arc::clone(&in_flight),
                        Arc::clone(&cells),
                    );
                    loom::thread::spawn(move || {
                        if let Some(_guard) = g.enter_writer(&f, 0) {
                            // SAFETY: loom's cell checks this access; the
                            // gate protocol is what the test puts under it.
                            c[i].with_mut(|p| unsafe { *p += 1 });
                        }
                    })
                })
                .collect();

            gate.close();
            WriterGate::wait_drained(&in_flight);
            for c in cells.iter() {
                // SAFETY: as above.
                let seen = c.with(|p| unsafe { *p });
                assert!(seen <= 1);
            }
            for w in writers {
                w.join().unwrap();
            }
            assert_eq!(in_flight.load(Ordering::Relaxed), 0);
            gate.open();
        });
    }

    /// S4: Writer 1 unlinks and retires an allocation; Writer 2 advances the epoch;
    /// Reader runs concurrently and pins. If Reader pinned at epoch >= 1 and observed the node while linked,
    /// the retired node must never be reclaimed while Reader remains pinned.
    /// Red when the `SeqCst` fence before the retire-side epoch load is removed.
    #[test]
    fn loom_multi_writer_ebr_safety() {
        loom::model(|| {
            let c = Arc::new(Collector::new());
            let reader = c.register();
            let root = Arc::new(AtomicBool::new(true));

            let (cw1, rw1) = (Arc::clone(&c), Arc::clone(&root));
            let w1 = loom::thread::spawn(move || {
                let layout = Layout::from_size_align(64, 16).unwrap();
                // SAFETY: nonzero test allocation.
                let ptr = NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) }).unwrap();
                rw1.store(false, Ordering::Release);
                cw1.retire(ptr, 64, 16);
            });

            let cw2 = Arc::clone(&c);
            let w2 = loom::thread::spawn(move || {
                cw2.try_advance();
                cw2.try_advance();
            });

            // Reader runs concurrently with w1 and w2:
            let pin = reader.pin();
            let linked = root.load(Ordering::Acquire);
            let pinned_epoch = reader.slot.load(Ordering::Relaxed);

            w1.join().unwrap();
            w2.join().unwrap();

            if linked && pinned_epoch > 0 {
                assert_eq!(
                    c.retained_bytes(),
                    64,
                    "retired block reclaimed while reader remained pinned"
                );
            }

            drop(pin);
            drop(reader);
            c.try_advance();
            c.try_advance();
            c.try_advance();
            assert_eq!(
                c.retained_bytes(),
                0,
                "retired block not reclaimed after reader unpinned"
            );
            c.drain();
        });
    }

    /// Spawns a writer that retires one 64-byte block on `slot`.
    #[cfg(feature = "ablation-striped-epoch")]
    fn spawn_striped_retire(c: &Arc<Collector>, slot: usize) -> loom::thread::JoinHandle<()> {
        let c = Arc::clone(c);
        loom::thread::spawn(move || {
            set_writer_slot(slot);
            let layout = Layout::from_size_align(64, 16).unwrap();
            // SAFETY: non-zero size and a valid power-of-two alignment.
            let ptr = NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) }).unwrap();
            c.retire(ptr, 64, 16);
        })
    }

    /// Two writers retire on stripes 0 and 1 while a third thread advances,
    /// all under a reader pinned before any of them start. The pin allows
    /// one advance at most, so whichever epoch each retire lands in, nothing
    /// is reclaimed until the reader unpins; after it does, every stripe is.
    #[test]
    #[cfg(feature = "ablation-striped-epoch")]
    fn loom_striped_epoch_pin_holds_across_concurrent_advance() {
        loom::model(|| {
            let c = Arc::new(Collector::new());
            let reader = c.register();
            let pin = reader.pin();

            let w1 = spawn_striped_retire(&c, 0);
            let w2 = spawn_striped_retire(&c, 1);
            let ca = Arc::clone(&c);
            let adv = loom::thread::spawn(move || {
                ca.try_advance();
                ca.try_advance();
            });
            w1.join().unwrap();
            w2.join().unwrap();
            adv.join().unwrap();
            assert_eq!(c.retained_bytes(), 128, "reclaimed under a pin");

            drop(pin);
            drop(reader);
            for _ in 0..BINS {
                c.try_advance();
            }
            assert_eq!(c.retained_bytes(), 0);
        });
    }

    /// No reader, so the concurrent thread's two advances both succeed, and
    /// a writer that read epoch `e` can push into bin `e` while the second
    /// advance is draining that bin's stripes. Every retired byte must be
    /// reclaimed exactly once: a block lost from a stripe leaves the count
    /// above zero, and one reclaimed twice wraps it.
    #[test]
    #[cfg(feature = "ablation-striped-epoch")]
    fn loom_striped_epoch_retire_races_drain_of_its_bin() {
        loom::model(|| {
            let c = Arc::new(Collector::new());

            let w1 = spawn_striped_retire(&c, 0);
            let w2 = spawn_striped_retire(&c, 1);
            let ca = Arc::clone(&c);
            let adv = loom::thread::spawn(move || {
                ca.try_advance();
                ca.try_advance();
            });
            w1.join().unwrap();
            w2.join().unwrap();
            adv.join().unwrap();

            // The epoch is at most 2; three more advances drain all three bins.
            for _ in 0..BINS {
                c.try_advance();
            }
            assert_eq!(c.retained_bytes(), 0);
        });
    }
}
