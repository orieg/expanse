//! Memory-footprint measurement per `docs/BENCHMARKING.md`: bytes/key at
//! population checkpoints, per key-distribution class, from the engine's
//! own byte-exact allocation accounting (`mem_used`). Deterministic — no
//! timing involved, so it is immune to machine load.
//!
//! Run: `cargo run --release -p expanse-trie --example bytes_per_key`
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `example_bytes_per_key` |
//! | `group` | 5 |
//! | `population` | 10k to 1M |
//! | `probes_and_reuse` | N/A (Memory) |
//! | `hit_rate` | N/A |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | `mem_used()` accounting |
//! | `measured_region` | Clean |
//! | `arm_symmetry` | Pure 64-bit census |
//! | `statistics` | Exact byte count |
//! | `verdict` | **PASS** `[verified: RUN (6c63826a)]`: Deterministic memory density census. |

use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use expanse_trie::types::LEAF_CAP;

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

fn keys(dist: &str, n: usize) -> Vec<u64> {
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut out = Vec::with_capacity(n);
    match dist {
        "sequential" => out.extend(0..n as u64),
        "random" => out.extend((0..n).map(|_| rng.next())),
        "clustered" => {
            let mut base = 0;
            for i in 0..n as u64 {
                if i % 256 == 0 {
                    base = rng.next() & !0xFF;
                }
                out.push(base + (i % 256));
            }
        }
        "sparse" => out.extend((0..n as u64).map(|i| i << 40)),
        // Runs of 4096 span two key bytes: exercises branch-targeted
        // narrow pointers (divergence level 2), not just leaf-targeted.
        "clustered-wide" => {
            let mut base = 0;
            for i in 0..n as u64 {
                if i % 4096 == 0 {
                    base = rng.next() & !0xFFF;
                }
                out.push(base + (i % 4096));
            }
        }
        _ => unreachable!(),
    }
    out
}

fn main() {
    println!("bytes/key by distribution and population (set flavor / map flavor)");
    println!("target from docs/ARCHITECTURE.md: < 9.5 B/key dense+clustered (set)\n");
    println!(
        "{:<15} {:>10} {:>14} {:>14}",
        "dist", "pop", "set B/key", "map B/key"
    );
    for dist in [
        "sequential",
        "random",
        "clustered",
        "clustered-wide",
        "sparse",
    ] {
        for pop in [1_000usize, 100_000, 1_000_000] {
            let ks = keys(dist, pop);
            let mut set = ExpanseSet::new();
            let mut map = ExpanseMap::new();
            for &k in &ks {
                set.insert(k);
                map.insert(k, !k);
            }
            let (sl, ml) = (set.len().max(1), map.len().max(1));
            println!(
                "{:<15} {:>10} {:>14.2} {:>14.2}",
                dist,
                pop,
                set.mem_used() as f64 / sl as f64,
                map.mem_used() as f64 / ml as f64,
            );
        }
    }
    println!("\n(map B/key includes the 8-byte value per key)");

    // Regression gate (CI `memory-budget` job). These are deterministic
    // allocator-accounting numbers — exact regardless of machine load —
    // so unlike the timing tables they can gate a build. Ceilings sit a
    // little above the measured values in docs/BENCHMARKING.md: they
    // catch structural regressions (a compression path stopping firing),
    // not the last few percent. Raise one deliberately, with the
    // BENCHMARKING.md row updated in the same commit.
    //
    // The `random` ceiling is population-specific, and not in the way the
    // numbers suggest. Per-key cost on uniform keys is a sawtooth in expanse
    // occupancy λ = N / 2^16 — the mean population of a 2-byte-prefix
    // expanse — with its tooth at λ ≈ LEAF_CAP, where the linear leaf
    // cascades into a branch of single-key immediates (ARCHITECTURE.md §3.5).
    // N = 1_000_000 is λ = 15.26, 48% of LEAF_CAP, in the trough of that
    // curve: 7.92 B/key against a 9.00 ceiling reads as 14% of headroom in
    // bytes, but the real headroom is about 2× in N. The same generator at
    // N = 2_000_000 (λ = 30.5, 95% of LEAF_CAP, ~39% of expanses cascaded)
    // measures 13.60 set / 19.38 map and breaches both ceilings with no code
    // change. Changing this row's N therefore means re-deriving its ceiling
    // from the curve, never raising the number until the gate is green. The
    // other four rows have occupancy fixed by their generators and do not
    // move with N. Keyspace width is the same knob: masking one top key bit
    // halves 2^16 and is exactly a doubling of N.
    let budget: &[(&str, usize, f64, f64)] = &[
        // dist, pop, set ceiling, map ceiling
        ("sequential", 1_000_000, 0.10, 9.00),
        ("clustered", 1_000_000, 0.50, 9.00),
        ("clustered-wide", 1_000_000, 0.30, 9.00),
        ("random", 1_000_000, 9.00, 18.00),
        ("sparse", 1_000_000, 17.00, 17.00),
    ];
    let mut over = Vec::new();
    for &(dist, pop, set_max, map_max) in budget {
        if dist == "random" {
            // Print the density the ceiling was calibrated at, so a change to
            // `pop` is visible as a move along the curve in the job log.
            let lambda = pop as f64 / 65_536.0;
            println!(
                "random gate density: N = {pop}, λ = N / 2^16 = {lambda:.2} \
                 ({:.0}% of LEAF_CAP = {LEAF_CAP}); the ceiling is valid at this \
                 λ only — see docs/ARCHITECTURE.md §3.5",
                100.0 * lambda / LEAF_CAP as f64
            );
        }
        let ks = keys(dist, pop);
        let mut set = ExpanseSet::new();
        let mut map = ExpanseMap::new();
        for &k in &ks {
            set.insert(k);
            map.insert(k, !k);
        }
        let sb = set.mem_used() as f64 / set.len().max(1) as f64;
        let mb = map.mem_used() as f64 / map.len().max(1) as f64;
        if sb > set_max {
            over.push(format!("{dist} set {sb:.2} > {set_max:.2} B/key"));
        }
        if mb > map_max {
            over.push(format!("{dist} map {mb:.2} > {map_max:.2} B/key"));
        }
    }
    if over.is_empty() {
        println!("memory budget: all distributions within ceilings");
    } else {
        for line in &over {
            println!("MEMORY BUDGET EXCEEDED: {line}");
        }
        std::process::exit(1);
    }
}
