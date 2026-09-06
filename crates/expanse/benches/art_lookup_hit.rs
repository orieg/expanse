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
//! | `probes_and_reuse` | Interleaved shuffled probe stream |
//! | `hit_rate` | 100% Hit |
//! | `miss_gen_method` | None |
//! | `value_dereference` | `black_box(*val)` |
//! | `measured_region` | Clean lookup loop |
//! | `arm_symmetry` | Symmetric keys, cold insertion, identical PRNG |
//! | `statistics` | Median + BCa 95% Bootstrap CI over paired rounds |
//! | `verdict` | **PASS** `[verified: CODE READ]`: ART comparison point lookup hit benchmark. |

#[path = "art_common/mod.rs"]
mod art_common;

use art_common::{
    ArtMap, BTreeMap, ExpanseMap, HashMap, Mapped, ToUBE, XorShift64, art_key, bca_ci,
    gen_clustered, gen_sequential, gen_sparse_stride, gen_uniform_random, gen_zipfian, median,
    rounds_raw, shuffle,
};
use serde_json::json;
use std::hint::black_box;
use std::time::Instant;

fn bench_dist(dist_name: &str, keys: &[u64], rounds: usize) -> serde_json::Value {
    let n = keys.len();

    // 1. Build structures (symmetric cold insertion)
    let mut expanse = ExpanseMap::new();
    let mut blart = ArtMap::new();
    let mut btree = BTreeMap::new();
    let mut hash = HashMap::new();

    for &k in keys {
        expanse.insert(k, k.wrapping_mul(3));
        let _ = blart.try_insert(art_key(k), k.wrapping_mul(3));
        btree.insert(k, k.wrapping_mul(3));
        hash.insert(k, k.wrapping_mul(3));
    }

    // Prepare probe stream: 100% hit rate, shuffled to prevent insertion-order cache artifacts
    let mut probe_keys = keys.to_vec();
    let mut shuffle_rng = XorShift64::new(art_common::PROBE_SHUFFLE_SEED);
    shuffle(&mut probe_keys, &mut shuffle_rng);

    let mut expanse_times = Vec::with_capacity(rounds);
    let mut blart_times = Vec::with_capacity(rounds);
    let mut btree_times = Vec::with_capacity(rounds);
    let mut hash_times = Vec::with_capacity(rounds);
    let mut paired_ratios = Vec::with_capacity(rounds);

    for round in 0..rounds {
        // Rotate arm order every round to eliminate machine drift bias
        let (e_ns, b_ns, bt_ns, h_ns) = match round % 4 {
            0 => {
                let e = time_expanse(&expanse, &probe_keys);
                let b = time_blart(&blart, &probe_keys);
                let bt = time_btree(&btree, &probe_keys);
                let h = time_hash(&hash, &probe_keys);
                (e, b, bt, h)
            }
            1 => {
                let b = time_blart(&blart, &probe_keys);
                let bt = time_btree(&btree, &probe_keys);
                let h = time_hash(&hash, &probe_keys);
                let e = time_expanse(&expanse, &probe_keys);
                (e, b, bt, h)
            }
            2 => {
                let bt = time_btree(&btree, &probe_keys);
                let h = time_hash(&hash, &probe_keys);
                let e = time_expanse(&expanse, &probe_keys);
                let b = time_blart(&blart, &probe_keys);
                (e, b, bt, h)
            }
            _ => {
                let h = time_hash(&hash, &probe_keys);
                let e = time_expanse(&expanse, &probe_keys);
                let b = time_blart(&blart, &probe_keys);
                let bt = time_btree(&btree, &probe_keys);
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
    let unique_count = if dist_name == "zipfian" {
        keys.iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len()
    } else {
        n
    };

    json!({
        "distribution": dist_name,
        "population": n,
        "unique_keys": unique_count,
        "expanse_ns_op": exp_med,
        "blart_art_ns_op": blart_med,
        "btree_ns_op": btree_med,
        "hashmap_ns_op": hash_med,
        "ratio_vs_art": ratio_mean,
        "ratio_bca_ci_95": [ci_lo, ci_hi],
        "rounds_raw": rounds_raw(&[
            ("expanse_ns", &expanse_times),
            ("blart_art_ns", &blart_times),
            ("btree_ns", &btree_times),
            ("hashmap_ns", &hash_times),
            ("ratio_vs_art", &paired_ratios),
        ])
    })
}

#[inline(never)]
fn time_expanse(map: &ExpanseMap, keys: &[u64]) -> f64 {
    let start = Instant::now();
    for &k in keys {
        let v = map.get(k).unwrap_or(0);
        black_box(v);
    }
    start.elapsed().as_nanos() as f64 / keys.len() as f64
}

#[inline(never)]
fn time_blart(map: &ArtMap<Mapped<ToUBE, u64>, u64>, keys: &[u64]) -> f64 {
    let start = Instant::now();
    for &k in keys {
        let ak = art_key(k);
        let v = map.get(&ak).copied().unwrap_or(0);
        black_box(v);
    }
    start.elapsed().as_nanos() as f64 / keys.len() as f64
}

#[inline(never)]
fn time_btree(map: &BTreeMap<u64, u64>, keys: &[u64]) -> f64 {
    let start = Instant::now();
    for &k in keys {
        let v = map.get(&k).copied().unwrap_or(0);
        black_box(v);
    }
    start.elapsed().as_nanos() as f64 / keys.len() as f64
}

#[inline(never)]
fn time_hash(map: &HashMap<u64, u64>, keys: &[u64]) -> f64 {
    let start = Instant::now();
    for &k in keys {
        let v = map.get(&k).copied().unwrap_or(0);
        black_box(v);
    }
    start.elapsed().as_nanos() as f64 / keys.len() as f64
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
        "benchmark": "art_lookup_hit",
        "workload_id": "art_lookup_hit",
        "quick": quick,
        "rounds": rounds,
        "results": results,
    });

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("=== art_lookup_hit (quick={quick}, rounds={rounds}) ===");
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
