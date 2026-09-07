//! Random point-lookup workload for the `perf stat` counter harness
//! (`scripts/perf_counters.py`, [#455](https://github.com/orieg/expanse/issues/455) R0).
//!
//! This binary exists to be wrapped by `perf stat`. It is **not** a
//! benchmark and prints no timing: `perf` owns the measurement, this owns
//! the workload. `docs/BENCHMARKING.md` records what the counters can and
//! cannot answer.
//!
//! **Why a separate binary.** The Callgrind harnesses gate on instructions
//! retired. Two mechanisms on this path (#455 M1, M2) change cache line
//! fills and address-dependency chains and retire no extra instructions, so
//! `Ir` is structurally blind to them. Counters are the instrument that is
//! not; `perf stat` needs a whole process to wrap, which a
//! `library_benchmark` function is not.
//!
//! **The measured region, and how it is separated.** `perf stat` counts a
//! whole process, so the build cannot be excluded the way iai-callgrind's
//! `setup =` excludes it. Instead the two phases are separate processes
//! doing *identical* work up to the probe loop:
//!
//! - `EXPANSE_PERF_PHASE=build` — generate keys, build the structure,
//!   generate and shuffle the probe vector, print the checksum, exit.
//! - `EXPANSE_PERF_PHASE=probe` — all of the above, then run the probe
//!   passes.
//!
//! Every input is derived from fixed seeds, so the two phases build the
//! same structure from the same keys in the same order. The driver runs
//! both and reports `probe - build` as the counts attributed to the probe
//! loop, alongside both raw phases so the reader can see what fraction the
//! subtraction removed. Differencing two processes is not the same
//! instrument as bracketing a region inside one: process setup, page faults
//! and allocator state do not cancel exactly. That is why the driver
//! publishes an interval over repeated runs rather than a single number.
//!
//! **Workload configuration** (`EXPANSE_PERF_*`, all optional):
//!
//! | Variable | Default | Meaning |
//! |---|---|---|
//! | `EXPANSE_PERF_ARM` | `map_get` | `map_get`, `set_contains`, `strmap_get` (`short` strings) or `strmap_get_counter` |
//! | `EXPANSE_PERF_PHASE` | `probe` | `probe` or `build` |
//! | `EXPANSE_PERF_POP` | `1000000` | keys inserted |
//! | `EXPANSE_PERF_HIT_PCT` | `100` | percent of probes that are present keys |
//! | `EXPANSE_PERF_PASSES` | `1` | passes over the probe vector |
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `example_perf_point_lookup` |
//! | `group` | 5 |
//! | `population` | 1M (configurable via `EXPANSE_PERF_POP`); the string arms draw the suites' `short` (mean 12.0 B) and `counter` (12 B) shapes |
//! | `insertion_order` | generator draw order — the population is inserted as drawn, neither sorted nor shuffled; the shuffle in this file is applied to the probe stream, not to the build |
//! | `probes_and_reuse` | 1M distinct per pass, reuse 1.0 |
//! | `hit_rate` | 100% default (configurable via `EXPANSE_PERF_HIT_PCT`) |
//! | `miss_gen_method` | Independent PRNG + membership rejection; the string arms continue the shape's own generator past the population and reject on membership (§8.6) |
//! | `value_dereference` | `sink ^= *pval`; the string and set arms add rather than XOR, since values `0..pop` XOR back to zero over a full pass and the sink would stop seeing its own loop |
//! | `measured_region` | Phase differencing (`probe - build`) |
//! | `arm_symmetry` | Twin to Callgrind `core_instructions` |
//! | `statistics` | `perf stat` hardware counters |
//! | `verdict` | **PASS** `[verified: RUN (reference host, results/baseline_perf_counters.json)]`: Gold standard hardware counter reference. |
//!
//! Distinct probes equal the population — every key probed once per pass,
//! in an order that is not the build order, so the working set is the whole
//! structure rather than a resident subset (the defect #454 records in
//! `bench_vs_libjudy.rs`). At the default 100% hit rate this arm is the
//! counter-side twin of the `map_get/random` and `set_contains/random`
//! Callgrind arms, which are also 100% hit. `EXPANSE_PERF_HIT_PCT=50`
//! gives the mixed rate methodology rule 14 asks of a read workload;
//! misses are drawn from an independent PRNG stream and verified absent,
//! never derived from a hit key by a fixed transform.
//!
//! Unrecognised values are fatal. A mistyped variable that silently fell
//! back to a default would publish counters for a workload nobody asked
//! for (`AGENTS.md` section 8.1).
//!
//! Run: `EXPANSE_PERF_PHASE=probe ./target/release/examples/perf_point_lookup`

use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use expanse_trie::strmap::ExpanseStrMap;
use std::hint::black_box;

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

/// Build stream. Same constant as `benches/instructions.rs`, so the two
/// instruments descend the same shaped tree.
const SEED_KEYS: u64 = 0x0DDB_1A5E_5EED_0001;
/// Probe-order shuffle. Same constant as `benches/instructions.rs`.
const SEED_SHUFFLE: u64 = 0x9E37_79B9;
/// Miss stream. Independent of `SEED_KEYS`, so a miss is not a hit key put
/// through a fixed transform — a perfectly correlated miss distribution is
/// the defect #454 records in `compare.rs`.
const SEED_MISSES: u64 = 0xC0FF_EE00_1234_5678;

fn env_str(name: &str, default: &str) -> String {
    match std::env::var(name) {
        Ok(v) if v.is_empty() => default.to_string(),
        Ok(v) => v,
        Err(std::env::VarError::NotPresent) => default.to_string(),
        Err(e) => panic!("{name} is set but unreadable: {e}"),
    }
}

fn env_usize(name: &str, default: usize) -> usize {
    let raw = env_str(name, &default.to_string());
    raw.parse().unwrap_or_else(|e| {
        panic!("{name}={raw} is not a non-negative integer: {e}");
    })
}

/// `pop` uniform-random 64-bit keys.
fn build_keys(pop: usize) -> Vec<u64> {
    let mut rng = XorShift(SEED_KEYS);
    (0..pop).map(|_| rng.next()).collect()
}

/// `pop` byte-string keys in one of the shapes the comparison suites use
/// (#724). Kept byte-for-byte compatible with
/// `crates/expanse-hot-bench/src/strings.rs` so a counter taken here and a
/// wall-clock cell taken there describe the same tree:
///
/// - `short`: `8 + rng % 9` alphanumeric bytes, mean length 12.0 — the shape
///   whose 100%-hit lookup costs about four times a `u64` lookup.
/// - `counter`: `k` followed by 11 decimal digits, 12 bytes exactly, every
///   key resolving inside its terminal chunk with no suffix leaf at all.
///
/// The two differ in exactly the structure this probe exists to price: a
/// `short` lookup ends at a suffix leaf, a `counter` lookup does not.
fn build_str_keys(pop: usize, shape: &str) -> Vec<Vec<u8>> {
    const ALNUM: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let mut rng = XorShift(SEED_KEYS);
    let mut out = Vec::with_capacity(pop);
    let mut seen = std::collections::HashSet::with_capacity(pop);
    match shape {
        "short" => {
            while out.len() < pop {
                let n = 8 + (rng.next() % 9) as usize;
                let k: Vec<u8> = (0..n)
                    .map(|_| ALNUM[(rng.next() % ALNUM.len() as u64) as usize])
                    .collect();
                if seen.insert(k.clone()) {
                    out.push(k);
                }
            }
        }
        "counter" => {
            for i in 0..pop {
                out.push(format!("k{i:011}").into_bytes());
            }
        }
        other => panic!("string shape {other} is not recognised: use `short` or `counter`"),
    }
    out
}

/// The string analogue of [`build_probes`], with the same miss discipline:
/// misses come from the shape's own generator and are rejected on membership,
/// never a transform of a present key (§8.6).
fn build_str_probes(
    keys: &[Vec<u8>],
    present: &dyn Fn(&[u8]) -> bool,
    hit_pct: usize,
    shape: &str,
) -> Vec<Vec<u8>> {
    let pop = keys.len();
    let hits = pop * hit_pct / 100;
    let mut probes: Vec<Vec<u8>> = keys[..hits].to_vec();

    if probes.len() < pop {
        // Continue the shape's own stream past the population rather than
        // restarting it, so a miss is the same kind of key as a hit and lands
        // at a comparable depth (§8.6, and the `with_offset` rule).
        let wanted = pop - probes.len();
        let extra = build_str_keys(pop + wanted * 4, shape);
        for k in extra.into_iter().rev() {
            if probes.len() == pop {
                break;
            }
            if !present(&k) {
                probes.push(k);
            }
        }
        assert_eq!(probes.len(), pop, "miss budget exhausted for shape {shape}");
    }

    let mut rng = XorShift(SEED_SHUFFLE);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    probes
}

/// The probe vector: `hit_pct` percent present keys, the rest drawn from an
/// independent stream and checked absent. Shuffled out of build order.
fn build_probes(keys: &[u64], present: &dyn Fn(u64) -> bool, hit_pct: usize) -> Vec<u64> {
    let pop = keys.len();
    let hits = pop * hit_pct / 100;
    let mut probes = Vec::with_capacity(pop);
    probes.extend_from_slice(&keys[..hits]);

    let mut miss_rng = XorShift(SEED_MISSES);
    while probes.len() < pop {
        let candidate = miss_rng.next();
        // A collision with the build stream is astronomically unlikely at
        // these populations, but "unlikely" is not "checked": a hit key
        // counted as a miss would move the measured hit rate away from the
        // declared one.
        if !present(candidate) {
            probes.push(candidate);
        }
    }

    let mut rng = XorShift(SEED_SHUFFLE);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    probes
}

fn main() {
    let arm = env_str("EXPANSE_PERF_ARM", "map_get");
    let phase = env_str("EXPANSE_PERF_PHASE", "probe");
    let pop = env_usize("EXPANSE_PERF_POP", 1_000_000);
    let hit_pct = env_usize("EXPANSE_PERF_HIT_PCT", 100);
    let passes = env_usize("EXPANSE_PERF_PASSES", 1);

    assert!(pop > 0, "EXPANSE_PERF_POP must be at least 1");
    assert!(hit_pct <= 100, "EXPANSE_PERF_HIT_PCT={hit_pct} exceeds 100");
    assert!(passes > 0, "EXPANSE_PERF_PASSES must be at least 1");
    let run_probe = match phase.as_str() {
        "probe" => true,
        "build" => false,
        other => panic!("EXPANSE_PERF_PHASE={other} is not recognised: use `probe` or `build`"),
    };

    let keys = build_keys(pop);
    let mut sink = 0u64;

    match arm.as_str() {
        "map_get" => {
            let mut map = ExpanseMap::new();
            for &k in &keys {
                map.insert(k, !k);
            }
            let probes = build_probes(&keys, &|k| map.get(k).is_some(), hit_pct);
            sink ^= map.len();
            if run_probe {
                for _ in 0..passes {
                    for &k in &probes {
                        sink ^= map.get(black_box(k)).unwrap_or(0);
                    }
                }
            }
            // Leaked deliberately: a `free_subtree` walk at exit would be
            // counted by `perf stat`, which measures the whole process.
            core::mem::forget(map);
            core::mem::forget(probes);
        }
        "set_contains" => {
            let mut set = ExpanseSet::new();
            for &k in &keys {
                set.insert(k);
            }
            let probes = build_probes(&keys, &|k| set.contains(k), hit_pct);
            sink ^= set.len();
            if run_probe {
                for _ in 0..passes {
                    for &k in &probes {
                        // Added, not XORed: an even number of hits
                        // would XOR back to zero and leave the sink
                        // indistinguishable from the build phase's.
                        sink = sink.wrapping_add(u64::from(set.contains(black_box(k))));
                    }
                }
            }
            core::mem::forget(set);
            core::mem::forget(probes);
        }
        // The two string arms exist to price #724's question with per-engine
        // attribution: the comparison suites run Expanse and the competitor in
        // one process, and `perf stat` counts the process, so their counters
        // cannot say which engine's translation misses moved. These run one
        // engine, so they can — and `map_get` above is the `u64` baseline the
        // 4x is measured against, on the same instrument in the same shape of
        // process.
        arm @ ("strmap_get" | "strmap_get_counter") => {
            let shape = if arm == "strmap_get" {
                "short"
            } else {
                "counter"
            };
            let str_keys = build_str_keys(pop, shape);
            let mut map = ExpanseStrMap::new();
            for (i, k) in str_keys.iter().enumerate() {
                map.insert(k, i as u64);
            }
            let probes = build_str_probes(&str_keys, &|k| map.get(k).is_some(), hit_pct, shape);
            sink ^= map.len();
            if run_probe {
                for _ in 0..passes {
                    for k in &probes {
                        // Added, not XORed, for the reason `set_contains`
                        // records: values here are `0..pop`, whose XOR over a
                        // full pass folds back to zero and leaves the probe
                        // phase's checksum identical to the build phase's —
                        // a sink that cannot see the loop it is guarding.
                        // Verified: with `^=` both phases printed 200000.
                        sink = sink.wrapping_add(map.get(black_box(k.as_slice())).unwrap_or(0));
                    }
                }
            }
            // Leaked for the same reason as the integer arms: a teardown walk
            // at exit would land inside the process `perf stat` is counting.
            core::mem::forget(map);
            core::mem::forget(probes);
            core::mem::forget(str_keys);
        }
        other => {
            panic!(
                "EXPANSE_PERF_ARM={other} is not recognised: use `map_get`, `set_contains`, \
                 `strmap_get` or `strmap_get_counter`"
            )
        }
    }

    core::mem::forget(keys);
    // The sink is printed, not discarded: an unconsumed probe loop is a
    // dead-code-elimination candidate (methodology rule 14). The line also
    // echoes the resolved workload shape, so the driver records what ran
    // rather than what it meant to ask for.
    println!("arm={arm} phase={phase} pop={pop} hit_pct={hit_pct} passes={passes} checksum={sink}");
}
