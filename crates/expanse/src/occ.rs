//! Phase 7: optimistic concurrency primitives — the seqlock version word
//! readers validate against, and epoch-based reclamation (EBR) so a
//! reader never dereferences a freed node.
//!
//! The concurrent wrappers (`SyncExpanseSet`/`SyncExpanseMap` in `sync`)
//! combine a **tree-level** [`SeqVersion`] (heading the wrapper's `Shared`
//! block; readers validate their root snapshot against it) with **per-node** versions in
//! the branch headers. The mutation engine brackets every store by the
//! version of the node that *contains* the stored address — the parent's
//! word for a slot, an immediate, or a leaf / subarray payload; the tree
//! word for the root state — through [`Cover`] (active only for
//! concurrently shared trees). Where the engine covers the root (the set
//! and map wrappers, `NodeAlloc::cover_root`), a branch child's frame is
//! entered with its parent's word closed, so a word is odd only while one
//! frame stores into that node, never for a whole descent; a covered write
//! there holds the tree word for its whole operation and the engine's own
//! tree bracket stands down (`NodeAlloc::hold_tree_word`), but the per-node
//! brackets stay brief. The string, bytes and blob wrappers never hand the
//! root to the engine. Their serialised paths (fallbacks, the operations
//! they serialise, and every mutation under their `ablation-*-serial-writers`
//! features) hold the tree word for the whole operation and run the
//! engine's nested mode (`Cover::nest_begin`), which keeps a node's word odd
//! across the descent beneath it: `by_mode!` selects it for the bytes and
//! blob index tries, the string map passes it explicitly for each `StrNode`
//! sub-map, which it also brackets with that node's cover word. Their
//! optimistic writers never store to the tree word and never enter nested
//! mode: they run the engine's OLC bodies under per-node version locks, the
//! string wrapper taking each `StrNode`'s cover word as the lock on that
//! node's root state (`docs/ARCHITECTURE.md` §4.1–§4.2). Readers validate
//! hand-over-hand with `node_sample`/`node_validate`. Measured motivation
//! and effect in `docs/BENCHMARKING.md` (concurrent read scaling) and
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
/// the engine (not the wrapper) opens the tree word for this tree now
/// (`NodeAlloc::engine_opens_tree_word`). The map and set wrappers hand root
/// coverage to the engine, and hold the word themselves around a whole
/// covered write (#1086); the string, bytes and blob wrappers keep
/// bracketing whole operations in `Shared::write`. In both held cases this
/// is a no-op, so the word is never opened twice.
#[inline(always)]
pub(crate) fn tree_begin_if<const OCC: bool>(a: &crate::alloc::NodeAlloc) {
    #[cfg(feature = "std")]
    if OCC && a.engine_opens_tree_word() {
        a.tree_version().begin();
    }
    #[cfg(all(debug_assertions, feature = "std"))]
    if OCC && a.engine_opens_tree_word() {
        a.bracket_enter(a.tree_cover_addr());
    }
    let _ = a;
}

/// Closes the tree-level bracket opened by [`tree_begin_if`].
#[inline(always)]
pub(crate) fn tree_end_if<const OCC: bool>(a: &crate::alloc::NodeAlloc) {
    #[cfg(all(debug_assertions, feature = "std"))]
    if OCC && a.engine_opens_tree_word() {
        a.bracket_leave(a.tree_cover_addr());
    }
    #[cfg(feature = "std")]
    if OCC && a.engine_opens_tree_word() {
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
/// The CAS is an acquire, so the node's reads and writes that follow it are
/// not reordered before the acquisition (S8), and a successful one is
/// followed by a **release fence**, so the odd word is visible before any
/// store the holder makes under it — [`version_begin`]'s construction, and
/// the half the reader protocol needs: a reader that loaded a store made
/// under the lock and then re-reads the word with an acquire fence
/// synchronises with this fence and sees the word odd, so it restarts
/// instead of returning the torn read (S1, S12).
/// `loom_str_suffix_value_read_validates_the_node_cover` and
/// `loom_str_prune_locks_the_child_before_unlinking_it` are red without
/// it: an acquire CAS orders nothing for a thread that never touches the
/// word between the lock and the store it reads.
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
    let old = v.compare_exchange(cur, cur + 1, Ordering::Acquire, Ordering::Relaxed)?;
    fence(Ordering::Release);
    Ok(old)
}

/// Attempts to acquire an exclusive write lock on a node's version word, verifying
/// that the current version matches `expected` (the OLC snapshot taken during descent).
///
/// This implements the canonical `lockVersionOrRestart` primitive (Leis et al., DaMoN 2016).
/// It succeeds ONLY if `v == expected`. If another writer modified or locked the node in the
/// meantime, the CAS fails, protecting against concurrent shifts and subarray reallocations.
///
/// Returns `Ok(expected)` on success, or `Err(current_version)` if the version changed,
/// was odd, obsolete, or if CAS failed. The release fence after a successful
/// CAS is [`version_try_lock`]'s, for the same reason.
#[cfg_attr(not(feature = "std"), allow(dead_code))]
#[inline]
pub(crate) fn version_try_lock_expect(v: &VersionCell, expected: u32) -> Result<u32, u32> {
    if !expected.is_multiple_of(2) || (expected & OBSOLETE != 0) {
        return Err(expected);
    }
    let old = v.compare_exchange(expected, expected + 1, Ordering::Acquire, Ordering::Relaxed)?;
    fence(Ordering::Release);
    Ok(old)
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

/// One slot of a tree's writer table: the words a writer on that slot
/// stores to on every optimistic operation, so they share its line.
#[cfg(feature = "std")]
#[derive(Debug)]
pub(crate) struct WriterSlot {
    /// Optimistic mutations this slot's writer has left before its next
    /// epoch-advance attempt on the tree's collector
    /// ([`WriterGuard::tick_advance`]), in `0..ADVANCE_EVERY`.
    ///
    /// It is per tree because a slot is: a thread's ticks on one tree never
    /// count towards another's advance, which a thread-local count shared
    /// by every collector the thread wrote to did (Refs #1314). The slot's
    /// writer loads and stores it without a read-modify-write, and every
    /// value stored is in range, so two writers hashed onto one slot can
    /// lose a tick to each other, which only delays an advance, but can
    /// never leave the count outside the interval.
    // Kept under `advance-never`, where nothing ticks, so the slot's layout
    // is the same across variants (as `Shared::advance_tick` is).
    #[cfg_attr(feature = "advance-never", allow(dead_code))]
    advance_countdown: AtomicUsize,
    /// Writers inside the gate on this slot ([`WriterGate::enter_writer`]).
    pub(crate) in_flight: AtomicUsize,
}

/// The countdown a fresh writer slot starts from: a full interval, so a
/// writer's first advance attempt comes on its `ADVANCE_EVERY`th optimistic
/// mutation, as the serialised path's does.
#[cfg(all(feature = "std", not(loom), not(feature = "advance-never")))]
const FRESH_COUNTDOWN: usize = crate::sync::ADVANCE_EVERY as usize - 1;
/// Under loom a model runs far fewer mutations than an interval, so a fresh
/// slot starts at zero: each writer's first optimistic mutation attempts an
/// advance, and every model reaches `try_advance` from the write path.
#[cfg(all(feature = "std", any(loom, feature = "advance-never")))]
const FRESH_COUNTDOWN: usize = 0;

#[cfg(feature = "std")]
impl WriterSlot {
    /// An empty slot: no writer in flight, and a full advance interval
    /// ahead of it (`FRESH_COUNTDOWN`).
    pub(crate) fn new() -> Self {
        Self {
            advance_countdown: AtomicUsize::new(FRESH_COUNTDOWN),
            in_flight: AtomicUsize::new(0),
        }
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
    slot: &'a WriterSlot,
    slot_id: usize,
}

#[cfg(feature = "std")]
#[allow(dead_code)]
impl<'a> WriterGuard<'a> {
    #[inline(always)]
    pub(crate) fn slot_id(&self) -> usize {
        self.slot_id
    }

    /// Records one optimistic mutation by this writer, and attempts an
    /// epoch advance on `collector` once every `ADVANCE_EVERY` such
    /// mutations by this writer.
    ///
    /// `collector` must be the collector of the tree whose gate this guard
    /// entered. The count lives in the guard's writer-table slot, so each
    /// collector advances on its own writers' mutations and never on a
    /// mutation of another tree the same thread wrote. A count per thread
    /// shared across collectors gave every advance to whichever collector
    /// made the crossing call, which starved one of two wrappers written
    /// alternately (Refs #1314). The slot is a line the writer already
    /// stores to on entering the gate, so the count adds no thread-local
    /// read and no shared read-modify-write; loom models run this same code.
    ///
    /// The collector is taken behind its `Arc` and dereferenced only on the
    /// advancing tick: taken as `&Collector`, the caller's dereference was
    /// hoisted above the branch and paid on every tick (x86-64 disassembly
    /// of `SyncExpanseMap::insert`).
    #[inline(always)]
    #[cfg_attr(feature = "advance-never", allow(unused_variables))]
    pub(crate) fn tick_advance(&self, collector: &Arc<Collector>) {
        #[cfg(not(feature = "advance-never"))]
        {
            let countdown = &self.slot.advance_countdown;
            let left = countdown.load(Ordering::Relaxed);
            if left == 0 {
                countdown.store(crate::sync::ADVANCE_EVERY as usize - 1, Ordering::Relaxed);
                collector.try_advance();
            } else {
                countdown.store(left - 1, Ordering::Relaxed);
            }
        }
    }
}

#[cfg(feature = "std")]
#[allow(dead_code)]
impl<'a> Drop for WriterGuard<'a> {
    #[inline]
    fn drop(&mut self) {
        self.gate.exit_writer(&self.slot.in_flight);
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
    /// The slot's `in_flight` counts the writers inside the slot rather than
    /// flagging one: a tree's writer table hashes threads onto taken slots
    /// once all `MAX_WRITER_SLOTS` are allocated, so two live writers can
    /// share a word, and a store of 0 by either would drain it under the
    /// other. The Dekker pairing with [`Self::close`] stays fence-to-fence, the
    /// form loom models: it treats `SeqCst` accesses as `AcqRel` and
    /// supports only `fence(SeqCst)`. The re-check itself acquires the
    /// [`Self::open`] that let this writer in, which orders a locked
    /// fallback's reads before the writer's stores
    /// (`loom_writer_entry_acquires_gate_reopen`).
    ///
    /// Returns `Some(WriterGuard)` if entry succeeded, or `None` if the gate is closed.
    #[inline]
    pub(crate) fn enter_writer<'a>(
        &'a self,
        slot: &'a WriterSlot,
        slot_id: usize,
    ) -> Option<WriterGuard<'a>> {
        if self.is_closed() {
            return None;
        }
        let in_flight = &slot.in_flight;
        in_flight.fetch_add(1, Ordering::Relaxed);
        fence(Ordering::SeqCst);
        // Acquire, pairing with the `Release` in [`Self::open`] (#1295): when
        // this re-check reads the reopen, everything the quiescent section
        // read happens-before this writer's stores. The fence above cannot
        // supply that edge, since it precedes the load, and the first check
        // may have read the gate's state from before the close.
        if self.closed.load(Ordering::Acquire) {
            in_flight.fetch_sub(1, Ordering::Relaxed);
            None
        } else {
            Some(WriterGuard {
                gate: self,
                slot,
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
pub(crate) const BINS: usize = 3;

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
    align: usize,
}

// SAFETY: a retired allocation is exclusively owned by the collector —
// no live reference remains once its grace period elapses.
#[cfg(feature = "std")]
unsafe impl Send for Garbage {}

#[cfg(feature = "std")]
use crate::alloc::{CLASS_SPECS, FreeBlock, NUM_CLASSES, class_for};

/// One size-class freelist of one stripe: the list head and, with
/// `collector-census`, the list's cumulative counters. Both are read and
/// written only under the list's mutex. Without the feature the second field
/// does not exist, so the type is one pointer, as it always was (asserted
/// below).
#[cfg(feature = "std")]
#[derive(Debug)]
struct FreeListHead(
    *mut FreeBlock,
    #[cfg(feature = "collector-census")] FreeListCounts,
);

#[cfg(feature = "std")]
impl FreeListHead {
    /// An empty list.
    #[cfg(not(feature = "collector-census"))]
    const fn empty() -> Self {
        Self(core::ptr::null_mut())
    }

    /// An empty list with zeroed counters.
    #[cfg(feature = "collector-census")]
    const fn empty() -> Self {
        Self(core::ptr::null_mut(), FreeListCounts::ZERO)
    }
}

#[cfg(feature = "std")]
// SAFETY: Access to the raw pointer in FreeListHead is synchronized by a Mutex.
unsafe impl Send for FreeListHead {}

/// What one epoch-bin stripe holds under its mutex: the retired blocks and,
/// with `collector-census`, the stripe's cumulative counters. Without the
/// feature it is the `Vec` alone, with the `Vec`'s layout (asserted below).
#[cfg(feature = "std")]
#[derive(Debug, Default)]
struct BinList {
    list: Vec<Garbage>,
    #[cfg(feature = "collector-census")]
    counts: BinCounts,
}

// G0 of #1310: with `collector-census` off, the two types the feature extends
// keep the layout of the types they wrap, so every field of `Collector` keeps
// its size and offset and `Collector` its size (`#[repr(C)]`, no other field
// changes with the feature).
#[cfg(all(feature = "std", not(feature = "collector-census")))]
const _: () = {
    assert!(core::mem::size_of::<FreeListHead>() == core::mem::size_of::<*mut FreeBlock>());
    assert!(core::mem::align_of::<FreeListHead>() == core::mem::align_of::<*mut FreeBlock>());
    assert!(core::mem::size_of::<BinList>() == core::mem::size_of::<Vec<Garbage>>());
    assert!(core::mem::align_of::<BinList>() == core::mem::align_of::<Vec<Garbage>>());
};

/// A freelist's cumulative counters (feature `collector-census`). Every field
/// is written under the list's mutex, which the path that moves the block
/// already holds.
#[cfg(feature = "collector-census")]
#[derive(Debug, Clone, Copy)]
struct FreeListCounts {
    /// Blocks on the list now. Kept so a release can count what it detaches
    /// while it holds the lock; the walk that frees them runs after the lock
    /// is dropped.
    len: u64,
    /// Blocks pushed by an epoch advance at the end of their grace period.
    reclaimed: u64,
    /// Blocks pushed by `recycle_unpublished` (never published, no grace).
    recycled: u64,
    /// Blocks popped by an allocation.
    reused: u64,
    /// Blocks released to the allocator by `release_free_lists`.
    released: u64,
}

#[cfg(feature = "collector-census")]
impl FreeListCounts {
    const ZERO: Self = Self {
        len: 0,
        reclaimed: 0,
        recycled: 0,
        reused: 0,
        released: 0,
    };
}

/// One epoch-bin stripe's cumulative counters (feature `collector-census`),
/// written under the stripe's mutex.
#[cfg(feature = "collector-census")]
#[derive(Debug)]
struct BinCounts {
    /// Blocks retired into this stripe, per size class.
    retired: [u64; NUM_CLASSES],
    /// Retired blocks no size class serves.
    unclassed: UnclassedCounters,
}

#[cfg(feature = "collector-census")]
impl Default for BinCounts {
    fn default() -> Self {
        Self {
            retired: [0; NUM_CLASSES],
            unclassed: UnclassedCounters::default(),
        }
    }
}

/// Blocks and bytes, summed.
#[cfg(feature = "std")]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlockTally {
    /// Number of blocks.
    pub blocks: usize,
    /// Their bytes, each block at the size it was allocated with.
    pub bytes: usize,
}

#[cfg(feature = "std")]
impl BlockTally {
    fn add(&mut self, blocks: usize, bytes: usize) {
        self.blocks += blocks;
        self.bytes += bytes;
    }
}

/// One size class's share of a [`CollectorCensus`].
#[cfg(feature = "std")]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClassCensus {
    /// The class's block size in bytes.
    pub block_bytes: usize,
    /// The class's block alignment. Two classes can share a size and differ
    /// in alignment.
    pub align: usize,
    /// Blocks past their grace period on the collector's freelists, summed
    /// over every stripe: reusable by the tree now, and what
    /// `shrink_to_fit` releases.
    pub free: BlockTally,
    /// Retired blocks of this class still in their grace period, in the
    /// epoch bins. Neither reusable nor releasable until an epoch advance
    /// moves them to a freelist.
    pub grace: BlockTally,
}

/// Where an epoch collector's bytes sit, by size class and by freelist
/// stripe: a snapshot taken by [`Collector::census`].
///
/// Each list and bin is walked under its own lock, one at a time, so the
/// snapshot is exact for a quiesced collector (no writer running) and
/// otherwise a sum of per-list snapshots taken at slightly different times.
#[cfg(feature = "std")]
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollectorCensus {
    /// One entry per node size class, in the allocator's class order.
    pub classes: Vec<ClassCensus>,
    /// Retired blocks in their grace period that no size class serves (for
    /// example string-map suffix leaves). They never reach a freelist: the
    /// advance that ends their grace period returns them to the allocator.
    pub unclassed_grace: BlockTally,
    /// Freelist blocks per writer stripe, every class summed. A reclaimed
    /// block goes to the freelist of the stripe that retired it and is
    /// reused only by allocations from that stripe, so blocks accumulating
    /// on a stripe whose writer has stopped show here. One entry per stripe,
    /// or a single entry under `ablation-unstriped-freelist`.
    pub stripes: Vec<BlockTally>,
}

#[cfg(feature = "std")]
impl CollectorCensus {
    /// Bytes on the freelists, every class and stripe.
    #[must_use]
    pub fn free_bytes(&self) -> usize {
        self.classes.iter().map(|c| c.free.bytes).sum()
    }

    /// Bytes still in their grace period: every class plus the unclassed
    /// bucket.
    #[must_use]
    pub fn grace_bytes(&self) -> usize {
        self.classes.iter().map(|c| c.grace.bytes).sum::<usize>() + self.unclassed_grace.bytes
    }

    /// `free_bytes() + grace_bytes()`: everything the collector holds.
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.free_bytes() + self.grace_bytes()
    }
}

/// One size class's cumulative counters (feature `collector-census`), as
/// [`Collector::counters`] sums them over every bin and stripe.
///
/// On a collector no writer is using, and before it drains, the counters
/// and a [`CollectorCensus`] of the same class agree:
/// `free.blocks == reclaimed + recycled - reused - released` and
/// `grace.blocks == retired - reclaimed`.
#[cfg(feature = "collector-census")]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClassCounters {
    /// The class's block size in bytes.
    pub block_bytes: usize,
    /// The class's block alignment.
    pub align: usize,
    /// Blocks retired into the epoch bins.
    pub retired: u64,
    /// Blocks moved from the bins to a freelist at the end of their grace
    /// period.
    pub reclaimed: u64,
    /// Blocks put on a freelist without a grace period: allocations the tree
    /// made and abandoned before publishing them.
    pub recycled: u64,
    /// Blocks an allocation took from a freelist instead of the system
    /// allocator.
    pub reused: u64,
    /// Freelist blocks returned to the allocator by `shrink_to_fit`.
    pub released: u64,
}

/// Cumulative counters for retired blocks no size class serves (feature
/// `collector-census`).
///
/// Before the collector drains,
/// `retired_blocks - released_blocks` equals the census's
/// `unclassed_grace.blocks` on a collector no writer is using.
#[cfg(feature = "collector-census")]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnclassedCounters {
    /// Blocks retired into the epoch bins.
    pub retired_blocks: u64,
    /// Their bytes.
    pub retired_bytes: u64,
    /// Blocks returned to the allocator by the epoch advance that ended
    /// their grace period. Counted when the advance takes them from their
    /// bin, under the bin's lock; the free follows once the lock is dropped.
    pub released_blocks: u64,
    /// Their bytes.
    pub released_bytes: u64,
}

/// An epoch collector's cumulative block counters (feature
/// `collector-census`): a snapshot taken by [`Collector::counters`].
///
/// Each counter is a plain integer updated inside a critical section the
/// path already holds (a bin's or a freelist's mutex), so the feature adds no
/// atomic and no thread-local read; it does add work under those locks, and
/// grows the collector.
#[cfg(feature = "collector-census")]
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollectorCounters {
    /// One entry per node size class, in the allocator's class order (the
    /// order of [`CollectorCensus::classes`]).
    pub classes: Vec<ClassCounters>,
    /// Retired blocks no size class serves.
    pub unclassed: UnclassedCounters,
}

#[cfg(feature = "collector-census")]
impl CollectorCounters {
    /// Bytes `shrink_to_fit` has returned to the allocator, every class.
    #[must_use]
    pub fn released_bytes(&self) -> u64 {
        self.classes
            .iter()
            .map(|c| c.released * c.block_bytes as u64)
            .sum()
    }

    /// Blocks reused from a freelist, every class.
    #[must_use]
    pub fn reused_blocks(&self) -> u64 {
        self.classes.iter().map(|c| c.reused).sum()
    }
}

#[cfg(not(feature = "ablation-unpadded-lock"))]
#[derive(Debug)]
#[repr(align(64))]
#[allow(dead_code)]
pub(crate) struct Line<X>(pub(crate) X);

#[cfg(not(feature = "ablation-unpadded-lock"))]
impl<X> core::ops::Deref for Line<X> {
    type Target = X;
    fn deref(&self) -> &X {
        &self.0
    }
}

#[cfg(not(feature = "ablation-unpadded-lock"))]
impl<X> From<X> for Line<X> {
    fn from(x: X) -> Self {
        Self(x)
    }
}

#[cfg(feature = "ablation-unpadded-lock")]
#[allow(dead_code)]
pub(crate) type Line<X> = X;

#[cfg(not(feature = "ablation-unpadded-lock"))]
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

/// Number of striped epoch garbage bins and retained-bytes counters.
///
/// Decoupled from `MAX_WRITER_SLOTS` (64) to bound the per-tree memory
/// overhead of `Collector` to 4.0 KiB (3.0 KiB bins + 1.0 KiB retained bytes),
/// a 4x reduction vs the naive 64-stripe layout (Refs #568).
///
/// Under Loom, 2 stripes are used to model concurrent striping without
/// state-space explosion.
#[cfg(all(feature = "std", not(loom)))]
pub(crate) const NUM_EPOCH_STRIPES: usize = 16;
#[cfg(all(feature = "std", loom))]
pub(crate) const NUM_EPOCH_STRIPES: usize = 2;

/// Number of striped size-class freelists in standard production architecture (Refs #568).
///
/// Decoupled from `MAX_WRITER_SLOTS` (64) to bound the per-wrapper memory footprint
/// to 16.0 KiB on Linux (24.0 KiB on macOS) at S=16.
#[cfg(all(feature = "std", not(loom)))]
pub(crate) const NUM_FREELIST_STRIPES: usize = 16;
#[cfg(all(feature = "std", loom))]
pub(crate) const NUM_FREELIST_STRIPES: usize = 2;

// The loop-stripe shortcut in `try_advance` routes reclaimed blocks to the
// freelist of the epoch stripe draining them. That is sound iff every block in
// `bins[e][stripe]` was placed there by a writer whose freelist stripe equals
// that same `stripe`. This holds iff NUM_FREELIST_STRIPES == NUM_EPOCH_STRIPES.
#[cfg(feature = "std")]
const _: () = assert!(
    NUM_FREELIST_STRIPES == NUM_EPOCH_STRIPES,
    "loop-stripe shortcut requires NUM_FREELIST_STRIPES == NUM_EPOCH_STRIPES"
);

/// One writer stripe of one epoch bin.
#[cfg(feature = "std")]
#[derive(Debug)]
#[repr(align(64))]
pub(crate) struct PaddedBin {
    garbage: Mutex<BinList>,
    /// Set when garbage is pushed and cleared when it is taken, both under
    /// `garbage`'s lock; read without it. It lets an advance skip empty
    /// stripes with a load instead of a lock, since an advance runs on the
    /// write path. It is a hint: a stale read only delays a stripe to the
    /// bin's next drain, and nothing is freed on its strength.
    nonempty: AtomicBool,
}

#[cfg(feature = "std")]
impl PaddedBin {
    fn new() -> Self {
        Self {
            garbage: Mutex::new(BinList::default()),
            nonempty: AtomicBool::new(false),
        }
    }

    /// Takes everything queued in this stripe. `DRAIN` says which path takes
    /// it, for the `collector-census` counters only: an epoch advance, which
    /// frees the blocks no size class serves and counts them as released, or
    /// `drain`, which frees every block and counts nothing (no caller can read
    /// a drained collector's counters). Without the feature the parameter is
    /// unused.
    fn take<const DRAIN: bool>(&self) -> Vec<Garbage> {
        let mut guard = self.garbage.lock().expect("garbage bin poisoned");
        self.nonempty.store(false, Ordering::Relaxed);
        let taken = core::mem::take(&mut guard.list);
        #[cfg(feature = "collector-census")]
        if !DRAIN {
            let counts = &mut guard.counts.unclassed;
            for g in &taken {
                // An advance reclaims a classed block to a freelist, which
                // counts it under its own lock.
                if class_for(g.bytes, g.align).is_none() {
                    counts.released_blocks += 1;
                    counts.released_bytes += g.bytes as u64;
                }
            }
        }
        taken
    }
}

#[cfg(feature = "std")]
#[derive(Debug)]
#[repr(align(64))]
pub(crate) struct PaddedRetained(pub(crate) AtomicUsize);

/// One writer stripe's size-class freelists (Refs #568).
/// A stripe's classes share lines only with each other, never with another
/// stripe's.
#[cfg(all(feature = "std", not(feature = "ablation-unstriped-freelist")))]
#[derive(Debug)]
#[repr(align(64))]
struct PaddedFreelists([Mutex<FreeListHead>; NUM_CLASSES]);

// Thread-exit slot recycling tracking mask (Refs #568).
// When a thread calls `writer_slot()`, it claims the lowest free bit in this mask
// via CAS and registers a thread-local `SlotRegistration` whose `Drop` implementation
// clears its bit on thread termination. This guarantees that N_live active threads
// strictly occupy slots 0..N_live-1 without modulo collisions across NUM_EPOCH_STRIPES (16).
#[cfg(all(feature = "std", not(loom)))]
static ALLOC_SLOTS_MASK: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// One past the highest epoch stripe any thread in the process can have
/// retired on. It only grows, and it grows before the thread that raised it
/// can retire anything, so every stripe at or above it is empty in every
/// `Collector`: an advance scans `0..stripe_bound()` instead of all
/// `NUM_EPOCH_STRIPES`. Written once per slot claim, never on `retire`, so
/// it adds no write to a line the writers share. A stale read only delays a
/// stripe to its bin's next drain, the same contract as `PaddedBin::nonempty`,
/// and `drain` ignores it.
#[cfg(all(feature = "std", not(loom)))]
static STRIPE_BOUND: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// Records that `slot` may retire, so advances scan its stripe.
#[cfg(all(feature = "std", not(loom)))]
fn raise_stripe_bound(slot: usize) {
    STRIPE_BOUND.fetch_max(slot % NUM_EPOCH_STRIPES + 1, Ordering::AcqRel);
}

/// How many stripes, from 0, an advance must scan.
#[cfg(all(feature = "std", not(loom)))]
#[inline]
fn stripe_bound() -> usize {
    STRIPE_BOUND.load(Ordering::Acquire)
}

// Loom's model threads pick their stripe with `set_writer_slot` and never
// claim one, so a model scans every stripe.
#[cfg(all(feature = "std", loom))]
#[inline]
fn stripe_bound() -> usize {
    NUM_EPOCH_STRIPES
}

#[cfg(all(feature = "std", not(loom)))]
struct SlotRegistration {
    claimed_bit: core::cell::Cell<Option<u8>>,
}

#[cfg(all(feature = "std", not(loom)))]
impl SlotRegistration {
    const fn new() -> Self {
        Self {
            claimed_bit: core::cell::Cell::new(None),
        }
    }
}

#[cfg(all(feature = "std", not(loom)))]
impl Drop for SlotRegistration {
    fn drop(&mut self) {
        if let Some(bit) = self.claimed_bit.get() {
            ALLOC_SLOTS_MASK.fetch_and(!(1u64 << bit), Ordering::Release);
        }
    }
}

#[cfg(all(feature = "std", not(loom)))]
std::thread_local! {
    // Read on every `writer_slot()` call, so it is deliberately `Drop`-free and
    // const-initialised: a thread-local whose type has a destructor cannot use
    // the cheapest TLS access path, because every access has to go through
    // destructor-registration bookkeeping first. Keeping the hot value in its
    // own `Drop`-free cell restores that path.
    static SLOT: core::cell::Cell<usize> = const { core::cell::Cell::new(usize::MAX) };
    // Carries the `Drop` that releases the claimed bit on thread exit. Touched
    // exactly once per thread, on the claim, and never on the hot read.
    static REGISTRATION: SlotRegistration = const { SlotRegistration::new() };
}

#[cfg(all(feature = "std", loom))]
loom::thread_local! {
    static STRIPE: core::cell::Cell<usize> = core::cell::Cell::new(usize::MAX);
}

/// The calling thread's writer slot, in `0..MAX_WRITER_SLOTS`.
///
/// On thread termination, the claimed slot bit is recycled back to
/// `ALLOC_SLOTS_MASK` via `Drop`, ensuring that $N_{\text{live}}$ active threads
/// occupy the densest set of slots in $0..N_{\text{live}}-1$ with zero modulo collision
/// when mapped to `NUM_EPOCH_STRIPES` (Refs #568).
#[cfg(all(feature = "std", not(loom)))]
#[inline]
pub(crate) fn writer_slot() -> usize {
    // Hot path: one read of a `Drop`-free, const-initialised thread-local.
    let v = SLOT.with(|s| s.get());
    if v < MAX_WRITER_SLOTS {
        return v;
    }
    claim_writer_slot()
}

/// The once-per-thread slow half of [`writer_slot`]: claim the lowest free bit
/// and arm the destructor that releases it. Kept out of line so the hot read
/// above stays a single TLS access with no claim code inlined around it.
#[cfg(all(feature = "std", not(loom)))]
#[cold]
#[inline(never)]
fn claim_writer_slot() -> usize {
    let mut curr = ALLOC_SLOTS_MASK.load(Ordering::Relaxed);
    loop {
        let free = !curr;
        if free == 0 {
            // All 64 slots are currently held by live threads.
            // Fall back without bit reservation.
            static OVERFLOW_COUNTER: core::sync::atomic::AtomicUsize =
                core::sync::atomic::AtomicUsize::new(0);
            let overflow = OVERFLOW_COUNTER.fetch_add(1, Ordering::Relaxed) % MAX_WRITER_SLOTS;
            raise_stripe_bound(overflow);
            SLOT.with(|s| s.set(overflow));
            return overflow;
        }
        let bit = free.trailing_zeros() as usize;
        if bit >= MAX_WRITER_SLOTS {
            raise_stripe_bound(0);
            SLOT.with(|s| s.set(0));
            return 0;
        }
        let next = curr | (1u64 << bit);
        match ALLOC_SLOTS_MASK.compare_exchange_weak(
            curr,
            next,
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            Ok(_) => {
                // Touch the destructor-carrying thread-local exactly once, here,
                // so thread exit still releases the bit.
                REGISTRATION.with(|reg| reg.claimed_bit.set(Some(bit as u8)));
                raise_stripe_bound(bit);
                SLOT.with(|s| s.set(bit));
                return bit;
            }
            Err(actual) => curr = actual,
        }
    }
}

#[cfg(all(feature = "std", loom))]
#[inline]
pub(crate) fn writer_slot() -> usize {
    STRIPE.with(|s| {
        let v = s.get();
        if v < MAX_WRITER_SLOTS {
            return v;
        }
        0
    })
}

/// Pins the calling thread to `slot`, so a test can place work on a chosen
/// stripe.
#[cfg(all(test, feature = "std", not(loom)))]
pub(crate) fn set_writer_slot(slot: usize) {
    assert!(slot < MAX_WRITER_SLOTS, "stripe {slot} out of range");
    raise_stripe_bound(slot);
    SLOT.with(|s| s.set(slot));
}

#[cfg(all(test, feature = "std", loom))]
pub(crate) fn set_writer_slot(slot: usize) {
    assert!(slot < MAX_WRITER_SLOTS, "stripe {slot} out of range");
    STRIPE.with(|s| s.set(slot));
}

/// Test helper to inspect the live slot allocation bitmask.
#[cfg(all(test, feature = "std", not(loom)))]
pub(crate) fn live_slot_mask() -> u64 {
    ALLOC_SLOTS_MASK.load(Ordering::Relaxed)
}

/// Test helper to release the current thread's claimed slot.
#[cfg(all(test, feature = "std", not(loom)))]
pub(crate) fn reset_thread_writer_slot() {
    REGISTRATION.with(|reg| {
        if let Some(bit) = reg.claimed_bit.get() {
            ALLOC_SLOTS_MASK.fetch_and(!(1u64 << bit), Ordering::Release);
            reg.claimed_bit.set(None);
        }
    });
    SLOT.with(|s| s.set(usize::MAX));
}

// Without `std` there are no threads to separate, and the sharded
// allocator accounting (the one ablation that builds there) uses shard 0.
#[cfg(not(feature = "std"))]
#[allow(dead_code)]
#[inline(always)]
pub(crate) fn writer_slot() -> usize {
    0
}

#[cfg(feature = "ablation-unpadded-lock")]
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
#[repr(C)]
pub struct Collector {
    // NOT boxed. `Collector` is only ever constructed behind an `Arc`
    // (`Arc::new(Collector::new())` in sync.rs; the only bare
    // constructions are in this file's test module), so it already lives on
    // the heap and there is no stack bloat for a `Box` to prevent. A `Box`
    // would only move the same 4 KiB into a second allocation behind a
    // pointer loaded on every `retire`.
    bins: [[PaddedBin; NUM_EPOCH_STRIPES]; BINS],
    epoch: Line<AtomicUsize>,
    advancing: AtomicBool,
    pub(crate) alive: AtomicBool,
    readers: Mutex<Vec<Arc<Slot>>>,
    pub(crate) retained_bytes: [PaddedRetained; NUM_EPOCH_STRIPES],
    #[cfg(feature = "ablation-unstriped-freelist")]
    freelists: [Mutex<FreeListHead>; NUM_CLASSES],
    #[cfg(not(feature = "ablation-unstriped-freelist"))]
    freelists: [PaddedFreelists; NUM_FREELIST_STRIPES],
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
            bins: core::array::from_fn(|_| core::array::from_fn(|_| PaddedBin::new())),
            epoch: line(AtomicUsize::new(0)),
            advancing: AtomicBool::new(false),
            alive: AtomicBool::new(true),
            readers: Mutex::new(Vec::new()),
            retained_bytes: core::array::from_fn(|_| PaddedRetained(AtomicUsize::new(0))),
            #[cfg(feature = "ablation-unstriped-freelist")]
            freelists: core::array::from_fn(|_| Mutex::new(FreeListHead::empty())),
            #[cfg(not(feature = "ablation-unstriped-freelist"))]
            freelists: core::array::from_fn(|_| {
                PaddedFreelists(core::array::from_fn(|_| Mutex::new(FreeListHead::empty())))
            }),
            #[cfg(test)]
            registrations: core::sync::atomic::AtomicU64::new(0),
        }
    }

    /// The freelist an allocation of `class` pops from: the one shared list
    /// under `ablation-unstriped-freelist`, or the calling writer's own stripe.
    #[inline(always)]
    fn alloc_freelist(&self, class: usize) -> &Mutex<FreeListHead> {
        #[cfg(feature = "ablation-unstriped-freelist")]
        {
            &self.freelists[class]
        }
        #[cfg(not(feature = "ablation-unstriped-freelist"))]
        {
            &self.freelists[writer_slot() % NUM_FREELIST_STRIPES].0[class]
        }
    }

    /// The freelist a reclaimed block of `class` is pushed to: the one shared
    /// list under `ablation-unstriped-freelist`, or the draining stripe's freelist.
    #[inline(always)]
    fn reclaim_freelist(&self, stripe: usize, class: usize) -> &Mutex<FreeListHead> {
        #[cfg(feature = "ablation-unstriped-freelist")]
        {
            let _ = stripe;
            &self.freelists[class]
        }
        #[cfg(not(feature = "ablation-unstriped-freelist"))]
        {
            &self.freelists[stripe].0[class]
        }
    }

    /// Pops a reclaimed block from this collector's size-class freelist.
    #[inline(never)]
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
        #[cfg(feature = "collector-census")]
        {
            head.1.len -= 1;
            head.1.reused += 1;
        }
        drop(head);
        let bytes = CLASS_SPECS[class].0;
        // SAFETY: zero out the reused memory before returning.
        unsafe { core::ptr::write_bytes(block.cast::<u8>(), 0, bytes) };
        block.cast::<u8>()
    }

    /// Immediately recycles an unpublished allocation back into this collector's size-class freelist,
    /// or frees it if outside size classes.
    ///
    /// # Concurrency & AGENTS.md §2.6 Architectural Contract
    /// AGENTS.md §2.6 strictly bans thread-local *retire* buffers for published/reachable memory
    /// to avoid S4 store-buffer pairing hazards across epoch advances. `recycle_unpublished`
    /// deals only with unshared, speculative memory that was aborted prior to publication.
    ///
    /// # Safety
    ///
    /// `ptr` must have been allocated by this collector's associated `TreeAlloc`, must NEVER have been
    /// published to any node/edge or seen by any reader, and must not be used afterwards.
    #[allow(dead_code)]
    pub(crate) unsafe fn recycle_unpublished(&self, ptr: NonNull<u8>, bytes: usize, align: usize) {
        if let Some(class) = class_for(bytes, align) {
            let block = ptr.as_ptr().cast::<FreeBlock>();
            let mut head = self
                .alloc_freelist(class)
                .lock()
                .expect("freelist poisoned");
            // SAFETY: block was allocated matching this size class, was never published, and caller relinquishes ownership.
            unsafe {
                (*block).next = head.0;
            }
            head.0 = block;
            #[cfg(feature = "collector-census")]
            {
                head.1.len += 1;
                head.1.recycled += 1;
            }
        } else {
            // SAFETY: ptr was never published and matches layout contract.
            free_raw(ptr, bytes, align);
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
            _not_sync: core::marker::PhantomData,
        }
    }

    /// Queues an allocation for deferred freeing (writer side).
    ///
    /// Ownership of the allocation passes to the collector. Once every reader
    /// pinned when it was retired has unpinned, a later [`try_advance`] (or
    /// the collector's drop) either returns it to the global allocator with
    /// `Layout::from_size_align(bytes, align)` or, when `(bytes, align)` is a
    /// node size class, keeps it on a freelist and hands it out again as a
    /// node of a tree that defers to this collector.
    ///
    /// # Safety
    ///
    /// The caller guarantees all of the following:
    ///
    /// - **Provenance and layout.** `ptr` was returned by the global
    ///   allocator (`std::alloc::alloc`, `alloc_zeroed` or `realloc`, or an
    ///   owner such as `Box` or `Vec` that allocates through it) for a
    ///   layout of exactly `bytes` bytes aligned to `align`: `bytes` is
    ///   non-zero, `align` is the alignment it was allocated with, and
    ///   `ptr` carries the provenance of that whole allocation (it is the
    ///   pointer the allocator returned, not one derived from a borrow).
    /// - **Retired once.** The allocation has not been freed, and is not
    ///   retired, freed or otherwise released again by anyone, through this
    ///   collector or any other path.
    /// - **Unreachable before the call.** No new reader can obtain `ptr`
    ///   once this call begins: every shared location that published it has
    ///   already been overwritten or unlinked.
    /// - **No use after the grace period.** After the call, the allocation
    ///   is accessed only by a thread that obtained `ptr` while pinned on a
    ///   [`Reader`] registered with *this* collector, only while that reader
    ///   stays pinned without interruption (dropping any [`Pin`] of a reader
    ///   unpins it; see [`Reader::pin`]), and never to free, retire or
    ///   reuse it. No reference or pointer to it is used after that pin
    ///   ends. The retiring thread is bound by the same rule.
    ///
    /// Retiring a pointer the global allocator did not return, as below, is
    /// the undefined behaviour this contract rules out, and safe code cannot
    /// express it:
    ///
    /// ```compile_fail,E0133
    /// let c = std::sync::Arc::new(expanse_trie::occ::Collector::new());
    /// c.retire(core::ptr::NonNull::dangling(), 64, 64);
    /// ```
    ///
    /// A sound retirement hands over a fresh allocation with its own layout:
    ///
    /// ```
    /// use std::alloc::{alloc_zeroed, Layout};
    /// use std::ptr::NonNull;
    /// use std::sync::Arc;
    ///
    /// let c = Arc::new(expanse_trie::occ::Collector::new());
    /// let layout = Layout::from_size_align(64, 16).unwrap();
    /// // SAFETY: `layout` has non-zero size.
    /// let ptr = NonNull::new(unsafe { alloc_zeroed(layout) }).unwrap();
    /// // SAFETY: `ptr` came from the global allocator with `(64, 16)`, was
    /// // never published and is not used again: the collector owns it.
    /// unsafe { c.retire(ptr, 64, 16) };
    /// assert_eq!(c.retained_bytes(), 64);
    /// drop(c); // frees it
    /// ```
    ///
    /// [`try_advance`]: Collector::try_advance
    pub unsafe fn retire(&self, ptr: NonNull<u8>, bytes: usize, align: usize) {
        crate::occ_stats::bump(crate::occ_stats::Stat::Retired);
        // S4 store-buffer pairing with `try_advance` / `Reader::pin` (ARCHITECTURE.md §4.2):
        // the retire-side epoch load is preceded by a SeqCst fence, ensuring that a retirer
        // has a happens-before with an advance and readers pinned at the next epoch.
        fence(Ordering::SeqCst);
        let e = self.epoch.load(Ordering::Relaxed);
        // One thread-local read, not two: `writer_slot()` goes through TLS and
        // `retire` is on the per-mutation path, so calling it twice paid for the
        // lookup twice for one value.
        let slot = writer_slot();
        let g = Garbage { ptr, bytes, align };
        let stripe = slot % NUM_EPOCH_STRIPES;
        let p_bin = &self.bins[e % BINS][stripe];
        // Classified before the lock is taken, so the critical section only
        // gains the increment.
        #[cfg(feature = "collector-census")]
        let class = class_for(bytes, align);
        let mut garbage = p_bin.garbage.lock().expect("garbage bin poisoned");
        // Counted before the push: an advance may take the block as soon
        // as the lock drops, and its per-stripe subtraction must not come
        // first, or the stripe's counter wraps.
        self.retained_bytes[stripe]
            .0
            .fetch_add(bytes, Ordering::Relaxed);
        #[cfg(feature = "collector-census")]
        match class {
            Some(class) => garbage.counts.retired[class] += 1,
            None => {
                garbage.counts.unclassed.retired_blocks += 1;
                garbage.counts.unclassed.retired_bytes += bytes as u64;
            }
        }
        garbage.list.push(g);
        p_bin.nonempty.store(true, Ordering::Relaxed);
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
        let stale_bin = (e + BINS - 1) % BINS;
        for stripe in 0..stripe_bound() {
            let p_bin = &self.bins[stale_bin][stripe];
            // A stripe whose flag reads clear is skipped without its
            // lock; one being filled right now keeps its garbage until
            // this bin's next drain.
            if !p_bin.nonempty.load(Ordering::Relaxed) {
                continue;
            }
            let stale = p_bin.take::<false>();
            let mut freed_bytes = 0;
            for g in stale {
                freed_bytes += g.bytes;
                if let Some(class) = class_for(g.bytes, g.align) {
                    let block = g.ptr.as_ptr().cast::<FreeBlock>();
                    let mut head = self
                        .reclaim_freelist(stripe, class)
                        .lock()
                        .expect("freelist poisoned");
                    // SAFETY: block was retired by a well-aligned allocation
                    // matching this size class, and grace period elapsed.
                    unsafe {
                        (*block).next = head.0;
                    }
                    head.0 = block;
                    #[cfg(feature = "collector-census")]
                    {
                        head.1.len += 1;
                        head.1.reclaimed += 1;
                    }
                } else {
                    free_raw(g.ptr, g.bytes, g.align);
                }
            }
            self.retained_bytes[stripe]
                .0
                .fetch_sub(freed_bytes, Ordering::Relaxed);
            crate::occ_stats::record_reclaim(freed_bytes);
        }
    }

    /// Total bytes currently queued across this collector's garbage bins.
    ///
    /// Measures unreclaimed garbage backlog queued in epoch bins awaiting
    /// reclamation, not total allocated heap footprint (reclaimed node
    /// blocks transition to collector size-class freelists for reuse).
    #[must_use]
    pub fn retained_bytes(&self) -> usize {
        let mut sum: usize = 0;
        for s in self.retained_bytes.iter() {
            sum += s.0.load(Ordering::Relaxed);
        }
        sum
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
    /// on drop, when exclusive ownership proves that). Crate-private for that
    /// reason: [`Self::register`] is public, so a public `drain` would let safe
    /// code free memory a pinned reader is still reading.
    pub(crate) fn drain(&self) {
        for b in 0..BINS {
            for stripe in 0..NUM_EPOCH_STRIPES {
                // Every stripe, whatever its flag says: nothing may
                // outlive the collector.
                let stale = self.bins[b][stripe].take::<true>();
                if stale.is_empty() {
                    continue;
                }
                let mut freed_bytes = 0;
                for g in stale {
                    freed_bytes += g.bytes;
                    free_raw(g.ptr, g.bytes, g.align);
                }
                self.retained_bytes[stripe]
                    .0
                    .fetch_sub(freed_bytes, Ordering::Relaxed);
                crate::occ_stats::record_reclaim(freed_bytes);
            }
        }
        self.release_free_lists_as::<true>();
    }

    /// The freelist rows: one per stripe, or the single shared row under
    /// the ablation.
    fn free_list_rows(&self) -> impl Iterator<Item = &[Mutex<FreeListHead>; NUM_CLASSES]> {
        #[cfg(feature = "ablation-unstriped-freelist")]
        let rows = core::iter::once(&self.freelists);
        #[cfg(not(feature = "ablation-unstriped-freelist"))]
        let rows = self.freelists.iter().map(|stripe| &stripe.0);
        rows
    }

    /// Bytes on the freelists: blocks past their grace period, kept for
    /// reuse by this collector's trees. Walks each list under its stripe's
    /// lock, so writers popping the same class wait for the walk.
    pub(crate) fn free_list_bytes(&self) -> usize {
        let mut sum = 0;
        for row in self.free_list_rows() {
            for (class, &(bytes, _)) in CLASS_SPECS.iter().enumerate() {
                let head = row[class].lock().expect("freelist poisoned");
                let mut cur = head.0;
                while !cur.is_null() {
                    sum += bytes;
                    // SAFETY: a freelist entry is a free block of this class
                    // with `next` written; the lock keeps it on the list.
                    cur = unsafe { (*cur).next };
                }
            }
        }
        sum
    }

    /// Returns every block on the freelists to the global allocator and
    /// returns the bytes released. Each list is detached under its stripe's
    /// lock, as `pop_freelist` and the reclaim path take it, and freed after
    /// the lock is dropped. Only blocks past their grace period are on a
    /// freelist, so no reader can hold one; blocks still in their grace
    /// period stay in their bins. Concurrent writers that find a list empty
    /// fall through to the system allocator, as on any miss.
    pub(crate) fn release_free_lists(&self) -> usize {
        self.release_free_lists_as::<false>()
    }

    /// [`Self::release_free_lists`]; `DRAIN` says whether `drain` is the
    /// caller, for the `collector-census` counters only: a drain is not
    /// counted as a release.
    fn release_free_lists_as<const DRAIN: bool>(&self) -> usize {
        let mut released = 0;
        for row in self.free_list_rows() {
            for (class, &(bytes, align)) in CLASS_SPECS.iter().enumerate() {
                let mut head = row[class].lock().expect("freelist poisoned");
                let mut cur = head.0;
                head.0 = core::ptr::null_mut();
                #[cfg(feature = "collector-census")]
                {
                    let counts = &mut head.1;
                    if !DRAIN {
                        counts.released += counts.len;
                    }
                    counts.len = 0;
                }
                drop(head);
                let layout = Layout::from_size_align(bytes, align).expect("valid node layout");
                while !cur.is_null() {
                    // SAFETY: cur was allocated with `layout`: a deferred
                    // tree carves no slab pages (`NodeAlloc::defer_to`
                    // asserts it), so every block reaching a freelist is a
                    // system allocation of exactly its class's layout.
                    let next = unsafe { (*cur).next };
                    // SAFETY: deallocating a detached, unreferenced freelist
                    // block with its original layout.
                    unsafe { dealloc(cur.cast::<u8>(), layout) };
                    released += bytes;
                    cur = next;
                }
            }
        }
        released
    }

    /// Where this collector's bytes sit: per size class, the blocks on the
    /// freelists and the retired blocks still in their grace period; the
    /// grace-period blocks no class serves; and the freelist blocks per
    /// writer stripe.
    ///
    /// Computed on demand by walking every freelist and every epoch bin
    /// under its own lock, one lock at a time, as `free_list_bytes` walks
    /// the freelists: a writer popping, retiring into or reclaiming onto the list being
    /// walked waits for the walk, and nothing is added to any write or read
    /// path. O(free blocks + retired blocks). Exact on a collector no writer
    /// is using; with writers running, each list's figure is exact at the
    /// moment it was walked.
    ///
    /// On a quiesced collector, [`CollectorCensus::free_bytes`] is the sum
    /// of the freelist bytes and [`CollectorCensus::grace_bytes`] equals
    /// [`Self::retained_bytes`].
    #[cold]
    #[inline(never)]
    #[must_use]
    pub fn census(&self) -> CollectorCensus {
        let mut classes: Vec<ClassCensus> = CLASS_SPECS
            .iter()
            .map(|&(block_bytes, align)| ClassCensus {
                block_bytes,
                align,
                ..ClassCensus::default()
            })
            .collect();
        let mut stripes = Vec::new();
        for row in self.free_list_rows() {
            let mut stripe = BlockTally::default();
            for (class, &(bytes, _)) in CLASS_SPECS.iter().enumerate() {
                let head = row[class].lock().expect("freelist poisoned");
                let mut blocks = 0;
                let mut cur = head.0;
                while !cur.is_null() {
                    blocks += 1;
                    // SAFETY: a freelist entry is a free block of this class
                    // with `next` written; the lock keeps it on the list.
                    cur = unsafe { (*cur).next };
                }
                drop(head);
                classes[class].free.add(blocks, blocks * bytes);
                stripe.add(blocks, blocks * bytes);
            }
            stripes.push(stripe);
        }
        let mut unclassed_grace = BlockTally::default();
        for bin in &self.bins {
            // Every stripe, whatever its `nonempty` hint says.
            for p_bin in bin {
                let garbage = p_bin.garbage.lock().expect("garbage bin poisoned");
                for g in &garbage.list {
                    match class_for(g.bytes, g.align) {
                        Some(class) => classes[class].grace.add(1, g.bytes),
                        None => unclassed_grace.add(1, g.bytes),
                    }
                }
            }
        }
        CollectorCensus {
            classes,
            unclassed_grace,
            stripes,
        }
    }

    /// This collector's cumulative block counters, per size class (feature
    /// `collector-census`): blocks retired, reclaimed to a freelist after
    /// their grace period, recycled unpublished, reused by an allocation,
    /// and released by `shrink_to_fit`; and for the blocks no class serves,
    /// retired and released after their grace period. `drain` counts
    /// nothing: the wrappers drain on drop, after which nothing can read the
    /// counters.
    ///
    /// Summed over every bin and freelist, each read under its own lock, one
    /// at a time: exact on a quiesced collector. A counter never decreases.
    #[cfg(feature = "collector-census")]
    #[cold]
    #[inline(never)]
    #[must_use]
    pub fn counters(&self) -> CollectorCounters {
        let mut classes: Vec<ClassCounters> = CLASS_SPECS
            .iter()
            .map(|&(block_bytes, align)| ClassCounters {
                block_bytes,
                align,
                ..ClassCounters::default()
            })
            .collect();
        for row in self.free_list_rows() {
            for (class, out) in classes.iter_mut().enumerate() {
                let c = row[class].lock().expect("freelist poisoned").1;
                out.reclaimed += c.reclaimed;
                out.recycled += c.recycled;
                out.reused += c.reused;
                out.released += c.released;
            }
        }
        let mut unclassed = UnclassedCounters::default();
        for bin in &self.bins {
            for p_bin in bin {
                let garbage = p_bin.garbage.lock().expect("garbage bin poisoned");
                let c = &garbage.counts;
                for (class, out) in classes.iter_mut().enumerate() {
                    out.retired += c.retired[class];
                }
                unclassed.retired_blocks += c.unclassed.retired_blocks;
                unclassed.retired_bytes += c.unclassed.retired_bytes;
                unclassed.released_blocks += c.unclassed.released_blocks;
                unclassed.released_bytes += c.unclassed.released_bytes;
            }
        }
        CollectorCounters { classes, unclassed }
    }
}

#[cfg(feature = "std")]
impl Drop for Collector {
    fn drop(&mut self) {
        // Frees queued garbage bins and size-class freelists.
        self.drain();
    }
}

/// Test harness: holds a deferred tree and its associated [`Collector`] and
/// tree-level [`SeqVersion`] word for OCC differential parity tests.
///
/// The tree's allocator holds a raw pointer to the boxed version word, so the
/// tree must never leave this holder: every field is private, and the holder
/// implements [`Deref`](core::ops::Deref) but not `DerefMut`, since a
/// `&mut ExpanseMap` is enough to move the tree out with `mem::take` or
/// `mem::swap`. Mutation goes through the forwarding methods defined beside
/// each tree type (`insert`, `remove`, and `ins_slot` on the map). The
/// collector is never handed out, so no [`Reader`] can be registered on it and
/// [`Self::drain`] cannot free memory a pinned reader holds.
///
/// Reading and mutating through the holder:
///
/// ```
/// use expanse_trie::map::ExpanseMap;
/// let mut dt = ExpanseMap::deferred_for_test(true);
/// for k in 0..2000u64 { dt.insert(k * 7919, k); }
/// assert_eq!(dt.len(), 2000);
/// dt.drain();
/// ```
///
/// The tree cannot be moved out through a field (the use-after-free shipped
/// in v0.10.1, where `tree` was a public field):
///
/// ```compile_fail,E0616
/// use expanse_trie::map::ExpanseMap;
/// let mut dt = ExpanseMap::deferred_for_test(true);
/// let t = std::mem::take(&mut dt.tree);
/// ```
///
/// nor through a mutable dereference:
///
/// ```compile_fail,E0596
/// use expanse_trie::map::ExpanseMap;
/// let mut dt = ExpanseMap::deferred_for_test(true);
/// let t = std::mem::take(&mut *dt);
/// ```
///
/// and the collector does not escape, so no reader can pin it:
///
/// ```compile_fail,E0616
/// use expanse_trie::map::ExpanseMap;
/// let dt = ExpanseMap::deferred_for_test(true);
/// let _reader = dt.collector.register();
/// ```
#[cfg(all(feature = "std", target_pointer_width = "64"))]
#[doc(hidden)]
pub struct DeferredTestTree<T> {
    pub(crate) tree: T,
    collector: Arc<Collector>,
    _word: core_alloc::boxed::Box<SeqVersion>,
    engine_covers_root: bool,
}

#[cfg(all(feature = "std", target_pointer_width = "64"))]
impl<T> DeferredTestTree<T> {
    /// `tree`'s allocator must be deferred to `collector` and bound to `word`,
    /// and `collector` must not be shared with any reader.
    #[must_use]
    pub(crate) fn new(
        tree: T,
        collector: Arc<Collector>,
        word: core_alloc::boxed::Box<SeqVersion>,
        engine_covers_root: bool,
    ) -> Self {
        Self {
            tree,
            collector,
            _word: word,
            engine_covers_root,
        }
    }

    /// Frees the collector's queued garbage and freelists. Sound because the
    /// collector never leaves this holder, so no reader is registered on it.
    #[inline(always)]
    pub fn drain(&self) {
        self.collector.drain();
    }
}

#[cfg(all(feature = "std", target_pointer_width = "64"))]
impl<T> core::ops::Deref for DeferredTestTree<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        &self.tree
    }
}

#[cfg(all(feature = "std", target_pointer_width = "64"))]
impl<T> Drop for DeferredTestTree<T> {
    fn drop(&mut self) {
        if !self.engine_covers_root {
            #[cfg(debug_assertions)]
            crate::alloc::bracket_stack::leave(core::ptr::without_provenance(usize::MAX));
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

/// A registered reader handle. Cheap to pin/unpin per operation.
///
/// **`Send`, and pinned as such.** A `Reader` may be moved to, and dropped on,
/// a thread other than the one that registered it: its fields are two `Arc`s,
/// and its [`Drop`] deregisters through the collector's registry mutex without
/// reading any thread-local. The C ABI relies on this (a reader handle may be
/// freed from another thread, `include/expanse.h`), so the bound is asserted at
/// compile time below rather than left to auto-derivation.
///
/// **Not `Sync`: one handle per reading thread.** The reader owns a single
/// epoch slot, and dropping *any* [`Pin`] unpins it entirely (see
/// [`Self::pin`]). Two threads pinning through one shared `&Reader` could
/// therefore clear each other's pin, and two epoch advances later the
/// collector would free memory the first thread is still reading. The type
/// rules that out: a `Reader` cannot be shared by reference across threads,
/// and neither can any wrapper handle that embeds one (`sync::MapReader`,
/// `sync::OwnedMapReader`, `sync::DetachedMapReader`, `sync::SetReader`,
/// `sync::StrReader`, `sync::BytesReader`, `sync::BlobReader`). Register one
/// handle per thread; to hand a handle to another thread, move it.
///
/// The handle moves between threads:
///
/// ```no_run
/// fn assert_send<T: Send>() {}
/// assert_send::<expanse_trie::occ::Reader>();
/// ```
///
/// and is not shareable between them. This is the same program with the bound
/// swapped, so what fails below is the missing `Sync` and nothing else:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<expanse_trie::occ::Reader>();
/// ```
#[cfg(feature = "std")]
pub struct Reader {
    pub(crate) collector: Arc<Collector>,
    slot: Arc<Slot>,
    /// Withdraws `Sync` and keeps `Send`: `Cell<()>` is `Send` and not `Sync`,
    /// and a `PhantomData` of it is zero-sized. See the type's documentation.
    _not_sync: core::marker::PhantomData<core::cell::Cell<()>>,
}

/// Fails the build if any listed type implements `Sync`.
///
/// Stable Rust has no negative bound, so this is the ambiguity construction:
/// every type gets the `()` impl, a `Sync` type gets a second one, and the
/// inferred `_` then has two candidates, which is an error (E0283). The
/// `compile_fail` doctests on the public types guard the same property from
/// outside the crate; this guards it at the definition, in every build.
#[cfg(feature = "std")]
macro_rules! assert_not_sync {
    ($($t:ty),+ $(,)?) => {
        const _: fn() = || {
            trait AmbiguousIfSync<A> {
                fn probe() {}
            }
            impl<T: ?Sized> AmbiguousIfSync<()> for T {}
            struct IsSync;
            impl<T: ?Sized + Sync> AmbiguousIfSync<IsSync> for T {}
            $(let _ = <$t as AmbiguousIfSync<_>>::probe;)+
        };
    };
}
#[cfg(feature = "std")]
pub(crate) use assert_not_sync;

// See `Reader`'s doc comment. A future `!Send` field must fail the build, not
// silently withdraw a bound the C ABI documents; and a change that makes the
// type `Sync` again must fail it too, because that makes the shared-handle
// program expressible again.
#[cfg(feature = "std")]
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<Reader>();
};
#[cfg(feature = "std")]
assert_not_sync!(Reader);

// The marker is a `Cell`, which is not `RefUnwindSafe`, and the auto trait
// would follow it. Nothing about the reader changed across an unwind: its
// state is two `Arc`s over atomics, as before, and the `Cell` is phantom. The
// bound is restored so that withdrawing `Sync` is the only change to the
// type's public surface.
#[cfg(feature = "std")]
impl core::panic::RefUnwindSafe for Reader {}

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
///
/// A `Pin` stays `Send` and `Sync` while [`Reader`] is not `Sync`, and that is
/// deliberate. A pin grants no access by itself: every walk that relies on one
/// is `unsafe` and states the pin in its contract. What a pin can do wrong is
/// overlap a sibling pin from the same reader (see [`Reader::pin`]), and that
/// is possible on one thread (`let a = r.pin(); let b = r.pin(); drop(b);`),
/// so withdrawing `Send` would close no case the single-threaded rule leaves
/// open. The wrappers close it where it matters: their reads pin and unpin
/// inside one call on a handle no other thread can reach, and the one
/// long-lived guard, `sync::BlobReadGuard`, borrows its reader `&mut`, so no
/// second pin can exist while it lives, on any thread. Moving that guard to
/// another thread and dropping it there is therefore sound, and removing
/// `Send` from `Pin` would remove it from the guard for nothing.
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

    /// AGENTS.md §2.7: a promoted mechanism must stay ablatable, and its inverse
    /// must never be a no-op that silently compares the default with itself.
    /// `Line<X>` is a 64-byte-aligned wrapper by default and a transparent alias
    /// under `ablation-unpadded-lock`, so the two builds disagree here. If the
    /// inverse stopped changing the layout, one of these branches would fail.
    #[test]
    fn ablation_line_padding_follows_the_build() {
        use core::sync::atomic::AtomicUsize;
        #[cfg(not(feature = "ablation-unpadded-lock"))]
        {
            assert_eq!(core::mem::align_of::<Line<AtomicUsize>>(), 64);
            assert_eq!(core::mem::size_of::<Line<AtomicUsize>>(), 64);
        }
        #[cfg(feature = "ablation-unpadded-lock")]
        {
            assert_eq!(
                core::mem::align_of::<Line<AtomicUsize>>(),
                core::mem::align_of::<AtomicUsize>()
            );
            assert_eq!(
                core::mem::size_of::<Line<AtomicUsize>>(),
                core::mem::size_of::<AtomicUsize>()
            );
        }
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
        // SAFETY: a fresh `(64, TEST_ALIGN)` global allocation, never published.
        unsafe { c.retire(alloc_test_block(64), 64, TEST_ALIGN) };
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

        // SAFETY: a fresh `(64, TEST_ALIGN)` global allocation, never published.
        unsafe { c.retire(alloc_test_block(64), 64, TEST_ALIGN) };
        // SAFETY: a fresh `(128, TEST_ALIGN)` global allocation, never published.
        unsafe { c.retire(alloc_test_block(128), 128, TEST_ALIGN) };
        assert_eq!(c.retained_bytes(), 192);

        // One advance is allowed while pinned at epoch 0 (advances to epoch 1)
        c.try_advance();
        assert_eq!(c.retained_bytes(), 192);

        // Subsequent advance refused because reader is lagging at epoch 0
        // SAFETY: a fresh `(256, TEST_ALIGN)` global allocation, never published.
        unsafe { c.retire(alloc_test_block(256), 256, TEST_ALIGN) };
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
        let slot = WriterSlot::new();
        assert!(!gate.is_closed());

        // Normal entry when open
        {
            let guard = gate.enter_writer(&slot, 0);
            assert!(guard.is_some());
            assert_eq!(slot.in_flight.load(Ordering::Relaxed), 1);
        }
        assert_eq!(slot.in_flight.load(Ordering::Relaxed), 0);

        // Quiescence closure
        gate.close();
        assert!(gate.is_closed());
        // Entry fails when closed
        assert!(gate.enter_writer(&slot, 0).is_none());
        assert_eq!(slot.in_flight.load(Ordering::Relaxed), 0);

        gate.open();
        assert!(!gate.is_closed());
        {
            let guard = gate.enter_writer(&slot, 0);
            assert!(guard.is_some());
            assert_eq!(slot.in_flight.load(Ordering::Relaxed), 1);
        }
        assert_eq!(slot.in_flight.load(Ordering::Relaxed), 0);
    }

    /// Two writers publishing through one in-flight word, as they do once a
    /// tree's writer table has allocated all its slots and hashes further
    /// threads onto taken ones. Quiescence reads the word as drained when it
    /// is zero, so neither one writer's exit nor another's back-out from a
    /// closed gate may zero it while a guard is still held.
    #[test]
    fn writer_gate_shared_slot_stays_in_flight_until_last_exit() {
        let gate = WriterGate::new();
        let slot = WriterSlot::new();

        let a = gate.enter_writer(&slot, 7).expect("gate is open");
        let b = gate.enter_writer(&slot, 7).expect("gate is open");
        drop(a);
        assert_ne!(
            slot.in_flight.load(Ordering::Relaxed),
            0,
            "the first exit drained a slot another writer still holds"
        );
        drop(b);
        assert_eq!(slot.in_flight.load(Ordering::Relaxed), 0);

        let c = gate.enter_writer(&slot, 7).expect("gate is open");
        gate.close();
        assert!(gate.enter_writer(&slot, 7).is_none());
        assert_ne!(
            slot.in_flight.load(Ordering::Relaxed),
            0,
            "a back-out from the closed gate drained a slot another writer still holds"
        );
        drop(c);
        assert_eq!(slot.in_flight.load(Ordering::Relaxed), 0);
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
    #[cfg(feature = "std")]
    fn ablation_striped_epoch_reclaims_every_stripe() {
        let c = Arc::new(Collector::new());
        let reader = c.register();
        let pin = reader.pin();
        let layout = Layout::from_size_align(64, 16).unwrap();
        for stripe in 0..NUM_EPOCH_STRIPES {
            set_writer_slot(stripe);
            // SAFETY: non-zero size and a valid power-of-two alignment.
            let ptr = NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) }).unwrap();
            // SAFETY: `ptr` is a fresh `(64, 16)` global allocation, never published.
            unsafe { c.retire(ptr, 64, 16) };
            assert_eq!(c.retained_bytes[stripe].0.load(Ordering::Relaxed), 64);
        }
        let all = 64 * NUM_EPOCH_STRIPES;
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
    #[cfg(feature = "std")]
    fn ablation_striped_epoch_drain_empties_every_stripe() {
        let c = Collector::new();
        let layout = Layout::from_size_align(64, 16).unwrap();
        for stripe in 0..NUM_EPOCH_STRIPES {
            set_writer_slot(stripe);
            // SAFETY: non-zero size and a valid power-of-two alignment.
            let ptr = NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) }).unwrap();
            // SAFETY: `ptr` is a fresh `(64, 16)` global allocation, never published.
            unsafe { c.retire(ptr, 64, 16) };
        }
        assert_eq!(c.retained_bytes(), 64 * NUM_EPOCH_STRIPES);
        c.drain();
        assert_eq!(c.retained_bytes(), 0);
    }

    /// Serialises the tests in this module that spawn threads to claim writer
    /// slots: a claim in one would change the process-wide slot mask another is
    /// asserting on. It cannot serialise a test elsewhere that claims a slot by
    /// retiring through a collector, which is why an assertion on exactly which
    /// slot a thread claims runs through `in_own_process`.
    #[cfg(all(feature = "std", not(loom)))]
    static SLOT_CLAIM_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Runs `body` as the only test in its process, and tells it whether it is.
    ///
    /// `ALLOC_SLOTS_MASK` is process-global: a test that retires through a
    /// collector holds a bit of it for as long as its thread lives, so which bit
    /// a new thread claims depends on every test running at the same time.
    /// Outside Miri the calling test re-runs itself with `--exact` in a child
    /// process, where no other test claims a slot, and `body` receives `true`
    /// there. Miri cannot spawn a process, so under Miri `body` runs in place
    /// and receives `false`.
    #[cfg(all(feature = "std", not(loom)))]
    fn in_own_process(test: &str, body: impl FnOnce(bool)) {
        const CHILD: &str = "EXPANSE_OCC_TEST_OWN_PROCESS";
        if cfg!(miri) {
            body(false);
            return;
        }
        if std::env::var_os(CHILD).is_some() {
            body(true);
            return;
        }
        let module = module_path!()
            .split_once("::")
            .map_or(module_path!(), |(_, m)| m);
        let name = format!("{module}::{test}");
        let out = std::process::Command::new(std::env::current_exe().expect("test binary path"))
            .args(["--exact", name.as_str(), "--test-threads=1"])
            .env(CHILD, "1")
            .output()
            .expect("re-run the test binary");
        let stdout = String::from_utf8_lossy(&out.stdout);
        // The child's summary line as well as its exit status: a filter that
        // matches no test also exits successfully.
        assert!(
            out.status.success() && stdout.contains("test result: ok. 1 passed"),
            "{name} in its own process ({}):\n{stdout}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A claimed slot raises the stripe bound before `writer_slot` returns, so
    /// an advance that scans only `0..stripe_bound()` reaches the stripe the
    /// claiming thread retires on, and the bound never exceeds the stripe count.
    #[test]
    #[cfg(all(feature = "std", not(loom)))]
    fn claimed_slot_is_inside_the_stripe_bound() {
        let _guard = SLOT_CLAIM_TEST_LOCK.lock().unwrap();
        std::thread::spawn(|| {
            let s = writer_slot();
            assert!(
                stripe_bound() > s % NUM_EPOCH_STRIPES,
                "slot {s} claimed without raising the stripe bound {}",
                stripe_bound()
            );
        })
        .join()
        .unwrap();
        assert!(stripe_bound() <= NUM_EPOCH_STRIPES);
    }

    /// Verifies static memory layout and footprint bounds for the striped epoch architecture (Refs #568).
    /// The default layout: `collector-census` grows the bins and the
    /// freelists by design (its own test below).
    #[test]
    #[cfg(all(feature = "std", not(feature = "collector-census")))]
    fn test_collector_striped_epoch_layout_bounds() {
        assert_eq!(core::mem::size_of::<PaddedBin>(), 64);
        assert_eq!(core::mem::align_of::<PaddedBin>(), 64);
        assert_eq!(core::mem::size_of::<PaddedRetained>(), 64);
        assert_eq!(core::mem::align_of::<PaddedRetained>(), 64);
        #[cfg(not(loom))]
        {
            assert_eq!(
                core::mem::size_of::<[[PaddedBin; NUM_EPOCH_STRIPES]; BINS]>(),
                3 * 16 * 64
            );
            assert_eq!(
                core::mem::size_of::<[PaddedRetained; NUM_EPOCH_STRIPES]>(),
                16 * 64
            );
            // Epoch bin structures overhead: 3072 + 1024 = 4096 bytes (4.0 KiB <= 4.5 KiB gate ceiling).
            let epoch_bin_overhead = core::mem::size_of::<[[PaddedBin; NUM_EPOCH_STRIPES]; BINS]>()
                + core::mem::size_of::<[PaddedRetained; NUM_EPOCH_STRIPES]>();
            assert_eq!(epoch_bin_overhead, 4096);
            assert!(
                epoch_bin_overhead <= 4608,
                "epoch bin overhead {epoch_bin_overhead} exceeds 4.5 KiB gate ceiling"
            );
        }
        // `Collector` is only ever constructed behind an `Arc`, so its own size is a
        // heap-allocation size and never a stack frame. The bound that matters is the
        // epoch-bin overhead asserted above; this one only keeps the struct from growing
        // an unrelated inline array by accident. It is deliberately loose enough to hold
        // the un-boxed 4 KiB of striped bins: boxing them to shrink this number would move
        // the same bytes into a second allocation behind a pointer loaded on every `retire`.
        assert_eq!(core::mem::size_of::<Garbage>(), 24);
        assert_eq!(core::mem::offset_of!(Collector, bins), 0);
        assert!(
            core::mem::offset_of!(Collector, bins) < core::mem::offset_of!(Collector, freelists)
        );
        #[cfg(feature = "ablation-unstriped-freelist")]
        assert!(
            core::mem::size_of::<Collector>() <= 8192,
            "Collector direct size {} exceeds 8192 bytes",
            core::mem::size_of::<Collector>()
        );
        #[cfg(not(feature = "ablation-unstriped-freelist"))]
        {
            let max_bytes = match NUM_FREELIST_STRIPES {
                16 => {
                    if cfg!(target_os = "macos") {
                        30_720
                    } else {
                        22_528
                    }
                }
                64 => {
                    if cfg!(target_os = "macos") {
                        107_520
                    } else {
                        73_728
                    }
                }
                _ => 107_520,
            };
            assert!(
                core::mem::size_of::<Collector>() <= max_bytes,
                "Collector direct size {} exceeds {} bytes ceiling for S={}",
                core::mem::size_of::<Collector>(),
                max_bytes,
                NUM_FREELIST_STRIPES
            );
        }
    }

    /// Verifies thread-exit slot recycling under sequential and concurrent churn (Refs #568).
    ///
    /// When threads exit, their `SlotRegistration` drops and releases the claimed bit in
    /// `ALLOC_SLOTS_MASK`. This guarantees that N_live active threads strictly occupy slots
    /// 0..N_live-1, eliminating modulo collisions across NUM_EPOCH_STRIPES (16). These are
    /// assertions on the whole process's slot mask, so the test runs in its own process.
    #[test]
    #[cfg(all(feature = "std", not(loom)))]
    fn test_writer_slot_recycling_under_churn() {
        in_own_process("test_writer_slot_recycling_under_churn", |alone| {
            let _guard = SLOT_CLAIM_TEST_LOCK.lock().unwrap();

            reset_thread_writer_slot();
            let initial_mask = live_slot_mask();
            assert!(
                !alone || initial_mask == 0,
                "slot mask {initial_mask:#x} is held outside this test in its own process"
            );

            // 1. Sequential churn: 32 threads run sequentially.
            // Each thread must claim the lowest available slot bit, and upon thread exit,
            // its Drop implementation must recycle the slot back to ALLOC_SLOTS_MASK.
            let expected_slot = (!initial_mask).trailing_zeros() as usize;
            for _ in 0..32 {
                let handle = std::thread::spawn(move || {
                    let s = writer_slot();
                    assert_eq!(
                        s, expected_slot,
                        "sequential thread must claim lowest free slot"
                    );
                });
                handle.join().unwrap();
                assert_eq!(
                    live_slot_mask(),
                    initial_mask,
                    "thread exit must release its slot back to the mask"
                );
            }

            // 2. Concurrent churn: spawn 8 concurrent threads.
            // Because previous threads released their slots, 8 concurrent threads
            // must claim 8 distinct bits. When initial_mask is 0 (or low), all 8 active
            // threads strictly occupy slots < 16, guaranteeing zero modulo collision
            // across NUM_EPOCH_STRIPES (16).
            let barrier = Arc::new(std::sync::Barrier::new(8));
            let mut handles = Vec::new();
            for _ in 0..8 {
                let b = Arc::clone(&barrier);
                handles.push(std::thread::spawn(move || {
                    let s = writer_slot();
                    b.wait();
                    s
                }));
            }

            let mut slots = std::collections::BTreeSet::new();
            for h in handles {
                let s = h.join().unwrap();
                slots.insert(s);
            }
            assert_eq!(slots.len(), 8, "all 8 threads must hold unique slots");
            for &s in &slots {
                assert!(
                    s < MAX_WRITER_SLOTS,
                    "slot {s} must be within MAX_WRITER_SLOTS"
                );
            }
            // Only a process no other test shares starts from an empty mask, so only there
            // can 8 threads be required to occupy exactly slots 0..8.
            if initial_mask == 0 {
                for &s in &slots {
                    assert!(s < 8, "dense allocation must place slot {s} < 8");
                    assert_eq!(
                        s % NUM_EPOCH_STRIPES,
                        s,
                        "slot {s} must map 1:1 to dedicated stripe without modulo collision"
                    );
                }
            }
            assert_eq!(
                live_slot_mask(),
                initial_mask,
                "all threads exiting must return mask to initial state"
            );
        });
    }

    /// A reclaimed block goes back to the freelist of the stripe that
    /// retired it, and only an allocation on that stripe pops it.
    #[test]
    #[cfg(all(feature = "std", not(feature = "ablation-unstriped-freelist")))]
    fn ablation_striped_freelist_routes_by_stripe() {
        let c = Collector::new();
        let class = 1;
        let (bytes, align) = CLASS_SPECS[class];
        assert_eq!(class_for(bytes, align), Some(class));
        let layout = Layout::from_size_align(bytes, align).unwrap();
        let home = NUM_FREELIST_STRIPES - 1;

        set_writer_slot(home);
        // SAFETY: non-zero size and a valid power-of-two alignment.
        let ptr = NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) }).unwrap();
        // SAFETY: `ptr` is a fresh `(bytes, align)` global allocation, never
        // published; the test takes it back only through `pop_freelist`.
        unsafe { c.retire(ptr, bytes, align) };
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

        #[cfg(not(loom))]
        if NUM_FREELIST_STRIPES < MAX_WRITER_SLOTS {
            // Verify that a different slot mapping to the same stripe via modulo shares the freelist.
            // SAFETY: `layout` has non-zero size and valid alignment.
            let ptr2 = NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) }).unwrap();
            set_writer_slot(0);
            // SAFETY: `ptr2` is a fresh `(bytes, align)` global allocation, never published.
            unsafe { c.retire(ptr2, bytes, align) };
            for _ in 0..BINS {
                c.try_advance();
            }
            // Slot 0 + NUM_FREELIST_STRIPES maps to stripe 0.
            set_writer_slot(NUM_FREELIST_STRIPES);
            let got2 = c.pop_freelist(class);
            assert_eq!(
                got2,
                ptr2.as_ptr(),
                "a slot sharing the stripe via modulo must see the reclaimed block"
            );
            // SAFETY: `got2` was allocated with `layout` and the test now owns it.
            unsafe { dealloc(got2, layout) };
        }
    }

    /// The stripe a census reports for blocks reclaimed by a writer on
    /// `slot`: the slot's own stripe, or the single shared row under the
    /// ablation.
    fn census_stripe(slot: usize) -> usize {
        #[cfg(feature = "ablation-unstriped-freelist")]
        {
            let _ = slot;
            0
        }
        #[cfg(not(feature = "ablation-unstriped-freelist"))]
        {
            slot % NUM_FREELIST_STRIPES
        }
    }

    /// The number of rows a census reports per-stripe totals for.
    fn census_rows() -> usize {
        if cfg!(feature = "ablation-unstriped-freelist") {
            1
        } else {
            NUM_FREELIST_STRIPES
        }
    }

    /// #1310: a census places each block by size class, by stripe, and by
    /// whether it is still in its grace period; a block no class serves
    /// lands in the unclassed bucket; and the totals match the two figures
    /// `mem_held` sums (`free_list_bytes`, `retained_bytes`). Runs on a
    /// thread of its own so its writer slot is the one it pins.
    #[test]
    fn collector_census_places_blocks_by_class_stripe_and_grace() {
        const SLOT: usize = 3;
        std::thread::spawn(|| {
            set_writer_slot(SLOT);
            let classed = class_for(64, TEST_ALIGN).expect("(64, 16) is a size class");
            assert_eq!(
                class_for(256, TEST_ALIGN),
                None,
                "(256, 16) must be unclassed"
            );
            let c = Arc::new(Collector::new());

            let empty = c.census();
            assert_eq!(empty.classes.len(), NUM_CLASSES);
            assert_eq!(empty.stripes.len(), census_rows());
            assert_eq!(empty.total_bytes(), 0);

            // SAFETY: fresh `(64, 16)` and `(256, 16)` global allocations,
            // never published.
            unsafe {
                c.retire(alloc_test_block(64), 64, TEST_ALIGN);
                c.retire(alloc_test_block(64), 64, TEST_ALIGN);
                c.retire(alloc_test_block(256), 256, TEST_ALIGN);
            }
            let s = c.census();
            assert_eq!(s.classes[classed].block_bytes, 64);
            assert_eq!(s.classes[classed].align, TEST_ALIGN);
            assert_eq!(
                s.classes[classed].grace,
                BlockTally {
                    blocks: 2,
                    bytes: 128
                }
            );
            assert_eq!(
                s.unclassed_grace,
                BlockTally {
                    blocks: 1,
                    bytes: 256
                },
                "a retired block no class serves is counted in the unclassed bucket"
            );
            assert_eq!(s.free_bytes(), 0);
            assert_eq!(s.grace_bytes(), c.retained_bytes());
            assert_eq!(s.grace_bytes(), 384);

            // Two advances end the grace period: the classed blocks move to
            // this writer's stripe, the unclassed one is freed.
            c.try_advance();
            c.try_advance();
            let s = c.census();
            assert_eq!(
                s.classes[classed].free,
                BlockTally {
                    blocks: 2,
                    bytes: 128
                }
            );
            assert_eq!(s.classes[classed].grace, BlockTally::default());
            assert_eq!(s.unclassed_grace, BlockTally::default());
            for (i, row) in s.stripes.iter().enumerate() {
                let want = if i == census_stripe(SLOT) {
                    BlockTally {
                        blocks: 2,
                        bytes: 128,
                    }
                } else {
                    BlockTally::default()
                };
                assert_eq!(*row, want, "stripe {i}");
            }
            assert_eq!(s.free_bytes(), c.free_list_bytes());
            assert_eq!(s.total_bytes(), c.free_list_bytes() + c.retained_bytes());

            assert_eq!(c.release_free_lists(), 128);
            assert_eq!(c.census().total_bytes(), 0);
            c.drain();
        })
        .join()
        .unwrap();
    }

    /// #1310: a block retired with an alignment no class uses is unclassed
    /// even when its size matches a class, and a block of a cache-line
    /// class is placed in that class rather than the raw class of the same
    /// size.
    #[test]
    fn collector_census_classes_by_size_and_alignment() {
        let c = Arc::new(Collector::new());
        let cache_line = class_for(64, CACHE_LINE_ALIGN).expect("(64, 64) is a size class");
        let raw = class_for(64, TEST_ALIGN).expect("(64, 16) is a size class");
        assert_ne!(cache_line, raw);
        let line = Layout::from_size_align(64, CACHE_LINE_ALIGN).unwrap();
        let odd = Layout::from_size_align(64, 8).unwrap();
        // SAFETY: non-zero sizes.
        let (p_line, p_odd) = unsafe {
            (
                NonNull::new(alloc_zeroed(line)).unwrap(),
                NonNull::new(alloc_zeroed(odd)).unwrap(),
            )
        };
        // SAFETY: fresh global allocations retired with their own layouts,
        // never published.
        unsafe {
            c.retire(p_line, 64, CACHE_LINE_ALIGN);
            c.retire(p_odd, 64, 8);
        }
        let s = c.census();
        assert_eq!(s.classes[cache_line].grace.blocks, 1);
        assert_eq!(s.classes[raw].grace.blocks, 0);
        assert_eq!(s.unclassed_grace.blocks, 1);
        assert_eq!(s.grace_bytes(), 128);
        c.drain();
    }

    const CACHE_LINE_ALIGN: usize = crate::types::CACHE_LINE;

    /// The census and the counters agree, class by class, on a collector no
    /// writer is using and before it drains (the identities stated on
    /// `ClassCounters` and `UnclassedCounters`).
    #[cfg(feature = "collector-census")]
    fn assert_counters_match_census(c: &Collector) {
        let s = c.census();
        let k = c.counters();
        for (class, (sc, kc)) in s.classes.iter().zip(&k.classes).enumerate() {
            assert_eq!(
                sc.free.blocks as u64,
                kc.reclaimed + kc.recycled - kc.reused - kc.released,
                "class {class}: freelist blocks"
            );
            assert_eq!(
                sc.grace.blocks as u64,
                kc.retired - kc.reclaimed,
                "class {class}: grace blocks"
            );
        }
        assert_eq!(
            s.unclassed_grace.blocks as u64,
            k.unclassed.retired_blocks - k.unclassed.released_blocks
        );
        assert_eq!(
            s.unclassed_grace.bytes as u64,
            k.unclassed.retired_bytes - k.unclassed.released_bytes
        );
    }

    /// #1310, feature `collector-census`: a block retired, reclaimed and
    /// reallocated by the same writer counts once in each of `retired`,
    /// `reclaimed` and `reused`; a writer on another stripe cannot reuse it
    /// and moves no counter.
    #[cfg(feature = "collector-census")]
    #[test]
    fn collector_census_counters_reuse_on_the_retiring_stripe() {
        const HOME: usize = 2;
        let c = Arc::new(Collector::new());
        let class = class_for(64, TEST_ALIGN).unwrap();
        let c2 = Arc::clone(&c);
        std::thread::spawn(move || {
            set_writer_slot(HOME);
            // SAFETY: a fresh `(64, 16)` global allocation, never published.
            unsafe { c2.retire(alloc_test_block(64), 64, TEST_ALIGN) };
            let k = c2.counters();
            assert_eq!(k.classes[class].retired, 1);
            assert_eq!(k.classes[class].reclaimed, 0);
            assert_counters_match_census(&c2);
            c2.try_advance();
            c2.try_advance();
            let k = c2.counters();
            assert_eq!(k.classes[class].reclaimed, 1);
            assert_counters_match_census(&c2);
        })
        .join()
        .unwrap();

        #[cfg(not(feature = "ablation-unstriped-freelist"))]
        {
            let c3 = Arc::clone(&c);
            std::thread::spawn(move || {
                set_writer_slot(HOME + 1);
                assert!(c3.pop_freelist(class).is_null(), "another stripe's block");
                assert_eq!(c3.counters().classes[class].reused, 0);
            })
            .join()
            .unwrap();
        }

        let c4 = Arc::clone(&c);
        std::thread::spawn(move || {
            set_writer_slot(HOME);
            let got = c4.pop_freelist(class);
            assert!(!got.is_null(), "the retiring stripe gets its block back");
            let k = c4.counters();
            assert_eq!(k.classes[class].reused, 1);
            assert_eq!(k.reused_blocks(), 1);
            assert_eq!(c4.census().classes[class].free.blocks, 0);
            assert_counters_match_census(&c4);
            // SAFETY: `got` is a `(64, 16)` block the test now owns.
            unsafe { dealloc(got, Layout::from_size_align(64, TEST_ALIGN).unwrap()) };
        })
        .join()
        .unwrap();
        c.drain();
    }

    /// #1310, feature `collector-census`: `release_free_lists` counts what it
    /// releases, by class, and the counted bytes equal the bytes it returns.
    #[cfg(feature = "collector-census")]
    #[test]
    fn collector_census_counters_release_counts_the_bytes_returned() {
        let c = Arc::new(Collector::new());
        let small = class_for(64, TEST_ALIGN).unwrap();
        let large = class_for(128, TEST_ALIGN).unwrap();
        // SAFETY: fresh global allocations, never published.
        unsafe {
            c.retire(alloc_test_block(64), 64, TEST_ALIGN);
            c.retire(alloc_test_block(64), 64, TEST_ALIGN);
            c.retire(alloc_test_block(128), 128, TEST_ALIGN);
        }
        c.try_advance();
        c.try_advance();
        assert_counters_match_census(&c);
        let before = c.counters().released_bytes();
        let returned = c.release_free_lists();
        assert_eq!(returned, 256);
        let k = c.counters();
        assert_eq!(k.released_bytes() - before, returned as u64);
        assert_eq!(k.classes[small].released, 2);
        assert_eq!(k.classes[large].released, 1);
        assert_eq!(c.census().free_bytes(), 0);
        assert_counters_match_census(&c);
        c.drain();
    }

    /// #1310, feature `collector-census`: a block no class serves is counted
    /// when retired and when the advance that ends its grace period frees
    /// it, with its bytes.
    #[cfg(feature = "collector-census")]
    #[test]
    fn collector_census_counters_unclassed_release_after_grace() {
        let c = Arc::new(Collector::new());
        // SAFETY: a fresh `(256, 16)` global allocation, never published.
        unsafe { c.retire(alloc_test_block(256), 256, TEST_ALIGN) };
        let k = c.counters();
        assert_eq!(k.unclassed.retired_blocks, 1);
        assert_eq!(k.unclassed.retired_bytes, 256);
        assert_eq!(k.unclassed.released_blocks, 0);
        assert_counters_match_census(&c);
        c.try_advance();
        c.try_advance();
        let k = c.counters();
        assert_eq!(k.unclassed.released_blocks, 1);
        assert_eq!(k.unclassed.released_bytes, 256);
        assert!(k.classes.iter().all(|kc| kc.retired == 0));
        assert_counters_match_census(&c);
        c.drain();
    }

    /// #1310, feature `collector-census`: an unpublished block put straight
    /// on a freelist counts as recycled, and `drain` frees the bins and the
    /// freelists without counting either as a release.
    #[cfg(feature = "collector-census")]
    #[test]
    fn collector_census_counters_recycle_and_drain() {
        let c = Arc::new(Collector::new());
        let class = class_for(64, TEST_ALIGN).unwrap();
        // SAFETY: fresh global allocations, never published; the collector
        // owns each from the call on.
        unsafe {
            c.recycle_unpublished(alloc_test_block(64), 64, TEST_ALIGN);
            c.retire(alloc_test_block(64), 64, TEST_ALIGN);
            c.retire(alloc_test_block(256), 256, TEST_ALIGN);
        }
        let k = c.counters();
        assert_eq!(k.classes[class].recycled, 1);
        assert_eq!(c.census().classes[class].free.blocks, 1);
        assert_counters_match_census(&c);
        c.drain();
        let k = c.counters();
        assert_eq!(k.classes[class].released, 0, "a drain is not a release");
        assert_eq!(k.unclassed.released_blocks, 0, "a drain is not a release");
        assert_eq!(c.census().total_bytes(), 0);
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

    /// Interleaving of reader-pin / validate / writer-swap / retire / advance
    /// for values used as locators into a caller's epoch-reclaimed store (issue #1141).
    ///
    /// When `unpin_before_use == false` (the winning contract), the reader pins the
    /// caller store before sampling the index and holds the pin across its use of
    /// the record. The writer updates the index under version bracketing, retires
    /// the old record, and advances the store's epoch. Because the reader holds
    /// its pin, the store can advance at most once, and the retired record cannot
    /// be reclaimed while the reader uses it.
    ///
    /// When `unpin_before_use == true` (the loser), the reader unpins immediately
    /// after index validation. The writer's advances both succeed, reclaiming the
    /// record and clobbering its first word via the freelist link, causing the
    /// use-time assertion to fail.
    fn locator_reclamation_interleaving(unpin_before_use: bool) {
        loom::model(move || {
            let store = Arc::new(Collector::new());
            let store_reader = store.register();
            let tree_v = Arc::new(SeqVersion::new());
            let slot = Arc::new(AtomicU64::new(1)); // 1 = initial locator
            let retired = Arc::new(AtomicBool::new(false));

            let layout = Layout::from_size_align(64, 16).unwrap();
            // SAFETY: fresh test allocation.
            let ptr = NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) }).unwrap();
            const SENTINEL: u64 = 0xCAFE_BABE_DEAD_BEEF;
            // SAFETY: `ptr` was just allocated above with sufficient size and alignment.
            unsafe {
                ptr.as_ptr().cast::<u64>().write(SENTINEL);
            }

            let (store_w, tree_vw, slot_w, retired_w) = (
                Arc::clone(&store),
                Arc::clone(&tree_v),
                Arc::clone(&slot),
                Arc::clone(&retired),
            );
            let writer = loom::thread::spawn(move || {
                // Writer swap: update index under version bracket
                tree_vw.begin();
                slot_w.store(2, Ordering::Relaxed); // 2 = replacement locator
                tree_vw.end();

                // Writer retires old locator only after index update returns
                // SAFETY: `ptr` is unlinked from the index and retired to store.
                unsafe { store_w.retire(ptr, 64, 16) };
                retired_w.store(true, Ordering::Release);

                // Advancer advances store epoch twice
                store_w.try_advance();
                store_w.try_advance();
            });

            // Reader: pins caller store, then performs optimistic index lookup
            let mut maybe_pin = Some(store_reader.pin());
            let snap = tree_v.sample();
            let val = slot.load(Ordering::Relaxed);
            let valid = tree_v.validate(snap);

            if unpin_before_use {
                // Loser: drops store pin before use
                maybe_pin = None;
            }

            let mut violation = false;
            if valid && val == 1 {
                // Valid read returned the old locator: check that the record is intact
                // SAFETY: `ptr` is valid and remains accessible while `maybe_pin` is held.
                let rec_val = unsafe { ptr.as_ptr().cast::<u64>().read() };
                let was_retired = retired.load(Ordering::Acquire);
                if rec_val != SENTINEL || (was_retired && store.retained_bytes() != 64) {
                    violation = true;
                }
            }

            drop(maybe_pin);

            writer.join().unwrap();
            drop(store_reader);
            store.try_advance();
            store.try_advance();
            store.try_advance();
            store.drain();

            assert!(
                !violation,
                "record in caller store was reclaimed while reader still using locator"
            );
        });
    }

    #[test]
    fn loom_sync_locator_reclamation_reader_pin_protects_use() {
        locator_reclamation_interleaving(false);
    }

    #[test]
    #[should_panic(
        expected = "record in caller store was reclaimed while reader still using locator"
    )]
    fn loom_sync_locator_reclamation_unpin_before_use_fails() {
        locator_reclamation_interleaving(true);
    }

    /// One thread pins through a handle while a sibling thread pins and
    /// unpins through the *same* handle (`shared == true`) or through a
    /// handle of its own (`shared == false`), and a writer advances twice.
    ///
    /// The invariant checked is the one [`Reader::pin`] documents: a pin
    /// stays registered — the reader's `slot` is not [`INACTIVE`] — until
    /// its own [`Pin`] drops, so the epoch can advance at most once past a
    /// live pin taken at 0 (`loom_pin_blocks_second_advance`).
    ///
    /// The shared arm is a program `std` no longer accepts: `Reader` is not
    /// `Sync`, so an `Arc<Reader>` is not `Send` and `std::thread::spawn`
    /// rejects the closure that captures it. It can still be written here
    /// only because `loom::thread::spawn` places no `Send` bound on its
    /// closure.
    fn pin_survives_sibling_pin(shared: bool) {
        loom::model(move || {
            let c = Arc::new(Collector::new());
            // Pinned at epoch 0 before any other thread starts, so the
            // pinned-at epoch is known without reading the slot back.
            let reader = Arc::new(c.register());
            let pin_a = reader.pin();

            let sibling_handle = if shared {
                Arc::clone(&reader)
            } else {
                Arc::new(c.register())
            };
            let sibling = loom::thread::spawn(move || {
                let _pin_b = sibling_handle.pin();
            });
            let cw = Arc::clone(&c);
            let writer = loom::thread::spawn(move || {
                cw.try_advance();
                cw.try_advance();
            });
            sibling.join().unwrap();
            writer.join().unwrap();

            // `pin_a` is still alive here.
            let slot = reader.slot.load(Ordering::SeqCst);
            let now = c.epoch.load(Ordering::SeqCst);
            assert!(
                slot != INACTIVE && now <= 1,
                "a live pin taken at epoch 0 lost its registration: slot {slot:#x} \
                 (INACTIVE is {INACTIVE:#x}), epoch now {now} (must be <= 1)"
            );
            drop(pin_a);
        });
    }

    /// One handle per thread, which is the only arrangement the type system
    /// admits: the first thread's pin holds across every interleaving.
    #[test]
    fn loom_separate_reader_handles_sibling_pin_drop_keeps_pin() {
        pin_survives_sibling_pin(false);
    }

    /// Why `Reader` must stay `!Sync`. Shared by reference, the sibling's
    /// `Pin::drop` clears the first thread's live pin, and this model finds
    /// that schedule. The pin protocol did not change with the fix: the
    /// marker on `Reader` is all that keeps this program out of safe code,
    /// and the guards for the marker are `assert_not_sync!` and the
    /// `compile_fail` doctests on the handle types.
    ///
    /// The expected panic is matched on the invariant's own message, so a
    /// build error or an unrelated panic does not pass for it (AGENTS.md §5).
    /// If this test starts failing because the model *passes*, pins have
    /// become shareable and the `!Sync` marker can be reconsidered.
    #[test]
    #[should_panic(expected = "a live pin taken at epoch 0 lost its registration")]
    fn loom_shared_reader_handle_sibling_pin_drop_clears_pin() {
        pin_survives_sibling_pin(true);
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

    // Ordered reads (#900). A predecessor search that finds nothing at or below
    // its target in child C backtracks and descends a sibling S to its maximum.
    // Each subtree is modelled by the one value it offers the search: C's
    // candidate (0 = none) and S's maximum. The writer inserts ORD_K_PRIME
    // into C, then ORD_M into S. The predecessor is ORD_M0 before the first
    // insert and ORD_K_PRIME from then on, so ORD_M is never the answer.
    const ORD_M0: u64 = 0x10;
    const ORD_M: u64 = 0x20;
    const ORD_K_PRIME: u64 = 0x1F0;

    fn ordered_writer(
        c_v: Arc<VersionCell>,
        c_key: Arc<AtomicU64>,
        s_v: Arc<VersionCell>,
        s_max: Arc<AtomicU64>,
    ) -> loom::thread::JoinHandle<()> {
        loom::thread::spawn(move || {
            version_begin(&c_v);
            c_key.store(ORD_K_PRIME, Ordering::Relaxed);
            version_end(&c_v);
            version_begin(&s_v);
            s_max.store(ORD_M, Ordering::Relaxed);
            version_end(&s_v);
        })
    }

    /// The negative control. Validating one cover at a time and dropping C's
    /// snapshot before descending S, as a single moving cover does, lets a
    /// search that backtracks return a key that was never the predecessor.
    /// The model must find that interleaving.
    #[test]
    #[should_panic(expected = "never the predecessor")]
    fn loom_ordered_read_hand_over_hand_is_not_enough() {
        loom::model(|| {
            let (c_v, s_v) = (Arc::new(VersionCell::new(0)), Arc::new(VersionCell::new(0)));
            let (c_key, s_max) = (
                Arc::new(AtomicU64::new(0)),
                Arc::new(AtomicU64::new(ORD_M0)),
            );
            let writer = ordered_writer(
                Arc::clone(&c_v),
                Arc::clone(&c_key),
                Arc::clone(&s_v),
                Arc::clone(&s_max),
            );
            let answer = (|| {
                let cs = node_sample(&c_v)?;
                let x = c_key.load(Ordering::Relaxed);
                if !node_validate(&c_v, cs) {
                    return None;
                }
                if x != 0 {
                    return Some(x);
                }
                let ss = node_sample(&s_v)?;
                let y = s_max.load(Ordering::Relaxed);
                node_validate(&s_v, ss).then_some(y)
            })();
            if let Some(a) = answer {
                assert!(
                    a == ORD_M0 || a == ORD_K_PRIME,
                    "ordered read returned {a:#x}, never the predecessor"
                );
            }
            writer.join().unwrap();
        });
    }

    /// Retaining every snapshot the search read, and validating all of them
    /// after the last load, returns only a key that was the predecessor at
    /// one instant: the one after every sample and before every validation.
    #[test]
    fn loom_ordered_read_retained_read_set() {
        loom::model(|| {
            let (c_v, s_v) = (Arc::new(VersionCell::new(0)), Arc::new(VersionCell::new(0)));
            let (c_key, s_max) = (
                Arc::new(AtomicU64::new(0)),
                Arc::new(AtomicU64::new(ORD_M0)),
            );
            let writer = ordered_writer(
                Arc::clone(&c_v),
                Arc::clone(&c_key),
                Arc::clone(&s_v),
                Arc::clone(&s_max),
            );
            let answer = (|| {
                let cs = node_sample(&c_v)?;
                let x = c_key.load(Ordering::Relaxed);
                if x != 0 {
                    return node_validate(&c_v, cs).then_some(x);
                }
                let ss = node_sample(&s_v)?;
                let y = s_max.load(Ordering::Relaxed);
                (node_validate(&c_v, cs) && node_validate(&s_v, ss)).then_some(y)
            })();
            if let Some(a) = answer {
                assert!(
                    a == ORD_M0 || a == ORD_K_PRIME,
                    "ordered read returned {a:#x}, never the predecessor"
                );
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
            let w1_slot = Arc::new(WriterSlot::new());
            let w2_slot = Arc::new(WriterSlot::new());
            let in_quiescence = Arc::new(AtomicBool::new(false));
            let stores_during_quiescence = Arc::new(AtomicUsize::new(0));

            let (g1, if1, q1, sq1) = (
                Arc::clone(&gate),
                Arc::clone(&w1_slot),
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
                Arc::clone(&w2_slot),
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
            while w1_slot.in_flight.load(Ordering::Relaxed) != 0
                || w2_slot.in_flight.load(Ordering::Relaxed) != 0
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

    /// The other direction of quiescence (#1295): the coordinator closes the
    /// gate, drains, reads the data as a locked fallback reads the tree, and
    /// reopens; a writer that enters after the reopen then stores to the same
    /// data. The writer takes no lock the coordinator released, so the reopen
    /// and the writer's entry check are the only edge that can order the
    /// fallback's read before the writer's store: loom reports a causality
    /// violation on the cell unless the entry check acquires the reopen.
    ///
    /// The writer starts before the close, so its first check may read the
    /// gate's initial `false` while its re-check after the fence reads the
    /// reopen's: then only the re-check reads from the reopen, and it must
    /// acquire it.
    #[test]
    fn loom_writer_entry_acquires_gate_reopen() {
        loom::model(|| {
            let gate = Arc::new(WriterGate::new());
            let slot = Arc::new(WriterSlot::new());
            let data = Arc::new(loom::cell::UnsafeCell::new(0usize));

            let (g, f, d) = (Arc::clone(&gate), Arc::clone(&slot), Arc::clone(&data));
            let w = loom::thread::spawn(move || {
                loop {
                    if let Some(_guard) = g.enter_writer(&f, 0) {
                        // SAFETY: loom's cell checks this access; the gate
                        // protocol is what the test puts under that check.
                        d.with_mut(|p| unsafe { *p += 1 });
                        return;
                    }
                    loom::thread::yield_now();
                }
            });

            gate.close();
            WriterGate::wait_drained(&slot.in_flight);
            // SAFETY: as above. The writer may have entered and left before
            // the close; the drain orders that store before this read.
            let seen = data.with(|p| unsafe { *p });
            assert!(seen <= 1);
            gate.open();
            w.join().unwrap();
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
            let slot = Arc::new(WriterSlot::new());
            let data = Arc::new(loom::cell::UnsafeCell::new(0usize));

            let (g, f, d) = (Arc::clone(&gate), Arc::clone(&slot), Arc::clone(&data));
            let w = loom::thread::spawn(move || {
                if let Some(_guard) = g.enter_writer(&f, 0) {
                    // SAFETY: loom's cell checks this access; the gate
                    // protocol is what the test puts under that check.
                    d.with_mut(|p| unsafe { *p += 1 });
                }
            });

            gate.close();
            WriterGate::wait_drained(&slot.in_flight);
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
            let slot = Arc::new(WriterSlot::new());
            let cells = Arc::new([
                loom::cell::UnsafeCell::new(0usize),
                loom::cell::UnsafeCell::new(0usize),
            ]);

            let writers: Vec<_> = (0..2)
                .map(|i| {
                    let (g, f, c) = (Arc::clone(&gate), Arc::clone(&slot), Arc::clone(&cells));
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
            WriterGate::wait_drained(&slot.in_flight);
            for c in cells.iter() {
                // SAFETY: as above.
                let seen = c.with(|p| unsafe { *p });
                assert!(seen <= 1);
            }
            for w in writers {
                w.join().unwrap();
            }
            assert_eq!(slot.in_flight.load(Ordering::Relaxed), 0);
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
                // SAFETY: `ptr` is a fresh `(64, 16)` global allocation that
                // no reader dereferences; the model only tracks its reclamation.
                unsafe { cw1.retire(ptr, 64, 16) };
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
    fn spawn_striped_retire(c: &Arc<Collector>, slot: usize) -> loom::thread::JoinHandle<()> {
        let c = Arc::clone(c);
        loom::thread::spawn(move || {
            set_writer_slot(slot);
            let layout = Layout::from_size_align(64, 16).unwrap();
            // SAFETY: non-zero size and a valid power-of-two alignment.
            let ptr = NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) }).unwrap();
            // SAFETY: `ptr` is a fresh `(64, 16)` global allocation, never published.
            unsafe { c.retire(ptr, 64, 16) };
        })
    }

    /// Two writers retire on stripes 0 and 1 while a third thread advances,
    /// all under a reader pinned before any of them start. The pin allows
    /// one advance at most, so whichever epoch each retire lands in, nothing
    /// is reclaimed until the reader unpins; after it does, every stripe is.
    #[test]
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
    // ----------------------------------------------------------------------
    // The string wrapper's per-`StrNode` cover word (Refs #929, METHODOLOGY
    // §17.5): three models on the protocol's primitives, each with the line
    // whose deletion turns it red, and the negative control §17.5 names.
    // ----------------------------------------------------------------------

    /// Two writers read a `StrNode`'s continuation entry under one cover
    /// snapshot and both try to publish over it — a split, or a value
    /// replace. `version_try_lock_expect` at that snapshot lets exactly one
    /// through; the other restarts and re-reads the entry. Red when the
    /// lock becomes a load and a store, or is deleted: both writers enter
    /// the section, and both publish.
    #[test]
    fn loom_str_cover_lock_serialises_entry_writers() {
        loom::model(|| {
            let cover = Arc::new(VersionCell::new(0));
            let inside = Arc::new(AtomicUsize::new(0));
            let published = Arc::new(AtomicUsize::new(0));
            let snap = node_sample(&cover).expect("even at start");
            let writers: Vec<_> = (0..2)
                .map(|_| {
                    let (c, i, p) = (
                        Arc::clone(&cover),
                        Arc::clone(&inside),
                        Arc::clone(&published),
                    );
                    loom::thread::spawn(move || {
                        if let Ok(old) = version_try_lock_expect(&c, snap) {
                            assert_eq!(
                                i.fetch_add(1, Ordering::Relaxed),
                                0,
                                "two writers publishing over one entry at once"
                            );
                            p.fetch_add(1, Ordering::Relaxed);
                            i.fetch_sub(1, Ordering::Relaxed);
                            version_unlock(&c, old, true);
                        }
                    })
                })
                .collect();
            for w in writers {
                w.join().unwrap();
            }
            assert_eq!(
                published.load(Ordering::Relaxed),
                1,
                "exactly one writer publishes from a snapshot; the other must restart"
            );
        });
    }

    /// The reader half of §17.2.2: a suffix value is replaced in place (T3)
    /// under the `StrNode` cover, and a reader that loaded it validates that
    /// word. The suffix is two words the writer changes together; a
    /// validated read never sees them disagree. `word_is_cover` selects
    /// which word the reader validates: the cover the writer bumps, or the
    /// tree word it no longer does.
    fn str_suffix_value_model(reader_validates_cover: bool) {
        loom::model(move || {
            let cover = Arc::new(VersionCell::new(0));
            let tree = Arc::new(VersionCell::new(0));
            let value = Arc::new(AtomicU64::new(1));
            let check = Arc::new(AtomicU64::new(1));
            let (cw, vw, kw) = (Arc::clone(&cover), Arc::clone(&value), Arc::clone(&check));
            let writer = loom::thread::spawn(move || {
                let old = version_try_lock(&cw).expect("uncontended");
                vw.store(2, Ordering::Relaxed);
                kw.store(2, Ordering::Relaxed);
                version_unlock(&cw, old, true);
            });
            let word = if reader_validates_cover {
                &cover
            } else {
                &tree
            };
            if let Some(s) = node_sample(word) {
                let v = value.load(Ordering::Relaxed);
                let k = check.load(Ordering::Relaxed);
                if node_validate(word, s) {
                    assert_eq!(
                        v, k,
                        "validated read of a suffix value is torn: value {v}, check {k}"
                    );
                }
            }
            writer.join().unwrap();
        });
    }

    /// Red when the writer's bracket around the two stores is deleted, or
    /// when the reader's final validate is.
    #[test]
    fn loom_str_suffix_value_read_validates_the_node_cover() {
        str_suffix_value_model(true);
    }

    /// The negative control §17.5 asks for: a reader that keeps validating
    /// the tree word — which the string wrapper's writers no longer bump —
    /// returns a torn suffix. Red by construction, on the same model.
    #[test]
    #[should_panic(expected = "validated read of a suffix value is torn")]
    fn loom_str_suffix_value_read_on_the_tree_word_is_torn() {
        str_suffix_value_model(false);
    }

    /// Property S3 for a pruned `StrNode` (T9): the emptied child is locked
    /// — odd, so no reader validates through it — before its parent's entry
    /// is rewritten, marked obsolete through that lock, and only then
    /// disposed. A reader that loaded the parent's entry before the unlink
    /// never returns what it read from the disposed child. Red when the
    /// child's lock and mark are deleted, so the dispose runs on an even
    /// word a reader can still validate.
    #[test]
    fn loom_str_prune_locks_the_child_before_unlinking_it() {
        loom::model(|| {
            let parent = Arc::new(VersionCell::new(0));
            let child = Arc::new(VersionCell::new(0));
            // 1: the parent's entry names the child; 0: unlinked.
            let slot = Arc::new(AtomicU64::new(1));
            let payload = Arc::new(AtomicU64::new(0xA));
            let (pw, cw, sw, dw) = (
                Arc::clone(&parent),
                Arc::clone(&child),
                Arc::clone(&slot),
                Arc::clone(&payload),
            );
            let writer = loom::thread::spawn(move || {
                let _child_lock = version_try_lock(&cw).expect("uncontended");
                let p_old = version_try_lock(&pw).expect("uncontended");
                sw.store(0, Ordering::Relaxed);
                version_obsolete_locked(&cw);
                dw.store(0xDEAD, Ordering::Relaxed);
                version_unlock(&pw, p_old, true);
            });
            if let Some(ps) = node_sample(&parent) {
                let s = slot.load(Ordering::Relaxed);
                if node_validate(&parent, ps) && s == 1 {
                    if let Some(cs) = node_sample(&child) {
                        let v = payload.load(Ordering::Relaxed);
                        if node_validate(&child, cs) {
                            assert_ne!(
                                v, 0xDEAD,
                                "validated read through a pruned node returned its disposed contents"
                            );
                        }
                    }
                }
            }
            writer.join().unwrap();
        });
    }

    /// Where a conditional publish takes its compare from.
    #[derive(Clone, Copy)]
    enum CasCompare {
        /// Re-read under the parent's version lock: `compare_exchange`'s
        /// publish sites.
        UnderLock,
        /// Read before the lock, which is then taken against the snapshot
        /// the read was made under: the conditional removal's form.
        BeforeLockExpectingSnapshot,
        /// Read before a lock that expects nothing. The negative control.
        BeforeLockExpectingNothing,
    }

    /// One conditional publish of `new` over `expected`; `Ok` is a store.
    fn cas_publish(
        v: &VersionCell,
        word: &AtomicU64,
        expected: u64,
        new: u64,
        how: CasCompare,
    ) -> Result<u64, u64> {
        loop {
            let Some(snap) = node_sample(v) else {
                loom::thread::yield_now();
                continue;
            };
            let seen = word.load(Ordering::Relaxed);
            if !node_validate(v, snap) {
                loom::thread::yield_now();
                continue;
            }
            if !matches!(how, CasCompare::UnderLock) && seen != expected {
                return Err(seen);
            }
            let locked = match how {
                CasCompare::BeforeLockExpectingNothing => version_try_lock(v),
                _ => version_try_lock_expect(v, snap),
            };
            let Ok(old_v) = locked else {
                loom::thread::yield_now();
                continue;
            };
            let old = word.load(Ordering::Relaxed);
            if matches!(how, CasCompare::UnderLock) && old != expected {
                version_unlock(v, old_v, false);
                return Err(old);
            }
            word.store(new, Ordering::Relaxed);
            version_unlock(v, old_v, true);
            return Ok(old);
        }
    }

    /// Two writers publish over the same expected word on one key.
    fn cas_two_writers_model(how: CasCompare) {
        loom::model(move || {
            let node_v = Arc::new(VersionCell::new(0));
            let word = Arc::new(AtomicU64::new(7));
            let (v1, w1) = (Arc::clone(&node_v), Arc::clone(&word));
            let t = loom::thread::spawn(move || cas_publish(&v1, &w1, 7, 100, how));
            let mine = cas_publish(&node_v, &word, 7, 200, how);
            let theirs = t.join().unwrap();
            match (mine, theirs) {
                (Ok(7), Err(seen)) => assert_eq!(seen, 200, "the loser saw a stale word"),
                (Err(seen), Ok(7)) => assert_eq!(seen, 100, "the loser saw a stale word"),
                other => panic!("conditional publish lost an update: {other:?}"),
            }
        });
    }

    /// Exactly one of two conditional publishes over one expected word
    /// stores, and the other observes the winner's word. Red when the
    /// compare under the lock is deleted.
    #[test]
    fn loom_compare_exchange_one_winner_under_the_version_lock() {
        cas_two_writers_model(CasCompare::UnderLock);
    }

    /// The conditional removal compares before it locks; it is sound because
    /// the lock expects the snapshot the compare was read under.
    #[test]
    fn loom_compare_exchange_one_winner_comparing_before_an_expecting_lock() {
        cas_two_writers_model(CasCompare::BeforeLockExpectingSnapshot);
    }

    /// The negative control: a compare trusted across a lock that expects
    /// nothing lets both writers store.
    #[test]
    #[should_panic(expected = "conditional publish lost an update")]
    fn loom_compare_exchange_compare_outside_the_lock_loses_an_update() {
        cas_two_writers_model(CasCompare::BeforeLockExpectingNothing);
    }

    /// One conditional removal; unlinks (stores 0) only if word matches expected.
    fn cas_remove(v: &VersionCell, word: &AtomicU64, expected: u64) -> Result<u64, u64> {
        loop {
            let Some(snap) = node_sample(v) else {
                loom::thread::yield_now();
                continue;
            };
            let seen = word.load(Ordering::Relaxed);
            if !node_validate(v, snap) {
                loom::thread::yield_now();
                continue;
            }
            if seen != expected {
                return Err(seen);
            }
            let Ok(old_v) = version_try_lock_expect(v, snap) else {
                loom::thread::yield_now();
                continue;
            };
            let old = word.load(Ordering::Relaxed);
            if old != expected {
                version_unlock(v, old_v, false);
                return Err(old);
            }
            word.store(0, Ordering::Relaxed);
            version_unlock(v, old_v, true);
            return Ok(old);
        }
    }

    /// CAS publish races a concurrent removal on the same key: either CAS wins
    /// (remove sees the new word and fails) or remove wins (CAS sees 0 and fails),
    /// with zero lost updates.
    #[test]
    fn loom_cas_vs_concurrent_remove() {
        loom::model(|| {
            let node_v = Arc::new(VersionCell::new(0));
            let word = Arc::new(AtomicU64::new(7));
            let (v1, w1) = (Arc::clone(&node_v), Arc::clone(&word));
            let t = loom::thread::spawn(move || cas_remove(&v1, &w1, 7));
            let cas_res = cas_publish(&node_v, &word, 7, 100, CasCompare::UnderLock);
            let rem_res = t.join().unwrap();
            match (cas_res, rem_res) {
                (Ok(7), Err(seen)) => {
                    assert_eq!(seen, 100, "removal must observe the CAS winner's new word");
                    assert_eq!(word.load(Ordering::Relaxed), 100);
                }
                (Err(seen), Ok(7)) => {
                    assert_eq!(seen, 0, "CAS must observe removal having cleared the word");
                    assert_eq!(word.load(Ordering::Relaxed), 0);
                }
                other => panic!("cas vs concurrent remove lost an update: {other:?}"),
            }
        });
    }

    /// `sync::null_branch_u_slot` reduced to its words (Refs #1079): a
    /// `BranchU`'s slots, each 0 (null) or non-zero, and the branch's
    /// version. [`FLOOR`] stands in for `BRANCHU_TO_B_DOWN`.
    const FLOOR: usize = 2;
    const SLOTS: usize = FLOOR + 2;

    /// What one optimistic null store did.
    #[derive(Debug, PartialEq)]
    enum NullStore {
        Stored,
        Retry,
        DemoteU,
    }

    /// One attempt, no retry loop: sample the branch, count its slots before
    /// any lock (then the acquire fence), fall back when nulling slot `i`
    /// would leave the branch at the floor, lock, re-check the slot, store.
    /// `expect_snapshot` false takes the lock against no snapshot, which is
    /// the negative control: the count is then never validated.
    fn null_store(
        v: &VersionCell,
        slots: &[AtomicU64],
        i: usize,
        expect_snapshot: bool,
    ) -> NullStore {
        let Some(snap) = node_sample(v) else {
            return NullStore::Retry;
        };
        let seen = slots[i].load(Ordering::Acquire);
        if seen == 0 || !node_validate(v, snap) {
            return NullStore::Retry;
        }
        let count = slots
            .iter()
            .filter(|s| s.load(Ordering::Acquire) != 0)
            .count();
        fence(Ordering::Acquire);
        if count - 1 <= FLOOR {
            return NullStore::DemoteU;
        }
        let locked = if expect_snapshot {
            version_try_lock_expect(v, snap)
        } else {
            version_try_lock(v)
        };
        let Ok(old_v) = locked else {
            return NullStore::Retry;
        };
        if slots[i].load(Ordering::Relaxed) != seen {
            version_unlock(v, old_v, false);
            return NullStore::Retry;
        }
        slots[i].store(0, Ordering::Release);
        version_unlock(v, old_v, true);
        NullStore::Stored
    }

    /// Two writers null two different slots of a branch one digit above the
    /// point where either store alone is allowed and both together are not.
    fn branch_u_floor_model(expect_snapshot: bool) {
        loom::model(move || {
            let v = Arc::new(VersionCell::new(0));
            let slots: Arc<[AtomicU64; SLOTS]> =
                Arc::new(core::array::from_fn(|_| AtomicU64::new(1)));
            let (v1, s1) = (Arc::clone(&v), Arc::clone(&slots));
            let t = loom::thread::spawn(move || null_store(&v1, &s1[..], 0, expect_snapshot));
            let mine = null_store(&v, &slots[..], 1, expect_snapshot);
            let theirs = t.join().unwrap();
            let left = slots
                .iter()
                .filter(|s| s.load(Ordering::Relaxed) != 0)
                .count();
            assert!(
                left > FLOOR,
                "optimistic stores left the branch at its floor ({mine:?}, {theirs:?})"
            );
            let stored = [&mine, &theirs]
                .iter()
                .filter(|o| ***o == NullStore::Stored)
                .count();
            assert_eq!(left, SLOTS - stored, "a store was lost or invented");
        });
    }

    /// Two optimistic null stores on one branch never take it to its floor:
    /// the lock expects the snapshot the count was read under, so the
    /// second writer's count is rejected once the first has stored.
    #[test]
    fn loom_branch_u_null_stores_stay_above_the_floor() {
        branch_u_floor_model(true);
    }

    /// The negative control: a lock that expects no snapshot leaves the
    /// count unvalidated, and both writers store.
    #[test]
    #[should_panic(expected = "optimistic stores left the branch at its floor")]
    fn loom_branch_u_null_stores_without_the_snapshot_reach_the_floor() {
        branch_u_floor_model(false);
    }

    /// The bytes wrapper's two bucket writers (#929), reduced to the two
    /// words they contend on: `word` is the trie slot naming the published
    /// bucket, and each bucket is one value word.
    ///
    /// - The **in-place publish** stores into the bucket `word` still names,
    ///   under the parent's version lock, leaving `word` alone.
    /// - The **replacement** copies the published bucket's value *outside*
    ///   the lock (that is where the real path clones keys and allocates),
    ///   then takes the lock, compares, and stores the new bucket.
    ///
    /// `refresh` is `refresh_replacement_values`: re-reading the value word
    /// under the lock, immediately before the publish. Without it the copy
    /// is stale for exactly as long as the allocation takes.
    fn bucket_two_writers_model(refresh: bool) {
        const OLD_BUCKET: u64 = 1;
        const NEW_BUCKET: u64 = 2;
        const INIT: u64 = 7;
        const OVERWRITTEN: u64 = 9;

        loom::model(move || {
            let node_v = Arc::new(VersionCell::new(0));
            let word = Arc::new(AtomicU64::new(OLD_BUCKET));
            let old_bucket = Arc::new(AtomicU64::new(INIT));
            let new_bucket = Arc::new(AtomicU64::new(0));

            // The in-place value publish.
            let (v1, w1, b1) = (
                Arc::clone(&node_v),
                Arc::clone(&word),
                Arc::clone(&old_bucket),
            );
            let inplace = loom::thread::spawn(move || {
                loop {
                    let Some(snap) = node_sample(&v1) else {
                        loom::thread::yield_now();
                        continue;
                    };
                    let Ok(old_v) = version_try_lock_expect(&v1, snap) else {
                        loom::thread::yield_now();
                        continue;
                    };
                    let seen = w1.load(Ordering::Relaxed);
                    if seen != OLD_BUCKET {
                        // The bucket moved: nothing is stored, and the
                        // caller re-reads it. Not a lost update.
                        version_unlock(&v1, old_v, false);
                        return Err(seen);
                    }
                    b1.store(OVERWRITTEN, Ordering::Relaxed);
                    version_unlock(&v1, old_v, false);
                    return Ok(());
                }
            });

            // The replacement: the copy happens before the lock.
            let mut copied = old_bucket.load(Ordering::Relaxed);
            let replaced = loop {
                let Some(snap) = node_sample(&node_v) else {
                    loom::thread::yield_now();
                    continue;
                };
                let Ok(old_v) = version_try_lock_expect(&node_v, snap) else {
                    loom::thread::yield_now();
                    continue;
                };
                let seen = word.load(Ordering::Relaxed);
                if seen != OLD_BUCKET {
                    version_unlock(&node_v, old_v, false);
                    break Err(seen);
                }
                if refresh {
                    copied = old_bucket.load(Ordering::Relaxed);
                }
                new_bucket.store(copied, Ordering::Relaxed);
                word.store(NEW_BUCKET, Ordering::Relaxed);
                version_unlock(&node_v, old_v, true);
                break Ok(());
            };

            let stored_in_place = inplace.join().unwrap().is_ok();
            let published = word.load(Ordering::Relaxed);
            let live = if published == NEW_BUCKET {
                new_bucket.load(Ordering::Relaxed)
            } else {
                old_bucket.load(Ordering::Relaxed)
            };
            let want = if stored_in_place { OVERWRITTEN } else { INIT };
            assert_eq!(
                live,
                want,
                "bucket replacement dropped an acknowledged in-place value \
                 (in place: {stored_in_place}, replaced: {:?})",
                replaced.is_ok()
            );
        });
    }

    /// An in-place value publish acknowledged before a bucket replacement
    /// survives it: the replacement re-reads the value word under the same
    /// version lock the publish took.
    #[test]
    fn loom_bucket_inplace_value_survives_a_replacement() {
        bucket_two_writers_model(true);
    }

    /// The negative control: without the re-read under the lock, the value
    /// copied while the replacement was being built is stale and the
    /// acknowledged overwrite is lost.
    #[test]
    #[should_panic(expected = "dropped an acknowledged in-place value")]
    fn loom_bucket_replacement_without_the_refresh_loses_an_overwrite() {
        bucket_two_writers_model(false);
    }

    /// The bytes wrapper's removal against the in-place publish (Refs
    /// #1047), reduced to the same two words as
    /// [`bucket_two_writers_model`]. The removal unlinks the bucket — it
    /// stores `REMOVED` over `word` under the parent's version lock — and
    /// must return the value the key held at that moment.
    ///
    /// `early` reads that value **before** the lock loop, which is where
    /// the remove body's conditional compare sits. That is not safe, and
    /// the reason is the in-place publish's unlock: it stores no trie word,
    /// so it unlocks *unmodified* and the version returns to the value the
    /// removal sampled. `version_try_lock_expect` therefore still succeeds
    /// across a completed, acknowledged overwrite, and the value read
    /// before it is stale. Reading after the unlink is what makes the word
    /// final: no writer can reach a bucket the trie no longer names.
    fn bucket_removal_model(early: bool) {
        const OLD_BUCKET: u64 = 1;
        const REMOVED: u64 = 0;
        const INIT: u64 = 7;
        const OVERWRITTEN: u64 = 9;

        loom::model(move || {
            let node_v = Arc::new(VersionCell::new(0));
            let word = Arc::new(AtomicU64::new(OLD_BUCKET));
            let old_bucket = Arc::new(AtomicU64::new(INIT));

            // The in-place value publish, as in `bucket_two_writers_model`:
            // it stores inside the bucket `word` still names and unlocks
            // unmodified.
            let (v1, w1, b1) = (
                Arc::clone(&node_v),
                Arc::clone(&word),
                Arc::clone(&old_bucket),
            );
            let inplace = loom::thread::spawn(move || {
                loop {
                    let Some(snap) = node_sample(&v1) else {
                        loom::thread::yield_now();
                        continue;
                    };
                    let Ok(old_v) = version_try_lock_expect(&v1, snap) else {
                        loom::thread::yield_now();
                        continue;
                    };
                    let seen = w1.load(Ordering::Relaxed);
                    if seen != OLD_BUCKET {
                        version_unlock(&v1, old_v, false);
                        return Err(seen);
                    }
                    b1.store(OVERWRITTEN, Ordering::Relaxed);
                    version_unlock(&v1, old_v, false);
                    return Ok(());
                }
            });

            // The removal.
            let mut read_early = 0u64;
            if early {
                read_early = old_bucket.load(Ordering::Relaxed);
            }
            let removed = loop {
                let Some(snap) = node_sample(&node_v) else {
                    loom::thread::yield_now();
                    continue;
                };
                let Ok(old_v) = version_try_lock_expect(&node_v, snap) else {
                    loom::thread::yield_now();
                    continue;
                };
                let seen = word.load(Ordering::Relaxed);
                if seen != OLD_BUCKET {
                    version_unlock(&node_v, old_v, false);
                    break Err(seen);
                }
                word.store(REMOVED, Ordering::Relaxed);
                version_unlock(&node_v, old_v, true);
                break Ok(());
            };

            let stored_in_place = inplace.join().unwrap().is_ok();
            let Ok(()) = removed else {
                // The bucket moved under the removal, which re-reads it.
                return;
            };
            // Unlinked: nothing can store into the bucket any more.
            let returned = if early {
                read_early
            } else {
                old_bucket.load(Ordering::Relaxed)
            };
            let want = if stored_in_place { OVERWRITTEN } else { INIT };
            assert_eq!(
                returned, want,
                "removal returned a stale value (in place: {stored_in_place})"
            );
        });
    }

    /// The removed entry's value word, read after the unlink, is the last
    /// value the key held — including an overwrite acknowledged just before
    /// the unlink.
    #[test]
    fn loom_bucket_removal_reads_the_value_the_unlink_froze() {
        bucket_removal_model(false);
    }

    /// The negative control: reading the value where the remove body's
    /// compare sits — before the expecting lock — returns a value an
    /// acknowledged in-place publish has already replaced, because that
    /// publish unlocks the terminal unmodified.
    #[test]
    #[should_panic(expected = "removal returned a stale value")]
    fn loom_bucket_removal_reading_before_the_lock_returns_a_stale_value() {
        bucket_removal_model(true);
    }

    /// `SyncExpanseBlobMap`'s dead-byte charge reduced to one slot word
    /// (#1280). The word names an arena record (`A`, then `B`) or is 0
    /// (absent). An overwrite reads the word under its version lock, stores
    /// `B` and unlocks; a removal reads the word before locking, as
    /// `olc_remove_map_body!`'s pre-lock reads do, then locks expecting that
    /// snapshot and stores 0. Each writer charges the record its unlink
    /// replaced, and the model checks the accounting that `compact` later
    /// recomputes from the index: every record that ended up unreferenced is
    /// charged exactly once. `overwrite_unlocks_modified` false is the
    /// negative control — the in-place publish shape #1055 found on the bytes
    /// map, which leaves the version where the removal sampled it.
    fn blob_dead_charge_model(overwrite_unlocks_modified: bool) {
        const A: u64 = 0xA;
        const B: u64 = 0xB;
        loom::model(move || {
            let v = Arc::new(VersionCell::new(0));
            let word = Arc::new(AtomicU64::new(A));
            let (ov, ow) = (Arc::clone(&v), Arc::clone(&word));
            let overwrite = loom::thread::spawn(move || {
                loop {
                    let Ok(old_v) = version_try_lock(&ov) else {
                        loom::thread::yield_now();
                        continue;
                    };
                    let old = ow.load(Ordering::Relaxed);
                    ow.store(B, Ordering::Relaxed);
                    version_unlock(&ov, old_v, overwrite_unlocks_modified);
                    return (old != 0).then_some(old);
                }
            });
            let removed = loop {
                let Some(snap) = node_sample(&v) else {
                    loom::thread::yield_now();
                    continue;
                };
                let seen = word.load(Ordering::Relaxed);
                if !node_validate(&v, snap) {
                    loom::thread::yield_now();
                    continue;
                }
                if seen == 0 {
                    break None;
                }
                let Ok(old_v) = version_try_lock_expect(&v, snap) else {
                    loom::thread::yield_now();
                    continue;
                };
                word.store(0, Ordering::Relaxed);
                version_unlock(&v, old_v, true);
                break Some(seen);
            };
            let overwritten = overwrite.join().unwrap();
            let live = word.load(Ordering::Relaxed);
            let charged: Vec<u64> = overwritten.into_iter().chain(removed).collect();
            for rec in [A, B] {
                let times = charged.iter().filter(|&&c| c == rec).count();
                let expected = usize::from(rec != live);
                assert_eq!(
                    times, expected,
                    "dead-byte charge miscounted record {rec:#x}: charged {charged:?}, live {live:#x}"
                );
            }
        });
    }

    /// Every unreferenced record is charged once: the overwrite unlocks
    /// modified, so a removal that sampled before it cannot lock over it.
    #[test]
    fn loom_blob_overwrite_and_removal_charge_each_record_once() {
        blob_dead_charge_model(true);
    }

    /// The negative control: an overwrite that unlocks unmodified lets the
    /// removal lock over it and charge the record it read before the lock.
    #[test]
    #[should_panic(expected = "dead-byte charge miscounted")]
    fn loom_blob_overwrite_unlocking_unmodified_double_charges() {
        blob_dead_charge_model(false);
    }

    /// Abstract concurrency protocol model of batch cursor traversal under retained
    /// parent branch version (issue #1142).
    ///
    /// Note: This is an abstract concurrency protocol model simulating hand-over-hand
    /// version validation across sibling edges under concurrent writer mutations
    /// (abstracting `SyncMapCursor::try_fill_buf` and `ReadSet::validate_all`), rather than
    /// executing the full 64-bit trie engine under Loom (which would lead to state explosion).
    ///
    /// Draining terminal leaves under the parent branch relies on the parent's
    /// version word to bracket updates to children and sibling slot allocations.
    /// `reader_validates_parent` controls whether the reader validates the
    /// retained parent branch version before accepting the batch.
    fn cursor_retained_path_resume_model(reader_validates_parent: bool) {
        loom::model(move || {
            let parent_ver = Arc::new(VersionCell::new(0));
            let dummy_ver = Arc::new(VersionCell::new(0));
            // Two fields representing sibling consistency across cursor drain:
            let child0 = Arc::new(AtomicU64::new(10));
            let child1 = Arc::new(AtomicU64::new(20));

            let (pw, c0w, c1w) = (
                Arc::clone(&parent_ver),
                Arc::clone(&child0),
                Arc::clone(&child1),
            );
            let writer = loom::thread::spawn(move || {
                let old = version_try_lock(&pw).expect("uncontended");
                c0w.store(15, Ordering::Relaxed);
                c1w.store(25, Ordering::Relaxed);
                version_unlock(&pw, old, true);
            });

            let ver_to_validate = if reader_validates_parent {
                &parent_ver
            } else {
                &dummy_ver
            };

            if let Some(s) = node_sample(&parent_ver) {
                let v0 = child0.load(Ordering::Relaxed);
                loom::thread::yield_now();
                let v1 = child1.load(Ordering::Relaxed);
                if node_validate(ver_to_validate, s) {
                    assert!(
                        (v0 == 10 && v1 == 20) || (v0 == 15 && v1 == 25),
                        "batch cursor read across siblings is torn: ({v0}, {v1})"
                    );
                }
            }
            writer.join().unwrap();
        });
    }

    /// Verifies the abstract concurrency protocol model: retaining and validating the
    /// covering parent branch version prevents torn or inconsistent sibling reads across
    /// the batch cursor traversal.
    #[test]
    fn loom_cursor_retained_path_resume() {
        cursor_retained_path_resume_model(true);
    }

    /// Negative control: omitting retained parent branch validation lets the
    /// cursor accept a torn batch where one sibling is pre-write and one post-write.
    #[test]
    #[should_panic(expected = "batch cursor read across siblings is torn")]
    fn loom_cursor_retained_path_resume_without_parent_validation_is_torn() {
        cursor_retained_path_resume_model(false);
    }
}
