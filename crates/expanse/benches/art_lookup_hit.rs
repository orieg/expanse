//! Pillar 1: ART Point Lookup (100% Hit Rate)
//!
//! Evaluates point lookup performance on 100% hit rate across five key distributions
//! (Uniform Random, Sequential, Clustered, Sparse Stride-2^32, Zipfian theta=0.99)
//! comparing ExpanseMap against blart::TreeMap (Adaptive Radix Tree), BTreeMap, and HashMap.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `art_lookup_hit` |
//! | `group` | 4 |
//! | `population` | 10k to 1M |
//! | `probes_and_reuse` | Batch stream |
//! | `hit_rate` | 100% Hit |
//! | `miss_gen_method` | None |
//! | `value_dereference` | `black_box(*val)` |
//! | `measured_region` | Clean lookup loop |
//! | `arm_symmetry` | Symmetric keys and PRNG |
//! | `statistics` | Median of interleaved rounds |
//! | `verdict` | **PASS** `[verified: CODE READ]`: ART comparison point lookup hit benchmark. |

#[path = "art_common/mod.rs"]
mod art_common;

use art_common::{
    ArtMap, BTreeMap, ExpanseMap, HashMap, XorShift64, art_key, gen_clustered, gen_sequential,
    gen_sparse_stride, gen_uniform_random, gen_zipfian, median,
};
use serde_json::json;
use std::hint::black_box;
use std::time::Instant;

fn bench_dist(dist_name: &str, keys: &[u64], rounds: usize) -> serde_json::Value {
    let n = keys.len();

    // 1. Build structures
    let mut expanse = ExpanseMap::new();
    let mut blart = ArtMap::new();
    let mut btree = BTreeMap::new();
    let mut hash = HashMap::with_capacity(n);

    for &k in keys {
        expanse.insert(k, k.wrapping_mul(3));
        let _ = blart.try_insert(art_key(k), k.wrapping_mul(3));
        btree.insert(k, k.wrapping_mul(3));
        hash.insert(k, k.wrapping_mul(3));
    }

    let mut expanse_times = Vec::with_capacity(rounds);
    let mut blart_times = Vec::with_capacity(rounds);
    let mut btree_times = Vec::with_capacity(rounds);
    let mut hash_times = Vec::with_capacity(rounds);

    for _ in 0..rounds {
        // Expanse
        let start = Instant::now();
        for &k in keys {
            let v = expanse.get(k).unwrap_or(0);
            black_box(v);
        }
        expanse_times.push(start.elapsed().as_nanos() as f64 / n as f64);

        // blart (ART)
        let start = Instant::now();
        for &k in keys {
            let ak = art_key(k);
            let v = blart.get(&ak).copied().unwrap_or(0);
            black_box(v);
        }
        blart_times.push(start.elapsed().as_nanos() as f64 / n as f64);

        // BTreeMap
        let start = Instant::now();
        for &k in keys {
            let v = btree.get(&k).copied().unwrap_or(0);
            black_box(v);
        }
        btree_times.push(start.elapsed().as_nanos() as f64 / n as f64);

        // HashMap
        let start = Instant::now();
        for &k in keys {
            let v = hash.get(&k).copied().unwrap_or(0);
            black_box(v);
        }
        hash_times.push(start.elapsed().as_nanos() as f64 / n as f64);
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
        results.push(bench_dist("sequential", &seq, rounds));

        let clustered = gen_clustered(n, &mut rng);
        results.push(bench_dist("clustered", &clustered, rounds));

        let uniform = gen_uniform_random(n, &mut rng);
        results.push(bench_dist("uniform_random", &uniform, rounds));

        let sparse = gen_sparse_stride(n);
        results.push(bench_dist("sparse_stride", &sparse, rounds));

        let zipf = gen_zipfian(n, 0.99, &mut rng);
        results.push(bench_dist("zipfian", &zipf, rounds));
    }

    let output = json!({
        "benchmark": "art_lookup_hit",
        "workload_id": "art_lookup_hit",
        "quick": quick,
        "results": results,
    });

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("=== ART Benchmark: Point Lookup (100% Hit Rate) ===");
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
