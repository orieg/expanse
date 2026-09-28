//! Pillar 1: hashbrown Native Criterion Suite Port
//!
//! Mirrors the upstream hashbrown/benches/bench.rs suite across:
//! - insert_growing (un-preallocated dynamic growth)
//! - lookup_hit (point query on present keys)
//! - lookup_miss (point query on absent keys)
//! - iter_all (full container iteration)
//!
//! Supports standalone execution with `--json` for automated script collection,
//! and `--round K` for which round the process is (the arm order rotates by
//! `K`). Each population is one `BENCH_WINDOW` load window.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `hashbrown_native_suite` |
//! | `group` | 3 |
//! | `population` | 10k, 100k, 500k |
//! | `insertion_order` | generator draw order — the population is inserted as drawn, neither sorted nor shuffled |
//! | `probes_and_reuse` | Sub-slice `iters < pop` sequential index |
//! | `hit_rate` | 100% hit / 100% miss arms |
//! | `miss_gen_method` | Separate PRNG seed (no membership check) |
//! | `value_dereference` | `black_box(get)` |
//! | `measured_region` | Lookups and scans: the loop of `iters` / `reps` ops. `insert_growing`: each build is timed alone and its map dropped after the clock stops (`bench_grow_op`). Map prepopulation is outside the window |
//! | `arm_symmetry` | Symmetric |
//! | `statistics` | `run_all.py` runs one process per round (`--round K`, 9 rounds; 3 under `--quick`), each timing every arm once with the arm order rotated by `K`; it publishes per-arm round means, the rows as `rounds_raw`, BCa 95% intervals and paired per-round ratios |
//! | `verdict` | **PASS / MINOR (Class 2)** `[verified: CODE READ]`: drop outside the timed build (#470), rounds and intervals (#1214); lookups still walk the first `iters` keys in index order. |

use expanse_trie::map::ExpanseMap;
use hashbrown::HashMap;
use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::Instant;

struct XorShift64(u64);
impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0xDEAD_BEEF_CAFE_BABE
        } else {
            seed
        })
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

fn generate_keys(n: usize, seed: u64) -> Vec<u64> {
    let mut rng = XorShift64::new(seed);
    let mut keys = Vec::with_capacity(n);
    for _ in 0..n {
        keys.push(rng.next());
    }
    keys
}

fn bench_op<F: FnMut()>(mut op: F, warmup_iters: usize, measure_iters: usize) -> (f64, f64) {
    for _ in 0..warmup_iters {
        op();
    }
    let start = Instant::now();
    for _ in 0..measure_iters {
        op();
    }
    let elapsed = start.elapsed();
    let total_secs = elapsed.as_secs_f64();
    let ns_per_op = (total_secs * 1e9) / measure_iters as f64;
    let mops = (measure_iters as f64 / total_secs) / 1e6;
    (ns_per_op, mops)
}

fn bench_grow_op<F: FnMut() -> R, R>(
    mut op: F,
    warmup_iters: usize,
    measure_iters: usize,
) -> (f64, f64) {
    for _ in 0..warmup_iters {
        let res = op();
        drop(res);
    }
    let mut total_duration = std::time::Duration::ZERO;
    for _ in 0..measure_iters {
        let start = Instant::now();
        let res = op();
        let elapsed = start.elapsed();
        total_duration += elapsed;
        drop(res);
    }
    let total_secs = total_duration.as_secs_f64();
    let ns_per_op = (total_secs * 1e9) / measure_iters as f64;
    let mops = (measure_iters as f64 / total_secs) / 1e6;
    (ns_per_op, mops)
}

/// The three arms, in the order they are rotated through: round `r` starts
/// with arm `r % 3`, so no arm is always timed first or last.
const ARMS: [&str; 3] = ["expanse", "hashbrown", "btree"];

/// Which round this process is: `--round K`, else 0. A process measures one
/// round, with the arm order rotated by `K`; `run_all.py` runs one process
/// per round, because a later round in the same process reuses memory the
/// round before it freed and inserts measurably faster (#1214).
fn round_arg(args: &[String]) -> usize {
    match args.iter().position(|a| a == "--round") {
        Some(i) => args
            .get(i + 1)
            .and_then(|v| v.parse().ok())
            .expect("--round takes a non-negative integer"),
        None => 0,
    }
}

/// Runs one timed case between two window markers on stderr, which
/// `scripts/bench_windowed.py` reads to snapshot the host's busy CPU and this
/// process's own CPU at each boundary (AGENTS.md section 8.17, #1214).
fn bench_window<T>(id: &str, case: impl FnOnce() -> T) -> T {
    eprintln!("BENCH_WINDOW begin {id}");
    let out = case();
    eprintln!("BENCH_WINDOW end {id}");
    out
}

/// The prepopulated maps the lookup and iteration cases read.
struct Bases {
    expanse: ExpanseMap,
    hashbrown: HashMap<u64, u64>,
    btree: BTreeMap<u64, u64>,
}

/// `(ns_per_op, mops)` of `iters` point lookups over `probes`, walked from
/// index 1 as the upstream port does.
fn lookup(arm: usize, b: &Bases, probes: &[u64], iters: usize) -> (f64, f64) {
    let mut idx = 0;
    match arm {
        0 => bench_op(
            || {
                idx = (idx + 1) % probes.len();
                black_box(b.expanse.get(black_box(probes[idx])));
            },
            5_000,
            iters,
        ),
        1 => bench_op(
            || {
                idx = (idx + 1) % probes.len();
                black_box(b.hashbrown.get(&black_box(probes[idx])));
            },
            5_000,
            iters,
        ),
        _ => bench_op(
            || {
                idx = (idx + 1) % probes.len();
                black_box(b.btree.get(&black_box(probes[idx])));
            },
            5_000,
            iters,
        ),
    }
}

/// `ns_per_scan` of a full iteration, averaged over `reps` scans.
fn iterate(arm: usize, b: &Bases, reps: usize) -> f64 {
    fn scan<T>(it: impl Iterator<Item = T>) {
        let mut count = 0usize;
        for kv in it {
            black_box(kv);
            count += 1;
        }
        black_box(count);
    }
    let (ns, _) = match arm {
        0 => bench_op(|| scan(b.expanse.iter()), 2, reps),
        1 => bench_op(|| scan(b.hashbrown.iter()), 2, reps),
        _ => bench_op(|| scan(b.btree.iter()), 2, reps),
    };
    ns
}

/// `ns` per build of a map grown from empty to `keys.len()`, with the drop
/// outside the timed region.
fn grow(arm: usize, keys: &[u64], reps: usize) -> f64 {
    let (ns, _) = match arm {
        0 => bench_grow_op(
            || {
                let mut m = ExpanseMap::new();
                for &k in keys {
                    m.insert(black_box(k), black_box(k));
                }
                m
            },
            1,
            reps,
        ),
        1 => bench_grow_op(
            || {
                let mut m = HashMap::new();
                for &k in keys {
                    m.insert(black_box(k), black_box(k));
                }
                m
            },
            1,
            reps,
        ),
        _ => bench_grow_op(
            || {
                let mut m = BTreeMap::new();
                for &k in keys {
                    m.insert(black_box(k), black_box(k));
                }
                m
            },
            1,
            reps,
        ),
    };
    ns
}

fn mean(v: impl Iterator<Item = f64>) -> f64 {
    let (sum, n) = v.fold((0.0, 0usize), |(s, n), x| (s + x, n + 1));
    sum / n as f64
}

/// The mean over the rounds of `field` for `(op, arm)`.
fn mean_of(rows: &[serde_json::Value], op: &str, arm: &str, field: &str) -> f64 {
    mean(
        rows.iter()
            .filter(|r| r["op"] == op && r["arm"] == arm)
            .map(|r| r[field].as_f64().expect("a round row carries its field")),
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");
    let round = round_arg(&args);

    let pops = if quick {
        vec![10_000, 100_000]
    } else {
        vec![10_000, 100_000, 500_000]
    };

    let mut results = Vec::new();

    for &pop in &pops {
        let keys = generate_keys(pop, 0x1234_5678_9ABC_DEF0);
        let absent_keys = generate_keys(pop, 0xFEDC_BA98_7654_3210);

        // Prepopulate maps for read/iter tests
        let mut bases = Bases {
            expanse: ExpanseMap::new(),
            hashbrown: HashMap::new(),
            btree: BTreeMap::new(),
        };
        for &k in &keys {
            bases.expanse.insert(k, k);
            bases.hashbrown.insert(k, k);
            bases.btree.insert(k, k);
        }

        let iters = if pop <= 10_000 {
            100_000
        } else if pop <= 100_000 {
            50_000
        } else {
            10_000
        };
        let iter_reps = if pop <= 10_000 {
            100
        } else if pop <= 100_000 {
            20
        } else {
            5
        };
        let build_reps = if pop <= 10_000 {
            30
        } else if pop <= 100_000 {
            5
        } else {
            2
        };

        // One row per (op, arm) of this process's round. Every op runs every
        // arm, in the round's rotated order, so a round's arms are paired.
        let rows = bench_window(&format!("pop={pop}"), || {
            let mut rows = Vec::new();
            for round in [round] {
                let order = (0..ARMS.len()).map(|i| (i + round) % ARMS.len());
                for arm in order.clone() {
                    let (ns, mops) = lookup(arm, &bases, &keys, iters);
                    rows.push(serde_json::json!({ "round": round, "op": "lookup_hit",
                        "arm": ARMS[arm], "ns_per_op": ns, "mops": mops }));
                }
                for arm in order.clone() {
                    let (ns, mops) = lookup(arm, &bases, &absent_keys, iters);
                    rows.push(serde_json::json!({ "round": round, "op": "lookup_miss",
                        "arm": ARMS[arm], "ns_per_op": ns, "mops": mops }));
                }
                for arm in order.clone() {
                    let ns = iterate(arm, &bases, iter_reps);
                    rows.push(serde_json::json!({ "round": round, "op": "iter_all",
                        "arm": ARMS[arm], "ns_per_scan": ns,
                        "mops_items": (pop as f64 / (ns * 1e-9)) / 1e6 }));
                }
                for arm in order {
                    let ns = grow(arm, &keys, build_reps);
                    rows.push(serde_json::json!({ "round": round, "op": "insert_growing",
                        "arm": ARMS[arm], "ns_per_build": ns,
                        "mops": (pop as f64 / (ns * 1e-9)) / 1e6 }));
                }
            }
            rows
        });

        // Per-arm figures are means over `rows`, which is one round here;
        // `run_all.py` recomputes them over the rounds it merges.
        let per_arm = |op: &str, fields: &[&str]| {
            let mut m = serde_json::Map::new();
            for arm in ARMS {
                let mut f = serde_json::Map::new();
                for &field in fields {
                    f.insert(field.into(), mean_of(&rows, op, arm, field).into());
                }
                m.insert(arm.into(), f.into());
            }
            serde_json::Value::Object(m)
        };
        results.push(serde_json::json!({
            "population": pop,
            "lookup_hit": per_arm("lookup_hit", &["ns_per_op", "mops"]),
            "lookup_miss": per_arm("lookup_miss", &["ns_per_op", "mops"]),
            "iter_all": per_arm("iter_all", &["ns_per_scan", "mops_items"]),
            "insert_growing": per_arm("insert_growing", &["mops"]),
            "rounds_raw": rows,
        }));
    }

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    } else {
        println!("{:#?}", results);
    }
}
