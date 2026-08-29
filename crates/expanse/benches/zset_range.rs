//! Pillar 2: ZRANGEBYSCORE and ZREVRANGEBYSCORE — range iteration.
//!
//! Throughput is reported as **members yielded per second** (M elem/sec) so
//! small-window and large-window scenarios are on the same scale. Four
//! scenarios, interleaved Expanse-then-SkipList over several rounds, median
//! reported:
//!
//! * `forward_small`  — many `ZRANGEBYSCORE` queries, ~64-member windows.
//! * `forward_large`  — few `ZRANGEBYSCORE` queries, ~8192-member windows.
//! * `reverse_small`  — many `ZREVRANGEBYSCORE` queries, ~64-member windows.
//! * `reverse_large`  — few `ZREVRANGEBYSCORE` queries, ~8192-member windows.
//!
//! The reverse scenarios report **two** Expanse arms per cell: the pre-#341
//! `emulated` arm (each descending step an `O(depth)` `prev_at_or_before`
//! re-descent) and the `native` arm (the reverse ordered iterator `range_rev`,
//! #341 — `O(1)` amortized per member). `expanse_melem_s` and the `winner`
//! track the native arm; `expanse_emulated_melem_s` retains the emulated
//! baseline for the emulated / native / skiplist comparison.
//!
//! Measured region: only the query loop. Set construction and query streams
//! are built in setup (rule 0).
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `domain_zset_range` |
//! | `group` | 4 |
//! | `population` | 10k, 100k |
//! | `probes_and_reuse` | Range windows |
//! | `hit_rate` | Range scan |
//! | `miss_gen_method` | Bounded score window |
//! | `value_dereference` | `black_box((m, sc))` |
//! | `measured_region` | Clean |
//! | `arm_symmetry` | Symmetric |
//! | `statistics` | Median reduction |
//! | `verdict` | **PASS** `[verified: CODE READ]`: Range scan benchmark. |

#[path = "zset_common/mod.rs"]
mod zset_common;

use serde_json::json;
use std::hint::black_box;
use std::time::Instant;
use zset_common::{ExpanseZSet, SkiplistZSet, XorShift64, median, shuffled_members};

const SEED: u64 = 0x2A9E_C0DE_5678_1234;
const SCORE_RANGE: u32 = 1_000_000;

/// Window width (in score units) that yields approximately `target` members at
/// the given density.
fn width_for(target: u32, pop: u32) -> u32 {
    ((target as u64 * SCORE_RANGE as u64) / pop.max(1) as u64).max(1) as u32
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    zset_common::validate();

    let pop: u32 = if quick { 50_000 } else { 500_000 };
    let rounds = if quick { 2 } else { 5 };
    let small_queries = if quick { 4_000 } else { 20_000 };
    let large_queries = if quick { 100 } else { 500 };

    let mut rng = XorShift64::new(SEED);
    let members = shuffled_members(pop, &mut rng);
    let scores: Vec<u32> = (0..pop).map(|_| rng.below(SCORE_RANGE)).collect();

    let mut exp = ExpanseZSet::new();
    let mut sl = SkiplistZSet::new(SEED ^ 0xBEEF);
    for i in 0..pop as usize {
        exp.zadd(members[i], scores[i]);
        sl.zadd(members[i], scores[i]);
    }

    let small_w = width_for(64, pop);
    let large_w = width_for(8192, pop);
    let starts_small: Vec<u32> = (0..small_queries)
        .map(|_| rng.below(SCORE_RANGE.saturating_sub(small_w).max(1)))
        .collect();
    let starts_large: Vec<u32> = (0..large_queries)
        .map(|_| rng.below(SCORE_RANGE.saturating_sub(large_w).max(1)))
        .collect();

    // Run one scenario: `run_exp`/`run_sl` return (elements, time) for a full
    // sweep of the query stream. Returns median M elem/sec for each engine.
    let mut exp_fs = Vec::new();
    let mut sl_fs = Vec::new();
    let mut exp_fl = Vec::new();
    let mut sl_fl = Vec::new();
    let mut exp_rs = Vec::new();
    let mut exp_rs_native = Vec::new();
    let mut sl_rs = Vec::new();
    let mut exp_rl = Vec::new();
    let mut exp_rl_native = Vec::new();
    let mut sl_rl = Vec::new();

    for _ in 0..rounds {
        // forward_small
        let (n, d) = {
            let mut cnt = 0usize;
            let t = Instant::now();
            for &s in &starts_small {
                cnt += exp.zrangebyscore(s, s + small_w, |m, sc| {
                    black_box((m, sc));
                });
            }
            (cnt, t.elapsed().as_secs_f64())
        };
        exp_fs.push(n as f64 / d / 1e6);
        let (n, d) = {
            let mut cnt = 0usize;
            let t = Instant::now();
            for &s in &starts_small {
                cnt += sl.zrangebyscore(s, s + small_w, |m, sc| {
                    black_box((m, sc));
                });
            }
            (cnt, t.elapsed().as_secs_f64())
        };
        sl_fs.push(n as f64 / d / 1e6);

        // forward_large
        let (n, d) = {
            let mut cnt = 0usize;
            let t = Instant::now();
            for &s in &starts_large {
                cnt += exp.zrangebyscore(s, s + large_w, |m, sc| {
                    black_box((m, sc));
                });
            }
            (cnt, t.elapsed().as_secs_f64())
        };
        exp_fl.push(n as f64 / d / 1e6);
        let (n, d) = {
            let mut cnt = 0usize;
            let t = Instant::now();
            for &s in &starts_large {
                cnt += sl.zrangebyscore(s, s + large_w, |m, sc| {
                    black_box((m, sc));
                });
            }
            (cnt, t.elapsed().as_secs_f64())
        };
        sl_fl.push(n as f64 / d / 1e6);

        // reverse_small (emulated arm)
        let (n, d) = {
            let mut cnt = 0usize;
            let t = Instant::now();
            for &s in &starts_small {
                cnt += exp.zrevrangebyscore_emulated(s, s + small_w, |m, sc| {
                    black_box((m, sc));
                });
            }
            (cnt, t.elapsed().as_secs_f64())
        };
        exp_rs.push(n as f64 / d / 1e6);
        // reverse_small (native arm)
        let (n, d) = {
            let mut cnt = 0usize;
            let t = Instant::now();
            for &s in &starts_small {
                cnt += exp.zrevrangebyscore(s, s + small_w, |m, sc| {
                    black_box((m, sc));
                });
            }
            (cnt, t.elapsed().as_secs_f64())
        };
        exp_rs_native.push(n as f64 / d / 1e6);
        let (n, d) = {
            let mut cnt = 0usize;
            let t = Instant::now();
            for &s in &starts_small {
                cnt += sl.zrevrangebyscore(s, s + small_w, |m, sc| {
                    black_box((m, sc));
                });
            }
            (cnt, t.elapsed().as_secs_f64())
        };
        sl_rs.push(n as f64 / d / 1e6);

        // reverse_large (emulated arm)
        let (n, d) = {
            let mut cnt = 0usize;
            let t = Instant::now();
            for &s in &starts_large {
                cnt += exp.zrevrangebyscore_emulated(s, s + large_w, |m, sc| {
                    black_box((m, sc));
                });
            }
            (cnt, t.elapsed().as_secs_f64())
        };
        exp_rl.push(n as f64 / d / 1e6);
        // reverse_large (native arm)
        let (n, d) = {
            let mut cnt = 0usize;
            let t = Instant::now();
            for &s in &starts_large {
                cnt += exp.zrevrangebyscore(s, s + large_w, |m, sc| {
                    black_box((m, sc));
                });
            }
            (cnt, t.elapsed().as_secs_f64())
        };
        exp_rl_native.push(n as f64 / d / 1e6);
        let (n, d) = {
            let mut cnt = 0usize;
            let t = Instant::now();
            for &s in &starts_large {
                cnt += sl.zrevrangebyscore(s, s + large_w, |m, sc| {
                    black_box((m, sc));
                });
            }
            (cnt, t.elapsed().as_secs_f64())
        };
        sl_rl.push(n as f64 / d / 1e6);
    }

    let scenario = |exp: Vec<f64>, sl: Vec<f64>| {
        let e = median(exp);
        let s = median(sl);
        json!({
            "expanse_melem_s": e,
            "skiplist_melem_s": s,
            "winner": if e >= s { "expanse" } else { "skiplist" },
        })
    };

    // Reverse cells carry emulated + native Expanse arms; the headline
    // `expanse_melem_s` and `winner` track the native (`range_rev`) arm.
    let scenario_rev = |exp_emul: Vec<f64>, exp_native: Vec<f64>, sl: Vec<f64>| {
        let emul = median(exp_emul);
        let native = median(exp_native);
        let s = median(sl);
        json!({
            "expanse_melem_s": native,
            "expanse_native_melem_s": native,
            "expanse_emulated_melem_s": emul,
            "skiplist_melem_s": s,
            "native_speedup_over_emulated": if emul > 0.0 { native / emul } else { 0.0 },
            "winner": if native >= s { "expanse" } else { "skiplist" },
        })
    };

    let results = json!({
        "population": pop,
        "rounds": rounds,
        "small_window_members": 64,
        "large_window_members": 8192,
        "scenarios": {
            "forward_small": scenario(exp_fs, sl_fs),
            "forward_large": scenario(exp_fl, sl_fl),
            "reverse_small": scenario_rev(exp_rs, exp_rs_native, sl_rs),
            "reverse_large": scenario_rev(exp_rl, exp_rl_native, sl_rl),
        }
    });

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    } else {
        println!("{results:#?}");
    }
}
