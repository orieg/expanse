//! `mem_held` and `shrink_to_fit` on the concurrent wrappers (#1135).
//!
//! A tree shared through a `Sync*` wrapper frees nodes into its epoch
//! collector: retired blocks wait out their grace period in the collector's
//! bins, then go onto its freelists for reuse. The tree's own `mem_held`
//! cannot see either, so before this change a half-drained
//! `SyncExpanseStrMap` reported `mem_held == mem_used` and released nothing
//! (the #1126 reproduction). The wrappers' `mem_held` now adds what the
//! collector holds, and `shrink_to_fit` returns its freelists.
//!
//! A counting global allocator records the bytes live through it (all
//! threads); the byte assertions run single-threaded and are on those
//! bytes, never on RSS.
//!
//! Excluded from Miri: it installs a global allocator, builds maps of
//! hundreds of thousands of keys and spawns threads; the collector's release
//! path is covered under Miri by `sync::miri_tests`.
#![cfg(not(miri))]

use expanse_trie::strmap::NulFreeStr;
use expanse_trie::sync::{SyncExpanseMap, SyncExpanseSet, SyncExpanseStrMap};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

static LIVE: AtomicIsize = AtomicIsize::new(0);

struct Counting;

// SAFETY: every method forwards to `System` unchanged; the bookkeeping is one
// atomic add and never allocates.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        // SAFETY: forwarded with the caller's layout.
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            LIVE.fetch_add(l.size() as isize, Ordering::Relaxed);
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        // SAFETY: forwarded with the caller's layout.
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            LIVE.fetch_add(l.size() as isize, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size() as isize, Ordering::Relaxed);
        // SAFETY: `p` came from `System` with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        // SAFETY: forwarded with the caller's arguments.
        let q = unsafe { System.realloc(p, l, n) };
        if !q.is_null() {
            LIVE.fetch_add(n as isize - l.size() as isize, Ordering::Relaxed);
        }
        q
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn live() -> isize {
    LIVE.load(Ordering::SeqCst)
}

/// Serialises the byte-counting tests: the counter is process-wide.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

const N: u64 = 200_000;

fn key(i: u64) -> Vec<u8> {
    format!("key:{i:012}").into_bytes()
}

/// After a half drain: the wrapper reports more than it uses, and
/// `shrink_to_fit` releases exactly what the allocator sees freed and what
/// `mem_held` stops counting.
fn check_release(
    label: &str,
    used: usize,
    held: usize,
    shrink: impl FnOnce() -> usize,
    held_after: impl FnOnce() -> usize,
) {
    assert!(
        held > used,
        "{label}: mem_held {held} does not exceed mem_used {used}: the collector's blocks are not counted"
    );
    let before = live();
    let released = shrink();
    assert!(released > 0, "{label}: shrink_to_fit released nothing");
    assert_eq!(
        live(),
        before - released as isize,
        "{label}: the allocator sees exactly the bytes shrink_to_fit reports"
    );
    assert_eq!(held_after(), held - released, "{label}: mem_held after");
}

#[test]
fn a_half_drained_sync_str_map_reports_and_returns_its_collector_blocks() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let m = SyncExpanseStrMap::new();
    for i in 0..N {
        m.insert(NulFreeStr::new(&key(i)).unwrap(), i);
    }
    for i in (0..N).step_by(2) {
        assert_eq!(m.remove(NulFreeStr::new(&key(i)).unwrap()), Some(i));
    }
    let used = m.with_locked(|t| t.mem_used());
    check_release(
        "strmap",
        used,
        m.mem_held(),
        || m.shrink_to_fit(),
        || m.mem_held(),
    );
    for i in (1..N).step_by(2) {
        assert_eq!(m.get(NulFreeStr::new(&key(i)).unwrap()), Some(i));
    }
}

#[test]
fn a_half_drained_sync_map_reports_and_returns_its_collector_blocks() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let m = SyncExpanseMap::new();
    for i in 0..N {
        m.insert(i.wrapping_mul(0x9E37_79B9_7F4A_7C15), i);
    }
    for i in (0..N).step_by(2) {
        assert_eq!(m.remove(i.wrapping_mul(0x9E37_79B9_7F4A_7C15)), Some(i));
    }
    check_release(
        "map",
        m.mem_used(),
        m.mem_held(),
        || m.shrink_to_fit(),
        || m.mem_held(),
    );
    for i in (1..N).step_by(2) {
        assert_eq!(m.get(i.wrapping_mul(0x9E37_79B9_7F4A_7C15)), Some(i));
    }
}

#[test]
fn a_half_drained_sync_set_reports_and_returns_its_collector_blocks() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let s = SyncExpanseSet::new();
    for i in 0..N {
        s.insert(i.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    }
    for i in (0..N).step_by(2) {
        assert!(s.remove(i.wrapping_mul(0x9E37_79B9_7F4A_7C15)));
    }
    let used = s.with_locked(|t| t.mem_used());
    check_release(
        "set",
        used,
        s.mem_held(),
        || s.shrink_to_fit(),
        || s.mem_held(),
    );
    for i in (1..N).step_by(2) {
        assert!(s.contains(i.wrapping_mul(0x9E37_79B9_7F4A_7C15)));
    }
}

/// `shrink_to_fit` runs beside a writer and readers without excluding
/// them; every key a reader looks up keeps its value.
#[test]
fn shrink_to_fit_runs_beside_a_writer_and_readers() {
    // Held too: this test's allocations would land inside another test's
    // counted window, since the counter is process-wide.
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let m = Arc::new(SyncExpanseMap::new());
    for i in 0..20_000u64 {
        m.insert(i, i);
    }
    let stop = Arc::new(AtomicBool::new(false));
    let writer = {
        let (m, stop) = (Arc::clone(&m), Arc::clone(&stop));
        std::thread::spawn(move || {
            let mut round = 0u64;
            while !stop.load(Ordering::Relaxed) {
                // Churn keys above the stable range: insert, then remove.
                let base = 1_000_000 + (round % 8) * 10_000;
                for k in base..base + 2_000 {
                    m.insert(k, k);
                }
                for k in base..base + 2_000 {
                    assert_eq!(m.remove(k), Some(k));
                }
                round += 1;
            }
        })
    };
    let readers: Vec<_> = (0..2)
        .map(|t| {
            let (m, stop) = (Arc::clone(&m), Arc::clone(&stop));
            std::thread::spawn(move || {
                let mut i = t;
                while !stop.load(Ordering::Relaxed) {
                    let k = i % 20_000;
                    assert_eq!(m.get(k), Some(k), "stable key {k}");
                    i = i.wrapping_add(7);
                }
            })
        })
        .collect();
    let mut released = 0;
    for _ in 0..200 {
        released += m.shrink_to_fit();
        std::thread::yield_now();
    }
    stop.store(true, Ordering::Relaxed);
    writer.join().unwrap();
    for r in readers {
        r.join().unwrap();
    }
    for k in 0..20_000u64 {
        assert_eq!(m.get(k), Some(k));
    }
    // Not asserted: how much a concurrent run releases depends on the
    // interleaving. Recorded so a failure shows it.
    let _ = released;
}
