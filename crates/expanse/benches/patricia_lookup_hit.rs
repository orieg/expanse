//! Patricia / radix tries vs Expanse: `u64` point lookup, 100% hit.
//!
//! Every present key is probed once per repetition, in a Fisher–Yates order
//! that is fixed per cell and independent of build order. Each arm is built on
//! its own from the same keys, in generator order and again in a shuffled
//! order: a Patricia tree's node *set* is fixed by its key set, but where its
//! nodes land in memory is not, and a sibling-list walk is dominated by that
//! (§8.12.4). Every arm is validated before timing; an arm that panics or
//! loses keys is recorded as invalid, not timed.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `patricia_lookup_hit` |
//! | `group` | 4 |
//! | `population` | 10k to 1M |
//! | `insertion_order` | both — generator draw order (ascending on `sequential` and `sparse_stride`) and a Fisher–Yates permutation under `PROBE_SHUFFLE_SEED`; one row per order |
//! | `probes_and_reuse` | Every present key once per repetition, shuffled; repetitions calibrated so each timed pass is at least `MIN_WINDOW` |
//! | `hit_rate` | 100% |
//! | `miss_gen_method` | None |
//! | `value_dereference` | `black_box` of the returned value |
//! | `measured_region` | Probe passes only; builds, validation and key encoding outside the window; one discarded warm-up pass per arm |
//! | `arm_symmetry` | Identical keys, values, probe order and build order; arms built separately; arm order rotates per round |
//! | `statistics` | Median per arm + geometric-mean Expanse/twin ratio with BCa 95% CI on per-round log ratios |
//! | `verdict` | **PENDING** `[unmeasured]`: no committed run yet. |

#[path = "art_common/mod.rs"]
mod art_common;
#[path = "patricia_common/mod.rs"]
mod patricia_common;

use patricia_common::{DISTS, cli, emit, shuffled, u64_dist, u64_lookup_cell};
use serde_json::{Value, json};

fn main() {
    let cli = cli(&[10_000, 100_000, 1_000_000]);
    let mut rows: Vec<Value> = Vec::new();
    for &n in &cli.pops {
        for dist in DISTS {
            let keys = u64_dist(dist, n);
            let probes = shuffled(&keys);
            for (order, build) in [("generator", keys.clone()), ("shuffled", shuffled(&keys))] {
                let mut row = u64_lookup_cell(&build, &probes, cli.rounds);
                row.insert("distribution".into(), json!(dist));
                row.insert("order".into(), json!(order));
                row.insert("population".into(), json!(keys.len()));
                row.insert("raw_draws".into(), json!(n));
                row.insert("hit_rate_pct".into(), json!(100));
                rows.push(Value::Object(row));
            }
        }
    }
    emit("patricia_lookup_hit", "patricia_lookup_hit", &cli, rows);
}
