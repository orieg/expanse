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

#[cfg(not(loom))]
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering, fence};
#[cfg(all(not(loom), feature = "std"))]
use core::sync::atomic::{AtomicPtr, AtomicUsize};
#[cfg(all(not(loom), feature = "std"))]
use std::sync::Mutex;

#[cfg(loom)]
use loom::sync::Mutex;
#[cfg(loom)]
use loom::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, AtomicUsize, Ordering, fence};

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
/// using `compare_exchange_weak`.
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
    match v.compare_exchange_weak(cur, cur + 1, Ordering::Relaxed, Ordering::Relaxed) {
        Ok(val) => {
            fence(Ordering::Acquire);
            Ok(val)
        }
        Err(actual) => Err(actual),
    }
}

/// Releases an exclusive write lock on a node's version word.
///
/// Follows the OLC protocol specified in `docs/ARCHITECTURE.md` §4.2:
/// - If `modified` is true, advances the version to `old_v + 2`, invalidating any
///   concurrent optimistic reader that sampled `old_v`.
/// - If `modified` is false, restores `old_v`, preventing spurious reader retries
///   on nodes that were locked during speculative descent or split sizing but left unmodified.
///
/// Executes a release fence prior to storing the updated version to ensure that all
/// node mutations are visible to readers and subsequent writers before the node is unlocked.
#[allow(dead_code)]
#[inline]
pub(crate) fn version_unlock(v: &VersionCell, old_v: u32, modified: bool) {
    debug_assert!(
        v.load(Ordering::Relaxed) % 2 == 1,
        "version_unlock without lock"
    );
    fence(Ordering::Release);
    let next = if modified {
        old_v.wrapping_add(2)
    } else {
        old_v
    };
    v.store(next, Ordering::Relaxed);
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
/// via [`version_unlock`] in LIFO order. If modified, the node's version advances
/// to `old_v + 2` (invalidating concurrent optimistic readers); if unmodified,
/// it restores `old_v` (preventing spurious reader retries).
///
/// # Invariant & Safety
///
/// `NodeLock` strictly dereferences to raw `*mut N`, NEVER `&mut N`, ensuring
/// that Stacked Borrows and Tree Borrows invariants are preserved while concurrent
/// readers sample and load fields of `N`.
#[allow(dead_code)]
pub(crate) struct NodeLock<'a, N> {
    ptr: *mut N,
    version: &'a VersionCell,
    old_v: u32,
    modified: core::cell::Cell<bool>,
}

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
            modified: core::cell::Cell::new(false),
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
            modified: core::cell::Cell::new(false),
        }
    }

    /// Marks the node as modified under this lock.
    #[inline(always)]
    pub(crate) fn mark_modified(&self) {
        self.modified.set(true);
    }

    /// Returns whether the node was marked modified.
    #[inline(always)]
    pub(crate) fn is_modified(&self) -> bool {
        self.modified.get()
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

impl<N> core::ops::Deref for NodeLock<'_, N> {
    type Target = *mut N;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.ptr
    }
}

impl<N> Drop for NodeLock<'_, N> {
    #[inline]
    fn drop(&mut self) {
        version_unlock(self.version, self.old_v, self.modified.get());
    }
}

/// Coordinates writer quiescence for reader fallback and exclusive operations (`with_locked`).
///
/// Follows the Dekker-style fence pairing protocol specified in `docs/ARCHITECTURE.md` §4.2:
/// - Writers publish an in-flight status flag, execute `fence(Ordering::SeqCst)`, and re-check
///   whether `WriterGate` is closed.
/// - Quiescence coordinators (reader fallback or `with_locked`) close `WriterGate`, execute
///   `fence(Ordering::SeqCst)`, and wait for all registered in-flight writer slots to drain to 0.
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct WriterGate {
    closed: AtomicBool,
}

#[allow(dead_code)]
impl WriterGate {
    /// Creates a fresh, open gate.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            closed: AtomicBool::new(false),
        }
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

    /// Writer op entry: publishes in-flight status, executes `fence(Ordering::SeqCst)`,
    /// and verifies that the gate is not closed.
    ///
    /// Returns `true` if entry succeeded, or `false` if the gate is closed and the writer
    /// must back off.
    #[cfg(feature = "std")]
    #[inline]
    pub(crate) fn enter_writer(&self, in_flight: &AtomicUsize) -> bool {
        in_flight.store(1, Ordering::Relaxed);
        fence(Ordering::SeqCst);
        if self.is_closed() {
            in_flight.store(0, Ordering::Relaxed);
            false
        } else {
            true
        }
    }

    /// Writer op exit: clears the in-flight status.
    #[cfg(feature = "std")]
    #[inline]
    pub(crate) fn exit_writer(in_flight: &AtomicUsize) {
        in_flight.store(0, Ordering::Release);
    }
}

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
pub unsafe trait OlcEngine: Send {}

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
}

// SAFETY: a retired allocation is exclusively owned by the collector —
// no live reference remains once its grace period elapses.
#[cfg(feature = "std")]
unsafe impl Send for Garbage {}

#[cfg(feature = "std")]
use crate::alloc::{CLASS_SPECS, FreeBlock, NUM_CLASSES, class_for};

/// Epoch-based reclamation for one tree: readers pin the current epoch
/// around each walk; retired allocations wait two epoch advances before
/// they are freed, so a pinned reader can never observe freed memory.
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct Collector {
    epoch: AtomicUsize,
    readers: Mutex<Vec<Arc<Slot>>>,
    bins: [Mutex<Vec<Garbage>>; BINS],
    freelists: [AtomicPtr<FreeBlock>; NUM_CLASSES],
    retained_bytes: AtomicUsize,
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
            epoch: AtomicUsize::new(0),
            readers: Mutex::new(Vec::new()),
            bins: [
                Mutex::new(Vec::new()),
                Mutex::new(Vec::new()),
                Mutex::new(Vec::new()),
            ],
            freelists: core::array::from_fn(|_| AtomicPtr::new(core::ptr::null_mut())),
            retained_bytes: AtomicUsize::new(0),
            #[cfg(test)]
            registrations: core::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Pops a reclaimed block from this collector's size-class freelist.
    #[inline(always)]
    pub(crate) fn pop_freelist(&self, class: usize) -> *mut u8 {
        loop {
            let head = self.freelists[class].load(Ordering::Acquire);
            if head.is_null() {
                return core::ptr::null_mut();
            }
            // SAFETY: head is a valid FreeBlock in this size class.
            let next = unsafe { (*head).next };
            if self.freelists[class]
                .compare_exchange_weak(head, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                let bytes = CLASS_SPECS[class].0;
                // SAFETY: zero out the reused memory before returning.
                unsafe { core::ptr::write_bytes(head.cast::<u8>(), 0, bytes) };
                return head.cast::<u8>();
            }
        }
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
        // SeqCst fence pairs with `try_advance` to establish happens-before with
        // concurrent advancers under multi-writer OLC (Stage B, S4).
        fence(Ordering::SeqCst);
        let e = self.epoch.load(Ordering::Relaxed);
        self.bins[e % BINS]
            .lock()
            .expect("garbage bin poisoned")
            .push(Garbage { ptr, bytes, align });
        self.retained_bytes.fetch_add(bytes, Ordering::Relaxed);
        crate::occ_stats::record_retire(bytes);
    }

    /// Attempts one epoch advance: succeeds when every pinned reader has
    /// caught up to the current epoch, then frees the bin two epochs
    /// back. Writer-side, amortized (call once per mutation batch).
    pub fn try_advance(&self) {
        crate::occ_stats::bump(crate::occ_stats::Stat::AdvanceCalls);
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
        crate::occ_stats::bump(crate::occ_stats::Stat::AdvanceOk);
        self.epoch.store(e + 1, Ordering::Release);
        // Everything retired at epoch e - 1 predates every possible pin
        // in epochs e and e + 1: no live reader can hold it.
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
                loop {
                    let head = self.freelists[class].load(Ordering::Relaxed);
                    // SAFETY: block points to a valid allocation of at least size_of::<FreeBlock>().
                    unsafe { (*block).next = head };
                    if self.freelists[class]
                        .compare_exchange_weak(head, block, Ordering::Release, Ordering::Relaxed)
                        .is_ok()
                    {
                        break;
                    }
                }
            } else {
                free_raw(g.ptr, g.bytes, g.align);
            }
        }
        self.retained_bytes
            .fetch_sub(freed_bytes, Ordering::Relaxed);
        crate::occ_stats::record_reclaim(freed_bytes);
    }

    /// Total bytes currently queued across this collector's garbage bins.
    ///
    /// Measures unreclaimed garbage backlog queued in epoch bins awaiting
    /// reclamation, not total allocated heap footprint (reclaimed node
    /// blocks transition to collector size-class freelists for reuse).
    #[must_use]
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes.load(Ordering::Relaxed)
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

    /// Frees everything still queued. Only sound once no reader can be
    /// pinned (the owning wrapper calls this on drop, when exclusive
    /// ownership proves that).
    pub(crate) fn drain(&self) {
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
}

#[cfg(feature = "std")]
impl Drop for Collector {
    fn drop(&mut self) {
        // Last owner: no readers remain by definition.
        self.drain();
        for (class, &(bytes, align)) in CLASS_SPECS.iter().enumerate() {
            let mut cur = self.freelists[class].load(Ordering::Relaxed);
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
    collector: Arc<Collector>,
    slot: Arc<Slot>,
}

#[cfg(feature = "std")]
impl Reader {
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
            assert!(!lock.is_modified());
            lock.mark_modified();
            assert!(lock.is_modified());
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
            // Dropped without mark_modified: restores old_v = 4
        }
        assert_eq!(cell.load(Ordering::Relaxed), 4);
    }

    #[test]
    fn writer_gate_protocol() {
        let gate = WriterGate::new();
        let in_flight = AtomicUsize::new(0);
        assert!(!gate.is_closed());

        // Normal entry when open
        assert!(gate.enter_writer(&in_flight));
        assert_eq!(in_flight.load(Ordering::Relaxed), 1);
        WriterGate::exit_writer(&in_flight);
        assert_eq!(in_flight.load(Ordering::Relaxed), 0);

        // Quiescence closure
        gate.close();
        assert!(gate.is_closed());
        // Entry fails when closed
        assert!(!gate.enter_writer(&in_flight));
        assert_eq!(in_flight.load(Ordering::Relaxed), 0);

        gate.open();
        assert!(!gate.is_closed());
        assert!(gate.enter_writer(&in_flight));
        WriterGate::exit_writer(&in_flight);
    }
}

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
                    if g1.enter_writer(&if1) {
                        if q1.load(Ordering::Relaxed) {
                            sq1.fetch_add(1, Ordering::SeqCst);
                        }
                        WriterGate::exit_writer(&if1);
                        break;
                    }
                    loom::thread::yield_now();
                }
            });

            // Coordinator (with_locked / fallback reader)
            gate.close();
            while w1_inflight.load(Ordering::Relaxed) != 0 {
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
        });
    }

    /// S4: Writer 1 retires an allocation; Writer 2 advances the epoch; Reader remains pinned at epoch 0.
    /// The epoch cannot advance past Reader's pin (at most to epoch 1), so retired blocks remain safe.
    #[test]
    fn loom_multi_writer_ebr_safety() {
        loom::model(|| {
            let c = Arc::new(Collector::new());
            let reader = c.register();
            let pin = reader.pin();

            let cw1 = Arc::clone(&c);
            let w1 = loom::thread::spawn(move || {
                let layout = Layout::from_size_align(64, 16).unwrap();
                // SAFETY: nonzero test allocation.
                let ptr = NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) }).unwrap();
                cw1.retire(ptr, 64, 16);
            });

            let cw2 = Arc::clone(&c);
            let w2 = loom::thread::spawn(move || {
                cw2.try_advance();
            });

            w1.join().unwrap();
            w2.join().unwrap();

            let now = c.epoch.load(Ordering::SeqCst);
            assert!(now <= 1, "epoch advanced twice past a live pin: now {now}");
            assert!(
                c.retained_bytes() == 64,
                "retired block reclaimed while reader remained pinned"
            );

            drop(pin);
            drop(reader);
            c.drain();
        });
    }
}
