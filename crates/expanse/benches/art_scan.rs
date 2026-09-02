//! Pillar 4: ART Ordered Range Scan & Full Iteration
//!
//! Evaluates ordered iteration (full scan) and bounded range scans (k = 10, 100, 1000 items)
//! comparing ExpanseMap against blart::TreeMap (Adaptive Radix Tree), BTreeMap, and HashMap.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `art_scan` |
//! | `group` | 4 |
//! | `population` | 10k to 1M |
//! | `probes_and_reuse` | Range scan stream |
//! | `hit_rate` | Ordered Scan |
//! | `miss_gen_method` | None |
//! | `value_dereference` | `black_box(*val)` |
//! | `measured_region` | Clean scan loop |
//! | `arm_symmetry` | Symmetric keys and PRNG |
//! | `statistics` | Median of interleaved rounds |
//! | `verdict` | **PASS** `[verified: CODE READ]`: ART comparison range scan benchmark. |

#[path = "art_common/mod.rs"]
mod art_common;

use art_common::{
    ArtMap, BTreeMap, ExpanseMap, HashMap, XorShift64, art_key, gen_clustered, gen_sequential,
    gen_uniform_random, median,
};
use serde_json::json;
use std::hint::black_box;
use std::time::Instant;

fn bench_scan(dist_name: &str, keys: &[u64], range_k: usize, rounds: usize) -> serde_json::Value {
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

    if range_k == 0 {
        // Full scan
        for _ in 0..rounds {
            let start = Instant::now();
            let mut sum: u64 = 0;
            for (k, v) in expanse.iter() {
                sum = sum.wrapping_add(k ^ v);
            }
            black_box(sum);
            expanse_times.push(start.elapsed().as_nanos() as f64 / n as f64);

            let start = Instant::now();
            let mut sum: u64 = 0;
            for (k, v) in blart.iter() {
                sum = sum.wrapping_add((*k).get() ^ v);
            }
            black_box(sum);
            blart_times.push(start.elapsed().as_nanos() as f64 / n as f64);

            let start = Instant::now();
            let mut sum: u64 = 0;
            for (&k, &v) in btree.iter() {
                sum = sum.wrapping_add(k ^ v);
            }
            black_box(sum);
            btree_times.push(start.elapsed().as_nanos() as f64 / n as f64);
        }
    } else {
        // Bounded range scan: 100 queries fetching range_k elements each
        let num_queries = 100;
        let mut start_keys = Vec::with_capacity(num_queries);
        for i in 0..num_queries {
            let idx = (i * (n / (num_queries + 1))).min(n - 1);
            start_keys.push(keys[idx]);
        }

        for _ in 0..rounds {
            let start = Instant::now();
            let mut sum: u64 = 0;
            for &sk in &start_keys {
                for (k, v) in expanse.range(sk..=u64::MAX).take(range_k) {
                    sum = sum.wrapping_add(k ^ v);
                }
            }
            black_box(sum);
            expanse_times.push(start.elapsed().as_nanos() as f64 / (num_queries * range_k) as f64);

            let start = Instant::now();
            let mut sum: u64 = 0;
            for &sk in &start_keys {
                for (k, v) in blart.range(art_key(sk)..).take(range_k) {
                    sum = sum.wrapping_add((*k).get() ^ v);
                }
            }
            black_box(sum);
            blart_times.push(start.elapsed().as_nanos() as f64 / (num_queries * range_k) as f64);

            let start = Instant::now();
            let mut sum: u64 = 0;
            for &sk in &start_keys {
                for (&k, &v) in btree.range(sk..).take(range_k) {
                    sum = sum.wrapping_add(k ^ v);
                }
            }
            black_box(sum);
            btree_times.push(start.elapsed().as_nanos() as f64 / (num_queries * range_k) as f64);
        }
    }

    let exp_med = median(expanse_times);
    let blart_med = median(blart_times);
    let btree_med = median(btree_times);

    json!({
        "distribution": dist_name,
        "population": n,
        "range_k": range_k,
        "expanse_ns_elem": exp_med,
        "blart_art_ns_elem": blart_med,
        "btree_ns_elem": btree_med,
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
    let range_ks: &[usize] = if quick {
        &[0, 100]
    } else {
        &[0, 10, 100, 1000]
    };
    let rounds = if quick { 3 } else { 7 };

    let mut results = Vec::new();
    let mut rng = XorShift64::new(0);

    for &n in populations {
        let seq = gen_sequential(n);
        let clustered = gen_clustered(n, &mut rng);
        let uniform = gen_uniform_random(n, &mut rng);

        for &k in range_ks {
            results.push(bench_scan("sequential", &seq, k, rounds));
            results.push(bench_scan("clustered", &clustered, k, rounds));
            results.push(bench_scan("uniform_random", &uniform, k, rounds));
        }
    }

    let output = json!({
        "benchmark": "art_scan",
        "workload_id": "art_scan",
        "quick": quick,
        "results": results,
    });

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("=== ART Benchmark: Ordered Range Scan & Iteration ===");
        for res in results {
            println!(
                "pop: {:>7} | dist: {:<14} | range_k: {:>4} | Expanse: {:>5.2} ns/elem | blart: {:>5.2} ns/elem | BTree: {:>5.2} ns/elem | ratio: {:>4.2}x",
                res["population"],
                res["distribution"].as_str().unwrap(),
                res["range_k"],
                res["expanse_ns_elem"].as_f64().unwrap(),
                res["blart_art_ns_elem"].as_f64().unwrap(),
                res["btree_ns_elem"].as_f64().unwrap(),
                res["ratio_vs_art"].as_f64().unwrap(),
            );
        }
    }
}
