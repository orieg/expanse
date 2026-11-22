//! `JudyLFirst`/`Next`/`Last`/`Prev`/`ByCount` against a model, checking the
//! value pointer each returns: it must be the slot `JudyLGet` returns for the
//! key it found (pointer identity, not just an equal value), and a write
//! through it must be what a later `JudyLGet` reads.
//!
//! These shims return the key and its slot from one search. The populations
//! are built to reach every place a slot can live: the root leaf, single-key
//! immediates (the slot is the edge itself), multi-key immediates, linear and
//! bitmap leaves, and leaves behind every branch form, including narrow
//! pointers. Probes cover present keys, their neighbours, both ends of the
//! key space and the empty and null arrays; `ByCount` covers 0, 1, the
//! population and one past it.
#![cfg(target_pointer_width = "64")]

use core::ffi::c_void;
use core::ptr::null_mut;
use expanse::{
    JError, JudyLByCount, JudyLDel, JudyLFirst, JudyLFreeArray, JudyLGet, JudyLIns, JudyLLast,
    JudyLNext, JudyLPrev, Word,
};
use std::collections::BTreeMap;

type NavL = unsafe extern "C" fn(*const c_void, *mut Word, *mut JError) -> *mut c_void;

/// `(name, fn, forward, strict)`.
const NAVL: [(&str, NavL, bool, bool); 4] = [
    ("JudyLFirst", JudyLFirst, true, false),
    ("JudyLNext", JudyLNext, true, true),
    ("JudyLLast", JudyLLast, false, false),
    ("JudyLPrev", JudyLPrev, false, true),
];

fn model_nav(m: &BTreeMap<u64, u64>, probe: u64, forward: bool, strict: bool) -> Option<u64> {
    match (forward, strict) {
        (true, false) => m.range(probe..).next().map(|(&k, _)| k),
        (true, true) => m.range(probe.checked_add(1)?..).next().map(|(&k, _)| k),
        (false, false) => m.range(..=probe).next_back().map(|(&k, _)| k),
        (false, true) => m.range(..probe).next_back().map(|(&k, _)| k),
    }
}

/// The value word behind a slot pointer a shim returned; a null pointer
/// fails the test.
fn read_slot(p: *mut c_void, what: &str) -> u64 {
    // SAFETY: `p` is null or a value slot a JudyL shim returned, read before
    // any later mutation of its array; `as_ref` checks null.
    let r = unsafe { p.cast::<Word>().as_ref() };
    *r.unwrap_or_else(|| panic!("{what}: null slot")) as u64
}

/// Writes `v` through a slot pointer a shim returned; a null pointer fails
/// the test.
fn write_slot(p: *mut c_void, v: u64, what: &str) {
    // SAFETY: as `read_slot`; the array is not mutated while `r` lives.
    let r = unsafe { p.cast::<Word>().as_mut() };
    *r.unwrap_or_else(|| panic!("{what}: null slot")) = v as Word;
}

struct Array(*mut c_void);

impl Array {
    fn build(m: &BTreeMap<u64, u64>) -> Self {
        let mut a = null_mut();
        for (&k, &v) in m {
            // SAFETY: `a` is null or a live JudyL array owned here.
            unsafe { *JudyLIns(&raw mut a, k as Word, null_mut()).cast::<Word>() = v as Word };
        }
        Self(a)
    }
    fn get(&self, k: u64) -> *mut c_void {
        // SAFETY: live (or null) JudyL array.
        unsafe { JudyLGet(self.0, k as Word, null_mut()) }
    }
}

impl Drop for Array {
    fn drop(&mut self) {
        // SAFETY: owned array.
        unsafe { JudyLFreeArray(&raw mut self.0, null_mut()) };
    }
}

/// Every nav shim at `probe`: found key, its slot is `JudyLGet`'s slot and
/// holds the model's value; a miss leaves the index one step past the
/// probe (doc-gap D8, `test_judy_miss_index.rs`).
fn check_nav(a: &Array, m: &BTreeMap<u64, u64>, probe: u64, tag: &str) {
    for (name, f, forward, strict) in NAVL {
        let mut i = probe as Word;
        // SAFETY: live (or null) JudyL array; `i` is writable.
        let p = unsafe { f(a.0, &raw mut i, null_mut()) };
        match model_nav(m, probe, forward, strict) {
            Some(k) => {
                assert!(!p.is_null(), "{tag}: {name}({probe:#x}) missed {k:#x}");
                assert_eq!(i as u64, k, "{tag}: {name}({probe:#x}) index");
                assert_eq!(
                    p,
                    a.get(k),
                    "{tag}: {name}({probe:#x}) slot is not JudyLGet's"
                );
                assert_eq!(read_slot(p, name), m[&k], "{tag}: {name} value");
            }
            None => {
                assert!(p.is_null(), "{tag}: {name}({probe:#x}) found {i:#x}");
                if !a.0.is_null() {
                    let step = if forward {
                        probe.wrapping_add(1)
                    } else {
                        probe.wrapping_sub(1)
                    };
                    assert_eq!(i as u64, step, "{tag}: {name}({probe:#x}) miss index");
                }
            }
        }
    }
}

fn check_by_count(a: &Array, m: &BTreeMap<u64, u64>, keys: &[u64], nth: u64, tag: &str) {
    let mut i: Word = 0xDEAD;
    // SAFETY: live (or null) JudyL array; `i` is writable.
    let p = unsafe { JudyLByCount(a.0, nth as Word, &raw mut i, null_mut()) };
    match nth.checked_sub(1).and_then(|n| keys.get(n as usize)) {
        Some(&k) => {
            assert!(!p.is_null(), "{tag}: ByCount({nth}) missed");
            assert_eq!(i as u64, k, "{tag}: ByCount({nth}) index");
            assert_eq!(p, a.get(k), "{tag}: ByCount({nth}) slot is not JudyLGet's");
            assert_eq!(read_slot(p, "ByCount"), m[&k], "{tag}: ByCount value");
        }
        None => assert!(p.is_null(), "{tag}: ByCount({nth}) found {i:#x}"),
    }
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

/// Populations reaching every slot location (see the module doc).
fn populations() -> Vec<(&'static str, Vec<u64>)> {
    let mut r = XorShift(0x9E37_79B9_7F4A_7C15);
    let random: Vec<u64> = (0..30_000).map(|_| r.next()).collect();
    let small: Vec<u64> = (0..5).map(|_| r.next()).collect();
    let mut ends = vec![0, 1, 2, u64::MAX - 2, u64::MAX - 1, u64::MAX];
    ends.extend((0..300).map(|i| i * 7));
    // Dense runs make bitmap and full leaves; a sparse tail beside them
    // makes single-key children (immediates) under the same branches.
    let mut clustered: Vec<u64> = (0..20_000u64).map(|i| 0x4000_0000 + i).collect();
    clustered.extend((0..2_000u64).map(|i| 0x4000_0000 + 50_000 + i * 3));
    clustered.extend((0..500u64).map(|i| i << 40));
    // Keys sharing their high bytes below one top digit: narrow pointers.
    let narrow: Vec<u64> = (0..3_000u64)
        .map(|i| 0x1234_5600_0000_0000 | (i * 0x1_0001))
        .collect();
    // Pairs and triples per expanse: multi-key immediates.
    let multi: Vec<u64> = (0..4_000u64)
        .flat_map(|i| [i << 24, (i << 24) | 1, (i << 24) | 0x100])
        .collect();
    vec![
        ("single-0", vec![0]),
        ("single-max", vec![u64::MAX]),
        ("small", small),
        ("ends", ends),
        ("random", random),
        ("clustered", clustered),
        ("narrow", narrow),
        ("multi", multi),
    ]
}

#[test]
fn nav_and_by_count_return_get_slot() {
    for (tag, keys) in populations() {
        let m: BTreeMap<u64, u64> = keys
            .iter()
            .map(|&k| (k, k ^ 0x5A5A_5A5A_5A5A_5A5A))
            .collect();
        let a = Array::build(&m);
        let mut probes = vec![0, 1, u64::MAX, u64::MAX - 1, 0x8000_0000_0000_0000];
        // Every key and its neighbours for the smaller populations; a
        // stride of them for the larger ones.
        let stride = (m.len() / 3_000).max(1);
        for &k in m.keys().step_by(stride) {
            probes.extend([k, k.wrapping_sub(1), k.wrapping_add(1)]);
        }
        for p in probes {
            check_nav(&a, &m, p, tag);
        }
        let keys: Vec<u64> = m.keys().copied().collect();
        let pop = m.len() as u64;
        for nth in [0, 1, 2, pop / 2, pop.saturating_sub(1), pop, pop + 1] {
            check_by_count(&a, &m, &keys, nth, tag);
        }
        for nth in (1..=pop).step_by(stride) {
            check_by_count(&a, &m, &keys, nth, tag);
        }
    }
}

#[test]
fn empty_and_null_arrays_find_nothing() {
    let m = BTreeMap::new();
    let null = Array(null_mut());
    for p in [0, 1, 42, u64::MAX] {
        check_nav(&null, &m, p, "null");
    }
    for nth in [0, 1, 2] {
        check_by_count(&null, &m, &[], nth, "null");
    }
    // A non-null array emptied by deletes returns to null, as JudyLDel
    // documents; it searches like one.
    let mut a = Array::build(&[(7u64, 70u64)].into_iter().collect());
    // SAFETY: live array owned by `a`.
    assert_eq!(unsafe { JudyLDel(&raw mut a.0, 7, null_mut()) }, 1);
    assert!(a.0.is_null());
    check_nav(&a, &m, 7, "emptied");
    check_by_count(&a, &m, &[], 1, "emptied");
}

/// A write through each shim's returned pointer is what a later `JudyLGet`
/// (and the next search) reads.
#[test]
fn writes_through_nav_slots_are_visible() {
    for (tag, keys) in populations() {
        let mut m: BTreeMap<u64, u64> = keys.iter().map(|&k| (k, k)).collect();
        let a = Array::build(&m);
        let ks: Vec<u64> = m.keys().copied().collect();
        let stride = (ks.len() / 500).max(1);
        for (n, &k) in ks.iter().enumerate().step_by(stride) {
            let fresh = !k;
            let mut i = k as Word;
            // Next from just below `k` and Prev from just above it find `k`,
            // which is present.
            let (name, f): (&str, NavL) = match n % 5 {
                0 => ("First", JudyLFirst),
                1 => ("Last", JudyLLast),
                2 if k > 0 => {
                    i = (k - 1) as Word;
                    ("Next", JudyLNext)
                }
                3 if k < u64::MAX => {
                    i = (k + 1) as Word;
                    ("Prev", JudyLPrev)
                }
                _ => ("ByCount", JudyLFirst),
            };
            let p = if name == "ByCount" {
                // SAFETY: live array; `i` is writable.
                unsafe { JudyLByCount(a.0, (n + 1) as Word, &raw mut i, null_mut()) }
            } else {
                // SAFETY: live array; `i` is writable.
                unsafe { f(a.0, &raw mut i, null_mut()) }
            };
            assert_eq!(i as u64, k, "{tag}: {name} index");
            assert!(!p.is_null(), "{tag}: {name} slot");
            // No mutation since the slot was returned.
            write_slot(p, fresh, name);
            m.insert(k, fresh);
            let g = a.get(k);
            assert!(
                !g.is_null(),
                "{tag}: {name} write through its slot lost key {k:#x}"
            );
            assert_eq!(read_slot(g, name), fresh, "{tag}: {name} write lost");
        }
        // Every value, written or not, still matches the model.
        for (&k, &v) in &m {
            let g = a.get(k);
            assert!(!g.is_null(), "{tag}: key {k:#x} lost");
            assert_eq!(read_slot(g, "JudyLGet"), v, "{tag}: {k:#x}");
        }
    }
}
