//! A concurrent map bulk-loaded in random order: what it holds, what
//! `shrink_to_fit` gives back, and what dropping it returns.
//!
//! Random inserts grow nodes through their size classes, so a random-order
//! build retires many more blocks than an ascending one. Behind a `Sync*`
//! wrapper those go to the epoch collector and, past their grace period,
//! onto its freelists. The tree's own `mem_used` sees none of them; these
//! tests pin that the wrapper's `mem_held` does, that `shrink_to_fit` brings
//! the random-order build down to the ascending build's footprint, and that
//! dropping the wrapper returns the tree even while the writer thread's
//! cached reader still holds the collector.
//!
//! A counting global allocator records the bytes live through it. That is
//! exact and allocator-independent; the system allocator's own per-chunk
//! overhead is outside what the crate allocates and is not asserted on.
//!
//! Every measured build runs on a thread of its own: a writer thread caches
//! a reader per collector, and a cache miss on a later build would free an
//! earlier collector in the middle of the later build's measurement.
//!
//! Excluded from Miri: a global allocator and hundreds of thousands of keys.
#![cfg(not(miri))]

use expanse_trie::sync::{SyncExpanseMap, SyncExpanseSet};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

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

static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

const N: u64 = 200_000;

/// Keys in a small build, whose residue after drop is the fixed cost the
/// large builds are held to.
const SMALL: u64 = 1_000;

/// Slack on top of the measured fixed cost: capacity rounding in the
/// collector's registries between a small and a large build.
const SLACK: isize = 4 * 1024;

fn live() -> isize {
    LIVE.load(Ordering::Relaxed)
}

/// 0..n in a Fisher–Yates permutation from a fixed xorshift64 seed.
fn shuffled(n: u64) -> Vec<u64> {
    let mut v: Vec<u64> = (0..n).collect();
    let mut s = 0x2545_F491_4F6C_DD1D_u64;
    for i in (1..v.len()).rev() {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        v.swap(i, (s % (i as u64 + 1)) as usize);
    }
    v
}

/// Runs `f` on a fresh thread and returns its result; the thread's reader
/// cache is gone when this returns.
fn on_fresh_thread<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    std::thread::scope(|s| s.spawn(f).join().expect("measured build panicked"))
}

/// Bytes still live right after a map of `keys` is dropped, with the writer
/// thread (and its cached reader) still alive.
fn map_residue(keys: &[u64]) -> isize {
    on_fresh_thread(|| {
        let before = live();
        let m = SyncExpanseMap::new();
        for &k in keys {
            m.insert(k, !k);
        }
        drop(m);
        live() - before
    })
}

fn set_residue(keys: &[u64]) -> isize {
    on_fresh_thread(|| {
        let before = live();
        let s = SyncExpanseSet::new();
        for &k in keys {
            s.insert(k);
        }
        drop(s);
        live() - before
    })
}

/// Bytes a live map of `keys` holds through the allocator beyond its
/// `mem_held`: the wrapper block and the collector's own structures, which no
/// accessor counts. Read in `build_and_shrink`'s order: `mem_held` locks
/// every freelist, and where the platform's `Mutex` boxes itself on first
/// lock (macOS), that first read allocates.
fn map_uncounted(keys: &[u64]) -> isize {
    on_fresh_thread(|| {
        let before = live();
        let m = SyncExpanseMap::new();
        for &k in keys {
            m.insert(k, !k);
        }
        let held = m.mem_held() as isize;
        live() - before - held
    })
}

/// Builds a map from `keys`, checks `mem_held` against the allocator, shrinks
/// it, and returns the bytes it holds through the allocator afterwards.
fn build_and_shrink(label: &str, keys: &[u64], fixed: isize) -> isize {
    on_fresh_thread(|| {
        let before = live();
        let m = SyncExpanseMap::new();
        for &k in keys {
            m.insert(k, !k);
        }
        let held = m.mem_held() as isize;
        let bytes = live() - before;
        let used = m.mem_used() as isize;
        assert!(
            held <= bytes && bytes - held <= fixed + SLACK,
            "{label}: mem_held {held} B (mem_used {used} B), but the map holds {bytes} B \
             through the allocator: {} B uncounted, fixed cost {fixed} B",
            bytes - held
        );
        let released = m.shrink_to_fit() as isize;
        assert_eq!(live(), before + bytes - released, "{label}: released bytes");
        assert_eq!(
            m.mem_held() as isize,
            held - released,
            "{label}: mem_held after"
        );
        assert_eq!(m.len(), keys.len() as u64);
        for &k in keys.iter().step_by(101) {
            assert_eq!(m.get(k), Some(!k), "{label}: key {k}");
        }
        eprintln!(
            "{label}: {:.2} B/key held before shrink, {:.2} after, mem_used {:.2}",
            held as f64 / N as f64,
            (live() - before) as f64 / N as f64,
            used as f64 / N as f64
        );
        live() - before
    })
}

#[test]
fn a_random_bulk_load_is_counted_and_shrinks_to_an_ascending_footprint() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let fixed = map_uncounted(&shuffled(SMALL));
    let asc: Vec<u64> = (0..N).collect();
    let rnd = shuffled(N);
    let asc_after = build_and_shrink("ascending", &asc, fixed);
    let rnd_after = build_and_shrink("random", &rnd, fixed);
    // The two builds hold one key set, and a digital trie's node census is
    // fixed by the key set; after the shrink only the tree is left.
    assert!(
        rnd_after <= asc_after + SLACK,
        "after shrink_to_fit the random build holds {rnd_after} B, the ascending one {asc_after} B"
    );
    // The case the accounting exists for: before the shrink, the collector
    // held more than the tree, and the tree's own figure cannot see it.
    on_fresh_thread(|| {
        let m = SyncExpanseMap::new();
        for &k in &rnd {
            m.insert(k, !k);
        }
        assert!(
            m.mem_held() > 2 * m.mem_used(),
            "random build: mem_held {} B, mem_used {} B: expected retained freelist blocks",
            m.mem_held(),
            m.mem_used()
        );
    });
}

#[test]
fn dropping_a_sync_map_returns_its_tree_while_the_writer_thread_lives() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let fixed = map_residue(&shuffled(SMALL));
    let left = map_residue(&shuffled(N));
    assert!(
        left <= fixed + SLACK,
        "{left} B live after dropping a {N}-key map, {fixed} B after a {SMALL}-key one: \
         the tree's nodes wait in a collector the writer thread's cached reader keeps alive"
    );
}

#[test]
fn dropping_a_sync_set_returns_its_tree_while_the_writer_thread_lives() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let fixed = set_residue(&shuffled(SMALL));
    let left = set_residue(&shuffled(N));
    assert!(
        left <= fixed + SLACK,
        "{left} B live after dropping a {N}-key set, {fixed} B after a {SMALL}-key one"
    );
}
