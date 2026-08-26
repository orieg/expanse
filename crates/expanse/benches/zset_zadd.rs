//! Pillar 1: ZADD — score insertion and in-place modification churn.
//!
//! Four scenarios, each measured as throughput (M ops/sec), interleaved A/B
//! (Expanse first, then SkipList+Dict) over several rounds with the median
//! reported (BENCHMARKING.md rules 1 and 5):
//!
//! * `fresh_insert` — cold build of `N` members with random scores.
//! * `score_update` — `M` score changes on existing members. Each is a
//!   composite-key delete-then-insert in both engines (the composite encoding
//!   admits no in-place score move), plus a member-map / dict update.
//! * `zincrby` — `M` increments on existing members (read-modify-write).
//! * `mixed_churn` — `M` ops of add-new / update / remove in steady state.
//!
//! Measured region: only the operation loop. Member permutations, score
//! streams, and pre-population are built in setup and excluded (rule 0).

#[path = "zset_common/mod.rs"]
mod zset_common;

use serde_json::json;
use std::hint::black_box;
use std::time::Instant;
use zset_common::{ExpanseZSet, SkiplistZSet, XorShift64, median, shuffled_members};

const SEED: u64 = 0x2ADD_0F30_1234_5678;
/// Bounded score domain: many members share scores, so tie-breaking by member
/// is exercised (a realistic leaderboard, not all-distinct scores).
const SCORE_RANGE: u32 = 1_000_000;

fn score_stream(n: usize, rng: &mut XorShift64) -> Vec<u32> {
    (0..n).map(|_| rng.below(SCORE_RANGE)).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    zset_common::validate();

    let pop: u32 = if quick { 50_000 } else { 1_000_000 };
    let ops: usize = if quick { 100_000 } else { 1_000_000 };
    let rounds = if quick { 2 } else { 5 };

    let mut rng = XorShift64::new(SEED);
    let members = shuffled_members(pop, &mut rng);
    let insert_scores = score_stream(pop as usize, &mut rng);

    // Streams for the update / incr / churn scenarios (indices into `members`).
    let update_targets: Vec<u32> = (0..ops).map(|_| members[rng.below(pop) as usize]).collect();
    let update_scores: Vec<u32> = (0..ops).map(|_| rng.below(SCORE_RANGE)).collect();
    let incr_targets: Vec<u32> = (0..ops).map(|_| members[rng.below(pop) as usize]).collect();
    let incr_deltas: Vec<i64> = (0..ops).map(|_| (rng.below(2000) as i64) - 1000).collect();

    let build_expanse = || {
        let mut z = ExpanseZSet::new();
        for (i, &m) in members.iter().enumerate() {
            z.zadd(m, insert_scores[i]);
        }
        z
    };
    let build_skiplist = || {
        let mut z = SkiplistZSet::new(SEED ^ 0xF00D);
        for (i, &m) in members.iter().enumerate() {
            z.zadd(m, insert_scores[i]);
        }
        z
    };

    let mut exp_fresh = Vec::new();
    let mut sl_fresh = Vec::new();
    let mut exp_upd = Vec::new();
    let mut sl_upd = Vec::new();
    let mut exp_inc = Vec::new();
    let mut sl_inc = Vec::new();
    let mut exp_chn = Vec::new();
    let mut sl_chn = Vec::new();

    for _ in 0..rounds {
        // --- fresh_insert (cold build) ---
        let t = Instant::now();
        let z = build_expanse();
        let d = t.elapsed().as_secs_f64();
        black_box(z.len());
        exp_fresh.push(pop as f64 / d / 1e6);

        let t = Instant::now();
        let z = build_skiplist();
        let d = t.elapsed().as_secs_f64();
        black_box(z.len());
        sl_fresh.push(pop as f64 / d / 1e6);

        // --- score_update (delete+insert of composite key) ---
        let mut ze = build_expanse();
        let t = Instant::now();
        for i in 0..ops {
            ze.zadd(black_box(update_targets[i]), black_box(update_scores[i]));
        }
        let d = t.elapsed().as_secs_f64();
        black_box(ze.len());
        exp_upd.push(ops as f64 / d / 1e6);

        let mut zs = build_skiplist();
        let t = Instant::now();
        for i in 0..ops {
            zs.zadd(black_box(update_targets[i]), black_box(update_scores[i]));
        }
        let d = t.elapsed().as_secs_f64();
        black_box(zs.len());
        sl_upd.push(ops as f64 / d / 1e6);

        // --- zincrby (read-modify-write) ---
        let mut ze = build_expanse();
        let t = Instant::now();
        for i in 0..ops {
            black_box(ze.zincrby(black_box(incr_targets[i]), black_box(incr_deltas[i])));
        }
        let d = t.elapsed().as_secs_f64();
        exp_inc.push(ops as f64 / d / 1e6);

        let mut zs = build_skiplist();
        let t = Instant::now();
        for i in 0..ops {
            black_box(zs.zincrby(black_box(incr_targets[i]), black_box(incr_deltas[i])));
        }
        let d = t.elapsed().as_secs_f64();
        sl_inc.push(ops as f64 / d / 1e6);

        // --- mixed_churn (add-new / update / remove in steady state) ---
        let mut ze = build_expanse();
        let mut next_new = pop;
        let t = Instant::now();
        for i in 0..ops {
            match i % 3 {
                0 => {
                    ze.zadd(black_box(next_new), black_box(update_scores[i]));
                    next_new = next_new.wrapping_add(1);
                }
                1 => {
                    ze.zadd(black_box(update_targets[i]), black_box(update_scores[i]));
                }
                _ => {
                    black_box(ze.zrem(black_box(update_targets[i])));
                }
            }
        }
        let d = t.elapsed().as_secs_f64();
        black_box(ze.len());
        exp_chn.push(ops as f64 / d / 1e6);

        let mut zs = build_skiplist();
        let mut next_new = pop;
        let t = Instant::now();
        for i in 0..ops {
            match i % 3 {
                0 => {
                    zs.zadd(black_box(next_new), black_box(update_scores[i]));
                    next_new = next_new.wrapping_add(1);
                }
                1 => {
                    zs.zadd(black_box(update_targets[i]), black_box(update_scores[i]));
                }
                _ => {
                    black_box(zs.zrem(black_box(update_targets[i])));
                }
            }
        }
        let d = t.elapsed().as_secs_f64();
        black_box(zs.len());
        sl_chn.push(ops as f64 / d / 1e6);
    }

    let scenario = |exp: Vec<f64>, sl: Vec<f64>, unit_n: f64| {
        let e = median(exp);
        let s = median(sl);
        json!({
            "expanse_mops": e,
            "skiplist_mops": s,
            "ops": unit_n,
            "winner": if e >= s { "expanse" } else { "skiplist" },
        })
    };

    let results = json!({
        "population": pop,
        "ops_per_scenario": ops,
        "rounds": rounds,
        "scenarios": {
            "fresh_insert": scenario(exp_fresh, sl_fresh, pop as f64),
            "score_update": scenario(exp_upd, sl_upd, ops as f64),
            "zincrby": scenario(exp_inc, sl_inc, ops as f64),
            "mixed_churn": scenario(exp_chn, sl_chn, ops as f64),
        }
    });

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    } else {
        println!("{results:#?}");
    }
}
