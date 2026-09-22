//! Patricia trie vs Expanse: point lookup, 100% hit.
//!
//! Every present key probed once per round, in a Fisher–Yates order so the
//! probe stream does not replay insertion order. Both arms receive pre-encoded
//! keys (`u64` for Expanse, big-endian `[u8; 8]` for `PatriciaMap`).
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `patricia_lookup_hit` |
//! | `group` | 4 |
//! | `population` | 10k to 1M |
//! | `insertion_order` | generator draw order — ascending on `sequential` and `sparse_stride`; a Patricia tree's node set is fixed by its key set, so build order does not change the structure probed |
//! | `probes_and_reuse` | Every present key once per round, shuffled under `PROBE_SHUFFLE_SEED`; same stream every round |
//! | `hit_rate` | 100% |
//! | `miss_gen_method` | None |
//! | `value_dereference` | `black_box` of the returned value |
//! | `measured_region` | Probe loop only; build and encoding outside the window |
//! | `arm_symmetry` | Identical keys, values and probe order; arm order alternates per round |
//! | `statistics` | Median per arm + BCa 95% CI on the paired per-round ratio |
//! | `verdict` | **PENDING** `[unmeasured]`: no committed run yet. |

#[path = "art_common/mod.rs"]
mod art_common;
#[path = "patricia_common/mod.rs"]
mod patricia_common;

use art_common::{
    PROBE_SHUFFLE_SEED, SHARED_SEED, XorShift64, dedupe_preserve_order, gen_clustered,
    gen_sequential, gen_sparse_stride, gen_uniform_random, gen_zipfian, shuffle,
};
use patricia_common::{cli, lookup_row, print_rows};
use serde_json::json;

fn row(dist: &str, raw: &[u64], rounds: usize) -> serde_json::Value {
    let keys = dedupe_preserve_order(raw);
    let mut probes = keys.clone();
    shuffle(&mut probes, &mut XorShift64::new(PROBE_SHUFFLE_SEED));
    lookup_row(dist, &keys, &probes, 100, rounds)
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
        let out = json!({"benchmark": "patricia_lookup_hit", "workload_id": "patricia_lookup_hit",
            "competitor": "patricia_tree 0.10.2", "quick": quick, "rounds": rounds, "results": rows});
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        print_rows(
            &format!("patricia_lookup_hit (quick={quick}, rounds={rounds})"),
            &rows,
        );
    }
}
