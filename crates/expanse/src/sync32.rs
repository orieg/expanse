//! Concurrent single-writer/many-reader wrappers for the 32-bit engine
//! ([`SyncExpanseMap32`], [`SyncExpanseSet32`]).
//!
//! Built for microcontroller-class targets: the whole protocol uses only
//! atomic **load/store and fences** — no compare-and-swap, no
//! read-modify-write, no locks — so it compiles and runs on targets
//! without the RISC-V A extension (`riscv32imc-unknown-none-elf`, the
//! ESP32-C2/C3). Reclamation is deferred into a fixed pending list and
//! drained at quiescent points; the arena is fixed-capacity so its
//! backing storage never moves under a reader.
//!
//! # Structure of the protocol
//!
//! - **Writer exclusion is by construction, not by lock.** [`Sync32::split`]
//!   takes `&mut self` and returns exactly one non-clonable [`Writer32`];
//!   the borrow checker guarantees no second writer can exist. There is no
//!   mutex anywhere in this module.
//! - **Reads are optimistic lock coupling** (Leis, Scheibner, Kemper &
//!   Neumann, "The ART of Practical Synchronization", DaMoN 2016) at tree
//!   granularity: the writer brackets each mutation with a seqlock
//!   ([`crate::occ32::SeqVersion32`]); a reader samples the version, walks
//!   racily, and validates after every racily-loaded pointer **before**
//!   dereferencing it, and again before trusting any result. The protocol
//!   is **blocking**, not lock-free — a reader concurrent with an open
//!   bracket reports [`Busy`] instead of spinning (see below).
//! - **Reclamation is quiescent-state-based with a per-reader walk
//!   counter**: freed nodes park in a fixed pending list. Each reader owns
//!   a padded counter it alone writes (odd while inside a walk, even
//!   outside — two plain stores per walk, never a shared counter, no
//!   read-modify-write). The writer frees a parked node once every reader
//!   has either been observed outside a walk since the node was retired or
//!   has *passed through* a quiescent state since — its counter changed —
//!   so the grace period is bounded by the longest single walk rather than
//!   by catching every reader idle at one sampled instant (#594). When
//!   every reader is outside at the moment of the check, everything
//!   pending is freed at once, which is the common case at low reader
//!   density.
//!
//! # Interrupt-handler contract
//!
//! On a single-core part, a reader that spins waiting for a writer's
//! bracket to close can never make progress if it preempted that writer —
//! there is no scheduler to run the writer, so the version word stays odd
//! forever. Every read on this surface is therefore **single-attempt and
//! bounded**: [`Reader32::try_get`]/[`Reader32::try_contains`] return
//! [`Busy`] instead of waiting, and the caller decides — an interrupt
//! handler surfaces the miss and retries on its next invocation; a main
//! loop may simply call again. Reader methods take `&mut self`, so one
//! [`Reader32`] cannot be shared between a main loop and the interrupt
//! handler that preempts it — take one reader per execution context from
//! the [`ReaderPool32`].
//!
//! # Bounded memory, declared trade
//!
//! [`Sync32::with_capacity`] pre-reserves `node_cap` arena slots and a
//! pending list, and never allocates node storage again: mutations past
//! capacity return [`WriteError::ArenaFull`] and mutations that would
//! overflow the pending list while readers are stalled return
//! [`WriteError::ReclaimBacklog`]. This is rigid preallocation — a
//! deliberate departure from the §2.1 expanse-proportional memory
//! invariant, confined to this opt-in surface where a bounded-memory
//! telemetry table is the point. The single-threaded `ExpanseMap32` /
//! `ExpanseSet32` are untouched. (Structural conversions still use
//! transient scratch allocations internally, exactly as the
//! single-threaded engine does; steady-state leaf inserts and removes
//! mutate node buffers in place (#577), and the fixed arena bounds
//! *retained* memory.)
//!
//! # No batched removal on the writer
//!
//! `ExpanseMap32::remove_range` / `ExpanseSet32::remove_range` (#578) are
//! deliberately not mirrored on [`Writer32`]: one call can retire a node
//! per touched leaf and reallocate every class-shrunk one, so its
//! allocation demand is bounded by the range's population, not by
//! [`MUTATION_HEADROOM`], and the `ensure_headroom` contract that makes a
//! refused mutation leave the tree untouched would not hold. Evict from
//! a writer with per-key `try_remove` calls, each individually
//! refusable.
//!
//! # Memory-model caveat
//!
//! Readers walk tree memory the writer may be mutating; the racy loads
//! are validated before use per the seqlock pattern (Boehm, "Can seqlocks
//! get along with programming language memory models?", MSPC 2012). A
//! reader never borrows the container: it loads the root edge and the
//! length the writer publishes as atomic words inside its bracket, and
//! resolves node handles through the arena's published slot table
//! (`trie32::PubSlot`), whose kind, address and length words the writer
//! stores on every allocation and free. What remains is node contents:
//! a reader reads leaf bytes and branch fields through references while
//! the writer edits the same nodes in place, which under the Rust memory
//! model is a data race and an aliasing violation, both classes #1086
//! names and still reachable from safe code here (#1187). The Miri census
//! records them: `sync32::map_reader_writer` and `sync32::set_reader_writer`
//! in `.github/miri-ub-sites.json`. The 64-bit `sync` module no longer makes
//! this trade (its shared accesses are atomic words and its writers use raw
//! pointers). The reclamation-fence
//! construction (reader: store the odd counter then `SeqCst` fence then
//! sample; writer: mutate/unlink, close bracket, `SeqCst` fence, then
//! load the counters) mirrors the store-buffer pairing the 64-bit `occ`
//! module model-checked with loom: if the writer's snapshot misses a
//! reader's pin store, that reader's subsequent sample is fence-ordered
//! after the writer's bracket close, so it walks the post-unlink tree and
//! cannot reach a pending allocation. The grace-period step rests on the
//! same total order of `SeqCst` fences: the writer frees a node parked
//! before snapshot fence `F1` once a later check fence `F2` observes every
//! reader either even at `F1` or with a counter different from its `F1`
//! value. A reader whose counter changed left the walk it was in at `F1`;
//! any walk it started afterwards began with a pin store and fence that
//! cannot precede `F1` in the fence order (or the snapshot would have read
//! the later counter), so that walk sampled the version after the unlink
//! and the bracket close, and cannot reach the node either.
//!
//! In-place mutation covers linear leaves and the bitmap/branch subarray
//! stores alike: linear-leaf inserts, removes, and value overwrites shift
//! or store into the leaf's byte buffer in place while the population
//! stays inside its capacity class (#577), and a bitmap node's rank-ordered
//! subarray does the same (#615) — it is allocated at `cap_class` of its
//! population, so a growth or shrink inside the class shifts the entries
//! rather than replacing the box. A validated reader may therefore observe
//! a mid-shift subarray, or read one of the trailing spare slots, from the
//! very array it is scanning. That is safe under the same argument as every
//! other racy load here: the spare slots always hold initialised filler
//! (`0` for values, a null `Edge32` for children) so nothing uninitialised
//! is ever observable; leaf buffers hold plain bytes (never pointers);
//! every content-derived index on the validated walks is bounds-checked
//! against the live allocation; and the version seal rejects any read that
//! overlapped a write bracket before its result can escape — including one
//! that resolved a child edge out of a spare slot, since only a concurrent
//! mutation can put a rank there. Node and subarray replacement — with the
//! old allocation retired for stalled readers — now happens only at
//! capacity-class boundaries and structural conversions.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering, fence};

use core_alloc::{boxed::Box, vec::Vec};

use crate::map32::ExpanseMap32;
use crate::occ32::SeqVersion32;
use crate::set32::ExpanseSet32;
use crate::trie32::{self, Arena, PubTable, Seek32, Torn};
use crate::types32::{Edge32, Key32, Value32};

/// Worst-case arena allocations (and retirements) a single mutation may
/// perform; [`Sync32::with_capacity`] requires at least this many free
/// slots before every mutation. Re-exported so capacity planning can
/// account for it.
pub const MUTATION_HEADROOM: usize = trie32::MUTATION_HEADROOM;

/// A read attempt coincided with an open writer bracket (or observed torn
/// data) and was abandoned without waiting. Retry when convenient; an
/// interrupt handler should surface this and retry on its next run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Busy;

/// A mutation was refused before touching the tree (the tree is unchanged).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteError {
    /// Fewer than [`MUTATION_HEADROOM`] free node slots remain.
    ArenaFull,
    /// The pending-reclamation list is nearly full and at least one reader
    /// is still inside a walk, so it cannot be drained. Retry after the
    /// stalled reader finishes.
    ReclaimBacklog,
}

/// One reader's walk counter, padded to its own cache line so concurrent
/// readers never false-share (§2.1 invariant 4; 64 B covers both the
/// 64-byte server lines and the 32-byte embedded lines).
#[repr(align(64))]
struct ReaderSlot {
    /// Odd while the owning [`Reader32`] is inside a walk, even outside;
    /// written only by that reader, with plain stores.
    walk: AtomicU32,
}

/// The tree-level seqlock, padded so the word every reader polls does not
/// share a cache line with the container it guards.
#[repr(align(64))]
struct PaddedVersion(SeqVersion32);

/// Containers `Sync32` can wrap (sealed to the 32-bit engines).
pub trait Container32: sealed::Sealed {}

mod sealed {
    use super::{Arena, Edge32, ExpanseMap32, ExpanseSet32};

    pub trait Sealed: Send + Sync {
        fn with_fixed_arena(node_cap: usize, pending_cap: usize) -> Self;
        fn arena(&self) -> &Arena;
        fn arena_mut(&mut self) -> &mut Arena;
        fn root_edge(&self) -> Edge32;
        fn len(&self) -> usize;
    }

    impl Sealed for ExpanseMap32 {
        fn with_fixed_arena(n: usize, p: usize) -> Self {
            ExpanseMap32::with_fixed_arena(n, p)
        }
        fn arena(&self) -> &Arena {
            self.arena()
        }
        fn arena_mut(&mut self) -> &mut Arena {
            self.arena_mut()
        }
        fn root_edge(&self) -> Edge32 {
            self.root_edge()
        }
        fn len(&self) -> usize {
            self.len()
        }
    }

    impl Sealed for ExpanseSet32 {
        fn with_fixed_arena(n: usize, p: usize) -> Self {
            ExpanseSet32::with_fixed_arena(n, p)
        }
        fn arena(&self) -> &Arena {
            self.arena()
        }
        fn arena_mut(&mut self) -> &mut Arena {
            self.arena_mut()
        }
        fn root_edge(&self) -> Edge32 {
            self.root_edge()
        }
        fn len(&self) -> usize {
            self.len()
        }
    }
}

impl Container32 for ExpanseMap32 {}
impl Container32 for ExpanseSet32 {}

/// A single-writer/many-reader shell around a 32-bit container. See the
/// module docs for the protocol; use the [`SyncExpanseMap32`] /
/// [`SyncExpanseSet32`] aliases.
pub struct Sync32<T: Container32> {
    inner: UnsafeCell<T>,
    version: PaddedVersion,
    readers: Box<[ReaderSlot]>,
    /// What readers load instead of touching `inner` (#1187): the root edge
    /// and the length as the writer publishes them inside its bracket, and
    /// the arena's published slot table. A reader never forms a reference
    /// to the container the writer holds `&mut` to.
    root: PubEdge,
    len: AtomicUsize,
    table: PubTable,
}

/// An [`Edge32`] as two atomic words: the node word, then the aux bytes and
/// the tag. Stored by the writer inside its bracket; a reader's torn pair is
/// discarded by the version check.
struct PubEdge([AtomicU32; 2]);

impl PubEdge {
    fn new(e: Edge32) -> Self {
        let (w0, w1) = Self::words(e);
        Self([AtomicU32::new(w0), AtomicU32::new(w1)])
    }

    fn words(e: Edge32) -> (u32, u32) {
        let a = e.aux_raw();
        (
            e.w0_raw(),
            u32::from_le_bytes([a[0], a[1], a[2], e.raw_tag()]),
        )
    }

    fn store(&self, e: Edge32) {
        let (w0, w1) = Self::words(e);
        self.0[0].store(w0, Ordering::Relaxed);
        self.0[1].store(w1, Ordering::Relaxed);
    }

    fn load(&self) -> Edge32 {
        let w0 = self.0[0].load(Ordering::Relaxed);
        let [a0, a1, a2, tag] = self.0[1].load(Ordering::Relaxed).to_le_bytes();
        Edge32::from_parts(w0, [a0, a1, a2], tag)
    }
}

// SAFETY: shared access is governed by the module's protocol — exactly one
// `Writer32` can exist (enforced by `split(&mut self)` + a non-clonable
// handle), all mutation happens inside its seqlock bracket, readers only
// perform validated optimistic reads, and freed memory outlives readers
// via the deferred pending list. The residual racy-read caveat is
// documented in the module docs.
unsafe impl<T: Container32> Sync for Sync32<T> {}

/// The concurrent single-writer/many-reader 32-bit ordered map.
pub type SyncExpanseMap32 = Sync32<ExpanseMap32>;
/// The concurrent single-writer/many-reader 32-bit ordered set.
pub type SyncExpanseSet32 = Sync32<ExpanseSet32>;

impl<T: Container32> Sync32<T> {
    /// Creates a wrapper with a fixed arena of `node_cap` slots and room
    /// for `max_readers` concurrent reader handles.
    ///
    /// # Panics
    ///
    /// Panics if `node_cap < MUTATION_HEADROOM` — such a wrapper could
    /// never accept a mutation, which is a configuration error better
    /// reported at construction than as an eternal `ArenaFull`.
    #[must_use]
    pub fn with_capacity(node_cap: usize, max_readers: usize) -> Self {
        assert!(
            node_cap >= MUTATION_HEADROOM,
            "node_cap must be at least MUTATION_HEADROOM ({MUTATION_HEADROOM})"
        );
        // Pending must be able to park every live node plus one mutation's
        // worth of churn, so the arena always saturates before the pending
        // list can (ReclaimBacklog then only signals a stalled reader).
        let pending_cap = node_cap + 2 * MUTATION_HEADROOM;
        let mut slots = Vec::with_capacity(max_readers);
        slots.resize_with(max_readers, || ReaderSlot {
            walk: AtomicU32::new(0),
        });
        let inner = T::with_fixed_arena(node_cap, pending_cap);
        let table = inner
            .arena()
            .published()
            .expect("a fixed arena publishes its slot table");
        Self {
            root: PubEdge::new(inner.root_edge()),
            len: AtomicUsize::new(inner.len()),
            table,
            inner: UnsafeCell::new(inner),
            version: PaddedVersion(SeqVersion32::new()),
            readers: slots.into_boxed_slice(),
        }
    }

    /// True while any reader handle is inside a walk. A destructor that
    /// runs with a pinned reader is a use-after-free in waiting; the C
    /// surface asserts on this in debug builds before freeing.
    #[must_use]
    pub fn any_reader_pinned(&self) -> bool {
        fence(Ordering::SeqCst);
        self.readers
            .iter()
            .any(|slot| slot.walk.load(Ordering::Acquire) & 1 == 1)
    }

    /// Splits into the unique writer handle and the reader-handle pool.
    ///
    /// `&mut self` proves no other handles exist, and the returned
    /// [`Writer32`] is not clonable, so single-writer exclusion is a
    /// compile-time property — no lock, no atomic RMW.
    pub fn split(&mut self) -> (Writer32<'_, T>, ReaderPool32<'_, T>) {
        let this: &Self = self;
        let n = this.readers.len();
        (
            Writer32 {
                owner: this,
                snapshot: core_alloc::vec![0u32; n].into_boxed_slice(),
                snapshot_version: 0,
                sealed: 0,
            },
            ReaderPool32 {
                owner: this,
                next: 0,
            },
        )
    }
}

/// Hands out at most `max_readers` reader handles. `take` requires
/// `&mut self`, so claims are serialized by the borrow checker — no CAS.
pub struct ReaderPool32<'a, T: Container32> {
    owner: &'a Sync32<T>,
    next: usize,
}

impl<'a, T: Container32> ReaderPool32<'a, T> {
    /// Claims the next reader slot, or `None` when all are taken.
    pub fn take(&mut self) -> Option<Reader32<'a, T>> {
        if self.next < self.owner.readers.len() {
            let idx = self.next;
            self.next += 1;
            Some(Reader32 {
                owner: self.owner,
                idx,
            })
        } else {
            None
        }
    }
}

/// A pinned optimistic reader. One per execution context (methods take
/// `&mut self`, so a main loop and the interrupt handler that can preempt
/// it must each hold their own).
pub struct Reader32<'a, T: Container32> {
    owner: &'a Sync32<T>,
    idx: usize,
}

impl<T: Container32> Reader32<'_, T> {
    /// Runs `walk` with this reader pinned: the pending list cannot be
    /// drained while it executes, so racily-reached allocations stay live.
    #[inline]
    fn pinned<R>(&mut self, walk: impl FnOnce(&Sync32<T>) -> Result<R, Busy>) -> Result<R, Busy> {
        let slot = &self.owner.readers[self.idx];
        // A handle re-entered by a preempting context (an interrupt handler
        // sharing the main loop's reader) would unpin the outer walk when
        // it finishes, letting reclamation free memory the outer walk still
        // dereferences. On one hart the preemptor sees the outer store, so
        // this is exact proof of that misuse; it is the one part of the
        // one-handle-per-context contract that can be checked at all.
        let v = slot.walk.load(Ordering::Relaxed);
        debug_assert!(
            v & 1 == 0,
            "sync32 reader handle re-entered while inside a walk"
        );
        // Only this reader writes its counter, so load-then-store is the
        // increment: no read-modify-write (the primary target has none).
        slot.walk.store(v.wrapping_add(1), Ordering::Relaxed);
        // Pairs with the writer's snapshot/check fence: if the writer misses
        // this pin, our sample below is fence-ordered after its bracket
        // close and we walk the post-unlink tree (see module docs).
        fence(Ordering::SeqCst);
        let out = walk(self.owner);
        // Release: our loads from tree memory complete before the unpin
        // becomes visible to the reclaiming writer.
        slot.walk.store(v.wrapping_add(2), Ordering::Release);
        out
    }
}

/// The unique mutating handle. Not clonable; obtained once per
/// [`Sync32::split`].
pub struct Writer32<'a, T: Container32> {
    owner: &'a Sync32<T>,
    /// Every reader's walk counter as loaded when the current sealed
    /// prefix of the pending list was sealed (writer-private).
    snapshot: Box<[u32]>,
    /// The tree version at that snapshot, for the wrap assertion.
    snapshot_version: u32,
    /// How many of the oldest pending entries the snapshot covers: they
    /// were all retired before the snapshot fence. Zero when unsealed.
    sealed: usize,
}

impl<T: Container32> Writer32<'_, T> {
    /// The wrapped container, shared. Sound: `self` is the only handle
    /// that ever forms `&mut`, and it is not doing so now.
    #[inline]
    fn inner(&self) -> &T {
        // SAFETY: see above — unique writer, no `&mut` outstanding.
        unsafe { &*self.owner.inner.get() }
    }

    /// The wrapped container, exclusive. Sound for the same reason;
    /// concurrent readers only perform the documented validated racy
    /// reads and never form references that outlive a validation.
    #[inline]
    fn inner_mut(&mut self) -> &mut T {
        // SAFETY: see the method docs and the module memory-model caveat.
        unsafe { &mut *self.owner.inner.get() }
    }

    /// Refuses a mutation whose worst case could not complete, per the
    /// fail-loud contract: the check happens *before* the tree is touched,
    /// so a refused mutation leaves the container untouched.
    fn ensure_headroom(&mut self) -> Result<(), WriteError> {
        if self.inner().arena().pending_spare() < MUTATION_HEADROOM {
            self.try_reclaim();
            if self.inner().arena().pending_spare() < MUTATION_HEADROOM {
                return Err(WriteError::ReclaimBacklog);
            }
        }
        if self.inner().arena().free_slots() < MUTATION_HEADROOM {
            return Err(WriteError::ArenaFull);
        }
        Ok(())
    }

    /// Drains the pending-reclamation list if every reader is currently
    /// outside a walk; returns whether the list is now empty. Runs
    /// automatically after mutations; exposed for proactive draining.
    pub fn try_reclaim(&mut self) -> bool {
        if self.inner().arena().pending_len() == 0 {
            self.sealed = 0;
            return true;
        }
        // Pairs with the readers' post-pin fence (module docs). One fence
        // serves both the grace-period check on the sealed prefix and the
        // fresh snapshot taken below.
        fence(Ordering::SeqCst);
        let readers = &self.owner.readers;
        let all_outside = readers
            .iter()
            .all(|slot| slot.walk.load(Ordering::Acquire) & 1 == 0);
        if all_outside {
            // Every reader is outside a walk right now: nothing parked can
            // still be referenced. The common case at low reader density.
            self.inner_mut().arena_mut().drain_pending();
            self.sealed = 0;
            return true;
        }
        if self.sealed > 0 {
            // A reader still inside the very walk it was in at the snapshot
            // (odd then, same value now) may hold a sealed node. Any other
            // reader has passed through a quiescent state since.
            let mut elapsed = true;
            for (slot, &snap) in readers.iter().zip(self.snapshot.iter()) {
                if snap & 1 == 1 && slot.walk.load(Ordering::Acquire) == snap {
                    elapsed = false;
                    break;
                }
            }
            if elapsed {
                let n = self.sealed;
                self.inner_mut().arena_mut().drain_pending_prefix(n);
                self.sealed = 0;
            } else {
                // A reader pinned across a full version wrap would validate a
                // torn walk as consistent; treat 2^30 brackets under one pin
                // as the fault it is, in debug builds.
                let now = self.owner.version.0.try_sample().unwrap_or(0);
                debug_assert!(
                    now.wrapping_sub(self.snapshot_version) < (1 << 30),
                    "sync32 reader pinned across a version wrap"
                );
            }
        }
        let remaining = self.inner().arena().pending_len();
        if remaining > 0 && self.sealed == 0 {
            // Seal everything retired so far under a fresh snapshot; the
            // next check frees it once the grace period has elapsed.
            for (slot, snap) in readers.iter().zip(self.snapshot.iter_mut()) {
                *snap = slot.walk.load(Ordering::Acquire);
            }
            self.snapshot_version = self.owner.version.0.try_sample().unwrap_or(0);
            self.sealed = remaining;
        }
        self.inner().arena().pending_len() == 0
    }

    /// Runs one mutation inside the version bracket.
    #[inline]
    fn write<R>(&mut self, f: impl FnOnce(&mut T) -> R) -> R {
        #[cfg(test)]
        self.inner_mut().arena_mut().reset_mutation_watermark();
        self.owner.version.0.begin();
        let r = f(self.inner_mut());
        // Republished inside the bracket, so a reader that loads either one
        // mid-change fails its validation.
        let (root, len) = (self.inner().root_edge(), self.inner().len());
        self.owner.root.store(root);
        self.owner.len.store(len, Ordering::Relaxed);
        self.owner.version.0.end();
        #[cfg(test)]
        {
            let (allocs, retires) = self.inner().arena().mutation_watermark();
            assert!(
                allocs <= MUTATION_HEADROOM && retires <= MUTATION_HEADROOM,
                "mutation exceeded MUTATION_HEADROOM: {allocs} allocs, {retires} retires"
            );
        }
        if self.inner().arena().pending_len() > 0 {
            self.try_reclaim();
        }
        r
    }

    /// Retired allocations awaiting a quiescent point.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.inner().arena().pending_len()
    }

    /// Bytes parked awaiting reclamation (not part of `mem_used`).
    #[must_use]
    pub fn pending_bytes(&self) -> usize {
        self.inner().arena().pending_bytes()
    }

    /// Free node slots remaining in the fixed arena.
    #[must_use]
    pub fn free_slots(&self) -> usize {
        self.inner().arena().free_slots()
    }
}

impl Writer32<'_, ExpanseMap32> {
    /// Inserts `key -> value`; returns the replaced value, or an error if
    /// the mutation was refused (tree untouched).
    pub fn try_insert(
        &mut self,
        key: Key32,
        value: Value32,
    ) -> Result<Option<Value32>, WriteError> {
        self.ensure_headroom()?;
        Ok(self.write(|m| m.insert(key, value)))
    }

    /// Removes `key`; returns its value, or an error if refused.
    pub fn try_remove(&mut self, key: Key32) -> Result<Option<Value32>, WriteError> {
        self.ensure_headroom()?;
        Ok(self.write(|m| m.remove(key)))
    }

    /// Point lookup through the writer (always consistent; never `Busy`).
    #[must_use]
    pub fn get(&self, key: Key32) -> Option<Value32> {
        self.inner().get(key)
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner().len()
    }

    /// True when empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner().is_empty()
    }

    /// Live heap bytes (excludes the pending list; see
    /// [`Writer32::pending_bytes`]).
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.inner().mem_used()
    }

    // Ordered reads through the writer: exact, since no other handle
    // mutates (#900). Hidden with the reader's `try_*` twins.
    /// Smallest entry.
    #[doc(hidden)]
    #[must_use]
    pub fn first(&self) -> Option<(Key32, Value32)> {
        self.inner().first()
    }

    /// Largest entry.
    #[doc(hidden)]
    #[must_use]
    pub fn last(&self) -> Option<(Key32, Value32)> {
        self.inner().last()
    }

    /// Smallest entry with key `>= bound`.
    #[doc(hidden)]
    #[must_use]
    pub fn next_at_or_after(&self, bound: Key32) -> Option<(Key32, Value32)> {
        self.inner().next_at_or_after(bound)
    }

    /// Smallest entry with key `> bound`.
    #[doc(hidden)]
    #[must_use]
    pub fn next_after(&self, bound: Key32) -> Option<(Key32, Value32)> {
        self.inner().next_after(bound)
    }

    /// Largest entry with key `<= bound`.
    #[doc(hidden)]
    #[must_use]
    pub fn prev_at_or_before(&self, bound: Key32) -> Option<(Key32, Value32)> {
        self.inner().prev_at_or_before(bound)
    }

    /// Largest entry with key `< bound`.
    #[doc(hidden)]
    #[must_use]
    pub fn prev_before(&self, bound: Key32) -> Option<(Key32, Value32)> {
        self.inner().prev_before(bound)
    }
}

impl Writer32<'_, ExpanseSet32> {
    /// Inserts `key`; returns whether it was newly inserted, or an error
    /// if the mutation was refused (tree untouched).
    pub fn try_insert(&mut self, key: Key32) -> Result<bool, WriteError> {
        self.ensure_headroom()?;
        Ok(self.write(|s| s.insert(key)))
    }

    /// Removes `key`; returns whether it was present, or an error if
    /// refused.
    pub fn try_remove(&mut self, key: Key32) -> Result<bool, WriteError> {
        self.ensure_headroom()?;
        Ok(self.write(|s| s.remove(key)))
    }

    /// Membership test through the writer (always consistent).
    #[must_use]
    pub fn contains(&self, key: Key32) -> bool {
        self.inner().contains(key)
    }

    /// Number of keys.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner().len()
    }

    /// True when empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner().is_empty()
    }

    /// Live heap bytes (excludes the pending list).
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.inner().mem_used()
    }
}

impl Reader32<'_, ExpanseMap32> {
    /// One bounded optimistic lookup. [`Busy`] means a writer bracket was
    /// open or a validation failed — retry when convenient; never spins.
    pub fn try_get(&mut self, key: Key32) -> Result<Option<Value32>, Busy> {
        self.pinned(|owner| {
            let Some(snap) = owner.version.0.try_sample() else {
                return Err(Busy);
            };
            let root = owner.root.load();
            if !owner.version.0.validate(snap) {
                return Err(Busy);
            }
            let still_valid = || owner.version.0.validate(snap);
            trie32::map_get_validated(owner.table, root, key, &still_valid).map_err(|Torn| Busy)
        })
    }

    /// One bounded optimistic length read.
    pub fn try_len(&mut self) -> Result<usize, Busy> {
        self.pinned(|owner| {
            let Some(snap) = owner.version.0.try_sample() else {
                return Err(Busy);
            };
            let n = owner.len.load(Ordering::Relaxed);
            if owner.version.0.validate(snap) {
                Ok(n)
            } else {
                Err(Busy)
            }
        })
    }

    // Ordered reads (#900), hidden while their soundness gates and
    // measurements (`docs/benchmarks/concurrency/METHODOLOGY.md` §12) are
    // outstanding.
    /// One bounded optimistic ordered read; same contract as `try_get`.
    fn try_seek(&mut self, seek: Seek32) -> Result<Option<(Key32, Value32)>, Busy> {
        self.pinned(|owner| {
            let Some(snap) = owner.version.0.try_sample() else {
                return Err(Busy);
            };
            let root = owner.root.load();
            if !owner.version.0.validate(snap) {
                return Err(Busy);
            }
            let still_valid = || owner.version.0.validate(snap);
            trie32::map_seek_validated(owner.table, root, seek, &still_valid).map_err(|Torn| Busy)
        })
    }

    /// Smallest entry, or [`Busy`]; never spins.
    #[doc(hidden)]
    pub fn try_first(&mut self) -> Result<Option<(Key32, Value32)>, Busy> {
        self.try_seek(Seek32::First)
    }

    /// Largest entry, or [`Busy`]; never spins.
    #[doc(hidden)]
    pub fn try_last(&mut self) -> Result<Option<(Key32, Value32)>, Busy> {
        self.try_seek(Seek32::Last)
    }

    /// Smallest entry with key `>= bound`, or [`Busy`]; never spins.
    #[doc(hidden)]
    pub fn try_next_at_or_after(&mut self, bound: Key32) -> Result<Option<(Key32, Value32)>, Busy> {
        self.try_seek(bound.checked_sub(1).map_or(Seek32::First, Seek32::After))
    }

    /// Smallest entry with key `> bound`, or [`Busy`]; never spins.
    #[doc(hidden)]
    pub fn try_next_after(&mut self, bound: Key32) -> Result<Option<(Key32, Value32)>, Busy> {
        self.try_seek(Seek32::After(bound))
    }

    /// Largest entry with key `<= bound`, or [`Busy`]; never spins.
    #[doc(hidden)]
    pub fn try_prev_at_or_before(
        &mut self,
        bound: Key32,
    ) -> Result<Option<(Key32, Value32)>, Busy> {
        self.try_seek(bound.checked_add(1).map_or(Seek32::Last, Seek32::Before))
    }

    /// Largest entry with key `< bound`, or [`Busy`]; never spins.
    #[doc(hidden)]
    pub fn try_prev_before(&mut self, bound: Key32) -> Result<Option<(Key32, Value32)>, Busy> {
        self.try_seek(Seek32::Before(bound))
    }
}

impl Reader32<'_, ExpanseSet32> {
    /// One bounded optimistic membership test. Never spins; see
    /// [`Reader32::<ExpanseMap32>::try_get`].
    pub fn try_contains(&mut self, key: Key32) -> Result<bool, Busy> {
        self.pinned(|owner| {
            let Some(snap) = owner.version.0.try_sample() else {
                return Err(Busy);
            };
            let root = owner.root.load();
            if !owner.version.0.validate(snap) {
                return Err(Busy);
            }
            let still_valid = || owner.version.0.validate(snap);
            trie32::set_contains_validated(owner.table, root, key, &still_valid)
                .map_err(|Torn| Busy)
        })
    }

    /// One bounded optimistic length read.
    pub fn try_len(&mut self) -> Result<usize, Busy> {
        self.pinned(|owner| {
            let Some(snap) = owner.version.0.try_sample() else {
                return Err(Busy);
            };
            let n = owner.len.load(Ordering::Relaxed);
            if owner.version.0.validate(snap) {
                Ok(n)
            } else {
                Err(Busy)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic keys with all four levels exercised (splits, branch
    /// flavours, bitmap conversions) via a plain LCG — no external PRNG.
    fn lcg_key(state: &mut u32) -> u32 {
        *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        *state
    }

    #[test]
    fn split_hands_out_one_writer_and_bounded_readers() {
        let mut m = SyncExpanseMap32::with_capacity(256, 2);
        let (_w, mut pool) = m.split();
        assert!(pool.take().is_some());
        assert!(pool.take().is_some());
        assert!(pool.take().is_none(), "pool must stop at max_readers");
    }

    #[test]
    fn map_reads_see_writes_and_misses() {
        let mut m = SyncExpanseMap32::with_capacity(4096, 1);
        let (mut w, mut pool) = m.split();
        let mut r = pool.take().unwrap();

        for k in 0u32..500 {
            assert_eq!(w.try_insert(k * 3, k).unwrap(), None);
        }
        assert_eq!(w.len(), 500);
        assert_eq!(r.try_len(), Ok(500));
        for k in 0u32..500 {
            assert_eq!(r.try_get(k * 3), Ok(Some(k)), "hit at {k}");
            assert_eq!(r.try_get(k * 3 + 1), Ok(None), "miss at {k}");
        }
        assert_eq!(w.try_remove(0).unwrap(), Some(0));
        assert_eq!(r.try_get(0), Ok(None));
    }

    #[test]
    fn open_bracket_reports_busy_never_spins() {
        let mut m = SyncExpanseMap32::with_capacity(256, 1);
        let (mut w, mut pool) = m.split();
        let mut r = pool.take().unwrap();
        w.try_insert(7, 42).unwrap();

        // Hold the tree bracket open, as a preempted writer would.
        m_version(&r).begin();
        assert_eq!(r.try_get(7), Err(Busy), "open bracket must be Busy");
        assert_eq!(r.try_len(), Err(Busy));
        m_version(&r).end();
        assert_eq!(r.try_get(7), Ok(Some(42)));
    }

    /// Test-only access to the shared version word.
    fn m_version<'a, T: Container32>(r: &Reader32<'a, T>) -> &'a SeqVersion32 {
        &r.owner.version.0
    }

    /// Test-only access to a reader slot's pin flag through the writer's
    /// shared borrow (the `&mut` from `split` outlives the test body).
    /// A reader's walk counter, poked directly to model a reader parked
    /// inside a walk (odd) or having left it (even) without a real thread.
    fn walk_counter<'a, T: Container32>(w: &Writer32<'a, T>, idx: usize) -> &'a AtomicU32 {
        &w.owner.readers[idx].walk
    }

    #[test]
    fn arena_full_is_reported_and_tree_stays_coherent() {
        let mut m = SyncExpanseMap32::with_capacity(MUTATION_HEADROOM + 8, 1);
        let (mut w, _) = m.split();
        let mut state = 0xC0FF_EE32u32;
        let mut inserted = Vec::new();
        let mut full = false;
        for _ in 0..10_000 {
            let k = lcg_key(&mut state);
            match w.try_insert(k, k ^ 0xFFFF) {
                Ok(_) => inserted.push(k),
                Err(WriteError::ArenaFull) => {
                    full = true;
                    break;
                }
                Err(e) => panic!("unexpected error: {e:?}"),
            }
        }
        assert!(full, "a tiny arena must eventually report ArenaFull");
        // The refused mutation left the tree untouched and coherent.
        for &k in &inserted {
            assert_eq!(w.get(k), Some(k ^ 0xFFFF));
        }
        assert_eq!(w.len(), inserted.len());
    }

    #[test]
    fn stalled_reader_causes_backlog_then_reclaim_recovers() {
        let mut m = SyncExpanseMap32::with_capacity(256, 1);
        let (mut w, _) = m.split();

        // Simulate a reader parked inside a walk (e.g. a wedged task).
        walk_counter(&w, 0).store(1, Ordering::Relaxed);
        let mut state = 1u32;
        let mut backlog = false;
        for i in 0..100_000u32 {
            let k = lcg_key(&mut state) % 4096;
            let res = if i % 2 == 0 {
                w.try_insert(k, k).map(|_| ())
            } else {
                w.try_remove(k).map(|_| ())
            };
            match res {
                Ok(()) => {}
                Err(WriteError::ReclaimBacklog) => {
                    backlog = true;
                    break;
                }
                Err(WriteError::ArenaFull) => {}
            }
        }
        assert!(
            backlog,
            "churn under a stalled reader must hit ReclaimBacklog"
        );
        assert!(w.pending_len() > 0);

        walk_counter(&w, 0).store(2, Ordering::Release);
        assert!(w.try_reclaim(), "quiescent readers allow draining");
        assert_eq!(w.pending_len(), 0);
        assert_eq!(w.pending_bytes(), 0);
        w.try_insert(1, 1).expect("writes proceed after reclaim");
    }

    /// The #594 case the flag scheme could not handle: a reader that is
    /// never observed idle but keeps passing through quiescent states.
    /// Nodes sealed under a snapshot must drain once its counter has
    /// changed, even though it is inside a (new) walk at check time.
    #[test]
    fn reader_that_passes_through_quiescence_lets_sealed_nodes_drain() {
        let mut m = SyncExpanseMap32::with_capacity(4096, 2);
        let (mut w, _) = m.split();
        for k in 0..2000u32 {
            w.try_insert(k, k).unwrap();
        }
        // Reader 0 is inside walk #1 (odd) while the writer retires nodes.
        walk_counter(&w, 0).store(1, Ordering::Relaxed);
        for k in 0..1500u32 {
            w.try_remove(k).unwrap();
        }
        assert!(w.pending_len() > 0);
        // The reclaim attempts inside those removes sealed a prefix; with
        // the reader still in walk #1 nothing may drain.
        assert!(!w.try_reclaim());
        let parked = w.pending_len();
        assert!(parked > 0);
        // The reader exits walk #1 and is already inside walk #3 when the
        // writer checks: never idle at a sampled instant, yet it passed
        // through a quiescent state, so the sealed prefix is safe to free.
        walk_counter(&w, 0).store(3, Ordering::Release);
        w.try_reclaim();
        assert!(w.pending_len() < parked, "sealed prefix must drain");
        // One more pass-through frees whatever was sealed after that.
        walk_counter(&w, 0).store(5, Ordering::Release);
        w.try_reclaim();
        walk_counter(&w, 0).store(6, Ordering::Release);
        assert!(w.try_reclaim());
        assert_eq!(w.pending_len(), 0);
        assert_eq!(w.pending_bytes(), 0);
    }

    /// A reader parked in the same walk across the whole grace period
    /// blocks exactly the prefix sealed before it, and nothing else frees
    /// behind its back.
    #[test]
    fn reader_stuck_in_one_walk_blocks_the_sealed_prefix_only() {
        let mut m = SyncExpanseMap32::with_capacity(4096, 2);
        let (mut w, _) = m.split();
        for k in 0..1000u32 {
            w.try_insert(k, k).unwrap();
        }
        walk_counter(&w, 0).store(7, Ordering::Relaxed);
        for k in 0..500u32 {
            w.try_remove(k).unwrap();
        }
        assert!(!w.try_reclaim());
        let before = w.pending_len();
        assert!(!w.try_reclaim(), "unchanged odd counter: still blocked");
        assert_eq!(
            w.pending_len(),
            before,
            "nothing frees under a stuck reader"
        );
        // Reader 1 idle throughout must not unblock reader 0's walk.
        assert_eq!(walk_counter(&w, 1).load(Ordering::Relaxed), 0);
        walk_counter(&w, 0).store(8, Ordering::Release);
        assert!(w.try_reclaim());
        assert_eq!(w.pending_len(), 0);
    }

    #[test]
    fn set_surface_round_trips() {
        let mut s = SyncExpanseSet32::with_capacity(4096, 1);
        let (mut w, mut pool) = s.split();
        let mut r = pool.take().unwrap();
        for k in 0u32..300 {
            assert!(w.try_insert(k * 5).unwrap());
        }
        assert_eq!(r.try_len(), Ok(300));
        for k in 0u32..300 {
            assert_eq!(r.try_contains(k * 5), Ok(true));
            assert_eq!(r.try_contains(k * 5 + 2), Ok(false));
        }
        assert!(w.try_remove(0).unwrap());
        assert_eq!(r.try_contains(0), Ok(false));
    }

    /// Deterministic churn across all node flavours; the watermark
    /// assertion inside `Writer32::write` validates MUTATION_HEADROOM on
    /// every single mutation of this run.
    #[test]
    fn churn_validates_mutation_headroom() {
        let mut m = SyncExpanseMap32::with_capacity(8192, 1);
        let (mut w, _) = m.split();
        let mut state = 0xDEAD_BEEFu32;
        for i in 0..30_000u32 {
            // Dense low keys force bitmap conversions; wide keys force
            // deep branch ladders.
            let k = if i % 3 == 0 {
                lcg_key(&mut state) % 512
            } else {
                lcg_key(&mut state)
            };
            if i % 5 == 4 {
                let _ = w.try_remove(k);
            } else {
                let _ = w.try_insert(k, i);
            }
        }
    }
    /// Shapes for the quiescent ordered-read checks: empty, an immediate, a
    /// few keys, dense low keys (bitmap leaves), a wide spread (branch
    /// ladders), a full fan-out (uncompressed branch) and the key boundaries.
    fn ordered_shapes() -> Vec<(&'static str, Vec<u32>)> {
        let mut state = 0x0DDB_A110u32;
        core_alloc::vec![
            ("empty", Vec::new()),
            ("immediate", core_alloc::vec![7]),
            ("small", core_alloc::vec![3, 300, 70_000]),
            ("dense low", (0..3000).collect()),
            ("wide", (0..2500).map(|_| lcg_key(&mut state)).collect()),
            (
                "fan-out",
                (0..256u32)
                    .flat_map(|hi| (0..4u32).map(move |lo| (hi << 16) | lo))
                    .collect(),
            ),
            (
                "boundaries",
                core_alloc::vec![0, 1, 0xFF, 0x100, 0xFFFF, 0x1_0000, u32::MAX - 1, u32::MAX],
            ),
        ]
    }

    /// G12.3 on the 32-bit surface: with no writer running, every `try_*`
    /// ordered read answers what the writer's exact call does, and the writer
    /// matches a `BTreeMap` model, across shapes and at the key boundaries.
    #[test]
    fn ordered_reads_match_the_single_threaded_map() {
        use core_alloc::collections::BTreeMap;
        let pair = |o: Option<(&u32, &u32)>| o.map(|(k, v)| (*k, *v));
        for (name, keys) in ordered_shapes() {
            let mut m = SyncExpanseMap32::with_capacity(16_384, 1);
            let (mut w, mut pool) = m.split();
            let mut r = pool.take().unwrap();
            let mut model = BTreeMap::new();
            for &k in &keys {
                let v = k.rotate_left(9) ^ 0x5A5A;
                w.try_insert(k, v).unwrap();
                model.insert(k, v);
            }
            assert_eq!(w.first(), pair(model.first_key_value()), "{name}: first");
            assert_eq!(r.try_first(), Ok(w.first()), "{name}: try_first");
            assert_eq!(w.last(), pair(model.last_key_value()), "{name}: last");
            assert_eq!(r.try_last(), Ok(w.last()), "{name}: try_last");
            let mut state = 0x5EED_0032u32;
            let mut probes = core_alloc::vec![0, 1, u32::MAX - 1, u32::MAX];
            for &k in &keys {
                probes.extend([k, k.wrapping_sub(1), k.wrapping_add(1)]);
            }
            probes.extend((0..256).map(|_| lcg_key(&mut state)));
            for q in probes {
                let want = pair(model.range(q..).next());
                assert_eq!(
                    w.next_at_or_after(q),
                    want,
                    "{name}: next_at_or_after({q:#x})"
                );
                assert_eq!(
                    r.try_next_at_or_after(q),
                    Ok(want),
                    "{name}: try_next_at_or_after({q:#x})"
                );
                let want = q.checked_add(1).and_then(|s| pair(model.range(s..).next()));
                assert_eq!(w.next_after(q), want, "{name}: next_after({q:#x})");
                assert_eq!(
                    r.try_next_after(q),
                    Ok(want),
                    "{name}: try_next_after({q:#x})"
                );
                let want = pair(model.range(..=q).next_back());
                assert_eq!(
                    w.prev_at_or_before(q),
                    want,
                    "{name}: prev_at_or_before({q:#x})"
                );
                assert_eq!(
                    r.try_prev_at_or_before(q),
                    Ok(want),
                    "{name}: try_prev_at_or_before({q:#x})"
                );
                let want = q
                    .checked_sub(1)
                    .and_then(|s| pair(model.range(..=s).next_back()));
                assert_eq!(w.prev_before(q), want, "{name}: prev_before({q:#x})");
                assert_eq!(
                    r.try_prev_before(q),
                    Ok(want),
                    "{name}: try_prev_before({q:#x})"
                );
            }
        }
    }

    /// Every ordered read is single-attempt: an open writer bracket makes each
    /// one report `Busy` rather than spin, and closing it restores answers.
    #[test]
    fn ordered_reads_report_busy_under_an_open_bracket() {
        let mut m = SyncExpanseMap32::with_capacity(256, 1);
        let (mut w, mut pool) = m.split();
        let mut r = pool.take().unwrap();
        w.try_insert(7, 42).unwrap();

        m_version(&r).begin();
        assert_eq!(r.try_first(), Err(Busy));
        assert_eq!(r.try_last(), Err(Busy));
        assert_eq!(r.try_next_at_or_after(0), Err(Busy));
        assert_eq!(r.try_next_after(0), Err(Busy));
        assert_eq!(r.try_prev_at_or_before(u32::MAX), Err(Busy));
        assert_eq!(r.try_prev_before(u32::MAX), Err(Busy));
        m_version(&r).end();
        assert_eq!(r.try_first(), Ok(Some((7, 42))));
        assert_eq!(r.try_prev_before(7), Ok(None));
        assert_eq!(r.try_next_after(7), Ok(None));
    }
}

/// The threaded workloads the Miri UB-site census runs for the 32-bit
/// wrappers (#1086, #1187): `scripts/miri_ub_sites.py` names them
/// `sync32::<test>` in `.github/miri-ub-sites.json`. Each runs one writer
/// against one reader, so every schedule overlaps the two.
#[cfg(test)]
mod miri_ub_sites {
    use super::*;
    use core::sync::atomic::AtomicBool;
    use std::thread;

    /// Keys a workload keeps: two linear leaves of `KEYS / 2` under a root
    /// branch (two top bytes), so the writer's overwrites, removals and
    /// reinsertions edit a published leaf in place.
    const KEYS: u32 = 24;
    const _: () = assert!(KEYS / 2 <= crate::types32::MAP_LEAF_MAX_32 as u32);
    /// Keys the writer adds and removes again: in pairs under fresh top
    /// bytes, so each pair allocates a leaf node and its removal frees it,
    /// and the next pair reuses the freed handle.
    const CHURN: u32 = 8;

    fn key(i: u32) -> u32 {
        if i < KEYS {
            ((i % 2) << 24) | 0x0042_0000 | i
        } else {
            let j = i - KEYS;
            ((2 + j / 2) << 24) | 0x0042_0000 | j
        }
    }

    /// Runs `pass` until the writer signals `done`, then once more.
    fn read_until(done: &AtomicBool, mut pass: impl FnMut()) {
        loop {
            let finished = done.load(Ordering::Acquire);
            pass();
            if finished {
                break;
            }
        }
    }

    /// A reader under the map writer's in-place overwrites, removals and
    /// reinsertions, and under the node frees and handle reuse that
    /// inserting and removing the churn keys cause.
    #[test]
    #[cfg_attr(miri, ignore = "UB site tracked by .github/miri-ub-sites.json (#1187)")]
    fn map_reader_writer() {
        let mut m = SyncExpanseMap32::with_capacity(MUTATION_HEADROOM * 2, 1);
        let (mut w, mut pool) = m.split();
        for i in 0..KEYS {
            w.try_insert(key(i), i).expect("prefill");
        }
        let mut r = pool.take().expect("one reader");
        let done = AtomicBool::new(false);
        thread::scope(|s| {
            s.spawn(|| {
                for i in 0..KEYS {
                    assert_eq!(w.try_insert(key(i), i + 100), Ok(Some(i)));
                    assert_eq!(w.try_remove(key(i)), Ok(Some(i + 100)));
                    assert_eq!(w.try_insert(key(i), i), Ok(None));
                }
                for i in KEYS..KEYS + CHURN {
                    assert_eq!(w.try_insert(key(i), i), Ok(None));
                }
                for i in KEYS..KEYS + CHURN {
                    assert_eq!(w.try_remove(key(i)), Ok(Some(i)));
                }
                done.store(true, Ordering::Release);
            });
            s.spawn(|| {
                read_until(&done, || {
                    for i in 0..KEYS {
                        if let Ok(Some(v)) = r.try_get(key(i)) {
                            assert!(v == i || v == i + 100);
                        }
                    }
                });
            });
        });
        assert_eq!(w.len(), KEYS as usize);
    }

    /// Top-byte digits the branch workload's root branch holds at each
    /// checkpoint: two (an `L2`), `BRANCH_L6_CAP_32`, a bitmap branch, then
    /// past `BRANCH_B_TO_UNCOMPRESSED_THRESHOLD_32`.
    const BRANCH_STEPS: [u32; 4] = [
        crate::types32::BRANCH_L2_CAP_32 as u32,
        crate::types32::BRANCH_L6_CAP_32 as u32,
        40,
        crate::types32::BRANCH_B_TO_UNCOMPRESSED_THRESHOLD_32 as u32 + 4,
    ];
    /// Keys under top byte 0: a full leaf, which one more key overflows into
    /// a root branch. The writer never edits it.
    const BRANCH_LEAF: u32 = crate::types32::MAP_LEAF_MAX_32 as u32;

    /// The key under top byte `d >= 1`: one key per digit, so each of those
    /// children is an immediate and every edit the writer makes is to the root
    /// branch itself.
    fn branch_key(d: u32) -> u32 {
        (d << 24) | 0x0042_0707
    }

    /// A reader under the root branch's in-place edits and form changes: the
    /// writer grows the root through every branch form, removes and reinserts
    /// a digit in each, and shrinks it back, and edits no leaf. The leaf
    /// workloads above report their leaf sites first, so branch sites are only
    /// observable here.
    #[test]
    #[cfg_attr(miri, ignore = "UB site tracked by .github/miri-ub-sites.json (#1187)")]
    fn map_branch_reader_writer() {
        let top = *BRANCH_STEPS.last().expect("steps");
        let mut m = SyncExpanseMap32::with_capacity(MUTATION_HEADROOM * 2, 1);
        let (mut w, mut pool) = m.split();
        for i in 0..BRANCH_LEAF {
            w.try_insert(0x0042_0700 | i, i).expect("leaf prefill");
        }
        // The key that overflows the leaf: the root becomes a branch with the
        // leaf under digit 0 and an immediate under digit 1.
        w.try_insert(branch_key(1), 1).expect("overflow");
        let mut r = pool.take().expect("one reader");
        let done = AtomicBool::new(false);
        let mut forms = std::vec::Vec::new();
        thread::scope(|s| {
            s.spawn(|| {
                let mut n = 2; // digits 0 and 1 are present
                for &step in &BRANCH_STEPS {
                    while n < step {
                        assert_eq!(w.try_insert(branch_key(n), n), Ok(None));
                        n += 1;
                    }
                    // One in-place removal and reinsertion in this form.
                    assert_eq!(w.try_remove(branch_key(n - 1)), Ok(Some(n - 1)));
                    assert_eq!(w.try_insert(branch_key(n - 1), n - 1), Ok(None));
                    forms.push(trie32::branch_form(&w.inner().root_edge()));
                }
                for &step in BRANCH_STEPS.iter().rev().skip(1) {
                    while n > step {
                        n -= 1;
                        assert_eq!(w.try_remove(branch_key(n)), Ok(Some(n)));
                    }
                    forms.push(trie32::branch_form(&w.inner().root_edge()));
                }
                done.store(true, Ordering::Release);
            });
            s.spawn(|| {
                read_until(&done, || {
                    for d in 1..top {
                        if let Ok(Some(v)) = r.try_get(branch_key(d)) {
                            assert_eq!(v, d);
                        }
                    }
                });
            });
        });
        // Every branch form was the root at some checkpoint, so each form's
        // in-place edits ran under the reader.
        for form in ["L2", "L6", "B", "U"] {
            assert!(
                forms.contains(&Some(form)),
                "form {form} not reached: {forms:?}"
            );
        }
        assert_eq!(w.len(), (BRANCH_LEAF + BRANCH_STEPS[0] - 1) as usize);
    }

    /// The set twin of `map_reader_writer`.
    #[test]
    #[cfg_attr(miri, ignore = "UB site tracked by .github/miri-ub-sites.json (#1187)")]
    fn set_reader_writer() {
        let mut m = SyncExpanseSet32::with_capacity(MUTATION_HEADROOM * 2, 1);
        let (mut w, mut pool) = m.split();
        for i in 0..KEYS {
            w.try_insert(key(i)).expect("prefill");
        }
        let mut r = pool.take().expect("one reader");
        let done = AtomicBool::new(false);
        thread::scope(|s| {
            s.spawn(|| {
                for i in 0..KEYS {
                    assert_eq!(w.try_remove(key(i)), Ok(true));
                    assert_eq!(w.try_insert(key(i)), Ok(true));
                }
                for i in KEYS..KEYS + CHURN {
                    assert_eq!(w.try_insert(key(i)), Ok(true));
                }
                for i in KEYS..KEYS + CHURN {
                    assert_eq!(w.try_remove(key(i)), Ok(true));
                }
                done.store(true, Ordering::Release);
            });
            s.spawn(|| {
                read_until(&done, || {
                    for i in 0..KEYS + CHURN {
                        let _ = r.try_contains(key(i));
                    }
                });
            });
        });
        assert_eq!(w.len(), KEYS as usize);
    }
}
