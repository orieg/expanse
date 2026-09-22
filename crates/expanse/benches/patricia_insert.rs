//! Patricia / radix tries vs Expanse: `u64` cold-build insertion, both orders.
//!
//! Each repetition inserts every unique key into a fresh, empty structure. All
//! four structures are timed in both orders — generator order (ascending on
//! `sequential` and `sparse_stride`) and a Fisher–Yates permutation — within
//! the same rounds, so both the Expanse/twin ratio per order and each arm's
//! generator/shuffled ratio are paired per round and carry BCa intervals
//! (§8.12.4, §8.4). Each structure is built once per order and validated before
//! timing; an invalid structure is recorded, not timed.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `patricia_insert` |
//! | `group` | 4 |
//! | `population` | 10k to 1M |
//! | `insertion_order` | both — generator draw order and a Fisher–Yates permutation under `PROBE_SHUFFLE_SEED`, timed as separate arms in the same rounds |
//! | `probes_and_reuse` | Every unique key inserted once per repetition into a fresh structure; repetitions calibrated to `MIN_WINDOW` |
//! | `hit_rate` | N/A (all inserts are new keys) |
//! | `miss_gen_method` | None |
//! | `value_dereference` | `black_box` of each insert's previous-value result |
//! | `measured_region` | Insert loops only; each empty map is constructed before its window and dropped after it; one discarded warm-up pass per arm |
//! | `arm_symmetry` | Identical keys, values and orders; arm order rotates per round |
//! | `statistics` | Median per arm + geometric-mean paired ratios (Expanse/twin per order, generator/shuffled per arm) with BCa 95% CI on per-round log ratios |
//! | `verdict` | **PENDING** `[unmeasured]`: no committed run yet. |

#[path = "art_common/mod.rs"]
mod art_common;
#[path = "patricia_common/mod.rs"]
mod patricia_common;

use patricia_common::{
    Arm, DISTS, Invalid, PatriciaMap, QpTrie, RadixMap, Twin, build_expanse, build_twin, cli, emit,
    paired_ratio, pass_expanse_insert, pass_twin_insert, pkey, run_cell, series, shuffled,
    u64_dist, val,
};
use serde_json::{Value, json};

const ORDERS: [&str; 2] = ["generator", "shuffled"];

/// Encoded keys and values for one order.
struct Order {
    keys: Vec<u64>,
    bytes: Vec<[u8; 8]>,
    vals: Vec<u64>,
}

fn push_twin<'a, T: Twin<[u8; 8]>>(
    arms: &mut Vec<Arm<'a>>,
    invalid: &mut Vec<Invalid>,
    orders: &'a [Order; 2],
    valid_twins: &mut Vec<&'static str>,
) {
    // Validate in both orders before timing either.
    for (o, name) in orders.iter().zip(ORDERS) {
        if let Err(e) = build_twin::<[u8; 8], T>(&o.bytes, &o.vals) {
            invalid.push(Invalid {
                name: T::NAME.into(),
                reason: format!("{name} order: {e}"),
            });
            return;
        }
    }
    valid_twins.push(T::NAME);
    for (o, name) in orders.iter().zip(ORDERS) {
        arms.push(Arm {
            name: format!("{}_{name}", T::NAME),
            ops: o.keys.len(),
            pass: Box::new(move |r| pass_twin_insert::<[u8; 8], T>(&o.bytes, &o.vals, r)),
        });
    }
}

fn main() {
    let cli = cli(&[10_000, 100_000, 1_000_000]);
    let mut rows: Vec<Value> = Vec::new();
    for &n in &cli.pops {
        for dist in DISTS {
            let keys = u64_dist(dist, n);
            let make = |ks: Vec<u64>| Order {
                bytes: ks.iter().map(|&k| pkey(k)).collect(),
                vals: ks.iter().map(|&k| val(k)).collect(),
                keys: ks,
            };
            let orders = [make(keys.clone()), make(shuffled(&keys))];
            for o in &orders {
                drop(build_expanse(&o.keys));
            }
            let mut arms: Vec<Arm<'_>> = orders
                .iter()
                .zip(ORDERS)
                .map(|(o, name)| Arm {
                    name: format!("expanse_{name}"),
                    ops: o.keys.len(),
                    pass: Box::new(move |r| pass_expanse_insert(&o.keys, r)),
                })
                .collect();
            let mut invalid = Vec::new();
            let mut twins = Vec::new();
            push_twin::<PatriciaMap<u64>>(&mut arms, &mut invalid, &orders, &mut twins);
            push_twin::<RadixMap<u64>>(&mut arms, &mut invalid, &orders, &mut twins);
            push_twin::<QpTrie<[u8; 8], u64>>(&mut arms, &mut invalid, &orders, &mut twins);

            let mut row = run_cell(arms, &invalid, cli.rounds, false);
            for arm in std::iter::once("expanse").chain(twins.iter().copied()) {
                let g = series(&row, &format!("{arm}_generator"));
                let s = series(&row, &format!("{arm}_shuffled"));
                paired_ratio(
                    &mut row,
                    &format!("ratio_{arm}_generator_over_shuffled"),
                    &g,
                    &s,
                );
            }
            for t in &twins {
                for o in ORDERS {
                    let e = series(&row, &format!("expanse_{o}"));
                    let tw = series(&row, &format!("{t}_{o}"));
                    paired_ratio(&mut row, &format!("ratio_expanse_over_{t}_{o}"), &e, &tw);
                }
            }
            row.insert("distribution".into(), json!(dist));
            row.insert("population".into(), json!(keys.len()));
            row.insert("raw_draws".into(), json!(n));
            rows.push(Value::Object(row));
        }
    }
    emit("patricia_insert", "patricia_insert", &cli, rows);
}
