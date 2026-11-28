//! DNS-shaped keys: closest encloser, against `BTreeMap` and `qp-trie`.
//!
//! `docs/benchmarks/dns_names/METHODOLOGY.md` §3, operation
//! `dns_closest_encloser`: strip the leftmost label of an absent name until a
//! lookup hits. No arm has a longest-prefix-match call, so the loop is the
//! same on every arm: one exact lookup per label. In the reversed key forms a
//! stripped name is a prefix of the probe, sliced at offsets computed in
//! setup. A probe sits k labels below a present two-label name, k uniform in
//! 1..=3, so it costs k + 1 lookups and only the last one hits.
//!
//! One process times one round (`--round K`): every arm once, arm order
//! rotated by `K`. The suite runner starts one process per round.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `dns_closest_encloser` |
//! | `group` | 4 |
//! | `population` | 1k, 10k, 100k and 1M synthetic names (`dns_common::World`); 1k and 10k under `--quick` |
//! | `insertion_order` | both: `sorted` is ascending key order, `shuffled` a Fisher–Yates permutation from the suite PRNG; one row per order |
//! | `probes_and_reuse` | N/2 `encloser` probes in a fixed permutation, k + 1 lookups each (3 on average); the reported figure is ns per probe, not per lookup; repetitions calibrated to the 20 ms `MIN_WINDOW` |
//! | `hit_rate` | Per probe, k misses then one hit; over a pass, one lookup in three hits on average |
//! | `miss_gen_method` | Generator-rule labels in front of a uniformly drawn present two-label name; the name one label below the present one is rejected on membership, and deeper names cannot be present (the population holds no name of four labels) |
//! | `value_dereference` | `black_box` of the encloser's value |
//! | `measured_region` | Probe passes only; generation, encoding, cut offsets, builds, validation and drops are outside the window; one discarded warm-up pass per arm |
//! | `arm_symmetry` | Identical names, values, build order, probe order and loop on every arm; `u64` payload on all four; `expanse_direct` reads the NUL-free key form, validated once outside the window |
//! | `statistics` | One round per process, arm order rotated by the round; `run_all.py` runs 12 processes and publishes the geometric mean of the per-round Expanse/twin ratio with a BCa 95% interval on the per-round log ratios |
//! | `verdict` | **PASS** `[verified: CODE READ]`: clean timed region; every arm's encloser and lookup count checked before timing. |

#[path = "dns_common/mod.rs"]
mod dns_common;

use dns_common::{
    Arm, EncloserProbe, ORDERS, World, build_btree, build_direct, build_ordered, build_qp, cli,
    emit, key_direct, key_nul, nul_free, time_round,
};
use expanse_trie::strmap::NulFreeStr;
use serde_json::json;
use std::hint::black_box;
use std::time::{Duration, Instant};

/// The value of the first name that is present, trying each cut in turn, and
/// the number of lookups made.
#[inline(always)]
fn encloser<F: Fn(&[u8]) -> Option<u64>>(p: &EncloserProbe, get: &F) -> (u64, usize) {
    for (i, &cut) in p.cuts[..p.tries].iter().enumerate() {
        if let Some(v) = get(&p.key[..cut as usize]) {
            return (v, i + 1);
        }
    }
    (u64::MAX, p.tries)
}

/// One timed pass: `reps` repetitions of the encloser walk of every probe.
#[inline(never)]
fn pass_encloser<F: Fn(&[u8]) -> Option<u64>>(
    probes: &[EncloserProbe],
    reps: usize,
    get: F,
) -> Duration {
    let start = Instant::now();
    for _ in 0..reps {
        for p in probes {
            black_box(encloser(black_box(p), &get).0);
        }
    }
    start.elapsed()
}

/// Checks that an arm finds every probe's encloser at its last cut; returns
/// the lookups one pass makes.
fn check<F: Fn(&[u8]) -> Option<u64>>(arm: &str, probes: &[EncloserProbe], get: F) -> usize {
    let mut lookups = 0;
    for p in probes {
        let (v, tries) = encloser(p, &get);
        assert_eq!(v, p.answer, "{arm}: wrong closest encloser");
        assert_eq!(tries, p.tries, "{arm}: encloser found at the wrong label");
        lookups += tries;
    }
    lookups
}

fn main() {
    let cli = cli();
    let mut cells = Vec::new();
    for &n in &cli.pops {
        let w = World::new(n);
        let probes = w.encloser_probes(key_nul);
        let probes_direct = w.encloser_probes(key_direct);
        assert!(
            probes_direct
                .iter()
                .all(|p| NulFreeStr::new(&p.key).is_some()),
            "a key_direct probe holds a NUL"
        );
        for order in ORDERS {
            let build = w.build_order(order);
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

            let lookups = check("expanse_ordered", &probes, |k| ordered.get(k));
            for got in [
                check("expanse_direct", &probes_direct, |k| {
                    direct.get(nul_free(k))
                }),
                check("btreemap", &probes, |k| btree.get(k).copied()),
                check("qp_trie", &probes, |k| qp.get(k).copied()),
            ] {
                assert_eq!(got, lookups, "arms disagree on the lookup count");
            }

            let arms = vec![
                Arm {
                    name: "expanse_ordered",
                    pass: Box::new(|r| pass_encloser(&probes, r, |k| ordered.get(k))),
                },
                Arm {
                    name: "expanse_direct",
                    pass: Box::new(|r| {
                        pass_encloser(&probes_direct, r, |k| direct.get(nul_free(k)))
                    }),
                },
                Arm {
                    name: "btreemap",
                    pass: Box::new(|r| pass_encloser(&probes, r, |k| btree.get(k).copied())),
                },
                Arm {
                    name: "qp_trie",
                    pass: Box::new(|r| pass_encloser(&probes, r, |k| qp.get(k).copied())),
                },
            ];
            let rows = time_round(arms, probes.len(), cli.round);
            cells.push(json!({
                "population": n,
                "order": order,
                "probe_class": "encloser",
                "probes": probes.len(),
                "lookups_per_pass": lookups,
                "rounds_raw": rows,
            }));
            drop((ordered, direct, btree, qp));
        }
    }
    emit("dns_closest_encloser", &cli, "cells", cells);
}
