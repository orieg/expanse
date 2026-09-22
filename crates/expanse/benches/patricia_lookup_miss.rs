//! Patricia trie vs Expanse: point lookup, 50% hit / 50% miss.
//!
//! Half the probes are present keys, half are drawn from the population's own
//! generator and rejected on membership (`art_common::gen_distribution_misses`,
//! AGENTS.md §8.6), interleaved by a Fisher–Yates shuffle. A Patricia miss can
//! stop early in a sorted sibling list, so this cell is where its miss path is
//! exercised.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `patricia_lookup_miss` |
//! | `group` | 4 |
//! | `population` | 10k to 1M |
//! | `insertion_order` | generator draw order — ascending on `sequential` and `sparse_stride`; a Patricia tree's node set is fixed by its key set, so build order does not change the structure probed |
//! | `probes_and_reuse` | N/2 present + N/2 absent keys, shuffled under `PROBE_SHUFFLE_SEED`; same stream every round |
//! | `hit_rate` | 50% hit / 50% miss |
//! | `miss_gen_method` | Same-distribution rejection sampling (`gen_distribution_misses`) |
//! | `value_dereference` | `black_box` of the returned value or 0 |
//! | `measured_region` | Probe loop only; build and encoding outside the window |
//! | `arm_symmetry` | Identical keys, values and probe order; arm order alternates per round |
//! | `statistics` | Median per arm + BCa 95% CI on the paired per-round ratio |
//! | `verdict` | **PENDING** `[unmeasured]`: no committed run yet. |

#[path = "art_common/mod.rs"]
mod art_common;
#[path = "patricia_common/mod.rs"]
mod patricia_common;

use art_common::{
    MISS_SEED, PROBE_SHUFFLE_SEED, SHARED_SEED, XorShift64, dedupe_preserve_order, gen_clustered,
    gen_distribution_misses, gen_sequential, gen_sparse_stride, gen_uniform_random, gen_zipfian,
    shuffle,
};
use patricia_common::{cli, lookup_row, print_rows};
use serde_json::json;

fn row(dist: &str, raw: &[u64], rounds: usize) -> serde_json::Value {
    let keys = dedupe_preserve_order(raw);
    let half = keys.len() / 2;
    let misses = gen_distribution_misses(dist, &keys, half, &mut XorShift64::new(MISS_SEED));
    let mut probes = Vec::with_capacity(half * 2);
    probes.extend_from_slice(&keys[..half]);
    probes.extend_from_slice(&misses);
    shuffle(&mut probes, &mut XorShift64::new(PROBE_SHUFFLE_SEED));
    lookup_row(dist, &keys, &probes, 50, rounds)
}

fn main() {
    let (pops, rounds, quick, json_mode) = cli();
    let mut rows = Vec::new();
    let mut rng = XorShift64::new(SHARED_SEED);
    for &n in pops {
        rows.push(row("sequential", &gen_sequential(n), rounds));
        rows.push(row("clustered", &gen_clustered(n, &mut rng), rounds));
        rows.push(row(
            "uniform_random",
            &gen_uniform_random(n, &mut rng),
            rounds,
        ));
        rows.push(row("sparse_stride", &gen_sparse_stride(n), rounds));
        rows.push(row("zipfian", &gen_zipfian(n, 0.99, &mut rng), rounds));
    }
    if json_mode {
        let out = json!({"benchmark": "patricia_lookup_miss", "workload_id": "patricia_lookup_miss",
            "competitor": "patricia_tree 0.10.2", "quick": quick, "rounds": rounds, "results": rows});
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        print_rows(
            &format!("patricia_lookup_miss (quick={quick}, rounds={rounds})"),
            &rows,
        );
    }
}
