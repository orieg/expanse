//! Bytes one small JudyL array holds from the allocator, through the C ABI.
//!
//! A hash table of JudyL buckets keeps most of its arrays at a handful of
//! keys, so the bytes a 1-, 10-, 20- or 31-key array holds decide such a
//! program's resident set. `JudyLMemUsed` reports live node bytes and leaves
//! out what the array holds beyond them, so this test counts at the
//! allocator instead: a counting global allocator records, for this test's
//! thread only, the bytes live after `JudyLIns` has inserted the keys.
//!
//! The table pins the figure exactly. Byte accounting is deterministic, so a
//! change to the handle, the root leaf or the allocator's page policy shows
//! here as a failed equality and is then a stated decision, not drift.
//!
//! Excluded from Miri: it installs a global allocator, and what it checks is
//! byte accounting that Miri's model does not change.
#![cfg(all(target_pointer_width = "64", not(miri)))]

use core::ffi::c_void;
use core::ptr::null_mut;
use expanse::{JudyLCount, JudyLDel, JudyLFreeArray, JudyLGet, JudyLIns, Word};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static LIVE_BYTES: Cell<isize> = const { Cell::new(0) };
    static LIVE_BLOCKS: Cell<isize> = const { Cell::new(0) };
}

struct Counting;

fn note(size: usize, sign: isize) {
    // `try_with`: the thread-locals may already be torn down while a
    // thread's own exit frees memory. A failed access counts nothing, which
    // is the intent there.
    if let Ok(true) = TRACK.try_with(Cell::get) {
        LIVE_BYTES.with(|b| b.set(b.get() + sign * size as isize));
        LIVE_BLOCKS.with(|c| c.set(c.get() + sign));
    }
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

/// This thread's live bytes and live blocks since tracking began.
fn live() -> (usize, usize) {
    let (bytes, blocks) = (LIVE_BYTES.with(Cell::get), LIVE_BLOCKS.with(Cell::get));
    assert!(
        bytes >= 0 && blocks >= 0,
        "the count saw a free it did not see allocated"
    );
    (bytes as usize, blocks as usize)
}

/// The `i`th key of an array: distinct, spread over the whole word.
fn key(i: usize) -> Word {
    (i as Word + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// Builds a `keys`-key array, returns the bytes and blocks it holds, checks
/// it reads back, and checks both ways of ending its life return every byte.
fn held(keys: usize) -> (usize, usize) {
    LIVE_BYTES.with(|b| b.set(0));
    LIVE_BLOCKS.with(|c| c.set(0));
    TRACK.with(|t| t.set(true));
    let mut observed = (0, 0);
    for by_delete in [false, true] {
        let mut arr: *mut c_void = null_mut();
        // SAFETY: standard JudyL usage; each slot is written before the next
        // call that mutates the array.
        unsafe {
            for i in 0..keys {
                *JudyLIns(&raw mut arr, key(i), null_mut()).cast::<Word>() = i;
            }
            observed = live();
            assert_eq!(JudyLCount(arr, 0, Word::MAX, null_mut()), keys);
            for i in 0..keys {
                assert_eq!(*JudyLGet(arr, key(i), null_mut()).cast::<Word>(), i);
            }
            if by_delete {
                for i in 0..keys {
                    assert_eq!(JudyLDel(&raw mut arr, key(i), null_mut()), 1);
                }
            } else {
                JudyLFreeArray(&raw mut arr, null_mut());
            }
        }
        assert!(arr.is_null(), "{keys} keys: the array word is not null");
        assert_eq!(live(), (0, 0), "{keys} keys: bytes left after the array");
    }
    TRACK.with(|t| t.set(false));
    observed
}

/// One test, so the table is read on one thread in one order.
#[test]
fn bytes_held_by_a_small_judyl_array() {
    // (keys, bytes held, blocks held). The 712-byte boxed handle is one
    // block; each 4 KiB slab page a leaf size class has touched is another,
    // and the 384-byte leaf of 17 to 31 keys is served outside the slabs.
    const EXPECTED: [(usize, usize, usize); 4] = [
        (1, 4_808, 2),
        (10, 21_192, 6),
        (20, 21_576, 7),
        (31, 21_704, 7),
    ];
    let got: Vec<(usize, usize, usize)> = EXPECTED
        .iter()
        .map(|&(keys, _, _)| {
            let (bytes, blocks) = held(keys);
            (keys, bytes, blocks)
        })
        .collect();
    assert_eq!(got, EXPECTED, "(keys, bytes held, blocks held)");
}
