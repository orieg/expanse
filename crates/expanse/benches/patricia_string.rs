//! Patricia / radix tries vs Expanse on shared-prefix string keys: point
//! lookup, 50% hit, across four prefix lengths.
//!
//! Keys are `prefix ++ <12 hex digits of 48 random bits>` with the prefix
//! shared by every key, swept over 8, 35, 128 and 240 bytes. A radix tree
//! stores the prefix once, as one label; `ExpanseStrMap` descends it eight
//! bytes per level. This is the regime pre-registered as the one where a twin
//! could win (§8.3), and the sweep is how a crossover would show up. Misses
//! come from the same generator on an independent stream, rejected on
//! membership (§8.6). Each arm is built on its own, in generator order (random
//! ids) and in sorted order, validated, and every key is checked NUL-free once
//! before any timing, so no arm scans for NUL per probe.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `patricia_string_lookup` |
//! | `group` | 4 |
//! | `population` | 10k to 1M, at each of four prefix lengths |
//! | `insertion_order` | both — generator order (random ids) and sorted byte order; one row per order |
//! | `probes_and_reuse` | Half the population (random) + as many absent keys, shuffled; repetitions calibrated to `MIN_WINDOW` |
//! | `hit_rate` | 50% hit / 50% miss |
//! | `miss_gen_method` | Same generator, independent seed, rejected on membership (`gen_path_misses`) |
//! | `value_dereference` | `black_box` of the returned value or 0 |
//! | `measured_region` | Probe passes only; builds, validation, NUL checks and probe assembly outside the window; one discarded warm-up pass per arm |
//! | `arm_symmetry` | Identical keys, values, probe order and build order; arms built separately; arm order rotates per round |
//! | `statistics` | Median per arm + geometric-mean Expanse/twin ratio with BCa 95% CI on per-round log ratios |
//! | `verdict` | **PENDING** `[unmeasured]`: no committed run yet. |

#[path = "art_common/mod.rs"]
mod art_common;
#[path = "patricia_common/mod.rs"]
mod patricia_common;

use patricia_common::{
    Arm, Invalid, PREFIX_LENS, PatriciaMap, QpTrie, RadixMap, STRING_SEED, as_nulfree,
    build_expanse_str, build_twin, cli, emit, gen_path_misses, gen_paths, pass_expanse_str_get,
    path_val, push_lookup_twin, run_cell, shuffled,
};
use serde_json::{Value, json};

fn cell(build: &[Vec<u8>], probes: &[Vec<u8>], rounds: usize) -> serde_json::Map<String, Value> {
    let vals: Vec<u64> = build.iter().map(|k| path_val(k)).collect();
    let nf = as_nulfree(build);
    let e = build_expanse_str(&nf, &vals);
    let pt: Result<PatriciaMap<u64>, String> = build_twin(build, &vals);
    let rx: Result<RadixMap<u64>, String> = build_twin(build, &vals);
    let qp: Result<QpTrie<Vec<u8>, u64>, String> = build_twin(build, &vals);

    let eprobes = as_nulfree(probes);
    let expected = eprobes.iter().filter(|k| e.get(k).is_some()).count();
    let mut arms = vec![Arm {
        name: "expanse".into(),
        ops: probes.len(),
        pass: Box::new(|r| pass_expanse_str_get(&e, &eprobes, r)),
    }];
    let mut invalid: Vec<Invalid> = Vec::new();
    push_lookup_twin(&mut arms, &mut invalid, &pt, probes, expected);
    push_lookup_twin(&mut arms, &mut invalid, &rx, probes, expected);
    push_lookup_twin(&mut arms, &mut invalid, &qp, probes, expected);
    let mut row = run_cell(arms, &invalid, rounds, true);
    row.insert("expected_hits".into(), json!(expected));
    row.insert("probes".into(), json!(probes.len()));
    row
}

fn main() {
    let cli = cli(&[10_000, 100_000, 1_000_000]);
    let mut rows: Vec<Value> = Vec::new();
    for &n in &cli.pops {
        for pl in PREFIX_LENS {
            let keys = gen_paths(n, pl, STRING_SEED);
            let half = n / 2;
            let mut probes: Vec<Vec<u8>> = shuffled(&keys)[..half].to_vec();
            probes.extend(gen_path_misses(&keys, half, pl));
            let probes = shuffled(&probes);
            let mut sorted = keys.clone();
            sorted.sort();
            for (order, build) in [("generator", &keys), ("sorted", &sorted)] {
                let mut row = cell(build, &probes, cli.rounds);
                row.insert("distribution".into(), json!("prefixed_path"));
                row.insert("prefix_len".into(), json!(pl));
                row.insert("key_len".into(), json!(pl + 12));
                row.insert("order".into(), json!(order));
                row.insert("population".into(), json!(n));
                row.insert("hit_rate_pct".into(), json!(50));
                rows.push(Value::Object(row));
            }
        }
    }
    emit("patricia_string", "patricia_string_lookup", &cli, rows);
}
