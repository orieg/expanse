//! 32-bit Optimistic Concurrency Control (OCC) primitives.
//!
//! Provides a 32-bit machine word seqlock [`SeqVersion32`] backed by [`core::sync::atomic::AtomicU32`]
//! for 32-bit embedded targets (RV32, ESP32, Cortex-M) per `docs/design/32-bit-embedded.md`.
//!
//! Avoids 64-bit atomic emulation libcalls (e.g. `__atomic_load_8` on RV32) by operating
//! natively on 32-bit words with single-instruction acquire/release semantics.

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

    /// Reader: samples the version, spinning past in-progress (odd) states.
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

/// Reader: sample node-level version word.
///
/// # Safety
///
/// `version_ptr` must point to an aligned readable `u32`.
#[inline(always)]
#[must_use]
pub unsafe fn node_sample(version_ptr: *const u32) -> u32 {
    loop {
        // SAFETY: caller guarantees pointer validity per contract.
        let v = unsafe { core::ptr::read_volatile(version_ptr) };
        if v.is_multiple_of(2) {
            fence(Ordering::Acquire);
            return v;
        }
        core::hint::spin_loop();
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
        let s0 = unsafe { node_sample(ptr) };
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
        let s1 = unsafe { node_sample(ptr) };
        assert_eq!(s1, 2);
        // SAFETY: ptr points to a valid local u32 on stack.
        assert!(unsafe { node_validate(ptr, s1) });
        // SAFETY: ptr points to a valid local u32 on stack.
        assert!(!unsafe { node_validate(ptr, s0) });
    }
}
