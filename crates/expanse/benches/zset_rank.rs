//! Pillar 3: ZRANK and ZCOUNT — rank and cardinality queries.
//!
//! This is the head-to-head the issue centers on: Expanse's `count_below`
//! (an `O(depth)` walk that sums precomputed subtree population counters) vs
//! the skip list's `O(log n)` span accumulation. Neither traverses the whole
//! set; both are sublinear. Published as measured — this is not a cell either
//! side is guaranteed to win.
//!
//! Scenarios (throughput, M queries/sec; also ns/query):
//!
//! * `zrank` — rank of a random existing member (member-map/dict lookup for the
//!   score, then the rank primitive).
//! * `zcount` — number of members in a random score window (two rank
//!   primitives).
//! * `zrank_by_rank` — the inverse (select): member at a random 0-based rank,
//!   via Expanse `by_count` vs skip-list span descent.
//!
//! Measured region: only the query loop. Query streams built in setup (rule 0).
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `domain_zset_rank` |
//! | `group` | 4 |
//! | `population` | 10k, 100k |
//! | `probes_and_reuse` | Rank queries |
//! | `hit_rate` | Rank queries |
//! | `miss_gen_method` | Bounded score window |
//! | `value_dereference` | `black_box(acc)` |
//! | `measured_region` | Clean |
//! | `arm_symmetry` | Symmetric |
//! | `statistics` | Median reduction |
//! | `verdict` | **PASS** `[verified: CODE READ]`: Rank query benchmark. |

#[path = "zset_common/mod.rs"]
mod zset_common;

use serde_json::json;
use std::hint::black_box;
use std::time::Instant;
use zset_common::{ExpanseZSet, SkiplistZSet, XorShift64, median, shuffled_members};

const SEED: u64 = 0x2A9C_0074_1234_5678;
const SCORE_RANGE: u32 = 1_000_000;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    zset_common::validate();

    let pop: u32 = if quick { 50_000 } else { 1_000_000 };
    let queries: usize = if quick { 100_000 } else { 1_000_000 };
    let rounds = if quick { 2 } else { 5 };

    let mut rng = XorShift64::new(SEED);
    let members = shuffled_members(pop, &mut rng);
    let scores: Vec<u32> = (0..pop).map(|_| rng.below(SCORE_RANGE)).collect();

    let mut exp = ExpanseZSet::new();
    let mut sl = SkiplistZSet::new(SEED ^ 0x1357);
    for i in 0..pop as usize {
        exp.zadd(members[i], scores[i]);
        sl.zadd(members[i], scores[i]);
    }
    let len = exp.len();

    // Query streams.
    let rank_targets: Vec<u32> = (0..queries)
        .map(|_| members[rng.below(pop) as usize])
        .collect();
    let count_lo: Vec<u32> = (0..queries).map(|_| rng.below(SCORE_RANGE)).collect();
    let count_wid: Vec<u32> = (0..queries)
        .map(|_| rng.below(SCORE_RANGE / 10 + 1))
        .collect();
    let select_targets: Vec<u64> = (0..queries).map(|_| rng.next() % len).collect();

    let mut exp_rk = Vec::new();
    let mut sl_rk = Vec::new();
    let mut exp_ct = Vec::new();
    let mut sl_ct = Vec::new();
    let mut exp_sel = Vec::new();
    let mut sl_sel = Vec::new();

    for _ in 0..rounds {
        // zrank
        let t = Instant::now();
        let mut acc = 0u64;
        for &m in &rank_targets {
            acc = acc.wrapping_add(exp.zrank(black_box(m)).unwrap_or(0));
        }
        let d = t.elapsed().as_secs_f64();
        black_box(acc);
        exp_rk.push(queries as f64 / d / 1e6);

        let t = Instant::now();
        let mut acc = 0u64;
        for &m in &rank_targets {
            acc = acc.wrapping_add(sl.zrank(black_box(m)).unwrap_or(0));
        }
        let d = t.elapsed().as_secs_f64();
        black_box(acc);
        sl_rk.push(queries as f64 / d / 1e6);

        // zcount
        let t = Instant::now();
        let mut acc = 0u64;
        for i in 0..queries {
            let lo = count_lo[i];
            acc = acc.wrapping_add(
                exp.zcount(black_box(lo), black_box(lo.saturating_add(count_wid[i]))),
            );
        }
        let d = t.elapsed().as_secs_f64();
        black_box(acc);
        exp_ct.push(queries as f64 / d / 1e6);

        let t = Instant::now();
        let mut acc = 0u64;
        for i in 0..queries {
            let lo = count_lo[i];
            acc = acc
                .wrapping_add(sl.zcount(black_box(lo), black_box(lo.saturating_add(count_wid[i]))));
        }
        let d = t.elapsed().as_secs_f64();
        black_box(acc);
        sl_ct.push(queries as f64 / d / 1e6);

        // zrank_by_rank (select)
        let t = Instant::now();
        let mut acc = 0usize;
        for &r in &select_targets {
            exp.zrange_by_rank(black_box(r), r, |m, _| {
                acc = acc.wrapping_add(m as usize);
            });
        }
        let d = t.elapsed().as_secs_f64();
        black_box(acc);
        exp_sel.push(queries as f64 / d / 1e6);

        let t = Instant::now();
        let mut acc = 0usize;
        for &r in &select_targets {
            sl.zrange_by_rank(black_box(r), r, |m, _| {
                acc = acc.wrapping_add(m as usize);
            });
        }
        let d = t.elapsed().as_secs_f64();
        black_box(acc);
        sl_sel.push(queries as f64 / d / 1e6);
    }

    let scenario = |exp: Vec<f64>, sl: Vec<f64>| {
        let e = median(exp);
        let s = median(sl);
        json!({
            "expanse_mops": e,
            "skiplist_mops": s,
            "expanse_ns": if e > 0.0 { 1e3 / e } else { 0.0 },
            "skiplist_ns": if s > 0.0 { 1e3 / s } else { 0.0 },
            "winner": if e >= s { "expanse" } else { "skiplist" },
        })
    };

    let results = json!({
        "population": pop,
        "queries_per_scenario": queries,
        "rounds": rounds,
        "scenarios": {
            "zrank": scenario(exp_rk, sl_rk),
            "zcount": scenario(exp_ct, sl_ct),
            "zrank_by_rank": scenario(exp_sel, sl_sel),
        }
    });

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    } else {
        println!("{results:#?}");
    }
}
