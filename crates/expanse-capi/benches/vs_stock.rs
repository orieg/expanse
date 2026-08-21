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
//! **Three arms, and the middle one is the honest comparison.** Every
//! benchmark below exists in up to three shapes:
//!
//! - `*_expanse` — our rlib, linked straight into the harness with LTO
//!   and called directly. This is the fastest shape our code has and the
//!   one no `libexpanse` user gets.
//! - `*_expanse_dl` — our own `libexpanse.so`, `dlopen`'d and called
//!   through resolved symbols, i.e. **exactly how stock is reached and
//!   exactly how a drop-in consumer reaches us**. Compare *this* against
//!   stock.
//! - `*_stock` — stock libjudy, `dlopen`'d.
//!
//! The rlib arm was the only shape measured until issue #1, and the
//! resulting ratios were optimistic twice over: our arm got cross-object
//! inlining and direct calls that stock's PIC shared object cannot have,
//! and stock's `dlopen` sat inside the measured region while ours paid
//! nothing. Both biases pointed the same way. Symbol resolution now
//! happens in `setup` for every arm, so no benchmark measures its own
//! dynamic linking; the `expanse_dl` − `expanse` difference is the
//! standing correction factor for every ratio this project has published.
//!
//! Instructions are **cost, not time**. Stock libjudy is a mature C
//! implementation; where we retire more instructions we are doing more
//! work, but cache behaviour and branch prediction decide how much of
//! that becomes wall-clock. The `Estimated Cycles` column and the
//! L1/LL/RAM counts are the closer proxy.
//!
//! Linux/CI only (valgrind, plus `libjudy-dev` for the stock library).

#![allow(missing_docs)]

use core::ffi::{c_int, c_void};
#[cfg(target_os = "linux")]
use iai_callgrind::main;
use iai_callgrind::{library_benchmark, library_benchmark_group};
use std::hint::black_box;
use std::ptr::null_mut;

type Word = usize;

unsafe extern "C" {
    fn dlopen(filename: *const u8, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const u8) -> *mut c_void;
}
const RTLD_NOW: c_int = 2;
/// Keep the loaded library's symbols out of the global namespace: both
/// libraries export the same names, and so does the harness itself.
const RTLD_LOCAL: c_int = 0;

type FpIns = unsafe extern "C" fn(*mut *mut c_void, Word, *mut c_void) -> *mut c_void;
type FpGet = unsafe extern "C" fn(*const c_void, Word, *mut c_void) -> *mut c_void;
type F1Set = unsafe extern "C" fn(*mut *mut c_void, Word, *mut c_void) -> c_int;
type F1Test = unsafe extern "C" fn(*const c_void, Word, *mut c_void) -> c_int;
type FpDel = unsafe extern "C" fn(*mut *mut c_void, Word, *mut c_void) -> c_int;

/// Keeps a built array observable so the optimizer cannot delete the
/// work that produced it, without calling into either library.
#[inline]
fn map_len_sentinel(arr: *mut c_void) -> Word {
    arr as Word
}

/// A privately loaded Judy-ABI shared object — stock libjudy or our own
/// `libexpanse.so`. Both export the same symbol names, which is the whole
/// point of the compat layer and the reason `RTLD_LOCAL` is not optional.
struct Lib(*mut c_void);

/// Where stock libjudy might live, in preference order.
const STOCK_NAMES: &[&core::ffi::CStr] = &[
    c"libJudy.so.1",
    c"libJudy.so",
    c"/opt/homebrew/opt/judy/lib/libJudy.dylib",
    c"libJudy.dylib",
];

/// Where our own cdylib lands. `EXPANSE_CDYLIB` is what CI sets, since
/// only the build knows the profile directory; the rest are for a local
/// `cargo bench` from the workspace root.
const OURS_NAMES: &[&core::ffi::CStr] = &[
    c"target/release/libexpanse.so",
    c"target/debug/libexpanse.so",
    c"libexpanse.so",
];

impl Lib {
    fn open_any(candidates: &[&core::ffi::CStr], what: &str) -> Self {
        for name in candidates {
            // SAFETY: valid NUL-terminated library name.
            let h = unsafe { dlopen(name.to_bytes_with_nul().as_ptr(), RTLD_NOW | RTLD_LOCAL) };
            if !h.is_null() {
                return Self(h);
            }
        }
        panic!("{what} not found (tried {candidates:?})");
    }

    fn stock() -> Self {
        Self::open_any(STOCK_NAMES, "stock libjudy — install libjudy-dev")
    }

    /// Our own cdylib, loaded the same way stock is. Prefers the path CI
    /// exports so the arm cannot silently measure a stale build.
    fn ours() -> Self {
        if let Ok(p) = std::env::var("EXPANSE_CDYLIB") {
            let c = std::ffi::CString::new(p.clone()).expect("EXPANSE_CDYLIB has an interior NUL");
            // SAFETY: valid NUL-terminated library name.
            let h = unsafe { dlopen(c.to_bytes_with_nul().as_ptr(), RTLD_NOW | RTLD_LOCAL) };
            assert!(!h.is_null(), "EXPANSE_CDYLIB set to {p} but dlopen failed");
            return Self(h);
        }
        Self::open_any(OURS_NAMES, "libexpanse.so — set EXPANSE_CDYLIB")
    }

    fn sym<T: Copy>(&self, name: &core::ffi::CStr) -> T {
        // SAFETY: live handle and NUL-terminated name; the caller names
        // the fn-pointer type matching the symbol.
        let p = unsafe { dlsym(self.0, name.to_bytes_with_nul().as_ptr()) };
        assert!(!p.is_null(), "missing symbol {name:?}");
        // SAFETY: fn-pointer transmute of a resolved symbol.
        unsafe { core::mem::transmute_copy(&p) }
    }

    /// Binds every entry point once, so a benchmark body never pays for
    /// symbol resolution. This is the fix for the bias that made stock
    /// carry a full `dlopen` inside its measured region while our rlib
    /// arm carried none.
    fn api(&self) -> Api {
        Api {
            ins: self.sym(c"JudyLIns"),
            get: self.sym(c"JudyLGet"),
            set1: self.sym(c"Judy1Set"),
            test1: self.sym(c"Judy1Test"),
            del: self.sym(c"JudyLDel"),
        }
    }
}

/// Entry points resolved ahead of the measured region.
#[derive(Clone, Copy)]
struct Api {
    ins: FpIns,
    get: FpGet,
    set1: F1Set,
    test1: F1Test,
    del: FpDel,
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
    /// `None` for the directly linked rlib arm; `Some` for the two
    /// `dlopen`'d arms, resolved here so the probe loop never pays for it.
    api: Option<Api>,
}

// SAFETY: the array is created and consumed on the same thread by the
// benchmark harness; the pointer is never shared.
unsafe impl Send for Built {}

/// Keys for an insert benchmark, generated in `setup`.
///
/// Key *generation* used to sit inside the measured region of every
/// insert arm — 30k (or 1.5M) xorshift steps and a `Vec` growth charged
/// to both libraries alike. Symmetric, but not harmless: it is identical
/// work added to both sides, so it pulls every insert ratio toward 1.00
/// and made us look closer to stock than we are.
struct Feed {
    ks: Vec<Word>,
    api: Option<Api>,
}

// SAFETY: fn pointers into a library that is never `dlclose`d; used only
// on the harness thread that built the struct.
unsafe impl Send for Feed {}

/// The handle is deliberately never `dlclose`d — the resolved pointers
/// outlive it and each benchmark is its own process.
fn stock_api() -> Option<Api> {
    Some(Lib::stock().api())
}

fn ours_api() -> Option<Api> {
    Some(Lib::ours().api())
}

fn feed(dist: &str, api: Option<Api>) -> Feed {
    Feed {
        ks: keys(dist),
        api,
    }
}

fn feed_expanse(dist: &str) -> Feed {
    feed(dist, None)
}
fn feed_expanse_dl(dist: &str) -> Feed {
    feed(dist, ours_api())
}
fn feed_stock(dist: &str) -> Feed {
    feed(dist, stock_api())
}

/// Builds a JudyL array through `api` (or directly, when `None`).
fn build_map(dist: &str, api: Option<Api>) -> Built {
    let ks = keys(dist);
    let probes = shuffled(ks.clone());
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard JudyL usage; the returned slot is valid until the
    // next mutation and is written immediately.
    unsafe {
        for &k in &ks {
            let slot = match api {
                Some(a) => (a.ins)(&raw mut arr, k, null_mut()),
                None => expanse::JudyLIns(&raw mut arr, k, null_mut()),
            }
            .cast::<Word>();
            *slot = k;
        }
    }
    Built { arr, probes, api }
}

/// Builds a Judy1 array through `api` (or directly, when `None`).
fn build_set(dist: &str, api: Option<Api>) -> Built {
    let ks = keys(dist);
    let probes = shuffled(ks.clone());
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard Judy1 usage.
    unsafe {
        for &k in &ks {
            match api {
                Some(a) => {
                    (a.set1)(&raw mut arr, k, null_mut());
                }
                None => {
                    expanse::Judy1Set(&raw mut arr, k, null_mut());
                }
            }
        }
    }
    Built { arr, probes, api }
}

fn build_expanse(dist: &str) -> Built {
    build_map(dist, None)
}
fn build_expanse_dl(dist: &str) -> Built {
    build_map(dist, ours_api())
}
fn build_stock(dist: &str) -> Built {
    build_map(dist, stock_api())
}
fn build_set_expanse(dist: &str) -> Built {
    build_set(dist, None)
}
fn build_set_expanse_dl(dist: &str) -> Built {
    build_set(dist, ours_api())
}
fn build_set_stock(dist: &str) -> Built {
    build_set(dist, stock_api())
}

// ---- JudyL insert -----------------------------------------------------

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = feed_expanse)]
#[bench::random(args = ("random",), setup = feed_expanse)]
#[bench::clustered(args = ("clustered",), setup = feed_expanse)]
fn judyl_insert_expanse(f: Feed) -> Word {
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard JudyL usage; the slot is valid until the next
    // mutation and is written immediately.
    unsafe {
        for &k in &f.ks {
            let slot = expanse::JudyLIns(&raw mut arr, black_box(k), null_mut()).cast::<Word>();
            *slot = k;
        }
        // Deliberately NOT freed: teardown is a different code path
        // (our class-sized dealloc vs stock's chunked free) and at
        // POP_BIG it rivals the work being measured. Each bench runs in
        // its own process, so the leak is bounded and intentional.
        black_box(map_len_sentinel(arr))
    }
}

// Our own `libexpanse.so`, reached exactly as stock is. THIS is the arm
// to compare against `judyl_insert_stock` — the rlib arm above gets LTO
// and direct calls that no drop-in consumer of libexpanse has.
#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = feed_expanse_dl)]
#[bench::random(args = ("random",), setup = feed_expanse_dl)]
#[bench::clustered(args = ("clustered",), setup = feed_expanse_dl)]
fn judyl_insert_expanse_dl(f: Feed) -> Word {
    let ins = f.api.expect("dl arm needs a resolved api").ins;
    let mut arr: *mut c_void = null_mut();
    // SAFETY: same contract as the rlib arm.
    unsafe {
        for &k in &f.ks {
            let slot = ins(&raw mut arr, black_box(k), null_mut()).cast::<Word>();
            *slot = k;
        }
        black_box(map_len_sentinel(arr))
    }
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = feed_stock)]
#[bench::random(args = ("random",), setup = feed_stock)]
#[bench::clustered(args = ("clustered",), setup = feed_stock)]
fn judyl_insert_stock(f: Feed) -> Word {
    let ins = f.api.expect("stock arm needs a resolved api").ins;
    let mut arr: *mut c_void = null_mut();
    // SAFETY: same contract as the libexpanse arms.
    unsafe {
        for &k in &f.ks {
            let slot = ins(&raw mut arr, black_box(k), null_mut()).cast::<Word>();
            *slot = k;
        }
        // Not freed — see the libexpanse arm.
        black_box(map_len_sentinel(arr))
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
fn judyl_get_expanse(built: Built) -> Word {
    let mut sink = 0usize;
    // SAFETY: array built by `build_expanse`, freed here.
    unsafe {
        for &k in &built.probes {
            let slot = expanse::JudyLGet(built.arr, black_box(k), null_mut()).cast::<Word>();
            if !slot.is_null() {
                sink ^= *slot;
            }
        }
        // Not freed — teardown stays out of the measured region.
    }
    black_box(sink)
}

// Our own `libexpanse.so`, reached exactly as stock is — the arm to
// compare against `judyl_get_stock`.
#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = build_expanse_dl)]
#[bench::random(args = ("random",), setup = build_expanse_dl)]
#[bench::clustered(args = ("clustered",), setup = build_expanse_dl)]
#[bench::random_big(args = ("random_big",), setup = build_expanse_dl)]
fn judyl_get_expanse_dl(built: Built) -> Word {
    let get = built.api.expect("dl arm needs a resolved api").get;
    let mut sink = 0usize;
    // SAFETY: array built by `build_expanse_dl` through the same library.
    unsafe {
        for &k in &built.probes {
            let slot = get(built.arr, black_box(k), null_mut()).cast::<Word>();
            if !slot.is_null() {
                sink ^= *slot;
            }
        }
    }
    black_box(sink)
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = build_stock)]
#[bench::random(args = ("random",), setup = build_stock)]
#[bench::clustered(args = ("clustered",), setup = build_stock)]
#[bench::random_big(args = ("random_big",), setup = build_stock)]
fn judyl_get_stock(built: Built) -> Word {
    // Symbols are bound in `setup` now. They used to be resolved here,
    // charging stock a full `dlopen` inside its measured region that
    // neither libexpanse arm paid — a bias that flattered every
    // published lookup ratio (issue #1).
    let get = built.api.expect("stock arm needs a resolved api").get;
    let mut sink = 0usize;
    // SAFETY: array built by `build_stock` through the same library.
    unsafe {
        for &k in &built.probes {
            let slot = get(built.arr, black_box(k), null_mut()).cast::<Word>();
            if !slot.is_null() {
                sink ^= *slot;
            }
        }
        // Not freed — teardown stays out of the measured region.
    }
    black_box(sink)
}

// ---- JudyL steady-state churn ------------------------------------------
//
// Measured region: ONLY the mixed op loop — upsert an existing key,
// insert a fresh neighbour, delete it again — over a prebuilt array
// (`setup = build_*`). The insert arms above insert each key once,
// fresh, so no arm exercised upsert-of-present or delete at steady
// state; structural growth was measured, steady-state operation never.

#[library_benchmark]
#[bench::random(args = ("random",), setup = build_expanse)]
fn judyl_churn_expanse(built: Built) -> Word {
    let mut arr = built.arr;
    let mut sink = 0usize;
    // SAFETY: standard JudyL usage on the prebuilt array; slots are
    // written immediately and fresh keys are removed before reuse.
    unsafe {
        for &k in &built.probes {
            let slot = expanse::JudyLIns(&raw mut arr, black_box(k), null_mut()).cast::<Word>();
            sink ^= *slot;
            *slot = !k;
            let fresh =
                expanse::JudyLIns(&raw mut arr, black_box(k ^ 1), null_mut()).cast::<Word>();
            *fresh = k;
            sink ^= expanse::JudyLDel(&raw mut arr, black_box(k ^ 1), null_mut()) as Word;
        }
        black_box(map_len_sentinel(arr));
    }
    black_box(sink)
}

// Our own `libexpanse.so` — the arm to compare against `judyl_churn_stock`.
#[library_benchmark]
#[bench::random(args = ("random",), setup = build_expanse_dl)]
fn judyl_churn_expanse_dl(built: Built) -> Word {
    let api = built.api.expect("dl arm needs a resolved api");
    let mut arr = built.arr;
    let mut sink = 0usize;
    // SAFETY: same contract as the rlib arm.
    unsafe {
        for &k in &built.probes {
            let slot = (api.ins)(&raw mut arr, black_box(k), null_mut()).cast::<Word>();
            sink ^= *slot;
            *slot = !k;
            let fresh = (api.ins)(&raw mut arr, black_box(k ^ 1), null_mut()).cast::<Word>();
            *fresh = k;
            sink ^= (api.del)(&raw mut arr, black_box(k ^ 1), null_mut()) as Word;
        }
        black_box(map_len_sentinel(arr));
    }
    black_box(sink)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = build_stock)]
fn judyl_churn_stock(built: Built) -> Word {
    let api = built.api.expect("stock arm needs a resolved api");
    let mut arr = built.arr;
    let mut sink = 0usize;
    // SAFETY: same contract as the libexpanse arms.
    unsafe {
        for &k in &built.probes {
            let slot = (api.ins)(&raw mut arr, black_box(k), null_mut()).cast::<Word>();
            sink ^= *slot;
            *slot = !k;
            let fresh = (api.ins)(&raw mut arr, black_box(k ^ 1), null_mut()).cast::<Word>();
            *fresh = k;
            sink ^= (api.del)(&raw mut arr, black_box(k ^ 1), null_mut()) as Word;
        }
        black_box(map_len_sentinel(arr));
    }
    black_box(sink)
}

// ---- Judy1 ------------------------------------------------------------

#[library_benchmark]
#[bench::random(args = ("random",), setup = feed_expanse)]
#[bench::clustered(args = ("clustered",), setup = feed_expanse)]
fn judy1_set_expanse(f: Feed) -> Word {
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard Judy1 usage.
    unsafe {
        for &k in &f.ks {
            expanse::Judy1Set(&raw mut arr, black_box(k), null_mut());
        }
        black_box(map_len_sentinel(arr))
    }
}

// Our own `libexpanse.so` — the arm to compare against `judy1_set_stock`.
#[library_benchmark]
#[bench::random(args = ("random",), setup = feed_expanse_dl)]
#[bench::clustered(args = ("clustered",), setup = feed_expanse_dl)]
fn judy1_set_expanse_dl(f: Feed) -> Word {
    let set = f.api.expect("dl arm needs a resolved api").set1;
    let mut arr: *mut c_void = null_mut();
    // SAFETY: same contract as the rlib arm.
    unsafe {
        for &k in &f.ks {
            set(&raw mut arr, black_box(k), null_mut());
        }
        black_box(map_len_sentinel(arr))
    }
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = feed_stock)]
#[bench::clustered(args = ("clustered",), setup = feed_stock)]
fn judy1_set_stock(f: Feed) -> Word {
    let set = f.api.expect("stock arm needs a resolved api").set1;
    let mut arr: *mut c_void = null_mut();
    // SAFETY: same contract as the libexpanse arms.
    unsafe {
        for &k in &f.ks {
            set(&raw mut arr, black_box(k), null_mut());
        }
        black_box(map_len_sentinel(arr))
    }
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = build_set_expanse)]
fn judy1_test_expanse(built: Built) -> Word {
    let mut hits = 0usize;
    // SAFETY: array built by `build_set_expanse`.
    unsafe {
        for &k in &built.probes {
            hits += expanse::Judy1Test(built.arr, black_box(k), null_mut()) as usize;
        }
        // Not freed — teardown stays out of the measured region.
    }
    black_box(hits)
}

// Our own `libexpanse.so` — the arm to compare against `judy1_test_stock`.
#[library_benchmark]
#[bench::random(args = ("random",), setup = build_set_expanse_dl)]
fn judy1_test_expanse_dl(built: Built) -> Word {
    let test = built.api.expect("dl arm needs a resolved api").test1;
    let mut hits = 0usize;
    // SAFETY: array built by `build_set_expanse_dl` through the same library.
    unsafe {
        for &k in &built.probes {
            hits += test(built.arr, black_box(k), null_mut()) as usize;
        }
    }
    black_box(hits)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = build_set_stock)]
fn judy1_test_stock(built: Built) -> Word {
    let test = built.api.expect("stock arm needs a resolved api").test1;
    let mut hits = 0usize;
    // SAFETY: array built by `build_set_stock` through the same library.
    unsafe {
        for &k in &built.probes {
            hits += test(built.arr, black_box(k), null_mut()) as usize;
        }
        // Not freed — teardown stays out of the measured region.
    }
    black_box(hits)
}

library_benchmark_group!(
    name = vs_stock;
    benchmarks =
        judyl_insert_expanse,
        judyl_insert_expanse_dl,
        judyl_insert_stock,
        judyl_get_expanse,
        judyl_get_expanse_dl,
        judyl_get_stock,
        judyl_churn_expanse,
        judyl_churn_expanse_dl,
        judyl_churn_stock,
        judy1_set_expanse,
        judy1_set_expanse_dl,
        judy1_set_stock,
        judy1_test_expanse,
        judy1_test_expanse_dl,
        judy1_test_stock
);

#[cfg(target_os = "linux")]
main!(library_benchmark_groups = vs_stock);

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("iai-callgrind vs_stock benchmarks run on Linux only.");
}
