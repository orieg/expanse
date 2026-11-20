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
//! `Judy1Test`/`JudySLIns`/`JudySLGet` with identical key streams. Stock is
//! reached through
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
//! that becomes wall-clock. The `Estimated Cycles` column is the closer
//! proxy.
//!
//! This harness does emit L1/LL/RAM counts. An earlier revision of this
//! header said it did not, on the grounds that `--cache-sim=yes` was not
//! passed; the runner sets `CACHE_SIM = true` as a default, so simulation
//! has always been on. `Estimated Cycles` is `L1 + 5*LL + 35*RAM`, which
//! is why it has never equalled `Instructions`. The flag is now stated
//! explicitly below so a dependency bump cannot turn it off silently.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `capi_vs_stock` |
//! | `group` | 1 |
//! | `population` | `POP = 30_000`, `POP_BIG = 1_500_000`; the `judysl_*`, `judyhs_*` and `judyhs_long_*` arms use `POP`; the `judy1_colour_*` arms (#1682) replay one breadth-first search over 10k (`n10k`) or 100k (`n100k`) vertices; the `judyl_small_array_*` arms (#1742) run 1,000 arrays of 1, 10, 20, 31, 32 or 64 keys (`k1` to `k64`) |
//! | `insertion_order` | generator draw order — the population is inserted as drawn, neither sorted nor shuffled; the shuffle in this file is applied to the probe stream, not to the build; `judy1_colour_*` set vertex ids in breadth-first visit order; `judyl_small_array_*` inserts each array's keys as drawn |
//! | `probes_and_reuse` | 30k / 1.5M, reuse 1.0; `judy1_colour_*`: the search's 12n + 2 Judy1 calls, once; `judyl_next_*` and `judyl_prev_*` one whole traversal, the four `judyl_*_empty_*` families every 64th probe (469 searches), `judyl_free_*` one call; `judyl_small_array_*`: per array k inserts, 2k lookups, one traversal, one count, one by-count, one `JudyLMemUsed`, k or k/2 deletes and one `JudyLFreeArray` |
//! | `hit_rate` | 100% / mixed; `judyhs_get_*` and `judyhs_long_get_*` 50% (every present key and `POP` absent ones, shuffled together); `judy1_colour_*` as the search produces it; `judyl_small_array_*` lookups 50% (each array's k keys and k absent ones) |
//! | `miss_gen_method` | Interleaved keys; `judyhs_get_*` continues `fixed8_stream` past the population, rejected on membership; `judyhs_long_get_*` continues `route_keys` past the population (an injective generator, so absent by construction); `judyl_small_array_*` continues its key generator past the population, rejected on membership |
//! | `value_dereference` | `*slot` dereferenced |
//! | `key_shape` | `Word` for the `judyl_*`/`judy1_*` arms; NUL-terminated fixed-width strings for `judysl_*`, derived from the same `keys(dist)` so the three distributions carry over; 8-byte zmap response-dedup keys for `judyhs_*` (`fixed8_stream`: a 198.18.0.0/15 address and one of four ports, big-endian, status and padding zero, in a scan's pseudo-random order); ~40-byte route strings (`route_keys`: `/api/v2/tenants/{:06}/resources/{:04}`, the engine's `routes` shape) for `judyhs_long_*`, every key longer than 8 bytes. Absolute counts for the string arms include `strlen` on both sides and are **not** comparable to the `Word`-keyed arms; their ratio is. |
//! | `measured_region` | Deliberately leaks to exclude drop; `judyl_del_*`, `judyl_free_*` and `judyl_small_array_*` measure teardown itself, so they free |
//! | `arm_symmetry` | **Three arms**: `*_expanse` (rlib, LTO-linked), `*_expanse_dl` (dlopen'd cdylib), and `*_stock` (dlopen'd libjudy). `*_expanse_dl` vs `*_stock` is symmetric; `*_expanse` vs `*_stock` is asymmetric (LTO bias). libexpanse's JudyHS hashes keys longer than 8 bytes; the bench builds both Expanse arms with `bench-fixed-hash-seed`, which fixes the hash keys so the `judyhs_long_*_expanse*` counts repeat between runs (#1761), and refuses to run without it or with a `libexpanse.so` built without it |
//! | `statistics` | iai Callgrind exact counts (simulated 8 MiB LL) |
//! | `verdict` | **PASS** `[verified: RUN (CI instruction-counts)]`: Carrying both `expanse` and `expanse_dl` isolates LTO delta; 1.5M population is load-bearing in #456. |
//!
//! The modelled hierarchy is fixed at I1/D1 32 KiB 8-way and LL 8 MiB
//! 16-way, for cross-machine comparability. It is not the reference
//! host's 30 MiB L3, so these columns cannot locate that host's L3 cliff.
//! Hardware counters for that are the `point_lookup_counters` suite; see
//! https://github.com/orieg/expanse/issues/455.
//!
//! Linux/CI only (valgrind, plus `libjudy-dev` for the stock library).

#![allow(missing_docs)]

#[path = "colour_map_common/mod.rs"]
mod colour_map_common;

use core::ffi::{c_int, c_void};
#[cfg(target_os = "linux")]
use iai_callgrind::main;
use iai_callgrind::{
    Callgrind, LibraryBenchmarkConfig, library_benchmark, library_benchmark_group,
};
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
type FFree1 = unsafe extern "C" fn(*mut *mut c_void, *mut c_void) -> Word;
/// JudySL takes a NUL-terminated `char*` rather than a `Word`, which is the
/// only shape difference from the families above.
type FslIns = unsafe extern "C" fn(*mut *mut c_void, *const u8, *mut c_void) -> *mut c_void;
type FslGet = unsafe extern "C" fn(*const c_void, *const u8, *mut c_void) -> *mut c_void;
/// JudyHS takes a key pointer and its length; `JudyHSGet` has no `PJError`.
type FhsIns = unsafe extern "C" fn(*mut *mut c_void, *mut c_void, Word, *mut c_void) -> *mut c_void;
type FhsGet = unsafe extern "C" fn(*const c_void, *mut c_void, Word) -> *mut c_void;
type FhsDel = unsafe extern "C" fn(*mut *mut c_void, *mut c_void, Word, *mut c_void) -> c_int;

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
            sl_ins: self.sym(c"JudySLIns"),
            sl_get: self.sym(c"JudySLGet"),
            hs_ins: self.sym(c"JudyHSIns"),
            hs_get: self.sym(c"JudyHSGet"),
            hs_del: self.sym(c"JudyHSDel"),
            unset1: self.sym(c"Judy1Unset"),
            free1: self.sym(c"Judy1FreeArray"),
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
    sl_ins: FslIns,
    sl_get: FslGet,
    hs_ins: FhsIns,
    hs_get: FhsGet,
    hs_del: FhsDel,
    unset1: F1Set,
    free1: FFree1,
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
/// exceeds last-level cache on a typical runner, so cache-miss counts
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
    let lib = Lib::ours();
    // SAFETY: live handle and a NUL-terminated name.
    let marker = unsafe { dlsym(lib.0, c"expanse_bench_fixed_hash_seed".as_ptr().cast()) };
    assert!(
        !marker.is_null(),
        "libexpanse.so was built without `--features bench-fixed-hash-seed`: its \
         JudyHS seeds each map at random, so the judyhs_long_* counts would move \
         between runs (#1761). Rebuild it with \
         `cargo build --release -p expanse-capi --features bench-fixed-hash-seed`."
    );
    Some(lib.api())
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
// Cache-pressure arm: exceeds LLC (not observable without --cache-sim; see POP_BIG).
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

/// Callgrind simulator settings for this harness.
///
/// **`--cache-sim=yes` is stated here, not inherited.** iai-callgrind's runner
/// already defaults it on (`iai-callgrind-runner` `defaults::CACHE_SIM = true`),
/// which is why the L1/LL/RAM hit counts `scripts/perf_report.py` renders have
/// been in every PR comment all along. Passing it explicitly changes no number
/// and costs nothing; it makes the harness say which instrument it uses instead
/// of depending on a dependency default that a version bump could flip.
///
/// The simulated cache is fixed by the runner and is **not this machine's**:
/// I1 and D1 are 32 KiB 8-way, LL is 8 MiB 16-way, 64-byte lines. Fixed sizes
/// are what make the counts comparable across hosts, and they are also why a
/// question about a real last-level cache — where the L3 cliff sits on the
/// reference host, for instance — cannot be answered here. That needs hardware
/// counters (`scripts/perf_counters.py`).
///
/// **`--branch-sim=yes` is opt-in through `EXPANSE_BRANCH_SIM=1`.** It has no
/// runner default, and it adds a branch-predictor simulation on top of
/// callgrind's own slowdown, so the regression pass — gated on instructions
/// retired, needing no branch column — leaves it off and measures exactly what
/// it measured before. Instruction counts are unaffected either way.
///
/// An unrecognised value is fatal rather than ignored. A mistyped
/// `EXPANSE_BRANCH_SIM=yes` that quietly produced a run with no branch columns
/// would be a run published as a misprediction measurement that never simulated
/// a predictor (`AGENTS.md` section 8.1).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn bench_config() -> LibraryBenchmarkConfig {
    let mut config = LibraryBenchmarkConfig::default();
    let mut args = vec!["--cache-sim=yes"];
    let requested = match std::env::var("EXPANSE_BRANCH_SIM") {
        Ok(v) => v,
        Err(std::env::VarError::NotPresent) => String::new(),
        Err(e) => panic!("EXPANSE_BRANCH_SIM is set but unreadable: {e}"),
    };
    match requested.as_str() {
        "" | "0" => {}
        "1" => args.push("--branch-sim=yes"),
        other => panic!(
            "EXPANSE_BRANCH_SIM={other} is not a recognised value: use 1 to add the \
             branch-predictor simulation, or 0 / unset to leave it off"
        ),
    }
    config.tool(Callgrind::with_args(args));
    config
}

// ---- JudySL (C-string keys) ------------------------------------------
//
// The fourth compat family, and until now the only one absent from this
// comparison: §2.2 commits `JudySL*` to the same zero-overhead drop-in
// parity as `JudyL*` and `Judy1*`, and nothing measured it.
//
// Keys are derived from the same `keys(dist)` the other arms use, so the
// three distributions carry over: a clustered `Word` set is a clustered
// string set once formatted at fixed width. Both sides pay the same
// `strlen`, so the ratio stays like-for-like even though the absolute
// counts are larger than the `Word`-keyed arms and are not comparable to
// them.

/// NUL-terminated, fixed-width keys derived from `keys(dist)`.
fn str_keys(dist: &str) -> Vec<Vec<u8>> {
    keys(dist)
        .into_iter()
        .map(|k| {
            let mut b = format!("/k/{k:016x}").into_bytes();
            b.push(0);
            b
        })
        .collect()
}

/// Keys for a JudySL insert arm, generated in `setup`.
struct StrFeed {
    ks: Vec<Vec<u8>>,
    api: Option<Api>,
}

// SAFETY: fn pointers into a library that is never `dlclose`d; used only on
// the harness thread that built the struct.
unsafe impl Send for StrFeed {}

/// A built JudySL array plus its shuffled probe order.
struct StrBuilt {
    arr: *mut c_void,
    probes: Vec<Vec<u8>>,
    api: Option<Api>,
}

// SAFETY: created and consumed on the same thread; never shared.
unsafe impl Send for StrBuilt {}

fn str_feed(dist: &str, api: Option<Api>) -> StrFeed {
    StrFeed {
        ks: str_keys(dist),
        api,
    }
}
fn str_feed_expanse(dist: &str) -> StrFeed {
    str_feed(dist, None)
}
fn str_feed_expanse_dl(dist: &str) -> StrFeed {
    str_feed(dist, ours_api())
}
fn str_feed_stock(dist: &str) -> StrFeed {
    str_feed(dist, stock_api())
}

/// Builds a JudySL array through `api` (or directly, when `None`), outside
/// the measured region.
fn build_strmap(dist: &str, api: Option<Api>) -> StrBuilt {
    let ks = str_keys(dist);
    let mut probes = ks.clone();
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard JudySL usage; each key is NUL-terminated and the
    // returned slot is valid until the next mutation.
    unsafe {
        for (i, k) in ks.iter().enumerate() {
            let slot = match api {
                Some(a) => (a.sl_ins)(&raw mut arr, k.as_ptr(), null_mut()),
                None => expanse::JudySLIns(&raw mut arr, k.as_ptr(), null_mut()),
            }
            .cast::<Word>();
            *slot = i as Word;
        }
    }
    StrBuilt { arr, probes, api }
}
fn build_str_expanse(dist: &str) -> StrBuilt {
    build_strmap(dist, None)
}
fn build_str_expanse_dl(dist: &str) -> StrBuilt {
    build_strmap(dist, ours_api())
}
fn build_str_stock(dist: &str) -> StrBuilt {
    build_strmap(dist, stock_api())
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = str_feed_expanse)]
#[bench::random(args = ("random",), setup = str_feed_expanse)]
#[bench::clustered(args = ("clustered",), setup = str_feed_expanse)]
fn judysl_insert_expanse(f: StrFeed) -> Word {
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard JudySL usage; the slot is written immediately.
    unsafe {
        for (i, k) in f.ks.iter().enumerate() {
            let slot =
                expanse::JudySLIns(&raw mut arr, black_box(k.as_ptr()), null_mut()).cast::<Word>();
            *slot = i as Word;
        }
        // Not freed, as in every other arm here: teardown is a different
        // code path and each bench runs in its own process.
        black_box(map_len_sentinel(arr))
    }
}

// Our own `libexpanse.so`, reached exactly as stock is. THIS is the arm to
// compare against `judysl_insert_stock`.
#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = str_feed_expanse_dl)]
#[bench::random(args = ("random",), setup = str_feed_expanse_dl)]
#[bench::clustered(args = ("clustered",), setup = str_feed_expanse_dl)]
fn judysl_insert_expanse_dl(f: StrFeed) -> Word {
    let ins = f.api.expect("dl arm needs a resolved api").sl_ins;
    let mut arr: *mut c_void = null_mut();
    // SAFETY: same contract as the rlib arm.
    unsafe {
        for (i, k) in f.ks.iter().enumerate() {
            let slot = ins(&raw mut arr, black_box(k.as_ptr()), null_mut()).cast::<Word>();
            *slot = i as Word;
        }
        black_box(map_len_sentinel(arr))
    }
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = str_feed_stock)]
#[bench::random(args = ("random",), setup = str_feed_stock)]
#[bench::clustered(args = ("clustered",), setup = str_feed_stock)]
fn judysl_insert_stock(f: StrFeed) -> Word {
    let ins = f.api.expect("stock arm needs a resolved api").sl_ins;
    let mut arr: *mut c_void = null_mut();
    // SAFETY: same contract as the libexpanse arms.
    unsafe {
        for (i, k) in f.ks.iter().enumerate() {
            let slot = ins(&raw mut arr, black_box(k.as_ptr()), null_mut()).cast::<Word>();
            *slot = i as Word;
        }
        black_box(map_len_sentinel(arr))
    }
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = build_str_expanse)]
#[bench::random(args = ("random",), setup = build_str_expanse)]
#[bench::clustered(args = ("clustered",), setup = build_str_expanse)]
fn judysl_get_expanse(built: StrBuilt) -> Word {
    let mut sink = 0 as Word;
    // SAFETY: the array was built in `setup`; JudySLGet only reads.
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

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = build_str_expanse_dl)]
#[bench::random(args = ("random",), setup = build_str_expanse_dl)]
#[bench::clustered(args = ("clustered",), setup = build_str_expanse_dl)]
fn judysl_get_expanse_dl(built: StrBuilt) -> Word {
    let get = built.api.expect("dl arm needs a resolved api").sl_get;
    let mut sink = 0 as Word;
    // SAFETY: same contract as the rlib arm.
    unsafe {
        for k in &built.probes {
            let slot = get(built.arr, black_box(k.as_ptr()), null_mut()).cast::<Word>();
            if !slot.is_null() {
                sink ^= *slot;
            }
        }
    }
    black_box(sink)
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = build_str_stock)]
#[bench::random(args = ("random",), setup = build_str_stock)]
#[bench::clustered(args = ("clustered",), setup = build_str_stock)]
fn judysl_get_stock(built: StrBuilt) -> Word {
    let get = built.api.expect("stock arm needs a resolved api").sl_get;
    let mut sink = 0 as Word;
    // SAFETY: same contract as the libexpanse arms.
    unsafe {
        for k in &built.probes {
            let slot = get(built.arr, black_box(k.as_ptr()), null_mut()).cast::<Word>();
            if !slot.is_null() {
                sink ^= *slot;
            }
        }
    }
    black_box(sink)
}

// ---- JudyHS (fixed 8-byte keys, #1671) -------------------------------
//
// zmap's only use of libjudy: JudyHS keyed by its 8-byte response-dedup key,
// `target_t { ip, port, status }` with its padding byte. The rlib arm calls
// `expanse::JudyHS*` directly; the other two reach their library as the rest
// of this file does. Keys of at most 8 bytes go to libexpanse's ordered
// tries unhashed, so these arms do not depend on the hash seed; the
// long-key arms below do, and run under `bench-fixed-hash-seed` (#1761).

/// Ports a zmap scan of the `fixed8` shape probes, in its key's port field.
const FIXED8_PORTS: [u16; 4] = [80, 443, 8080, 8443];

/// Draws `n` distinct zmap-shaped 8-byte keys: the 198.18.0.0/15
/// benchmarking range times four ports (524,288 keys), in the pseudo-random
/// order a scan visits them, address and port big-endian, status and padding
/// zero. Draws in `reject` are passed over, so misses come from the same
/// generator as the population and are rejected on membership (§8.6). The
/// generator is `crates/expanse/benches/instructions.rs`' `fixed8_stream`.
fn fixed8_stream(n: usize, reject: &std::collections::HashSet<[u8; 8]>) -> Vec<[u8; 8]> {
    let mut rng = XorShift(0x2E9A_7C01_D3D0_5EED);
    let mut seen = std::collections::HashSet::with_capacity(n);
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        let r = rng.next();
        let off = (r >> 2) as u32 & 0x1_FFFF;
        let ip = 0xC612_0000u32 | off;
        let port = FIXED8_PORTS[(r & 3) as usize];
        let mut k = [0u8; 8];
        k[..4].copy_from_slice(&ip.to_be_bytes());
        k[4..6].copy_from_slice(&port.to_be_bytes());
        if !reject.contains(&k) && seen.insert(k) {
            out.push(k);
        }
    }
    out
}

/// Keys for a JudyHS insert arm, generated in `setup`.
struct HsFeed {
    ks: Vec<[u8; 8]>,
    api: Option<Api>,
}

// SAFETY: fn pointers into a library that is never `dlclose`d; used only on
// the harness thread that built the struct.
unsafe impl Send for HsFeed {}

/// A built JudyHS array plus its probe stream.
struct HsBuilt {
    arr: *mut c_void,
    probes: Vec<[u8; 8]>,
    api: Option<Api>,
}

// SAFETY: created and consumed on the same thread; never shared.
unsafe impl Send for HsBuilt {}

fn hs_feed(api: Option<Api>) -> HsFeed {
    HsFeed {
        ks: fixed8_stream(POP, &Default::default()),
        api,
    }
}
fn hs_feed_expanse(_dist: &str) -> HsFeed {
    hs_feed(None)
}
fn hs_feed_expanse_dl(_dist: &str) -> HsFeed {
    hs_feed(ours_api())
}
fn hs_feed_stock(_dist: &str) -> HsFeed {
    hs_feed(stock_api())
}

fn shuffle_hs(probes: &mut [[u8; 8]], seed: u64) {
    let mut rng = XorShift(seed);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
}

/// Builds the `POP`-key JudyHS array through `api` (or directly, when
/// `None`), outside the measured region. With `mixed`, the probes are every
/// present key and `POP` absent ones, shuffled together (50% hits); without
/// it, the present keys shuffled.
fn build_hs(api: Option<Api>, mixed: bool) -> HsBuilt {
    let ks = fixed8_stream(POP, &Default::default());
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard JudyHS usage; each key is 8 live bytes and the
    // returned slot is valid until the next mutation.
    unsafe {
        for (i, k) in ks.iter().enumerate() {
            let kp = k.as_ptr().cast_mut().cast();
            let slot = match api {
                Some(a) => (a.hs_ins)(&raw mut arr, kp, 8, null_mut()),
                None => expanse::JudyHSIns(&raw mut arr, kp, 8, null_mut()),
            }
            .cast::<Word>();
            *slot = i as Word + 1;
        }
    }
    let mut probes = ks;
    if mixed {
        let present = probes.iter().copied().collect();
        probes.extend(fixed8_stream(POP, &present));
        shuffle_hs(&mut probes, 0x5851_F42D);
    } else {
        shuffle_hs(&mut probes, 0x9E37_79B9);
    }
    HsBuilt { arr, probes, api }
}
fn build_hs_get_expanse(_dist: &str) -> HsBuilt {
    build_hs(None, true)
}
fn build_hs_get_expanse_dl(_dist: &str) -> HsBuilt {
    build_hs(ours_api(), true)
}
fn build_hs_get_stock(_dist: &str) -> HsBuilt {
    build_hs(stock_api(), true)
}
fn build_hs_del_expanse(_dist: &str) -> HsBuilt {
    build_hs(None, false)
}
fn build_hs_del_expanse_dl(_dist: &str) -> HsBuilt {
    build_hs(ours_api(), false)
}
fn build_hs_del_stock(_dist: &str) -> HsBuilt {
    build_hs(stock_api(), false)
}

/// The insert loop, shared by the three arms: `ins` is the resolved entry
/// point, or `expanse::JudyHSIns` for the rlib arm.
#[inline(always)]
fn hs_insert(ks: &[[u8; 8]], ins: FhsIns) -> Word {
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard JudyHS usage; the slot is written immediately.
    unsafe {
        for (i, k) in ks.iter().enumerate() {
            let kp = black_box(k.as_ptr()).cast_mut().cast();
            *ins(&raw mut arr, kp, 8, null_mut()).cast::<Word>() = i as Word + 1;
        }
    }
    // Not freed — teardown stays out of the measured region.
    black_box(map_len_sentinel(arr))
}

#[inline(always)]
fn hs_get(built: &HsBuilt, get: FhsGet) -> Word {
    let mut sink = 0 as Word;
    // SAFETY: the array was built in `setup`; JudyHSGet only reads.
    unsafe {
        for k in &built.probes {
            let slot = get(built.arr, black_box(k.as_ptr()).cast_mut().cast(), 8).cast::<Word>();
            if !slot.is_null() {
                sink ^= *slot;
            }
        }
    }
    black_box(sink)
}

#[inline(always)]
fn hs_del(mut built: HsBuilt, del: FhsDel) -> Word {
    let mut removed = 0 as Word;
    // SAFETY: the array was built in `setup` through the library `del`
    // belongs to; every probe is a present key.
    unsafe {
        for k in &built.probes {
            let kp = black_box(k.as_ptr()).cast_mut().cast();
            removed += del(&raw mut built.arr, kp, 8, null_mut()) as Word;
        }
    }
    black_box(removed)
}

unsafe extern "C" fn rlib_hs_ins(
    pp: *mut *mut c_void,
    k: *mut c_void,
    n: Word,
    pj: *mut c_void,
) -> *mut c_void {
    // SAFETY: forwarded unchanged.
    unsafe { expanse::JudyHSIns(pp, k, n, pj.cast()) }
}
unsafe extern "C" fn rlib_hs_get(p: *const c_void, k: *mut c_void, n: Word) -> *mut c_void {
    // SAFETY: forwarded unchanged.
    unsafe { expanse::JudyHSGet(p, k, n) }
}
unsafe extern "C" fn rlib_hs_del(
    pp: *mut *mut c_void,
    k: *mut c_void,
    n: Word,
    pj: *mut c_void,
) -> c_int {
    // SAFETY: forwarded unchanged.
    unsafe { expanse::JudyHSDel(pp, k, n, pj.cast()) }
}

#[library_benchmark]
#[bench::fixed8(args = ("fixed8",), setup = hs_feed_expanse)]
fn judyhs_insert_expanse(f: HsFeed) -> Word {
    hs_insert(&f.ks, rlib_hs_ins)
}

// Our own `libexpanse.so` — the arm to compare against `judyhs_insert_stock`.
#[library_benchmark]
#[bench::fixed8(args = ("fixed8",), setup = hs_feed_expanse_dl)]
fn judyhs_insert_expanse_dl(f: HsFeed) -> Word {
    hs_insert(&f.ks, f.api.expect("dl arm needs a resolved api").hs_ins)
}

#[library_benchmark]
#[bench::fixed8(args = ("fixed8",), setup = hs_feed_stock)]
fn judyhs_insert_stock(f: HsFeed) -> Word {
    hs_insert(&f.ks, f.api.expect("stock arm needs a resolved api").hs_ins)
}

#[library_benchmark]
#[bench::fixed8(args = ("fixed8",), setup = build_hs_get_expanse)]
fn judyhs_get_expanse(built: HsBuilt) -> Word {
    hs_get(&built, rlib_hs_get)
}

#[library_benchmark]
#[bench::fixed8(args = ("fixed8",), setup = build_hs_get_expanse_dl)]
fn judyhs_get_expanse_dl(built: HsBuilt) -> Word {
    hs_get(
        &built,
        built.api.expect("dl arm needs a resolved api").hs_get,
    )
}

#[library_benchmark]
#[bench::fixed8(args = ("fixed8",), setup = build_hs_get_stock)]
fn judyhs_get_stock(built: HsBuilt) -> Word {
    hs_get(
        &built,
        built.api.expect("stock arm needs a resolved api").hs_get,
    )
}

#[library_benchmark]
#[bench::fixed8(args = ("fixed8",), setup = build_hs_del_expanse)]
fn judyhs_del_expanse(built: HsBuilt) -> Word {
    hs_del(built, rlib_hs_del)
}

#[library_benchmark]
#[bench::fixed8(args = ("fixed8",), setup = build_hs_del_expanse_dl)]
fn judyhs_del_expanse_dl(built: HsBuilt) -> Word {
    let del = built.api.expect("dl arm needs a resolved api").hs_del;
    hs_del(built, del)
}

#[library_benchmark]
#[bench::fixed8(args = ("fixed8",), setup = build_hs_del_stock)]
fn judyhs_del_stock(built: HsBuilt) -> Word {
    let del = built.api.expect("stock arm needs a resolved api").hs_del;
    hs_del(built, del)
}

// ---- JudyHS on long keys (#1681) ---------------------------------------
//
// The `routes` shape the engine's `bytesmap_*` arms use: ~40-byte route
// strings with long shared prefixes. Every key is longer than 8 bytes, so
// libexpanse's JudyHS takes the hash path after one length compare; these arms
// measure that compare against stock on keys it does not help. Separate types
// and loops from the `fixed8` arms, whose code stays as it was.

/// `n` route-shaped keys starting at index `from`: index `i` is
/// `/api/v2/tenants/{i / 16:06}/resources/{i % 16:04}`. The generator is
/// injective, so indices past the population are absent keys of the same
/// shape (§8.6).
/// Refuses a harness built without `bench-fixed-hash-seed`: its rlib arm's
/// JudyHS would key each map's hasher at random, and the `judyhs_long_*`
/// counts would move between runs (#1761).
fn require_fixed_hash_seed() {
    #[cfg(not(feature = "bench-fixed-hash-seed"))]
    panic!(
        "vs_stock was built without `--features bench-fixed-hash-seed`: the rlib \
         arm's JudyHS would key each map's hasher at random, so the judyhs_long_* \
         counts would move between runs (#1761)."
    );
}

fn route_keys(from: usize, n: usize) -> Vec<Vec<u8>> {
    require_fixed_hash_seed();
    (from..from + n)
        .map(|i| format!("/api/v2/tenants/{:06}/resources/{:04}", i / 16, i % 16).into_bytes())
        .collect()
}

/// Keys for a long-key JudyHS insert arm, generated in `setup`.
struct HsLongFeed {
    ks: Vec<Vec<u8>>,
    api: Option<Api>,
}

// SAFETY: fn pointers into a library that is never `dlclose`d; used only on
// the harness thread that built the struct.
unsafe impl Send for HsLongFeed {}

/// A built long-key JudyHS array plus its probe stream.
struct HsLongBuilt {
    arr: *mut c_void,
    probes: Vec<Vec<u8>>,
    api: Option<Api>,
}

// SAFETY: created and consumed on the same thread; never shared.
unsafe impl Send for HsLongBuilt {}

fn hs_long_feed_expanse(_dist: &str) -> HsLongFeed {
    HsLongFeed {
        ks: route_keys(0, POP),
        api: None,
    }
}
fn hs_long_feed_expanse_dl(_dist: &str) -> HsLongFeed {
    HsLongFeed {
        ks: route_keys(0, POP),
        api: ours_api(),
    }
}
fn hs_long_feed_stock(_dist: &str) -> HsLongFeed {
    HsLongFeed {
        ks: route_keys(0, POP),
        api: stock_api(),
    }
}

fn shuffle_long(probes: &mut [Vec<u8>], seed: u64) {
    let mut rng = XorShift(seed);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
}

/// Builds the `POP`-key long-key array through `api` (or directly, when
/// `None`), outside the measured region. With `mixed`, the probes are every
/// present key and `POP` absent ones, shuffled together (50% hits).
fn build_hs_long(api: Option<Api>, mixed: bool) -> HsLongBuilt {
    let ks = route_keys(0, POP);
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard JudyHS usage; each key is live for its length and the
    // returned slot is valid until the next mutation.
    unsafe {
        for (i, k) in ks.iter().enumerate() {
            let kp = k.as_ptr().cast_mut().cast();
            let slot = match api {
                Some(a) => (a.hs_ins)(&raw mut arr, kp, k.len() as Word, null_mut()),
                None => expanse::JudyHSIns(&raw mut arr, kp, k.len() as Word, null_mut()),
            }
            .cast::<Word>();
            *slot = i as Word + 1;
        }
    }
    let mut probes = ks;
    if mixed {
        probes.extend(route_keys(POP, POP));
        shuffle_long(&mut probes, 0x5851_F42D);
    } else {
        shuffle_long(&mut probes, 0x9E37_79B9);
    }
    HsLongBuilt { arr, probes, api }
}
fn build_hs_long_get_expanse(_dist: &str) -> HsLongBuilt {
    build_hs_long(None, true)
}
fn build_hs_long_get_expanse_dl(_dist: &str) -> HsLongBuilt {
    build_hs_long(ours_api(), true)
}
fn build_hs_long_get_stock(_dist: &str) -> HsLongBuilt {
    build_hs_long(stock_api(), true)
}
fn build_hs_long_del_expanse(_dist: &str) -> HsLongBuilt {
    build_hs_long(None, false)
}
fn build_hs_long_del_expanse_dl(_dist: &str) -> HsLongBuilt {
    build_hs_long(ours_api(), false)
}
fn build_hs_long_del_stock(_dist: &str) -> HsLongBuilt {
    build_hs_long(stock_api(), false)
}

#[inline(always)]
fn hs_long_insert(ks: &[Vec<u8>], ins: FhsIns) -> Word {
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard JudyHS usage; the slot is written immediately.
    unsafe {
        for (i, k) in ks.iter().enumerate() {
            let kp = black_box(k.as_ptr()).cast_mut().cast();
            *ins(&raw mut arr, kp, k.len() as Word, null_mut()).cast::<Word>() = i as Word + 1;
        }
    }
    // Not freed — teardown stays out of the measured region.
    black_box(map_len_sentinel(arr))
}

#[inline(always)]
fn hs_long_get(built: &HsLongBuilt, get: FhsGet) -> Word {
    let mut sink = 0 as Word;
    // SAFETY: the array was built in `setup`; JudyHSGet only reads.
    unsafe {
        for k in &built.probes {
            let kp = black_box(k.as_ptr()).cast_mut().cast();
            let slot = get(built.arr, kp, k.len() as Word).cast::<Word>();
            if !slot.is_null() {
                sink ^= *slot;
            }
        }
    }
    black_box(sink)
}

#[inline(always)]
fn hs_long_del(mut built: HsLongBuilt, del: FhsDel) -> Word {
    let mut removed = 0 as Word;
    // SAFETY: the array was built in `setup` through the library `del`
    // belongs to; every probe is a present key.
    unsafe {
        for k in &built.probes {
            let kp = black_box(k.as_ptr()).cast_mut().cast();
            removed += del(&raw mut built.arr, kp, k.len() as Word, null_mut()) as Word;
        }
    }
    black_box(removed)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = hs_long_feed_expanse)]
fn judyhs_long_insert_expanse(f: HsLongFeed) -> Word {
    hs_long_insert(&f.ks, rlib_hs_ins)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = hs_long_feed_expanse_dl)]
fn judyhs_long_insert_expanse_dl(f: HsLongFeed) -> Word {
    hs_long_insert(&f.ks, f.api.expect("dl arm needs a resolved api").hs_ins)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = hs_long_feed_stock)]
fn judyhs_long_insert_stock(f: HsLongFeed) -> Word {
    hs_long_insert(&f.ks, f.api.expect("stock arm needs a resolved api").hs_ins)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = build_hs_long_get_expanse)]
fn judyhs_long_get_expanse(built: HsLongBuilt) -> Word {
    hs_long_get(&built, rlib_hs_get)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = build_hs_long_get_expanse_dl)]
fn judyhs_long_get_expanse_dl(built: HsLongBuilt) -> Word {
    hs_long_get(
        &built,
        built.api.expect("dl arm needs a resolved api").hs_get,
    )
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = build_hs_long_get_stock)]
fn judyhs_long_get_stock(built: HsLongBuilt) -> Word {
    hs_long_get(
        &built,
        built.api.expect("stock arm needs a resolved api").hs_get,
    )
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = build_hs_long_del_expanse)]
fn judyhs_long_del_expanse(built: HsLongBuilt) -> Word {
    hs_long_del(built, rlib_hs_del)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = build_hs_long_del_expanse_dl)]
fn judyhs_long_del_expanse_dl(built: HsLongBuilt) -> Word {
    let del = built.api.expect("dl arm needs a resolved api").hs_del;
    hs_long_del(built, del)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = build_hs_long_del_stock)]
fn judyhs_long_del_stock(built: HsLongBuilt) -> Word {
    let del = built.api.expect("stock arm needs a resolved api").hs_del;
    hs_long_del(built, del)
}

// ---- Judy1 as OQGraph's two-bit colour map (#1682) ---------------------
//
// One breadth-first search's Judy1 calls, recorded once in setup as a tape
// (`colour_map_common/mod.rs`) and replayed against two fresh arrays: tests,
// sets and unsets over vertex ids `0..n` in visit order, ending with both
// arrays freed. The tape is identical for every arm; `examples/colour_map.rs`
// times the same tape and is the `perf stat` workload.

/// A recorded colour-map tape plus the entry points to replay it through.
struct ColourTape {
    tape: Vec<u64>,
    j: colour_map_common::Judy1,
}

// SAFETY: fn pointers into a library that is never `dlclose`d; used only on
// the harness thread that built the struct.
unsafe impl Send for ColourTape {}

fn colour_pop(dist: &str) -> u64 {
    match dist {
        "n10k" => 10_000,
        "n100k" => 100_000,
        other => panic!("unknown colour-map population {other}"),
    }
}

fn colour_api(api: Api) -> colour_map_common::Judy1 {
    colour_map_common::Judy1 {
        test: api.test1,
        set: api.set1,
        unset: api.unset1,
        free: api.free1,
    }
}

unsafe extern "C" fn rlib_j1_test(p: *const c_void, k: Word, pj: *mut c_void) -> c_int {
    // SAFETY: forwarded unchanged.
    unsafe { expanse::Judy1Test(p, k, pj.cast()) }
}
unsafe extern "C" fn rlib_j1_set(p: *mut *mut c_void, k: Word, pj: *mut c_void) -> c_int {
    // SAFETY: forwarded unchanged.
    unsafe { expanse::Judy1Set(p, k, pj.cast()) }
}
unsafe extern "C" fn rlib_j1_unset(p: *mut *mut c_void, k: Word, pj: *mut c_void) -> c_int {
    // SAFETY: forwarded unchanged.
    unsafe { expanse::Judy1Unset(p, k, pj.cast()) }
}
unsafe extern "C" fn rlib_j1_free(p: *mut *mut c_void, pj: *mut c_void) -> Word {
    // SAFETY: forwarded unchanged.
    unsafe { expanse::Judy1FreeArray(p, pj.cast()) }
}

fn colour_expanse(dist: &str) -> ColourTape {
    ColourTape {
        tape: colour_map_common::tape(colour_pop(dist)),
        j: colour_map_common::Judy1 {
            test: rlib_j1_test,
            set: rlib_j1_set,
            unset: rlib_j1_unset,
            free: rlib_j1_free,
        },
    }
}
fn colour_expanse_dl(dist: &str) -> ColourTape {
    ColourTape {
        tape: colour_map_common::tape(colour_pop(dist)),
        j: colour_api(Lib::ours().api()),
    }
}
fn colour_stock(dist: &str) -> ColourTape {
    ColourTape {
        tape: colour_map_common::tape(colour_pop(dist)),
        j: colour_api(Lib::stock().api()),
    }
}

#[library_benchmark]
#[bench::n10k(args = ("n10k",), setup = colour_expanse)]
#[bench::n100k(args = ("n100k",), setup = colour_expanse)]
fn judy1_colour_expanse(c: ColourTape) -> Word {
    // SAFETY: `c.j` is one library's Judy1 entry points.
    black_box(unsafe { colour_map_common::replay(&c.tape, c.j) })
}

// Our own `libexpanse.so` — the arm to compare against `judy1_colour_stock`.
#[library_benchmark]
#[bench::n10k(args = ("n10k",), setup = colour_expanse_dl)]
#[bench::n100k(args = ("n100k",), setup = colour_expanse_dl)]
fn judy1_colour_expanse_dl(c: ColourTape) -> Word {
    // SAFETY: as above.
    black_box(unsafe { colour_map_common::replay(&c.tape, c.j) })
}

#[library_benchmark]
#[bench::n10k(args = ("n10k",), setup = colour_stock)]
#[bench::n100k(args = ("n100k",), setup = colour_stock)]
fn judy1_colour_stock(c: ColourTape) -> Word {
    // SAFETY: as above.
    black_box(unsafe { colour_map_common::replay(&c.tape, c.j) })
}

// ---- JudyL: the entry points beyond Ins / Get / churn (#1742) ----------
//
// Delete, count, by-count, the four neighbour searches, the four empty-index
// searches and `JudyLFreeArray` had no arm. Each family below measures one
// entry point over the `POP` array, and `judyl_small_array` measures whole
// lives of arrays holding 1 to 64 keys, the shape a hash table of JudyL
// buckets produces. Every arm reaches its library through one `JudyL` table
// of entry points, so the three arms of a family run one body.

type FlCount = unsafe extern "C" fn(*const c_void, Word, Word, *mut c_void) -> Word;
type FlByCount = unsafe extern "C" fn(*const c_void, Word, *mut Word, *mut c_void) -> *mut c_void;
type FlNav = unsafe extern "C" fn(*const c_void, *mut Word, *mut c_void) -> *mut c_void;
type FlEmpty = unsafe extern "C" fn(*const c_void, *mut Word, *mut c_void) -> c_int;
type FlMem = unsafe extern "C" fn(*const c_void) -> Word;

/// One library's JudyL entry points, resolved ahead of the measured region.
#[derive(Clone, Copy)]
struct JudyL {
    ins: FpIns,
    get: FpGet,
    del: FpDel,
    count: FlCount,
    by_count: FlByCount,
    first: FlNav,
    next: FlNav,
    last: FlNav,
    prev: FlNav,
    first_empty: FlEmpty,
    next_empty: FlEmpty,
    last_empty: FlEmpty,
    prev_empty: FlEmpty,
    free: FFree1,
    mem_used: FlMem,
}

impl Lib {
    fn judyl(&self) -> JudyL {
        JudyL {
            ins: self.sym(c"JudyLIns"),
            get: self.sym(c"JudyLGet"),
            del: self.sym(c"JudyLDel"),
            count: self.sym(c"JudyLCount"),
            by_count: self.sym(c"JudyLByCount"),
            first: self.sym(c"JudyLFirst"),
            next: self.sym(c"JudyLNext"),
            last: self.sym(c"JudyLLast"),
            prev: self.sym(c"JudyLPrev"),
            first_empty: self.sym(c"JudyLFirstEmpty"),
            next_empty: self.sym(c"JudyLNextEmpty"),
            last_empty: self.sym(c"JudyLLastEmpty"),
            prev_empty: self.sym(c"JudyLPrevEmpty"),
            free: self.sym(c"JudyLFreeArray"),
            mem_used: self.sym(c"JudyLMemUsed"),
        }
    }
}

unsafe extern "C" fn rlib_jl_ins(p: *mut *mut c_void, k: Word, pj: *mut c_void) -> *mut c_void {
    // SAFETY: forwarded unchanged.
    unsafe { expanse::JudyLIns(p, k, pj.cast()) }
}
unsafe extern "C" fn rlib_jl_get(p: *const c_void, k: Word, pj: *mut c_void) -> *mut c_void {
    // SAFETY: forwarded unchanged.
    unsafe { expanse::JudyLGet(p, k, pj.cast()) }
}
unsafe extern "C" fn rlib_jl_del(p: *mut *mut c_void, k: Word, pj: *mut c_void) -> c_int {
    // SAFETY: forwarded unchanged.
    unsafe { expanse::JudyLDel(p, k, pj.cast()) }
}
unsafe extern "C" fn rlib_jl_count(p: *const c_void, a: Word, b: Word, pj: *mut c_void) -> Word {
    // SAFETY: forwarded unchanged.
    unsafe { expanse::JudyLCount(p, a, b, pj.cast()) }
}
unsafe extern "C" fn rlib_jl_by_count(
    p: *const c_void,
    nth: Word,
    pi: *mut Word,
    pj: *mut c_void,
) -> *mut c_void {
    // SAFETY: forwarded unchanged.
    unsafe { expanse::JudyLByCount(p, nth, pi, pj.cast()) }
}
unsafe extern "C" fn rlib_jl_free(p: *mut *mut c_void, pj: *mut c_void) -> Word {
    // SAFETY: forwarded unchanged.
    unsafe { expanse::JudyLFreeArray(p, pj.cast()) }
}
unsafe extern "C" fn rlib_jl_mem_used(p: *const c_void) -> Word {
    // SAFETY: forwarded unchanged.
    unsafe { expanse::JudyLMemUsed(p) }
}

macro_rules! rlib_jl_search {
    ($($wrapper:ident => $entry:ident -> $ret:ty;)*) => {$(
        unsafe extern "C" fn $wrapper(p: *const c_void, pi: *mut Word, pj: *mut c_void) -> $ret {
            // SAFETY: forwarded unchanged.
            unsafe { expanse::$entry(p, pi, pj.cast()) }
        }
    )*};
}
rlib_jl_search! {
    rlib_jl_first => JudyLFirst -> *mut c_void;
    rlib_jl_next => JudyLNext -> *mut c_void;
    rlib_jl_last => JudyLLast -> *mut c_void;
    rlib_jl_prev => JudyLPrev -> *mut c_void;
    rlib_jl_first_empty => JudyLFirstEmpty -> c_int;
    rlib_jl_next_empty => JudyLNextEmpty -> c_int;
    rlib_jl_last_empty => JudyLLastEmpty -> c_int;
    rlib_jl_prev_empty => JudyLPrevEmpty -> c_int;
}

/// The rlib's entry points, linked into the harness.
fn judyl_rlib() -> JudyL {
    JudyL {
        ins: rlib_jl_ins,
        get: rlib_jl_get,
        del: rlib_jl_del,
        count: rlib_jl_count,
        by_count: rlib_jl_by_count,
        first: rlib_jl_first,
        next: rlib_jl_next,
        last: rlib_jl_last,
        prev: rlib_jl_prev,
        first_empty: rlib_jl_first_empty,
        next_empty: rlib_jl_next_empty,
        last_empty: rlib_jl_last_empty,
        prev_empty: rlib_jl_prev_empty,
        free: rlib_jl_free,
        mem_used: rlib_jl_mem_used,
    }
}

/// A `POP` array built through `j`, its shuffled probe order, and `j`.
struct JlBuilt {
    arr: *mut c_void,
    probes: Vec<Word>,
    j: JudyL,
}

// SAFETY: the array is created and consumed on the harness thread; the fn
// pointers lead into a library that is never `dlclose`d.
unsafe impl Send for JlBuilt {}

fn jl_build(dist: &str, j: JudyL) -> JlBuilt {
    let ks = keys(dist);
    let probes = shuffled(ks.clone());
    let mut arr: *mut c_void = null_mut();
    // SAFETY: standard JudyL usage; the slot is written before the next call.
    unsafe {
        for &k in &ks {
            *(j.ins)(&raw mut arr, k, null_mut()).cast::<Word>() = k;
        }
    }
    JlBuilt { arr, probes, j }
}
fn jl_expanse(dist: &str) -> JlBuilt {
    jl_build(dist, judyl_rlib())
}
fn jl_expanse_dl(dist: &str) -> JlBuilt {
    jl_build(dist, Lib::ours().judyl())
}
fn jl_stock(dist: &str) -> JlBuilt {
    jl_build(dist, Lib::stock().judyl())
}

/// Deletes every key in probe order; the last delete empties the array.
fn jl_del(mut b: JlBuilt) -> Word {
    let mut sink = 0usize;
    // SAFETY: `b.arr` was built through `b.j`.
    unsafe {
        for &k in &b.probes {
            sink += (b.j.del)(&raw mut b.arr, black_box(k), null_mut()) as Word;
        }
    }
    black_box(sink ^ map_len_sentinel(b.arr))
}

/// One `JudyLCount` per probe, over the range between it and the next probe.
fn jl_count(b: &JlBuilt) -> Word {
    let n = b.probes.len();
    let mut sink = 0usize;
    // SAFETY: `b.arr` was built through `b.j`.
    unsafe {
        for i in 0..n {
            let (x, y) = (b.probes[i], b.probes[(i + 1) % n]);
            sink ^= (b.j.count)(b.arr, black_box(x.min(y)), black_box(x.max(y)), null_mut());
        }
    }
    black_box(sink)
}

/// One `JudyLByCount` per probe, at a rank the probe selects.
fn jl_by_count(b: &JlBuilt) -> Word {
    let n = b.probes.len();
    let mut sink = 0usize;
    // SAFETY: `b.arr` was built through `b.j`; a returned slot is live until
    // the next mutation.
    unsafe {
        for &k in &b.probes {
            let mut idx: Word = 0;
            let nth = black_box(k % n + 1);
            let slot = (b.j.by_count)(b.arr, nth, &raw mut idx, null_mut()).cast::<Word>();
            if !slot.is_null() {
                sink ^= idx ^ *slot;
            }
        }
    }
    black_box(sink)
}

/// One neighbour search per probe, from the index one step past the probe:
/// `step` is `1` for `JudyLFirst` and `-1` for `JudyLLast`.
fn jl_seek(b: &JlBuilt, seek: FlNav, step: isize) -> Word {
    let mut sink = 0usize;
    // SAFETY: `b.arr` was built through `b.j`; a returned slot is live until
    // the next mutation.
    unsafe {
        for &k in &b.probes {
            let mut idx = black_box(k.wrapping_add_signed(step));
            let slot = seek(b.arr, &raw mut idx, null_mut()).cast::<Word>();
            if !slot.is_null() {
                sink ^= idx ^ *slot;
            }
        }
    }
    black_box(sink)
}

/// One whole traversal: `start` from `from`, then `advance` until it misses.
fn jl_walk(b: &JlBuilt, start: FlNav, advance: FlNav, from: Word) -> Word {
    let mut sink = 0usize;
    let mut idx = from;
    // SAFETY: `b.arr` was built through `b.j`; a returned slot is live until
    // the next mutation.
    unsafe {
        let mut slot = start(b.arr, &raw mut idx, null_mut()).cast::<Word>();
        while !slot.is_null() {
            sink ^= idx ^ *slot;
            slot = advance(b.arr, &raw mut idx, null_mut()).cast::<Word>();
        }
    }
    black_box(sink)
}

/// Every `EMPTY_STRIDE`th probe runs an empty-index search: 469 of `POP`.
/// A search from inside a 256-key run costs libexpanse about 418,000
/// instructions on x86-64 (#1749), so one per probe would be over 12
/// billion per arm.
const EMPTY_STRIDE: usize = 64;

/// One empty-index search per sampled probe, starting at the probe (a
/// present key inside a run of 256).
fn jl_empty(b: &JlBuilt, search: FlEmpty) -> Word {
    let mut sink = 0usize;
    // SAFETY: `b.arr` was built through `b.j`.
    unsafe {
        for &k in b.probes.iter().step_by(EMPTY_STRIDE) {
            let mut idx = black_box(k);
            sink ^= search(b.arr, &raw mut idx, null_mut()) as Word ^ idx;
        }
    }
    black_box(sink)
}

/// `JudyLFreeArray` of the whole `POP` array, once.
fn jl_free(mut b: JlBuilt) -> Word {
    // SAFETY: `b.arr` was built through `b.j` and is not used again.
    black_box(unsafe { (b.j.free)(&raw mut b.arr, null_mut()) })
}

/// Arrays per `judyl_small_array` arm.
const SMALL_ARRAYS: usize = 1_000;

/// Keys and absent probes for `SMALL_ARRAYS` arrays of `k` keys each.
struct JlSmall {
    k: usize,
    keys: Vec<Word>,
    misses: Vec<Word>,
    j: JudyL,
}

// SAFETY: fn pointers into a library that is never `dlclose`d; used only on
// the harness thread that built the struct.
unsafe impl Send for JlSmall {}

fn jl_small_feed(k: usize, j: JudyL) -> JlSmall {
    let mut rng = XorShift(0x5EED_0A11_A22A_1742);
    let keys: Vec<Word> = (0..SMALL_ARRAYS * k).map(|_| rng.next() as Word).collect();
    let present: std::collections::HashSet<Word> = keys.iter().copied().collect();
    let mut misses = Vec::with_capacity(keys.len());
    // The same generator, rejected on membership.
    while misses.len() < keys.len() {
        let m = rng.next() as Word;
        if !present.contains(&m) {
            misses.push(m);
        }
    }
    JlSmall { k, keys, misses, j }
}
fn jl_small_expanse(k: usize) -> JlSmall {
    jl_small_feed(k, judyl_rlib())
}
fn jl_small_expanse_dl(k: usize) -> JlSmall {
    jl_small_feed(k, Lib::ours().judyl())
}
fn jl_small_stock(k: usize) -> JlSmall {
    jl_small_feed(k, Lib::stock().judyl())
}

/// `SMALL_ARRAYS` whole array lives of `k` keys: insert `k`, look each up,
/// probe `k` absent keys, traverse, count, take the median by rank, read
/// `JudyLMemUsed`, then delete every key (even arrays, which the last delete
/// empties) or half of them followed by `JudyLFreeArray` (odd arrays).
/// Creation and teardown are the work measured, so nothing is leaked.
fn jl_small(s: &JlSmall) -> Word {
    let j = s.j;
    let mut sink = 0usize;
    let lives = s.keys.chunks_exact(s.k).zip(s.misses.chunks_exact(s.k));
    // SAFETY: standard JudyL usage; each slot is used before the next call
    // that mutates its array.
    unsafe {
        for (a, (ks, ms)) in lives.enumerate() {
            let mut arr: *mut c_void = null_mut();
            for &k in ks {
                *(j.ins)(&raw mut arr, black_box(k), null_mut()).cast::<Word>() = k;
            }
            for &k in ks {
                sink ^= *(j.get)(arr, black_box(k), null_mut()).cast::<Word>();
            }
            for &m in ms {
                sink += (j.get)(arr, black_box(m), null_mut()).is_null() as Word;
            }
            let mut idx: Word = 0;
            let mut slot = (j.first)(arr, &raw mut idx, null_mut()).cast::<Word>();
            while !slot.is_null() {
                sink ^= idx ^ *slot;
                slot = (j.next)(arr, &raw mut idx, null_mut()).cast::<Word>();
            }
            sink ^= (j.count)(arr, 0, Word::MAX, null_mut());
            let mut median: Word = 0;
            if !(j.by_count)(arr, s.k / 2 + 1, &raw mut median, null_mut()).is_null() {
                sink ^= median;
            }
            black_box((j.mem_used)(arr));
            let deleted = if a % 2 == 0 { s.k } else { s.k / 2 };
            for &k in ks[..deleted].iter().rev() {
                sink += (j.del)(&raw mut arr, black_box(k), null_mut()) as Word;
            }
            sink += ((j.free)(&raw mut arr, null_mut()) != 0) as Word;
        }
    }
    black_box(sink)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_expanse)]
#[bench::clustered(args = ("clustered",), setup = jl_expanse)]
fn judyl_del_expanse(b: JlBuilt) -> Word {
    jl_del(b)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_expanse_dl)]
#[bench::clustered(args = ("clustered",), setup = jl_expanse_dl)]
fn judyl_del_expanse_dl(b: JlBuilt) -> Word {
    jl_del(b)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_stock)]
#[bench::clustered(args = ("clustered",), setup = jl_stock)]
fn judyl_del_stock(b: JlBuilt) -> Word {
    jl_del(b)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_expanse)]
fn judyl_count_expanse(b: JlBuilt) -> Word {
    jl_count(&b)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_expanse_dl)]
fn judyl_count_expanse_dl(b: JlBuilt) -> Word {
    jl_count(&b)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_stock)]
fn judyl_count_stock(b: JlBuilt) -> Word {
    jl_count(&b)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_expanse)]
fn judyl_by_count_expanse(b: JlBuilt) -> Word {
    jl_by_count(&b)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_expanse_dl)]
fn judyl_by_count_expanse_dl(b: JlBuilt) -> Word {
    jl_by_count(&b)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_stock)]
fn judyl_by_count_stock(b: JlBuilt) -> Word {
    jl_by_count(&b)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_expanse)]
fn judyl_first_expanse(b: JlBuilt) -> Word {
    jl_seek(&b, b.j.first, 1)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_expanse_dl)]
fn judyl_first_expanse_dl(b: JlBuilt) -> Word {
    jl_seek(&b, b.j.first, 1)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_stock)]
fn judyl_first_stock(b: JlBuilt) -> Word {
    jl_seek(&b, b.j.first, 1)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_expanse)]
fn judyl_last_expanse(b: JlBuilt) -> Word {
    jl_seek(&b, b.j.last, -1)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_expanse_dl)]
fn judyl_last_expanse_dl(b: JlBuilt) -> Word {
    jl_seek(&b, b.j.last, -1)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_stock)]
fn judyl_last_stock(b: JlBuilt) -> Word {
    jl_seek(&b, b.j.last, -1)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_expanse)]
fn judyl_next_expanse(b: JlBuilt) -> Word {
    jl_walk(&b, b.j.first, b.j.next, 0)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_expanse_dl)]
fn judyl_next_expanse_dl(b: JlBuilt) -> Word {
    jl_walk(&b, b.j.first, b.j.next, 0)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_stock)]
fn judyl_next_stock(b: JlBuilt) -> Word {
    jl_walk(&b, b.j.first, b.j.next, 0)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_expanse)]
fn judyl_prev_expanse(b: JlBuilt) -> Word {
    jl_walk(&b, b.j.last, b.j.prev, Word::MAX)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_expanse_dl)]
fn judyl_prev_expanse_dl(b: JlBuilt) -> Word {
    jl_walk(&b, b.j.last, b.j.prev, Word::MAX)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_stock)]
fn judyl_prev_stock(b: JlBuilt) -> Word {
    jl_walk(&b, b.j.last, b.j.prev, Word::MAX)
}

#[library_benchmark]
#[bench::clustered(args = ("clustered",), setup = jl_expanse)]
fn judyl_first_empty_expanse(b: JlBuilt) -> Word {
    jl_empty(&b, b.j.first_empty)
}

#[library_benchmark]
#[bench::clustered(args = ("clustered",), setup = jl_expanse_dl)]
fn judyl_first_empty_expanse_dl(b: JlBuilt) -> Word {
    jl_empty(&b, b.j.first_empty)
}

#[library_benchmark]
#[bench::clustered(args = ("clustered",), setup = jl_stock)]
fn judyl_first_empty_stock(b: JlBuilt) -> Word {
    jl_empty(&b, b.j.first_empty)
}

#[library_benchmark]
#[bench::clustered(args = ("clustered",), setup = jl_expanse)]
fn judyl_next_empty_expanse(b: JlBuilt) -> Word {
    jl_empty(&b, b.j.next_empty)
}

#[library_benchmark]
#[bench::clustered(args = ("clustered",), setup = jl_expanse_dl)]
fn judyl_next_empty_expanse_dl(b: JlBuilt) -> Word {
    jl_empty(&b, b.j.next_empty)
}

#[library_benchmark]
#[bench::clustered(args = ("clustered",), setup = jl_stock)]
fn judyl_next_empty_stock(b: JlBuilt) -> Word {
    jl_empty(&b, b.j.next_empty)
}

#[library_benchmark]
#[bench::clustered(args = ("clustered",), setup = jl_expanse)]
fn judyl_last_empty_expanse(b: JlBuilt) -> Word {
    jl_empty(&b, b.j.last_empty)
}

#[library_benchmark]
#[bench::clustered(args = ("clustered",), setup = jl_expanse_dl)]
fn judyl_last_empty_expanse_dl(b: JlBuilt) -> Word {
    jl_empty(&b, b.j.last_empty)
}

#[library_benchmark]
#[bench::clustered(args = ("clustered",), setup = jl_stock)]
fn judyl_last_empty_stock(b: JlBuilt) -> Word {
    jl_empty(&b, b.j.last_empty)
}

#[library_benchmark]
#[bench::clustered(args = ("clustered",), setup = jl_expanse)]
fn judyl_prev_empty_expanse(b: JlBuilt) -> Word {
    jl_empty(&b, b.j.prev_empty)
}

#[library_benchmark]
#[bench::clustered(args = ("clustered",), setup = jl_expanse_dl)]
fn judyl_prev_empty_expanse_dl(b: JlBuilt) -> Word {
    jl_empty(&b, b.j.prev_empty)
}

#[library_benchmark]
#[bench::clustered(args = ("clustered",), setup = jl_stock)]
fn judyl_prev_empty_stock(b: JlBuilt) -> Word {
    jl_empty(&b, b.j.prev_empty)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_expanse)]
fn judyl_free_expanse(b: JlBuilt) -> Word {
    jl_free(b)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_expanse_dl)]
fn judyl_free_expanse_dl(b: JlBuilt) -> Word {
    jl_free(b)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = jl_stock)]
fn judyl_free_stock(b: JlBuilt) -> Word {
    jl_free(b)
}

#[library_benchmark]
#[bench::k1(args = (1,), setup = jl_small_expanse)]
#[bench::k10(args = (10,), setup = jl_small_expanse)]
#[bench::k20(args = (20,), setup = jl_small_expanse)]
#[bench::k31(args = (31,), setup = jl_small_expanse)]
#[bench::k32(args = (32,), setup = jl_small_expanse)]
#[bench::k64(args = (64,), setup = jl_small_expanse)]
fn judyl_small_array_expanse(s: JlSmall) -> Word {
    jl_small(&s)
}

#[library_benchmark]
#[bench::k1(args = (1,), setup = jl_small_expanse_dl)]
#[bench::k10(args = (10,), setup = jl_small_expanse_dl)]
#[bench::k20(args = (20,), setup = jl_small_expanse_dl)]
#[bench::k31(args = (31,), setup = jl_small_expanse_dl)]
#[bench::k32(args = (32,), setup = jl_small_expanse_dl)]
#[bench::k64(args = (64,), setup = jl_small_expanse_dl)]
fn judyl_small_array_expanse_dl(s: JlSmall) -> Word {
    jl_small(&s)
}

#[library_benchmark]
#[bench::k1(args = (1,), setup = jl_small_stock)]
#[bench::k10(args = (10,), setup = jl_small_stock)]
#[bench::k20(args = (20,), setup = jl_small_stock)]
#[bench::k31(args = (31,), setup = jl_small_stock)]
#[bench::k32(args = (32,), setup = jl_small_stock)]
#[bench::k64(args = (64,), setup = jl_small_stock)]
fn judyl_small_array_stock(s: JlSmall) -> Word {
    jl_small(&s)
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
        judy1_test_stock,
        judysl_insert_expanse,
        judysl_insert_expanse_dl,
        judysl_insert_stock,
        judysl_get_expanse,
        judysl_get_expanse_dl,
        judysl_get_stock,
        judyhs_insert_expanse,
        judyhs_insert_expanse_dl,
        judyhs_insert_stock,
        judyhs_get_expanse,
        judyhs_get_expanse_dl,
        judyhs_get_stock,
        judyhs_del_expanse,
        judyhs_del_expanse_dl,
        judyhs_del_stock,
        judyhs_long_insert_expanse,
        judyhs_long_insert_expanse_dl,
        judyhs_long_insert_stock,
        judyhs_long_get_expanse,
        judyhs_long_get_expanse_dl,
        judyhs_long_get_stock,
        judyhs_long_del_expanse,
        judyhs_long_del_expanse_dl,
        judyhs_long_del_stock,
        judy1_colour_expanse,
        judy1_colour_expanse_dl,
        judy1_colour_stock,
        judyl_del_expanse,
        judyl_del_expanse_dl,
        judyl_del_stock,
        judyl_count_expanse,
        judyl_count_expanse_dl,
        judyl_count_stock,
        judyl_by_count_expanse,
        judyl_by_count_expanse_dl,
        judyl_by_count_stock,
        judyl_first_expanse,
        judyl_first_expanse_dl,
        judyl_first_stock,
        judyl_last_expanse,
        judyl_last_expanse_dl,
        judyl_last_stock,
        judyl_next_expanse,
        judyl_next_expanse_dl,
        judyl_next_stock,
        judyl_prev_expanse,
        judyl_prev_expanse_dl,
        judyl_prev_stock,
        judyl_first_empty_expanse,
        judyl_first_empty_expanse_dl,
        judyl_first_empty_stock,
        judyl_next_empty_expanse,
        judyl_next_empty_expanse_dl,
        judyl_next_empty_stock,
        judyl_last_empty_expanse,
        judyl_last_empty_expanse_dl,
        judyl_last_empty_stock,
        judyl_prev_empty_expanse,
        judyl_prev_empty_expanse_dl,
        judyl_prev_empty_stock,
        judyl_free_expanse,
        judyl_free_expanse_dl,
        judyl_free_stock,
        judyl_small_array_expanse,
        judyl_small_array_expanse_dl,
        judyl_small_array_stock
);

#[cfg(target_os = "linux")]
main!(config = bench_config(); library_benchmark_groups = vs_stock);

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("iai-callgrind vs_stock benchmarks run on Linux only.");
}
