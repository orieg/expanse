//! What an emptied `ExpanseMap` or `ExpanseSet` holds, on each way of
//! emptying it.
//!
//! The tree's allocator keeps freed blocks and slab pages for reuse. `clear`
//! returns them, so a cleared tree holds no more heap than a new one. A tree
//! drained by `remove` keeps them for the next insert, and `shrink_to_fit`
//! returns them; the rustdoc of `remove` sends callers there.
//! `ExpanseStrMap` behaves the same; its twin is
//! `tests/test_strmap_drain_release.rs`.
//!
//! A counting global allocator records the bytes live through it on this
//! thread only; the assertions are on those bytes, never on RSS. Each case's
//! control asserts that the tree held idle blocks before it was emptied, so
//! the emptied-tree assertion cannot pass on a workload that never retained
//! anything.
//!
//! Excluded from Miri: it installs a global allocator and builds trees of
//! tens of thousands of keys, and what it checks is byte accounting.
#![cfg(not(miri))]

use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
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

/// Keys drawn from the whole 64-bit space, and the same keys shuffled for
/// removal. Built before counting starts.
fn keys() -> (Vec<u64>, Vec<u64>) {
    let mut x = 0x1234_5678_9ABC_DEF1u64;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let ks: Vec<u64> = (0..50_000).map(|_| next()).collect();
    let mut order = ks.clone();
    for i in (1..order.len()).rev() {
        order.swap(i, (next() % (i as u64 + 1)) as usize);
    }
    (ks, order)
}

/// A throwaway map and set built and emptied first, so a lazily created
/// per-thread structure (the debug build's bracket stack) is not charged
/// to the tree under test; then counting starts from zero.
fn start_counting(ks: &[u64]) {
    let mut warm_map = ExpanseMap::new();
    let mut warm_set = ExpanseSet::new();
    for &k in &ks[..1_000] {
        warm_map.insert(k, k);
        warm_set.insert(k);
    }
    for &k in &ks[..1_000] {
        warm_map.remove(k);
        warm_set.remove(k);
    }
    drop((warm_map, warm_set));
    LIVE.with(|b| b.set(0));
    TRACK.with(|t| t.set(true));
}

fn stop_counting() {
    TRACK.with(|t| t.set(false));
}

#[test]
fn trees_emptied_by_clear_hold_what_new_ones_do() {
    let (ks, order) = keys();
    start_counting(&ks);
    let mut m = ExpanseMap::new();
    let mut s = ExpanseSet::new();
    let empty = live();
    // Twice: the second fill runs on allocators the first clear emptied.
    for round in 0..2 {
        for &k in &ks {
            m.insert(k, k);
            s.insert(k);
        }
        // Remove half first, so the allocators hold freed blocks when
        // `clear` runs, not only live ones.
        for &k in &order[..ks.len() / 2] {
            m.remove(k);
            s.remove(k);
        }
        assert!(
            m.mem_held() > m.mem_used() && s.mem_held() > s.mem_used(),
            "control, round {round}: the half-drained trees keep idle blocks"
        );
        m.clear();
        s.clear();
        assert_eq!(
            live(),
            empty,
            "round {round}: trees emptied by clear hold {} B more than new ones",
            live() - empty
        );
        assert_eq!(m.mem_held() + s.mem_held(), 0);
    }
    stop_counting();
}

#[test]
fn a_map_drained_by_remove_keeps_its_blocks_until_shrink_to_fit() {
    let (ks, order) = keys();
    start_counting(&ks);
    let mut m = ExpanseMap::new();
    let empty = live();
    for &k in &ks {
        m.insert(k, !k);
    }
    for &k in &order {
        assert_eq!(m.remove(k), Some(!k));
    }
    assert!(m.is_empty());
    // Control and contract: the drained map still holds its blocks.
    let held = live() - empty;
    assert!(held > 0, "the drained map holds its freed blocks for reuse");
    assert_eq!(m.mem_held() as isize, held);
    assert_eq!(
        m.shrink_to_fit() as isize,
        held,
        "shrink_to_fit returns all of it"
    );
    assert_eq!(
        live(),
        empty,
        "a drained map after shrink_to_fit holds what a new one does"
    );
    // And it keeps working afterwards.
    for &k in &ks[..1_000] {
        m.insert(k, k);
    }
    assert_eq!(m.len(), 1_000);
    stop_counting();
}

#[test]
fn a_set_drained_by_remove_keeps_its_blocks_until_shrink_to_fit() {
    let (ks, order) = keys();
    start_counting(&ks);
    let mut s = ExpanseSet::new();
    let empty = live();
    for &k in &ks {
        s.insert(k);
    }
    for &k in &order {
        assert!(s.remove(k));
    }
    assert!(s.is_empty());
    let held = live() - empty;
    assert!(held > 0, "the drained set holds its freed blocks for reuse");
    assert_eq!(s.mem_held() as isize, held);
    assert_eq!(
        s.shrink_to_fit() as isize,
        held,
        "shrink_to_fit returns all of it"
    );
    assert_eq!(
        live(),
        empty,
        "a drained set after shrink_to_fit holds what a new one does"
    );
    for &k in &ks[..1_000] {
        s.insert(k);
    }
    assert_eq!(s.len(), 1_000);
    stop_counting();
}

/// 256 children under one byte, four keys each: the branch at that byte is
/// a `BranchU`. Built before counting starts.
fn branch_u_keys() -> Vec<u64> {
    (0..256u64)
        .flat_map(|d| (0..4u64).map(move |j| (d << 32) | (j * 0x0101_0101)))
        .collect()
}

/// The 4,160-byte `BranchU` a removal downgrades to a `BranchB` is kept on
/// its size-class freelist: it leaves `mem_used`, stays in `mem_held` and in
/// the bytes live through the global allocator, and `shrink_to_fit` returns
/// it (Refs #1680). Without a size class for it the downgrade frees the block
/// to the system allocator, and the census assertion below fails.
#[test]
#[cfg(target_pointer_width = "64")]
fn a_downgraded_branch_u_is_kept_until_shrink_to_fit() {
    const BRANCH_U: usize = 4160;
    let (warm, _) = keys();
    let ks = branch_u_keys();
    start_counting(&warm);
    let mut m = ExpanseMap::new();
    let empty = live();
    for &k in &ks {
        m.insert(k, !k);
    }
    // Control: the tree holds exactly one BranchU, and nothing of its size
    // is idle yet.
    assert_eq!(m.stats().node_counts.branch_u, 1);
    assert_eq!(m.stats().node_bytes.branch_u, BRANCH_U);
    assert!(m.alloc_census().system_free.iter().all(|f| f.0 != BRANCH_U));
    // Remove the children of the upper digits until the branch is below its
    // demotion floor, whatever that floor is: the node count says when.
    let mut removed = 0;
    for &k in ks.iter().rev() {
        assert_eq!(m.remove(k), Some(!k));
        removed += 1;
        if m.stats().node_counts.branch_u == 0 {
            break;
        }
    }
    assert!(removed < ks.len(), "the branch was demoted, not emptied");
    assert!(!m.is_empty());
    assert_eq!(m.stats().node_bytes.branch_u, 0);

    // In a block: the census owns heap memory, which must be gone before the
    // counted bytes are read below.
    {
        let census = m.alloc_census();
        let mut kept = census.system_free.iter().filter(|f| f.0 == BRANCH_U);
        assert_eq!(
            kept.next(),
            Some(&(BRANCH_U, 1, BRANCH_U)),
            "the demoted BranchU is on its class freelist"
        );
        assert_eq!(kept.next(), None);
    }
    let used = m.mem_used();
    assert_eq!(m.stats().node_bytes.total(), used);
    assert!(m.mem_held() >= used + BRANCH_U);
    assert_eq!(live() - empty, m.mem_held() as isize);

    let released = m.shrink_to_fit();
    assert!(released >= BRANCH_U);
    assert_eq!(m.mem_used(), used, "shrink_to_fit moves nothing live");
    assert!(
        m.alloc_census().system_free.iter().all(|f| f.0 != BRANCH_U),
        "shrink_to_fit returned the block"
    );
    assert_eq!(live() - empty, m.mem_held() as isize);
    for &k in &ks[..ks.len() - removed] {
        assert_eq!(m.get(k), Some(!k));
    }
    stop_counting();
}
