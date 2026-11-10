//! **MariaDB OQGraph's two-bit colour map on Judy1, libexpanse against
//! stock libjudy, in wall clock and under `perf stat`** (#1682).
//!
//! OQGraph keeps each breadth-first search's vertex colours in two Judy1
//! arrays and calls only `Judy1Test`, `Judy1Set`, `Judy1Unset` and
//! `Judy1FreeArray` on them (`benches/colour_map_common/mod.rs` has the
//! pattern). Its evaluation measured every libexpanse build slower than stock
//! end to end while per-call instructions were at parity, so the cost is in
//! how the calls execute, which a Callgrind count cannot see. The
//! `judy1_colour_*` arms in `benches/vs_stock.rs` count instructions on the
//! same tape; this binary times it and is the workload `perf stat` wraps.
//!
//! Both modes replay the same recorded call sequence (the tape) against fresh
//! arrays, so every arm makes exactly the same calls in the same order and
//! the measured region holds nothing else.
//!
//! **Timing mode** (default): `--rounds R` (default 15, at least 3) rounds;
//! each round runs every arm once per population, in an order that rotates
//! per round, and forms the paired ratio `expanse_dl / stock` inside the
//! round. One arm's sample is `--reps K` (default 20) replays of the tape,
//! timed together. Prints the mean per-call nanoseconds per arm and the mean
//! paired ratio, and writes every round's samples and ratios to
//! `EXPANSE_COLOUR_JSON` (default `target/bench/colour_map_paired.json`, a
//! gitignored scratch path, AGENTS.md §8.5) in the shape
//! `scripts/bca_bootstrap.py` takes; the interval is computed there, not here.
//! Rounds share one process, so a later round reuses memory an earlier one
//! freed, as successive queries in one OQGraph server do.
//!
//! **Counter mode** (`EXPANSE_PERF_PHASE` set): the workload half of
//! `scripts/perf_counters.py --workload colour_map`. `build` records the tape
//! and resolves the library, then exits; `probe` does the same and replays the
//! tape `EXPANSE_PERF_PASSES` times. The driver differences the two phases.
//! `EXPANSE_PERF_ARM` is `stock`, `expanse_dl` or `expanse_rlib`;
//! `EXPANSE_PERF_POP` is the vertex count. Unrecognised values are fatal
//! (AGENTS.md §8.1).
//!
//! Needs stock libjudy (`libjudy-dev`, or `brew install judy`) and, for
//! `expanse_dl`, `libexpanse` (`EXPANSE_CDYLIB`, or `target/release`).
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `capi_colour_map` |
//! | `group` | 1 |
//! | `population` | 10k and 100k vertices (`EXPANSE_PERF_POP` in counter mode) |
//! | `insertion_order` | generator — breadth-first visit order of a fixed graph (four out-edges per vertex: three from an LCG, plus `v + 1`), from vertex 0 |
//! | `probes_and_reuse` | one search's tape: 8 tests, 4 set or unset calls per vertex and 2 frees; replayed `--reps` times per sample (fresh arrays each time) |
//! | `hit_rate` | as the search produces it: a test of a vertex already gray or black hits |
//! | `miss_gen_method` | n/a — no separate miss stream; every test is the search's own colour read |
//! | `value_dereference` | every return value summed into a sink |
//! | `measured_region` | the replays only; the tape is recorded and the library resolved before timing |
//! | `arm_symmetry` | Symmetric (`dlopen` vs `dlopen`); `expanse_rlib` is LTO-linked and reported for its correction factor only |
//! | `statistics` | Paired ratio per round; BCa 95% over the rounds via `scripts/bca_bootstrap.py` |
//! | `verdict` | **PENDING** `[unverified: reference-host run pending, #1682]`: Instrument added before any engine change on this path. |

#[cfg(not(unix))]
fn main() {
    eprintln!("colour_map needs a dlopen platform (Linux/macOS) with stock libjudy");
    std::process::exit(1);
}

#[cfg(unix)]
fn main() {
    bench::main();
}

#[cfg(unix)]
#[path = "../benches/colour_map_common/mod.rs"]
mod colour_map_common;

#[cfg(unix)]
mod bench {
    use crate::colour_map_common::{self as cm, Judy1, Word};
    use core::ffi::{CStr, c_int, c_void};
    use std::fmt::Write as _;
    use std::hint::black_box;
    use std::time::Instant;

    unsafe extern "C" {
        fn dlopen(filename: *const u8, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const u8) -> *mut c_void;
    }

    const RTLD_NOW: c_int = 2;
    /// Keep a loaded library's symbols out of the global namespace: stock,
    /// our cdylib and this binary export the same names. macOS numbers the
    /// flag differently and defaults to `RTLD_GLOBAL`.
    #[cfg(target_os = "macos")]
    const RTLD_LOCAL: c_int = 4;
    #[cfg(not(target_os = "macos"))]
    const RTLD_LOCAL: c_int = 0;

    const DEFAULT_ROUNDS: usize = 15;
    const DEFAULT_REPS: usize = 20;
    const POPULATIONS: [u64; 2] = [10_000, 100_000];
    /// Arms, in the order the first round runs them.
    const ARMS: [&str; 3] = ["stock", "expanse_dl", "expanse_rlib"];

    const STOCK_NAMES: &[&CStr] = &[
        c"libJudy.so.1",
        c"libJudy.so",
        c"/opt/homebrew/opt/judy/lib/libJudy.dylib",
        c"/usr/local/opt/judy/lib/libJudy.dylib",
        c"libJudy.dylib",
    ];
    const OURS_NAMES: &[&CStr] = &[
        c"target/release/libexpanse.so",
        c"target/release/libexpanse.dylib",
    ];

    fn open_any(candidates: &[&CStr], what: &str) -> *mut c_void {
        for name in candidates {
            // SAFETY: valid NUL-terminated library name.
            let h = unsafe { dlopen(name.to_bytes_with_nul().as_ptr(), RTLD_NOW | RTLD_LOCAL) };
            if !h.is_null() {
                return h;
            }
        }
        panic!("{what} not found (tried {candidates:?})");
    }

    fn ours() -> *mut c_void {
        if let Ok(p) = std::env::var("EXPANSE_CDYLIB") {
            let c = std::ffi::CString::new(p.clone()).expect("EXPANSE_CDYLIB has an interior NUL");
            // SAFETY: valid NUL-terminated library name.
            let h = unsafe { dlopen(c.to_bytes_with_nul().as_ptr(), RTLD_NOW | RTLD_LOCAL) };
            assert!(!h.is_null(), "EXPANSE_CDYLIB set to {p} but dlopen failed");
            return h;
        }
        open_any(OURS_NAMES, "libexpanse cdylib — set EXPANSE_CDYLIB")
    }

    fn sym<T: Copy>(h: *mut c_void, name: &CStr) -> T {
        // SAFETY: live handle and NUL-terminated name; the caller names the
        // fn-pointer type matching the symbol.
        let p = unsafe { dlsym(h, name.to_bytes_with_nul().as_ptr()) };
        assert!(!p.is_null(), "missing symbol {name:?}");
        // SAFETY: fn-pointer transmute of a resolved symbol.
        unsafe { core::mem::transmute_copy(&p) }
    }

    fn loaded(h: *mut c_void) -> Judy1 {
        Judy1 {
            test: sym(h, c"Judy1Test"),
            set: sym(h, c"Judy1Set"),
            unset: sym(h, c"Judy1Unset"),
            free: sym(h, c"Judy1FreeArray"),
        }
    }

    unsafe extern "C" fn rlib_test(p: *const c_void, k: Word, pj: *mut c_void) -> c_int {
        // SAFETY: forwarded unchanged.
        unsafe { expanse::Judy1Test(p, k, pj.cast()) }
    }
    unsafe extern "C" fn rlib_set(p: *mut *mut c_void, k: Word, pj: *mut c_void) -> c_int {
        // SAFETY: forwarded unchanged.
        unsafe { expanse::Judy1Set(p, k, pj.cast()) }
    }
    unsafe extern "C" fn rlib_unset(p: *mut *mut c_void, k: Word, pj: *mut c_void) -> c_int {
        // SAFETY: forwarded unchanged.
        unsafe { expanse::Judy1Unset(p, k, pj.cast()) }
    }
    unsafe extern "C" fn rlib_free(p: *mut *mut c_void, pj: *mut c_void) -> Word {
        // SAFETY: forwarded unchanged.
        unsafe { expanse::Judy1FreeArray(p, pj.cast()) }
    }

    /// The rlib arm's entry points: our code linked into this binary.
    fn rlib() -> Judy1 {
        Judy1 {
            test: rlib_test,
            set: rlib_set,
            unset: rlib_unset,
            free: rlib_free,
        }
    }

    fn arm(name: &str) -> Judy1 {
        match name {
            "stock" => loaded(open_any(
                STOCK_NAMES,
                "stock libjudy — install libjudy-dev (Linux) or `brew install judy` (macOS)",
            )),
            "expanse_dl" => loaded(ours()),
            "expanse_rlib" => rlib(),
            other => panic!("unknown arm {other:?} (accepted: stock, expanse_dl, expanse_rlib)"),
        }
    }

    fn env_or(name: &str, default: &str) -> String {
        match std::env::var(name) {
            Ok(v) if !v.is_empty() => v,
            Ok(_) | Err(std::env::VarError::NotPresent) => default.to_string(),
            Err(e) => panic!("{name} is set but unreadable: {e}"),
        }
    }

    fn parse<T: core::str::FromStr>(name: &str, v: &str) -> T
    where
        T::Err: core::fmt::Display,
    {
        v.parse()
            .unwrap_or_else(|e| panic!("{name}={v:?} is not a number: {e}"))
    }

    /// The `perf stat` workload: one arm, one population, one phase.
    fn counter_mode(phase: &str) {
        let name = env_or("EXPANSE_PERF_ARM", "expanse_dl");
        let pop: u64 = parse("EXPANSE_PERF_POP", &env_or("EXPANSE_PERF_POP", "100000"));
        let passes: usize = parse("EXPANSE_PERF_PASSES", &env_or("EXPANSE_PERF_PASSES", "1"));
        let tape = cm::tape(pop);
        let j = arm(&name);
        let sink = match phase {
            "build" => 0,
            "probe" => {
                let mut sink: Word = 0;
                for _ in 0..passes {
                    // SAFETY: `j` is one library's Judy1 entry points.
                    sink = sink.wrapping_add(unsafe { cm::replay(black_box(&tape), j) });
                }
                sink
            }
            other => panic!("EXPANSE_PERF_PHASE={other:?} (accepted: build, probe)"),
        };
        // `ops=` is the call count `perf_counters.py` divides by.
        println!(
            "arm={name} pop={pop} passes={passes} ops={} sink={}",
            tape.len() * passes,
            black_box(sink)
        );
    }

    fn parse_args() -> (usize, usize) {
        let (mut rounds, mut reps) = (DEFAULT_ROUNDS, DEFAULT_REPS);
        let mut args = std::env::args().skip(1);
        while let Some(a) = args.next() {
            let v = args.next().unwrap_or_else(|| panic!("{a} needs a value"));
            match a.as_str() {
                "--rounds" => rounds = parse("--rounds", &v),
                "--reps" => reps = parse("--reps", &v),
                other => panic!("unknown argument {other:?} (accepted: --rounds N, --reps K)"),
            }
        }
        assert!(
            rounds >= 3,
            "--rounds must be at least 3 for a BCa interval"
        );
        assert!(reps >= 1, "--reps must be at least 1");
        (rounds, reps)
    }

    /// Nanoseconds per call for `reps` replays of `tape`, and the replay's
    /// result sum.
    fn sample(tape: &[u64], j: Judy1, reps: usize) -> (f64, Word) {
        let t0 = Instant::now();
        let mut sink: Word = 0;
        for _ in 0..reps {
            // SAFETY: `j` is one library's Judy1 entry points.
            sink = sink.wrapping_add(unsafe { cm::replay(black_box(tape), j) });
        }
        let ns = t0.elapsed().as_nanos() as f64;
        (ns / (tape.len() * reps) as f64, black_box(sink))
    }

    fn mean(v: &[f64]) -> f64 {
        v.iter().sum::<f64>() / v.len() as f64
    }

    pub fn main() {
        if let Ok(phase) = std::env::var("EXPANSE_PERF_PHASE") {
            counter_mode(&phase);
            return;
        }
        let (rounds, reps) = parse_args();
        let apis: Vec<Judy1> = ARMS.iter().map(|a| arm(a)).collect();
        let mut json = String::from("{\n  \"workload_id\": \"capi_colour_map\",\n");
        let _ = writeln!(
            json,
            "  \"rounds\": {rounds},\n  \"reps\": {reps},\n  \"series\": ["
        );
        println!("colour map, {rounds} rounds x {reps} replays per sample; ns per Judy1 call");
        println!(
            "{:>8} {:>10} {:>10} {:>10} {:>14} {:>14}",
            "pop", "stock", "dl", "rlib", "dl/stock", "rlib/stock"
        );
        for (pi, &pop) in POPULATIONS.iter().enumerate() {
            let tape = cm::tape(pop);
            let mut ns = [vec![], vec![], vec![]];
            for r in 0..rounds {
                eprintln!("BENCH_WINDOW begin colour/pop={pop}/round={r}");
                let mut sinks = [0; 3];
                for k in 0..ARMS.len() {
                    let a = (r + k) % ARMS.len();
                    let (t, sink) = sample(&tape, apis[a], reps);
                    ns[a].push(t);
                    sinks[a] = sink;
                }
                // Every arm made the same calls, so every arm returned the
                // same results; a difference is a correctness defect, and a
                // timing over it means nothing (AGENTS.md section 8.1).
                assert!(
                    sinks.iter().all(|&s| s == sinks[0]),
                    "results differ between arms at pop={pop}: {sinks:?} (stock, dl, rlib)"
                );
                eprintln!("BENCH_WINDOW end colour/pop={pop}/round={r}");
            }
            let ratio =
                |a: usize| -> Vec<f64> { ns[a].iter().zip(&ns[0]).map(|(x, s)| x / s).collect() };
            let (dl, rl) = (ratio(1), ratio(2));
            println!(
                "{pop:>8} {:>10.2} {:>10.2} {:>10.2} {:>14.3} {:>14.3}",
                mean(&ns[0]),
                mean(&ns[1]),
                mean(&ns[2]),
                mean(&dl),
                mean(&rl)
            );
            let c = cm::census(&tape);
            let fmt = |v: &[f64]| {
                v.iter()
                    .map(|x| format!("{x:.6}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let _ = write!(
                json,
                "    {{\"id\": \"colour/pop={pop}\", \"population\": {pop}, \
                 \"calls\": {}, \"tests\": {}, \"sets\": {}, \"unsets\": {}, \"frees\": {},\n     \
                 \"ns_per_call\": {{\"stock\": [{}], \"expanse_dl\": [{}], \"expanse_rlib\": [{}]}},\n     \
                 \"ratios\": [{}], \"rlib_ratios\": [{}]}}{}\n",
                tape.len(),
                c[0],
                c[1],
                c[2],
                c[3],
                fmt(&ns[0]),
                fmt(&ns[1]),
                fmt(&ns[2]),
                fmt(&dl),
                fmt(&rl),
                if pi + 1 < POPULATIONS.len() { "," } else { "" }
            );
        }
        json.push_str("  ]\n}\n");
        let path = std::path::PathBuf::from(env_or(
            "EXPANSE_COLOUR_JSON",
            "target/bench/colour_map_paired.json",
        ));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("create the JSON directory");
        }
        std::fs::write(&path, json).expect("write the paired-ratio JSON");
        println!(
            "per-round samples and paired ratios: {} (BCa: scripts/bca_bootstrap.py)",
            path.display()
        );
        println!(
            "⚠️ Local diagnostic sample unless run on the quiet reference host; \
             continuous wall-clock gating requires BCa 95% intervals (AGENTS.md §8.4)."
        );
    }
}
