//! Dependent node visits per probe diagnostic harness (Refs #1249).
//!
//! # Diagnostic disclosure
//!
//! **NOT A GATE.** This metric models the length of a probe's chain of
//! dependent memory loads (branch edges followed + BranchB subarray loads +
//! leaf loads). For a lookup that misses CPU cache, this chain length is what
//! sets latency. However, this count is diagnostic output only and is **NOT
//! CITABLE** as a performance claim until compared against measured wall-clock
//! latency across the `map_get` distributions on the reference host.
//!
//! # Usage
//!
//! ```bash
//! cargo run --release -p expanse-trie --example probe_visits
//! ```
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `example_probe_visits` |
//! | `group` | 5 |
//! | `population` | 50,000 keys per distribution (`sequential`, `random`, `clustered`, `dense_leaf`, `linear_leaf`) matching `instructions.rs` |
//! | `insertion_order` | generator — keys generated and inserted in deterministic generator order |
//! | `probes_and_reuse` | 50,000 probes per distribution, 100% of inserted population probed |
//! | `hit_rate` | 100% hits |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | walker validates value match against `get(k)` |
//! | `measured_region` | Clean |
//! | `arm_symmetry` | Diagnostic walker mirroring `get` traversal |
//! | `statistics` | Exact deterministic load counts (mean, min, max, depth histogram) |
//! | `verdict` | **PENDING**: diagnostic instrument for #1249; NOT a performance gate and not citable until compared with reference-host wall clock. |

use expanse_trie::map::ExpanseMap;

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

/// Population matching `crates/expanse/benches/instructions.rs`.
const POP: usize = 50_000;

fn keys(dist: &str) -> Vec<u64> {
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut out = Vec::with_capacity(POP);
    match dist {
        "sequential" => out.extend(0..POP as u64),
        "random" => out.extend((0..POP).map(|_| rng.next())),
        "clustered" => {
            let mut base = 0;
            for i in 0..POP as u64 {
                if i % 256 == 0 {
                    base = rng.next() & !0xFF;
                }
                out.push(base + (i % 256));
            }
        }
        "dense_leaf" => {
            for _ in 0..(POP / 32) {
                let prefix = rng.next() & !0xFF;
                for j in 0..32 {
                    out.push(prefix | (j as u64));
                }
            }
        }
        "linear_leaf" => {
            for _ in 0..(POP / 15) {
                let prefix = rng.next() & !0xFF;
                for j in 0..15 {
                    out.push(prefix | (j as u64));
                }
            }
        }
        other => panic!("unknown distribution {other}"),
    }
    out
}

fn built_map(dist: &str) -> (ExpanseMap, Vec<u64>) {
    let ks = keys(dist);
    let mut map = ExpanseMap::new();
    for &k in &ks {
        map.insert(k, !k);
    }
    let mut probes = ks;
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    (map, probes)
}

fn analyze_distribution(dist: &str) {
    let (map, probes) = built_map(dist);
    let n = probes.len();

    let mut sum_edges = 0usize;
    let mut sum_branch_b = 0usize;
    let mut sum_leaf = 0usize;
    let mut sum_total = 0usize;
    let mut min_total = usize::MAX;
    let mut max_total = 0usize;
    let mut histogram = [0usize; 16];

    for &k in &probes {
        let v = map.probe_visits(k);
        debug_assert_eq!(v.value, Some(!k), "walker must agree with map content");
        debug_assert!(v.found, "probe key must be found");
        let tot = v.total_visits();

        sum_edges += v.edges_followed;
        sum_branch_b += v.branch_b_subarrays;
        sum_leaf += v.leaf_loads;
        sum_total += tot;

        min_total = min_total.min(tot);
        max_total = max_total.max(tot);
        if tot < histogram.len() {
            histogram[tot] += 1;
        }
    }

    let mean_edges = sum_edges as f64 / n as f64;
    let mean_branch_b = sum_branch_b as f64 / n as f64;
    let mean_leaf = sum_leaf as f64 / n as f64;
    let mean_total = sum_total as f64 / n as f64;

    println!(
        "{:<14} | {:>6} | {:>10.2} | {:>12.2} | {:>10.2} | {:>11.2} | {:>4}–{:<4}",
        dist, n, mean_total, mean_edges, mean_branch_b, mean_leaf, min_total, max_total
    );

    print!("  depth histogram: ");
    for (depth, &count) in histogram.iter().enumerate() {
        if count > 0 {
            print!("{depth}: {:.1}%  ", (count as f64 / n as f64) * 100.0);
        }
    }
    println!();
}

fn main() {
    println!("==========================================================================");
    println!("DIAGNOSTIC INSTRUMENT (Issue #1249) — NOT A GATE");
    println!("Dependent node visits per probe across map_get distributions (POP = 50,000)");
    println!("Chain length = edges_followed + BranchB_subarrays + leaf_loads");
    println!("NOT CITABLE until compared with reference-host wall clock.");
    println!("==========================================================================");
    println!(
        "{:<14} | {:>6} | {:>10} | {:>12} | {:>10} | {:>11} | {:>9}",
        "distribution",
        "probes",
        "mean visits",
        "mean edges",
        "mean BranchB",
        "mean leaves",
        "range"
    );
    println!(
        "{:-<14}-+-{:-<6}-+-{:-<10}-+-{:-<12}-+-{:-<10}-+-{:-<11}-+-{:-<9}",
        "", "", "", "", "", "", ""
    );

    for dist in &[
        "sequential",
        "random",
        "clustered",
        "dense_leaf",
        "linear_leaf",
    ] {
        analyze_distribution(dist);
    }
    println!("==========================================================================");
}
