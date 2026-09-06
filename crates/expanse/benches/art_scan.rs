//! Pillar 4: ART Range Scan & In-Order Iteration
//!
//! Evaluates ordered iteration (full container traversal) and bounded range scans (k=10, 100, 1000)
//! across key distributions, comparing ExpanseMap's contiguous bitmap chunk iterator against
//! blart (ART) iterator and BTreeMap.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `art_scan` |
//! | `group` | 4 |
//! | `population` | 10k to 1M |
//! | `probes_and_reuse` | Full iteration, and bounded scans from `max(1000, 10^6/k)` shuffled starts per timed window |
//! | `hit_rate` | 100% (Existing keys in populated range) |
//! | `miss_gen_method` | None |
//! | `value_dereference` | `black_box(k ^ v)` during traversal |
//! | `measured_region` | Clean scan loop; starts and population built outside it |
//! | `arm_symmetry` | Symmetric keys, PRNG and starts; visited-element counts compared per round and the cell voided when they differ |
//! | `statistics` | Median + BCa 95% Bootstrap CI over paired rounds, arms rotated round-robin; every cell carries `rounds_raw` |
//! | `verdict` | **PASS** `[verified: RUN (b447dbc0, reference host)]`: every `k > 0` cell at 1M reverses to Expanse under scan starts that scale with `k`; the `k = 0` control reproduces the superseded run within 2%. |

#[path = "art_common/mod.rs"]
mod art_common;

use art_common::{
    ArtMap, BTreeMap, ExpanseMap, Mapped, PROBE_SHUFFLE_SEED, ToUBE, XorShift64, art_key, bca_ci,
    gen_clustered, gen_sequential, gen_uniform_random, median, rounds_raw, shuffle,
};
use serde_json::json;
use std::hint::black_box;
use std::time::Instant;

/// Starts visited in one timed bounded-scan window, so that every `k` visits
/// about a million elements rather than leaving `k = 10` a ten-element window.
///
/// Until #745 this harness scanned from **one** start key, `keys[len / 4]`, on
/// every round and for every `k`. At `k = 10` the timed region was ten element
/// visits bracketed by two clock reads, and the published `ns/element` divided
/// that whole window — clock reads included — by ten; the same start also meant
/// fifteen rounds re-walked one warm path. Mirrors the fix HOT took for the
/// same defect (`expanse_hot_bench::workload::scan_starts`, #731).
fn scan_starts(k: usize) -> usize {
    (1_000_000 / k.max(1)).max(1_000)
}

fn bench_scan_case(
    dist_name: &str,
    keys: &[u64],
    range_k: usize,
    rounds: usize,
) -> serde_json::Value {
    let n = keys.len();

    let mut expanse = ExpanseMap::new();
    let mut blart = ArtMap::new();
    let mut btree = BTreeMap::new();

    for &k in keys {
        expanse.insert(k, k.wrapping_mul(3));
        let _ = blart.try_insert(art_key(k), k.wrapping_mul(3));
        btree.insert(k, k.wrapping_mul(3));
    }

    // `try_insert` returns an error on a duplicate key and the loop above
    // discards it, so a distribution that draws the same key twice leaves the
    // three containers holding different populations and divides a full
    // iteration by a count none of them visited. Fail loudly instead
    // (AGENTS.md section 8.1).
    let held = btree.len();
    assert_eq!(
        expanse.len() as usize,
        held,
        "{dist_name} n={n}: ExpanseMap holds {} keys, BTreeMap {held}",
        expanse.len()
    );
    assert_eq!(
        blart.len(),
        held,
        "{dist_name} n={n}: blart holds {} keys, BTreeMap {held}",
        blart.len()
    );

    // Bounded scans start at shuffled population keys, cycled to the count
    // `scan_starts` asks for; the full-iteration case (`range_k == 0`) walks
    // the whole container and takes no starts.
    let starts: Vec<u64> = if range_k == 0 {
        Vec::new()
    } else {
        let mut pool = keys.to_vec();
        shuffle(&mut pool, &mut XorShift64::new(PROBE_SHUFFLE_SEED));
        pool.iter()
            .copied()
            .cycle()
            .take(scan_starts(range_k))
            .collect()
    };

    let mut expanse_times = Vec::with_capacity(rounds);
    let mut blart_times = Vec::with_capacity(rounds);
    let mut btree_times = Vec::with_capacity(rounds);
    let mut paired_ratios = Vec::with_capacity(rounds);

    let mut visited_per_round: Vec<usize> = Vec::with_capacity(rounds);

    for round in 0..rounds {
        let ((e_t, e_v), (b_t, b_v), (bt_t, bt_v)) = match round % 3 {
            0 => {
                let e = time_expanse_scan(&expanse, &starts, range_k, held);
                let b = time_blart_scan(&blart, &starts, range_k, held);
                let bt = time_btree_scan(&btree, &starts, range_k, held);
                (e, b, bt)
            }
            1 => {
                let b = time_blart_scan(&blart, &starts, range_k, held);
                let bt = time_btree_scan(&btree, &starts, range_k, held);
                let e = time_expanse_scan(&expanse, &starts, range_k, held);
                (e, b, bt)
            }
            _ => {
                let bt = time_btree_scan(&btree, &starts, range_k, held);
                let e = time_expanse_scan(&expanse, &starts, range_k, held);
                let b = time_blart_scan(&blart, &starts, range_k, held);
                (e, b, bt)
            }
        };

        // Both columns of a ratio must count the same elements, or a
        // divergence is divided into both as if it were one quantity.
        assert!(
            e_v == b_v && e_v == bt_v,
            "{dist_name} n={n} k={range_k} round {round}: visited counts differ \
             (Expanse {e_v}, blart {b_v}, BTreeMap {bt_v})"
        );
        let divisor = e_v.max(1) as f64;
        visited_per_round.push(e_v);

        let (e_ns, b_ns, bt_ns) = (e_t / divisor, b_t / divisor, bt_t / divisor);
        expanse_times.push(e_ns);
        blart_times.push(b_ns);
        btree_times.push(bt_ns);
        if b_ns > 0.0 {
            paired_ratios.push(e_ns / b_ns);
        }
    }

    let exp_med = median(expanse_times.clone());
    let blart_med = median(blart_times.clone());
    let btree_med = median(btree_times.clone());
    let (ratio_mean, ci_lo, ci_hi) = bca_ci(&paired_ratios);

    json!({
        "distribution": dist_name,
        "population": n,
        "keys_held": held,
        "range_k": range_k,
        // The methodology of this cell, checkable from the artifact: how many
        // scans one timed window held, and how many elements it visited.
        "scan_starts": starts.len(),
        "visited_per_round": visited_per_round,
        "expanse_ns_elem": exp_med,
        "blart_art_ns_elem": blart_med,
        "btree_ns_elem": btree_med,
        "ratio_vs_art": ratio_mean,
        "ratio_bca_ci_95": [ci_lo, ci_hi],
        "rounds_raw": rounds_raw(&[
            ("expanse_ns", &expanse_times),
            ("blart_art_ns", &blart_times),
            ("btree_ns", &btree_times),
            ("ratio_vs_art", &paired_ratios),
        ])
    })
}

#[inline(never)]
fn time_expanse_scan(
    map: &ExpanseMap,
    starts: &[u64],
    range_k: usize,
    held: usize,
) -> (f64, usize) {
    let t0 = Instant::now();
    let mut acc = 0u64;
    let mut visited = 0usize;
    if range_k == 0 {
        for (k, v) in map.iter() {
            black_box(k);
            acc ^= v;
        }
        visited = held;
    } else {
        for &s in starts {
            let mut c = 0usize;
            for (k, v) in map.range(s..=u64::MAX) {
                black_box(k);
                acc ^= v;
                c += 1;
                if c >= range_k {
                    break;
                }
            }
            visited += c;
        }
    }
    let t = t0.elapsed().as_nanos() as f64;
    black_box(acc);
    black_box(visited);
    (t, visited)
}

#[inline(never)]
fn time_blart_scan(
    map: &ArtMap<Mapped<ToUBE, u64>, u64>,
    starts: &[u64],
    range_k: usize,
    held: usize,
) -> (f64, usize) {
    let t0 = Instant::now();
    let mut acc = 0u64;
    let mut visited = 0usize;
    if range_k == 0 {
        for (k, v) in map.iter() {
            black_box(k);
            acc ^= *v;
        }
        visited = held;
    } else {
        for &s in starts {
            let mut c = 0usize;
            for (k, v) in map.range(art_key(s)..) {
                black_box(k);
                acc ^= *v;
                c += 1;
                if c >= range_k {
                    break;
                }
            }
            visited += c;
        }
    }
    let t = t0.elapsed().as_nanos() as f64;
    black_box(acc);
    black_box(visited);
    (t, visited)
}

#[inline(never)]
fn time_btree_scan(
    map: &BTreeMap<u64, u64>,
    starts: &[u64],
    range_k: usize,
    held: usize,
) -> (f64, usize) {
    let t0 = Instant::now();
    let mut acc = 0u64;
    let mut visited = 0usize;
    if range_k == 0 {
        for (k, v) in map.iter() {
            black_box(k);
            acc ^= *v;
        }
        visited = held;
    } else {
        for &s in starts {
            let mut c = 0usize;
            for (k, v) in map.range(s..) {
                black_box(k);
                acc ^= *v;
                c += 1;
                if c >= range_k {
                    break;
                }
            }
            visited += c;
        }
    }
    let t = t0.elapsed().as_nanos() as f64;
    black_box(acc);
    black_box(visited);
    (t, visited)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    let populations: &[usize] = if quick {
        &[10_000, 50_000]
    } else {
        &[10_000, 100_000, 1_000_000]
    };
    let rounds = if quick { 3 } else { 15 };

    let mut results = Vec::new();
    let mut rng = XorShift64::new(0);

    for &n in populations {
        let seq = gen_sequential(n);
        let clustered = gen_clustered(n, &mut rng);
        let uniform = gen_uniform_random(n, &mut rng);

        // Full in-order iteration (range_k = 0)
        results.push(bench_scan_case("sequential", &seq, 0, rounds));
        results.push(bench_scan_case("clustered", &clustered, 0, rounds));
        results.push(bench_scan_case("uniform_random", &uniform, 0, rounds));

        // Bounded range scans (k=10, 100, 1000)
        for &k in &[10, 100, 1000] {
            results.push(bench_scan_case("sequential", &seq, k, rounds));
            results.push(bench_scan_case("clustered", &clustered, k, rounds));
            results.push(bench_scan_case("uniform_random", &uniform, k, rounds));
        }
    }

    let output = json!({
        "benchmark": "art_scan",
        "workload_id": "art_scan",
        "quick": quick,
        "rounds": rounds,
        "results": results,
    });

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("=== art_scan (quick={quick}, rounds={rounds}) ===");
        for r in results {
            let d = r["distribution"].as_str().unwrap();
            let pop = r["population"].as_u64().unwrap();
            let k = r["range_k"].as_u64().unwrap();
            let exp = r["expanse_ns_elem"].as_f64().unwrap();
            let art = r["blart_art_ns_elem"].as_f64().unwrap();
            let rat = r["ratio_vs_art"].as_f64().unwrap();
            let ci = &r["ratio_bca_ci_95"];
            println!(
                "  pop={pop:7} | dist={d:15} | k={k:4} | Expanse: {exp:6.2} ns/el | ART: {art:6.2} ns/el | Ratio: {rat:5.2}x CI [{:.2}, {:.2}]",
                ci[0].as_f64().unwrap(),
                ci[1].as_f64().unwrap()
            );
        }
    }
}
