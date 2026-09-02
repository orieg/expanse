//! Pillar 3: ART Dynamic Growth Insertion Throughput
//!
//! Evaluates dynamic growth insertion performance from cold (empty container) to full population
//! across five key distributions, measuring ns/op for ExpanseMap vs blart (ART), BTreeMap, and HashMap.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `art_insert` |
//! | `group` | 4 |
//! | `population` | 10k to 1M |
//! | `probes_and_reuse` | Fresh cold build per round |
//! | `hit_rate` | 0% (Fresh distinct insertions) |
//! | `miss_gen_method` | None |
//! | `value_dereference` | Insertion return value `black_box`'d |
//! | `measured_region` | Cold build loop |
//! | `arm_symmetry` | All containers grow cold from empty; identical key sequences |
//! | `statistics` | Median + BCa 95% Bootstrap CI over paired rounds |
//! | `verdict` | **PASS** `[verified: CODE READ]`: ART comparison dynamic insertion benchmark. |

#[path = "art_common/mod.rs"]
mod art_common;

use art_common::{
    ArtMap, BTreeMap, ExpanseMap, HashMap, XorShift64, art_key, bca_ci, dedupe_preserve_order,
    gen_clustered, gen_sequential, gen_sparse_stride, gen_uniform_random, gen_zipfian, median,
};
use serde_json::json;
use std::hint::black_box;
use std::time::Instant;

fn bench_dist(dist_name: &str, raw_keys: &[u64], rounds: usize) -> serde_json::Value {
    // For associative containers, dynamic insertion throughput is measured over unique keys
    let keys = if dist_name == "zipfian" {
        dedupe_preserve_order(raw_keys)
    } else {
        raw_keys.to_vec()
    };
    let n = keys.len();

    let mut expanse_times = Vec::with_capacity(rounds);
    let mut blart_times = Vec::with_capacity(rounds);
    let mut btree_times = Vec::with_capacity(rounds);
    let mut hash_times = Vec::with_capacity(rounds);
    let mut paired_ratios = Vec::with_capacity(rounds);

    for round in 0..rounds {
        let (e_ns, b_ns, bt_ns, h_ns) = match round % 4 {
            0 => {
                let e = time_expanse_insert(&keys);
                let b = time_blart_insert(&keys);
                let bt = time_btree_insert(&keys);
                let h = time_hash_insert(&keys);
                (e, b, bt, h)
            }
            1 => {
                let b = time_blart_insert(&keys);
                let bt = time_btree_insert(&keys);
                let h = time_hash_insert(&keys);
                let e = time_expanse_insert(&keys);
                (e, b, bt, h)
            }
            2 => {
                let bt = time_btree_insert(&keys);
                let h = time_hash_insert(&keys);
                let e = time_expanse_insert(&keys);
                let b = time_blart_insert(&keys);
                (e, b, bt, h)
            }
            _ => {
                let h = time_hash_insert(&keys);
                let e = time_expanse_insert(&keys);
                let b = time_blart_insert(&keys);
                let bt = time_btree_insert(&keys);
                (e, b, bt, h)
            }
        };

        expanse_times.push(e_ns);
        blart_times.push(b_ns);
        btree_times.push(bt_ns);
        hash_times.push(h_ns);
        if b_ns > 0.0 {
            paired_ratios.push(e_ns / b_ns);
        }
    }

    let exp_med = median(expanse_times.clone());
    let blart_med = median(blart_times.clone());
    let btree_med = median(btree_times.clone());
    let hash_med = median(hash_times.clone());
    let (ratio_mean, ci_lo, ci_hi) = bca_ci(&paired_ratios);

    json!({
        "distribution": dist_name,
        "population": n,
        "raw_draws": raw_keys.len(),
        "expanse_ns_op": exp_med,
        "blart_art_ns_op": blart_med,
        "btree_ns_op": btree_med,
        "hashmap_ns_op": hash_med,
        "ratio_vs_art": ratio_mean,
        "ratio_bca_ci_95": [ci_lo, ci_hi],
        "samples": {
            "expanse": expanse_times,
            "blart_art": blart_times,
            "btree": btree_times,
            "hashmap": hash_times,
            "ratios": paired_ratios,
        }
    })
}

#[inline(never)]
fn time_expanse_insert(keys: &[u64]) -> f64 {
    let mut map = ExpanseMap::new();
    let start = Instant::now();
    for &k in keys {
        let old = map.insert(k, k.wrapping_mul(3));
        black_box(old);
    }
    let elapsed = start.elapsed();
    black_box(&map);
    elapsed.as_nanos() as f64 / keys.len() as f64
}

#[inline(never)]
fn time_blart_insert(keys: &[u64]) -> f64 {
    let mut map = ArtMap::new();
    let start = Instant::now();
    for &k in keys {
        let res = map.try_insert(art_key(k), k.wrapping_mul(3));
        let _ = black_box(res);
    }
    let elapsed = start.elapsed();
    black_box(&map);
    elapsed.as_nanos() as f64 / keys.len() as f64
}

#[inline(never)]
fn time_btree_insert(keys: &[u64]) -> f64 {
    let mut map = BTreeMap::new();
    let start = Instant::now();
    for &k in keys {
        let old = map.insert(k, k.wrapping_mul(3));
        black_box(old);
    }
    let elapsed = start.elapsed();
    black_box(&map);
    elapsed.as_nanos() as f64 / keys.len() as f64
}

#[inline(never)]
fn time_hash_insert(keys: &[u64]) -> f64 {
    let mut map = HashMap::new();
    let start = Instant::now();
    for &k in keys {
        let old = map.insert(k, k.wrapping_mul(3));
        black_box(old);
    }
    let elapsed = start.elapsed();
    black_box(&map);
    elapsed.as_nanos() as f64 / keys.len() as f64
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
    let rounds = if quick { 3 } else { 15 };

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
        "benchmark": "art_insert",
        "workload_id": "art_insert",
        "quick": quick,
        "rounds": rounds,
        "results": results,
    });

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("=== art_insert (quick={quick}, rounds={rounds}) ===");
        for r in results {
            let d = r["distribution"].as_str().unwrap();
            let pop = r["population"].as_u64().unwrap();
            let exp = r["expanse_ns_op"].as_f64().unwrap();
            let art = r["blart_art_ns_op"].as_f64().unwrap();
            let rat = r["ratio_vs_art"].as_f64().unwrap();
            let ci = &r["ratio_bca_ci_95"];
            println!(
                "  pop={pop:7} | dist={d:15} | Expanse: {exp:6.2} ns | ART: {art:6.2} ns | Ratio (Exp/ART): {rat:5.2}x CI [{:.2}, {:.2}]",
                ci[0].as_f64().unwrap(),
                ci[1].as_f64().unwrap()
            );
        }
    }
}
