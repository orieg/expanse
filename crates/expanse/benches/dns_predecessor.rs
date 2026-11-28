//! DNS-shaped keys: the largest name at or before an absent name, against
//! `BTreeMap`.
//!
//! `docs/benchmarks/dns_names/METHODOLOGY.md` §3, operation `dns_predecessor`:
//! the lookup a name server makes to find the record that covers an absent
//! name. Key and value are both returned, and the sink reads both, so no arm's
//! key reconstruction is dead code. `qp-trie` 0.8.2 has no predecessor or
//! range call and is absent from this harness (`NOT_APPLICABLE`, §2).
//!
//! `expanse_ordered` decodes the key into a caller buffer;
//! `expanse_ordered_owned` is the allocating form of the same call, timed as
//! an extra row with no prediction.
//!
//! One process times one round (`--round K`): every arm once, arm order
//! rotated by `K`. The suite runner starts one process per round.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `dns_predecessor` |
//! | `group` | 4 |
//! | `population` | 1k, 10k, 100k and 1M synthetic names (`dns_common::World`); 1k and 10k under `--quick` |
//! | `insertion_order` | both: `sorted` is ascending key order, `shuffled` a Fisher–Yates permutation from the suite PRNG; one row per order |
//! | `probes_and_reuse` | N/2 absent names of one miss class in a fixed permutation, each class a row of its own; repetitions calibrated to the 20 ms `MIN_WINDOW` |
//! | `hit_rate` | 0% exact hits: every probe is absent, and every probe has a predecessor except those below the smallest name |
//! | `miss_gen_method` | `miss_new_sld`: the continuation of the population's generator stream. `miss_under_sld`: a generator-rule label in front of a uniformly drawn present two-label name, rejected on membership |
//! | `value_dereference` | `black_box` of value + returned key length + the key's last byte |
//! | `measured_region` | Probe passes only; generation, encoding, builds, validation and drops are outside the window; one discarded warm-up pass per arm |
//! | `arm_symmetry` | Identical names, values, build order and probe order; `u64` payload on every arm; the Expanse arms rebuild the key into a 512-byte stack buffer where `BTreeMap` returns a borrowed key, which is the difference the row measures |
//! | `statistics` | One round per process, arm order rotated by the round; `run_all.py` runs 12 processes and publishes the geometric mean of the per-round Expanse/twin ratio with a BCa 95% interval on the per-round log ratios |
//! | `verdict` | **PASS** `[verified: CODE READ]`: clean timed region, every arm's answer checked against `BTreeMap`'s before timing. |

#[path = "dns_common/mod.rs"]
mod dns_common;

use dns_common::{
    Arm, ORDERS, World, assert_nul_free, build_btree, build_direct, build_ordered, cli, emit,
    key_direct, key_nul, nul_free, time_round,
};
use expanse_trie::ExpanseOrderedBytesMap;
use expanse_trie::strmap::ExpanseStrMap;
use serde_json::json;
use std::collections::BTreeMap;
use std::hint::black_box;
use std::ops::Bound;
use std::time::{Duration, Instant};

/// Decode buffer: the longest generated key is under 80 bytes.
const BUF: usize = 512;

/// One timed pass: `reps` repetitions of a predecessor lookup of every probe.
/// `pred` returns the fold of value, key length and the key's last byte.
#[inline(never)]
fn pass_pred<F: FnMut(&[u8]) -> u64>(probes: &[Vec<u8>], reps: usize, mut pred: F) -> Duration {
    let start = Instant::now();
    for _ in 0..reps {
        for k in probes {
            black_box(pred(black_box(k)));
        }
    }
    start.elapsed()
}

#[inline(always)]
fn fold(value: u64, key: &[u8]) -> u64 {
    value
        .wrapping_add(key.len() as u64)
        .wrapping_add(u64::from(key[key.len() - 1]))
}

#[inline(always)]
fn ordered_into(m: &ExpanseOrderedBytesMap, k: &[u8], buf: &mut [u8; BUF]) -> u64 {
    match m.prev_at_or_before_entry_decode_into(k, buf) {
        Ok(Some((len, v))) => fold(v, &buf[..len]),
        Ok(None) => 0,
        Err(e) => panic!("decode buffer too small: {e:?}"),
    }
}

#[inline(always)]
fn ordered_owned(m: &ExpanseOrderedBytesMap, k: &[u8]) -> u64 {
    match m.prev_at_or_before_entry(k) {
        Some((key, v)) => fold(v, &key),
        None => 0,
    }
}

#[inline(always)]
fn direct_into(m: &ExpanseStrMap, k: &[u8], buf: &mut [u8; BUF]) -> u64 {
    match m.prev_at_or_before_into(nul_free(k), buf) {
        Some(Ok((len, slot))) => {
            // SAFETY: `m` is borrowed shared for the call, so no mutation can
            // move or free the slot, and the engine returns a valid, aligned,
            // initialised `u64`.
            let v = unsafe { slot.as_ptr().read() };
            fold(v, &buf[..len])
        }
        Some(Err(need)) => panic!("decode buffer too small: {need} bytes needed"),
        None => 0,
    }
}

#[inline(always)]
fn btree_pred(m: &BTreeMap<Vec<u8>, u64>, k: &[u8]) -> u64 {
    match m
        .range::<[u8], _>((Bound::Unbounded, Bound::Included(k)))
        .next_back()
    {
        Some((key, v)) => fold(*v, key),
        None => 0,
    }
}

fn main() {
    let cli = cli();
    let mut cells = Vec::new();
    for &n in &cli.pops {
        let w = World::new(n);
        for order in ORDERS {
            let build = w.build_order(order);
            let mut ordered = None;
            let mut direct = None;
            let mut btree = None;
            for i in 0..3 {
                match (cli.round + i) % 3 {
                    0 => ordered = Some(build_ordered(&w, &build)),
                    1 => direct = Some(build_direct(&w, &build)),
                    _ => btree = Some(build_btree(&w, &build)),
                }
            }
            let (ordered, direct, btree) = (
                ordered.expect("built"),
                direct.expect("built"),
                btree.expect("built"),
            );

            for (class, misses) in [
                ("miss_new_sld", &w.new_sld),
                ("miss_under_sld", &w.under_sld),
            ] {
                let probes = w.miss_probes(misses, key_nul);
                let probes_direct = w.miss_probes(misses, key_direct);
                assert_nul_free(&probes_direct);

                // Every arm must name BTreeMap's entry: same value, and for
                // the key_nul arms the same key bytes.
                let mut buf = [0u8; BUF];
                let mut answered = 0usize;
                for (k, kd) in probes.iter().zip(&probes_direct) {
                    let want = btree
                        .range::<[u8], _>((Bound::Unbounded, Bound::Included(k.as_slice())))
                        .next_back();
                    let got = ordered
                        .prev_at_or_before_entry_decode_into(k, &mut buf)
                        .expect("decode buffer");
                    assert_eq!(
                        got.map(|(len, v)| (&buf[..len], v)),
                        want.map(|(key, v)| (key.as_slice(), *v)),
                        "{class}: expanse_ordered disagrees with btreemap"
                    );
                    assert_eq!(
                        ordered.prev_at_or_before_entry(k),
                        want.map(|(key, v)| (key.clone(), *v)),
                        "{class}: expanse_ordered_owned disagrees with btreemap"
                    );
                    let dv = direct
                        .prev_at_or_before(nul_free(kd))
                        // SAFETY: `direct` is borrowed shared; the slot is a
                        // valid, aligned, initialised `u64`.
                        .map(|(key, slot)| (key.len(), unsafe { slot.as_ptr().read() }));
                    assert_eq!(
                        dv,
                        want.map(|(key, v)| (key.len(), *v)),
                        "{class}: expanse_direct disagrees with btreemap"
                    );
                    answered += usize::from(want.is_some());
                }

                let mut buf_o = [0u8; BUF];
                let mut buf_d = [0u8; BUF];
                let arms = vec![
                    Arm {
                        name: "expanse_ordered",
                        pass: Box::new(|r| {
                            pass_pred(&probes, r, |k| ordered_into(&ordered, k, &mut buf_o))
                        }),
                    },
                    Arm {
                        name: "expanse_ordered_owned",
                        pass: Box::new(|r| pass_pred(&probes, r, |k| ordered_owned(&ordered, k))),
                    },
                    Arm {
                        name: "expanse_direct",
                        pass: Box::new(|r| {
                            pass_pred(&probes_direct, r, |k| direct_into(&direct, k, &mut buf_d))
                        }),
                    },
                    Arm {
                        name: "btreemap",
                        pass: Box::new(|r| pass_pred(&probes, r, |k| btree_pred(&btree, k))),
                    },
                ];
                let rows = time_round(arms, probes.len(), cli.round);
                cells.push(json!({
                    "population": n,
                    "order": order,
                    "probe_class": class,
                    "probes": probes.len(),
                    "answered": answered,
                    "rounds_raw": rows,
                }));
            }
            drop((ordered, direct, btree));
        }
    }
    emit("dns_predecessor", &cli, "cells", cells);
}
