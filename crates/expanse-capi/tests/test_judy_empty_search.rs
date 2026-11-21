//! `Judy1`/`JudyL` `FirstEmpty`/`NextEmpty`/`LastEmpty`/`PrevEmpty` against a
//! brute-force model, on populations built to reach each branch of the search:
//! the ordered walk that finds a gap inside a run, the rank bisection a run
//! longer than the walk budget falls back to, runs that end at either edge of
//! the key space (0 and `Word::MAX`), an index that is itself absent, and the
//! null array.
//!
//! A search that finds nothing returns 0 and leaves `*PIndex` untouched
//! (`test_judy_miss_index.rs`); `NextEmpty` from `Word::MAX` and `PrevEmpty`
//! from 0 have no index to search and return 0 the same way.
#![cfg(target_pointer_width = "64")]

use core::ffi::{c_int, c_void};
use core::ptr::null_mut;
use expanse::{
    JError, Judy1FirstEmpty, Judy1FreeArray, Judy1LastEmpty, Judy1NextEmpty, Judy1PrevEmpty,
    Judy1Set, JudyLFirstEmpty, JudyLFreeArray, JudyLIns, JudyLLastEmpty, JudyLNextEmpty,
    JudyLPrevEmpty, Word,
};
use std::collections::BTreeSet;

type Empty = unsafe extern "C" fn(*const c_void, *mut Word, *mut JError) -> c_int;

/// `(name, Judy1 fn, JudyL fn, ascending, strict)`.
const SEARCHES: [(&str, Empty, Empty, bool, bool); 4] = [
    ("FirstEmpty", Judy1FirstEmpty, JudyLFirstEmpty, true, false),
    ("NextEmpty", Judy1NextEmpty, JudyLNextEmpty, true, true),
    ("LastEmpty", Judy1LastEmpty, JudyLLastEmpty, false, false),
    ("PrevEmpty", Judy1PrevEmpty, JudyLPrevEmpty, false, true),
];

/// The walk budget in `compat.rs` is 1024 keys; runs straddle it.
const LONG_RUN: u64 = 3000;

/// Brute-force model: step from the start until a key is absent.
fn model(keys: &BTreeSet<u64>, probe: u64, ascending: bool, strict: bool) -> Option<u64> {
    let step = |k: u64| {
        if ascending {
            k.checked_add(1)
        } else {
            k.checked_sub(1)
        }
    };
    let mut k = if strict { step(probe)? } else { probe };
    while keys.contains(&k) {
        k = step(k)?;
    }
    Some(k)
}

struct Arrays {
    set: *mut c_void,
    map: *mut c_void,
}

impl Arrays {
    fn build(keys: &BTreeSet<u64>) -> Self {
        let (mut set, mut map) = (null_mut(), null_mut());
        // SAFETY: both arrays are owned by the returned value.
        unsafe {
            for &k in keys {
                assert_eq!(Judy1Set(&raw mut set, k as Word, null_mut()), 1);
                *JudyLIns(&raw mut map, k as Word, null_mut()).cast::<Word>() = k as Word;
            }
        }
        Self { set, map }
    }
}

impl Drop for Arrays {
    fn drop(&mut self) {
        // SAFETY: both arrays were built by `Arrays::build` and are freed once.
        unsafe {
            Judy1FreeArray(&raw mut self.set, null_mut());
            JudyLFreeArray(&raw mut self.map, null_mut());
        }
    }
}

/// Checks all eight searches at every probe against the model.
fn check(keys: &BTreeSet<u64>, probes: &[u64]) {
    let arrays = Arrays::build(keys);
    for &p in probes {
        for (name, j1, jl, ascending, strict) in SEARCHES {
            let want = model(keys, p, ascending, strict);
            for (family, f, arr) in [("Judy1", j1, arrays.set), ("JudyL", jl, arrays.map)] {
                let mut index = p as Word;
                // SAFETY: `arr` is a live array or null; `index` is a valid pointer.
                let r = unsafe { f(arr, &raw mut index, null_mut()) };
                match want {
                    Some(k) => assert_eq!((r, index), (1, k as Word), "{family}{name} from {p:#x}"),
                    None => assert_eq!(
                        (r, index),
                        (0, p as Word),
                        "{family}{name} from {p:#x}: a miss leaves the index"
                    ),
                }
            }
        }
    }
}

/// Probes at, just inside and just outside each run's ends.
fn run_probes(runs: &[(u64, u64)]) -> Vec<u64> {
    let mut probes = Vec::new();
    for &(lo, hi) in runs {
        for k in [lo, hi, lo.wrapping_sub(1), hi.wrapping_add(1)] {
            probes.push(k);
        }
        for d in [1, 2, 255, 256, 1023, 1024, 1025] {
            if lo.checked_add(d).is_some_and(|k| k <= hi) {
                probes.push(lo + d);
            }
            if hi.checked_sub(d).is_some_and(|k| k >= lo) {
                probes.push(hi - d);
            }
        }
        let mid = lo + (hi - lo) / 2;
        probes.push(mid);
    }
    probes
}

fn keys_of(runs: &[(u64, u64)]) -> BTreeSet<u64> {
    runs.iter().flat_map(|&(lo, hi)| lo..=hi).collect()
}

#[test]
fn null_array_returns_the_start() {
    let keys = BTreeSet::new();
    check(&keys, &[0, 1, 42, Word::MAX as u64 - 1, Word::MAX as u64]);
}

#[test]
fn absent_index_is_its_own_answer() {
    let keys: BTreeSet<u64> = [10, 12, 1 << 40, u64::MAX - 7].into_iter().collect();
    check(
        &keys,
        &[0, 11, 13, (1 << 40) - 1, (1 << 40) + 1, u64::MAX - 6],
    );
}

#[test]
fn clustered_runs_of_256() {
    // The bench arm's population shape: runs of 256, gaps between them.
    let runs: Vec<(u64, u64)> = (0..40u64)
        .map(|r| {
            let lo = 0x1_0000 + r * 1000;
            (lo, lo + 255)
        })
        .collect();
    check(&keys_of(&runs), &run_probes(&runs));
}

#[test]
fn runs_at_the_budget_boundary() {
    // Runs of 1023, 1024 and 1025 keys: the walk finds the gap, or reaches
    // its budget on the last present key and bisects the remainder.
    let runs = [
        (1_000_000, 1_000_000 + 1022),
        (2_000_000, 2_000_000 + 1023),
        (3_000_000, 3_000_000 + 1024),
    ];
    check(&keys_of(&runs), &run_probes(&runs));
}

#[test]
fn long_runs_fall_back_to_bisection() {
    // Runs longer than the walk budget, crossing leaf and bitmap boundaries.
    let runs = [
        (0xFF00, 0xFF00 + LONG_RUN),
        (1 << 33, (1 << 33) + 2 * LONG_RUN),
    ];
    check(&keys_of(&runs), &run_probes(&runs));
}

#[test]
fn runs_reaching_both_edges_of_the_key_space() {
    // A run from 0 and a run to Word::MAX: searches toward the edge find
    // nothing, both through the walk (short run) and through the bisection
    // (long run).
    for len in [5, LONG_RUN] {
        let runs = [(0, len), (u64::MAX - len, u64::MAX)];
        check(&keys_of(&runs), &run_probes(&runs));
    }
}
