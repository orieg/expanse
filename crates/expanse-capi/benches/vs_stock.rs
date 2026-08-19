//! **libexpanse vs stock libjudy, in instructions retired** — the
//! project's headline comparison, measured deterministically.
//!
//! `examples/bench_vs_libjudy.rs` answers the same question in
//! wall-clock, which is the number users ultimately feel but which
//! neither available environment can resolve below ~15-20%
//! (`docs/BENCHMARKING.md`). Under callgrind the same comparison becomes
//! exact and reproducible, so "ours does N% more work than stock" is
//! reviewable on every pull request instead of being an artifact nobody
//! opens.
//!
//! Both arms drive the **same C ABI**: `JudyLIns`/`JudyLGet`/`Judy1Set`/
//! `Judy1Test` with identical key streams. Stock is reached through
//! `dlopen`/`dlsym` because it exports the very symbols we do — loading
//! it privately is what keeps the two from colliding at link time (the
//! differential oracle uses the same trick).
//!
//! Reading the numbers:
//!
//! - Instructions are **cost, not time**. Stock libjudy is a mature C
//!   implementation; where we retire more instructions we are doing more
//!   work, but cache behaviour and branch prediction decide how much of
//!   that becomes wall-clock. The `Estimated Cycles` column and the
//!   L1/LL/RAM counts are the closer proxy.
//! - The stock arm calls through a function pointer while ours is a
//!   direct call, which biases *against* stock by a couple of
//!   instructions per operation — negligible against the thousands each
//!   operation costs, and it never flatters us.
//!
//! Linux/CI only (valgrind, plus `libjudy-dev` for the stock library).

#![allow(missing_docs)]

use core::ffi::{c_int, c_void};
use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use std::hint::black_box;
use std::ptr::null_mut;

type Word = usize;

unsafe extern "C" {
    fn dlopen(filename: *const u8, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const u8) -> *mut c_void;
}
const RTLD_NOW: c_int = 2;

type FpIns = unsafe extern "C" fn(*mut *mut c_void, Word, *mut c_void) -> *mut c_void;
type FpGet = unsafe extern "C" fn(*const c_void, Word, *mut c_void) -> *mut c_void;
type F1Set = unsafe extern "C" fn(*mut *mut c_void, Word, *mut c_void) -> c_int;
type F1Test = unsafe extern "C" fn(*const c_void, Word, *mut c_void) -> c_int;
type FFree = unsafe extern "C" fn(*mut *mut c_void, *mut c_void) -> Word;

/// Stock libjudy, loaded privately so its symbols never collide with the
/// identically named ones libexpanse exports.
struct Stock(*mut c_void);

impl Stock {
    fn open() -> Self {
        for name in [
            c"libJudy.so.1",
            c"libJudy.so",
            c"/opt/homebrew/opt/judy/lib/libJudy.dylib",
            c"libJudy.dylib",
        ] {
            // SAFETY: valid NUL-terminated library name.
            let h = unsafe { dlopen(name.to_bytes_with_nul().as_ptr(), RTLD_NOW) };
            if !h.is_null() {
                return Self(h);
            }
        }
        panic!("stock libjudy not found — install libjudy-dev");
    }

    fn sym<T: Copy>(&self, name: &core::ffi::CStr) -> T {
        // SAFETY: live handle and NUL-terminated name; the caller names
        // the fn-pointer type matching the symbol.
        let p = unsafe { dlsym(self.0, name.to_bytes_with_nul().as_ptr()) };
        assert!(!p.is_null(), "missing symbol {name:?}");
        // SAFETY: fn-pointer transmute of a resolved symbol.
        unsafe { core::mem::transmute_copy(&p) }
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

/// Population per arm. Callgrind runs ~50x slower than native, so this
/// trades absolute scale for a job that finishes; it is still deep
/// enough to build a multi-level trie on every distribution.
///
/// **Scale caveat, and why `POP_BIG` exists.** At 30k keys the whole
/// structure is a few hundred KB and lives in cache: RAM hits measured
/// 0.04% of memory accesses for *both* libraries. So at this size
/// "estimated cycles track instruction counts" is nearly tautological
/// and says nothing about memory layout — the question the chunked
/// allocator of the original is supposed to answer. The large-population
/// benchmarks below exceed last-level cache so that miss behaviour, not
/// just instruction count, is exercised.
const POP: usize = 30_000;

/// Population for the cache-pressure arm: large enough that the tree
/// exceeds last-level cache on a typical runner, so LL/RAM hit counts
/// become load-bearing rather than rounding.
const POP_BIG: usize = 1_500_000;

fn keys(dist: &str) -> Vec<Word> {
    let n = if dist.ends_with("_big") { POP_BIG } else { POP };
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut out = Vec::with_capacity(n);
    match dist.trim_end_matches("_big") {
        "sequential" => out.extend((0..n as u64).map(|k| k as Word)),
        "random" => out.extend((0..n).map(|_| rng.next() as Word)),
        "clustered" => {
            let mut base = 0u64;
            for i in 0..n as u64 {
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

/// Probe order deliberately differs from build order: a sequential probe
/// measures the prefetcher rather than the lookup.
fn shuffled(mut ks: Vec<Word>) -> Vec<Word> {
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..ks.len()).rev() {
        ks.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    ks
}

/// Built array plus probe order, produced by `setup` so that **only the
/// probe loop is measured**. Without this the lookup benchmarks counted
/// the 30k-key build and the teardown too, which made "get" ratios a
/// blend dominated by insert — two independent reviews caught the same
/// bug, and the published lookup numbers were wrong because of it.
struct Built {
    arr: *mut c_void,
    probes: Vec<Word>,
}

// SAFETY: the array is created and consumed on the same thread by the
// benchmark harness; the pointer is never shared.
unsafe impl Send for Built {}

fn build_expanse(dist: &str) -> Built {
    let ks = keys(dist);
    let probes = shuffled(ks.clone());
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard JudyL usage.
    unsafe {
        for &k in &ks {
            let slot = expanse::JudyLIns(&raw mut arr, k, null_mut()).cast::<Word>();
            *slot = k;
        }
    }
    Built { arr, probes }
}

fn build_stock(dist: &str) -> Built {
    let ks = keys(dist);
    let probes = shuffled(ks.clone());
    let lib = Stock::open();
    let ins: FpIns = lib.sym(c"JudyLIns");
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard JudyL usage.
    unsafe {
        for &k in &ks {
            let slot = ins(&raw mut arr, k, null_mut()).cast::<Word>();
            *slot = k;
        }
    }
    Built { arr, probes }
}

fn build_set_expanse(dist: &str) -> Built {
    let ks = keys(dist);
    let probes = shuffled(ks.clone());
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard Judy1 usage.
    unsafe {
        for &k in &ks {
            expanse::Judy1Set(&raw mut arr, k, null_mut());
        }
    }
    Built { arr, probes }
}

fn build_set_stock(dist: &str) -> Built {
    let ks = keys(dist);
    let probes = shuffled(ks.clone());
    let lib = Stock::open();
    let set: F1Set = lib.sym(c"Judy1Set");
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard Judy1 usage.
    unsafe {
        for &k in &ks {
            set(&raw mut arr, k, null_mut());
        }
    }
    Built { arr, probes }
}

// ---- JudyL insert -----------------------------------------------------

#[library_benchmark]
#[bench::sequential("sequential")]
#[bench::random("random")]
#[bench::clustered("clustered")]
fn judyl_insert_expanse(dist: &str) -> Word {
    let ks = keys(dist);
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard JudyL usage; the slot is valid until the next
    // mutation and is written immediately.
    unsafe {
        for &k in &ks {
            let slot = expanse::JudyLIns(&raw mut arr, black_box(k), null_mut()).cast::<Word>();
            *slot = k;
        }
        let freed = expanse::JudyLFreeArray(&raw mut arr, null_mut());
        black_box(freed)
    }
}

#[library_benchmark]
#[bench::sequential("sequential")]
#[bench::random("random")]
#[bench::clustered("clustered")]
fn judyl_insert_stock(dist: &str) -> Word {
    let ks = keys(dist);
    let lib = Stock::open();
    let ins: FpIns = lib.sym(c"JudyLIns");
    let free: FFree = lib.sym(c"JudyLFreeArray");
    let mut arr: *mut c_void = null_mut();
    // SAFETY: same contract as the libexpanse arm.
    unsafe {
        for &k in &ks {
            let slot = ins(&raw mut arr, black_box(k), null_mut()).cast::<Word>();
            *slot = k;
        }
        black_box(free(&raw mut arr, null_mut()))
    }
}

// ---- JudyL lookup ----------------------------------------------------
//
// `setup =` keeps the build phase OUT of the measured region. Previously
// these functions built the 30k-key array inside the benchmark body, so
// the reported "get" ratio was a blend of insert and lookup — insert
// dominated it. Only the probe loop is counted now.

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = build_expanse)]
#[bench::random(args = ("random",), setup = build_expanse)]
#[bench::clustered(args = ("clustered",), setup = build_expanse)]
// Cache-pressure arm: exceeds LLC, so LL/RAM hits matter (see POP_BIG).
#[bench::random_big(args = ("random_big",), setup = build_expanse)]
fn judyl_get_expanse(mut built: Built) -> Word {
    let mut sink = 0usize;
    // SAFETY: array built by `build_expanse`, freed here.
    unsafe {
        for &k in &built.probes {
            let slot = expanse::JudyLGet(built.arr, black_box(k), null_mut()).cast::<Word>();
            if !slot.is_null() {
                sink ^= *slot;
            }
        }
        expanse::JudyLFreeArray(&raw mut built.arr, null_mut());
    }
    black_box(sink)
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = build_stock)]
#[bench::random(args = ("random",), setup = build_stock)]
#[bench::clustered(args = ("clustered",), setup = build_stock)]
#[bench::random_big(args = ("random_big",), setup = build_stock)]
fn judyl_get_stock(mut built: Built) -> Word {
    let lib = Stock::open();
    let get: FpGet = lib.sym(c"JudyLGet");
    let free: FFree = lib.sym(c"JudyLFreeArray");
    let mut sink = 0usize;
    // SAFETY: array built by `build_stock`, freed here.
    unsafe {
        for &k in &built.probes {
            let slot = get(built.arr, black_box(k), null_mut()).cast::<Word>();
            if !slot.is_null() {
                sink ^= *slot;
            }
        }
        free(&raw mut built.arr, null_mut());
    }
    black_box(sink)
}

// ---- Judy1 ------------------------------------------------------------

#[library_benchmark]
#[bench::random("random")]
#[bench::clustered("clustered")]
fn judy1_set_expanse(dist: &str) -> Word {
    let ks = keys(dist);
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard Judy1 usage.
    unsafe {
        for &k in &ks {
            expanse::Judy1Set(&raw mut arr, black_box(k), null_mut());
        }
        black_box(expanse::Judy1FreeArray(&raw mut arr, null_mut()))
    }
}

#[library_benchmark]
#[bench::random("random")]
#[bench::clustered("clustered")]
fn judy1_set_stock(dist: &str) -> Word {
    let ks = keys(dist);
    let lib = Stock::open();
    let set: F1Set = lib.sym(c"Judy1Set");
    let free: FFree = lib.sym(c"Judy1FreeArray");
    let mut arr: *mut c_void = null_mut();
    // SAFETY: same contract as the libexpanse arm.
    unsafe {
        for &k in &ks {
            set(&raw mut arr, black_box(k), null_mut());
        }
        black_box(free(&raw mut arr, null_mut()))
    }
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = build_set_expanse)]
fn judy1_test_expanse(mut built: Built) -> Word {
    let mut hits = 0usize;
    // SAFETY: array built by `build_set_expanse`, freed here.
    unsafe {
        for &k in &built.probes {
            hits += expanse::Judy1Test(built.arr, black_box(k), null_mut()) as usize;
        }
        expanse::Judy1FreeArray(&raw mut built.arr, null_mut());
    }
    black_box(hits)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = build_set_stock)]
fn judy1_test_stock(mut built: Built) -> Word {
    let lib = Stock::open();
    let test: F1Test = lib.sym(c"Judy1Test");
    let free: FFree = lib.sym(c"Judy1FreeArray");
    let mut hits = 0usize;
    // SAFETY: array built by `build_set_stock`, freed here.
    unsafe {
        for &k in &built.probes {
            hits += test(built.arr, black_box(k), null_mut()) as usize;
        }
        free(&raw mut built.arr, null_mut());
    }
    black_box(hits)
}

library_benchmark_group!(
    name = vs_stock;
    benchmarks =
        judyl_insert_expanse,
        judyl_insert_stock,
        judyl_get_expanse,
        judyl_get_stock,
        judy1_set_expanse,
        judy1_set_stock,
        judy1_test_expanse,
        judy1_test_stock
);

main!(library_benchmark_groups = vs_stock);
