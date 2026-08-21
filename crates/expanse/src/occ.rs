//! Phase 7: optimistic concurrency primitives — the seqlock version word
//! readers validate against, and epoch-based reclamation (EBR) so a
//! reader never dereferences a freed node.
//!
//! The concurrent wrappers (`SyncExpanseSet`/`SyncExpanseMap` in `sync`)
//! combine a **tree-level** [`SeqVersion`] (bracketing each write op;
//! readers validate their root snapshot against it) with **per-node**
//! versions in the branch headers: the mutation engine brackets each
//! node's in-place mutation region — child-slot rewrites and the
//! recursion beneath them — via [`version_begin_if`] (active only for
//! concurrently shared trees), and readers validate hand-over-hand with
//! [`node_sample`]/[`node_validate`]. Measured motivation and effect in
//! `docs/BENCHMARKING.md` (concurrent read scaling).
//!
//! Under `--cfg loom` the atomics and sync types swap to loom's, and the
//! `loom_` tests model-check writer/reader/reclamation interleavings.

#[cfg(not(loom))]
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering, fence};
#[cfg(not(loom))]
use std::sync::Mutex;

#[cfg(loom)]
use loom::sync::Mutex;
#[cfg(loom)]
use loom::sync::atomic::{AtomicU64, AtomicUsize, Ordering, fence};

// `Arc` stays std's in both builds: loom's would infect every consumer's
// signature, and the model only needs to explore the atomics/locks.
use std::sync::Arc;

use core::ptr::NonNull;
use std::alloc::{Layout, dealloc};

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
        debug_assert!(v % 2 == 0, "nested or unpaired begin");
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
        loop {
            let v = self.0.load(Ordering::Acquire);
            if v % 2 == 0 {
                return v;
            }
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

/// Per-node seqlock over the plain `u32` version fields embedded in the
/// node headers (`BranchHeader.version`, `BranchB.version`,
/// `BranchU.version`). The single writer wraps each node's in-place
/// mutation region — child-slot rewrites and the recursion beneath them
/// included — in [`version_begin`]/[`version_end`]; readers
/// [`node_sample`]/[`node_validate`] hand-over-hand along their walk.
/// Same Boehm fence construction as [`SeqVersion`], `u32`-wide (a
/// 2^31-write wrap mid-walk is not a practical hazard).
///
/// Writer stores are volatile through the ordinary `&mut` — never split,
/// merged, or elided, with no raw-pointer aliasing against the engine's
/// live borrows — while readers load the same field atomically; that
/// mixed access is part of the documented seqlock caveat (`sync` docs).
///
/// Writer: opens a node's mutation bracket when `OCC` — i.e. when the
/// tree is shared through a Phase 7 wrapper. The flag is a **const
/// generic threaded down from the operation's entry point**, not a
/// per-node check: `NodeAlloc::occ_enabled()` is an atomic load through
/// a `OnceLock`, and calling it twice per branch level cost ~10 atomic
/// loads per insert on a deep tree (issue #1 item 1). Single-threaded
/// trees now compile the brackets out entirely.
#[inline]
pub(crate) fn version_begin_if<const OCC: bool>(a: &crate::alloc::NodeAlloc, v: &mut u32) {
    if OCC {
        debug_assert!(a.occ_enabled(), "OCC=true on a non-shared tree");
        version_begin(v);
    }
    #[cfg(debug_assertions)]
    a.bracket_enter();
    let _ = a;
}

/// Writer: closes a node's mutation bracket (see [`version_begin_if`]).
#[inline]
pub(crate) fn version_end_if<const OCC: bool>(a: &crate::alloc::NodeAlloc, v: &mut u32) {
    if OCC {
        version_end(v);
    }
    #[cfg(debug_assertions)]
    a.bracket_leave();
    let _ = a;
}

/// Writer: marks a node mutation in progress (even → odd, then a release
/// fence so the odd version is visible before any covered write).
#[inline]
pub(crate) fn version_begin(v: &mut u32) {
    let cur = *v;
    debug_assert!(cur % 2 == 0, "nested node write bracket");
    // SAFETY: plain field write through the exclusive borrow; volatile
    // only pins the store's shape for concurrent atomic readers.
    unsafe { core::ptr::write_volatile(v, cur + 1) };
    core::sync::atomic::fence(core::sync::atomic::Ordering::Release);
}

/// Writer: marks the node mutation complete (odd → even; the release
/// fence orders every covered write before the even version).
#[inline]
pub(crate) fn version_end(v: &mut u32) {
    let cur = *v;
    debug_assert!(cur % 2 == 1, "version_end without begin");
    core::sync::atomic::fence(core::sync::atomic::Ordering::Release);
    // SAFETY: as in version_begin.
    unsafe { core::ptr::write_volatile(v, cur + 1) };
}

/// Reader: samples a node version; `None` while a mutation is in
/// progress (odd).
///
/// # Safety
///
/// `ptr` must point at a live node's version field (EBR-pinned).
pub(crate) unsafe fn node_sample(ptr: *const u32) -> Option<u32> {
    // SAFETY: valid, aligned version field per contract.
    let a = unsafe { core::sync::atomic::AtomicU32::from_ptr(ptr.cast_mut()) };
    let v = a.load(core::sync::atomic::Ordering::Acquire);
    (v % 2 == 0).then_some(v)
}

/// Reader: true when the node version still equals `snap` (the acquire
/// fence orders the caller's preceding loads before the re-read).
///
/// # Safety
///
/// Same contract as [`node_sample`].
pub(crate) unsafe fn node_validate(ptr: *const u32, snap: u32) -> bool {
    core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);
    // SAFETY: valid, aligned version field per contract.
    let a = unsafe { core::sync::atomic::AtomicU32::from_ptr(ptr.cast_mut()) };
    a.load(core::sync::atomic::Ordering::Relaxed) == snap
}

/// Number of epoch garbage bins. A retired node becomes freeable once
/// the global epoch has advanced twice past its retirement epoch: every
/// reader pinned at retirement time has since unpinned.
const BINS: usize = 3;

/// A reader-registration slot: the epoch the reader is pinned at, or
/// [`INACTIVE`].
type Slot = AtomicUsize;

const INACTIVE: usize = usize::MAX;

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
unsafe impl Send for Garbage {}

/// Epoch-based reclamation for one tree: readers pin the current epoch
/// around each walk; retired allocations wait two epoch advances before
/// they are freed, so a pinned reader can never observe freed memory.
#[derive(Debug)]
pub struct Collector {
    epoch: AtomicUsize,
    readers: Mutex<Vec<Arc<Slot>>>,
    bins: [Mutex<Vec<Garbage>>; BINS],
    freelists: std::sync::OnceLock<Arc<crate::alloc::FreelistPool>>,
}

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

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
            freelists: std::sync::OnceLock::new(),
        }
    }

    /// Links a size-class freelist pool to recycle reclaimed blocks.
    pub(crate) fn set_freelist_pool(&self, pool: Arc<crate::alloc::FreelistPool>) {
        let _ = self.freelists.set(pool);
    }

    /// Registers a reader; the returned handle pins/unpins cheaply.
    #[must_use]
    pub fn register(self: &Arc<Self>) -> Reader {
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
        let e = self.epoch.load(Ordering::Relaxed);
        self.bins[e % BINS]
            .lock()
            .expect("garbage bin poisoned")
            .push(Garbage { ptr, bytes, align });
    }

    /// Attempts one epoch advance: succeeds when every pinned reader has
    /// caught up to the current epoch, then frees the bin two epochs
    /// back. Writer-side, amortized (call once per mutation batch).
    pub fn try_advance(&self) {
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
        self.epoch.store(e + 1, Ordering::Release);
        // Everything retired at epoch e - 1 predates every possible pin
        // in epochs e and e + 1: no live reader can hold it.
        let stale = core::mem::take(
            &mut *self.bins[(e + BINS - 1) % BINS]
                .lock()
                .expect("garbage bin poisoned"),
        );
        let pool = self.freelists.get();
        for g in stale {
            if let Some(p) = pool {
                // SAFETY: reclaimed after grace period; recycling into size-class pool.
                if unsafe { p.recycle(g.ptr, g.bytes, g.align) } {
                    continue;
                }
            }
            free_raw(g.ptr, g.bytes, g.align);
        }
    }

    /// Frees everything still queued. Only sound once no reader can be
    /// pinned (the owning wrapper calls this on drop, when exclusive
    /// ownership proves that).
    pub(crate) fn drain(&self) {
        for bin in &self.bins {
            let stale = core::mem::take(&mut *bin.lock().expect("garbage bin poisoned"));
            for g in stale {
                free_raw(g.ptr, g.bytes, g.align);
            }
        }
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        // Last owner: no readers remain by definition.
        self.drain();
    }
}

fn free_raw(ptr: NonNull<u8>, bytes: usize, align: usize) {
    let layout = Layout::from_size_align(bytes, align).expect("valid retired layout");
    // SAFETY: retired allocations come from `NodeAlloc` (same
    // size/alignment contract) and are freed exactly once, after their
    // grace period.
    unsafe { dealloc(ptr.as_ptr(), layout) };
}

/// A registered reader handle. Cheap to pin/unpin per operation; not
/// `Sync` — one per reading thread.
pub struct Reader {
    collector: Arc<Collector>,
    slot: Arc<Slot>,
}

impl Reader {
    /// Pins the current epoch for the duration of the returned guard:
    /// nothing retired from here on is freed while the guard lives.
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
pub struct Pin<'a> {
    slot: &'a Slot,
}

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
}
