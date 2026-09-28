//! Pillar 4: Martin Ankerl & Tessil Container Key Distribution Suite
//!
//! Evaluates ExpanseMap vs hashbrown::HashMap vs std::collections::BTreeMap
//! across distinct key distributions recognized in systems literature:
//! 1. Uniform Random 64-bit
//! 2. Dense Sequential (0..N)
//! 3. Sparse Clustered / Stride
//! 4. Zipfian Skewed (s = 0.99)
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `hashbrown_container_dists` |
//! | `group` | 3 |
//! | `population` | 500k (50k under `--quick`) |
//! | `insertion_order` | generator draw order — the population is inserted as drawn, neither sorted nor shuffled |
//! | `probes_and_reuse` | Lookups walk the inserted key sequence in draw order, 500k probes (100k under `--quick`) |
//! | `hit_rate` | 100% |
//! | `miss_gen_method` | None |
//! | `value_dereference` | `black_box(get)` |
//! | `measured_region` | Clean: each arm's build and its lookup loop are timed separately; key generation is outside the clock and the round's maps are dropped after it |
//! | `arm_symmetry` | Symmetric |
//! | `statistics` | `run_all.py` runs one process per round (`--round K`, 9 rounds; 3 under `--quick`), each timing every arm once with the arm order rotated by `K`; it publishes per-arm round means, the rows as `rounds_raw`, BCa 95% intervals and paired per-round ratios |
//! | `verdict` | **PASS** `[verified: CODE READ]`: clean timing; rounds and intervals (#1214). |

use expanse_trie::map::ExpanseMap;
use hashbrown::HashMap;
use rand::SeedableRng;
use rand_distr::{Distribution, Zipf};
use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::Instant;

struct XorShift64(u64);
impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x89AB_CDEF_0123_4567
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

fn generate_distribution(dist: &str, n: usize, seed: u64) -> Vec<u64> {
    let mut rng = XorShift64::new(seed);
    let mut keys = Vec::with_capacity(n);
    match dist {
        "uniform" => {
            for _ in 0..n {
                keys.push(rng.next());
            }
        }
        "sequential" => {
            for i in 0..n {
                keys.push(i as u64);
            }
        }
        "clustered" => {
            let mut base = 0u64;
            for i in 0..n {
                if i % 256 == 0 {
                    base = rng.next() & !0xFF;
                }
                keys.push(base + (i % 256) as u64);
            }
        }
        "zipfian" => {
            let zipf = Zipf::new(n as u64, 0.99).unwrap();
            // Seeded (#374): the Zipfian stream must be reproducible
            // run-to-run. Committed baselines predate seeding and are
            // refreshed on the next run.
            let mut rand_rng = rand::rngs::StdRng::seed_from_u64(seed);
            for _ in 0..n {
                keys.push(zipf.sample(&mut rand_rng) as u64);
            }
        }
        _ => unreachable!(),
    }
    keys
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

/// The maps a round builds, one per arm.
#[derive(Default)]
struct Maps {
    expanse: Option<ExpanseMap>,
    hashbrown: Option<HashMap<u64, u64>>,
    btree: Option<BTreeMap<u64, u64>>,
}

/// Builds `arm`'s map from `keys` into `maps`; returns the insert Mops/s.
fn insert(arm: usize, keys: &[u64], maps: &mut Maps) -> f64 {
    let start = Instant::now();
    match arm {
        0 => {
            let mut m = ExpanseMap::new();
            for &k in keys {
                m.insert(black_box(k), black_box(k));
            }
            let secs = start.elapsed().as_secs_f64();
            maps.expanse = Some(m);
            (keys.len() as f64 / secs) / 1e6
        }
        1 => {
            let mut m = HashMap::new();
            for &k in keys {
                m.insert(black_box(k), black_box(k));
            }
            let secs = start.elapsed().as_secs_f64();
            maps.hashbrown = Some(m);
            (keys.len() as f64 / secs) / 1e6
        }
        _ => {
            let mut m = BTreeMap::new();
            for &k in keys {
                m.insert(black_box(k), black_box(k));
            }
            let secs = start.elapsed().as_secs_f64();
            maps.btree = Some(m);
            (keys.len() as f64 / secs) / 1e6
        }
    }
}

/// `query_count` lookups of `keys`, cycled, against `arm`'s map; returns Mops/s.
fn lookup(arm: usize, keys: &[u64], query_count: usize, maps: &Maps) -> f64 {
    let n = keys.len();
    let start = Instant::now();
    match arm {
        0 => {
            let m = maps.expanse.as_ref().expect("built before it is read");
            for i in 0..query_count {
                let k = keys[i % n];
                black_box(m.get(black_box(k)));
            }
        }
        1 => {
            let m = maps.hashbrown.as_ref().expect("built before it is read");
            for i in 0..query_count {
                let k = keys[i % n];
                black_box(m.get(&black_box(k)));
            }
        }
        _ => {
            let m = maps.btree.as_ref().expect("built before it is read");
            for i in 0..query_count {
                let k = keys[i % n];
                black_box(m.get(&black_box(k)));
            }
        }
    }
    (query_count as f64 / start.elapsed().as_secs_f64()) / 1e6
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");
    let round = round_arg(&args);

    let num_keys = if quick { 50_000 } else { 500_000 };
    let query_count = if quick { 100_000 } else { 500_000 };
    let distributions = ["uniform", "sequential", "clustered", "zipfian"];

    let mut results = serde_json::Map::new();
    let mut cells = Vec::new();

    for &dist in &distributions {
        // One load window per distribution, read from stderr by
        // `scripts/bench_windowed.py` (AGENTS.md section 8.17, #1214).
        eprintln!("BENCH_WINDOW begin {dist}");
        let keys = generate_distribution(dist, num_keys, 0x1337_C0DE_CAFE_BABE);

        // One row per arm. Every arm builds its map, then every arm reads
        // its own, both in the round's rotated order.
        let mut rows: Vec<serde_json::Value> = Vec::new();
        for round in [round] {
            let order: Vec<usize> = (0..ARMS.len()).map(|i| (i + round) % ARMS.len()).collect();
            let mut maps = Maps::default();
            let mut ins = [0.0f64; 3];
            let mut get = [0.0f64; 3];
            for &arm in &order {
                ins[arm] = insert(arm, &keys, &mut maps);
            }
            for &arm in &order {
                get[arm] = lookup(arm, &keys, query_count, &maps);
            }
            drop(maps);
            for arm in 0..ARMS.len() {
                rows.push(serde_json::json!({
                    "round": round,
                    "arm": ARMS[arm],
                    "insert_mops": ins[arm],
                    "lookup_mops": get[arm]
                }));
            }
        }

        // Per-arm figures are means over `rows`, one round here;
        // `run_all.py` recomputes them over the rounds it merges.
        let mean = |field: &str| {
            let mut out = serde_json::Map::new();
            for arm in ARMS {
                let v: Vec<f64> = rows
                    .iter()
                    .filter(|r| r["arm"] == arm)
                    .map(|r| r[field].as_f64().expect("a round row carries its field"))
                    .collect();
                out.insert(arm.into(), (v.iter().sum::<f64>() / v.len() as f64).into());
            }
            serde_json::Value::Object(out)
        };

        results.insert(
            dist.to_string(),
            serde_json::json!({
                "distribution": dist,
                "population": num_keys,
                "insert_mops": mean("insert_mops"),
                "lookup_mops": mean("lookup_mops")
            }),
        );
        cells.push(serde_json::json!({
            "distribution": dist,
            "population": num_keys,
            "rounds_raw": rows
        }));
        eprintln!("BENCH_WINDOW end {dist}");
    }
    results.insert("throughput".into(), cells.into());

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    } else {
        println!("{:#?}", results);
    }
}
