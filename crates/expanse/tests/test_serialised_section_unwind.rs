//! A panic out of a serialised section must not leave the writer gate closed.
//!
//! `Shared::quiesce_writers` closes the writer gate and drains every allocated
//! writer slot; the gate reopens when the `QuiesceGuard` it returns drops. Before that guard existed, reopening was a plain statement at the
//! tail of each of the four serialised sections — `write_root_covered_with`,
//! `read_locked`, `with_locked` and `with_locked_pre` — and an unwind skipped
//! it. `with_locked` and `with_locked_mut` take a caller's closure, and
//! `validate` panics by design, so calling the corruption checker through the
//! escape hatch left the gate closed for the life of the tree.
//!
//! The failure is silent rather than loud, which is why it needs a test:
//! optimistic readers never consult the gate, so the process goes on serving
//! reads at full rate while every later writer spins in
//! `enter_writer_blocking` with no yield, no bound and no diagnostic. A
//! root-leaf tree instead reaches the poisoned fallback mutex and panics, so
//! the tree here is populated past the root leaf on purpose — the hang is only
//! reachable once the root is a tree, and that is the shape any real workload
//! has.
//!
//! Each case waits with a deadline rather than blocking, so a regression fails
//! the test instead of hanging the suite.

#![cfg(not(miri))]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use expanse_trie::strmap::NulFreeStr;
use expanse_trie::sync::{SyncExpanseMap, SyncExpanseSet, SyncExpanseStrMap};

/// Enough keys that the root is a tree rather than a root leaf, so a later
/// writer takes the optimistic path and meets the gate.
const KEYS: u64 = 20_000;

/// How long a writer gets to finish. It needs microseconds; the margin is for
/// a loaded CI runner.
const DEADLINE: Duration = Duration::from_secs(20);

/// Runs `writer` on its own thread and reports whether it returned.
///
/// The panic a poisoned mutex raises counts as returning: it is loud, it is
/// pre-existing, and it is not what this file pins. Only a writer that neither
/// completes nor panics is the closed-gate spin.
fn writer_finishes(writer: impl FnOnce() + Send + 'static) -> bool {
    let (tx, rx) = mpsc::channel();
    let h = std::thread::spawn(move || {
        let _ = catch_unwind(AssertUnwindSafe(writer));
        let _ = tx.send(());
    });
    let finished = rx.recv_timeout(DEADLINE).is_ok();
    if finished {
        let _ = h.join();
    }
    finished
}

#[test]
fn map_with_locked_panic_leaves_the_gate_open() {
    let m = Arc::new(SyncExpanseMap::new());
    for k in 0..KEYS {
        m.insert(k, k);
    }

    let trigger = Arc::clone(&m);
    let caught = catch_unwind(AssertUnwindSafe(|| {
        trigger.with_locked(|_inner| -> u32 { panic!("a caller's closure panics") });
    }));
    assert!(caught.is_err(), "the closure's panic must propagate");

    let probe = Arc::clone(&m);
    assert!(
        writer_finishes(move || {
            probe.insert(u64::MAX - 1, 7);
        }),
        "a writer neither completed nor panicked: the gate was left closed by \
         the unwind out of with_locked, and every later writer spins"
    );
}

#[test]
fn set_with_locked_panic_leaves_the_gate_open() {
    let s = Arc::new(SyncExpanseSet::new());
    for k in 0..KEYS {
        s.insert(k);
    }

    let trigger = Arc::clone(&s);
    let caught = catch_unwind(AssertUnwindSafe(|| {
        trigger.with_locked(|_inner| -> u32 { panic!("a caller's closure panics") });
    }));
    assert!(caught.is_err(), "the closure's panic must propagate");

    let probe = Arc::clone(&s);
    assert!(
        writer_finishes(move || {
            probe.insert(u64::MAX - 1);
        }),
        "a writer neither completed nor panicked after a panic out of \
         SyncExpanseSet::with_locked"
    );
}

#[test]
fn strmap_with_locked_mut_panic_leaves_the_gate_open() {
    let m = Arc::new(SyncExpanseStrMap::new());
    for k in 0..KEYS {
        let key = format!("key-{k:012}");
        m.insert(
            NulFreeStr::new(key.as_bytes()).expect("formatted keys are NUL-free"),
            k,
        );
    }

    let trigger = Arc::clone(&m);
    let caught = catch_unwind(AssertUnwindSafe(|| {
        trigger.with_locked_mut(|_inner| -> u32 { panic!("a caller's closure panics") });
    }));
    assert!(caught.is_err(), "the closure's panic must propagate");

    let probe = Arc::clone(&m);
    assert!(
        writer_finishes(move || {
            probe.insert(NulFreeStr::new(b"zzz-probe").expect("NUL-free"), 7);
        }),
        "a writer neither completed nor panicked after a panic out of \
         SyncExpanseStrMap::with_locked_mut, which reaches with_locked_pre"
    );
}
