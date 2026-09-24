//! `mem_held` and `shrink_to_fit` on the 32-bit engine (#1135).
//!
//! A 32-bit tree frees each node as it is removed, but its arena's slot
//! table and free-handle stack keep their peak capacity. `mem_held` counts
//! them and `shrink_to_fit` returns the unused part. A counting global
//! allocator records the bytes live through it on this thread; the
//! assertions are on those bytes, never on RSS.
//!
//! Excluded from Miri: it installs a global allocator and builds trees of
//! tens of thousands of keys, and what it checks is byte accounting.
#![cfg(not(miri))]

use expanse_trie::map32::ExpanseMap32;
use expanse_trie::set32::ExpanseSet32;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static LIVE: Cell<isize> = const { Cell::new(0) };
}

struct Counting;

fn note(size: usize, sign: isize) {
    // `try_with`: the thread-locals may already be torn down while a
    // thread's own exit frees memory.
    let _ = TRACK.try_with(|t| {
        if t.get() {
            LIVE.with(|b| b.set(b.get() + sign * size as isize));
        }
    });
}

// SAFETY: forwards every call to `System` unchanged; the bookkeeping touches
// only thread-local integers and never allocates.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        // SAFETY: forwarded with the caller's layout.
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            note(l.size(), 1);
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        // SAFETY: forwarded with the caller's layout.
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            note(l.size(), 1);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        note(l.size(), -1);
        // SAFETY: `p` came from `System` with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        // SAFETY: forwarded with the caller's arguments.
        let q = unsafe { System.realloc(p, l, n) };
        if !q.is_null() {
            note(l.size(), -1);
            note(n, 1);
        }
        q
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn live() -> isize {
    LIVE.with(Cell::get)
}

fn start_counting() {
    LIVE.with(|b| b.set(0));
    TRACK.with(|t| t.set(true));
}

fn stop_counting() {
    TRACK.with(|t| t.set(false));
}

/// Keys spread over the whole 32-bit space, and the same keys shuffled.
fn keys() -> (Vec<u32>, Vec<u32>) {
    let mut x = 0x9E37_79B9u32;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        x
    };
    let mut ks: Vec<u32> = (0..40_000).map(|_| next()).collect();
    ks.sort_unstable();
    ks.dedup();
    let mut order = ks.clone();
    for i in (1..order.len()).rev() {
        order.swap(i, (next() as usize) % (i + 1));
    }
    (ks, order)
}

#[test]
fn a_drained_map32_keeps_its_tables_until_shrink_to_fit() {
    let (ks, order) = keys();
    start_counting();
    let mut m = ExpanseMap32::new();
    let empty = live();
    for &k in &ks {
        m.insert(k, !k);
    }
    assert!(m.mem_held() >= m.mem_used());
    for &k in &order {
        assert_eq!(m.remove(k), Some(!k));
    }
    assert!(m.is_empty());
    assert_eq!(m.mem_used(), 0);
    let held = live() - empty;
    assert!(held > 0, "the drained map keeps its arena tables");
    assert_eq!(
        m.mem_held() as isize,
        held,
        "mem_held counts every held byte"
    );
    assert_eq!(m.shrink_to_fit() as isize, held);
    assert_eq!(
        live(),
        empty,
        "after shrink_to_fit it holds what a new map does"
    );
    assert_eq!(m.mem_held(), 0);
    assert_eq!(m.shrink_to_fit(), 0, "nothing left to release");
    // It keeps working afterwards.
    for &k in &ks[..1_000] {
        m.insert(k, k);
    }
    assert_eq!(m.get(ks[999]), Some(ks[999]));
    stop_counting();
}

#[test]
fn a_drained_set32_keeps_its_tables_until_shrink_to_fit() {
    let (ks, order) = keys();
    start_counting();
    let mut s = ExpanseSet32::new();
    let empty = live();
    for &k in &ks {
        s.insert(k);
    }
    for &k in &order {
        assert!(s.remove(k));
    }
    assert!(s.is_empty());
    let held = live() - empty;
    assert!(held > 0, "the drained set keeps its arena tables");
    assert_eq!(s.mem_held() as isize, held);
    assert_eq!(s.shrink_to_fit() as isize, held);
    assert_eq!(live(), empty);
    assert_eq!(s.mem_held(), 0);
    stop_counting();
}

#[test]
fn a_partly_drained_map32_releases_exactly_what_it_reports() {
    let (ks, order) = keys();
    start_counting();
    let mut m = ExpanseMap32::new();
    for &k in &ks {
        m.insert(k, k);
    }
    // Remove the later-inserted half in shuffled order: their slots sit at
    // the table's tail, so some of it becomes trailing and releasable.
    let keep: std::collections::BTreeSet<u32> = ks[..ks.len() / 2].iter().copied().collect();
    for &k in &order {
        if !keep.contains(&k) {
            assert_eq!(m.remove(k), Some(k));
        }
    }
    let (held, used, before) = (m.mem_held(), m.mem_used(), live());
    let released = m.shrink_to_fit();
    assert_eq!(
        live(),
        before - released as isize,
        "the allocator sees exactly the bytes shrink_to_fit reports"
    );
    assert_eq!(m.mem_held(), held - released);
    assert_eq!(m.mem_used(), used, "mem_used is unchanged");
    for &k in &keep {
        assert_eq!(m.get(k), Some(k), "key {k:#x} after shrink_to_fit");
    }
    stop_counting();
}
