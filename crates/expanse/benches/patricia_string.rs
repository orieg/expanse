//! Patricia trie vs Expanse on shared-prefix string keys: point lookup, 50% hit.
//!
//! Keys are `https://example.com/api/v2/objects/<12 hex>` — a 34-byte prefix
//! shared by every key, then 48 random bits. A Patricia tree stores the prefix
//! once as a single label; `ExpanseStrMap` (the JudySL counterpart) descends it
//! eight bytes per level. This is the regime pre-registered as the twin's
//! plausible win (AGENTS.md §8.3). Misses come from the same generator and are
//! rejected on membership (§8.6). Every key is validated as NUL-free once,
//! outside the timed region, so neither arm scans for NUL per probe.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `patricia_string_lookup` |
//! | `group` | 4 |
//! | `population` | 10k to 1M |
//! | `insertion_order` | generator draw order (random ids); a Patricia tree's node set is fixed by its key set, so build order does not change the structure probed |
//! | `probes_and_reuse` | N/2 present + N/2 absent keys, shuffled under `PROBE_SHUFFLE_SEED`; same stream every round |
//! | `hit_rate` | 50% hit / 50% miss |
//! | `miss_gen_method` | Same generator, independent seed, rejected on membership (`gen_prefixed_path_misses`) |
//! | `value_dereference` | `black_box` of the returned value or 0 |
//! | `measured_region` | Probe loop only; build, key validation and probe assembly outside the window |
//! | `arm_symmetry` | Identical keys, values and probe order; arm order alternates per round |
//! | `statistics` | Median per arm + BCa 95% CI on the paired per-round ratio |
//! | `verdict` | **PENDING** `[unmeasured]`: no committed run yet. |

#[path = "art_common/mod.rs"]
mod art_common;
#[path = "patricia_common/mod.rs"]
mod patricia_common;

use art_common::{MISS_SEED, PROBE_SHUFFLE_SEED, XorShift64, bca_ci, median, rounds_raw};
use expanse_trie::strmap::{ExpanseStrMap, NulFreeStr};
use patricia_common::{
    PatriciaMap, STRING_SEED, as_nulfree, ci_method, cli, gen_prefixed_path_misses,
    gen_prefixed_paths, print_rows,
};
use serde_json::json;
use std::hint::black_box;
use std::time::Instant;

#[inline(never)]
fn time_expanse(map: &ExpanseStrMap, probes: &[&NulFreeStr]) -> f64 {
    let start = Instant::now();
    for k in probes {
        black_box(map.get(k).unwrap_or(0));
    }
    start.elapsed().as_nanos() as f64 / probes.len() as f64
}

#[inline(never)]
fn time_patricia(map: &PatriciaMap<u64>, probes: &[&[u8]]) -> f64 {
    let start = Instant::now();
    for k in probes {
        black_box(map.get(k).copied().unwrap_or(0));
    }
    start.elapsed().as_nanos() as f64 / probes.len() as f64
}

fn row(n: usize, rounds: usize) -> serde_json::Value {
    let keys = gen_prefixed_paths(n, &mut XorShift64::new(STRING_SEED));
    let half = n / 2;
    let misses = gen_prefixed_path_misses(&keys, half, MISS_SEED);
    let mut probes: Vec<Vec<u8>> = keys[..half].to_vec();
    probes.extend(misses);
    // Shuffle probe order through an index permutation (art_common's shuffle is u64-only).
    let mut idx: Vec<u64> = (0..probes.len() as u64).collect();
    art_common::shuffle(&mut idx, &mut XorShift64::new(PROBE_SHUFFLE_SEED));
    let probes: Vec<Vec<u8>> = idx.iter().map(|&i| probes[i as usize].clone()).collect();

    let mut e = ExpanseStrMap::new();
    let mut p = PatriciaMap::new();
    for (i, k) in as_nulfree(&keys).into_iter().enumerate() {
        e.insert(k, i as u64);
    }
    for (i, k) in keys.iter().enumerate() {
        p.insert(k, i as u64);
    }
    let eprobes = as_nulfree(&probes);
    let pprobes: Vec<&[u8]> = probes.iter().map(Vec::as_slice).collect();

    let (mut et, mut pt, mut ratios) = (Vec::new(), Vec::new(), Vec::new());
    for round in 0..rounds {
        let (a, b) = if round % 2 == 0 {
            let a = time_expanse(&e, &eprobes);
            (a, time_patricia(&p, &pprobes))
        } else {
            let b = time_patricia(&p, &pprobes);
            (time_expanse(&e, &eprobes), b)
        };
        et.push(a);
        pt.push(b);
        if b > 0.0 {
            ratios.push(a / b);
        }
    }
    let (r, lo, hi) = bca_ci(&ratios);
    json!({
        "distribution": "prefixed_path",
        "population": n,
        "probes": pprobes.len(),
        "hit_rate_pct": 50,
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

fn main() {
    let (pops, rounds, quick, json_mode) = cli();
    let rows: Vec<_> = pops.iter().map(|&n| row(n, rounds)).collect();
    if json_mode {
        let out = json!({"benchmark": "patricia_string", "workload_id": "patricia_string_lookup",
            "competitor": "patricia_tree 0.10.2", "quick": quick, "rounds": rounds, "results": rows});
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        print_rows(
            &format!("patricia_string (quick={quick}, rounds={rounds})"),
            &rows,
        );
    }
}
