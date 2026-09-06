//! Negative control canary test for ThreadSanitizer.
//!
//! This test contains an intentional unsynchronized data race on a non-storage
//! memory location across two worker threads.
//!
//! In standard un-sanitized builds, this test executes and completes normally.
//! Under ThreadSanitizer (`-Zsanitizer=thread`), this test MUST be flagged as a
//! data race and cause the test process to abort (`SIGABRT`).
//!
//! Nightly CI runs this test under TSan with an inverted exit-code assertion to
//! continuously verify that ThreadSanitizer is armed and active, and that
//! `.github/tsan-suppressions.txt` has not rotted into an overbroad suppression
//! that masks real data races outside storage engine nodes.
//!
//! Not a Miri target: Miri's own data-race detector reports the intended race
//! as undefined behaviour and fails the binary, and the nightly Miri lane has
//! no inverted assertion to make that failure the expected outcome. The TSan
//! lane is the one with the inverted check.
#![cfg(not(miri))]

use std::cell::UnsafeCell;
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

struct UnsynchronizedCounter {
    val: UnsafeCell<usize>,
    start: AtomicBool,
}

// SAFETY: This struct intentionally implements Sync despite holding an UnsafeCell
// without internal synchronization, solely to provide a negative-control canary
// for ThreadSanitizer data race detection in tests.
unsafe impl Sync for UnsynchronizedCounter {}

#[test]
fn test_tsan_canary_intentional_race() {
    let counter = Arc::new(UnsynchronizedCounter {
        val: UnsafeCell::new(0),
        start: AtomicBool::new(false),
    });

    let c1 = counter.clone();
    let t1 = thread::spawn(move || {
        while !c1.start.load(Ordering::Acquire) {
            std::hint::spin_loop();
        }
        let ptr = c1.val.get();
        for i in 0..50_000 {
            // SAFETY: Intentional unsynchronized race for TSan canary verification.
            unsafe {
                *ptr = (*ptr).wrapping_add(black_box(i));
            }
        }
    });

    let c2 = counter.clone();
    let t2 = thread::spawn(move || {
        while !c2.start.load(Ordering::Acquire) {
            std::hint::spin_loop();
        }
        let ptr = c2.val.get();
        for i in 0..50_000 {
            // SAFETY: Intentional unsynchronized race for TSan canary verification.
            unsafe {
                *ptr = (*ptr).wrapping_add(black_box(i));
            }
        }
    });

    // Release both threads simultaneously to guarantee race overlap
    counter.start.store(true, Ordering::Release);

    t1.join().unwrap();
    t2.join().unwrap();
}
