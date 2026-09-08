//! Fast Callgrind C ABI instruction regression smoke gate ($N = 10,000$).
//!
//! Provides deterministic instruction and cache-miss metrics for libexpanse
//! C ABI entry points (JudyLIns, JudyLGet, Judy1Set, Judy1Test, JudySLIns,
//! JudySLGet) in <20s.
//!
//! The `judysl_*` arms exist because `JudySLIns` and `JudySLGet` reach the
//! engine through `ins_slot` and `get_value_slot`, neither of which any arm
//! covered: the `strmap_*` arms in `crates/expanse/benches/instructions.rs`
//! call `insert`/`get` instead, so the C surface's cost was inferred from a
//! proxy rather than measured (§6's benchmark-arm prerequisite).
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `capi_smoke_instructions` |
//! | `group` | 1 |
//! | `population` | 10k |
//! | `insertion_order` | generator draw order — the population is inserted as drawn, neither sorted nor shuffled; the shuffle in this file is applied to the probe stream, not to the build |
//! | `probes_and_reuse` | 10k (shuffled), reuse 1.0 |
//! | `hit_rate` | 100% |
//! | `miss_gen_method` | None (hits only) |
//! | `value_dereference` | `sink ^= *slot` |
//! | `measured_region` | Clean (setup in setup) |
//! | `arm_symmetry` | Self (C ABI) |
//! | `statistics` | iai Callgrind exact counts |
//! | `verdict` | **PASS** `[verified: RUN (CI callgrind-smoke)]` |

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

/// NUL-terminated route-shaped keys, byte-identical to `str_keys("routes")` in
/// `crates/expanse/benches/instructions.rs` plus the terminator the C ABI
/// requires, so the `judysl_*` arms and the `strmap_*` arms describe the same
/// workload on the two surfaces.
fn str_keys(_dist: &str) -> Vec<Vec<u8>> {
    (0..POP)
        .map(|i| {
            let mut k =
                format!("/api/v2/tenants/{:06}/resources/{:04}", i / 16, i % 16).into_bytes();
            k.push(0);
            k
        })
        .collect()
}

struct BuiltStr {
    arr: *mut c_void,
    probes: Vec<Vec<u8>>,
}

// SAFETY: built and consumed on the same thread by the benchmark harness.
unsafe impl Send for BuiltStr {}

fn build_judysl(dist: &str) -> BuiltStr {
    let ks = str_keys(dist);
    let mut probes = ks.clone();
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard JudySLIns usage; each key is NUL-terminated and the
    // returned slot is valid until the next mutation.
    unsafe {
        for (i, k) in ks.iter().enumerate() {
            let slot = expanse::JudySLIns(&raw mut arr, k.as_ptr(), null_mut()).cast::<Word>();
            *slot = i;
        }
    }
    BuiltStr { arr, probes }
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = str_keys)]
fn judysl_insert(ks: Vec<Vec<u8>>) -> Word {
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard JudySLIns usage; the slot is written immediately.
    unsafe {
        for (i, k) in ks.iter().enumerate() {
            let slot =
                expanse::JudySLIns(&raw mut arr, black_box(k.as_ptr()), null_mut()).cast::<Word>();
            *slot = i;
        }
        black_box(map_len_sentinel(arr))
    }
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = build_judysl)]
fn judysl_get(built: BuiltStr) -> Word {
    let mut sink = 0usize;
    // SAFETY: the array was built by `build_judysl`; JudySLGet only reads.
    unsafe {
        for k in &built.probes {
            let slot =
                expanse::JudySLGet(built.arr, black_box(k.as_ptr()), null_mut()).cast::<Word>();
            if !slot.is_null() {
                sink ^= *slot;
            }
        }
    }
    black_box(sink)
}

library_benchmark_group!(
    name = smoke_capi_cost;
    benchmarks =
        judyl_insert,
        judyl_get,
        judy1_set,
        judy1_test,
        judysl_insert,
        judysl_get
);

#[cfg(target_os = "linux")]
main!(library_benchmark_groups = smoke_capi_cost);

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("iai-callgrind smoke_instructions benchmarks run on Linux only.");
}
