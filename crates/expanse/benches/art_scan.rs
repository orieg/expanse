//! Pillar 4: ART Range Scan & In-Order Iteration
//!
//! Evaluates ordered iteration (full container traversal) and bounded range scans (k=10, 100, 1000)
//! across key distributions, comparing ExpanseMap's contiguous bitmap chunk iterator against
//! blart (ART) iterator and BTreeMap.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `art_scan` |
//! | `group` | 4 |
//! | `population` | 10k to 1M |
//! | `probes_and_reuse` | Full iteration and range scans |
//! | `hit_rate` | 100% (Existing keys in populated range) |
//! | `miss_gen_method` | None |
//! | `value_dereference` | `black_box(k ^ v)` during traversal |
//! | `measured_region` | Iteration loop |
//! | `arm_symmetry` | Identical keys, direct iterator traversal |
//! | `statistics` | Median + BCa 95% Bootstrap CI over paired rounds |
//! | `verdict` | **PASS** `[verified: CODE READ]`: ART comparison ordered scan benchmark. |

#[path = "art_common/mod.rs"]
mod art_common;

use art_common::{
    ArtMap, BTreeMap, ExpanseMap, Mapped, ToUBE, XorShift64, art_key, bca_ci, gen_clustered,
    gen_sequential, gen_uniform_random, median,
};
use serde_json::json;
use std::hint::black_box;
use std::time::Instant;

fn bench_scan_case(
    dist_name: &str,
    keys: &[u64],
    range_k: usize,
    rounds: usize,
) -> serde_json::Value {
    let n = keys.len();

    let mut expanse = ExpanseMap::new();
    let mut blart = ArtMap::new();
    let mut btree = BTreeMap::new();

    for &k in keys {
        expanse.insert(k, k.wrapping_mul(3));
        let _ = blart.try_insert(art_key(k), k.wrapping_mul(3));
        btree.insert(k, k.wrapping_mul(3));
    }

    let mut expanse_times = Vec::with_capacity(rounds);
    let mut blart_times = Vec::with_capacity(rounds);
    let mut btree_times = Vec::with_capacity(rounds);
    let mut paired_ratios = Vec::with_capacity(rounds);

    let divisor = if range_k == 0 {
        n as f64
    } else {
        range_k as f64
    };

    for round in 0..rounds {
        let (e_ns, b_ns, bt_ns) = match round % 3 {
            0 => {
                let e = time_expanse_scan(&expanse, keys, range_k, divisor);
                let b = time_blart_scan(&blart, keys, range_k, divisor);
                let bt = time_btree_scan(&btree, keys, range_k, divisor);
                (e, b, bt)
            }
            1 => {
                let b = time_blart_scan(&blart, keys, range_k, divisor);
                let bt = time_btree_scan(&btree, keys, range_k, divisor);
                let e = time_expanse_scan(&expanse, keys, range_k, divisor);
                (e, b, bt)
            }
            _ => {
                let bt = time_btree_scan(&btree, keys, range_k, divisor);
                let e = time_expanse_scan(&expanse, keys, range_k, divisor);
                let b = time_blart_scan(&blart, keys, range_k, divisor);
                (e, b, bt)
            }
        };

        expanse_times.push(e_ns);
        blart_times.push(b_ns);
        btree_times.push(bt_ns);
        if b_ns > 0.0 {
            paired_ratios.push(e_ns / b_ns);
        }
    }

    let exp_med = median(expanse_times.clone());
    let blart_med = median(blart_times.clone());
    let btree_med = median(btree_times.clone());
    let (ratio_mean, ci_lo, ci_hi) = bca_ci(&paired_ratios);

    json!({
        "distribution": dist_name,
        "population": n,
        "range_k": range_k,
        "expanse_ns_elem": exp_med,
        "blart_art_ns_elem": blart_med,
        "btree_ns_elem": btree_med,
        "ratio_vs_art": ratio_mean,
        "ratio_bca_ci_95": [ci_lo, ci_hi],
        "samples": {
            "expanse": expanse_times,
            "blart_art": blart_times,
            "btree": btree_times,
            "ratios": paired_ratios,
        }
    })
}

#[inline(never)]
fn time_expanse_scan(map: &ExpanseMap, keys: &[u64], range_k: usize, divisor: f64) -> f64 {
    let start = Instant::now();
    let mut acc = 0u64;
    if range_k == 0 {
        for (k, v) in map.iter() {
            black_box(k);
            acc ^= v;
        }
    } else {
        let start_key = keys[keys.len() / 4];
        let mut count = 0;
        for (k, v) in map.range(start_key..=u64::MAX) {
            black_box(k);
            acc ^= v;
            count += 1;
            if count >= range_k {
                break;
            }
        }
    }
    black_box(acc);
    start.elapsed().as_nanos() as f64 / divisor
}

#[inline(never)]
fn time_blart_scan(
    map: &ArtMap<Mapped<ToUBE, u64>, u64>,
    keys: &[u64],
    range_k: usize,
    divisor: f64,
) -> f64 {
    let start = Instant::now();
    let mut acc = 0u64;
    if range_k == 0 {
        for (k, v) in map.iter() {
            black_box(k);
            acc ^= *v;
        }
    } else {
        let start_key = art_key(keys[keys.len() / 4]);
        let mut count = 0;
        for (k, v) in map.range(start_key..) {
            black_box(k);
            acc ^= *v;
            count += 1;
            if count >= range_k {
                break;
            }
        }
    }
    black_box(acc);
    start.elapsed().as_nanos() as f64 / divisor
}

#[inline(never)]
fn time_btree_scan(map: &BTreeMap<u64, u64>, keys: &[u64], range_k: usize, divisor: f64) -> f64 {
    let start = Instant::now();
    let mut acc = 0u64;
    if range_k == 0 {
        for (k, v) in map.iter() {
            black_box(k);
            acc ^= *v;
        }
    } else {
        let start_key = keys[keys.len() / 4];
        let mut count = 0;
        for (k, v) in map.range(start_key..) {
            black_box(k);
            acc ^= *v;
            count += 1;
            if count >= range_k {
                break;
            }
        }
    }
    black_box(acc);
    start.elapsed().as_nanos() as f64 / divisor
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
        let clustered = gen_clustered(n, &mut rng);
        let uniform = gen_uniform_random(n, &mut rng);

        // Full in-order iteration (range_k = 0)
        results.push(bench_scan_case("sequential", &seq, 0, rounds));
        results.push(bench_scan_case("clustered", &clustered, 0, rounds));
        results.push(bench_scan_case("uniform_random", &uniform, 0, rounds));

        // Bounded range scans (k=10, 100, 1000)
        for &k in &[10, 100, 1000] {
            results.push(bench_scan_case("sequential", &seq, k, rounds));
            results.push(bench_scan_case("clustered", &clustered, k, rounds));
            results.push(bench_scan_case("uniform_random", &uniform, k, rounds));
        }
    }

    let output = json!({
        "benchmark": "art_scan",
        "workload_id": "art_scan",
        "quick": quick,
        "rounds": rounds,
        "results": results,
    });

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("=== art_scan (quick={quick}, rounds={rounds}) ===");
        for r in results {
            let d = r["distribution"].as_str().unwrap();
            let pop = r["population"].as_u64().unwrap();
            let k = r["range_k"].as_u64().unwrap();
            let exp = r["expanse_ns_elem"].as_f64().unwrap();
            let art = r["blart_art_ns_elem"].as_f64().unwrap();
            let rat = r["ratio_vs_art"].as_f64().unwrap();
            let ci = &r["ratio_bca_ci_95"];
            println!(
                "  pop={pop:7} | dist={d:15} | k={k:4} | Expanse: {exp:6.2} ns/el | ART: {art:6.2} ns/el | Ratio: {rat:5.2}x CI [{:.2}, {:.2}]",
                ci[0].as_f64().unwrap(),
                ci[1].as_f64().unwrap()
            );
        }
    }
}
