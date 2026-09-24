//! What an emptied `ExpanseStrMap` holds, on each way of emptying it.
//!
//! The map's allocator keeps freed blocks and slab pages for reuse, and a
//! map drained by `remove` keeps them for its next insert (#1119).
//! `shrink_to_fit` returns them, after which the map holds no more heap
//! than a new one; `clear` returns them itself. The workload is the one that
//! surfaced the retention: 17-byte keys of a 5-byte group prefix, `t/` and a
//! 10-byte id, both in a 7-bit big-endian encoding offset by one, drained
//! one group at a time in key order.
//!
//! A counting global allocator records the bytes live through it on this
//! thread only; the assertions are on those bytes, never on RSS. The
//! workload's control asserts that the map does keep idle blocks while
//! keys remain, so the emptied-map assertion cannot pass on a workload that
//! never retained anything.
//!
//! Excluded from Miri: it installs a global allocator and builds maps of
//! thousands of keys, and what it checks is byte accounting.
#![cfg(not(miri))]

use expanse_trie::strmap::{ExpanseStrMap, NulFreeStr};
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

/// Bytes live through the global allocator on this thread since counting
/// started.
fn live() -> isize {
    LIVE.with(Cell::get)
}

/// Starts counting from zero. A throwaway insert and remove first, so a
/// lazily created per-thread structure (the debug build's bracket stack)
/// is not charged to the map under test.
fn start_counting() {
    let mut warm = ExpanseStrMap::new();
    let mut buf = Vec::new();
    for id in 0..PER_GROUP {
        key(&mut buf, 0, id);
        warm.insert(nul_free(&buf), id);
    }
    for id in 0..PER_GROUP {
        key(&mut buf, 0, id);
        warm.remove(nul_free(&buf));
    }
    drop((warm, buf));
    LIVE.with(|b| b.set(0));
    TRACK.with(|t| t.set(true));
}

const GROUPS: u64 = 200;
const PER_GROUP: u64 = 100;

fn enc7(out: &mut Vec<u8>, n: u64, width: u32) {
    for i in (0..width).rev() {
        out.push(((n >> (7 * i)) & 0x7F) as u8 + 1);
    }
}

fn key(buf: &mut Vec<u8>, group: u64, id: u64) {
    buf.clear();
    enc7(buf, group, 5);
    buf.extend_from_slice(b"t/");
    enc7(buf, id, 10);
}

fn nul_free(b: &[u8]) -> &NulFreeStr {
    NulFreeStr::new(b).expect("encoded keys carry no NUL")
}

/// Every group's keys, inserted group by group in key order.
fn fill(m: &mut ExpanseStrMap, buf: &mut Vec<u8>) {
    for g in 0..GROUPS {
        for r in 0..PER_GROUP {
            let id = g * PER_GROUP + r;
            key(buf, g, id);
            assert_eq!(m.insert(nul_free(buf), id), None);
        }
    }
}

/// Drains the map one group at a time: an ordered scan of the group, then
/// its keys removed in ascending order. Returns, from just before the last
/// removal, the bytes live on this thread beyond the map's `mem_used()`.
fn drain(m: &mut ExpanseStrMap, buf: &mut Vec<u8>, empty: isize) -> isize {
    let mut idle_before_last = 0;
    for g in 0..GROUPS {
        let mut prefix = Vec::with_capacity(7);
        enc7(&mut prefix, g, 5);
        prefix.extend_from_slice(b"t/");
        let mut seen = 0;
        {
            let mut c = m.cursor_at_or_after(nul_free(&prefix));
            while let Some((k, _)) = c.next() {
                if !k.starts_with(&prefix) {
                    break;
                }
                seen += 1;
            }
        }
        drop(prefix);
        assert_eq!(seen, PER_GROUP, "group {g} scans whole");
        for r in 0..PER_GROUP {
            let id = g * PER_GROUP + r;
            key(buf, g, id);
            if m.len() == 1 {
                idle_before_last = live() - empty - m.mem_used() as isize;
            }
            assert_eq!(m.remove(nul_free(buf)), Some(id));
        }
    }
    assert!(m.is_empty());
    idle_before_last
}

#[test]
fn a_map_drained_by_remove_holds_what_a_new_map_does_after_shrink_to_fit() {
    let mut buf = Vec::with_capacity(17);
    start_counting();
    let mut m = ExpanseStrMap::new();
    let empty = live();

    fill(&mut m, &mut buf);
    let idle = drain(&mut m, &mut buf, empty);
    // Control: with one key left the map keeps idle blocks, so the drain
    // exercised the retention the next assertion is about.
    assert!(
        idle > 0,
        "control: with one key left the map holds {idle} B beyond mem_used(); \
         the workload never retained a block"
    );
    // The drained map keeps its blocks for the next insert, and reports
    // them; shrink_to_fit returns all of them.
    let held = live() - empty;
    assert!(
        held > 0,
        "a map drained by remove keeps its blocks for reuse"
    );
    assert_eq!(m.mem_held() as isize, held);
    assert_eq!(m.shrink_to_fit() as isize, held);
    assert_eq!(
        live(),
        empty,
        "a drained map after shrink_to_fit holds {} B more than a new map",
        live() - empty
    );
    assert_eq!(m.mem_used(), 0);

    // The released pages are not reachable any more: the map refills and
    // drains again with every value intact.
    fill(&mut m, &mut buf);
    for g in 0..GROUPS {
        key(&mut buf, g, g * PER_GROUP);
        assert_eq!(m.get(nul_free(&buf)), Some(g * PER_GROUP));
    }
    drain(&mut m, &mut buf, empty);
    m.shrink_to_fit();
    assert_eq!(live(), empty, "after a second fill, drain and shrink");
    TRACK.with(|t| t.set(false));
}

#[test]
fn a_map_emptied_by_clear_holds_what_a_new_map_does() {
    let mut buf = Vec::with_capacity(17);
    start_counting();
    let mut m = ExpanseStrMap::new();
    let empty = live();

    fill(&mut m, &mut buf);
    // Remove half the groups first so the allocator holds freed blocks
    // when `clear` runs, not only live ones.
    for g in 0..GROUPS / 2 {
        for r in 0..PER_GROUP {
            key(&mut buf, g, g * PER_GROUP + r);
            m.remove(nul_free(&buf));
        }
    }
    assert!(
        m.mem_held() > m.mem_used(),
        "control: the half-drained map keeps idle blocks"
    );
    m.clear();
    assert_eq!(
        live(),
        empty,
        "a map emptied by clear holds {} B more than a new map",
        live() - empty
    );
    assert_eq!(m.mem_held(), 0);
    TRACK.with(|t| t.set(false));
}

/// A small map emptied by `remove` keeps its few pages for the next insert
/// (#1119): a map that repeatedly empties at a small population must not
/// return and re-carve them every time. `shrink_to_fit` still returns them.
#[test]
fn a_small_map_emptied_by_remove_keeps_its_pages_until_shrink_to_fit() {
    let mut buf = Vec::with_capacity(17);
    start_counting();
    let mut m = ExpanseStrMap::new();
    let empty = live();
    for round in 0..3 {
        for id in 0..10 {
            key(&mut buf, 0, id);
            m.insert(nul_free(&buf), id);
        }
        for id in 0..10 {
            key(&mut buf, 0, id);
            assert_eq!(m.remove(nul_free(&buf)), Some(id));
        }
        assert!(m.is_empty());
        let held = live() - empty;
        assert!(
            held > 0,
            "round {round}: a 10-key map emptied by remove keeps its pages"
        );
        assert_eq!(m.mem_held() as isize, held, "round {round}");
    }
    let held = live() - empty;
    assert_eq!(m.shrink_to_fit() as isize, held);
    assert_eq!(live(), empty, "shrink_to_fit returns all of it");
    TRACK.with(|t| t.set(false));
}
