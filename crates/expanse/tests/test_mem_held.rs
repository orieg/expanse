//! `mem_held()` accounts for every byte a plain tree holds from the global
//! allocator, while `mem_used()` omits the freed blocks the tree keeps on
//! its per-tree freelists and the unused blocks of its slab pages.
//!
//! A counting global allocator records, for this test's thread only, the
//! bytes live through the global allocator, each request rounded up to its
//! alignment — the rule `NodeAlloc` charges by (`accounted_size`). For the
//! integer trees `mem_held()` must equal that total exactly. The string map
//! also holds suffix leaves its walk charges at their unrounded layout size,
//! so there it is bracketed: at least the requested bytes, at most the
//! rounded ones.
//!
//! Before `mem_held()` existed the only figure was `mem_used()`, which
//! fails the lower bound on every workload here: random inserts grow leaves
//! through size classes and leave the outgrown blocks on the freelists.
//! Substituting `mem_used` for `mem_held` in `check` reproduces that.
//!
//! Excluded from Miri: it installs a global allocator and builds trees of
//! tens of thousands of keys, and what it checks is byte accounting that
//! Miri's model does not change.
#![cfg(not(miri))]

use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use expanse_trie::strmap::{ExpanseStrMap, NulFreeStr};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::fmt::Write as _;

thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static LIVE_BYTES: Cell<isize> = const { Cell::new(0) };
    static LIVE_ROUNDED: Cell<isize> = const { Cell::new(0) };
}

struct Counting;

fn note(size: usize, align: usize, sign: isize) {
    // `try_with`: the thread-locals may already be torn down while a
    // thread's own exit frees memory.
    let _ = TRACK.try_with(|t| {
        if t.get() {
            let rounded = size.next_multiple_of(align);
            LIVE_BYTES.with(|b| b.set(b.get() + sign * size as isize));
            LIVE_ROUNDED.with(|c| c.set(c.get() + sign * rounded as isize));
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
            note(l.size(), l.align(), 1);
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        // SAFETY: forwarded with the caller's layout.
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            note(l.size(), l.align(), 1);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        note(l.size(), l.align(), -1);
        // SAFETY: `p` came from `System` with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        // SAFETY: forwarded with the caller's arguments.
        let q = unsafe { System.realloc(p, l, n) };
        if !q.is_null() {
            note(l.size(), l.align(), -1);
            note(n, l.align(), 1);
        }
        q
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Runs `build` with this thread's allocations counted and returns the tree
/// with the bytes it left live: as requested, and rounded to alignment.
fn tracked<T>(build: impl FnOnce() -> T) -> (T, usize, usize) {
    LIVE_BYTES.with(|b| b.set(0));
    LIVE_ROUNDED.with(|c| c.set(0));
    TRACK.with(|t| t.set(true));
    let tree = build();
    TRACK.with(|t| t.set(false));
    let bytes = LIVE_BYTES.with(Cell::get);
    let rounded = LIVE_ROUNDED.with(Cell::get);
    assert!(
        bytes >= 0 && rounded >= 0,
        "the count saw a free it did not see allocated"
    );
    (tree, bytes as usize, rounded as usize)
}

/// `mem_held()` covers every byte the tree holds from the global allocator
/// and charges nothing it does not hold.
fn check(label: &str, held: usize, used: usize, live_bytes: usize, rounded: usize) {
    assert!(
        held >= live_bytes,
        "{label}: mem_held() = {held} B but the tree holds {live_bytes} B from the global \
         allocator (mem_used() = {used} B)"
    );
    assert!(
        held <= rounded,
        "{label}: mem_held() = {held} B exceeds the {rounded} B the tree holds, rounded to \
         alignment"
    );
    assert!(
        used <= held,
        "{label}: mem_used() = {used} B > mem_held() = {held} B"
    );
}

/// For the integer trees every charge is `accounted_size`, so the rounded
/// total is exact.
fn check_exact(label: &str, held: usize, used: usize, live_bytes: usize, rounded: usize) {
    check(label, held, used, live_bytes, rounded);
    assert_eq!(
        held, rounded,
        "{label}: mem_held() against the rounded live bytes"
    );
}

struct SplitMix(u64);
impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[test]
fn mem_held_brackets_what_the_tree_holds() {
    const N: u64 = 50_000;

    // In debug builds the first removal on a thread grows the thread-local
    // bracket stack (`alloc::bracket_stack`, a `Vec<usize>` behind
    // `#[cfg(debug_assertions)]`) by one 32-byte allocation, which lives
    // until the thread exits. Reach it untracked, so the counts below are the
    // trees' own: without this the emptied-map bracket is 32 B short. Release
    // builds have no bracket stack.
    {
        let mut warm = ExpanseMap::new();
        for k in 0..1_000u64 {
            warm.insert(k.wrapping_mul(0x9E37_79B9_7F4A_7C15), k);
        }
        for k in 0..1_000u64 {
            warm.remove(k.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        }
    }

    let (map, bytes, rounded) = tracked(|| {
        let mut rng = SplitMix(1);
        let mut m = ExpanseMap::new();
        for i in 0..N {
            m.insert(rng.next(), i);
        }
        m
    });
    // The workload must exhibit the defect this guards: freed blocks kept.
    assert!(
        bytes > map.mem_used(),
        "random map: expected retained freelist blocks, but the tree holds {bytes} B and \
         mem_used() = {} B",
        map.mem_used()
    );
    check_exact("random map", map.mem_held(), map.mem_used(), bytes, rounded);
    drop(map);

    let (set, bytes, rounded) = tracked(|| {
        let mut rng = SplitMix(2);
        let mut s = ExpanseSet::new();
        for _ in 0..N {
            s.insert(rng.next());
        }
        s
    });
    check_exact("random set", set.mem_held(), set.mem_used(), bytes, rounded);
    drop(set);

    // Remove every key: nothing is live, yet the tree still holds its pages
    // and freelists until it is dropped.
    let (map, bytes, rounded) = tracked(|| {
        let mut rng = SplitMix(3);
        let mut m = ExpanseMap::new();
        let keys: Vec<u64> = (0..N).map(|_| rng.next()).collect();
        for &k in &keys {
            m.insert(k, k);
        }
        for &k in &keys {
            m.remove(k);
        }
        drop(keys);
        m
    });
    assert_eq!(map.mem_used(), 0, "every key was removed");
    assert!(
        bytes > 0,
        "an emptied tree still holds its slab pages and freelists"
    );
    check_exact(
        "emptied map",
        map.mem_held(),
        map.mem_used(),
        bytes,
        rounded,
    );
    drop(map);

    let (strs, bytes, rounded) = tracked(|| {
        let mut rng = SplitMix(4);
        let mut m = ExpanseStrMap::new();
        let mut buf = String::new();
        for i in 0..N / 5 {
            let (a, b) = (rng.next(), rng.next());
            buf.clear();
            write!(buf, "{a:016x}-{b:016x}").expect("String write");
            let key: &NulFreeStr = buf.as_bytes().try_into().expect("hex has no NUL");
            m.insert(key, i);
        }
        drop(buf);
        m
    });
    if cfg!(feature = "packed-suffix") {
        // Suffix leaves come from `NodeAlloc` too, so the bracket closes.
        check_exact(
            "string map",
            strs.mem_held(),
            strs.mem_used(),
            bytes,
            rounded,
        );
    } else {
        check(
            "string map",
            strs.mem_held(),
            strs.mem_used(),
            bytes,
            rounded,
        );
    }
}

/// `shrink_to_fit()` lowers `mem_held()` by exactly what it reports, leaves
/// `mem_held()` equal to what the tree still holds from the global
/// allocator, moves nothing, and leaves a tree that keeps working. Before it
/// existed an emptied tree held its slab pages and freelists until dropped
/// and `malloc_trim` could not reach them; the emptied case asserts they
/// are all returned.
#[test]
fn shrink_to_fit_returns_retained_memory() {
    const N: u64 = 50_000;
    let keys: Vec<u64> = {
        let mut rng = SplitMix(5);
        (0..N).map(|_| rng.next()).collect()
    };

    let ((map, before, released), bytes, rounded) = tracked(|| {
        let mut m = ExpanseMap::new();
        for (i, &k) in keys.iter().enumerate() {
            m.insert(k, i as u64);
        }
        let before = m.mem_held();
        let released = m.shrink_to_fit();
        (m, before, released)
    });
    assert!(
        released > 0,
        "a random build leaves outgrown blocks to release"
    );
    assert_eq!(map.mem_held(), before - released);
    check_exact("shrunk map", map.mem_held(), map.mem_used(), bytes, rounded);
    map.validate();
    for (i, &k) in keys.iter().enumerate() {
        assert_eq!(map.get(k), Some(i as u64), "key {k:#x} after shrink_to_fit");
    }
    drop(map);

    // The tree keeps working after a shrink: reuse, growth and removal.
    let mut m = ExpanseMap::new();
    for &k in &keys {
        m.insert(k, k);
    }
    m.shrink_to_fit();
    for &k in &keys[..N as usize / 2] {
        assert_eq!(m.remove(k), Some(k));
    }
    m.shrink_to_fit();
    for &k in &keys[..N as usize / 2] {
        m.insert(k, !k);
    }
    m.validate();
    for (i, &k) in keys.iter().enumerate() {
        let want = if i < N as usize / 2 { !k } else { k };
        assert_eq!(m.get(k), Some(want));
    }

    // Emptied: nothing live, so nothing held after a shrink.
    for &k in &keys {
        m.remove(k);
    }
    assert_eq!(m.mem_used(), 0);
    assert!(
        m.mem_held() > 0,
        "an emptied tree still holds its pages until shrunk"
    );
    m.shrink_to_fit();
    assert_eq!(m.mem_held(), 0, "every page and free block returned");

    let mut s = ExpanseSet::new();
    for &k in &keys {
        s.insert(k);
    }
    let before = s.mem_held();
    let released = s.shrink_to_fit();
    assert_eq!(s.mem_held(), before - released);
    s.validate();
    assert!(keys.iter().all(|&k| s.contains(k)));
}
