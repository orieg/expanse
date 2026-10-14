//! `SyncExpanseBlobMap::mem_used_estimate` (#1478): equal to `mem_used` on a
//! quiet map, returning while a compaction is held between its two halves,
//! and within its documented bounds there.

use super::*;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// How long a step that must not block may take before the test fails.
const DEADLINE: Duration = Duration::from_secs(30);

fn payload(k: u64, round: u64) -> Vec<u8> {
    (0..128).map(|i| (k ^ round ^ i) as u8).collect()
}

/// Joins `h`, failing with `what` if it has not finished by the deadline.
fn join_by_deadline<T>(h: std::thread::JoinHandle<T>, what: &str) -> T {
    let deadline = Instant::now() + DEADLINE;
    while !h.is_finished() {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::yield_now();
    }
    h.join().expect("the thread panicked")
}

#[track_caller]
fn assert_quiet_equal(m: &SyncExpanseBlobMap) -> usize {
    let exact = m.mem_used();
    assert_eq!(
        m.mem_used_estimate(),
        exact,
        "on a quiet map the estimate is the exact figure"
    );
    exact
}

/// With nothing in flight the estimate equals `mem_used`, through inline and
/// arena inserts, overwrites, removals, a compaction, a clear, a map shared
/// after it was populated, and a multi-writer run read before and after the
/// writers' arenas are folded.
#[test]
fn quiet_estimate_equals_mem_used() {
    let m = Arc::new(SyncExpanseBlobMap::with_chunk_size(4096));
    assert_eq!(assert_quiet_equal(&m), 0);
    let keys: u64 = if cfg!(miri) { 96 } else { 2000 };
    for k in 0..keys {
        // Every third payload is inline: it grows the index and not the arena.
        let data = if k % 3 == 0 {
            vec![k as u8; 5]
        } else {
            payload(k, 0)
        };
        m.insert(k, &data, 0).unwrap();
    }
    assert!(m.shared.published().is_tree());
    let loaded = assert_quiet_equal(&m);
    assert!(loaded > 4096, "the arena and the index both hold bytes");
    for k in 0..keys / 2 {
        m.insert(k, &payload(k, 1), 0).unwrap();
    }
    let overwritten = assert_quiet_equal(&m);
    assert!(overwritten > loaded, "the overwrites took new chunks");
    for k in keys / 4..keys / 2 {
        assert!(m.remove(k));
    }
    assert_quiet_equal(&m);
    m.compact().expect("compaction");
    assert!(
        assert_quiet_equal(&m) < overwritten,
        "the compaction dropped the dead chunks"
    );

    // Four writers on disjoint keys, each with a private chunk.
    let per: u64 = if cfg!(miri) { 24 } else { 500 };
    let writers: Vec<_> = (0..4u64)
        .map(|t| {
            let m = Arc::clone(&m);
            std::thread::spawn(move || {
                let base = 0x5000_0000 + t * 0x1_0000;
                for i in 0..per {
                    m.insert(base + i, &payload(i, t), 0).unwrap();
                }
                for i in 0..per / 2 {
                    assert!(m.remove(base + i));
                }
            })
        })
        .collect();
    for w in writers {
        w.join().expect("writer panicked");
    }
    // Before any fold: the writers' live-byte deltas are still in their slots.
    let unfolded = m.mem_used_estimate();
    // `with_locked` folds them; the figure does not depend on the fold.
    assert_eq!(m.with_locked(ExpanseBlobMap::mem_used), unfolded);
    assert_quiet_equal(&m);
    m.compact().expect("compaction");
    assert_quiet_equal(&m);
    m.clear();
    assert_eq!(assert_quiet_equal(&m), 0);

    // A map shared after it was populated: its index is rebuilt through a
    // deferred allocator, and its chunks are published as they stand.
    let mut plain = ExpanseBlobMap::with_chunk_size(4096);
    for k in 0..keys {
        plain.insert(k, &payload(k, 7), 0).unwrap();
    }
    let shared = SyncExpanseBlobMap::from(plain);
    assert!(assert_quiet_equal(&shared) > 4096);
}

/// The issue's done-when test. A compaction is held where its copy has
/// finished and its index rewrite has not started; it holds the fallback
/// mutex, the closed writer gate and the writer mutex there. The estimate
/// returns while it is held, twice, and equals the figure from before the
/// compaction; a thread in `mem_used` has not returned by then and cannot
/// until the compaction ends.
#[test]
fn estimate_returns_while_a_compaction_is_held_between_its_halves() {
    const LIVE: u64 = 48;
    let m = Arc::new(SyncExpanseBlobMap::with_chunk_size(4096));
    for round in 0..4 {
        for k in 0..LIVE {
            m.insert(k, &payload(k, round), 1).unwrap();
        }
    }
    assert!(m.shared.published().is_tree());
    let before = assert_quiet_equal(&m);

    let during = Arc::new(AtomicUsize::new(usize::MAX));
    let during_again = Arc::new(AtomicUsize::new(usize::MAX));
    let locked_started = Arc::new(AtomicBool::new(false));
    let locked_done = Arc::new(AtomicBool::new(false));
    let locked_thread = Arc::new(Mutex::new(None));
    let held = Arc::new(AtomicBool::new(false));
    {
        let (m, during, during_again) = (
            Arc::clone(&m),
            Arc::clone(&during),
            Arc::clone(&during_again),
        );
        let (locked_started, locked_done, locked_thread, held) = (
            Arc::clone(&locked_started),
            Arc::clone(&locked_done),
            Arc::clone(&locked_thread),
            Arc::clone(&held),
        );
        crate::blobmap::COPIED_HOOK.with(|h| {
            *h.borrow_mut() = Some(Box::new(move || {
                // The compaction holds what `mem_used` needs first.
                assert!(
                    m.shared.fallback_mutex.try_lock().is_err(),
                    "the held compaction owns the fallback mutex"
                );
                let m2 = Arc::clone(&m);
                let est = std::thread::spawn(move || m2.mem_used_estimate());
                during.store(
                    join_by_deadline(
                        est,
                        "mem_used_estimate did not return while the compaction was held",
                    ),
                    Ordering::Release,
                );
                // A locked read started now waits for the compaction.
                let (m2, started, done) = (
                    Arc::clone(&m),
                    Arc::clone(&locked_started),
                    Arc::clone(&locked_done),
                );
                let locked = std::thread::spawn(move || {
                    started.store(true, Ordering::Release);
                    let exact = m2.mem_used();
                    done.store(true, Ordering::Release);
                    exact
                });
                let deadline = Instant::now() + DEADLINE;
                while !locked_started.load(Ordering::Acquire) {
                    assert!(Instant::now() < deadline, "the locked reader never started");
                    std::thread::yield_now();
                }
                // The estimate still returns with a locked reader queued.
                let m2 = Arc::clone(&m);
                let est = std::thread::spawn(move || m2.mem_used_estimate());
                during_again.store(
                    join_by_deadline(
                        est,
                        "mem_used_estimate did not return beside a waiting mem_used",
                    ),
                    Ordering::Release,
                );
                assert!(
                    !locked_done.load(Ordering::Acquire),
                    "mem_used returned while the compaction held its locks"
                );
                *locked_thread.lock().unwrap() = Some(locked);
                held.store(true, Ordering::Release);
            }));
        });
    }
    let stats = m.compact().expect("compaction");
    crate::blobmap::COPIED_HOOK.with(|h| h.borrow_mut().take());
    assert!(
        held.load(Ordering::Acquire),
        "the compaction must reach the copy"
    );
    assert!(stats.total_allocated_after < stats.total_allocated_before);

    let locked = locked_thread
        .lock()
        .unwrap()
        .take()
        .expect("the hook stored the locked reader");
    let after = join_by_deadline(locked, "mem_used did not return after the compaction");
    // Bounds: the pre-compaction figure throughout the hold, the
    // post-compaction figure once it has ended, and never both chunk sets.
    assert_eq!(during.load(Ordering::Acquire), before);
    assert_eq!(during_again.load(Ordering::Acquire), before);
    assert!(after < before);
    assert_eq!(
        before - after,
        stats.total_allocated_before - stats.total_allocated_after,
        "a compaction changes the arena term alone"
    );
    assert_eq!(assert_quiet_equal(&m), after);
}

/// Writers, a compacting thread and an estimate poller on one map. While
/// they run, the estimate stays inside what the map can hold: at least the
/// one chunk the never-removed keys keep, at most the arena cap plus an
/// index bound. Afterwards it equals `mem_used`.
#[test]
fn estimate_polled_beside_writers_and_compactions() {
    /// Above everything the writers allocate if no compaction ever freed a
    /// chunk, so no insert meets the cap.
    const CAP: usize = 8 * 1024 * 1024;
    /// Far above the index of the few hundred keys this test holds.
    const INDEX_BOUND: usize = 1024 * 1024;
    let rounds: u64 = if cfg!(miri) { 2 } else { 40 };
    // Past `ROOT_LEAF_CAP` in both builds, so the root is a tree and the
    // writers take the optimistic path.
    let span: u64 = if cfg!(miri) { 40 } else { 96 };
    assert!(span > crate::types::ROOT_LEAF_CAP as u64);
    let m = Arc::new(SyncExpanseBlobMap::with_chunk_size_and_max_capacity(
        4096, CAP,
    ));
    for k in 0..span {
        m.insert(k, &payload(k, 0), 0).unwrap();
    }
    assert!(m.shared.published().is_tree());
    let stop = Arc::new(AtomicBool::new(false));
    let polls = Arc::new(AtomicUsize::new(0));
    let poller = {
        let (m, stop, polls) = (Arc::clone(&m), Arc::clone(&stop), Arc::clone(&polls));
        std::thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                let est = m.mem_used_estimate();
                assert!(est >= 4096, "estimate {est} lost the kept keys' chunk");
                assert!(est <= CAP + INDEX_BOUND, "estimate {est} above the cap");
                polls.fetch_add(1, Ordering::Relaxed);
                std::thread::yield_now();
            }
        })
    };
    let writers: Vec<_> = (0..3u64)
        .map(|t| {
            let m = Arc::clone(&m);
            std::thread::spawn(move || {
                let base = 0x9000_0000 + t * 0x1_0000;
                for round in 0..rounds {
                    for i in 0..span {
                        m.insert(base + i, &payload(i, round), 0).unwrap();
                    }
                    for i in 0..span {
                        assert!(m.remove(base + i));
                    }
                }
            })
        })
        .collect();
    for _ in 0..rounds {
        m.compact().expect("compaction");
        std::thread::yield_now();
    }
    for w in writers {
        w.join().expect("writer panicked");
    }
    // The poller must have read at least once beside the work above.
    let deadline = Instant::now() + DEADLINE;
    while polls.load(Ordering::Relaxed) == 0 {
        assert!(Instant::now() < deadline, "the poller never ran");
        std::thread::yield_now();
    }
    stop.store(true, Ordering::Release);
    poller.join().expect("poller panicked");
    assert_quiet_equal(&m);
    assert_eq!(m.len(), span);
}
