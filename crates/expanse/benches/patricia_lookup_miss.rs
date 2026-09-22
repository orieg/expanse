//! Patricia / radix tries vs Expanse: `u64` point lookup, 50% hit / 50% miss.
//!
//! Misses have the shape of hits (§8.6): the generator is drawn for `2n` keys,
//! the distinct keys are split at random into halves, one half is the
//! population (kept in generator order) and the other supplies the misses. A
//! miss therefore lies inside the populated range and ends at the same depth
//! as a hit — a dense generator has no in-range misses otherwise, and misses
//! drawn above the maximum stop near the root. Hits are a random half of the
//! population. Builds, validation and orders as in `patricia_lookup_hit`.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `patricia_lookup_miss` |
//! | `group` | 4 |
//! | `population` | about n keys: a random half of the distinct keys of a 2n generator draw, n from 10k to 1M |
//! | `insertion_order` | both — generator draw order of the kept half and a Fisher–Yates permutation under `PROBE_SHUFFLE_SEED`; one row per order |
//! | `probes_and_reuse` | Half the population (random) + as many held-out keys, shuffled; repetitions calibrated to `MIN_WINDOW` |
//! | `hit_rate` | 50% hit / 50% miss |
//! | `miss_gen_method` | Held-out random half of the same generator draw (`split_half`), so misses interleave the population |
//! | `value_dereference` | `black_box` of the returned value or 0 |
//! | `measured_region` | Probe passes only; builds, validation and key encoding outside the window; one discarded warm-up pass per arm |
//! | `arm_symmetry` | Identical keys, values, probe order and build order; arms built separately; arm order rotates per round |
//! | `statistics` | Median per arm + geometric-mean Expanse/twin ratio with BCa 95% CI on per-round log ratios |
//! | `verdict` | **PENDING** `[unmeasured]`: no committed run yet. |

#[path = "art_common/mod.rs"]
mod art_common;
#[path = "patricia_common/mod.rs"]
mod patricia_common;

use patricia_common::{DISTS, cli, emit, shuffled, split_half, u64_lookup_cell};
use serde_json::{Value, json};

fn main() {
    let cli = cli(&[10_000, 100_000, 1_000_000]);
    let mut rows: Vec<Value> = Vec::new();
    for &n in &cli.pops {
        for dist in DISTS {
            let (pop, misses) = split_half(dist, n);
            let half = pop.len().min(misses.len()) / 2;
            let mut probes: Vec<u64> = shuffled(&pop)[..half].to_vec();
            probes.extend_from_slice(&misses[..half]);
            let probes = shuffled(&probes);
            for (order, build) in [("generator", pop.clone()), ("shuffled", shuffled(&pop))] {
                let mut row = u64_lookup_cell(&build, &probes, cli.rounds);
                row.insert("distribution".into(), json!(dist));
                row.insert("order".into(), json!(order));
                row.insert("population".into(), json!(pop.len()));
                row.insert("raw_draws".into(), json!(2 * n));
                row.insert("hit_rate_pct".into(), json!(50));
                rows.push(Value::Object(row));
            }
        }
    }
    emit("patricia_lookup_miss", "patricia_lookup_miss", &cli, rows);
}
