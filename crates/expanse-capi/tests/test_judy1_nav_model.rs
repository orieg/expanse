//! `Judy1` `First`/`Next`/`Last`/`Prev` and `FirstEmpty`/`NextEmpty`/
//! `LastEmpty`/`PrevEmpty` against a `BTreeSet` model, over populations
//! built to reach every node form the empty-index descent reads: single-key
//! and multi-key immediates, linear leaves of each key width (narrow and at
//! their slot level), bitmap leaves, full expanses, and linear, bitmap and
//! uncompressed branches, with runs that end on and cross leaf and branch
//! boundaries and runs that reach 0 and `Word::MAX`.
//!
//! Probes are every key and its two neighbours (strided on the large
//! populations), the edges of the key space, and expanse boundaries.
#![cfg(target_pointer_width = "64")]

use core::ffi::{c_int, c_void};
use core::ptr::{NonNull, null_mut};
use expanse::{
    JError, Judy1First, Judy1FirstEmpty, Judy1FreeArray, Judy1Last, Judy1LastEmpty, Judy1Next,
    Judy1NextEmpty, Judy1Prev, Judy1PrevEmpty, Judy1Set, Judy1Unset, Word,
};
use std::collections::BTreeSet;

type Search = unsafe extern "C" fn(*const c_void, *mut Word, *mut JError) -> c_int;

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

/// The population as maximal runs of consecutive keys, `(first, last)`,
/// ascending: the model of the empty searches.
fn runs_of(keys: &BTreeSet<u64>) -> Vec<(u64, u64)> {
    let mut runs: Vec<(u64, u64)> = Vec::new();
    for &k in keys {
        match runs.last_mut() {
            Some((_, hi)) if hi.checked_add(1) == Some(k) => *hi = k,
            _ => runs.push((k, k)),
        }
    }
    runs
}

/// Smallest absent key `>= p` (`asc`) or largest absent key `<= p`.
fn model_empty(runs: &[(u64, u64)], p: u64, asc: bool) -> Option<u64> {
    // The run holding `p`, if any.
    let i = runs.partition_point(|&(lo, _)| lo <= p);
    match i.checked_sub(1).map(|i| runs[i]) {
        Some((lo, hi)) if hi >= p => {
            if asc {
                hi.checked_add(1)
            } else {
                lo.checked_sub(1)
            }
        }
        _ => Some(p),
    }
}

/// The expected `(return, *PIndex)` of each of the eight searches from `p`.
fn model(keys: &BTreeSet<u64>, runs: &[(u64, u64)], p: u64) -> [(c_int, u64); 8] {
    // A neighbour miss on a non-empty array steps the index one past the
    // probe, wrapping (COMPAT.md doc-gap D8); an empty-search miss leaves it.
    let hit = |r: Option<u64>, miss: u64| r.map_or((0, miss), |k| (1, k));
    let first = keys.range(p..).next().copied();
    let next = p
        .checked_add(1)
        .and_then(|q| keys.range(q..).next().copied());
    let last = keys.range(..=p).next_back().copied();
    let prev = p
        .checked_sub(1)
        .and_then(|q| keys.range(..=q).next_back().copied());
    let empty = |start: Option<u64>, asc| start.and_then(|s| model_empty(runs, s, asc));
    [
        hit(first, p.wrapping_add(1)),
        hit(next, p.wrapping_add(1)),
        hit(last, p.wrapping_sub(1)),
        hit(prev, p.wrapping_sub(1)),
        hit(empty(Some(p), true), p),
        hit(empty(p.checked_add(1), true), p),
        hit(empty(Some(p), false), p),
        hit(empty(p.checked_sub(1), false), p),
    ]
}

const NAMES: [&str; 8] = [
    "Judy1First",
    "Judy1Next",
    "Judy1Last",
    "Judy1Prev",
    "Judy1FirstEmpty",
    "Judy1NextEmpty",
    "Judy1LastEmpty",
    "Judy1PrevEmpty",
];
const SEARCHES: [Search; 8] = [
    Judy1First,
    Judy1Next,
    Judy1Last,
    Judy1Prev,
    Judy1FirstEmpty,
    Judy1NextEmpty,
    Judy1LastEmpty,
    Judy1PrevEmpty,
];

/// A Judy1 array owned by the test, freed on drop.
struct Array(*mut c_void);

impl Array {
    fn build(keys: &BTreeSet<u64>, rng: &mut XorShift) -> Self {
        // Insert in a shuffled order: node forms must not depend on the
        // model's sorted iteration.
        let mut order: Vec<u64> = keys.iter().copied().collect();
        for i in (1..order.len()).rev() {
            order.swap(i, (rng.next() % (i as u64 + 1)) as usize);
        }
        let mut arr = null_mut();
        for k in order {
            // SAFETY: `arr` is this array's word.
            assert_eq!(unsafe { Judy1Set(&raw mut arr, k as Word, null_mut()) }, 1);
        }
        Self(arr)
    }

    fn unset(&mut self, k: u64) {
        // SAFETY: `self.0` is this array's word.
        let removed = unsafe { Judy1Unset(&raw mut self.0, k as Word, null_mut()) };
        assert_eq!(removed, 1);
    }
}

impl Drop for Array {
    fn drop(&mut self) {
        // SAFETY: the array was built by `Array::build` and is freed once.
        unsafe { Judy1FreeArray(&raw mut self.0, null_mut()) };
    }
}

fn check_probe(arr: &Array, keys: &BTreeSet<u64>, runs: &[(u64, u64)], p: u64, shape: &str) {
    let want = model(keys, runs, p);
    for (f, search) in SEARCHES.iter().enumerate() {
        let mut index = p as Word;
        let pindex = NonNull::from(&mut index);
        // SAFETY: `arr.0` is a live array; `pindex` is a valid index word.
        let r = unsafe { search(arr.0, pindex.as_ptr(), null_mut()) };
        assert_eq!((r, index as u64), want[f], "{shape}: {}({p:#x})", NAMES[f]);
    }
}

/// Every key and its neighbours (every `stride`th key), the key space's
/// edges, and the boundaries of each key's 256- and 65,536-key expanses.
fn probes(keys: &BTreeSet<u64>, stride: usize) -> Vec<u64> {
    let mut out = vec![0, 1, u64::MAX - 1, u64::MAX, 1 << 63, (1 << 63) - 1];
    for &k in keys.iter().step_by(stride) {
        out.extend([k.wrapping_sub(1), k, k.wrapping_add(1)]);
        // The same key with one higher digit changed: inside the slot of a
        // narrow node, outside the sub-expanse its decode digits fix.
        for j in 1..8 {
            out.extend([k ^ (1 << (8 * j)), k ^ (0xFF << (8 * j))]);
        }
        for mask in [0xFF, 0xFFFF] {
            out.extend([
                k & !mask,
                (k & !mask).wrapping_sub(1),
                k | mask,
                (k | mask).wrapping_add(1),
            ]);
        }
    }
    out
}

fn check(shape: &str, keys: &BTreeSet<u64>, stride: usize, rng: &mut XorShift) {
    let arr = Array::build(keys, rng);
    let runs = runs_of(keys);
    for p in probes(keys, stride) {
        check_probe(&arr, keys, &runs, p, shape);
    }
}

fn run(lo: u64, len: u64) -> impl Iterator<Item = u64> {
    (0..len).map(move |i| lo.wrapping_add(i))
}

#[test]
fn sparse_random_keys() {
    // Single-key immediates under bitmap and uncompressed branches.
    let mut rng = XorShift(0x1749_0001);
    for n in [3, 40, 300, 30_000] {
        let keys: BTreeSet<u64> = (0..n).map(|_| rng.next()).collect();
        check(
            &format!("random n={n}"),
            &keys,
            1 + n as usize / 2000,
            &mut rng,
        );
    }
}

#[test]
fn small_groups_per_expanse() {
    // Pairs and triples sharing their high bytes: multi-key immediates and
    // short linear leaves of every key width, some of them narrow.
    let mut rng = XorShift(0x1749_0002);
    for width in 1..=7u32 {
        let mut keys = BTreeSet::new();
        for _ in 0..60 {
            let base = rng.next();
            let span = if width == 7 {
                u64::MAX
            } else {
                (1u64 << (8 * width)) - 1
            };
            for _ in 0..(1 + rng.next() % 4) {
                let low = rng.next() & span;
                keys.insert((base & !span) | low);
                keys.insert(((base & !span) | low).wrapping_add(1));
            }
        }
        check(&format!("groups width={width}"), &keys, 1, &mut rng);
    }
}

#[test]
fn narrow_nodes() {
    // A few keys per top-level expanse that share more digits than their
    // slot: narrow linear leaves, bitmap leaves and branches whose decode
    // digits a probe can differ from.
    let mut rng = XorShift(0x1749_0007);
    for shared in 2..=6u32 {
        let mut keys = BTreeSet::new();
        for top in 0..12u64 {
            let base = (top << 56) | (rng.next() >> 8);
            let span = 1u64 << (8 * (7 - shared));
            let base = base & !(span - 1);
            for _ in 0..(2 + rng.next() % 12) {
                keys.insert(base | (rng.next() % span));
            }
            if top % 3 == 0 {
                // A dense 256-key expanse: a narrow bitmap leaf or full expanse.
                let leaf = base | ((rng.next() % span) & !0xFF);
                keys.extend(
                    run(leaf, 256)
                        .filter(|_| top.is_multiple_of(2) || !rng.next().is_multiple_of(5)),
                );
            }
        }
        check(&format!("narrow shared={shared}"), &keys, 1, &mut rng);
    }
}

#[test]
fn dense_expanses_and_runs() {
    // Bitmap leaves (dense 256-key expanses with holes), full expanses,
    // runs that end on and cross 256- and 65,536-key boundaries, and runs
    // of adjacent full expanses under linear, bitmap and uncompressed
    // branches.
    let mut rng = XorShift(0x1749_0003);
    let mut keys = BTreeSet::new();
    for _ in 0..20 {
        let base = rng.next() & !0xFF;
        // A dense leaf with a few holes.
        keys.extend(run(base, 256).filter(|_| !rng.next().is_multiple_of(8)));
    }
    for _ in 0..20 {
        // An aligned full expanse.
        keys.extend(run(rng.next() & !0xFF, 256));
    }
    for len in [255, 256, 257, 511, 512, 513, 700, 4096, 70_000] {
        // Unaligned and aligned runs.
        keys.extend(run(rng.next(), len));
        keys.extend(run(rng.next() & !0xFFFF, len));
        keys.extend(run((rng.next() | 0xFFFF).wrapping_sub(len / 2), len));
    }
    check("dense", &keys, 31, &mut rng);
}

#[test]
fn runs_at_both_ends_of_the_key_space() {
    let mut rng = XorShift(0x1749_0004);
    for len in [1, 2, 15, 255, 256, 257, 65_536, 70_000] {
        let mut keys: BTreeSet<u64> = run(0, len).collect();
        keys.extend(run(u64::MAX - (len - 1), len));
        // A few keys in between keep the root a branch.
        keys.extend((0..20).map(|_| rng.next()));
        check(&format!("ends len={len}"), &keys, 3, &mut rng);
    }
    // The whole top 65,536-key expanse full, with one hole near its start.
    let mut keys: BTreeSet<u64> = run(u64::MAX - 0xFFFF, 0x1_0000).collect();
    keys.remove(&(u64::MAX - 0xFFF0));
    keys.extend((0..20).map(|_| rng.next()));
    check("top expanse", &keys, 5, &mut rng);
}

#[test]
fn runs_past_the_walk_budget() {
    // More than 1024 adjacent full leaves: the empty search spends its
    // walk budget and finishes by rank.
    let mut rng = XorShift(0x1749_0005);
    let mut keys: BTreeSet<u64> = run(0x4_0000_0000 - 3, 1100 * 256 + 9).collect();
    keys.extend((0..50).map(|_| rng.next()));
    check("long run", &keys, 97, &mut rng);
}

#[test]
fn after_removals() {
    // Holes punched by `Judy1Unset` into full and bitmap leaves, so the
    // node forms are the ones a shrinking array reaches.
    let mut rng = XorShift(0x1749_0006);
    let mut keys: BTreeSet<u64> = BTreeSet::new();
    for _ in 0..10 {
        keys.extend(run(rng.next() & !0xFF, 512));
    }
    keys.extend((0..2000).map(|_| rng.next()));
    let mut arr = Array::build(&keys, &mut rng);
    let all: Vec<u64> = keys.iter().copied().collect();
    for &k in all.iter().step_by(5) {
        arr.unset(k);
        keys.remove(&k);
    }
    let runs = runs_of(&keys);
    for p in probes(&keys, 3) {
        check_probe(&arr, &keys, &runs, p, "after removals");
    }
}

#[test]
fn narrow_leaves_left_by_removals() {
    // One dense 256-key expanse per top-level slot is a narrow bitmap leaf;
    // removing most of its keys demotes it to a narrow linear leaf that
    // keeps the decode digits.
    let mut rng = XorShift(0x1749_0008);
    for keep in [3, 9, 20, 40] {
        let mut keys = BTreeSet::new();
        for top in 0..16u64 {
            let leaf = (top << 56) | (rng.next() >> 8 & !0xFF);
            keys.extend(run(leaf, 200));
        }
        let mut arr = Array::build(&keys, &mut rng);
        let all: Vec<u64> = keys.iter().copied().collect();
        for (i, &k) in all.iter().enumerate() {
            if i % 200 >= keep {
                arr.unset(k);
                keys.remove(&k);
            }
        }
        let runs = runs_of(&keys);
        for p in probes(&keys, 1) {
            check_probe(&arr, &keys, &runs, p, &format!("narrow leaves keep={keep}"));
        }
    }
}
