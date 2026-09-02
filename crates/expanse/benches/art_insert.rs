//! Pillar 3: ART Dynamic Insertion Throughput
//!
//! Evaluates dynamic insertion throughput into cold/empty structures across five key
//! distributions comparing ExpanseMap against blart::TreeMap (Adaptive Radix Tree),
//! BTreeMap, and HashMap. Measures only insertion loop, isolating teardown/drop.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `art_insert` |
//! | `group` | 4 |
//! | `population` | 10k to 1M |
//! | `probes_and_reuse` | Insertion stream |
//! | `hit_rate` | Dynamic Growth |
//! | `miss_gen_method` | None |
//! | `value_dereference` | Insertion into container |
//! | `measured_region` | Clean insertion loop |
//! | `arm_symmetry` | Symmetric keys and PRNG |
//! | `statistics` | Median of interleaved rounds |
//! | `verdict` | **PASS** `[verified: CODE READ]`: ART comparison insertion throughput benchmark. |

#[path = "art_common/mod.rs"]
mod art_common;

use art_common::{
    ArtMap, BTreeMap, ExpanseMap, HashMap, XorShift64, art_key, gen_clustered, gen_sequential,
    gen_sparse_stride, gen_uniform_random, gen_zipfian, median,
};
use serde_json::json;
use std::hint::black_box;
use std::time::Instant;

fn bench_dist_insert(dist_name: &str, keys: &[u64], rounds: usize) -> serde_json::Value {
    let n = keys.len();

    let mut expanse_times = Vec::with_capacity(rounds);
    let mut blart_times = Vec::with_capacity(rounds);
    let mut btree_times = Vec::with_capacity(rounds);
    let mut hash_times = Vec::with_capacity(rounds);

    for _ in 0..rounds {
        // Expanse
        let mut expanse = ExpanseMap::new();
        let start = Instant::now();
        for &k in keys {
            expanse.insert(k, k.wrapping_mul(3));
        }
        let elapsed = start.elapsed().as_nanos() as f64 / n as f64;
        black_box(&expanse);
        expanse_times.push(elapsed);
        drop(expanse);

        // blart (ART)
        let mut blart = ArtMap::new();
        let start = Instant::now();
        for &k in keys {
            let _ = blart.try_insert(art_key(k), k.wrapping_mul(3));
        }
        let elapsed = start.elapsed().as_nanos() as f64 / n as f64;
        black_box(&blart);
        blart_times.push(elapsed);
        drop(blart);

        // BTreeMap
        let mut btree = BTreeMap::new();
        let start = Instant::now();
        for &k in keys {
            btree.insert(k, k.wrapping_mul(3));
        }
        let elapsed = start.elapsed().as_nanos() as f64 / n as f64;
        black_box(&btree);
        btree_times.push(elapsed);
        drop(btree);

        // HashMap
        let mut hash = HashMap::with_capacity(n);
        let start = Instant::now();
        for &k in keys {
            hash.insert(k, k.wrapping_mul(3));
        }
        let elapsed = start.elapsed().as_nanos() as f64 / n as f64;
        black_box(&hash);
        hash_times.push(elapsed);
        drop(hash);
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
        results.push(bench_dist_insert("sequential", &seq, rounds));

        let clustered = gen_clustered(n, &mut rng);
        results.push(bench_dist_insert("clustered", &clustered, rounds));

        let uniform = gen_uniform_random(n, &mut rng);
        results.push(bench_dist_insert("uniform_random", &uniform, rounds));

        let sparse = gen_sparse_stride(n);
        results.push(bench_dist_insert("sparse_stride", &sparse, rounds));

        let zipf = gen_zipfian(n, 0.99, &mut rng);
        results.push(bench_dist_insert("zipfian", &zipf, rounds));
    }

    let output = json!({
        "benchmark": "art_insert",
        "workload_id": "art_insert",
        "quick": quick,
        "results": results,
    });

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("=== ART Benchmark: Dynamic Insertion Throughput ===");
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
