//! Fast Callgrind C ABI instruction regression smoke gate ($N = 10,000$).
//!
//! Provides deterministic instruction and cache-miss metrics for libexpanse
//! C ABI entry points (JudyLIns, JudyLGet, Judy1Set, Judy1Test) in <20s.

#![allow(missing_docs)]

use core::ffi::c_void;
#[cfg(target_os = "linux")]
use iai_callgrind::main;
use iai_callgrind::{library_benchmark, library_benchmark_group};
use std::hint::black_box;
use std::ptr::null_mut;

type Word = usize;

#[inline]
fn map_len_sentinel(arr: *mut c_void) -> Word {
    arr as Word
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

const POP: usize = 10_000;

fn keys(dist: &str) -> Vec<Word> {
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut out = Vec::with_capacity(POP);
    match dist {
        "sequential" => out.extend((0..POP as u64).map(|k| k as Word)),
        "random" => out.extend((0..POP).map(|_| rng.next() as Word)),
        "clustered" => {
            let mut base = 0u64;
            for i in 0..POP as u64 {
                if i % 256 == 0 {
                    base = rng.next() & !0xFF;
                }
                out.push((base + (i % 256)) as Word);
            }
        }
        other => panic!("unknown distribution {other}"),
    }
    out
}

fn shuffled(mut ks: Vec<Word>) -> Vec<Word> {
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..ks.len()).rev() {
        ks.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    ks
}

struct Built {
    arr: *mut c_void,
    probes: Vec<Word>,
}

// SAFETY: The array is created and consumed on the same thread by the benchmark harness.
unsafe impl Send for Built {}

fn build_judyl(dist: &str) -> Built {
    let ks = keys(dist);
    let probes = shuffled(ks.clone());
    let mut arr: *mut c_void = null_mut();
    // SAFETY: Standard JudyLIns usage; returned slot is valid until next mutation.
    unsafe {
        for &k in &ks {
            let slot = expanse::JudyLIns(&raw mut arr, k, null_mut()).cast::<Word>();
            *slot = k;
        }
    }
    Built { arr, probes }
}

fn build_judy1(dist: &str) -> Built {
    let ks = keys(dist);
    let probes = shuffled(ks.clone());
    let mut arr: *mut c_void = null_mut();
    // SAFETY: Standard Judy1Set usage with valid pointer.
    unsafe {
        for &k in &ks {
            expanse::Judy1Set(&raw mut arr, k, null_mut());
        }
    }
    Built { arr, probes }
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = keys)]
#[bench::random(args = ("random",), setup = keys)]
#[bench::clustered(args = ("clustered",), setup = keys)]
fn judyl_insert(ks: Vec<Word>) -> Word {
    let mut arr: *mut c_void = null_mut();
    // SAFETY: Standard JudyLIns usage; slot is written immediately.
    unsafe {
        for &k in &ks {
            let slot = expanse::JudyLIns(&raw mut arr, black_box(k), null_mut()).cast::<Word>();
            *slot = k;
        }
        black_box(map_len_sentinel(arr))
    }
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = build_judyl)]
#[bench::random(args = ("random",), setup = build_judyl)]
#[bench::clustered(args = ("clustered",), setup = build_judyl)]
fn judyl_get(built: Built) -> Word {
    let mut sink = 0usize;
    // SAFETY: Array was built by build_judyl; JudyLGet performs lookup.
    unsafe {
        for &k in &built.probes {
            let slot = expanse::JudyLGet(built.arr, black_box(k), null_mut()).cast::<Word>();
            if !slot.is_null() {
                sink ^= *slot;
            }
        }
    }
    black_box(sink)
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = keys)]
#[bench::random(args = ("random",), setup = keys)]
#[bench::clustered(args = ("clustered",), setup = keys)]
fn judy1_set(ks: Vec<Word>) -> Word {
    let mut arr: *mut c_void = null_mut();
    // SAFETY: Standard Judy1Set usage.
    unsafe {
        for &k in &ks {
            expanse::Judy1Set(&raw mut arr, black_box(k), null_mut());
        }
        black_box(map_len_sentinel(arr))
    }
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = build_judy1)]
#[bench::random(args = ("random",), setup = build_judy1)]
#[bench::clustered(args = ("clustered",), setup = build_judy1)]
fn judy1_test(built: Built) -> Word {
    let mut hits = 0usize;
    // SAFETY: Array was built by build_judy1; Judy1Test checks bit membership.
    unsafe {
        for &k in &built.probes {
            hits += expanse::Judy1Test(built.arr, black_box(k), null_mut()) as usize;
        }
    }
    black_box(hits)
}

library_benchmark_group!(
    name = smoke_capi_cost;
    benchmarks =
        judyl_insert,
        judyl_get,
        judy1_set,
        judy1_test
);

#[cfg(target_os = "linux")]
main!(library_benchmark_groups = smoke_capi_cost);

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("iai-callgrind smoke_instructions benchmarks run on Linux only.");
}
