//! The three states of a `JudyL` array word: null, a small block, and a
//! boxed map carrying a tag bit.
//!
//! An array of up to 31 keys is one block; the insert that makes 32 keys
//! replaces it by an `ExpanseMap` whose array word is the box's address plus
//! one. This walks an array through every state against a sorted model and
//! checks each entry point on the way, with no stock library involved, so it
//! runs under Miri: the tag is pointer arithmetic on the box's address, and
//! Miri is what checks that the untagged pointer still reaches the box and
//! that a slot pointer taken before a promotion is not used after it.
//!
//! Stock parity across the same boundary is the `oracle` feature's
//! `judyl_small_form_boundary_matches_stock_libjudy`.
#![cfg(target_pointer_width = "64")]

use core::ffi::c_void;
use core::ptr::null_mut;
use expanse::{
    JudyLByCount, JudyLCount, JudyLDel, JudyLFirst, JudyLFirstEmpty, JudyLFreeArray, JudyLGet,
    JudyLIns, JudyLLast, JudyLLastEmpty, JudyLMemUsed, JudyLNext, JudyLNextEmpty, JudyLPrev,
    JudyLPrevEmpty, Word, expanse_validate,
};
use std::collections::BTreeMap;

/// Keys an array is walked through: both ends of the key space, neighbours,
/// and a spread, so every search has a hit, a neighbour and a miss.
fn key(i: usize) -> Word {
    match i % 4 {
        0 => i as Word,
        1 => Word::MAX - i as Word,
        2 => (i as Word).wrapping_mul(0x9E37_79B9_7F4A_7C15),
        _ => 0x4000 + i as Word,
    }
}

/// Whether the array word is the tagged-map form.
fn is_map(arr: *const c_void) -> bool {
    arr.addr() & 1 != 0
}

/// Inserts `k` and writes `v` through the returned slot.
///
/// # Safety
/// `arr` is a JudyL array word.
unsafe fn put(arr: &mut *mut c_void, k: Word, v: Word) {
    // SAFETY: per this function's contract; the slot is written before any
    // other call on the array.
    unsafe {
        let slot = JudyLIns(arr, k, null_mut()).cast::<Word>();
        assert!(
            !slot.is_null() && slot.addr() != usize::MAX,
            "JudyLIns failed"
        );
        *slot = v;
    }
}

/// Every read-only entry point against the model.
///
/// # Safety
/// `arr` is a JudyL array word.
unsafe fn check(arr: *const c_void, model: &BTreeMap<Word, Word>, at: &str) {
    let mut probes = vec![0, 1, Word::MAX - 1, Word::MAX];
    for &k in model.keys() {
        probes.extend([k.wrapping_sub(1), k, k.wrapping_add(1)]);
    }
    let read = |slot: *mut c_void, idx: Word| {
        // SAFETY: a non-null slot is live; nothing mutates the array here.
        (!slot.is_null()).then(|| (idx, unsafe { *slot.cast::<Word>() }))
    };
    let pair = |e: Option<(&Word, &Word)>| e.map(|(k, v)| (*k, *v));
    // SAFETY: per this function's contract; the array is not mutated.
    unsafe {
        assert_eq!(expanse_validate(arr, 1), 1, "{at}: validate");
        assert_eq!(
            JudyLCount(arr, 0, Word::MAX, null_mut()),
            model.len(),
            "{at}"
        );
        assert_eq!(JudyLMemUsed(arr) == 0, model.is_empty(), "{at}: mem used");
        for &p in &probes {
            let slot = JudyLGet(arr, p, null_mut());
            assert_eq!(
                read(slot, p).map(|e| e.1),
                model.get(&p).copied(),
                "{at}: get {p:#x}"
            );

            let mut i = p;
            let s = JudyLFirst(arr, &raw mut i, null_mut());
            assert_eq!(
                read(s, i),
                pair(model.range(p..).next()),
                "{at}: first {p:#x}"
            );
            let mut i = p;
            let s = JudyLLast(arr, &raw mut i, null_mut());
            assert_eq!(
                read(s, i),
                pair(model.range(..=p).next_back()),
                "{at}: last {p:#x}"
            );
            let mut i = p;
            let s = JudyLNext(arr, &raw mut i, null_mut());
            let want = p.checked_add(1).and_then(|n| pair(model.range(n..).next()));
            assert_eq!(read(s, i), want, "{at}: next {p:#x}");
            let mut i = p;
            let s = JudyLPrev(arr, &raw mut i, null_mut());
            assert_eq!(
                read(s, i),
                pair(model.range(..p).next_back()),
                "{at}: prev {p:#x}"
            );

            let absent = |k: &Word| !model.contains_key(k);
            let mut i = p;
            let rc = JudyLFirstEmpty(arr, &raw mut i, null_mut());
            let want = (p..=Word::MAX).find(absent);
            assert_eq!((rc == 1).then_some(i), want, "{at}: first empty {p:#x}");
            let mut i = p;
            let rc = JudyLLastEmpty(arr, &raw mut i, null_mut());
            let want = (0..=p).rev().find(absent);
            assert_eq!((rc == 1).then_some(i), want, "{at}: last empty {p:#x}");
            let mut i = p;
            let rc = JudyLNextEmpty(arr, &raw mut i, null_mut());
            let want = p.checked_add(1).and_then(|n| (n..=Word::MAX).find(absent));
            assert_eq!((rc == 1).then_some(i), want, "{at}: next empty {p:#x}");
            let mut i = p;
            let rc = JudyLPrevEmpty(arr, &raw mut i, null_mut());
            let want = p.checked_sub(1).and_then(|n| (0..=n).rev().find(absent));
            assert_eq!((rc == 1).then_some(i), want, "{at}: prev empty {p:#x}");

            let n = JudyLCount(arr, p, Word::MAX, null_mut());
            assert_eq!(n, model.range(p..).count(), "{at}: count from {p:#x}");
            let n = JudyLCount(arr, Word::MAX, p, null_mut());
            let want = usize::from(p == Word::MAX && model.contains_key(&p));
            assert_eq!(n, want, "{at}: reversed count {p:#x}");
        }
        for (n, (k, v)) in model.iter().enumerate() {
            let mut i: Word = 0;
            let s = JudyLByCount(arr, n + 1, &raw mut i, null_mut());
            assert_eq!(read(s, i), Some((*k, *v)), "{at}: by count {}", n + 1);
        }
        let mut i: Word = 7;
        assert!(JudyLByCount(arr, 0, &raw mut i, null_mut()).is_null());
        assert!(JudyLByCount(arr, model.len() + 1, &raw mut i, null_mut()).is_null());
    }
}

#[test]
fn an_array_walks_null_small_map_and_back() {
    let mut arr: *mut c_void = null_mut();
    let mut model = BTreeMap::new();
    // SAFETY: `arr` is a JudyL array word throughout.
    unsafe {
        check(arr, &model, "empty");
        for i in 0..31 {
            put(&mut arr, key(i), !key(i));
            model.insert(key(i), !key(i));
            assert!(!arr.is_null() && !is_map(arr), "small at {} keys", i + 1);
            if i % 6 == 0 || i >= 29 {
                check(arr, &model, &format!("small {}", i + 1));
            }
        }
        // A present key at the full population is found, not promoted.
        put(&mut arr, key(5), 55);
        model.insert(key(5), 55);
        assert!(!is_map(arr), "an upsert at 31 keys keeps the small form");

        // The 32nd key promotes; every earlier value survives it.
        put(&mut arr, key(31), 3_131);
        model.insert(key(31), 3_131);
        assert!(is_map(arr), "32 keys is a map");
        check(arr, &model, "map 32");
        for i in 32..36 {
            put(&mut arr, key(i), i);
            model.insert(key(i), i);
        }
        check(arr, &model, "map 36");

        // A map does not come back to a block as it shrinks.
        for i in (4..36).rev() {
            assert_eq!(JudyLDel(&raw mut arr, key(i), null_mut()), 1);
            assert_eq!(JudyLDel(&raw mut arr, key(i), null_mut()), 0);
            model.remove(&key(i));
            assert!(is_map(arr), "still a map at {i} keys");
        }
        check(arr, &model, "map 4");
        for i in 0..4 {
            assert_eq!(JudyLDel(&raw mut arr, key(i), null_mut()), 1);
            model.remove(&key(i));
        }
        assert!(arr.is_null(), "the last delete nulls a map's word");
        check(arr, &model, "emptied map");
    }
}

#[test]
fn a_small_array_deletes_to_null_and_frees() {
    let mut arr: *mut c_void = null_mut();
    let mut model = BTreeMap::new();
    // SAFETY: `arr` is a JudyL array word throughout.
    unsafe {
        assert_eq!(JudyLDel(&raw mut arr, 9, null_mut()), 0);
        for i in 0..12 {
            put(&mut arr, key(i), i);
            model.insert(key(i), i);
        }
        for i in [7, 0, 11, 3, 5, 1, 9, 2, 10, 4, 8, 6] {
            assert_eq!(JudyLDel(&raw mut arr, key(i), null_mut()), 1);
            assert_eq!(JudyLDel(&raw mut arr, key(i), null_mut()), 0);
            model.remove(&key(i));
            check(arr, &model, &format!("after deleting {i}"));
            assert_eq!(arr.is_null(), model.is_empty());
        }
        // Freed while small: the bytes reported are the block's.
        for i in 0..10 {
            put(&mut arr, key(i), i);
        }
        assert_eq!(JudyLMemUsed(arr), 200);
        assert_eq!(JudyLFreeArray(&raw mut arr, null_mut()), 200);
        assert!(arr.is_null());
        assert_eq!(JudyLFreeArray(&raw mut arr, null_mut()), 0);
        // Freed as a map.
        for i in 0..40 {
            put(&mut arr, key(i), i);
        }
        assert!(is_map(arr));
        assert!(JudyLFreeArray(&raw mut arr, null_mut()) > 0);
        assert!(arr.is_null());
    }
}

#[test]
fn a_slot_is_writable_until_the_next_insert_or_delete() {
    let mut arr: *mut c_void = null_mut();
    // SAFETY: each slot is used only before the next call that mutates the
    // array, in both forms.
    unsafe {
        for round in 0..40usize {
            let slot = JudyLIns(&raw mut arr, key(round), null_mut()).cast::<Word>();
            assert_eq!(*slot, 0, "a new slot is zero");
            *slot = round;
            // Lookups and searches between the insert and the next mutation
            // leave the slot valid and return the same slot.
            let again = JudyLGet(arr, key(round), null_mut()).cast::<Word>();
            assert_eq!(again, slot);
            *again += 1;
            assert_eq!(*slot, round + 1);
        }
        for round in 0..40usize {
            assert_eq!(
                *JudyLGet(arr, key(round), null_mut()).cast::<Word>(),
                round + 1
            );
        }
        JudyLFreeArray(&raw mut arr, null_mut());
    }
}
