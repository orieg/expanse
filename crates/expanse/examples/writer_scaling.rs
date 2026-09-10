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
//! Run:
//! ```text
//! cargo run --release -p expanse-trie --features occ-stats --example writer_scaling -- [--arm <map|set|str|all>] [--writers <1,2,4,8>] [--rounds <N>]
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
//! | `insertion_order` | generator draw order — prefill and fresh-key stream both draw from XorShift64 |
//! | `probes_and_reuse` | none — pure writer scaling (R = 0), insert-only |
//! | `hit_rate` | n/a — no read probes |
//! | `miss_gen_method` | same-generator rejection sampling against prefill |
//! | `value_dereference` | map arms check stored values against key-derived expectation |
//! | `measured_region` | barrier release to last-writer join; prefill and teardown outside |
//! | `arm_symmetry` | symmetric across thread counts; W in {1, 2, 4, 8} on physical P-cores |
//! | `statistics` | per-round throughput ops/sec emitted raw, median and BCa 95% CI across rounds |
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

fn run_map_cell(workload: &WriterWorkload, writers: usize, round: usize) -> (f64, u64, u64) {
    let map = SyncExpanseMap::new();
    for &k in &workload.prefill {
        map.insert(k, value_of(k));
    }

    let per = workload.fresh_keys.len() / writers.max(1);
    let barrier = Barrier::new(writers + 1);

    occ_stats::reset();

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

    let elapsed = start.elapsed().as_secs_f64();
    let lock_fallbacks = occ_stats::snapshot()[Stat::LockFallbacks as usize];
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

fn run_set_cell(workload: &WriterWorkload, writers: usize, round: usize) -> (f64, u64, u64) {
    let set = SyncExpanseSet::new();
    for &k in &workload.prefill {
        set.insert(k);
    }

    let per = workload.fresh_keys.len() / writers.max(1);
    let barrier = Barrier::new(writers + 1);

    occ_stats::reset();

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

    let elapsed = start.elapsed().as_secs_f64();
    let lock_fallbacks = occ_stats::snapshot()[Stat::LockFallbacks as usize];
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

fn run_str_cell(workload: &WriterStrWorkload, writers: usize, round: usize) -> (f64, u64, u64) {
    let map = SyncExpanseStrMap::new();
    for k in &workload.prefill {
        let nk = NulFreeStr::new(k).expect("alnum bytes are NUL-free");
        map.insert(nk, str_value_of(k));
    }

    let per = workload.fresh_keys.len() / writers.max(1);
    let barrier = Barrier::new(writers + 1);

    occ_stats::reset();

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

    let elapsed = start.elapsed().as_secs_f64();
    let lock_fallbacks = occ_stats::snapshot()[Stat::LockFallbacks as usize];
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

fn self_test() -> Result<(), String> {
    eprintln!("running writer_scaling self-test...");
    let n0 = 1024;
    let m = 1024;
    let wl_map = WriterWorkload::generate(n0, m, 64);
    assert_eq!(wl_map.prefill.len(), n0);
    assert_eq!(wl_map.fresh_keys.len(), m);

    let (el_map, pop_map, fb_map) = run_map_cell(&wl_map, 2, 0);
    if pop_map != (n0 + m) as u64 {
        return Err(format!("map expected pop {}, got {pop_map}", n0 + m));
    }
    if el_map <= 0.0 {
        return Err(format!("invalid map elapsed {el_map}"));
    }
    let _ = fb_map;

    let wl_set = WriterWorkload::generate(n0, m, 63);
    let (el_set, pop_set, fb_set) = run_set_cell(&wl_set, 2, 0);
    if pop_set != (n0 + m) as u64 {
        return Err(format!("set expected pop {}, got {pop_set}", n0 + m));
    }
    if el_set <= 0.0 {
        return Err(format!("invalid set elapsed {el_set}"));
    }
    let _ = fb_set;

    let wl_str = WriterStrWorkload::generate(n0, m);
    let (el_str, pop_str, fb_str) = run_str_cell(&wl_str, 2, 0);
    if pop_str != (n0 + m) as u64 {
        return Err(format!("str expected pop {}, got {pop_str}", n0 + m));
    }
    if el_str <= 0.0 {
        return Err(format!("invalid str elapsed {el_str}"));
    }
    // str arm must have 0 lock fallbacks by construction
    if fb_str != 0 {
        return Err(format!("str expected 0 lock fallbacks, got {fb_str}"));
    }

    eprintln!("writer_scaling self-test PASSED");
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--self-test") {
        if let Err(e) = self_test() {
            eprintln!("self-test failed: {e}");
            std::process::exit(1);
        }
        return;
    }

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

    if run_map {
        eprintln!("generating map 64-bit workload (prefill={n0}, fresh={m})...");
        let wl = WriterWorkload::generate(n0, m, 64);
        for &w in &writers_list {
            for round in 0..rounds {
                let (elapsed_s, final_pop, fallbacks) = run_map_cell(&wl, w, round);
                let write_ops = m;
                let writer_mops = (write_ops as f64) / elapsed_s / 1e6;
                let bits = wl.keyspace_bits;
                println!(
                    "{{\"workload_id\":\"concurrency_writer_map_64bit\",\"role\":\"counters\",\
                     \"arm\":\"expanse\",\"cell\":\"map_w{w}_r0\",\"keyspace_bits\":{bits},\
                     \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{w},\"readers\":0,\
                     \"round\":{round},\"write_ops\":{write_ops},\"writer_elapsed_s\":{elapsed_s:.6},\
                     \"writer_mops\":{writer_mops:.4},\"lock_fallbacks\":{fallbacks},\"population_after\":{final_pop}}}"
                );
            }
        }
    }

    if run_set {
        eprintln!("generating set 63-bit workload (prefill={n0}, fresh={m})...");
        let wl = WriterWorkload::generate(n0, m, 63);
        for &w in &writers_list {
            for round in 0..rounds {
                let (elapsed_s, final_pop, fallbacks) = run_set_cell(&wl, w, round);
                let write_ops = m;
                let writer_mops = (write_ops as f64) / elapsed_s / 1e6;
                let bits = wl.keyspace_bits;
                println!(
                    "{{\"workload_id\":\"concurrency_writer_set_63bit\",\"role\":\"counters\",\
                     \"arm\":\"expanse\",\"cell\":\"set_w{w}_r0\",\"keyspace_bits\":{bits},\
                     \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{w},\"readers\":0,\
                     \"round\":{round},\"write_ops\":{write_ops},\"writer_elapsed_s\":{elapsed_s:.6},\
                     \"writer_mops\":{writer_mops:.4},\"lock_fallbacks\":{fallbacks},\"population_after\":{final_pop}}}"
                );
            }
        }
    }

    if run_str {
        eprintln!("generating str workload (prefill={n0}, fresh={m})...");
        let wl = WriterStrWorkload::generate(n0, m);
        for &w in &writers_list {
            for round in 0..rounds {
                let (elapsed_s, final_pop, fallbacks) = run_str_cell(&wl, w, round);
                let write_ops = m;
                let writer_mops = (write_ops as f64) / elapsed_s / 1e6;
                println!(
                    "{{\"workload_id\":\"concurrency_writer_str\",\"role\":\"counters\",\
                     \"arm\":\"expanse\",\"cell\":\"str_w{w}_r0\",\"dist\":\"short\",\
                     \"prefill\":{n0},\"fresh_keys\":{m},\"writers\":{w},\"readers\":0,\
                     \"round\":{round},\"write_ops\":{write_ops},\"writer_elapsed_s\":{elapsed_s:.6},\
                     \"writer_mops\":{writer_mops:.4},\"lock_fallbacks\":{fallbacks},\"population_after\":{final_pop}}}"
                );
            }
        }
    }
}
