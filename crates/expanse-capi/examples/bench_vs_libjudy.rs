//! **libexpanse vs stock libjudy in wall-clock, on the JudyL surface.**
//!
//! `benches/vs_stock.rs` answers the same question in instructions
//! retired, exactly and reproducibly. This harness answers it in the
//! unit users actually feel, which no available environment resolves
//! below ~15-20% (`docs/BENCHMARKING.md`) — so every number below is a
//! *paired ratio with an interval*, never a bare nanosecond count.
//!
//! Run (needs stock libjudy: `libjudy-dev` on Linux, `brew install judy`
//! on macOS):
//! `cargo run --release -p expanse-capi --example bench_vs_libjudy`
//! `--rounds N` overrides the round count (minimum 3; default
//! `DEFAULT_ROUNDS`).
//!
//! # Workload shape — what is actually measured
//!
//! None of this is inferable from the output table, and the numbers this
//! harness produced before [#453] were published as if the shape were
//! something else, so it is stated in full:
//!
//! [#453]: https://github.com/orieg/expanse/issues/453
//!
//! | property | value |
//! |---|---|
//! | populations | 100,000 and 1,000,000 keys, three distributions |
//! | value written per key | `!key`, one `Word` in the JudyL value slot |
//! | distinct probe keys | `2 × population` — every populated key once, plus an equal number of distinct absent keys |
//! | probe reuse factor | **1.0** — each probe key is looked up exactly once per timed pass |
//! | hit rate | **50%** (AGENTS.md §8.6) |
//! | probe order | Fisher-Yates shuffled, so hits and misses interleave and the order differs from insert order |
//! | dereferenced | **yes** — the returned slot is read and the *value* is accumulated; a null (miss) return is skipped |
//! | timed region | the insert loop and the probe loop only; key generation, miss generation, shuffling, `MemUsed` and `JudyLFreeArray` are all outside it |
//!
//! The probe set is sized from the population on purpose. It used to be
//! 4,096 keys sampled *with replacement* and walked eight times: ~4,096
//! distinct root-to-leaf paths, a couple of MiB, entirely resident in a
//! 30 MiB last-level cache. The resulting `get` ratio was published as a
//! memory-latency-bound comparison and was nothing of the kind. Probing
//! `2 × population` distinct keys makes the timed footprint span the
//! structure, which is the only shape under which the phrase
//! "memory-latency-bound" is honest.
//!
//! Misses are drawn from the **same generator as the keys** and rejected
//! if present, so they descend the same expanse the hits do. They are
//! deliberately *not* a fixed transform of a hit key (`k ^ (1<<63)`, or
//! similar): such a probe lands in a region of the key space the trie
//! never populated and is answered by a single top-level branch test,
//! which measures the shape of the transform rather than the miss path.
//!
//! # Three arms, and the middle one is the honest comparison
//!
//! Same reasoning as `benches/vs_stock.rs`, which fixed this for the
//! instruction-count harness and left this one un-corrected:
//!
//! - `expanse_rlib` — our rlib, linked straight into this binary with
//!   LTO and called directly. **The fastest shape our code has and the
//!   one no `libexpanse` user gets.** Reported for the correction factor
//!   it gives against the row below it; not a comparison against stock.
//! - `expanse_dl` — our own `libexpanse.{so,dylib}`, `dlopen`'d and
//!   called through resolved symbols, i.e. exactly how stock is reached
//!   and exactly how a drop-in consumer reaches us. **Compare this row
//!   against stock.** Set `EXPANSE_CDYLIB` to the built artifact; if it
//!   is unset the usual `target/{release,debug}` paths are tried, and if
//!   none resolves the run aborts rather than quietly dropping the arm.
//! - `stock` — stock libjudy, `dlopen`'d, the ratio denominator.
//!
//! Symbols for both dynamic arms are resolved before any timed region,
//! so no arm measures its own dynamic linking.
//!
//! # Statistics
//!
//! Arm order rotates every round so machine drift lands on each arm
//! equally. The pairing is then **kept**: a ratio is formed inside each
//! round, and the reported figure is the mean of those per-round paired
//! ratios with a BCa 95% bootstrap confidence interval over them
//! (AGENTS.md §8.4). Taking a ratio of two independently-taken medians,
//! as this harness used to, discards the very pairing the interleaving
//! was there to buy.
//!
//! The per-round ratios are written as JSON (`EXPANSE_BENCH_RATIOS_JSON`,
//! default `target/bench/vs_libjudy_paired_ratios.json` — under the
//! gitignored build directory, per AGENTS.md §8.5) with each series'
//! `ratios` array in the exact shape `scripts/bca_bootstrap.py`'s
//! `bca_bootstrap_ci` takes, so the interval printed here can be
//! re-derived independently:
//!
//! ```python
//! import json, sys; sys.path.insert(0, "scripts")
//! from bca_bootstrap import bca_bootstrap_ci
//! for s in json.load(open("target/bench/vs_libjudy_paired_ratios.json"))["series"]:
//!     print(s["id"], bca_bootstrap_ci(s["ratios"]))
//! ```
//!
//! # What the output may and may not be used to claim
//!
//! **May:** "on <named host, named commit>, libexpanse reached through
//! its shared object retires a `get` in X× the wall-clock of stock
//! libjudy on this distribution, 95% CI [lo, hi], at 50% hit rate over a
//! probe set spanning the whole structure." Ratios transfer between
//! machines far better than nanoseconds do; the paired arms normalize
//! machine speed away.
//!
//! **May not:** be quoted as an absolute nanosecond figure without the
//! host and commit; be quoted from the `expanse_rlib` row as a
//! comparison against stock; be read as a statement about any hit rate,
//! population, or working-set size other than the ones in the table
//! above; be treated as significant when the interval spans 1.00.
//! `bytes/key` is the one exception: it is `JudyLMemUsed` accounting,
//! deterministic, and reproduces byte-for-byte, so it carries no
//! interval.

#[cfg(not(unix))]
fn main() {
    eprintln!("bench_vs_libjudy needs a dlopen platform (Linux/macOS) with stock libjudy");
    std::process::exit(1);
}

#[cfg(unix)]
fn main() {
    bench::main();
}

/// The whole harness. Gated as one unit so the non-`unix` build has a
/// single trivial `main` instead of a per-item `cfg` thicket.
#[cfg(unix)]
mod bench {
    use core::ffi::{CStr, c_int, c_void};
    use core::ptr::null_mut;
    use std::collections::HashSet;
    use std::fmt::Write as _;
    use std::io::Write as _;
    use std::time::Instant;

    /// Machine word, matching the JudyL C ABI's `Word_t`.
    type Word = usize;

    unsafe extern "C" {
        fn dlopen(filename: *const u8, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const u8) -> *mut c_void;
    }

    const RTLD_NOW: c_int = 2;
    /// Keep a loaded library's symbols out of the global namespace: stock
    /// libjudy, our own cdylib and this binary all export the same names.
    /// macOS numbers the flag differently from Linux, and its default when
    /// neither flag is given is `RTLD_GLOBAL`, so it must be explicit.
    #[cfg(target_os = "macos")]
    const RTLD_LOCAL: c_int = 4;
    #[cfg(not(target_os = "macos"))]
    const RTLD_LOCAL: c_int = 0;

    type FIns = unsafe extern "C" fn(*mut *mut c_void, Word, *mut c_void) -> *mut c_void;
    type FGet = unsafe extern "C" fn(*const c_void, Word, *mut c_void) -> *mut c_void;
    type FFree = unsafe extern "C" fn(*mut *mut c_void, *mut c_void) -> Word;
    type FMem = unsafe extern "C" fn(*const c_void) -> Word;

    /// Rounds per cell. Well above the 3 BCa needs, because a bootstrap
    /// over 5 paired ratios produces an interval whose endpoints are two
    /// of the five observations.
    pub const DEFAULT_ROUNDS: usize = 15;
    /// Bootstrap resamples; matches `scripts/bca_bootstrap.py`'s default.
    const RESAMPLES: usize = 2000;
    /// Populations swept per distribution.
    const POPULATIONS: [usize; 2] = [100_000, 1_000_000];
    /// Distributions swept.
    const DISTRIBUTIONS: [&str; 3] = ["sequential", "random", "clustered"];
    /// Fraction of probes that hit. Fixed at one absent key per present
    /// key (AGENTS.md §8.6).
    const HIT_RATE: f64 = 0.5;

    // ---- library loading -------------------------------------------------

    /// A privately loaded Judy-ABI shared object.
    struct Lib(*mut c_void);

    /// Where stock libjudy might live, in preference order.
    const STOCK_NAMES: &[&CStr] = &[
        c"libJudy.so.1",
        c"libJudy.so",
        c"/opt/homebrew/opt/judy/lib/libJudy.dylib",
        c"/usr/local/opt/judy/lib/libJudy.dylib",
        c"libJudy.dylib",
    ];

    /// Where our own cdylib lands for a `cargo run` from the workspace
    /// root. `EXPANSE_CDYLIB` takes precedence and is what CI sets, since
    /// only the build knows the profile directory.
    const OURS_NAMES: &[&CStr] = &[
        c"target/release/libexpanse.so",
        c"target/release/libexpanse.dylib",
        c"target/debug/libexpanse.so",
        c"target/debug/libexpanse.dylib",
    ];

    impl Lib {
        /// Opens the first candidate that resolves, or aborts. There is no
        /// silent-skip path: a missing library means the comparison this
        /// harness exists to make cannot be made (AGENTS.md §8.1).
        fn open_any(candidates: &[&CStr], what: &str) -> Self {
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
            Self::open_any(
                STOCK_NAMES,
                "stock libjudy — install libjudy-dev (Linux) or `brew install judy` (macOS)",
            )
        }

        /// Our own cdylib, loaded exactly the way stock is.
        fn ours() -> Self {
            if let Ok(p) = std::env::var("EXPANSE_CDYLIB") {
                let c =
                    std::ffi::CString::new(p.clone()).expect("EXPANSE_CDYLIB has an interior NUL");
                // SAFETY: valid NUL-terminated library name.
                let h = unsafe { dlopen(c.to_bytes_with_nul().as_ptr(), RTLD_NOW | RTLD_LOCAL) };
                assert!(!h.is_null(), "EXPANSE_CDYLIB set to {p} but dlopen failed");
                return Self(h);
            }
            Self::open_any(
                OURS_NAMES,
                "libexpanse cdylib — build it (`cargo build --release -p expanse-capi`) \
                 and point EXPANSE_CDYLIB at it",
            )
        }

        fn sym<T: Copy>(&self, name: &CStr) -> T {
            // SAFETY: live handle and NUL-terminated name; the caller names
            // the fn-pointer type matching the symbol.
            let p = unsafe { dlsym(self.0, name.to_bytes_with_nul().as_ptr()) };
            assert!(!p.is_null(), "missing symbol {name:?}");
            // SAFETY: fn-pointer transmute of a resolved symbol.
            unsafe { core::mem::transmute_copy(&p) }
        }

        /// Binds every entry point once, ahead of any timed region.
        fn api(&self) -> Api {
            Api {
                ins: self.sym(c"JudyLIns"),
                get: self.sym(c"JudyLGet"),
                free: self.sym(c"JudyLFreeArray"),
                mem: self.sym(c"JudyLMemUsed"),
            }
        }
    }

    /// Entry points resolved before measurement starts.
    #[derive(Clone, Copy)]
    struct Api {
        ins: FIns,
        get: FGet,
        free: FFree,
        mem: FMem,
    }

    // ---- arms ------------------------------------------------------------

    /// The four JudyL entry points a measured arm needs. Implemented twice
    /// so the linked-rlib arm keeps its direct, inlinable calls instead of
    /// being flattened to an indirect call by the abstraction.
    trait JudyL {
        /// Inserts `k` if absent and returns its value slot.
        ///
        /// # Safety
        /// `arr` must point to a live JudyL root owned by the caller.
        unsafe fn ins(&self, arr: *mut *mut c_void, k: Word) -> *mut Word;
        /// Returns `k`'s value slot, or null when absent.
        ///
        /// # Safety
        /// `arr` must be a live JudyL array built through this same arm.
        unsafe fn get(&self, arr: *const c_void, k: Word) -> *mut Word;
        /// Allocator-accounted bytes held by the array.
        ///
        /// # Safety
        /// `arr` must be a live JudyL array built through this same arm.
        unsafe fn mem(&self, arr: *const c_void) -> Word;
        /// Frees the array and nulls the root.
        ///
        /// # Safety
        /// `arr` must point to a live root built through this same arm.
        unsafe fn free(&self, arr: *mut *mut c_void);
    }

    /// The statically linked rlib: direct calls, LTO across the boundary.
    /// Not a shape any consumer of `libexpanse` gets — see the module
    /// header.
    struct Rlib;

    impl JudyL for Rlib {
        #[inline(always)]
        unsafe fn ins(&self, arr: *mut *mut c_void, k: Word) -> *mut Word {
            // SAFETY: standard JudyL usage; caller owns the live root.
            unsafe { expanse::JudyLIns(arr, k, null_mut()) }.cast::<Word>()
        }
        #[inline(always)]
        unsafe fn get(&self, arr: *const c_void, k: Word) -> *mut Word {
            // SAFETY: array built through this same arm.
            unsafe { expanse::JudyLGet(arr, k, null_mut()) }.cast::<Word>()
        }
        #[inline(always)]
        unsafe fn mem(&self, arr: *const c_void) -> Word {
            // SAFETY: array built through this same arm.
            unsafe { expanse::JudyLMemUsed(arr) }
        }
        #[inline(always)]
        unsafe fn free(&self, arr: *mut *mut c_void) {
            // SAFETY: array built through this same arm; root is nulled.
            unsafe { expanse::JudyLFreeArray(arr, null_mut()) };
        }
    }

    /// A `dlopen`'d Judy-ABI library, reached through resolved symbols —
    /// stock libjudy, or our own cdylib on identical terms.
    struct Dl(Api);

    impl JudyL for Dl {
        #[inline(always)]
        unsafe fn ins(&self, arr: *mut *mut c_void, k: Word) -> *mut Word {
            // SAFETY: standard JudyL usage; caller owns the live root.
            unsafe { (self.0.ins)(arr, k, null_mut()) }.cast::<Word>()
        }
        #[inline(always)]
        unsafe fn get(&self, arr: *const c_void, k: Word) -> *mut Word {
            // SAFETY: array built through this same library.
            unsafe { (self.0.get)(arr, k, null_mut()) }.cast::<Word>()
        }
        #[inline(always)]
        unsafe fn mem(&self, arr: *const c_void) -> Word {
            // SAFETY: array built through this same library.
            unsafe { (self.0.mem)(arr) }
        }
        #[inline(always)]
        unsafe fn free(&self, arr: *mut *mut c_void) {
            // SAFETY: array built through this same library; root is nulled.
            unsafe { (self.0.free)(arr, null_mut()) };
        }
    }

    /// The three arms, in table order.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum ArmId {
        /// Linked rlib — the non-representative shape.
        Rlib,
        /// Our cdylib, `dlopen`'d — the arm to compare against stock.
        Dl,
        /// Stock libjudy, `dlopen`'d — the ratio denominator.
        Stock,
    }

    impl ArmId {
        const ALL: [ArmId; 3] = [ArmId::Rlib, ArmId::Dl, ArmId::Stock];

        fn label(self) -> &'static str {
            match self {
                ArmId::Rlib => "expanse_rlib",
                ArmId::Dl => "expanse_dl",
                ArmId::Stock => "stock",
            }
        }

        fn index(self) -> usize {
            match self {
                ArmId::Rlib => 0,
                ArmId::Dl => 1,
                ArmId::Stock => 2,
            }
        }
    }

    // ---- workload --------------------------------------------------------

    /// XorShift64. Identical algorithm and seeding discipline across every
    /// arm and every language binding in this repo (AGENTS.md §8.3).
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

    /// Draws successive keys of one distribution. The miss generator is
    /// the same type seeded differently, so absent probes come from the
    /// same distribution as the population rather than from a transform of
    /// a present key.
    struct KeyGen {
        dist: &'static str,
        rng: XorShift,
        i: u64,
        base: u64,
    }

    impl KeyGen {
        fn new(dist: &'static str, seed: u64) -> Self {
            Self {
                dist,
                rng: XorShift(seed),
                i: 0,
                base: 0,
            }
        }

        fn next(&mut self) -> Word {
            let k = match self.dist {
                "sequential" => self.i,
                "random" => self.rng.next(),
                "clustered" => {
                    if self.i.is_multiple_of(256) {
                        self.base = self.rng.next() & !0xFF;
                    }
                    self.base + (self.i % 256)
                }
                other => panic!("unknown distribution {other}"),
            };
            self.i += 1;
            k as Word
        }
    }

    /// The populated key set.
    fn keys(dist: &'static str, n: usize) -> Vec<Word> {
        let mut g = KeyGen::new(dist, 0x0DDB_1A5E_5EED_0001);
        (0..n).map(|_| g.next()).collect()
    }

    /// `n` distinct keys that are **absent** from `present`, drawn from the
    /// same generator as the population and rejected on membership.
    ///
    /// For `sequential` every key inside the populated range is present by
    /// construction, so rejection necessarily walks the generator past the
    /// end of that range — a dense integer run has no interior holes, and
    /// that is a property of the distribution, not a shortcut taken here.
    /// For `random` and `clustered` the misses interleave with the
    /// population across the whole expanse.
    fn miss_keys(dist: &'static str, present: &HashSet<Word>, n: usize) -> Vec<Word> {
        let mut g = KeyGen::new(dist, 0x51ED_0FF5_C0FF_EE01);
        let mut seen: HashSet<Word> = HashSet::with_capacity(n * 2);
        let mut out = Vec::with_capacity(n);
        // Bounded so a distribution that cannot yield enough absent keys
        // aborts loudly instead of spinning forever.
        let budget = n.saturating_mul(64).saturating_add(1024);
        for _ in 0..budget {
            if out.len() == n {
                return out;
            }
            let c = g.next();
            if !present.contains(&c) && seen.insert(c) {
                out.push(c);
            }
        }
        panic!("{dist}: could not draw {n} distinct absent keys within budget");
    }

    /// In-place Fisher-Yates with a fixed seed, so probe order differs
    /// from insert order and hits and misses interleave.
    fn shuffle(v: &mut [Word], seed: u64) {
        let mut rng = XorShift(seed);
        for i in (1..v.len()).rev() {
            v.swap(i, (rng.next() % (i as u64 + 1)) as usize);
        }
    }

    /// Everything a round needs, built once per cell and reused by every
    /// arm and every round. None of this is inside a timed region.
    struct Workload {
        ks: Vec<Word>,
        probes: Vec<Word>,
    }

    fn workload(dist: &'static str, pop: usize) -> Workload {
        let ks = keys(dist, pop);
        let present: HashSet<Word> = ks.iter().copied().collect();
        assert_eq!(
            present.len(),
            ks.len(),
            "{dist}/{pop}: key generator produced duplicates, which would \
             make the reported population wrong"
        );
        let misses = miss_keys(dist, &present, pop);
        let mut probes = Vec::with_capacity(pop * 2);
        probes.extend_from_slice(&ks);
        probes.extend_from_slice(&misses);
        shuffle(&mut probes, 0xBEEF_CAFE_1234_5678);
        Workload { ks, probes }
    }

    // ---- measurement -----------------------------------------------------

    /// One arm's result for one round.
    #[derive(Clone, Copy)]
    struct Sample {
        ins_ns: f64,
        get_ns: f64,
        bytes_per_key: f64,
    }

    /// One measured pass: build (timed), probe (timed), account, tear down.
    ///
    /// Generic over the arm so the rlib arm keeps direct calls; teardown
    /// and `MemUsed` sit outside both timed regions.
    fn run_arm<J: JudyL>(j: &J, w: &Workload) -> Sample {
        let mut arr: *mut c_void = null_mut();

        // SAFETY: the array is created, used and freed here, driven
        // strictly per the JudyL C contract, and never escapes.
        unsafe {
            let t0 = Instant::now();
            for &k in &w.ks {
                let slot = j.ins(&raw mut arr, std::hint::black_box(k));
                slot.write(!k);
            }
            let ins_ns = t0.elapsed().as_nanos() as f64 / w.ks.len() as f64;

            let t0 = Instant::now();
            let mut acc = 0usize;
            for &p in &w.probes {
                let slot = j.get(arr, std::hint::black_box(p));
                if !slot.is_null() {
                    // The value line is where the two structures' locality
                    // genuinely differs. Accumulating the *pointer* — which
                    // is what this harness used to do — leaves that access
                    // out of every published number.
                    acc ^= slot.read();
                }
            }
            std::hint::black_box(acc);
            let get_ns = t0.elapsed().as_nanos() as f64 / w.probes.len() as f64;

            let bytes_per_key = j.mem(arr) as f64 / w.ks.len() as f64;
            j.free(&raw mut arr);

            Sample {
                ins_ns,
                get_ns,
                bytes_per_key,
            }
        }
    }

    // ---- BCa bootstrap ---------------------------------------------------
    //
    // A Rust transcription of `scripts/bca_bootstrap.py` (same estimator,
    // same jackknife acceleration, same percentile indexing) so the table
    // carries its interval without a Python dependency. Resample draws come
    // from XorShift rather than CPython's Mersenne Twister, so the endpoints
    // differ from the script's in the last digits; the emitted JSON exists
    // so the script remains the checkable reference.

    /// Standard-normal CDF, via the Numerical Recipes `erfc` Chebyshev fit
    /// (fractional error < 1.2e-7 — orders finer than the 1/2000 index
    /// granularity of the bootstrap percentile it feeds).
    fn norm_cdf(x: f64) -> f64 {
        let z = x.abs() / core::f64::consts::SQRT_2;
        let t = 1.0 / (1.0 + 0.5 * z);
        let poly = -z * z - 1.265_512_23
            + t * (1.000_023_68
                + t * (0.374_091_96
                    + t * (0.096_784_18
                        + t * (-0.186_288_06
                            + t * (0.278_868_07
                                + t * (-1.135_203_98
                                    + t * (1.488_515_87
                                        + t * (-0.822_152_23 + t * 0.170_872_77))))))));
        let erfc_z = t * poly.exp();
        if x >= 0.0 {
            1.0 - 0.5 * erfc_z
        } else {
            0.5 * erfc_z
        }
    }

    /// Standard-normal inverse CDF (Acklam's rational approximation,
    /// relative error < 1.15e-9).
    fn norm_ppf(p: f64) -> f64 {
        assert!(p > 0.0 && p < 1.0, "probability must be in (0, 1), got {p}");
        const A: [f64; 6] = [
            -3.969_683_028_665_376e1,
            2.209_460_984_245_205e2,
            -2.759_285_104_469_687e2,
            1.383_577_518_672_69e2,
            -3.066_479_806_614_716e1,
            2.506_628_277_459_239e0,
        ];
        const B: [f64; 5] = [
            -5.447_609_879_822_406e1,
            1.615_858_368_580_409e2,
            -1.556_989_798_598_866e2,
            6.680_131_188_771_972e1,
            -1.328_068_155_288_572e1,
        ];
        const C: [f64; 6] = [
            -7.784_894_002_430_293e-3,
            -3.223_964_580_411_365e-1,
            -2.400_758_277_161_838e0,
            -2.549_732_539_343_734e0,
            4.374_664_141_464_968e0,
            2.938_163_982_698_783e0,
        ];
        const D: [f64; 4] = [
            7.784_695_709_041_462e-3,
            3.224_671_290_700_398e-1,
            2.445_134_137_142_996e0,
            3.754_408_661_907_416e0,
        ];
        const P_LOW: f64 = 0.02425;
        if p < P_LOW {
            let q = (-2.0 * p.ln()).sqrt();
            (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
                / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
        } else if p <= 1.0 - P_LOW {
            let q = p - 0.5;
            let r = q * q;
            (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
                / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
        } else {
            let q = (-2.0 * (1.0 - p).ln()).sqrt();
            -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
                / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
        }
    }

    /// `(mean, ci_lower, ci_upper)` at 95%.
    fn bca_ci(data: &[f64]) -> (f64, f64, f64) {
        let n = data.len();
        assert!(n >= 3, "BCa needs at least 3 observations, got {n}");
        let theta_hat = data.iter().sum::<f64>() / n as f64;

        let mut rng = XorShift(0x0BCA_0BCA_0BCA_0001);
        let mut boot: Vec<f64> = (0..RESAMPLES)
            .map(|_| {
                let s: f64 = (0..n).map(|_| data[(rng.next() % n as u64) as usize]).sum();
                s / n as f64
            })
            .collect();
        boot.sort_by(f64::total_cmp);

        let less = boot.iter().filter(|b| **b < theta_hat).count();
        let prop_less = (less as f64 / RESAMPLES as f64).clamp(1e-6, 1.0 - 1e-6);
        let z0 = norm_ppf(prop_less);

        let total: f64 = data.iter().sum();
        let jack: Vec<f64> = data
            .iter()
            .map(|d| (total - d) / (n as f64 - 1.0))
            .collect();
        let jack_bar = jack.iter().sum::<f64>() / n as f64;
        let diffs: Vec<f64> = jack.iter().map(|m| jack_bar - m).collect();
        let num: f64 = diffs.iter().map(|d| d * d * d).sum();
        let den = 6.0 * diffs.iter().map(|d| d * d).sum::<f64>().powf(1.5);
        let a = if den.abs() > 1e-12 { num / den } else { 0.0 };

        let adjusted = |z: f64| {
            let mut denom = 1.0 - a * (z0 + z);
            if denom.abs() < 1e-6 {
                denom = if denom >= 0.0 { 1e-6 } else { -1e-6 };
            }
            norm_cdf(z0 + (z0 + z) / denom).clamp(0.0, 1.0)
        };
        let p1 = adjusted(norm_ppf(0.025));
        let p2 = adjusted(norm_ppf(0.975));
        let idx = |p: f64| ((p * RESAMPLES as f64) as usize).min(RESAMPLES - 1);

        (theta_hat, boot[idx(p1)], boot[idx(p2)])
    }

    // ---- reporting -------------------------------------------------------

    /// One `(cell, metric, arm)` series of per-round paired ratios.
    struct Series {
        id: String,
        dist: &'static str,
        pop: usize,
        metric: &'static str,
        arm: &'static str,
        ratios: Vec<f64>,
    }

    /// Where the per-round ratios land. Under the gitignored build
    /// directory by default, so a smoke run can never overwrite a
    /// committed baseline (AGENTS.md §8.5).
    fn ratios_path() -> std::path::PathBuf {
        std::env::var("EXPANSE_BENCH_RATIOS_JSON")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::path::PathBuf::from("target/bench/vs_libjudy_paired_ratios.json")
            })
    }

    fn write_ratios(path: &std::path::Path, rounds: usize, series: &[Series]) {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .unwrap_or_else(|e| panic!("cannot create {}: {e}", dir.display()));
        }
        let mut s = String::new();
        writeln!(s, "{{").unwrap();
        writeln!(
            s,
            "  \"harness\": \"crates/expanse-capi/examples/bench_vs_libjudy.rs\","
        )
        .unwrap();
        writeln!(s, "  \"rounds\": {rounds},").unwrap();
        writeln!(s, "  \"hit_rate\": {HIT_RATE},").unwrap();
        writeln!(s, "  \"denominator_arm\": \"stock\",").unwrap();
        writeln!(
            s,
            "  \"note\": \"each series' `ratios` is a per-round paired ratio, \
             ready for scripts/bca_bootstrap.py:bca_bootstrap_ci\","
        )
        .unwrap();
        writeln!(s, "  \"series\": [").unwrap();
        for (i, ser) in series.iter().enumerate() {
            let vals: Vec<String> = ser.ratios.iter().map(|r| format!("{r:.6}")).collect();
            writeln!(
                s,
                "    {{\"id\": \"{}\", \"dist\": \"{}\", \"pop\": {}, \"metric\": \"{}\", \
                 \"arm\": \"{}\", \"ratios\": [{}]}}{}",
                ser.id,
                ser.dist,
                ser.pop,
                ser.metric,
                ser.arm,
                vals.join(", "),
                if i + 1 == series.len() { "" } else { "," }
            )
            .unwrap();
        }
        writeln!(s, "  ]").unwrap();
        writeln!(s, "}}").unwrap();

        let mut f = std::fs::File::create(path)
            .unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
        f.write_all(s.as_bytes())
            .unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
    }

    fn parse_rounds() -> usize {
        let mut rounds = DEFAULT_ROUNDS;
        let mut args = std::env::args().skip(1);
        while let Some(a) = args.next() {
            match a.as_str() {
                "--rounds" => {
                    let v = args
                        .next()
                        .unwrap_or_else(|| panic!("--rounds needs a value"));
                    rounds = v
                        .parse()
                        .unwrap_or_else(|e| panic!("--rounds {v:?} is not an integer: {e}"));
                }
                other => panic!("unknown argument {other:?} (accepted: --rounds N)"),
            }
        }
        assert!(
            rounds >= 3,
            "--rounds must be at least 3 for a BCa interval, got {rounds}"
        );
        rounds
    }

    /// Formats a ratio and its interval, or dashes for the denominator arm.
    fn fmt_ci(ci: Option<(f64, f64, f64)>) -> String {
        match ci {
            Some((p, lo, hi)) => format!("{p:>6.2} [{lo:>5.2},{hi:>5.2}]"),
            None => format!("{:>6} {:>13}", "—", "(denominator)"),
        }
    }

    pub fn main() {
        let rounds = parse_rounds();

        // Both dynamic arms are opened and bound before anything is timed.
        // The handles are deliberately never `dlclose`d: the resolved
        // pointers outlive them and the process is the benchmark.
        let ours_dl = Dl(Lib::ours().api());
        let stock_dl = Dl(Lib::stock().api());

        println!("libexpanse vs stock libjudy — JudyL surface, wall-clock, paired interleaved A/B");
        println!(
            "rounds: {rounds}   probes/pass: 2 x population distinct keys, {:.0}% hit, reuse 1.0",
            HIT_RATE * 100.0
        );
        println!(
            "point estimate = mean of the per-round paired ratios vs `stock`; \
             [lo,hi] = BCa 95% CI ({RESAMPLES} resamples)"
        );
        println!(
            "ratio < 1.00 means libexpanse is faster / smaller; an interval spanning 1.00 is not a difference"
        );
        println!(
            "`expanse_dl` is the like-for-like arm; `expanse_rlib` is the LTO-linked shape no libexpanse user gets\n"
        );
        println!(
            "{:<11} {:>8} {:<13} | {:>9} {:<20} | {:>9} {:<20} | {:>7} {:>6}",
            "dist",
            "pop",
            "arm",
            "ins ns/op",
            "ins ratio [95% CI]",
            "get ns/op",
            "get ratio [95% CI]",
            "B/key",
            "B/k r"
        );

        let mut all_series: Vec<Series> = Vec::new();

        for dist in DISTRIBUTIONS {
            for pop in POPULATIONS {
                let w = workload(dist, pop);
                // [round][arm]
                let mut samples: Vec<[Option<Sample>; 3]> = vec![[None; 3]; rounds];

                for (r, slot) in samples.iter_mut().enumerate() {
                    // Rotate arm order so drift lands on each arm equally.
                    for step in 0..ArmId::ALL.len() {
                        let id = ArmId::ALL[(r + step) % ArmId::ALL.len()];
                        let s = match id {
                            ArmId::Rlib => run_arm(&Rlib, &w),
                            ArmId::Dl => run_arm(&ours_dl, &w),
                            ArmId::Stock => run_arm(&stock_dl, &w),
                        };
                        slot[id.index()] = Some(s);
                    }
                }

                let take = |arm: ArmId, f: fn(&Sample) -> f64| -> Vec<f64> {
                    samples
                        .iter()
                        .map(|row| f(row[arm.index()].as_ref().expect("every arm ran")))
                        .collect()
                };

                let denom_ins = take(ArmId::Stock, |s| s.ins_ns);
                let denom_get = take(ArmId::Stock, |s| s.get_ns);
                let denom_mem = take(ArmId::Stock, |s| s.bytes_per_key);
                let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;

                for id in ArmId::ALL {
                    let ins = take(id, |s| s.ins_ns);
                    let get = take(id, |s| s.get_ns);
                    let mem = take(id, |s| s.bytes_per_key);

                    let (ins_ci, get_ci, mem_ratio) = if id == ArmId::Stock {
                        (None, None, None)
                    } else {
                        let paired = |num: &[f64], den: &[f64]| -> Vec<f64> {
                            num.iter().zip(den).map(|(a, b)| a / b).collect()
                        };
                        let ins_r = paired(&ins, &denom_ins);
                        let get_r = paired(&get, &denom_get);
                        all_series.push(Series {
                            id: format!("{dist}/{pop}/ins/{}_over_stock", id.label()),
                            dist,
                            pop,
                            metric: "ins_ns",
                            arm: id.label(),
                            ratios: ins_r.clone(),
                        });
                        all_series.push(Series {
                            id: format!("{dist}/{pop}/get/{}_over_stock", id.label()),
                            dist,
                            pop,
                            metric: "get_ns",
                            arm: id.label(),
                            ratios: get_r.clone(),
                        });
                        (
                            Some(bca_ci(&ins_r)),
                            Some(bca_ci(&get_r)),
                            Some(mean(&mem) / mean(&denom_mem)),
                        )
                    };

                    println!(
                        "{:<11} {:>8} {:<13} | {:>9.1} {:<20} | {:>9.1} {:<20} | {:>7.2} {:>6}",
                        dist,
                        pop,
                        id.label(),
                        mean(&ins),
                        fmt_ci(ins_ci),
                        mean(&get),
                        fmt_ci(get_ci),
                        mean(&mem),
                        match mem_ratio {
                            Some(r) => format!("{r:.2}"),
                            None => "—".to_string(),
                        }
                    );
                }
                println!();
            }
        }

        let path = ratios_path();
        write_ratios(&path, rounds, &all_series);
        println!(
            "per-round paired ratios: {} ({} series; feed each `ratios` array to \
             scripts/bca_bootstrap.py:bca_bootstrap_ci to re-derive the intervals)",
            path.display(),
            all_series.len()
        );
        println!(
            "B/key is deterministic JudyLMemUsed accounting and carries no interval; \
             wall-clock rows are indicative unless the host was quiet (docs/BENCHMARKING.md)"
        );
    }
}
