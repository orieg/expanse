//! Patricia trie vs Expanse: cold-build insertion, both insertion orders.
//!
//! Each round builds both structures from empty over the same unique keys.
//! Insertion is order-sensitive for a sorted-sibling-list Patricia tree — an
//! ascending run appends at the tail of the deepest list, walking every
//! sibling before it — so every distribution is measured in generator order
//! and in a Fisher–Yates permutation (AGENTS.md §8.12.4), and the order is
//! recorded in each row.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `patricia_insert` |
//! | `group` | 4 |
//! | `population` | 10k to 1M |
//! | `insertion_order` | both — generator draw order (ascending on `sequential` and `sparse_stride`) and a Fisher–Yates permutation under `PROBE_SHUFFLE_SEED`; `order` recorded per row |
//! | `probes_and_reuse` | Every unique key inserted once per round into a fresh structure |
//! | `hit_rate` | N/A (all inserts are new keys) |
//! | `miss_gen_method` | None |
//! | `value_dereference` | `black_box` of each insert's previous-value result |
//! | `measured_region` | Insert loop only; construction of the empty map precedes the window and drop follows it |
//! | `arm_symmetry` | Identical keys, values and order; arm order alternates per round |
//! | `statistics` | Median per arm + BCa 95% CI on the paired per-round ratio |
//! | `verdict` | **PENDING** `[unmeasured]`: no committed run yet. |

#[path = "art_common/mod.rs"]
mod art_common;
#[path = "patricia_common/mod.rs"]
mod patricia_common;

use art_common::{
    ExpanseMap, PROBE_SHUFFLE_SEED, SHARED_SEED, XorShift64, bca_ci, dedupe_preserve_order,
    gen_clustered, gen_sequential, gen_sparse_stride, gen_uniform_random, gen_zipfian, median,
    rounds_raw, shuffle,
};
use patricia_common::{PatriciaMap, ci_method, cli, pkey, print_rows};
use serde_json::json;
use std::hint::black_box;
use std::time::Instant;

#[inline(never)]
fn time_expanse(keys: &[u64]) -> f64 {
    let mut m = ExpanseMap::new();
    let start = Instant::now();
    for &k in keys {
        black_box(m.insert(k, k.wrapping_mul(3)));
    }
    let ns = start.elapsed().as_nanos() as f64;
    black_box(&m);
    drop(m);
    ns / keys.len() as f64
}

#[inline(never)]
fn time_patricia(keys: &[[u8; 8]]) -> f64 {
    let mut m = PatriciaMap::new();
    let start = Instant::now();
    for k in keys {
        black_box(m.insert(k, u64::from_be_bytes(*k).wrapping_mul(3)));
    }
    let ns = start.elapsed().as_nanos() as f64;
    black_box(&m);
    drop(m);
    ns / keys.len() as f64
}

fn row(
    dist: &str,
    order: &str,
    keys: &[u64],
    raw_draws: usize,
    rounds: usize,
) -> serde_json::Value {
    let pkeys: Vec<[u8; 8]> = keys.iter().map(|&k| pkey(k)).collect();
    let (mut et, mut pt, mut ratios) = (Vec::new(), Vec::new(), Vec::new());
    for round in 0..rounds {
        let (a, b) = if round % 2 == 0 {
            let a = time_expanse(keys);
            (a, time_patricia(&pkeys))
        } else {
            let b = time_patricia(&pkeys);
            (time_expanse(keys), b)
        };
        et.push(a);
        pt.push(b);
        if b > 0.0 {
            ratios.push(a / b);
        }
    }
    let (r, lo, hi) = bca_ci(&ratios);
    json!({
        "distribution": dist,
        "order": order,
        "population": keys.len(),
        "raw_draws": raw_draws,
        "expanse_ns_op": median(et.clone()),
        "patricia_ns_op": median(pt.clone()),
        "ratio_expanse_over_patricia": r,
        "ratio_ci": [lo, hi],
        "ratio_ci_method": ci_method(ratios.len()),
        "rounds_raw": rounds_raw(&[
            ("expanse_ns", &et),
            ("patricia_ns", &pt),
            ("ratio_expanse_over_patricia", &ratios),
        ]),
    })
}

fn both(dist: &str, raw: &[u64], rounds: usize, out: &mut Vec<serde_json::Value>) {
    let keys = dedupe_preserve_order(raw);
    out.push(row(dist, "generator", &keys, raw.len(), rounds));
    let mut shuffled = keys;
    shuffle(&mut shuffled, &mut XorShift64::new(PROBE_SHUFFLE_SEED));
    out.push(row(dist, "shuffled", &shuffled, raw.len(), rounds));
}

fn main() {
    let (pops, rounds, quick, json_mode) = cli();
    let mut rows = Vec::new();
    let mut rng = XorShift64::new(SHARED_SEED);
    for &n in pops {
        both("sequential", &gen_sequential(n), rounds, &mut rows);
        both("clustered", &gen_clustered(n, &mut rng), rounds, &mut rows);
        both(
            "uniform_random",
            &gen_uniform_random(n, &mut rng),
            rounds,
            &mut rows,
        );
        both("sparse_stride", &gen_sparse_stride(n), rounds, &mut rows);
        both(
            "zipfian",
            &gen_zipfian(n, 0.99, &mut rng),
            rounds,
            &mut rows,
        );
    }
    if json_mode {
        let out = json!({"benchmark": "patricia_insert", "workload_id": "patricia_insert",
            "competitor": "patricia_tree 0.10.2", "quick": quick, "rounds": rounds, "results": rows});
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        for r in rows.iter_mut() {
            let label = format!(
                "{}/{}",
                r["distribution"].as_str().unwrap(),
                r["order"].as_str().unwrap()
            );
            r["distribution"] = json!(label);
        }
        print_rows(
            &format!("patricia_insert (quick={quick}, rounds={rounds})"),
            &rows,
        );
    }
}
