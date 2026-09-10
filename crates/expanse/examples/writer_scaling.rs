//! Expanse-native multi-writer OLC scaling instrument (Refs #568, Phase 1.5D).
//!
//! Measures writer throughput of [`SyncExpanseMap`], [`SyncExpanseSet`], and
//! [`SyncExpanseStrMap`] as writer count scales across physical P-cores (W in {1, 2, 4, 8}, R = 0).
//!
//! Decoupled from `crates/expanse-hot-bench`: links no third-party competitor
//! trees, requiring zero C++ submodules. Uses the shared XorShift64 seeds and
//! population parameters (2^20 prefill, 2^20 fresh inserts) to ensure 1:1
//! comparability with historical baselines.
//!
//! ## Two builds, never one (AGENTS.md §6)
//!
//! Throughput comes from the default build (`--role throughput`, refuses to run if
//! `occ-stats` is enabled). Fallback counters come from the diagnostic build
//! (`--role counters`, requires `--features occ-stats` and refuses to run without it).
//! The roles cannot share a binary because counter instrumentation perturbs timing.
//!
//! Run (throughput — default build, no occ-stats):
//! ```text
//! cargo run --release -p expanse-trie --example writer_scaling -- [--role throughput] [--arm <map|set|str|all>] [--writers <1,2,4,8>] [--rounds <N>]
//! ```
//!
//! Run (counters — occ-stats build only):
//! ```text
//! cargo run --release -p expanse-trie --features occ-stats --example writer_scaling -- --role counters [--arm <map|set|str|all>] [--writers <1,2,4,8>] [--rounds <N>]
//! ```
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `concurrency_writer_scaling` |
//! | `group` | 5 |
//! | `emits` | `concurrency_writer_map_64bit`, `concurrency_writer_set_63bit`, `concurrency_writer_str` |
//! | `population` | prefill 2^20 keys (1M), plus 2^20 fresh keys inserted concurrently by W writers |
//! | `insertion_order` | sorted — prefill ascending, matching expanse-hot-bench; fresh stream in generator draw order |
//! | `probes_and_reuse` | none — pure writer scaling (R = 0), insert-only |
//! | `hit_rate` | n/a — no read probes |
//! | `miss_gen_method` | same-generator rejection sampling against prefill |
//! | `value_dereference` | map arms check stored values against key-derived expectation |
//! | `measured_region` | barrier release to last-writer join; prefill and teardown outside |
//! | `arm_symmetry` | symmetric across thread counts; W in {1, 2, 4, 8} on physical P-cores |
//! | `statistics` | throughput ops/sec emitted raw, paired bootstrap BCa 95% CI for C(N); lock fallbacks from occ-stats counters pass |
//! | `verdict` | pending measurement |

use std::collections::HashSet;
use std::sync::Barrier;
use std::time::Instant;

use expanse_trie::occ_stats::{self, Stat};
use expanse_trie::strmap::NulFreeStr;
use expanse_trie::sync::{SyncExpanseMap, SyncExpanseSet, SyncExpanseStrMap};

/// Prefill population (2^20 keys = 1,048,576).
pub const N_PREFILL: usize = 1 << 20;
/// Fresh keys the writers insert (2^20 keys = 1,048,576).
pub const M_FRESH: usize = 1 << 20;

/// The shared suite seed (`expanse_hot_bench::workload::XorShift::SEED`).
pub const SEED_PREFILL: u64 = 0x0DDB_1A5E_5EED_0001;
/// The continuation seed for fresh integer keys, derived identically to `hot_concurrent`.
pub const SEED_FRESH: u64 = SEED_PREFILL ^ 0x5EED_C0DE_0000_0692;
/// The continuation seed for fresh string keys, derived identically to `masstree_concurrent`.
pub const SEED_STR_FRESH: u64 = SEED_PREFILL ^ 0x5EED_C0DE_0000_0661;

/// The 62 ASCII alphanumerics — NUL-free by construction.
const ALNUM: &[u8; 62] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// Deterministic XorShift64 PRNG matching repo standard.
#[derive(Clone)]
struct XorShift(u64);

impl XorShift {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    #[allow(clippy::should_implement_trait)]
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// The stored value for a key on the map arm, matching `hot_concurrent`.
#[inline]
fn value_of(k: u64) -> u64 {
    k.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// Deterministic key-derived value for string map arm.
#[inline]
fn str_value_of(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

fn fill_alnum(rng: &mut XorShift, out: &mut Vec<u8>, n: usize) {
    for _ in 0..n {
        let idx = (rng.next() % 62) as usize;
        out.push(ALNUM[idx]);
    }
}

/// Workload containing prefill keys and fresh disjoint keys for integer writers.
struct WriterWorkload {
    prefill: Vec<u64>,
    fresh_keys: Vec<u64>,
    keyspace_bits: u32,
}

impl WriterWorkload {
    fn generate(n_prefill: usize, m_fresh: usize, keyspace_bits: u32) -> Self {
        let mask = if keyspace_bits >= 64 {
            u64::MAX
        } else {
            (1u64 << keyspace_bits) - 1
        };

        // 1. Generate prefill keys.
        let mut rng = XorShift::new(SEED_PREFILL);
        let mut prefill = Vec::with_capacity(n_prefill);
        for _ in 0..n_prefill {
            prefill.push(rng.next() & mask);
        }
        prefill.sort_unstable();
        prefill.dedup();
        // If duplicates occurred, top up to exactly n_prefill.
        while prefill.len() < n_prefill {
            let k = rng.next() & mask;
            if let Err(pos) = prefill.binary_search(&k) {
                prefill.insert(pos, k);
            }
        }

        // 2. Generate fresh keys rejection-sampled against prefill.
        let mut fresh_rng = XorShift::new(SEED_FRESH);
        let mut fresh_keys = Vec::with_capacity(m_fresh);
        while fresh_keys.len() < m_fresh {
            let c = fresh_rng.next() & mask;
            if prefill.binary_search(&c).is_err() {
                fresh_keys.push(c);
            }
        }

        Self {
            prefill,
            fresh_keys,
            keyspace_bits,
        }
    }
}

/// Workload containing prefill keys and fresh disjoint keys for string writers.
struct WriterStrWorkload {
    prefill: Vec<Vec<u8>>,
    fresh_keys: Vec<Vec<u8>>,
}

impl WriterStrWorkload {
    fn generate(n_prefill: usize, m_fresh: usize) -> Self {
        let mut rng = XorShift::new(SEED_PREFILL);
        let mut prefill = Vec::with_capacity(n_prefill);
        let mut buf = Vec::with_capacity(16);
        for _ in 0..n_prefill {
            buf.clear();
            let n = 8 + (rng.next() % 9) as usize;
            fill_alnum(&mut rng, &mut buf, n);
            prefill.push(buf.clone());
        }
        prefill.sort_unstable();
        prefill.dedup();
        while prefill.len() < n_prefill {
            buf.clear();
            let n = 8 + (rng.next() % 9) as usize;
            fill_alnum(&mut rng, &mut buf, n);
            if let Err(pos) = prefill.binary_search(&buf) {
                prefill.insert(pos, buf.clone());
            }
        }

        let mut fresh_rng = XorShift::new(SEED_STR_FRESH);
        let mut fresh_keys = Vec::with_capacity(m_fresh);
        let mut seen = HashSet::with_capacity(m_fresh);
        while fresh_keys.len() < m_fresh {
            buf.clear();
            let n = 8 + (fresh_rng.next() % 9) as usize;
            fill_alnum(&mut fresh_rng, &mut buf, n);
            if prefill.binary_search(&buf).is_err() && seen.insert(buf.clone()) {
                fresh_keys.push(buf.clone());
            }
        }

        Self {
            prefill,
            fresh_keys,
        }
    }
}

fn run_map_cell(
    workload: &WriterWorkload,
    writers: usize,
    round: usize,
    is_counters: bool,
) -> (f64, u64, u64) {
    let map = SyncExpanseMap::new();
    for &k in &workload.prefill {
        map.insert(k, value_of(k));
    }

    let per = workload.fresh_keys.len() / writers.max(1);
    let barrier = Barrier::new(writers + 1);

    if is_counters {
        occ_stats::reset();
    }

    let start = std::thread::scope(|s| {
        for w in 0..writers {
            let lo = w * per;
            let hi = if w + 1 == writers {
                workload.fresh_keys.len()
            } else {
                lo + per
            };
            let slice = &workload.fresh_keys[lo..hi];
            let b = &barrier;
            let m = &map;
            s.spawn(move || {
                b.wait();
                for &k in slice {
                    m.insert(k, value_of(k));
                }
            });
        }

        barrier.wait();
        Instant::now()
    });

    let elapsed = if is_counters {
        0.0
    } else {
        start.elapsed().as_secs_f64()
    };
    let lock_fallbacks = if is_counters {
        occ_stats::snapshot()[Stat::LockFallbacks as usize]
    } else {
        0
    };
    let final_pop = map.len();

    // Verify samples
    let reader = map.reader();
    for &k in workload.fresh_keys.iter().step_by(10_000) {
        let val = reader.get(k);
        assert_eq!(
            val,
            Some(value_of(k)),
            "map missing fresh key {k} at round {round}"
        );
    }

    (elapsed, final_pop, lock_fallbacks)
}

fn run_set_cell(
    workload: &WriterWorkload,
    writers: usize,
    round: usize,
    is_counters: bool,
) -> (f64, u64, u64) {
    let set = SyncExpanseSet::new();
    for &k in &workload.prefill {
        set.insert(k);
    }

    let per = workload.fresh_keys.len() / writers.max(1);
    let barrier = Barrier::new(writers + 1);

    if is_counters {
        occ_stats::reset();
    }

    let start = std::thread::scope(|s| {
        for w in 0..writers {
            let lo = w * per;
            let hi = if w + 1 == writers {
                workload.fresh_keys.len()
            } else {
                lo + per
            };
            let slice = &workload.fresh_keys[lo..hi];
            let b = &barrier;
            let s_ref = &set;
            s.spawn(move || {
                b.wait();
                for &k in slice {
                    s_ref.insert(k);
                }
            });
        }

        barrier.wait();
        Instant::now()
    });

    let elapsed = if is_counters {
        0.0
    } else {
        start.elapsed().as_secs_f64()
    };
    let lock_fallbacks = if is_counters {
        occ_stats::snapshot()[Stat::LockFallbacks as usize]
    } else {
        0
    };
    let final_pop = set.len();

    // Verify samples
    let reader = set.reader();
    for &k in workload.fresh_keys.iter().step_by(10_000) {
        assert!(
            reader.contains(k),
            "set missing fresh key {k} at round {round}"
        );
    }

    (elapsed, final_pop, lock_fallbacks)
}

fn run_str_cell(
    workload: &WriterStrWorkload,
    writers: usize,
    round: usize,
    is_counters: bool,
) -> (f64, u64, u64) {
    let map = SyncExpanseStrMap::new();
    for k in &workload.prefill {
        let nk = NulFreeStr::new(k).expect("alnum bytes are NUL-free");
        map.insert(nk, str_value_of(k));
    }

    let per = workload.fresh_keys.len() / writers.max(1);
    let barrier = Barrier::new(writers + 1);

    if is_counters {
        occ_stats::reset();
    }

    let start = std::thread::scope(|s| {
        for w in 0..writers {
            let lo = w * per;
            let hi = if w + 1 == writers {
                workload.fresh_keys.len()
            } else {
                lo + per
            };
            let slice = &workload.fresh_keys[lo..hi];
            let b = &barrier;
            let m = &map;
            s.spawn(move || {
                b.wait();
                for k in slice {
                    let nk = NulFreeStr::new(k).expect("alnum bytes are NUL-free");
                    m.insert(nk, str_value_of(k));
                }
            });
        }

        barrier.wait();
        Instant::now()
    });

    let elapsed = if is_counters {
        0.0
    } else {
        start.elapsed().as_secs_f64()
    };
    let lock_fallbacks = if is_counters {
        occ_stats::snapshot()[Stat::LockFallbacks as usize]
    } else {
        0
    };
    let final_pop = map.len();

    // Verify samples
    let reader = map.reader();
    for k in workload.fresh_keys.iter().step_by(10_000) {
        let nk = NulFreeStr::new(k).expect("alnum bytes are NUL-free");
        let val = reader.get(nk);
        assert_eq!(
            val,
            Some(str_value_of(k)),
            "str missing fresh key at round {round}"
        );
    }

    (elapsed, final_pop, lock_fallbacks)
}

fn self_test(role_opt: Option<&str>) -> Result<(), String> {
    eprintln!("running writer_scaling self-test...");
    let n0 = 1024;
    let m = 1024;

    let is_counters = match role_opt {
        Some("counters") => {
            if !occ_stats::enabled() {
                return Err(
                    "build/role mismatch: occ-stats is OFF but role is 'counters' (AGENTS.md §6 / two builds, never one)".into()
                );
            }
            true
        }
        Some("throughput") => {
            if occ_stats::enabled() {
                return Err(
                    "build/role mismatch: occ-stats is ON but role is 'throughput' (AGENTS.md §6 / two builds, never one)".into()
                );
            }
            false
        }
        _ => occ_stats::enabled(),
    };

    let wl_map = WriterWorkload::generate(n0, m, 64);
    assert_eq!(wl_map.prefill.len(), n0);
    assert_eq!(wl_map.fresh_keys.len(), m);

    let (el_map, pop_map, fb_map) = run_map_cell(&wl_map, 2, 0, is_counters);
    if pop_map != (n0 + m) as u64 {
        return Err(format!("map expected pop {}, got {pop_map}", n0 + m));
    }
    if is_counters {
        if fb_map == 0 {
            return Err(format!(
                "counters test: expected fb_map > 0 at quick scale, got {fb_map}"
            ));
        }
    } else if el_map <= 0.0 {
        return Err(format!("throughput test: invalid map elapsed {el_map}"));
    }

    let wl_set = WriterWorkload::generate(n0, m, 63);
    let (el_set, pop_set, fb_set) = run_set_cell(&wl_set, 2, 0, is_counters);
    if pop_set != (n0 + m) as u64 {
        return Err(format!("set expected pop {}, got {pop_set}", n0 + m));
    }
    if is_counters {
        if fb_set == 0 {
            return Err(format!(
                "counters test: expected fb_set > 0 at quick scale, got {fb_set}"
            ));
        }
    } else if el_set <= 0.0 {
        return Err(format!("throughput test: invalid set elapsed {el_set}"));
    }

    let wl_str = WriterStrWorkload::generate(n0, m);
    let (el_str, pop_str, fb_str) = run_str_cell(&wl_str, 2, 0, is_counters);
    if pop_str != (n0 + m) as u64 {
        return Err(format!("str expected pop {}, got {pop_str}", n0 + m));
    }
    if is_counters {
        // str arm is the alpha=1 coarse-mutex reference curve: 0 lock fallbacks by construction
        if fb_str != 0 {
            return Err(format!("counters test: expected fb_str == 0, got {fb_str}"));
        }
    } else if el_str <= 0.0 {
        return Err(format!("throughput test: invalid str elapsed {el_str}"));
    }

    let mode_str = if is_counters {
        "counters"
    } else {
        "throughput"
    };
    eprintln!("writer_scaling {mode_str} self-test PASSED");
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let role_opt = args
        .iter()
        .position(|a| a == "--role")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str());

    if args.iter().any(|a| a == "--self-test") {
        if let Err(e) = self_test(role_opt) {
            eprintln!("self-test failed: {e}");
            std::process::exit(1);
        }
        return;
    }

    let role_arg = role_opt.unwrap_or("throughput");
    let is_counters = match role_arg {
        "counters" => {
            if !occ_stats::enabled() {
                eprintln!(
                    "build/role mismatch: occ-stats is OFF but role is 'counters' — \
                     counter extraction requires --features occ-stats (AGENTS.md §6 / two builds, never one)"
                );
                std::process::exit(1);
            }
            true
        }
        "throughput" => {
            if occ_stats::enabled() {
                eprintln!(
                    "build/role mismatch: occ-stats is ON but role is 'throughput' — \
                     throughput must come from the default build only (AGENTS.md §6 / two builds, never one)"
                );
                std::process::exit(1);
            }
            false
        }
        other => {
            eprintln!("unknown role: {other} (expected 'throughput' or 'counters')");
            std::process::exit(1);
        }
    };

    let arm_arg = args
        .iter()
        .position(|a| a == "--arm")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("all");

    let writers_arg = args
        .iter()
        .position(|a| a == "--writers")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("1,2,4,8");

    let rounds: usize = args
        .iter()
        .position(|a| a == "--rounds")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(7);

    let round_opt: Option<usize> = args
        .iter()
        .position(|a| a == "--round")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok());

    let (round_start, round_end) = if let Some(r) = round_opt {
        (r, r + 1)
    } else {
        (0, rounds)
    };

    let is_quick = args.iter().any(|a| a == "--quick");
    let (n0, m) = if is_quick {
        (4096, 4096)
    } else {
        (N_PREFILL, M_FRESH)
    };

    let writers_list: Vec<usize> = writers_arg
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let run_map = arm_arg == "map" || arm_arg == "all" || arm_arg == "both";
    let run_set = arm_arg == "set" || arm_arg == "all" || arm_arg == "both";
    let run_str = arm_arg == "str" || arm_arg == "all";

    // Interleaved execution across writer counts within each round:
    if run_map {
        eprintln!("generating map 64-bit workload (prefill={n0}, fresh={m})...");
        let wl = WriterWorkload::generate(n0, m, 64);
        let bits = wl.keyspace_bits;
        for round in round_start..round_end {
            for &w in &writers_list {
                let (elapsed_s, final_pop, fallbacks) = run_map_cell(&wl, w, round, is_counters);
                let write_ops = m;
                if is_counters {
                    println!(
                        "{{\"workload_id\":\"concurrency_writer_map_64bit\",\"role\":\"counters\",\
                         \"arm\":\"expanse\",\"cell\":\"map_w{w}_r0\",\"keyspace_bits\":{bits},\
                         \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{w},\"readers\":0,\
                         \"round\":{round},\"write_ops\":{write_ops},\
                         \"lock_fallbacks\":{fallbacks},\"population_after\":{final_pop}}}"
                    );
                } else {
                    let writer_mops = (write_ops as f64) / elapsed_s / 1e6;
                    println!(
                        "{{\"workload_id\":\"concurrency_writer_map_64bit\",\"role\":\"throughput\",\
                         \"arm\":\"expanse\",\"cell\":\"map_w{w}_r0\",\"keyspace_bits\":{bits},\
                         \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{w},\"readers\":0,\
                         \"round\":{round},\"write_ops\":{write_ops},\"writer_elapsed_s\":{elapsed_s:.6},\
                         \"writer_mops\":{writer_mops:.4},\"population_after\":{final_pop}}}"
                    );
                }
            }
        }
    }

    if run_set {
        eprintln!("generating set 63-bit workload (prefill={n0}, fresh={m})...");
        let wl = WriterWorkload::generate(n0, m, 63);
        let bits = wl.keyspace_bits;
        for round in round_start..round_end {
            for &w in &writers_list {
                let (elapsed_s, final_pop, fallbacks) = run_set_cell(&wl, w, round, is_counters);
                let write_ops = m;
                if is_counters {
                    println!(
                        "{{\"workload_id\":\"concurrency_writer_set_63bit\",\"role\":\"counters\",\
                         \"arm\":\"expanse\",\"cell\":\"set_w{w}_r0\",\"keyspace_bits\":{bits},\
                         \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{w},\"readers\":0,\
                         \"round\":{round},\"write_ops\":{write_ops},\
                         \"lock_fallbacks\":{fallbacks},\"population_after\":{final_pop}}}"
                    );
                } else {
                    let writer_mops = (write_ops as f64) / elapsed_s / 1e6;
                    println!(
                        "{{\"workload_id\":\"concurrency_writer_set_63bit\",\"role\":\"throughput\",\
                         \"arm\":\"expanse\",\"cell\":\"set_w{w}_r0\",\"keyspace_bits\":{bits},\
                         \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{w},\"readers\":0,\
                         \"round\":{round},\"write_ops\":{write_ops},\"writer_elapsed_s\":{elapsed_s:.6},\
                         \"writer_mops\":{writer_mops:.4},\"population_after\":{final_pop}}}"
                    );
                }
            }
        }
    }

    if run_str {
        eprintln!("generating str workload (prefill={n0}, fresh={m})...");
        let wl = WriterStrWorkload::generate(n0, m);
        for round in round_start..round_end {
            for &w in &writers_list {
                let (elapsed_s, final_pop, fallbacks) = run_str_cell(&wl, w, round, is_counters);
                let write_ops = m;
                if is_counters {
                    println!(
                        "{{\"workload_id\":\"concurrency_writer_str\",\"role\":\"counters\",\
                         \"arm\":\"expanse\",\"cell\":\"str_w{w}_r0\",\"dist\":\"short\",\
                         \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{w},\"readers\":0,\
                         \"round\":{round},\"write_ops\":{write_ops},\
                         \"lock_fallbacks\":{fallbacks},\"population_after\":{final_pop}}}"
                    );
                } else {
                    let writer_mops = (write_ops as f64) / elapsed_s / 1e6;
                    println!(
                        "{{\"workload_id\":\"concurrency_writer_str\",\"role\":\"throughput\",\
                         \"arm\":\"expanse\",\"cell\":\"str_w{w}_r0\",\"dist\":\"short\",\
                         \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{w},\"readers\":0,\
                         \"round\":{round},\"write_ops\":{write_ops},\"writer_elapsed_s\":{elapsed_s:.6},\
                         \"writer_mops\":{writer_mops:.4},\"population_after\":{final_pop}}}"
                    );
                }
            }
        }
    }
}
