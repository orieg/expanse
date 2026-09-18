//! Concurrent YCSB benchmark harness (Refs #1006, `docs/benchmarks/concurrency/METHODOLOGY.md` §20).
//!
//! Multi-threaded YCSB execution measuring scaling and skew retention across
//! physical P-cores under Zipfian key choice (θ = 0.99) and uniform key choice (θ = 0).
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `concurrency_ycsb` |
//! | `group` | 5 |
//! | `emits` | `concurrency_ycsb_a`, `concurrency_ycsb_b`, `concurrency_ycsb_d`, `concurrency_ycsb_f`, `concurrency_ycsb_a0`, `concurrency_ycsb_b0`, `concurrency_ycsb_f0`, `concurrency_ycsb_c`, `concurrency_ycsb_c0`, `concurrency_ycsb_ac`, `concurrency_ycsb_fc`, `concurrency_ycsb_a_dram`, `concurrency_ycsb_c_dram` |
//! | `population` | prefill 2^20 keys (1,048,576 keys) drawn from XorShift64 over the full 64-bit space, distinct, present before the window opens; A-dram and C-dram prefill 2^24 keys (16,777,216 keys); quick runs prefill 4,096 keys |
//! | `insertion_order` | sorted — prefill ascending in every arm, matching expanse-hot-bench and writer_scaling convention |
//! | `probes_and_reuse` | T concurrent worker threads executing the family mix across pre-generated operation streams; point lookups and updates/inserts/RMW |
//! | `hit_rate` | 100% — every point read names a present key, and the harness counts misses and reports a cell with any as void; miss_gen_method is n/a |
//! | `miss_gen_method` | n/a |
//! | `value_dereference` | every read folds its value into a per-thread accumulator passed to black_box after the window; every write's return value is folded likewise (the previous value for a map insert and for fetch_add; a value-cell store returns nothing and folds 0); the post-window oracle checks every population key against the per-thread last writes, the per-key RMW counts and the final count |
//! | `measured_region` | barrier release to join of last thread, with each thread's own loop timed between two instants taken before anything it owns can drop; prefill, stream generation, Zipfian tables, reader-handle registration and deregistration, stream deallocation, post-run checks and teardown outside |
//! | `arm_symmetry` | symmetric across arms and thread counts T in {1, 2, 4, 8} (and extra load points T in {3, 6} for olc and mutex on families A and B); one generic operation loop serves every arm |
//! | `statistics` | throughput total M ops/s emitted raw, paired bootstrap BCa 95% CI across interleaved Williams rounds; lock fallbacks and their causes, lock restarts, read-validation failures and stripe contended acquisitions from the occ-stats build, which emits no timing |
//! | `verdict` | pending measurement |
//!
//! ## Builds and roles (AGENTS.md §6, METHODOLOGY §20.12 item 1)
//!
//! * Throughput comes from the uninstrumented release build (`--role throughput`),
//!   which refuses to run if `occ-stats` is enabled.
//! * Diagnostic counters come from the `occ-stats` build (`--role occ-stats`), which
//!   requires `--features occ-stats` and emits no elapsed time and no throughput.
//! * `--role latency` (§20.14 (a)) is not implemented: the harness refuses it
//!   with a non-zero exit rather than running the throughput loop under its name.
//!
//! The stream generator, the timed loop and the oracle live in
//! `benches/ycsb_concurrent_common/mod.rs`, which
//! `tests/test_ycsb_concurrent.rs` includes too.

use expanse_trie::occ_stats::Stat;

#[path = "../benches/ycsb_common/mod.rs"]
mod ycsb_common;
#[path = "../benches/ycsb_concurrent_common/mod.rs"]
mod ycsb_concurrent_common;

use ycsb_concurrent_common::{
    Arm, DASH_SHARDS, DEFAULT_SUITE_SEED, DRAM_POPULATION_N, Family, NUM_STRIPES,
    QUICK_OPS_PER_THREAD, QUICK_POPULATION_N, RANK_K, STANDARD_OPS_PER_THREAD,
    STANDARD_POPULATION_N, census_json, generate_initial_population, generate_thread_stream,
    run_cell, run_cell_latency, run_cell_monotonicity, tally_expected,
};

/// The six causes that partition `Stat::LockFallbacks` (as `writer_scaling.rs` names them).
const CAUSES: [(Stat, &str); 6] = [
    (Stat::FallbackCapExpansion, "cap_expansion"),
    (Stat::FallbackImmediateConversion, "immediate_conversion"),
    (Stat::FallbackBranchSplit, "branch_split"),
    (Stat::FallbackRootGrowth, "root_growth"),
    (Stat::FallbackContention, "contention"),
    (Stat::FallbackUnknownTag, "unknown_tag"),
];

/// Execution role.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Role {
    Throughput,
    OccStats,
    Latency,
    Monotonicity,
}

impl Role {
    fn parse(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "throughput" => Ok(Self::Throughput),
            "occ-stats" | "counters" => Ok(Self::OccStats),
            "latency" => Ok(Self::Latency),
            "monotonicity" => Ok(Self::Monotonicity),
            _ => Err(format!(
                "unknown role '{s}'; expected throughput, occ-stats, latency, or monotonicity"
            )),
        }
    }
}

/// Refuses a role the build cannot serve. Returns the message to exit on.
fn role_refusal(role: Role, has_occ_stats: bool) -> Option<&'static str> {
    match role {
        Role::Throughput if has_occ_stats => Some(
            "build/role mismatch: occ-stats is ON but role is 'throughput' (AGENTS.md §6: timings never come from an occ-stats build)",
        ),
        Role::OccStats if !has_occ_stats => Some(
            "build/role mismatch: occ-stats is OFF but role is 'occ-stats' (AGENTS.md §6: counters need --features occ-stats)",
        ),
        Role::Latency if has_occ_stats => Some(
            "build/role mismatch: occ-stats is ON but role is 'latency' (latency requires uninstrumented build)",
        ),
        Role::Monotonicity if has_occ_stats => Some(
            "build/role mismatch: occ-stats is ON but role is 'monotonicity' (monotonicity requires uninstrumented build)",
        ),
        _ => None,
    }
}

fn json_f64_list(vals: &[f64]) -> String {
    format!(
        "[{}]",
        vals.iter()
            .map(|v| format!("{v:.9}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn main() {
    let mut arm = Arm::Olc;
    let mut family = Family::A;
    let mut threads = 1usize;
    let mut round = 0usize;
    let mut position = 0usize;
    let mut quick = false;
    let mut role = Role::Throughput;
    let mut suite_seed = DEFAULT_SUITE_SEED;
    let mut is_self_test = false;

    let args: Vec<String> = std::env::args().collect();
    let mut idx = 1;
    while idx < args.len() {
        match args[idx].as_str() {
            "--arm" => {
                idx += 1;
                arm = Arm::parse(&args[idx]).expect("valid arm");
            }
            "--family" => {
                idx += 1;
                family = Family::parse(&args[idx]).expect("valid family");
            }
            "--threads" | "--writers" => {
                idx += 1;
                threads = args[idx].parse().expect("valid thread count");
            }
            "--round" => {
                idx += 1;
                round = args[idx].parse().expect("valid round");
            }
            "--position" => {
                idx += 1;
                position = args[idx].parse().expect("valid position");
            }
            "--quick" => {
                quick = true;
            }
            "--role" => {
                idx += 1;
                role = Role::parse(&args[idx]).expect("valid role");
            }
            "--seed" => {
                idx += 1;
                suite_seed = args[idx].parse().expect("valid seed");
            }
            "--self-test" => {
                is_self_test = true;
            }
            "--help" | "-h" => {
                println!(
                    "Usage: cargo run --release -p expanse-trie --example ycsb_concurrent -- [OPTIONS]"
                );
                println!("Options:");
                println!("  --arm <olc|mutex|skip|dash|rwbtree>");
                println!("  --family <A|B|D|F|A0|B0|F0|C|C0|Ac|Fc|A-dram|C-dram>");
                println!("  --threads <1..8>");
                println!("  --round <r>");
                println!("  --position <p>");
                println!("  --role <throughput|occ-stats|latency|monotonicity>");
                println!("  --seed <u64>");
                println!("  --quick");
                println!("  --self-test");
                return;
            }
            arg => panic!("unknown argument '{arg}'"),
        }
        idx += 1;
    }

    let has_occ_stats = cfg!(feature = "occ-stats");
    if let Some(msg) = role_refusal(role, has_occ_stats) {
        eprintln!("{msg}");
        std::process::exit(2);
    }
    if is_self_test {
        println!("ycsb_concurrent self-test: OK");
        return;
    }
    assert!(threads >= 1, "--threads must be at least 1");

    let population_n = if quick {
        QUICK_POPULATION_N
    } else if family.is_dram() {
        DRAM_POPULATION_N
    } else {
        STANDARD_POPULATION_N
    };
    let ops_per_thread = if quick {
        QUICK_OPS_PER_THREAD
    } else {
        STANDARD_OPS_PER_THREAD
    };

    // Population, streams and the oracle's expectations: all before the window.
    let initial_keys = generate_initial_population(population_n);
    let mut sorted_keys = initial_keys.clone();
    sorted_keys.sort_unstable();

    let mut streams = Vec::with_capacity(threads);
    let mut stream0_hist = None;
    for t in 0..threads {
        let (stream, hist) = generate_thread_stream(
            family,
            t,
            threads,
            ops_per_thread,
            population_n,
            &initial_keys,
            &sorted_keys,
            suite_seed,
        );
        if t == 0 {
            stream0_hist = Some(hist);
        }
        streams.push(stream);
    }
    let expected = tally_expected(&streams);
    let hist = stream0_hist.expect("at least one thread");
    let total_ops = (threads * ops_per_thread) as u64;
    let mut f = Vec::new();
    f.push(format!("\"workload_id\":\"{}\"", family.tag()));
    f.push(format!("\"arm\":\"{}\"", arm.as_str()));
    f.push(format!("\"family\":\"{}\"", family.as_str()));
    f.push(format!("\"threads\":{threads}"));
    f.push(format!("\"round\":{round}"));
    f.push(format!("\"position\":{position}"));
    f.push(format!(
        "\"role\":\"{}\"",
        match role {
            Role::Throughput => "throughput",
            Role::OccStats => "occ-stats",
            Role::Latency => "latency",
            Role::Monotonicity => "monotonicity",
        }
    ));
    // What the cell was, as the harness ran it; the driver checks these
    // against the registration instead of stamping its own constants.
    f.push(format!("\"population\":{population_n}"));
    f.push(format!("\"ops_per_thread\":{ops_per_thread}"));
    f.push(format!("\"seed\":{suite_seed}"));
    f.push(format!("\"theta\":{}", family.theta()));
    f.push(format!("\"update_idiom\":\"{}\"", arm.update_idiom()));
    f.push(format!("\"value_type\":\"{}\"", arm.value_type()));
    if arm == Arm::Dash {
        f.push(format!("\"shard_amount\":{DASH_SHARDS}"));
    }
    if family.is_rmw() {
        f.push(format!("\"rmw_provider\":\"{}\"", arm.rmw_provider()));
        if arm == Arm::Olc {
            f.push(format!("\"rmw_stripes\":{NUM_STRIPES}"));
        }
    }
    f.push(format!(
        "\"rank_histogram\":{{\"stream\":0,\"draws\":{},\"k\":[{}],\"observed_share\":{}}}",
        hist.draws,
        RANK_K
            .iter()
            .map(|k| k.to_string())
            .collect::<Vec<_>>()
            .join(","),
        json_f64_list(&hist.shares()),
    ));

    f.push(format!("\"ops\":{total_ops}"));
    f.push(format!("\"read_ops\":{}", expected.read_ops));
    f.push(format!("\"write_ops\":{}", expected.write_ops()));
    f.push(format!("\"rmw_ops\":{}", expected.rmw_ops));
    f.push(format!("\"insert_ops\":{}", expected.insert_ops));

    match role {
        Role::Throughput => {
            drop(initial_keys);
            let out = run_cell(arm, family, &sorted_keys, streams, &expected);
            let nvcsw_per_op = out.total_nvcsw as f64 / total_ops.max(1) as f64;
            f.push(format!("\"read_misses\":{}", out.window.tally.read_misses));
            f.push(format!(
                "\"write_misses\":{}",
                out.window.tally.write_misses
            ));
            f.push(format!(
                "\"successful_inserts\":{}",
                out.window.tally.successful_inserts
            ));
            f.push(format!("\"value_sum\":{}", out.oracle.value_sum));
            f.push(format!(
                "\"per_key_mismatches\":{}",
                out.oracle.per_key_mismatches
            ));
            f.push(format!("\"lost_updates\":{}", out.oracle.lost_updates));
            f.push(format!(
                "\"missing_population_keys\":{}",
                out.oracle.missing_population_keys
            ));
            f.push(format!("\"final_count\":{}", out.oracle.final_count));
            f.push(format!(
                "\"expected_final_count\":{}",
                out.oracle.expected_final_count
            ));
            f.push(format!(
                "\"oracle\":\"{}\"",
                out.verdict.replace('\\', "\\\\").replace('"', "\\\"")
            ));
            if let Some(m) = out.mem_used {
                f.push(format!("\"mem_used\":{m}"));
            }
            let total_mops = total_ops as f64 / out.window.elapsed_s / 1_000_000.0;
            f.push(format!("\"elapsed_s\":{:.9}", out.window.elapsed_s));
            f.push(format!("\"total_mops\":{total_mops:.6}"));
            f.push(format!(
                "\"thread_elapsed_s\":{}",
                json_f64_list(&out.window.thread_elapsed_s)
            ));
            f.push(format!("\"nvcsw_per_op\":{nvcsw_per_op:.9}"));
        }
        Role::OccStats => {
            drop(initial_keys);
            let out = run_cell(arm, family, &sorted_keys, streams, &expected);
            let nvcsw_per_op = out.total_nvcsw as f64 / total_ops.max(1) as f64;
            f.push(format!("\"read_misses\":{}", out.window.tally.read_misses));
            f.push(format!(
                "\"write_misses\":{}",
                out.window.tally.write_misses
            ));
            f.push(format!(
                "\"successful_inserts\":{}",
                out.window.tally.successful_inserts
            ));
            f.push(format!("\"value_sum\":{}", out.oracle.value_sum));
            f.push(format!(
                "\"per_key_mismatches\":{}",
                out.oracle.per_key_mismatches
            ));
            f.push(format!("\"lost_updates\":{}", out.oracle.lost_updates));
            f.push(format!(
                "\"missing_population_keys\":{}",
                out.oracle.missing_population_keys
            ));
            f.push(format!("\"final_count\":{}", out.oracle.final_count));
            f.push(format!(
                "\"expected_final_count\":{}",
                out.oracle.expected_final_count
            ));
            f.push(format!(
                "\"oracle\":\"{}\"",
                out.verdict.replace('\\', "\\\\").replace('"', "\\\"")
            ));
            if let Some(m) = out.mem_used {
                f.push(format!("\"mem_used\":{m}"));
            }
            f.push(format!("\"nvcsw_per_op\":{nvcsw_per_op:.9}"));
            let c = out
                .window
                .counters
                .expect("the occ-stats build snapshots its counters around the window");
            let causes = CAUSES
                .iter()
                .map(|(s, name)| format!("\"{name}\":{}", c[*s as usize]))
                .collect::<Vec<_>>()
                .join(",");
            let read_ops_counted = c[Stat::ReadOps as usize];
            let read_attempts = c[Stat::ReadAttempts as usize];
            f.push(format!(
                "\"lock_fallbacks\":{}",
                c[Stat::LockFallbacks as usize]
            ));
            f.push(format!("\"fallback_causes_total\":{{{causes}}}"));
            f.push(format!(
                "\"lock_restarts\":{}",
                c[Stat::LockRestarts as usize]
            ));
            f.push(format!("\"occ_read_ops\":{read_ops_counted}"));
            f.push(format!("\"occ_read_attempts\":{read_attempts}"));
            f.push(format!(
                "\"read_validation_failures\":{}",
                read_attempts.saturating_sub(read_ops_counted)
            ));
            f.push(format!(
                "\"read_fallbacks\":{}",
                c[Stat::ReadFallbacks as usize]
            ));
            f.push(format!("\"occ_write_ops\":{}", c[Stat::WriteOps as usize]));
            if family.is_rmw() && arm == Arm::Olc {
                f.push(format!(
                    "\"stripe_contended_acquisitions\":{}",
                    out.contended_acquisitions
                ));
            }
        }
        Role::Latency => {
            drop(initial_keys);
            let out = run_cell_latency(arm, family, &sorted_keys, streams, &expected, quick);
            let nvcsw_per_op = out.total_nvcsw as f64 / total_ops.max(1) as f64;
            f.push(format!("\"read_misses\":{}", out.tally.read_misses));
            f.push(format!("\"write_misses\":{}", out.tally.write_misses));
            f.push(format!(
                "\"successful_inserts\":{}",
                out.tally.successful_inserts
            ));
            f.push(format!("\"value_sum\":{}", out.oracle.value_sum));
            f.push(format!(
                "\"per_key_mismatches\":{}",
                out.oracle.per_key_mismatches
            ));
            f.push(format!("\"lost_updates\":{}", out.oracle.lost_updates));
            f.push(format!(
                "\"missing_population_keys\":{}",
                out.oracle.missing_population_keys
            ));
            f.push(format!("\"final_count\":{}", out.oracle.final_count));
            f.push(format!(
                "\"expected_final_count\":{}",
                out.oracle.expected_final_count
            ));
            f.push(format!(
                "\"oracle\":\"{}\"",
                out.verdict.replace('\\', "\\\\").replace('"', "\\\"")
            ));
            if let Some(m) = out.mem_used {
                f.push(format!("\"mem_used\":{m}"));
            }
            f.push(format!("\"tsc_hz\":{}", out.tsc_hz));
            f.push(format!(
                "\"bracket_overhead_ns\":{:.3}",
                out.bracket_overhead_ns
            ));
            f.push(format!("\"nvcsw_per_op\":{nvcsw_per_op:.9}"));
            let latency_json = out
                .latency
                .iter()
                .map(|r| r.to_json())
                .collect::<Vec<_>>()
                .join(",");
            f.push(format!("\"latency\":[{latency_json}]"));
        }
        Role::Monotonicity => {
            let out =
                run_cell_monotonicity(arm, family, &initial_keys, &sorted_keys, streams, &expected);
            drop(initial_keys);
            f.push("\"read_misses\":0".to_string());
            f.push("\"write_misses\":0".to_string());
            f.push(format!("\"successful_inserts\":{}", expected.insert_ops));
            f.push(format!("\"value_sum\":{}", out.oracle.value_sum));
            f.push(format!(
                "\"per_key_mismatches\":{}",
                out.oracle.per_key_mismatches
            ));
            f.push(format!("\"lost_updates\":{}", out.oracle.lost_updates));
            f.push(format!(
                "\"missing_population_keys\":{}",
                out.oracle.missing_population_keys
            ));
            f.push(format!("\"final_count\":{}", out.oracle.final_count));
            f.push(format!(
                "\"expected_final_count\":{}",
                out.oracle.expected_final_count
            ));
            f.push(format!(
                "\"oracle\":\"{}\"",
                out.verdict.replace('\\', "\\\\").replace('"', "\\\"")
            ));
            if let Some(m) = out.mem_used {
                f.push(format!("\"mem_used\":{m}"));
            }
            f.push(format!(
                "\"monotonicity_violations\":{}",
                out.monotonicity_violations
            ));
            f.push(format!("\"tracked_keys\":{}", out.tracked_keys));
            if let Some(ref census) = out.node_census {
                f.push(format!("\"node_census\":{}", census_json(census)));
            }
        }
    }

    println!("{{{}}}", f.join(","));
}
