//! Pillar 2: YCSB (Yahoo! Cloud Serving Benchmark) Workloads A–F
//!
//! Evaluates ExpanseMap vs hashbrown::HashMap vs std::collections::BTreeMap
//! across standard industry database workloads, on **dense sequential keys**
//! (`1..=N`) with `u64` values:
//! - Workload A (50% Read, 50% Update)
//! - Workload B (95% Read, 5% Update)
//! - Workload C (100% Read)
//! - Workload D (95% Read Latest, 5% Insert) — reads are Zipfian over recency:
//!   rank 1 is the newest key, *including the keys this stream has inserted so
//!   far*. Until #1005 the reads were a plain Zipfian draw over the initial
//!   population and never touched an inserted key, so the cell was workload B
//!   with inserts in place of updates; it is read-latest now rather than
//!   relabelled, because read-latest is what the published row is called.
//! - Workload E (95% Short Range Scan, 5% Insert) -> Disqualifies hashbrown
//! - Workload F (50% Read, 50% Read-Modify-Write)
//!
//! **This E is not `workload_ycsb`'s E.** Here a scan takes 10..=59 records
//! with no predicate over dense sequential `u64` values; `benches/ycsb.rs`
//! takes 10..=100 records that pass a key-parity predicate over 128 B blobs.
//! The two cells share a letter and nothing else, and no figure from one is
//! comparable with a figure from the other.
//!
//! # Rounds, orders and the pin
//!
//! A cell is one `(workload, arm, insertion order)`; a run measures it
//! `--rounds` times (default 5), round-major, with the arm that goes first
//! rotating by round and workload. `BTreeMap` is order-sensitive — ascending
//! insertion is its rightmost-append path — so every cell is built once sorted
//! ascending and once Fisher–Yates shuffled, and every row says which
//! (AGENTS.md §8.12.4). Every row carries its round, so an interval can exist:
//! `scripts/ycsb_bench.py --suite hashbrown` runs one process per round, takes the core pin through `scripts/bench_pin.py`,
//! snapshots host load around each round and puts BCa 95% intervals on every
//! cell and every paired per-round ratio. This binary times; it does not pin.
//!
//! Flags: `--json`, `--quick`, `--rounds N`, `--round-index I`,
//! `--population N`, `--ops N`. Under `cargo test --bench` (no `--bench` flag
//! from cargo) the run is a smoke: quick sizes, one round, and it says so.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `hashbrown_ycsb` |
//! | `group` | 3 |
//! | `population` | 500k dense sequential keys `1..=N` (`--quick`: 50k; `--population` overrides), recorded in every row |
//! | `insertion_order` | both — every cell is built once ascending and once Fisher–Yates shuffled from the suite PRNG, the order recorded in every row; the operation stream is the same for both |
//! | `probes_and_reuse` | 100k ops per cell (`--quick`: 20k; `--ops` overrides); one seeded stream per workload, reused across arms, orders and rounds |
//! | `hit_rate` | 100% on reads — Zipfian θ = 0.99 over the population (workload D: Zipfian over recency, in-run inserts first) |
//! | `miss_gen_method` | n/a — no miss probes; every read key is present |
//! | `value_dereference` | `black_box` on op results; values are `u64`, so there is no payload to dereference |
//! | `measured_region` | Op loop only: the population is built before the timer starts and dropped after it stops |
//! | `arm_symmetry` | One op stream for every arm; a per-cell work checksum asserted equal across arms; hashbrown is disqualified from E (no ordered scan) and reported as such, never as zero |
//! | `statistics` | Per-round Mops/s rows; BCa 95% intervals and paired per-round ratios are computed by `scripts/ycsb_bench.py` |
//! | `verdict` | **RE-MEASURE PENDING (#1005)** `[verified: CODE READ]`: the committed artifact is one unpinned pass per cell on ascending keys with a non-read-latest D; this table now matches the code. |

use expanse_trie::map::ExpanseMap;
use hashbrown::HashMap;
use rand::SeedableRng;
use rand_distr::{Distribution, Zipf};
use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::Instant;

const STREAM_SEED: u64 = 0x5EED_C001_1234_5678;
const SHUFFLE_SEED: u64 = 0x5EED_0F0F_1005_0002;
const THETA: f64 = 0.99;

struct XorShift64(u64);
impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x1A2B_3C4D_5E6F_7081
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

#[derive(Copy, Clone)]
enum Op {
    Read(u64),
    Update(u64, u64),
    Insert(u64, u64),
    Scan(u64, usize),
    Rmw(u64),
}

fn generate_workload_ops(
    workload: char,
    num_ops: usize,
    dataset_size: usize,
    seed: u64,
) -> Vec<Op> {
    let mut rng = XorShift64::new(seed);
    let mut next_insert_key = dataset_size as u64 + 1;
    let zipf = Zipf::new(dataset_size as u64, THETA).unwrap();
    // Seeded (#374): the Zipfian stream must be reproducible run-to-run.
    let mut rand_rng = rand::rngs::StdRng::seed_from_u64(seed);

    let mut ops = Vec::with_capacity(num_ops);
    for _ in 0..num_ops {
        let pct = (rng.next() % 100) as u8;
        // Rank in 1..=dataset_size; for A, B, C, E, F the rank is the key.
        let rank = zipf.sample(&mut rand_rng) as u64;
        let key = rank;

        let op = match workload {
            'A' => {
                if pct < 50 {
                    Op::Read(key)
                } else {
                    Op::Update(key, rng.next())
                }
            }
            'B' => {
                if pct < 95 {
                    Op::Read(key)
                } else {
                    Op::Update(key, rng.next())
                }
            }
            'C' => Op::Read(key),
            'D' => {
                if pct < 95 {
                    // Read-latest: keys are dense and inserts append, so the
                    // newest key is `next_insert_key - 1` and rank r is the
                    // r-th newest. Every such key is present: the oldest a
                    // rank can reach is `latest - dataset_size + 1 >= 1`.
                    let latest = next_insert_key - 1;
                    Op::Read(latest - (rank - 1))
                } else {
                    let k = next_insert_key;
                    next_insert_key += 1;
                    Op::Insert(k, rng.next())
                }
            }
            'E' => {
                if pct < 95 {
                    let scan_len = (rng.next() % 50 + 10) as usize;
                    Op::Scan(key, scan_len)
                } else {
                    let k = next_insert_key;
                    next_insert_key += 1;
                    Op::Insert(k, rng.next())
                }
            }
            'F' => {
                if pct < 50 {
                    Op::Read(key)
                } else {
                    Op::Rmw(key)
                }
            }
            _ => unreachable!(),
        };
        ops.push(op);
    }
    ops
}

/// The population `1..=n` in the order it is inserted.
fn build_keys(n: usize, order: &str) -> Vec<u64> {
    let mut keys: Vec<u64> = (1..=n as u64).collect();
    match order {
        "sorted" => {}
        "shuffled" => {
            let mut rng = XorShift64::new(SHUFFLE_SEED);
            for i in (1..keys.len()).rev() {
                let j = (rng.next() % (i as u64 + 1)) as usize;
                keys.swap(i, j);
            }
        }
        other => panic!("unknown insertion order {other:?}"),
    }
    keys
}

/// One timed cell: `(seconds, work checksum)`. The checksum counts reads and
/// RMWs that found their key plus records a scan consumed; it is the loop's
/// dead-code-elimination sink and the arm-symmetry check.
fn run_expanse(keys: &[u64], ops: &[Op]) -> (f64, u64) {
    let mut map = ExpanseMap::new();
    for &k in keys {
        map.insert(k, k * 10);
    }
    let mut consumed = 0u64;
    let start = Instant::now();
    for op in ops {
        match *op {
            Op::Read(k) => {
                let v = map.get(black_box(k));
                black_box(v);
                consumed += v.is_some() as u64;
            }
            Op::Update(k, v) | Op::Insert(k, v) => {
                map.insert(black_box(k), black_box(v));
            }
            Op::Scan(start_k, len) => {
                let mut count = 0;
                for (k, v) in map.range(start_k..=u64::MAX) {
                    black_box((k, v));
                    count += 1;
                    if count >= len {
                        break;
                    }
                }
                consumed += count as u64;
            }
            Op::Rmw(k) => {
                if let Some(v) = map.get(k) {
                    map.insert(k, v + 1);
                    consumed += 1;
                }
            }
        }
    }
    let secs = start.elapsed().as_secs_f64();
    black_box(consumed);
    // `map` drops here, after the timer stopped.
    (secs, consumed)
}

fn run_btree(keys: &[u64], ops: &[Op]) -> (f64, u64) {
    let mut map = BTreeMap::new();
    for &k in keys {
        map.insert(k, k * 10);
    }
    let mut consumed = 0u64;
    let start = Instant::now();
    for op in ops {
        match *op {
            Op::Read(k) => {
                let v = map.get(&black_box(k));
                black_box(v);
                consumed += v.is_some() as u64;
            }
            Op::Update(k, v) | Op::Insert(k, v) => {
                map.insert(black_box(k), black_box(v));
            }
            Op::Scan(start_k, len) => {
                let mut count = 0;
                for (k, v) in map.range(start_k..) {
                    black_box((k, v));
                    count += 1;
                    if count >= len {
                        break;
                    }
                }
                consumed += count as u64;
            }
            Op::Rmw(k) => {
                if let Some(&v) = map.get(&k) {
                    map.insert(k, v + 1);
                    consumed += 1;
                }
            }
        }
    }
    let secs = start.elapsed().as_secs_f64();
    black_box(consumed);
    (secs, consumed)
}

fn run_hashbrown(keys: &[u64], ops: &[Op]) -> (f64, u64) {
    let mut map: HashMap<u64, u64> = HashMap::with_capacity(keys.len());
    for &k in keys {
        map.insert(k, k * 10);
    }
    let mut consumed = 0u64;
    let start = Instant::now();
    for op in ops {
        match *op {
            Op::Read(k) => {
                let v = map.get(&black_box(k));
                black_box(v);
                consumed += v.is_some() as u64;
            }
            Op::Update(k, v) | Op::Insert(k, v) => {
                map.insert(black_box(k), black_box(v));
            }
            Op::Scan(..) => unreachable!("hashbrown is disqualified from workload E"),
            Op::Rmw(k) => {
                if let Some(&v) = map.get(&k) {
                    map.insert(k, v + 1);
                    consumed += 1;
                }
            }
        }
    }
    let secs = start.elapsed().as_secs_f64();
    black_box(consumed);
    (secs, consumed)
}

const ARMS: [&str; 3] = ["expanse", "btree", "hashbrown"];
const ORDERS: [&str; 2] = ["sorted", "shuffled"];
const HASHBROWN_E: &str =
    "DISQUALIFIED: cannot perform ordered range scans without full O(N log N) dump and sort";

fn description(wl: char) -> &'static str {
    match wl {
        'A' => "50% Read, 50% Update (Heavy Update)",
        'B' => "95% Read, 5% Update (Read Heavy)",
        'C' => "100% Read (Read Only)",
        'D' => "95% Read Latest, 5% Insert (Latest Insert)",
        'E' => "95% Short Range Scan, 5% Insert (Scan)",
        'F' => "50% Read, 50% Read-Modify-Write (RMW)",
        _ => "",
    }
}

/// The value after `flag`, parsed, or `default` when the flag is absent. A flag
/// with a missing or malformed value stops the run (AGENTS.md §8.1).
fn flag_value(args: &[String], flag: &str, default: usize) -> usize {
    match args.iter().position(|a| a == flag) {
        None => default,
        Some(i) => args
            .get(i + 1)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or_else(|| panic!("{flag} needs an integer value")),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let json_mode = args.iter().any(|a| a == "--json");
    // `cargo bench` passes `--bench`; `cargo test --bench` does not. Without it
    // this is a smoke run, sized like `--quick` with one round, and says so.
    let smoke = !args.iter().any(|a| a == "--bench");
    let quick = smoke || args.iter().any(|a| a == "--quick");

    let dataset_size = flag_value(&args, "--population", if quick { 50_000 } else { 500_000 });
    let num_ops = flag_value(&args, "--ops", if quick { 20_000 } else { 100_000 });
    let rounds = flag_value(&args, "--rounds", if smoke { 1 } else { 5 });
    let round_base = flag_value(&args, "--round-index", 0);
    assert!(dataset_size >= 2 && num_ops >= 1 && rounds >= 1);
    if smoke {
        eprintln!(
            "hashbrown_ycsb: SMOKE RUN (no --bench flag): population={dataset_size} ops={num_ops} rounds={rounds}; not a measurement"
        );
    }

    let workloads = ['A', 'B', 'C', 'D', 'E', 'F'];
    let streams: Vec<Vec<Op>> = workloads
        .iter()
        .map(|&wl| generate_workload_ops(wl, num_ops, dataset_size, STREAM_SEED))
        .collect();
    let built: Vec<Vec<u64>> = ORDERS.iter().map(|o| build_keys(dataset_size, o)).collect();

    let mut rows = Vec::new();
    // Round-major: every cell of a round runs before any cell of the next, so
    // drift across the run lands inside every cell's samples alike.
    for r in 0..rounds {
        let round = round_base + r;
        for (oi, order) in ORDERS.iter().enumerate() {
            for (wi, &wl) in workloads.iter().enumerate() {
                let ops = &streams[wi];
                let mut checksum: Option<u64> = None;
                for slot in 0..ARMS.len() {
                    let arm = ARMS[(slot + round + wi) % ARMS.len()];
                    let wl_name = format!("workload_{}", wl.to_ascii_lowercase());
                    if arm == "hashbrown" && wl == 'E' {
                        rows.push(serde_json::json!({
                            "workload": wl_name,
                            "arm": arm,
                            "insertion_order": order,
                            "population": dataset_size,
                            "round": round,
                            "status": HASHBROWN_E,
                        }));
                        continue;
                    }
                    let (secs, consumed) = match arm {
                        "expanse" => run_expanse(&built[oi], ops),
                        "btree" => run_btree(&built[oi], ops),
                        _ => run_hashbrown(&built[oi], ops),
                    };
                    // Deterministic invariant (AGENTS.md §8.4): one stream over
                    // one population does the same work on every arm.
                    match checksum {
                        None => checksum = Some(consumed),
                        Some(c) => assert_eq!(
                            c, consumed,
                            "workload {wl}/{arm}/{order}: arms did different work"
                        ),
                    }
                    rows.push(serde_json::json!({
                        "workload": wl_name,
                        "arm": arm,
                        "insertion_order": order,
                        "population": dataset_size,
                        "round": round,
                        "ops": num_ops,
                        "elapsed_s": secs,
                        "mops": (num_ops as f64 / secs) / 1e6,
                        "consumed": consumed,
                        "status": "COMPLETED",
                    }));
                }
            }
        }
    }

    let out = serde_json::json!({
        "workload_id": "hashbrown_ycsb",
        "settings": {
            "population": dataset_size,
            "ops_per_cell": num_ops,
            "rounds": rounds,
            "round_base": round_base,
            "insertion_orders": ORDERS,
            "zipfian_theta": THETA,
            "stream_seed": format!("{STREAM_SEED:#x}"),
            "smoke": smoke,
            "quick": quick,
        },
        "descriptions": workloads
            .iter()
            .map(|&wl| (format!("workload_{}", wl.to_ascii_lowercase()), description(wl)))
            .collect::<std::collections::BTreeMap<_, _>>(),
        "rows": rows,
    });

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        println!("{out:#}");
    }
}
