//! Pillar 2: ART Point Lookup (50% Hit / 50% Miss Rate)
//!
//! Evaluates point lookup performance on 50% hit / 50% miss rate with rejection-sampled
//! absent keys per AGENTS.md §8.6 across five key distributions comparing ExpanseMap
//! against blart::TreeMap (Adaptive Radix Tree), BTreeMap, and HashMap.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `art_lookup_miss` |
//! | `group` | 4 |
//! | `population` | 10k to 1M |
//! | `probes_and_reuse` | 50/50 mixed probe stream |
//! | `hit_rate` | 50% Hit / 50% Miss |
//! | `miss_gen_method` | Rejection sampling |
//! | `value_dereference` | `black_box(*val)` |
//! | `measured_region` | Clean lookup loop |
//! | `arm_symmetry` | Symmetric keys and PRNG |
//! | `statistics` | Median of interleaved rounds |
//! | `verdict` | **PASS** `[verified: CODE READ]`: ART comparison point lookup 50/50 miss benchmark. |

#[path = "art_common/mod.rs"]
mod art_common;

use art_common::{
    ArtMap, BTreeMap, ExpanseMap, HashMap, XorShift64, art_key, gen_clustered,
    gen_rejection_misses, gen_sequential, gen_sparse_stride, gen_uniform_random, gen_zipfian,
    median,
};
use serde_json::json;
use std::hint::black_box;
use std::time::Instant;

fn bench_dist_50_50(
    dist_name: &str,
    present_keys: &[u64],
    rounds: usize,
    rng: &mut XorShift64,
) -> serde_json::Value {
    let n = present_keys.len();

    // 1. Build structures
    let mut expanse = ExpanseMap::new();
    let mut blart = ArtMap::new();
    let mut btree = BTreeMap::new();
    let mut hash = HashMap::with_capacity(n);

    for &k in present_keys {
        expanse.insert(k, k.wrapping_mul(3));
        let _ = blart.try_insert(art_key(k), k.wrapping_mul(3));
        btree.insert(k, k.wrapping_mul(3));
        hash.insert(k, k.wrapping_mul(3));
    }

    // 2. Generate rejection-sampled absent keys
    let miss_keys = gen_rejection_misses(present_keys, n, rng);

    // 3. Build interleaved 50/50 probe stream
    let mut probes = Vec::with_capacity(2 * n);
    for i in 0..n {
        probes.push(present_keys[i]);
        probes.push(miss_keys[i]);
    }
    let total_probes = probes.len();

    let mut expanse_times = Vec::with_capacity(rounds);
    let mut blart_times = Vec::with_capacity(rounds);
    let mut btree_times = Vec::with_capacity(rounds);
    let mut hash_times = Vec::with_capacity(rounds);

    for _ in 0..rounds {
        // Expanse
        let start = Instant::now();
        for &k in &probes {
            let v = expanse.get(k).unwrap_or(0);
            black_box(v);
        }
        expanse_times.push(start.elapsed().as_nanos() as f64 / total_probes as f64);

        // blart (ART)
        let start = Instant::now();
        for &k in &probes {
            let ak = art_key(k);
            let v = blart.get(&ak).copied().unwrap_or(0);
            black_box(v);
        }
        blart_times.push(start.elapsed().as_nanos() as f64 / total_probes as f64);

        // BTreeMap
        let start = Instant::now();
        for &k in &probes {
            let v = btree.get(&k).copied().unwrap_or(0);
            black_box(v);
        }
        btree_times.push(start.elapsed().as_nanos() as f64 / total_probes as f64);

        // HashMap
        let start = Instant::now();
        for &k in &probes {
            let v = hash.get(&k).copied().unwrap_or(0);
            black_box(v);
        }
        hash_times.push(start.elapsed().as_nanos() as f64 / total_probes as f64);
    }

    let exp_med = median(expanse_times);
    let blart_med = median(blart_times);
    let btree_med = median(btree_times);
    let hash_med = median(hash_times);

    json!({
        "distribution": dist_name,
        "population": n,
        "expanse_ns_op": exp_med,
        "blart_art_ns_op": blart_med,
        "btree_ns_op": btree_med,
        "hashmap_ns_op": hash_med,
        "ratio_vs_art": if blart_med > 0.0 { exp_med / blart_med } else { 1.0 },
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    let populations: &[usize] = if quick {
        &[10_000, 50_000]
    } else {
        &[10_000, 100_000, 1_000_000]
    };
    let rounds = if quick { 3 } else { 7 };

    let mut results = Vec::new();
    let mut rng = XorShift64::new(0);

    for &n in populations {
        let seq = gen_sequential(n);
        results.push(bench_dist_50_50("sequential", &seq, rounds, &mut rng));

        let clustered = gen_clustered(n, &mut rng);
        results.push(bench_dist_50_50("clustered", &clustered, rounds, &mut rng));

        let uniform = gen_uniform_random(n, &mut rng);
        results.push(bench_dist_50_50(
            "uniform_random",
            &uniform,
            rounds,
            &mut rng,
        ));

        let sparse = gen_sparse_stride(n);
        results.push(bench_dist_50_50("sparse_stride", &sparse, rounds, &mut rng));

        let zipf = gen_zipfian(n, 0.99, &mut rng);
        results.push(bench_dist_50_50("zipfian", &zipf, rounds, &mut rng));
    }

    let output = json!({
        "benchmark": "art_lookup_miss",
        "workload_id": "art_lookup_miss",
        "quick": quick,
        "results": results,
    });

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("=== ART Benchmark: Point Lookup (50% Hit / 50% Miss Rate) ===");
        for res in results {
            println!(
                "pop: {:>7} | dist: {:<14} | Expanse: {:>5.1} ns | blart: {:>5.1} ns | BTree: {:>5.1} ns | Hash: {:>5.1} ns | ratio: {:>4.2}x",
                res["population"],
                res["distribution"].as_str().unwrap(),
                res["expanse_ns_op"].as_f64().unwrap(),
                res["blart_art_ns_op"].as_f64().unwrap(),
                res["btree_ns_op"].as_f64().unwrap(),
                res["hashmap_ns_op"].as_f64().unwrap(),
                res["ratio_vs_art"].as_f64().unwrap(),
            );
        }
    }
}
