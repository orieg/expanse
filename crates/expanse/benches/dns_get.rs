//! DNS-shaped keys: exact lookup, 50% hit, against `BTreeMap` and `qp-trie`.
//!
//! `docs/benchmarks/dns_names/METHODOLOGY.md` §3, operation `dns_get`. The
//! subject is `ExpanseOrderedBytesMap` over `key_nul`, whose every probe is
//! re-encoded before the lookup; `ExpanseStrMap` over `key_direct` skips that
//! pass and is the control for it. `expanse_ordered_pre_encoded` (METHODOLOGY
//! amendment A2) reads the subject's own map with probes encoded outside the
//! window, so the pass is removed and the stored key is not changed. Two miss classes are measured as separate
//! rows, never averaged: a name under an absent second-level name, and a name
//! one label below a present one. They end at different depths.
//!
//! One process times one round (`--round K`): every arm once, arm order
//! rotated by `K`. The suite runner starts one process per round.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `dns_get` |
//! | `group` | 4 |
//! | `population` | 1k, 10k, 100k and 1M synthetic names (`dns_common::World`, the generator of `scripts/dns_name_bounds.py`); 1k and 10k under `--quick` |
//! | `insertion_order` | both: `sorted` is ascending key order, `shuffled` a Fisher–Yates permutation from the suite PRNG; one row per order |
//! | `probes_and_reuse` | N/2 present names (a uniformly drawn half) with N/2 absent names of one miss class, in a fixed permutation; repetitions calibrated to the 20 ms `MIN_WINDOW` |
//! | `hit_rate` | 50% hit / 50% miss, per miss class |
//! | `miss_gen_method` | `miss_new_sld`: the continuation of the population's generator stream, whose second-level names the population does not hold. `miss_under_sld`: a generator-rule label in front of a uniformly drawn present two-label name, rejected on membership |
//! | `value_dereference` | `black_box` of the returned value or 0 |
//! | `measured_region` | Probe passes only; name generation, key encoding, builds, validation and drops are outside the window; one discarded warm-up pass per arm |
//! | `arm_symmetry` | Identical names, values, build order and probe order on every arm, `u64` payload on all five; `expanse_direct` reads the same names in the NUL-free key form `ExpanseStrMap` requires, validated once outside the window; `expanse_ordered_pre_encoded` reads `expanse_ordered`'s map with the same probes escape-encoded outside the window; all arms built before any is timed |
//! | `statistics` | One round per process, arm order rotated by the round; `run_all.py` runs 12 processes and publishes the geometric mean of the per-round Expanse/twin ratio with a BCa 95% interval on the per-round log ratios |
//! | `verdict` | **PASS** `[verified: CODE READ]`: clean timed region, same-generator misses rejected on membership, both insertion orders. |

#[path = "dns_common/mod.rs"]
mod dns_common;

use dns_common::{
    Arm, ORDERS, World, assert_nul_free, build_btree, build_direct, build_ordered, build_qp, cli,
    emit, key_direct, key_nul, nul_free, time_round,
};
use expanse_trie::domain::escape_encode;
use serde_json::json;
use std::hint::black_box;
use std::time::{Duration, Instant};

/// One timed pass: `reps` repetitions of a lookup of every probe.
#[inline(never)]
fn pass_get<F: Fn(&[u8]) -> Option<u64>>(probes: &[Vec<u8>], reps: usize, get: F) -> Duration {
    let start = Instant::now();
    for _ in 0..reps {
        for k in probes {
            black_box(get(black_box(k)).unwrap_or(0));
        }
    }
    start.elapsed()
}

/// Hits and the sum of the values found, for the cross-arm check.
fn census<F: Fn(&[u8]) -> Option<u64>>(probes: &[Vec<u8>], get: F) -> (usize, u64) {
    probes.iter().fold((0, 0u64), |(h, s), k| match get(k) {
        Some(v) => (h + 1, s.wrapping_add(v)),
        None => (h, s),
    })
}

fn main() {
    let cli = cli();
    let mut cells = Vec::new();
    for &n in &cli.pops {
        let w = World::new(n);
        for order in ORDERS {
            let build = w.build_order(order);
            // Every arm is built before any is timed, in the round's order.
            let mut ordered = None;
            let mut direct = None;
            let mut btree = None;
            let mut qp = None;
            for i in 0..4 {
                match (cli.round + i) % 4 {
                    0 => ordered = Some(build_ordered(&w, &build)),
                    1 => direct = Some(build_direct(&w, &build)),
                    2 => btree = Some(build_btree(&w, &build)),
                    _ => qp = Some(build_qp(&w, &build)),
                }
            }
            let (ordered, direct) = (ordered.expect("built"), direct.expect("built"));
            let (btree, qp) = (btree.expect("built"), qp.expect("built"));

            for (class, misses) in [
                ("hit+miss_new_sld", &w.new_sld),
                ("hit+miss_under_sld", &w.under_sld),
            ] {
                let probes = w.get_probes(misses, false);
                let probes_direct = w.get_probes(misses, true);
                assert_nul_free(&probes_direct);
                debug_assert_eq!(key_nul(&misses[0]).len(), key_direct(&misses[0]).len());
                // The subject's probes in the form its map stores (A2): the
                // encoding `get` applies on every call, done once here.
                let probes_pre: Vec<Vec<u8>> = probes.iter().map(|k| escape_encode(k)).collect();
                assert_nul_free(&probes_pre);
                assert!(
                    probes.iter().zip(&probes_pre).all(|(k, e)| k != e),
                    "{class}: a key_nul probe needed no encoding"
                );
                let stored = ordered.as_inner();

                let want = census(&probes, |k| ordered.get(k));
                assert_eq!(want.0, w.hit_idx.len(), "{class}: hit count");
                assert_eq!(
                    census(&probes_pre, |k| stored.get(nul_free(k))),
                    want,
                    "{class}: expanse_ordered_pre_encoded disagrees"
                );
                assert_eq!(
                    census(&probes_direct, |k| direct.get(nul_free(k))),
                    want,
                    "{class}: expanse_direct disagrees"
                );
                assert_eq!(
                    census(&probes, |k| btree.get(k).copied()),
                    want,
                    "{class}: btreemap disagrees"
                );
                assert_eq!(
                    census(&probes, |k| qp.get(k).copied()),
                    want,
                    "{class}: qp_trie disagrees"
                );

                let arms = vec![
                    Arm {
                        name: "expanse_ordered",
                        pass: Box::new(|r| pass_get(&probes, r, |k| ordered.get(k))),
                    },
                    Arm {
                        name: "expanse_ordered_pre_encoded",
                        pass: Box::new(|r| pass_get(&probes_pre, r, |k| stored.get(nul_free(k)))),
                    },
                    Arm {
                        name: "expanse_direct",
                        pass: Box::new(|r| {
                            pass_get(&probes_direct, r, |k| direct.get(nul_free(k)))
                        }),
                    },
                    Arm {
                        name: "btreemap",
                        pass: Box::new(|r| pass_get(&probes, r, |k| btree.get(k).copied())),
                    },
                    Arm {
                        name: "qp_trie",
                        pass: Box::new(|r| pass_get(&probes, r, |k| qp.get(k).copied())),
                    },
                ];
                let rows = time_round(arms, probes.len(), cli.round);
                cells.push(json!({
                    "population": n,
                    "order": order,
                    "probe_class": class,
                    "probes": probes.len(),
                    "hits": want.0,
                    "rounds_raw": rows,
                }));
            }
            drop((ordered, direct, btree, qp));
        }
    }
    emit("dns_get", &cli, "cells", cells);
}
