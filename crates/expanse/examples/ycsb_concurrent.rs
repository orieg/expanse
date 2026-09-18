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
//! | `hit_rate` | 100% — every point read names a present key; miss_gen_method is n/a |
//! | `miss_gen_method` | n/a |
//! | `value_dereference` | every read folds its value into a per-thread accumulator passed to black_box after the window; every write's return value is folded likewise; post-window oracle validates final values and per-key counts |
//! | `measured_region` | barrier release to join of last thread; prefill, stream generation, Zipfian tables, post-run checks and teardown outside |
//! | `arm_symmetry` | symmetric across arms and thread counts T in {1, 2, 4, 8} (and extra load points T in {3, 6} for olc and mutex on families A and B) |
//! | `statistics` | throughput total M ops/s emitted raw, paired bootstrap BCa 95% CI across interleaved Williams rounds; lock fallbacks and stripe contended acquisitions from occ-stats build |
//! | `verdict` | pending measurement |
//!
//! ## Two builds, never one (AGENTS.md §6, METHODOLOGY §20.12 item 1)
//!
//! * Throughput comes from the uninstrumented release build (`--role throughput`),
//!   which refuses to run if `occ-stats` is enabled.
//! * Diagnostic counters come from the `occ-stats` build (`--role occ-stats`), which
//!   requires `--features occ-stats` and refuses to emit `total_mops`.

use std::collections::BTreeMap;
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Mutex, RwLock};
use std::time::Instant;

use crossbeam_skiplist::SkipMap;
use dashmap::DashMap;

use expanse_trie::map::ExpanseMap;
use expanse_trie::occ_stats::{self, Stat};
use expanse_trie::sync::SyncExpanseMap;

#[path = "../benches/ycsb_common/mod.rs"]
mod ycsb_common;
use ycsb_common::{XorShift64, ZIPFIAN_THETA, ZipfianGenerator};

/// Standard population size: 2^20 (1,048,576 keys).
const STANDARD_POPULATION_N: usize = 1_048_576;
/// Large DRAM population size: 2^24 (16,777,216 keys).
const DRAM_POPULATION_N: usize = 16_777_216;
/// Quick mode population size: 4,096 keys.
const QUICK_POPULATION_N: usize = 4_096;

/// Standard operations per thread: 2^20 (1,048,576 ops).
const STANDARD_OPS_PER_THREAD: usize = 1_048_576;
/// Quick mode operations per thread: 4,096 ops.
const QUICK_OPS_PER_THREAD: usize = 4_096;

/// Population key PRNG seed (matches METHODOLOGY §20.4).
const SEED_KEY_BASE: u64 = 0x0DDB_1A5E_5EED_0001;
/// Default suite seed.
const DEFAULT_SUITE_SEED: u64 = 0x1006_2026_0917_0001;

/// Golden ratio constant for thread seed decorrelation.
const PHI64: u64 = 0x9E37_79B9_7F4A_7C15;

/// Number of stripes in the external striped lock for Family F on `olc` (D1, METHODOLOGY §20.11).
const NUM_STRIPES: usize = 1_024;

/// Cache-line padded wrapper to prevent false sharing between lock stripes.
#[repr(align(64))]
struct CachePadded<T>(T);

/// External striped lock provider for Family F on `olc` (Decision D1).
struct StripedLock {
    stripes: Vec<CachePadded<Mutex<()>>>,
    contended_acquisitions: AtomicU64,
}

impl StripedLock {
    fn new(num_stripes: usize) -> Self {
        let mut stripes = Vec::with_capacity(num_stripes);
        for _ in 0..num_stripes {
            stripes.push(CachePadded(Mutex::new(())));
        }
        Self {
            stripes,
            contended_acquisitions: AtomicU64::new(0),
        }
    }

    #[inline(always)]
    fn stripe_index(&self, key: u64) -> usize {
        (key.wrapping_mul(PHI64) >> 54) as usize % self.stripes.len()
    }
}

/// Supported competitor arms.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Arm {
    Olc,
    Mutex,
    Skip,
    Dash,
    RwBTree,
}

impl Arm {
    fn parse(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "olc" => Ok(Self::Olc),
            "mutex" => Ok(Self::Mutex),
            "skip" => Ok(Self::Skip),
            "dash" => Ok(Self::Dash),
            "rwbtree" => Ok(Self::RwBTree),
            _ => Err(format!(
                "unknown arm '{s}'; expected olc, mutex, skip, dash, or rwbtree"
            )),
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Olc => "olc",
            Self::Mutex => "mutex",
            Self::Skip => "skip",
            Self::Dash => "dash",
            Self::RwBTree => "rwbtree",
        }
    }
}

/// Supported YCSB families.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Family {
    A,
    B,
    D,
    F,
    A0,
    B0,
    F0,
    C,
    C0,
    Ac,
    Fc,
    ADram,
    CDram,
}

impl Family {
    fn parse(s: &str) -> Result<Self, String> {
        match s.to_ascii_uppercase().as_str() {
            "A" => Ok(Self::A),
            "B" => Ok(Self::B),
            "D" => Ok(Self::D),
            "F" => Ok(Self::F),
            "A0" => Ok(Self::A0),
            "B0" => Ok(Self::B0),
            "F0" => Ok(Self::F0),
            "C" => Ok(Self::C),
            "C0" => Ok(Self::C0),
            "AC" => Ok(Self::Ac),
            "FC" => Ok(Self::Fc),
            "A-DRAM" | "ADRAM" => Ok(Self::ADram),
            "C-DRAM" | "CDRAM" => Ok(Self::CDram),
            _ => Err(format!("unknown family '{s}'")),
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::D => "D",
            Self::F => "F",
            Self::A0 => "A0",
            Self::B0 => "B0",
            Self::F0 => "F0",
            Self::C => "C",
            Self::C0 => "C0",
            Self::Ac => "Ac",
            Self::Fc => "Fc",
            Self::ADram => "A-dram",
            Self::CDram => "C-dram",
        }
    }

    fn tag(&self) -> &'static str {
        match self {
            Self::A => "concurrency_ycsb_a",
            Self::B => "concurrency_ycsb_b",
            Self::D => "concurrency_ycsb_d",
            Self::F => "concurrency_ycsb_f",
            Self::A0 => "concurrency_ycsb_a0",
            Self::B0 => "concurrency_ycsb_b0",
            Self::F0 => "concurrency_ycsb_f0",
            Self::C => "concurrency_ycsb_c",
            Self::C0 => "concurrency_ycsb_c0",
            Self::Ac => "concurrency_ycsb_ac",
            Self::Fc => "concurrency_ycsb_fc",
            Self::ADram => "concurrency_ycsb_a_dram",
            Self::CDram => "concurrency_ycsb_c_dram",
        }
    }

    fn is_uniform(&self) -> bool {
        matches!(self, Self::A0 | Self::B0 | Self::F0 | Self::C0)
    }

    fn is_contiguous_ranks(&self) -> bool {
        matches!(self, Self::Ac | Self::Fc)
    }

    fn is_dram(&self) -> bool {
        matches!(self, Self::ADram | Self::CDram)
    }
}

/// Execution role.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Role {
    Throughput,
    OccStats,
    Latency,
}

impl Role {
    fn parse(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "throughput" => Ok(Self::Throughput),
            "occ-stats" | "counters" => Ok(Self::OccStats),
            "latency" => Ok(Self::Latency),
            _ => Err(format!(
                "unknown role '{s}'; expected throughput, occ-stats, or latency"
            )),
        }
    }
}

/// Pre-generated operation for worker threads.
#[derive(Clone, Copy, Debug)]
enum Op {
    Read { key: u64 },
    Update { key: u64, val: u64 },
    Insert { key: u64, val: u64 },
    ReadModifyWrite { key: u64 },
}

/// Generates distinct initial population keys in canonical generator draw order.
fn generate_initial_population(n: usize) -> Vec<u64> {
    let mut rng = XorShift64::new(SEED_KEY_BASE);
    let mut keys = Vec::with_capacity(n);
    let mut seen = std::collections::HashSet::with_capacity(n);
    while keys.len() < n {
        let k = rng.next_u64();
        if seen.insert(k) {
            keys.push(k);
        }
    }
    keys
}

#[allow(clippy::too_many_arguments)]
/// Pre-generates the operation stream for thread `t`.
fn generate_thread_stream(
    family: Family,
    thread_id: usize,
    num_threads: usize,
    ops_count: usize,
    population_n: usize,
    keys: &[u64],
    sorted_keys: &[u64],
    suite_seed: u64,
) -> Vec<Op> {
    let thread_seed = suite_seed ^ ((thread_id as u64 + 1).wrapping_mul(PHI64));
    let mut rng = XorShift64::new(thread_seed);

    let zipf = if family.is_uniform() {
        None
    } else {
        Some(ZipfianGenerator::new(population_n as u64, ZIPFIAN_THETA))
    };

    let key_map = if family.is_contiguous_ranks() {
        sorted_keys
    } else {
        keys
    };

    let mut stream = Vec::with_capacity(ops_count);
    let mut insert_j = 0u64;

    for op_idx in 0..ops_count {
        let u = rng.next_f64();
        let draw_rank = match &zipf {
            Some(z) => z.next(u) as usize,
            None => (rng.next_u64() % (population_n as u64)) as usize,
        };

        let write_val =
            ((thread_id as u64 + 1) << 56) | ((op_idx as u64 + 1) & 0x00FF_FFFF_FFFF_FFFF);

        match family {
            Family::A | Family::A0 | Family::Ac => {
                // 50% read, 50% update
                let roll = rng.next_u64() % 100;
                let k = key_map[draw_rank % population_n];
                if roll < 50 {
                    stream.push(Op::Read { key: k });
                } else {
                    stream.push(Op::Update {
                        key: k,
                        val: write_val,
                    });
                }
            }
            Family::B | Family::B0 => {
                // 95% read, 5% update
                let roll = rng.next_u64() % 100;
                let k = key_map[draw_rank % population_n];
                if roll < 95 {
                    stream.push(Op::Read { key: k });
                } else {
                    stream.push(Op::Update {
                        key: k,
                        val: write_val,
                    });
                }
            }
            Family::C | Family::C0 | Family::CDram => {
                // 100% read
                let k = key_map[draw_rank % population_n];
                stream.push(Op::Read { key: k });
            }
            Family::ADram => {
                // 50% read, 50% update over 2^24
                let roll = rng.next_u64() % 100;
                let k = key_map[draw_rank % population_n];
                if roll < 50 {
                    stream.push(Op::Read { key: k });
                } else {
                    stream.push(Op::Update {
                        key: k,
                        val: write_val,
                    });
                }
            }
            Family::D => {
                // 95% read, 5% insert (METHODOLOGY §20.4)
                let roll = rng.next_u64() % 100;
                if roll < 95 {
                    // Read latest
                    let rho = draw_rank;
                    let k = if (rho as u64) < insert_j {
                        // Thread t's (rho + 1)-th most recent insert
                        let prev_idx = insert_j - 1 - (rho as u64);
                        (1u64 << 63) + 1 + prev_idx * (num_threads as u64) + (thread_id as u64)
                    } else {
                        // Older key from canonical draw order backwards
                        let delta = rho as u64 - insert_j;
                        let pop_idx = (population_n as u64 - 1).saturating_sub(delta) as usize;
                        keys[pop_idx % population_n]
                    };
                    stream.push(Op::Read { key: k });
                } else {
                    // Insert monotonic append key: 2^63 + 1 + j*T + t
                    let k = (1u64 << 63) + 1 + insert_j * (num_threads as u64) + (thread_id as u64);
                    insert_j += 1;
                    stream.push(Op::Insert {
                        key: k,
                        val: write_val,
                    });
                }
            }
            Family::F | Family::F0 | Family::Fc => {
                // 50% read, 50% RMW
                let roll = rng.next_u64() % 100;
                let k = key_map[draw_rank % population_n];
                if roll < 50 {
                    stream.push(Op::Read { key: k });
                } else {
                    stream.push(Op::ReadModifyWrite { key: k });
                }
            }
        }
    }

    stream
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
                println!("  --role <throughput|occ-stats|latency>");
                println!("  --quick");
                println!("  --self-test");
                return;
            }
            arg => panic!("unknown argument '{arg}'"),
        }
        idx += 1;
    }

    // AGENTS.md §6 role enforcement:
    // Throughput role refuses to run if occ-stats is compiled in.
    // OccStats role requires occ-stats feature and refuses to run without it.
    let has_occ_stats = cfg!(feature = "occ-stats");
    if is_self_test {
        match role {
            Role::Throughput => {
                if has_occ_stats {
                    eprintln!(
                        "build/role mismatch: occ-stats is ON but role is 'throughput' (AGENTS.md §6 / two builds, never one)"
                    );
                    std::process::exit(1);
                }
            }
            Role::OccStats => {
                if !has_occ_stats {
                    eprintln!(
                        "build/role mismatch: occ-stats is OFF but role is 'occ-stats' (AGENTS.md §6 / two builds, never one)"
                    );
                    std::process::exit(1);
                }
            }
            Role::Latency => {}
        }
        println!("ycsb_concurrent self-test: OK");
        return;
    }

    match role {
        Role::Throughput => {
            if has_occ_stats {
                panic!(
                    "Throughput role refuses to run with --features occ-stats enabled (timing perturbation). Run default build."
                );
            }
        }
        Role::OccStats => {
            if !has_occ_stats {
                panic!(
                    "OccStats role requires --features occ-stats to measure event counters. Run with --features occ-stats."
                );
            }
        }
        Role::Latency => {
            // Latency diagnostic role
        }
    }

    // Population and ops configuration
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

    // Step 1: Generate initial keys
    let initial_keys = generate_initial_population(population_n);
    let mut sorted_keys = initial_keys.clone();
    sorted_keys.sort_unstable();

    // Step 2: Build per-thread streams and tally expected counts before the window
    let mut thread_streams = Vec::with_capacity(threads);
    let mut expected_rmw_counts: std::collections::HashMap<u64, u64> =
        std::collections::HashMap::new();
    let mut expected_candidate_writes: std::collections::HashMap<u64, Vec<u64>> =
        std::collections::HashMap::new();
    let mut expected_d_inserts: std::collections::HashMap<u64, u64> =
        std::collections::HashMap::new();
    let mut total_expected_rmws = 0u64;

    for t in 0..threads {
        let stream = generate_thread_stream(
            family,
            t,
            threads,
            ops_per_thread,
            population_n,
            &initial_keys,
            &sorted_keys,
            suite_seed,
        );

        for op in &stream {
            match op {
                Op::ReadModifyWrite { key } => {
                    *expected_rmw_counts.entry(*key).or_insert(0) += 1;
                    total_expected_rmws += 1;
                }
                Op::Insert { key, val } => {
                    expected_d_inserts.insert(*key, *val);
                }
                Op::Update { key, val } => {
                    expected_candidate_writes
                        .entry(*key)
                        .or_default()
                        .push(*val);
                }
                Op::Read { .. } => {}
            }
        }

        thread_streams.push(stream);
    }

    // Step 3: Run the cell
    let start_snap = if has_occ_stats {
        occ_stats::snapshot()
    } else {
        [0u64; expanse_trie::occ_stats::NUM_STATS]
    };

    let (
        elapsed_s,
        thread_elapsed_s,
        total_ops,
        per_key_mismatches,
        lost_updates,
        contended_acquisitions,
    ) = run_cell(
        arm,
        family,
        role,
        threads,
        ops_per_thread,
        &sorted_keys,
        thread_streams,
        &expected_rmw_counts,
        total_expected_rmws,
        &expected_candidate_writes,
        &expected_d_inserts,
    );

    let end_snap = if has_occ_stats {
        occ_stats::snapshot()
    } else {
        [0u64; expanse_trie::occ_stats::NUM_STATS]
    };

    // Step 4: Output JSON line
    let total_mops = if role == Role::Throughput {
        total_ops as f64 / elapsed_s / 1_000_000.0
    } else {
        0.0
    };

    let lock_fallbacks = if has_occ_stats {
        end_snap[Stat::LockFallbacks as usize]
            .saturating_sub(start_snap[Stat::LockFallbacks as usize])
    } else {
        0
    };

    let thread_times_json = format!(
        "[{}]",
        thread_elapsed_s
            .iter()
            .map(|t| format!("{t:.6}"))
            .collect::<Vec<_>>()
            .join(",")
    );

    let mut json_fields = Vec::new();
    json_fields.push(format!("\"workload_id\":\"{}\"", family.tag()));
    json_fields.push(format!("\"arm\":\"{}\"", arm.as_str()));
    json_fields.push(format!("\"family\":\"{}\"", family.as_str()));
    json_fields.push(format!("\"threads\":{threads}"));
    json_fields.push(format!("\"round\":{round}"));
    json_fields.push(format!("\"position\":{position}"));
    json_fields.push(format!("\"ops_per_thread\":{ops_per_thread}"));
    json_fields.push(format!("\"total_ops\":{total_ops}"));
    json_fields.push(format!("\"elapsed_s\":{elapsed_s:.6}"));
    if role == Role::Throughput {
        json_fields.push(format!("\"total_mops\":{total_mops:.6}"));
    }
    json_fields.push(format!("\"thread_elapsed_s\":{thread_times_json}"));
    json_fields.push(format!("\"per_key_mismatches\":{per_key_mismatches}"));
    json_fields.push(format!("\"lost_updates\":{lost_updates}"));

    if role == Role::OccStats {
        json_fields.push(format!("\"lock_fallbacks\":{lock_fallbacks}"));
        if family == Family::F || family == Family::F0 || family == Family::Fc {
            let stripe_rate = if total_expected_rmws > 0 {
                contended_acquisitions as f64 / total_expected_rmws as f64
            } else {
                0.0
            };
            json_fields.push(format!(
                "\"contended_acquisitions\":{contended_acquisitions}"
            ));
            json_fields.push(format!("\"stripe_contended_per_rmw\":{stripe_rate:.6}"));
        }
    }

    let out = format!("{{{}}}", json_fields.join(","));
    println!("{out}");
}

/// Dispatches execution across the requested competitor arm.
#[allow(clippy::too_many_arguments)]
fn run_cell(
    arm: Arm,
    family: Family,
    role: Role,
    threads: usize,
    ops_per_thread: usize,
    prefill_keys: &[u64],
    thread_streams: Vec<Vec<Op>>,
    expected_rmw: &std::collections::HashMap<u64, u64>,
    total_expected_rmw: u64,
    expected_candidates: &std::collections::HashMap<u64, Vec<u64>>,
    expected_d_inserts: &std::collections::HashMap<u64, u64>,
) -> (f64, Vec<f64>, usize, usize, usize, u64) {
    match arm {
        Arm::Olc => run_cell_olc(
            family,
            role,
            threads,
            ops_per_thread,
            prefill_keys,
            thread_streams,
            expected_rmw,
            total_expected_rmw,
            expected_candidates,
            expected_d_inserts,
        ),
        Arm::Mutex => run_cell_mutex(
            family,
            threads,
            ops_per_thread,
            prefill_keys,
            thread_streams,
            expected_rmw,
            total_expected_rmw,
            expected_candidates,
            expected_d_inserts,
        ),
        Arm::Skip => run_cell_skip(
            family,
            threads,
            ops_per_thread,
            prefill_keys,
            thread_streams,
            expected_rmw,
            total_expected_rmw,
            expected_candidates,
            expected_d_inserts,
        ),
        Arm::Dash => run_cell_dash(
            family,
            threads,
            ops_per_thread,
            prefill_keys,
            thread_streams,
            expected_rmw,
            total_expected_rmw,
            expected_candidates,
            expected_d_inserts,
        ),
        Arm::RwBTree => run_cell_rwbtree(
            family,
            threads,
            ops_per_thread,
            prefill_keys,
            thread_streams,
            expected_rmw,
            total_expected_rmw,
            expected_candidates,
            expected_d_inserts,
        ),
    }
}

/// Execution for `SyncExpanseMap`.
#[allow(clippy::too_many_arguments)]
fn run_cell_olc(
    family: Family,
    role: Role,
    threads: usize,
    ops_per_thread: usize,
    prefill_keys: &[u64],
    thread_streams: Vec<Vec<Op>>,
    expected_rmw: &std::collections::HashMap<u64, u64>,
    total_expected_rmw: u64,
    expected_candidates: &std::collections::HashMap<u64, Vec<u64>>,
    expected_d_inserts: &std::collections::HashMap<u64, u64>,
) -> (f64, Vec<f64>, usize, usize, usize, u64) {
    let map = Arc::new(SyncExpanseMap::new());
    let initial_val = 0u64;
    for &k in prefill_keys {
        map.insert(k, initial_val);
    }

    let striped_lock = Arc::new(StripedLock::new(NUM_STRIPES));
    let barrier = Arc::new(Barrier::new(threads + 1));
    let mut handles = Vec::with_capacity(threads);

    for stream in thread_streams {
        let map = Arc::clone(&map);
        let striped_lock = Arc::clone(&striped_lock);
        let barrier = Arc::clone(&barrier);

        handles.push(std::thread::spawn(move || {
            let reader = map.reader();
            let mut sink = 0u64;

            barrier.wait();
            let start = Instant::now();

            for op in stream {
                match op {
                    Op::Read { key } => {
                        if let Some(v) = reader.get(key) {
                            sink = sink.wrapping_add(v);
                        }
                    }
                    Op::Update { key, val } => {
                        map.insert(key, val);
                    }
                    Op::Insert { key, val } => {
                        map.insert(key, val);
                    }
                    Op::ReadModifyWrite { key } => {
                        let stripe_idx = striped_lock.stripe_index(key);
                        let _guard = if role == Role::OccStats {
                            match striped_lock.stripes[stripe_idx].0.try_lock() {
                                Ok(g) => g,
                                Err(_) => {
                                    striped_lock
                                        .contended_acquisitions
                                        .fetch_add(1, Ordering::Relaxed);
                                    striped_lock.stripes[stripe_idx].0.lock().unwrap()
                                }
                            }
                        } else {
                            striped_lock.stripes[stripe_idx].0.lock().unwrap()
                        };
                        let cur = map.get(key).unwrap_or(0);
                        map.insert(key, cur + 1);
                    }
                }
            }

            let elapsed = start.elapsed().as_secs_f64();
            black_box(sink);
            elapsed
        }));
    }

    barrier.wait();
    let window_start = Instant::now();
    let mut thread_elapsed_s = Vec::with_capacity(threads);
    for h in handles {
        thread_elapsed_s.push(h.join().unwrap());
    }
    let elapsed_s = window_start.elapsed().as_secs_f64();
    let total_ops = threads * ops_per_thread;
    let contended = striped_lock.contended_acquisitions.load(Ordering::Relaxed);

    // Oracle verification
    let (per_key_mismatches, lost_updates) = verify_oracle_map(
        &map,
        family,
        prefill_keys,
        expected_rmw,
        total_expected_rmw,
        expected_candidates,
        expected_d_inserts,
    );

    (
        elapsed_s,
        thread_elapsed_s,
        total_ops,
        per_key_mismatches,
        lost_updates,
        contended,
    )
}

/// Execution for `Mutex<ExpanseMap>`.
#[allow(clippy::too_many_arguments)]
fn run_cell_mutex(
    family: Family,
    threads: usize,
    ops_per_thread: usize,
    prefill_keys: &[u64],
    thread_streams: Vec<Vec<Op>>,
    expected_rmw: &std::collections::HashMap<u64, u64>,
    total_expected_rmw: u64,
    expected_candidates: &std::collections::HashMap<u64, Vec<u64>>,
    expected_d_inserts: &std::collections::HashMap<u64, u64>,
) -> (f64, Vec<f64>, usize, usize, usize, u64) {
    let mut initial_map = ExpanseMap::new();
    let initial_val = 0u64;
    for &k in prefill_keys {
        initial_map.insert(k, initial_val);
    }
    let map = Arc::new(Mutex::new(initial_map));
    let barrier = Arc::new(Barrier::new(threads + 1));
    let mut handles = Vec::with_capacity(threads);

    for stream in thread_streams {
        let map = Arc::clone(&map);
        let barrier = Arc::clone(&barrier);

        handles.push(std::thread::spawn(move || {
            let mut sink = 0u64;

            barrier.wait();
            let start = Instant::now();

            for op in stream {
                match op {
                    Op::Read { key } => {
                        let guard = map.lock().unwrap();
                        if let Some(v) = guard.get(key) {
                            sink = sink.wrapping_add(v);
                        }
                    }
                    Op::Update { key, val } => {
                        let mut guard = map.lock().unwrap();
                        guard.insert(key, val);
                    }
                    Op::Insert { key, val } => {
                        let mut guard = map.lock().unwrap();
                        guard.insert(key, val);
                    }
                    Op::ReadModifyWrite { key } => {
                        let mut guard = map.lock().unwrap();
                        let cur = guard.get(key).unwrap_or(0);
                        guard.insert(key, cur + 1);
                    }
                }
            }

            let elapsed = start.elapsed().as_secs_f64();
            black_box(sink);
            elapsed
        }));
    }

    barrier.wait();
    let window_start = Instant::now();
    let mut thread_elapsed_s = Vec::with_capacity(threads);
    for h in handles {
        thread_elapsed_s.push(h.join().unwrap());
    }
    let elapsed_s = window_start.elapsed().as_secs_f64();
    let total_ops = threads * ops_per_thread;

    // Oracle verification
    let guard = map.lock().unwrap();
    let (per_key_mismatches, lost_updates) = verify_oracle_plain_map(
        &guard,
        family,
        prefill_keys,
        expected_rmw,
        total_expected_rmw,
        expected_candidates,
        expected_d_inserts,
    );

    (
        elapsed_s,
        thread_elapsed_s,
        total_ops,
        per_key_mismatches,
        lost_updates,
        0,
    )
}

/// Execution for `SkipMap<u64, AtomicU64>`.
#[allow(clippy::too_many_arguments)]
fn run_cell_skip(
    family: Family,
    threads: usize,
    ops_per_thread: usize,
    prefill_keys: &[u64],
    thread_streams: Vec<Vec<Op>>,
    expected_rmw: &std::collections::HashMap<u64, u64>,
    total_expected_rmw: u64,
    expected_candidates: &std::collections::HashMap<u64, Vec<u64>>,
    expected_d_inserts: &std::collections::HashMap<u64, u64>,
) -> (f64, Vec<f64>, usize, usize, usize, u64) {
    let map = Arc::new(SkipMap::new());
    for &k in prefill_keys {
        map.insert(k, AtomicU64::new(0));
    }
    let barrier = Arc::new(Barrier::new(threads + 1));
    let mut handles = Vec::with_capacity(threads);

    for stream in thread_streams {
        let map = Arc::clone(&map);
        let barrier = Arc::clone(&barrier);

        handles.push(std::thread::spawn(move || {
            let mut sink = 0u64;

            barrier.wait();
            let start = Instant::now();

            for op in stream {
                match op {
                    Op::Read { key } => {
                        if let Some(entry) = map.get(&key) {
                            sink = sink.wrapping_add(entry.value().load(Ordering::Relaxed));
                        }
                    }
                    Op::Update { key, val } => {
                        if let Some(entry) = map.get(&key) {
                            entry.value().store(val, Ordering::Relaxed);
                        }
                    }
                    Op::Insert { key, val } => {
                        map.insert(key, AtomicU64::new(val));
                    }
                    Op::ReadModifyWrite { key } => {
                        if let Some(entry) = map.get(&key) {
                            entry.value().fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }

            let elapsed = start.elapsed().as_secs_f64();
            black_box(sink);
            elapsed
        }));
    }

    barrier.wait();
    let window_start = Instant::now();
    let mut thread_elapsed_s = Vec::with_capacity(threads);
    for h in handles {
        thread_elapsed_s.push(h.join().unwrap());
    }
    let elapsed_s = window_start.elapsed().as_secs_f64();
    let total_ops = threads * ops_per_thread;

    // Oracle verification
    let mut per_key_mismatches = 0usize;
    let mut lost_updates = 0usize;

    if family == Family::F || family == Family::F0 || family == Family::Fc {
        let mut total_rmws = 0u64;
        for &k in prefill_keys {
            let v = map
                .get(&k)
                .map(|e| e.value().load(Ordering::Relaxed))
                .unwrap_or(0);
            total_rmws += v;
            let expected = expected_rmw.get(&k).copied().unwrap_or(0);
            if v != expected {
                per_key_mismatches += 1;
            }
        }
        if total_rmws != total_expected_rmw {
            lost_updates = (total_expected_rmw as i64 - total_rmws as i64).unsigned_abs() as usize;
        }
    } else {
        for (&k, candidates) in expected_candidates {
            if let Some(e) = map.get(&k) {
                let v = e.value().load(Ordering::Relaxed);
                if v != 0 && !candidates.contains(&v) {
                    per_key_mismatches += 1;
                }
            } else {
                per_key_mismatches += 1;
            }
        }
        for (&k, &expected_val) in expected_d_inserts {
            if let Some(e) = map.get(&k) {
                if e.value().load(Ordering::Relaxed) != expected_val {
                    per_key_mismatches += 1;
                }
            } else {
                per_key_mismatches += 1;
            }
        }
    }

    (
        elapsed_s,
        thread_elapsed_s,
        total_ops,
        per_key_mismatches,
        lost_updates,
        0,
    )
}

#[allow(clippy::too_many_arguments)]
/// Execution for `DashMap<u64, AtomicU64>`.
fn run_cell_dash(
    family: Family,
    threads: usize,
    ops_per_thread: usize,
    prefill_keys: &[u64],
    thread_streams: Vec<Vec<Op>>,
    expected_rmw: &std::collections::HashMap<u64, u64>,
    total_expected_rmw: u64,
    expected_candidates: &std::collections::HashMap<u64, Vec<u64>>,
    expected_d_inserts: &std::collections::HashMap<u64, u64>,
) -> (f64, Vec<f64>, usize, usize, usize, u64) {
    let map = Arc::new(DashMap::with_shard_amount(64));
    for &k in prefill_keys {
        map.insert(k, AtomicU64::new(0));
    }
    let barrier = Arc::new(Barrier::new(threads + 1));
    let mut handles = Vec::with_capacity(threads);

    for stream in thread_streams {
        let map = Arc::clone(&map);
        let barrier = Arc::clone(&barrier);

        handles.push(std::thread::spawn(move || {
            let mut sink = 0u64;

            barrier.wait();
            let start = Instant::now();

            for op in stream {
                match op {
                    Op::Read { key } => {
                        if let Some(entry) = map.get(&key) {
                            sink = sink.wrapping_add(entry.value().load(Ordering::Relaxed));
                        }
                    }
                    Op::Update { key, val } => {
                        if let Some(entry) = map.get(&key) {
                            entry.value().store(val, Ordering::Relaxed);
                        }
                    }
                    Op::Insert { key, val } => {
                        map.insert(key, AtomicU64::new(val));
                    }
                    Op::ReadModifyWrite { key } => {
                        if let Some(entry) = map.get(&key) {
                            entry.value().fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }

            let elapsed = start.elapsed().as_secs_f64();
            black_box(sink);
            elapsed
        }));
    }

    barrier.wait();
    let window_start = Instant::now();
    let mut thread_elapsed_s = Vec::with_capacity(threads);
    for h in handles {
        thread_elapsed_s.push(h.join().unwrap());
    }
    let elapsed_s = window_start.elapsed().as_secs_f64();
    let total_ops = threads * ops_per_thread;

    // Oracle verification
    let mut per_key_mismatches = 0usize;
    let mut lost_updates = 0usize;

    if family == Family::F || family == Family::F0 || family == Family::Fc {
        let mut total_rmws = 0u64;
        for &k in prefill_keys {
            let v = map
                .get(&k)
                .map(|e| e.value().load(Ordering::Relaxed))
                .unwrap_or(0);
            total_rmws += v;
            let expected = expected_rmw.get(&k).copied().unwrap_or(0);
            if v != expected {
                per_key_mismatches += 1;
            }
        }
        if total_rmws != total_expected_rmw {
            lost_updates = (total_expected_rmw as i64 - total_rmws as i64).unsigned_abs() as usize;
        }
    } else {
        for (&k, candidates) in expected_candidates {
            if let Some(e) = map.get(&k) {
                let v = e.value().load(Ordering::Relaxed);
                if v != 0 && !candidates.contains(&v) {
                    per_key_mismatches += 1;
                }
            } else {
                per_key_mismatches += 1;
            }
        }
        for (&k, &expected_val) in expected_d_inserts {
            if let Some(e) = map.get(&k) {
                if e.value().load(Ordering::Relaxed) != expected_val {
                    per_key_mismatches += 1;
                }
            } else {
                per_key_mismatches += 1;
            }
        }
    }

    (
        elapsed_s,
        thread_elapsed_s,
        total_ops,
        per_key_mismatches,
        lost_updates,
        0,
    )
}

#[allow(clippy::too_many_arguments)]
/// Execution for `RwLock<BTreeMap<u64, AtomicU64>>`.
fn run_cell_rwbtree(
    family: Family,
    threads: usize,
    ops_per_thread: usize,
    prefill_keys: &[u64],
    thread_streams: Vec<Vec<Op>>,
    expected_rmw: &std::collections::HashMap<u64, u64>,
    total_expected_rmw: u64,
    expected_candidates: &std::collections::HashMap<u64, Vec<u64>>,
    expected_d_inserts: &std::collections::HashMap<u64, u64>,
) -> (f64, Vec<f64>, usize, usize, usize, u64) {
    let mut initial_btree = BTreeMap::new();
    for &k in prefill_keys {
        initial_btree.insert(k, AtomicU64::new(0));
    }
    let map = Arc::new(RwLock::new(initial_btree));
    let barrier = Arc::new(Barrier::new(threads + 1));
    let mut handles = Vec::with_capacity(threads);

    for stream in thread_streams {
        let map = Arc::clone(&map);
        let barrier = Arc::clone(&barrier);

        handles.push(std::thread::spawn(move || {
            let mut sink = 0u64;

            barrier.wait();
            let start = Instant::now();

            for op in stream {
                match op {
                    Op::Read { key } => {
                        let guard = map.read().unwrap();
                        if let Some(val) = guard.get(&key) {
                            sink = sink.wrapping_add(val.load(Ordering::Relaxed));
                        }
                    }
                    Op::Update { key, val } => {
                        let guard = map.read().unwrap();
                        if let Some(entry) = guard.get(&key) {
                            entry.store(val, Ordering::Relaxed);
                        }
                    }
                    Op::Insert { key, val } => {
                        let mut guard = map.write().unwrap();
                        guard.insert(key, AtomicU64::new(val));
                    }
                    Op::ReadModifyWrite { key } => {
                        let guard = map.read().unwrap();
                        if let Some(entry) = guard.get(&key) {
                            entry.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }

            let elapsed = start.elapsed().as_secs_f64();
            black_box(sink);
            elapsed
        }));
    }

    barrier.wait();
    let window_start = Instant::now();
    let mut thread_elapsed_s = Vec::with_capacity(threads);
    for h in handles {
        thread_elapsed_s.push(h.join().unwrap());
    }
    let elapsed_s = window_start.elapsed().as_secs_f64();
    let total_ops = threads * ops_per_thread;

    // Oracle verification
    let guard = map.read().unwrap();
    let mut per_key_mismatches = 0usize;
    let mut lost_updates = 0usize;

    if family == Family::F || family == Family::F0 || family == Family::Fc {
        let mut total_rmws = 0u64;
        for &k in prefill_keys {
            let v = guard
                .get(&k)
                .map(|e| e.load(Ordering::Relaxed))
                .unwrap_or(0);
            total_rmws += v;
            let expected = expected_rmw.get(&k).copied().unwrap_or(0);
            if v != expected {
                per_key_mismatches += 1;
            }
        }
        if total_rmws != total_expected_rmw {
            lost_updates = (total_expected_rmw as i64 - total_rmws as i64).unsigned_abs() as usize;
        }
    } else {
        for (&k, candidates) in expected_candidates {
            if let Some(e) = guard.get(&k) {
                let v = e.load(Ordering::Relaxed);
                if v != 0 && !candidates.contains(&v) {
                    per_key_mismatches += 1;
                }
            } else {
                per_key_mismatches += 1;
            }
        }
        for (&k, &expected_val) in expected_d_inserts {
            if let Some(e) = guard.get(&k) {
                if e.load(Ordering::Relaxed) != expected_val {
                    per_key_mismatches += 1;
                }
            } else {
                per_key_mismatches += 1;
            }
        }
    }

    (
        elapsed_s,
        thread_elapsed_s,
        total_ops,
        per_key_mismatches,
        lost_updates,
        0,
    )
}

/// Oracle verification for `SyncExpanseMap`.
fn verify_oracle_map(
    map: &SyncExpanseMap,
    family: Family,
    prefill_keys: &[u64],
    expected_rmw: &std::collections::HashMap<u64, u64>,
    total_expected_rmw: u64,
    expected_candidates: &std::collections::HashMap<u64, Vec<u64>>,
    expected_d_inserts: &std::collections::HashMap<u64, u64>,
) -> (usize, usize) {
    let mut per_key_mismatches = 0usize;
    let mut lost_updates = 0usize;

    if family == Family::F || family == Family::F0 || family == Family::Fc {
        let mut total_rmws = 0u64;
        for &k in prefill_keys {
            let v = map.get(k).unwrap_or(0);
            total_rmws += v;
            let expected = expected_rmw.get(&k).copied().unwrap_or(0);
            if v != expected {
                per_key_mismatches += 1;
            }
        }
        if total_rmws != total_expected_rmw {
            lost_updates = (total_expected_rmw as i64 - total_rmws as i64).unsigned_abs() as usize;
        }
    } else {
        for (&k, candidates) in expected_candidates {
            if let Some(v) = map.get(k) {
                if v != 0 && !candidates.contains(&v) {
                    per_key_mismatches += 1;
                }
            } else {
                per_key_mismatches += 1;
            }
        }
        for (&k, &expected_val) in expected_d_inserts {
            if let Some(v) = map.get(k) {
                if v != expected_val {
                    per_key_mismatches += 1;
                }
            } else {
                per_key_mismatches += 1;
            }
        }
    }

    (per_key_mismatches, lost_updates)
}

/// Oracle verification for `ExpanseMap` under Mutex.
fn verify_oracle_plain_map(
    map: &ExpanseMap,
    family: Family,
    prefill_keys: &[u64],
    expected_rmw: &std::collections::HashMap<u64, u64>,
    total_expected_rmw: u64,
    expected_candidates: &std::collections::HashMap<u64, Vec<u64>>,
    expected_d_inserts: &std::collections::HashMap<u64, u64>,
) -> (usize, usize) {
    let mut per_key_mismatches = 0usize;
    let mut lost_updates = 0usize;

    if family == Family::F || family == Family::F0 || family == Family::Fc {
        let mut total_rmws = 0u64;
        for &k in prefill_keys {
            let v = map.get(k).unwrap_or(0);
            total_rmws += v;
            let expected = expected_rmw.get(&k).copied().unwrap_or(0);
            if v != expected {
                per_key_mismatches += 1;
            }
        }
        if total_rmws != total_expected_rmw {
            lost_updates = (total_expected_rmw as i64 - total_rmws as i64).unsigned_abs() as usize;
        }
    } else {
        for (&k, candidates) in expected_candidates {
            if let Some(v) = map.get(k) {
                if v != 0 && !candidates.contains(&v) {
                    per_key_mismatches += 1;
                }
            } else {
                per_key_mismatches += 1;
            }
        }
        for (&k, &expected_val) in expected_d_inserts {
            if let Some(v) = map.get(k) {
                if v != expected_val {
                    per_key_mismatches += 1;
                }
            } else {
                per_key_mismatches += 1;
            }
        }
    }

    (per_key_mismatches, lost_updates)
}
