//! Patricia / radix tries vs Expanse: full traversal and prefix scan.
//!
//! Two operations, both summing values and allocating nothing in the subject:
//! - **full traversal** over `u64` keys: every value visited once
//!   (`ExpanseMap::iter` vs each twin's `values()`). `qp-trie` does not
//!   iterate in byte order (it takes a byte's low nybble first), so its cell is
//!   a traversal, not an ordered scan; the other three are ordered;
//! - **prefix scan** over shared-prefix string keys: for 64 fixed two-hex-digit
//!   extensions of the shared prefix (about 1/256 of the keys each), sum the
//!   values of every key under it. `ExpanseStrMap` seeks once with
//!   `cursor_at_or_after` and steps a cursor that borrows its keys; the twins
//!   use their public prefix read, which on `patricia_tree` and
//!   `fast_radix_trie` reconstructs an owned key per entry — the only prefix
//!   read those crates offer, so that cost is part of what is measured.
//!
//! Every arm must return the same sums before any timing; an arm that panics,
//! loses keys or disagrees is recorded as invalid.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `patricia_scan` |
//! | `group` | 4 |
//! | `population` | 10k to 1M |
//! | `insertion_order` | both — `u64`: generator order and a Fisher–Yates permutation; strings: generator (random ids) and sorted; one row per order |
//! | `probes_and_reuse` | Full traversal once per repetition; 64 fixed prefixes per repetition; repetitions calibrated to `MIN_WINDOW` |
//! | `hit_rate` | N/A (scans) |
//! | `miss_gen_method` | None |
//! | `value_dereference` | Every visited value summed; the sum reaches `black_box` |
//! | `measured_region` | Scan passes only; builds and validation outside the window; one discarded warm-up pass per arm |
//! | `arm_symmetry` | Identical keys, values and build order; arms built separately; arm order rotates per round; the two twins without a borrowing prefix read pay an owned key per entry (disclosed) |
//! | `statistics` | Median per arm + geometric-mean Expanse/twin ratio with BCa 95% CI on per-round log ratios |
//! | `verdict` | **PENDING** `[unmeasured]`: no committed run yet. |

#[path = "art_common/mod.rs"]
mod art_common;
#[path = "patricia_common/mod.rs"]
mod patricia_common;

use expanse_trie::strmap::{ExpanseStrMap, NulFreeStr};
use patricia_common::{
    Arm, DISTS, Invalid, PatriciaMap, QpTrie, RadixMap, STRING_SEED, Twin, as_nulfree,
    build_expanse, build_expanse_str, build_twin, cli, emit, gen_paths, path_prefix, path_val,
    pkey, run_cell, shuffled, u64_dist, val,
};
use serde_json::{Map, Value, json};
use std::hint::black_box;
use std::time::{Duration, Instant};

/// Prefix length of the string scan cells (`https://example.com/api/v2/objects/`).
const SCAN_PREFIX_LEN: usize = 35;
/// Number of scanned prefixes per repetition.
const SCAN_PREFIXES: usize = 64;

fn push<'a, K, T: Twin<K>>(
    arms: &mut Vec<Arm<'a>>,
    invalid: &mut Vec<Invalid>,
    built: &'a Result<T, String>,
    expect: u64,
    ops: usize,
    f: fn(&T) -> u64,
) {
    match built {
        Ok(t) if f(t) == expect => arms.push(Arm {
            name: T::NAME.into(),
            ops,
            pass: Box::new(move |r| {
                let start = Instant::now();
                for _ in 0..r {
                    black_box(f(t));
                }
                start.elapsed()
            }),
        }),
        Ok(t) => invalid.push(Invalid {
            name: T::NAME.into(),
            reason: format!("scan sum {} differs from the subject's {expect}", f(t)),
        }),
        Err(e) => invalid.push(Invalid {
            name: T::NAME.into(),
            reason: e.clone(),
        }),
    }
}

fn traversal_row(dist: &str, order: &str, keys: &[u64], rounds: usize) -> Map<String, Value> {
    let bytes: Vec<[u8; 8]> = keys.iter().map(|&k| pkey(k)).collect();
    let vals: Vec<u64> = keys.iter().map(|&k| val(k)).collect();
    let e = build_expanse(keys);
    let pt: Result<PatriciaMap<u64>, String> = build_twin(&bytes, &vals);
    let rx: Result<RadixMap<u64>, String> = build_twin(&bytes, &vals);
    let qp: Result<QpTrie<[u8; 8], u64>, String> = build_twin(&bytes, &vals);
    let sum_e = |m: &art_common::ExpanseMap| m.iter().fold(0u64, |a, (_, v)| a.wrapping_add(v));
    let expect = sum_e(&e);
    let n = keys.len();
    let mut arms = vec![Arm {
        name: "expanse".into(),
        ops: n,
        pass: Box::new(|r| {
            let start = Instant::now();
            for _ in 0..r {
                black_box(sum_e(&e));
            }
            start.elapsed()
        }),
    }];
    let mut invalid = Vec::new();
    push::<[u8; 8], _>(&mut arms, &mut invalid, &pt, expect, n, |t| {
        Twin::<[u8; 8]>::sum_all(t)
    });
    push::<[u8; 8], _>(&mut arms, &mut invalid, &rx, expect, n, |t| {
        Twin::<[u8; 8]>::sum_all(t)
    });
    push::<[u8; 8], _>(&mut arms, &mut invalid, &qp, expect, n, |t| {
        Twin::<[u8; 8]>::sum_all(t)
    });
    let mut row = run_cell(arms, &invalid, rounds, true);
    row.insert("operation".into(), json!("full_traversal"));
    row.insert("distribution".into(), json!(dist));
    row.insert("order".into(), json!(order));
    row.insert("population".into(), json!(n));
    row.insert("ops_unit".into(), json!("entry visited"));
    row
}

/// The scanned prefixes: the shared prefix plus 64 fixed two-hex-digit ids.
fn scan_prefixes() -> Vec<Vec<u8>> {
    (0..SCAN_PREFIXES)
        .map(|i| {
            let mut p = path_prefix(SCAN_PREFIX_LEN);
            p.extend_from_slice(format!("{:02x}", (i * 4) as u8).as_bytes());
            p
        })
        .collect()
}

fn expanse_prefix_sum(m: &mut ExpanseStrMap, prefix: &NulFreeStr) -> u64 {
    let p = prefix.as_bytes();
    let mut c = m.cursor_at_or_after(prefix);
    let mut sum = 0u64;
    while let Some((k, slot)) = c.next() {
        if !k.starts_with(p) {
            break;
        }
        // SAFETY: the slot points at a live value word of the map, valid until
        // the next structural mutation, which the cursor's borrow of `m` rules out.
        sum = sum.wrapping_add(unsafe { slot.as_ptr().read() });
    }
    sum
}

fn prefix_sums<T: Twin<Vec<u8>>>(t: &T, prefixes: &[Vec<u8>]) -> u64 {
    prefixes
        .iter()
        .fold(0u64, |a, p| a.wrapping_add(t.sum_prefix(p)))
}

fn push_prefix_twin<'a, T: Twin<Vec<u8>>>(
    arms: &mut Vec<Arm<'a>>,
    invalid: &mut Vec<Invalid>,
    built: &'a Result<T, String>,
    prefixes: &'a [Vec<u8>],
    expect: u64,
) {
    match built {
        Ok(t) if prefix_sums(t, prefixes) == expect => arms.push(Arm {
            name: T::NAME.into(),
            ops: SCAN_PREFIXES,
            pass: Box::new(move |r| -> Duration {
                let start = Instant::now();
                for _ in 0..r {
                    black_box(prefix_sums(t, prefixes));
                }
                start.elapsed()
            }),
        }),
        Ok(t) => invalid.push(Invalid {
            name: T::NAME.into(),
            reason: format!(
                "prefix sums {} differ from the subject's {expect}",
                prefix_sums(t, prefixes)
            ),
        }),
        Err(e) => invalid.push(Invalid {
            name: T::NAME.into(),
            reason: e.clone(),
        }),
    }
}

fn prefix_row(order: &str, keys: &[Vec<u8>], rounds: usize) -> Map<String, Value> {
    let vals: Vec<u64> = keys.iter().map(|k| path_val(k)).collect();
    let nf = as_nulfree(keys);
    let mut e = build_expanse_str(&nf, &vals);
    let pt: Result<PatriciaMap<u64>, String> = build_twin(keys, &vals);
    let rx: Result<RadixMap<u64>, String> = build_twin(keys, &vals);
    let qp: Result<QpTrie<Vec<u8>, u64>, String> = build_twin(keys, &vals);
    let prefixes = scan_prefixes();
    let pnf = as_nulfree(&prefixes);
    let expect = pnf
        .iter()
        .fold(0u64, |a, p| a.wrapping_add(expanse_prefix_sum(&mut e, p)));
    let scanned: usize = prefixes
        .iter()
        .map(|p| keys.iter().filter(|k| k.starts_with(p)).count())
        .sum();
    let mut arms = vec![Arm {
        name: "expanse".into(),
        ops: SCAN_PREFIXES,
        pass: Box::new(|r| {
            let start = Instant::now();
            for _ in 0..r {
                let mut s = 0u64;
                for p in &pnf {
                    s = s.wrapping_add(expanse_prefix_sum(&mut e, p));
                }
                black_box(s);
            }
            start.elapsed()
        }),
    }];
    let mut invalid = Vec::new();
    push_prefix_twin(&mut arms, &mut invalid, &pt, &prefixes, expect);
    push_prefix_twin(&mut arms, &mut invalid, &rx, &prefixes, expect);
    push_prefix_twin(&mut arms, &mut invalid, &qp, &prefixes, expect);
    let mut row = run_cell(arms, &invalid, rounds, true);
    row.insert("operation".into(), json!("prefix_scan"));
    row.insert("distribution".into(), json!("prefixed_path"));
    row.insert("prefix_len".into(), json!(SCAN_PREFIX_LEN));
    row.insert("order".into(), json!(order));
    row.insert("population".into(), json!(keys.len()));
    row.insert("scanned_entries_per_rep".into(), json!(scanned));
    row.insert("ops_unit".into(), json!("prefix scanned"));
    row
}

fn main() {
    let cli = cli(&[10_000, 100_000, 1_000_000]);
    let mut rows: Vec<Value> = Vec::new();
    for &n in &cli.pops {
        for dist in DISTS {
            let keys = u64_dist(dist, n);
            for (order, ks) in [("generator", keys.clone()), ("shuffled", shuffled(&keys))] {
                let mut row = traversal_row(dist, order, &ks, cli.rounds);
                row.insert("raw_draws".into(), json!(n));
                rows.push(Value::Object(row));
            }
        }
        let keys = gen_paths(n, SCAN_PREFIX_LEN, STRING_SEED);
        let mut sorted = keys.clone();
        sorted.sort();
        for (order, ks) in [("generator", &keys), ("sorted", &sorted)] {
            rows.push(Value::Object(prefix_row(order, ks, cli.rounds)));
        }
    }
    emit("patricia_scan", "patricia_scan", &cli, rows);
}
