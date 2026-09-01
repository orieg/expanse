//! 32-bit Optimistic Concurrency Control (OCC) primitives.
//!
//! Provides a 32-bit machine word seqlock [`SeqVersion32`] backed by [`core::sync::atomic::AtomicU32`]
//! for 32-bit embedded targets (RV32, ESP32, Cortex-M) per `docs/design/32-bit-embedded.md`.
//!
//! Avoids 64-bit atomic emulation libcalls (e.g. `__atomic_load_8` on RV32) by operating
//! natively on 32-bit words with single-instruction acquire/release semantics.
//!
//! # Interrupt-handler contract
//!
//! On a single-core part, a reader that **spins** waiting for a writer's
//! bracket to close can never make progress if it preempted that writer:
//! there is no scheduler to run the writer, so the version word stays odd
//! forever. Hard hang, watchdog reset, no core dump.
//!
//! Every sampling entry point here is therefore **single-attempt and
//! bounded** — [`SeqVersion32::try_sample`] and [`try_node_sample`] return
//! `None` rather than spinning, and the caller decides what to do. This
//! mirrors the 64-bit `occ::node_sample`, which is already `Option`-shaped.
//! A caller in an interrupt handler must surface the `None` (as a `Busy`
//! result) rather than retry in place.
//!
//! [`SeqVersion32::sample`] does spin and is retained for single-threaded
//! and cooperatively-scheduled callers; it is **not** safe to call from an
//! interrupt handler that can preempt the writer.

use core::sync::atomic::{AtomicU32, Ordering, fence};

/// A 32-bit seqlock version word: even = stable, odd = mutation in progress.
///
/// One writer at a time; any number of concurrent readers.
#[derive(Debug, Default)]
pub struct SeqVersion32(AtomicU32);

impl SeqVersion32 {
    /// Create a fresh, even (stable) version initialized to 0.
    #[inline(always)]
    #[must_use]
    pub const fn new() -> Self {
        Self(AtomicU32::new(0))
    }

    /// Writer: marks a mutation in progress (even -> odd).
    #[inline(always)]
    pub fn begin(&self) {
        let v = self.0.load(Ordering::Relaxed);
        debug_assert!(v.is_multiple_of(2), "nested or unpaired begin");
        self.0.store(v.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
    }

    /// Writer: marks the mutation complete (odd -> even), publishing writes.
    #[inline(always)]
    pub fn end(&self) {
        let v = self.0.load(Ordering::Relaxed);
        debug_assert!(!v.is_multiple_of(2), "end without begin");
        self.0.store(v.wrapping_add(1), Ordering::Release);
    }

    /// Reader: one bounded attempt. `Some(even)` on a stable snapshot,
    /// `None` while a writer's bracket is open.
    ///
    /// This is the entry point an interrupt handler must use — see the
    /// module's interrupt-handler contract. It never spins, so a reader
    /// that preempted the writer returns immediately instead of hanging.
    #[inline(always)]
    #[must_use]
    pub fn try_sample(&self) -> Option<u32> {
        let v = self.0.load(Ordering::Acquire);
        v.is_multiple_of(2).then_some(v)
    }

    /// Reader: samples the version, spinning past in-progress (odd) states.
    ///
    /// **Not interrupt-safe.** On a single-core target this deadlocks if the
    /// caller preempted the writer that opened the bracket. Use
    /// [`Self::try_sample`] there.
    #[inline(always)]
    #[must_use]
    pub fn sample(&self) -> u32 {
        loop {
            let v = self.0.load(Ordering::Acquire);
            if v.is_multiple_of(2) {
                return v;
            }
            core::hint::spin_loop();
        }
    }

    /// Reader: returns true if no mutation began since `snapshot` was taken.
    #[inline(always)]
    #[must_use]
    pub fn validate(&self, snapshot: u32) -> bool {
        fence(Ordering::Acquire);
        self.0.load(Ordering::Relaxed) == snapshot
    }
}

/// Begin node-level in-place mutation on a raw pointer to a version word.
///
/// # Safety
///
/// `version_ptr` must point to an aligned, readable/writable `u32` within a live node.
#[inline(always)]
pub unsafe fn node_version_begin(version_ptr: *mut u32) {
    // SAFETY: caller guarantees pointer validity per contract.
    unsafe {
        let v = core::ptr::read_volatile(version_ptr);
        debug_assert!(v.is_multiple_of(2), "node version already in mutation");
        core::ptr::write_volatile(version_ptr, v.wrapping_add(1));
        fence(Ordering::Release);
    }
}

/// End node-level in-place mutation on a raw pointer to a version word.
///
/// # Safety
///
/// `version_ptr` must point to an aligned, readable/writable `u32` within a live node.
#[inline(always)]
pub unsafe fn node_version_end(version_ptr: *mut u32) {
    // SAFETY: caller guarantees pointer validity per contract.
    unsafe {
        fence(Ordering::Release);
        let v = core::ptr::read_volatile(version_ptr);
        debug_assert!(!v.is_multiple_of(2), "node version end without begin");
        core::ptr::write_volatile(version_ptr, v.wrapping_add(1));
    }
}

/// Reader: one bounded attempt at a node-level version word.
///
/// `Some(even)` on a stable snapshot, `None` while that node's bracket is
/// open. Never spins — see the module's interrupt-handler contract. Same
/// shape as the 64-bit `occ::node_sample`.
///
/// # Safety
///
/// `version_ptr` must point to an aligned readable `u32`.
#[inline(always)]
#[must_use]
pub unsafe fn try_node_sample(version_ptr: *const u32) -> Option<u32> {
    // SAFETY: caller guarantees pointer validity per contract.
    let v = unsafe { core::ptr::read_volatile(version_ptr) };
    if v.is_multiple_of(2) {
        fence(Ordering::Acquire);
        Some(v)
    } else {
        None
    }
}

/// Reader: validate node-level version word against snapshot.
///
/// # Safety
///
/// `version_ptr` must point to an aligned readable `u32`.
#[inline(always)]
#[must_use]
pub unsafe fn node_validate(version_ptr: *const u32, snapshot: u32) -> bool {
    fence(Ordering::Acquire);
    // SAFETY: caller guarantees pointer validity per contract.
    unsafe { core::ptr::read_volatile(version_ptr) == snapshot }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The interrupt-handler contract: a bounded sample must report an open
    /// bracket rather than spin. Spinning here is what hangs a single-core
    /// part when the reader preempted the writer.
    #[test]
    fn try_sample_reports_open_bracket_instead_of_spinning() {
        let seq = SeqVersion32::new();
        assert_eq!(seq.try_sample(), Some(0));

        seq.begin();
        assert_eq!(
            seq.try_sample(),
            None,
            "an open bracket must yield None, never a spin"
        );

        seq.end();
        assert_eq!(seq.try_sample(), Some(2));
    }

    /// Same contract at node level.
    #[test]
    fn try_node_sample_reports_open_bracket() {
        let mut version = 0u32;
        let ptr = &raw mut version;

        // SAFETY: ptr points to a valid local u32 on the stack.
        unsafe {
            assert_eq!(try_node_sample(ptr), Some(0));
            node_version_begin(ptr);
            assert_eq!(try_node_sample(ptr), None, "open bracket must yield None");
            node_version_end(ptr);
            assert_eq!(try_node_sample(ptr), Some(2));
        }
    }

    #[test]
    fn test_seq_version32_transitions() {
        let seq = SeqVersion32::new();
        let s0 = seq.sample();
        assert_eq!(s0, 0);
        assert!(seq.validate(s0));

        seq.begin();
        // Validation fails during mutation
        assert!(!seq.validate(s0));

        seq.end();
        let s1 = seq.sample();
        assert_eq!(s1, 2);
        assert!(seq.validate(s1));
        assert!(!seq.validate(s0));
    }

    #[test]
    fn test_node_version_raw_transitions() {
        let mut version = 0u32;
        let ptr = &mut version as *mut u32;

        // SAFETY: ptr points to a valid local u32 on stack.
        let s0 = unsafe { try_node_sample(ptr) }.expect("stable");
        assert_eq!(s0, 0);
        // SAFETY: ptr points to a valid local u32 on stack.
        assert!(unsafe { node_validate(ptr, s0) });

        // SAFETY: ptr points to a valid local u32 on stack.
        unsafe { node_version_begin(ptr) };
        assert_eq!(version, 1);
        // SAFETY: ptr points to a valid local u32 on stack.
        assert!(!unsafe { node_validate(ptr, s0) });

        // SAFETY: ptr points to a valid local u32 on stack.
        unsafe { node_version_end(ptr) };
        assert_eq!(version, 2);
        // SAFETY: ptr points to a valid local u32 on stack.
        let s1 = unsafe { try_node_sample(ptr) }.expect("stable");
        assert_eq!(s1, 2);
        // SAFETY: ptr points to a valid local u32 on stack.
        assert!(unsafe { node_validate(ptr, s1) });
        // SAFETY: ptr points to a valid local u32 on stack.
        assert!(!unsafe { node_validate(ptr, s0) });
    }
}
