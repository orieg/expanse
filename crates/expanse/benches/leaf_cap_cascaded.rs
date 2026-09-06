//! Deterministic read-path cost of `LEAF_CAP` where the constant changes the
//! structure: `ExpanseSet::contains` and `ExpanseMap::get`, hit and miss, at
//! expanse occupancy λ = 30.52 — the cascaded regime under the shipped
//! `LEAF_CAP = 32` (#715, `docs/ARCHITECTURE.md` §3.5).
//!
//! Why this cell. The committed `core_instructions` arms hold 50k keys
//! (λ = 0.76 on `random`), so no linear leaf ever reaches either cap and a
//! `LEAF_CAP` change cannot move them; the 1M @64 pair of
//! `docs/benchmarks/hot_comparison/METHODOLOGY.md` §9.10.5 (λ = 15.26) builds
//! the same nodes under both caps but for 7 of 65,536 expanses. This harness
//! draws 1,000,000 uniform keys masked to 63 bits, which is arithmetically the
//! 2M @64 cell (λ = N / 2^(63−48) = 1,000,000 / 32,768 = 30.52) at half the
//! build cost. Under `LEAF_CAP = 32` about 35% of the 32,768 two-byte
//! expanses — holding 42% of the keys — have cascaded into a bitmap branch of
//! single-key immediates (`scripts/density_poisson.py`; the 2M @64 census at
//! the same λ counts 35.05%); under a `LEAF_CAP = 48` build all but ~0.1% of
//! them are still a packed linear leaf of up to 48 keys, and the memory sweep
//! puts the two builds at 13.60 and 6.99 B/key on the set (`density_sweep`,
//! same generator and seed). What a lookup costs in each of those structures
//! is the number this harness produces.
//!
//! The same generator and seed as `examples/keyspace_density.rs`, so the
//! structure under measurement is byte-for-byte the one the memory cells
//! describe. Miss probes are drawn from the same generator at the same width
//! under a second seed and rejected on membership (AGENTS.md §8.6): a miss
//! shares the population's prefix distribution and terminates at the same
//! depth a hit does, which a fixed transform of a present key would not.
//!
//! Build and probe generation run in `setup`, outside the measured region, and
//! the structure is leaked rather than dropped (a teardown walk is a different
//! code path measured under a lookup label — see `instructions.rs`).
//!
//! The cap-48 cell is produced by a build-time patch of
//! `crates/expanse/src/types.rs` (`pub const LEAF_CAP: usize = 48;`), never by a
//! default change; the harness itself is cap-agnostic and reports whatever the
//! build it was compiled into does.
//!
//! Requires valgrind, which does not support arm64 macOS — runs on Linux:
//! `cargo bench -p expanse-trie --bench leaf_cap_cascaded`.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `core_leaf_cap_cascaded_instructions` |
//! | `group` | 2 |
//! | `population` | 1,000,000 uniform random keys masked to 63 bits (λ = N / 2^(63−48) = 30.52; the 2M @64 cell at half the N) |
//! | `probes_and_reuse` | 1,000,000 hit probes (the population, fixed-seed Fisher-Yates shuffle) and 1,000,000 miss probes, each probed once; reuse 1.0 |
//! | `hit_rate` | 100% on the `hit` arms, 0% on the `miss` arms (separate arms, so each Ir/probe is one descent shape) |
//! | `miss_gen_method` | Rejection-sampled from the same XorShift64 generator at the same 63-bit width under a second seed, rejected on membership and deduplicated (§8.6) |
//! | `value_dereference` | `hits += contains(k)` / `sink ^= get(k).unwrap_or(0)`, both through `black_box` |
//! | `measured_region` | Clean (build, shuffle and miss sampling in `setup`; structure leaked, not dropped) |
//! | `arm_symmetry` | Set and map arms probe bit-identical key vectors; across a cap-32 / cap-48 pair the only variable is `LEAF_CAP` (build-time patch) |
//! | `statistics` | iai Callgrind exact counts |
//! | `verdict` | **PASS** `[verified: RUN (c81eaf5d, x86_64 dev host, Callgrind in a container)]`: Cascaded-regime read path; the cap-32 / cap-48 pair is published in `docs/benchmarks/hot_comparison/results/leaf_cap_cascaded_callgrind.json`. |

// The `library_benchmark` macro expands to modules that carry no docs of
// their own; the workspace `missing_docs` lint does not apply to a bench
// harness.
#![allow(missing_docs)]

use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
#[cfg(target_os = "linux")]
use iai_callgrind::main;
use iai_callgrind::{
    Callgrind, LibraryBenchmarkConfig, library_benchmark, library_benchmark_group,
};
use std::collections::HashSet;
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

/// The seed every committed random cell in this repository draws from
/// (`examples/keyspace_density.rs`, `instructions.rs`).
const SEED: u64 = 0x0DDB_1A5E_5EED_0001;
/// Second stream for the absent keys — same generator, different seed
/// (the `miss_keys` reference form in `expanse-capi/examples/bench_vs_libjudy.rs`).
const MISS_SEED: u64 = 0x51ED_0FF5_C0FF_EE01;
/// Fixed-seed shuffle of the hit probes, so probe order is not build order
/// (a sequential probe would measure the prefetcher, not the lookup).
const SHUFFLE_SEED: u64 = 0x9E37_79B9;

/// Population and keyspace width. 1,000,000 draws at 63 bits put
/// λ = N / 2^(BITS − 48) = 30.52 keys per two-byte expanse — past the shipped
/// `LEAF_CAP = 32` on ~39% of expanses (Poisson), under a cap of 48 on all
/// but a handful.
const POP: usize = 1_000_000;
const BITS: u32 = 63;
const MASK: u64 = (1u64 << BITS) - 1;

/// The population, in generator order — the order the density sweep inserts.
fn keys() -> Vec<u64> {
    let mut rng = XorShift(SEED);
    (0..POP).map(|_| rng.next() & MASK).collect()
}

fn shuffled(mut v: Vec<u64>) -> Vec<u64> {
    let mut rng = XorShift(SHUFFLE_SEED);
    for i in (1..v.len()).rev() {
        v.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    v
}

/// `n` distinct keys absent from `present`, drawn from the same generator at
/// the same width and rejected on membership (AGENTS.md §8.6). Bounded so a
/// stream that cannot yield enough absent keys aborts loudly.
fn miss_keys(present: &HashSet<u64>, n: usize) -> Vec<u64> {
    let mut rng = XorShift(MISS_SEED);
    let mut seen: HashSet<u64> = HashSet::with_capacity(n * 2);
    let mut out = Vec::with_capacity(n);
    let budget = n.saturating_mul(64).saturating_add(1024);
    for _ in 0..budget {
        if out.len() == n {
            return out;
        }
        let c = rng.next() & MASK;
        if !present.contains(&c) && seen.insert(c) {
            out.push(c);
        }
    }
    panic!("could not draw {n} distinct absent keys within budget");
}

/// The probe vector for `case`: the shuffled population for `hit`, the
/// rejection-sampled absent keys for `miss`. Identical for the set and map
/// arms, so the two flavors are measured over the same descents.
fn probes(case: &str, ks: &[u64]) -> Vec<u64> {
    match case {
        "hit" => shuffled(ks.to_vec()),
        "miss" => {
            let present: HashSet<u64> = ks.iter().copied().collect();
            miss_keys(&present, POP)
        }
        other => panic!("unknown probe case {other}"),
    }
}

fn built_set(case: &str) -> (ExpanseSet, Vec<u64>) {
    let ks = keys();
    let mut set = ExpanseSet::new();
    for &k in &ks {
        set.insert(k);
    }
    let probes = probes(case, &ks);
    (set, probes)
}

fn built_map(case: &str) -> (ExpanseMap, Vec<u64>) {
    let ks = keys();
    let mut map = ExpanseMap::new();
    for &k in &ks {
        map.insert(k, !k);
    }
    let probes = probes(case, &ks);
    (map, probes)
}

#[library_benchmark]
#[bench::hit(args = ("hit",), setup = built_set)]
#[bench::miss(args = ("miss",), setup = built_set)]
fn cascaded_set_contains(built: (ExpanseSet, Vec<u64>)) -> u64 {
    let (set, probes) = built;
    let mut hits = 0u64;
    for &k in &probes {
        hits += u64::from(set.contains(black_box(k)));
    }
    // Leaked: dropping here would count a `free_subtree` walk under a
    // lookup label.
    core::mem::forget(set);
    black_box(hits)
}

#[library_benchmark]
#[bench::hit(args = ("hit",), setup = built_map)]
#[bench::miss(args = ("miss",), setup = built_map)]
fn cascaded_map_get(built: (ExpanseMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for &k in &probes {
        sink ^= map.get(black_box(k)).unwrap_or(0);
    }
    // Leaked — see `cascaded_set_contains`.
    core::mem::forget(map);
    black_box(sink)
}

/// Same instrument configuration as `instructions.rs`: cache simulation on,
/// branch-predictor simulation opt-in through `EXPANSE_BRANCH_SIM=1`, and an
/// unrecognised value fatal rather than ignored (AGENTS.md §8.1).
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

library_benchmark_group!(
    name = cascaded;
    benchmarks = cascaded_set_contains, cascaded_map_get
);

#[cfg(target_os = "linux")]
main!(config = bench_config(); library_benchmark_groups = cascaded);

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("iai-callgrind instruction benchmarks run on Linux only.");
}
