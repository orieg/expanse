//! Pillar 2: YCSB (Yahoo! Cloud Serving Benchmark) Workloads A–F
//!
//! Evaluates ExpanseMap vs hashbrown::HashMap vs std::collections::BTreeMap
//! across standard industry database workloads:
//! - Workload A (50% Read, 50% Update)
//! - Workload B (95% Read, 5% Update)
//! - Workload C (100% Read)
//! - Workload D (95% Read, 5% Insert Latest)
//! - Workload E (95% Short Range Scan, 5% Insert) -> Disqualifies hashbrown
//! - Workload F (50% Read, 50% Read-Modify-Write)

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
    let zipf = Zipf::new(dataset_size as u64, 0.99).unwrap();
    // Seeded (#374): the Zipfian stream must be reproducible run-to-run.
    // Committed baselines predate seeding and are refreshed on the next run.
    let mut rand_rng = rand::rngs::StdRng::seed_from_u64(seed);

    let mut ops = Vec::with_capacity(num_ops);
    for _ in 0..num_ops {
        let pct = (rng.next() % 100) as u8;
        let key = zipf.sample(&mut rand_rng) as u64;

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
                    Op::Read(key)
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

fn populate_expanse(n: usize) -> ExpanseMap {
    let mut m = ExpanseMap::new();
    for i in 1..=n as u64 {
        m.insert(i, i * 10);
    }
    m
}

fn populate_hashbrown(n: usize) -> HashMap<u64, u64> {
    let mut m = HashMap::with_capacity(n);
    for i in 1..=n as u64 {
        m.insert(i, i * 10);
    }
    m
}

fn populate_btree(n: usize) -> BTreeMap<u64, u64> {
    let mut m = BTreeMap::new();
    for i in 1..=n as u64 {
        m.insert(i, i * 10);
    }
    m
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    let dataset_size = if quick { 50_000 } else { 500_000 };
    let num_ops = if quick { 20_000 } else { 100_000 };

    let workloads = ['A', 'B', 'C', 'D', 'E', 'F'];
    let mut results = serde_json::Map::new();

    for &wl in &workloads {
        let ops = generate_workload_ops(wl, num_ops, dataset_size, 0x5EED_C001_1234_5678);

        // Expanse Run
        let mut map_exp = populate_expanse(dataset_size);
        let start_exp = Instant::now();
        for op in &ops {
            match *op {
                Op::Read(k) => {
                    black_box(map_exp.get(black_box(k)));
                }
                Op::Update(k, v) | Op::Insert(k, v) => {
                    map_exp.insert(black_box(k), black_box(v));
                }
                Op::Scan(start_k, len) => {
                    let mut count = 0;
                    for (k, v) in map_exp.range(start_k..=u64::MAX) {
                        black_box((k, v));
                        count += 1;
                        if count >= len {
                            break;
                        }
                    }
                    black_box(count);
                }
                Op::Rmw(k) => {
                    if let Some(v) = map_exp.get(k) {
                        map_exp.insert(k, v + 1);
                    }
                }
            }
        }
        let dur_exp = start_exp.elapsed().as_secs_f64();
        let mops_exp = (num_ops as f64 / dur_exp) / 1e6;

        // BTreeMap Run
        let mut map_bt = populate_btree(dataset_size);
        let start_bt = Instant::now();
        for op in &ops {
            match *op {
                Op::Read(k) => {
                    black_box(map_bt.get(&black_box(k)));
                }
                Op::Update(k, v) | Op::Insert(k, v) => {
                    map_bt.insert(black_box(k), black_box(v));
                }
                Op::Scan(start_k, len) => {
                    let mut count = 0;
                    for (k, v) in map_bt.range(start_k..) {
                        black_box((k, v));
                        count += 1;
                        if count >= len {
                            break;
                        }
                    }
                    black_box(count);
                }
                Op::Rmw(k) => {
                    if let Some(&v) = map_bt.get(&k) {
                        map_bt.insert(k, v + 1);
                    }
                }
            }
        }
        let dur_bt = start_bt.elapsed().as_secs_f64();
        let mops_bt = (num_ops as f64 / dur_bt) / 1e6;

        // Hashbrown Run (Disqualified for Workload E)
        let (mops_hb, hb_status) = if wl == 'E' {
            (
                0.0,
                "DISQUALIFIED: cannot perform ordered range scans without full O(N log N) dump and sort",
            )
        } else {
            let mut map_hb = populate_hashbrown(dataset_size);
            let start_hb = Instant::now();
            for op in &ops {
                match *op {
                    Op::Read(k) => {
                        black_box(map_hb.get(&black_box(k)));
                    }
                    Op::Update(k, v) | Op::Insert(k, v) => {
                        map_hb.insert(black_box(k), black_box(v));
                    }
                    Op::Scan(..) => unreachable!(),
                    Op::Rmw(k) => {
                        if let Some(&v) = map_hb.get(&k) {
                            map_hb.insert(k, v + 1);
                        }
                    }
                }
            }
            let dur_hb = start_hb.elapsed().as_secs_f64();
            let m = (num_ops as f64 / dur_hb) / 1e6;
            (m, "COMPLETED")
        };

        let wl_name = format!("workload_{}", wl.to_ascii_lowercase());
        results.insert(wl_name, serde_json::json!({
            "workload": format!("Workload {}", wl),
            "description": match wl {
                'A' => "50% Read, 50% Update (Heavy Update)",
                'B' => "95% Read, 5% Update (Read Heavy)",
                'C' => "100% Read (Read Only)",
                'D' => "95% Read, 5% Insert (Latest Insert)",
                'E' => "95% Short Range Scan, 5% Insert (Scan)",
                'F' => "50% Read, 50% Read-Modify-Write (RMW)",
                _ => "",
            },
            "expanse_mops": mops_exp,
            "btree_mops": mops_bt,
            "hashbrown_mops": if wl == 'E' { serde_json::Value::Null } else { serde_json::json!(mops_hb) },
            "hashbrown_status": hb_status
        }));
    }

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    } else {
        println!("{:#?}", results);
    }
}
