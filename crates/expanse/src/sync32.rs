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
//! - **Reclamation is degenerate quiescent-state-based**: freed nodes park
//!   in a fixed pending list; the writer drains it only after observing
//!   every reader outside a walk. Readers pin with one padded per-reader
//!   flag (a store), never a shared counter — registration and pinning are
//!   CAS-free.
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
//! # Memory-model caveat
//!
//! Readers walk tree memory the writer may be mutating; the racy loads
//! are validated before use per the seqlock pattern (Boehm, "Can seqlocks
//! get along with programming language memory models?", MSPC 2012). Under
//! a strict reading of the Rust memory model those plain loads are data
//! races; this is the same industry-standard trade the 64-bit `sync`
//! module documents, pending tearable-atomics support, and it is why the
//! concurrent stress tests are excluded under Miri. The reclamation-fence
//! construction (reader: store-pin then `SeqCst` fence then sample;
//! writer: mutate/unlink, close bracket, `SeqCst` fence, then read pins)
//! mirrors the store-buffer pairing the 64-bit `occ` module model-checked
//! with loom: if the writer misses a reader's pin store, that reader's
//! subsequent sample is fence-ordered after the writer's bracket close, so
//! it walks the post-unlink tree and cannot reach a pending allocation.
//!
//! In-place mutation extends beyond the bitmap/branch subarray stores:
//! linear-leaf inserts, removes, and value overwrites shift or store
//! into the leaf's byte buffer in place while the population stays
//! inside its capacity class (#577), so a validated reader may observe
//! mid-shift bytes from the very buffer it is scanning. That is safe
//! under the same argument as every other racy load here: leaf buffers
//! hold plain bytes (never pointers), every content-derived index on
//! the validated walks is bounds-checked against the live buffer
//! length, and the version seal rejects any read that overlapped a
//! write bracket before its result can escape. Node replacement — with
//! the old node retired for stalled readers — now happens only at
//! capacity-class boundaries and structural conversions.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, Ordering, fence};

use core_alloc::{boxed::Box, vec::Vec};

use crate::map32::ExpanseMap32;
use crate::occ32::SeqVersion32;
use crate::set32::ExpanseSet32;
use crate::trie32::{self, Arena, Torn};
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

/// One reader's pin flag, padded to its own cache line so concurrent
/// readers never false-share (§2.1 invariant 4; 64 B covers both the
/// 64-byte server lines and the 32-byte embedded lines).
#[repr(align(64))]
struct ReaderSlot {
    /// Nonzero while the owning [`Reader32`] is inside a walk.
    in_walk: AtomicU32,
}

/// The tree-level seqlock, padded so the word every reader polls does not
/// share a cache line with the container it guards.
#[repr(align(64))]
struct PaddedVersion(SeqVersion32);

/// Containers `Sync32` can wrap (sealed to the 32-bit engines).
pub trait Container32: sealed::Sealed {}

mod sealed {
    use super::{Arena, ExpanseMap32, ExpanseSet32};

    pub trait Sealed: Send + Sync {
        fn with_fixed_arena(node_cap: usize, pending_cap: usize) -> Self;
        fn arena(&self) -> &Arena;
        fn arena_mut(&mut self) -> &mut Arena;
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
            in_walk: AtomicU32::new(0),
        });
        Self {
            inner: UnsafeCell::new(T::with_fixed_arena(node_cap, pending_cap)),
            version: PaddedVersion(SeqVersion32::new()),
            readers: slots.into_boxed_slice(),
        }
    }

    /// Splits into the unique writer handle and the reader-handle pool.
    ///
    /// `&mut self` proves no other handles exist, and the returned
    /// [`Writer32`] is not clonable, so single-writer exclusion is a
    /// compile-time property — no lock, no atomic RMW.
    pub fn split(&mut self) -> (Writer32<'_, T>, ReaderPool32<'_, T>) {
        let this: &Self = self;
        (
            Writer32 { owner: this },
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
        slot.in_walk.store(1, Ordering::Relaxed);
        // Pairs with the writer's pre-drain fence: if the writer misses
        // this pin, our sample below is fence-ordered after its bracket
        // close and we walk the post-unlink tree (see module docs).
        fence(Ordering::SeqCst);
        let out = walk(self.owner);
        // Release: our loads from tree memory complete before the unpin
        // becomes visible to the draining writer.
        slot.in_walk.store(0, Ordering::Release);
        out
    }
}

/// The unique mutating handle. Not clonable; obtained once per
/// [`Sync32::split`].
pub struct Writer32<'a, T: Container32> {
    owner: &'a Sync32<T>,
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
            return true;
        }
        // Pairs with the readers' post-pin fence (module docs).
        fence(Ordering::SeqCst);
        for slot in self.owner.readers.iter() {
            if slot.in_walk.load(Ordering::Acquire) != 0 {
                return false;
            }
        }
        self.inner_mut().arena_mut().drain_pending();
        true
    }

    /// Runs one mutation inside the version bracket.
    #[inline]
    fn write<R>(&mut self, f: impl FnOnce(&mut T) -> R) -> R {
        #[cfg(test)]
        self.inner_mut().arena_mut().reset_mutation_watermark();
        self.owner.version.0.begin();
        let r = f(self.inner_mut());
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
            // SAFETY: validated racy read per the module memory-model
            // caveat; the pin keeps racily-reachable memory alive.
            let m = unsafe { &*owner.inner.get() };
            let root: Edge32 = m.root_edge();
            if !owner.version.0.validate(snap) {
                return Err(Busy);
            }
            let still_valid = || owner.version.0.validate(snap);
            trie32::map_get_validated(m.arena(), root, key, &still_valid).map_err(|Torn| Busy)
        })
    }

    /// One bounded optimistic length read.
    pub fn try_len(&mut self) -> Result<usize, Busy> {
        self.pinned(|owner| {
            let Some(snap) = owner.version.0.try_sample() else {
                return Err(Busy);
            };
            // SAFETY: as in `try_get`.
            let n = unsafe { &*owner.inner.get() }.len();
            if owner.version.0.validate(snap) {
                Ok(n)
            } else {
                Err(Busy)
            }
        })
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
            // SAFETY: validated racy read per the module memory-model
            // caveat; the pin keeps racily-reachable memory alive.
            let s = unsafe { &*owner.inner.get() };
            let root: Edge32 = s.root_edge();
            if !owner.version.0.validate(snap) {
                return Err(Busy);
            }
            let still_valid = || owner.version.0.validate(snap);
            trie32::set_contains_validated(s.arena(), root, key, &still_valid).map_err(|Torn| Busy)
        })
    }

    /// One bounded optimistic length read.
    pub fn try_len(&mut self) -> Result<usize, Busy> {
        self.pinned(|owner| {
            let Some(snap) = owner.version.0.try_sample() else {
                return Err(Busy);
            };
            // SAFETY: as in `try_contains`.
            let n = unsafe { &*owner.inner.get() }.len();
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
    fn pin_flag<'a, T: Container32>(w: &Writer32<'a, T>, idx: usize) -> &'a AtomicU32 {
        &w.owner.readers[idx].in_walk
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
        pin_flag(&w, 0).store(1, Ordering::Relaxed);
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

        pin_flag(&w, 0).store(0, Ordering::Release);
        assert!(w.try_reclaim(), "quiescent readers allow draining");
        assert_eq!(w.pending_len(), 0);
        assert_eq!(w.pending_bytes(), 0);
        w.try_insert(1, 1).expect("writes proceed after reclaim");
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
}
