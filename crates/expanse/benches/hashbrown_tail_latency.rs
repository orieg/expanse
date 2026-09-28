//! Pillar 3: HdrHistogram P99.99 Tail-Latency & Ingestion Cliff
//!
//! Measures per-operation latency percentiles during dynamic un-preallocated table growth:
//! - P50, P75, P90, P95, P99, P99.9, P99.99, Max
//! - Captures the SwissTable table-doubling rehash cliff vs Expanse local subexpanse growth.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `hashbrown_tail_latency` |
//! | `group` | 3 |
//! | `population` | 1M (100k under `--quick`) |
//! | `insertion_order` | generator draw order — the population is inserted as drawn, neither sorted nor shuffled |
//! | `probes_and_reuse` | `population` inserts per arm per round, the same key sequence every round |
//! | `hit_rate` | N/A |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | Records clamped latency |
//! | `measured_region` | `Instant::now()` bracket per insert, uncalibrated (the bracket's own cost is inside every sample); the three maps of a round are dropped after the round's last window |
//! | `arm_symmetry` | Symmetric |
//! | `statistics` | HdrHistogram percentiles per arm per round; `run_all.py` runs one process per round (`--round K`, 9 rounds; 3 under `--quick`), each timing every arm once with the arm order rotated by `K`; it publishes per-arm round means, the rows as `rounds_raw`, BCa 95% intervals and paired per-round ratios |
//! | `verdict` | **PASS / MINOR** `[verified: CODE READ]`: rounds and intervals (#1214); the per-op timer bracket is not calibrated, so the low percentiles carry its cost. |

use expanse_trie::map::ExpanseMap;
use hashbrown::HashMap;
use hdrhistogram::Histogram;
use std::collections::BTreeMap;
use std::time::Instant;

struct XorShift64(u64);
impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x4D5E_6F70_8192_A3B4
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

fn measure_growth_latency<F: FnMut(u64, u64)>(mut insert_fn: F, keys: &[u64]) -> Histogram<u64> {
    // 3 significant figures, tracking up to 10 seconds (10_000_000_000 ns)
    let mut hist = Histogram::<u64>::new_with_max(10_000_000_000, 3).unwrap();

    for &k in keys {
        let start = Instant::now();
        insert_fn(k, k);
        let elapsed_ns = start.elapsed().as_nanos() as u64;
        let clamped = elapsed_ns.clamp(1, 9_999_999_999);
        hist.record(clamped).unwrap();
    }
    hist
}

/// The fields `extract_percentiles` writes, in order.
const PERCENTILES: [&str; 8] = [
    "p50_ns",
    "p75_ns",
    "p90_ns",
    "p95_ns",
    "p99_ns",
    "p99_9_ns",
    "p99_99_ns",
    "max_ns",
];

fn extract_percentiles(hist: &Histogram<u64>) -> serde_json::Value {
    serde_json::json!({
        "p50_ns": hist.value_at_quantile(0.50),
        "p75_ns": hist.value_at_quantile(0.75),
        "p90_ns": hist.value_at_quantile(0.90),
        "p95_ns": hist.value_at_quantile(0.95),
        "p99_ns": hist.value_at_quantile(0.99),
        "p99_9_ns": hist.value_at_quantile(0.999),
        "p99_99_ns": hist.value_at_quantile(0.9999),
        "max_ns": hist.max()
    })
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");
    let round = round_arg(&args);

    let num_keys = if quick { 100_000 } else { 1_000_000 };
    let mut rng = XorShift64::new(0xCAFE_BABE_0123_4567);
    let mut keys = Vec::with_capacity(num_keys);
    for _ in 0..num_keys {
        keys.push(rng.next());
    }

    // One row per arm. The round grows one map per arm from empty; the
    // three stay resident until the round ends and are dropped outside
    // every window.
    let mut per_arm: Vec<Vec<serde_json::Value>> = vec![Vec::new(); ARMS.len()];
    for round in [round] {
        let mut expanse_map = ExpanseMap::new();
        let mut hashbrown_map = HashMap::new();
        let mut btree_map = BTreeMap::new();
        for arm in (0..ARMS.len()).map(|i| (i + round) % ARMS.len()) {
            let id = format!("growth/{}/round={round}", ARMS[arm]);
            let hist = bench_window(&id, || match arm {
                0 => measure_growth_latency(
                    |k, v| {
                        expanse_map.insert(k, v);
                    },
                    &keys,
                ),
                1 => measure_growth_latency(
                    |k, v| {
                        hashbrown_map.insert(k, v);
                    },
                    &keys,
                ),
                _ => measure_growth_latency(
                    |k, v| {
                        btree_map.insert(k, v);
                    },
                    &keys,
                ),
            });
            let mut row = extract_percentiles(&hist);
            row["round"] = round.into();
            per_arm[arm].push(row);
        }
        drop((expanse_map, hashbrown_map, btree_map));
    }

    // Per-arm percentiles are means over the rows, one round here;
    // `run_all.py` recomputes them over the rounds it merges.
    let mean = |rows: &[serde_json::Value]| {
        let mut out = serde_json::Map::new();
        for q in PERCENTILES {
            let sum: f64 = rows
                .iter()
                .map(|r| r[q].as_f64().expect("a round row carries every percentile"))
                .sum();
            out.insert(q.into(), (sum / rows.len() as f64).into());
        }
        serde_json::Value::Object(out)
    };
    let cells: Vec<serde_json::Value> = ARMS
        .iter()
        .zip(&per_arm)
        .map(|(arm, rows)| serde_json::json!({ "arm": arm, "rounds_raw": rows }))
        .collect();

    let output = serde_json::json!({
        "total_inserts": num_keys,
        "mode": "un_preallocated_dynamic_growth",
        "expanse": mean(&per_arm[0]),
        "hashbrown": mean(&per_arm[1]),
        "btree": mean(&per_arm[2]),
        "latency": cells
    });

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("{:#?}", output);
    }
}
