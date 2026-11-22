//! `Judy1`/`JudyL` `FirstEmpty`/`NextEmpty`/`LastEmpty`/`PrevEmpty` against a
//! model, on populations built to reach each branch of the search: a gap
//! found inside a bitmap, full or linear leaf or a root leaf, a run that
//! crosses leaves, a run longer than the walk budget (which falls back to
//! rank bisection), runs that end at either edge of the key space (0 and
//! `Word::MAX`), an index that is itself absent, and the null array.
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

/// The shim's walk budget is 1024 steps, and a full 256-key leaf is one
/// step, so a run of more than 1024 full leaves reaches the rank bisection.
const LONG_RUN: u64 = 1100 * 256;

/// Model: walk the present keys from the start until one is not the next
/// expected key.
fn model(keys: &BTreeSet<u64>, probe: u64, ascending: bool, strict: bool) -> Option<u64> {
    if ascending {
        let mut expect = if strict { probe.checked_add(1)? } else { probe };
        for &k in keys.range(expect..) {
            if k != expect {
                break;
            }
            expect = expect.checked_add(1)?;
        }
        Some(expect)
    } else {
        let mut expect = if strict { probe.checked_sub(1)? } else { probe };
        for &k in keys.range(..=expect).rev() {
            if k != expect {
                break;
            }
            expect = expect.checked_sub(1)?;
        }
        Some(expect)
    }
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

struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// Short runs separated by short gaps from `base`, `span` keys wide: sparse
/// enough that the leaves stay linear (or a root leaf, for a few keys).
fn short_runs(rng: &mut XorShift, base: u64, count: usize, stride: u64) -> Vec<(u64, u64)> {
    let mut runs = Vec::new();
    let mut lo = base;
    for _ in 0..count {
        let len = rng.next() % 6;
        runs.push((lo, lo + len));
        lo += len + 2 + (rng.next() % stride);
    }
    runs
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
    // The bench arm's population shape: aligned runs of 256 (one full
    // bitmap leaf each), gaps between them.
    let runs: Vec<(u64, u64)> = (0..40u64)
        .map(|r| {
            let lo = 0x1_0000 + r * 0x1000;
            (lo, lo + 255)
        })
        .collect();
    check(&keys_of(&runs), &run_probes(&runs));
}

#[test]
fn runs_crossing_leaf_boundaries() {
    // Unaligned runs spanning several 256-key leaves, and adjacent full
    // leaves with one hole between two runs.
    let runs = [
        (1_000_000, 1_000_000 + 1022),
        (2_000_000, 2_000_000 + 1023),
        (3_000_000, 3_000_000 + 1024),
        (0x40_0000, 0x40_02FF),
        (0x40_0301, 0x40_05FF),
    ];
    check(&keys_of(&runs), &run_probes(&runs));
}

#[test]
fn short_runs_in_linear_and_root_leaves() {
    let mut rng = XorShift(0x5EED_1749);
    // A handful of keys: the root leaf.
    let tiny = short_runs(&mut rng, 100, 3, 3);
    // Under `LEAF_CAP` keys in one 256-key expanse, beside a full leaf that
    // keeps the root out of root-leaf form: a linear leaf of 1-byte keys.
    let mut one_byte = short_runs(&mut rng, 0x7700, 6, 4);
    one_byte.push((0x9_0000, 0x9_00FF));
    // Keys spread over a 64 Ki expanse: linear leaves of wider keys.
    let wide = short_runs(&mut rng, 0x5_0000, 40, 1500);
    for runs in [tiny, one_byte, wide] {
        let keys = keys_of(&runs);
        let probes: Vec<u64> = keys
            .iter()
            .flat_map(|&k| [k.wrapping_sub(1), k, k + 1])
            .collect();
        check(&keys, &probes);
    }
}

#[test]
fn long_runs_fall_back_to_bisection() {
    // Runs longer than the walk budget: the search resumes by rank.
    let runs = [
        (0xFF00, 0xFF00 + LONG_RUN),
        (1 << 33, (1 << 33) + LONG_RUN + 7),
    ];
    check(&keys_of(&runs), &run_probes(&runs));
}

#[test]
fn runs_reaching_both_edges_of_the_key_space() {
    // A run from 0 and a run to Word::MAX: searches toward the edge find
    // nothing, both through the walk (short run) and through the bisection
    // (long run).
    for len in [5, 300, LONG_RUN] {
        let runs = [(0, len), (u64::MAX - len, u64::MAX)];
        check(&keys_of(&runs), &run_probes(&runs));
    }
}
