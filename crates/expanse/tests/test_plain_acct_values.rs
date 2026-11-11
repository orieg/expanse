//! A plain tree's allocator accounting reads the same values whichever way
//! its counters are updated (Refs #1679).
//!
//! On a plain tree `bytes_in_use`, `live_allocs` and `total_allocs` are
//! updated on the allocation and free paths; a deferred tree's sharded
//! accounting is a different path and is not exercised here. Each test runs a
//! deterministic mixed workload (random inserts, a strided removal, sequential
//! re-inserts, overwrites, removal of everything) and compares `mem_used()`,
//! `live_allocs()` and `total_node_allocs()` at every checkpoint against values
//! pinned from the locked read-modify-write build (`main` at the time the
//! default changed, where the locked form was the default). They must hold under the
//! default build and under the inverse ablation feature alike, so an update
//! that drops, doubles or misdirects a count on either path fails here.
//!
//! Allocation sizes depend on the leaf ladder and on where string suffixes
//! live, so the pinned values hold only for the default layout: the layout
//! ablations that change them compile these tests out. 64-bit targets only,
//! where the pinned sizes were taken. Not under Miri (the workloads are sized
//! for the native test run; the allocator's own unit tests cover Miri).
#![cfg(all(
    not(miri),
    target_pointer_width = "64",
    not(feature = "ablation-leaf-class-10"),
    not(feature = "ablation-unpacked-suffix")
))]

use expanse_trie::strmap::{ExpanseStrMap, NulFreeStr};
use expanse_trie::{ExpanseMap, ExpanseMap32, ExpanseSet};

const N: u64 = 20_000;
const SEQ: u64 = 5_000;

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn random_keys(seed: u64) -> Vec<u64> {
    let mut s = seed;
    (0..N).map(|_| splitmix(&mut s)).collect()
}

/// (mem_used, live_allocs, total_node_allocs) at one checkpoint.
type Acct = (usize, usize, usize);

fn check(name: &str, got: &[Acct], want: &[Acct]) {
    assert_eq!(
        got, want,
        "{name}: accounting differs from the values pinned on the locked build; \
         got {got:?}"
    );
}

#[test]
fn map_accounting_matches_pinned_values() {
    let keys = random_keys(1);
    let mut m = ExpanseMap::new();
    let mut got: Vec<Acct> = Vec::new();
    let mut cp = |m: &ExpanseMap| got.push((m.mem_used(), m.live_allocs(), m.total_node_allocs()));
    for (i, &k) in keys.iter().enumerate() {
        m.insert(k, i as u64);
    }
    cp(&m);
    for &k in keys.iter().step_by(3) {
        assert!(m.remove(k).is_some());
    }
    cp(&m);
    for k in 0..SEQ {
        m.insert(k << 8, k);
    }
    cp(&m);
    for (i, &k) in keys.iter().enumerate().skip(1).step_by(3) {
        m.insert(k, !(i as u64));
    }
    cp(&m);
    for (i, &k) in keys.iter().enumerate().rev() {
        if i % 3 != 0 {
            assert!(m.remove(k).is_some());
        }
    }
    for k in (0..SEQ).rev() {
        assert!(m.remove(k << 8).is_some());
    }
    cp(&m);
    check("ExpanseMap", &got, MAP);
    let last = got[got.len() - 1];
    assert_eq!((last.0, last.1), (0, 0), "an emptied map holds nothing");
}

#[test]
fn set_accounting_matches_pinned_values() {
    let keys = random_keys(2);
    let mut s = ExpanseSet::new();
    let mut got: Vec<Acct> = Vec::new();
    let mut cp = |s: &ExpanseSet| got.push((s.mem_used(), s.live_allocs(), s.total_node_allocs()));
    for &k in &keys {
        s.insert(k);
    }
    cp(&s);
    for &k in keys.iter().step_by(3) {
        assert!(s.remove(k));
    }
    cp(&s);
    for k in 0..SEQ {
        s.insert(k << 8);
    }
    cp(&s);
    for (i, &k) in keys.iter().enumerate().rev() {
        if i % 3 != 0 {
            assert!(s.remove(k));
        }
    }
    for k in (0..SEQ).rev() {
        assert!(s.remove(k << 8));
    }
    cp(&s);
    check("ExpanseSet", &got, SET);
    let last = got[got.len() - 1];
    assert_eq!((last.0, last.1), (0, 0), "an emptied set holds nothing");
}

#[test]
fn map32_accounting_matches_pinned_values() {
    let keys: Vec<u32> = random_keys(3).iter().map(|&k| (k >> 32) as u32).collect();
    let mut m = ExpanseMap32::new();
    let mut got: Vec<Acct> = Vec::new();
    let mut cp =
        |m: &ExpanseMap32| got.push((m.mem_used(), m.live_allocs(), m.total_node_allocs()));
    for (i, &k) in keys.iter().enumerate() {
        m.insert(k, i as u32);
    }
    cp(&m);
    for &k in keys.iter().step_by(3) {
        m.remove(k);
    }
    cp(&m);
    for k in 0..SEQ as u32 {
        m.insert(k << 4, k);
    }
    cp(&m);
    m.clear();
    cp(&m);
    check("ExpanseMap32", &got, MAP32);
    let last = got[got.len() - 1];
    assert_eq!((last.0, last.1), (0, 0), "a cleared map holds nothing");
}

#[test]
fn strmap_accounting_matches_pinned_values() {
    // `ExpanseStrMap` exposes only `mem_used()` (its allocator's
    // `bytes_in_use`), not the allocation counts.
    let keys: Vec<String> = random_keys(4)
        .iter()
        .map(|k| format!("k{k:x}/{}", k % 97))
        .collect();
    fn key(s: &str) -> &NulFreeStr {
        NulFreeStr::new(s.as_bytes()).expect("NUL-free")
    }
    let mut m = ExpanseStrMap::new();
    let mut got: Vec<usize> = Vec::new();
    for (i, k) in keys.iter().enumerate() {
        m.insert(key(k), i as u64);
    }
    got.push(m.mem_used());
    for k in keys.iter().step_by(3) {
        assert!(m.remove(key(k)).is_some());
    }
    got.push(m.mem_used());
    for (i, k) in keys.iter().enumerate().rev() {
        if i % 3 != 0 {
            assert!(m.remove(key(k)).is_some());
        }
    }
    got.push(m.mem_used());
    assert_eq!(
        got, STRMAP,
        "ExpanseStrMap: mem_used differs from the pinned values; got {got:?}"
    );
}

const MAP: &[Acct] = &[
    (450_208, 4_808, 13_619),
    (332_288, 4_544, 16_523),
    (414_208, 4_571, 17_657),
    (414_208, 4_571, 17_657),
    (0, 0, 25_601),
];
const SET: &[Acct] = &[
    (369_552, 2_547, 10_838),
    (281_600, 2_483, 12_381),
    (363_520, 2_511, 13_463),
    (0, 0, 20_268),
];
// `clear()` replaces the allocator, so its total starts again at 0.
const MAP32: &[Acct] = &[
    (221_648, 2_752, 4_786),
    (160_864, 1_442, 4_954),
    (188_560, 1_757, 6_534),
    (0, 0, 0),
];
const STRMAP: &[usize] = &[1_053_232, 744_784, 0];
