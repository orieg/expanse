//! Pillar 4: Martin Ankerl & Tessil Container Key Distribution Suite
//!
//! Evaluates ExpanseMap vs hashbrown::HashMap vs std::collections::BTreeMap
//! across distinct key distributions recognized in systems literature:
//! 1. Uniform Random 64-bit
//! 2. Dense Sequential (0..N)
//! 3. Sparse Clustered / Stride
//! 4. Zipfian Skewed (s = 0.99)

use expanse_trie::map::ExpanseMap;
use hashbrown::HashMap;
use rand_distr::{Distribution, Zipf};
use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::Instant;

struct XorShift64(u64);
impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0x89AB_CDEF_0123_4567 } else { seed })
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
            let mut rand_rng = rand::thread_rng();
            for _ in 0..n {
                keys.push(zipf.sample(&mut rand_rng) as u64);
            }
        }
        _ => unreachable!(),
    }
    keys
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    let num_keys = if quick { 50_000 } else { 500_000 };
    let distributions = ["uniform", "sequential", "clustered", "zipfian"];

    let mut results = serde_json::Map::new();

    for &dist in &distributions {
        let keys = generate_distribution(dist, num_keys, 0x1337_C0DE_CAFE_BABE);

        // 1. Insert Throughput
        let start_exp = Instant::now();
        let mut exp_map = ExpanseMap::new();
        for &k in &keys {
            exp_map.insert(black_box(k), black_box(k));
        }
        let dur_ins_exp = start_exp.elapsed().as_secs_f64();
        let mops_ins_exp = (num_keys as f64 / dur_ins_exp) / 1e6;

        let start_hb = Instant::now();
        let mut hb_map = HashMap::new();
        for &k in &keys {
            hb_map.insert(black_box(k), black_box(k));
        }
        let dur_ins_hb = start_hb.elapsed().as_secs_f64();
        let mops_ins_hb = (num_keys as f64 / dur_ins_hb) / 1e6;

        let start_bt = Instant::now();
        let mut bt_map = BTreeMap::new();
        for &k in &keys {
            bt_map.insert(black_box(k), black_box(k));
        }
        let dur_ins_bt = start_bt.elapsed().as_secs_f64();
        let mops_ins_bt = (num_keys as f64 / dur_ins_bt) / 1e6;

        // 2. Lookup Throughput
        let query_count = if quick { 100_000 } else { 500_000 };
        let start_get_exp = Instant::now();
        for i in 0..query_count {
            let k = keys[i % num_keys];
            black_box(exp_map.get(black_box(k)));
        }
        let mops_get_exp = (query_count as f64 / start_get_exp.elapsed().as_secs_f64()) / 1e6;

        let start_get_hb = Instant::now();
        for i in 0..query_count {
            let k = keys[i % num_keys];
            black_box(hb_map.get(&black_box(k)));
        }
        let mops_get_hb = (query_count as f64 / start_get_hb.elapsed().as_secs_f64()) / 1e6;

        let start_get_bt = Instant::now();
        for i in 0..query_count {
            let k = keys[i % num_keys];
            black_box(bt_map.get(&black_box(k)));
        }
        let mops_get_bt = (query_count as f64 / start_get_bt.elapsed().as_secs_f64()) / 1e6;

        results.insert(dist.to_string(), serde_json::json!({
            "distribution": dist,
            "population": num_keys,
            "insert_mops": {
                "expanse": mops_ins_exp,
                "hashbrown": mops_ins_hb,
                "btree": mops_ins_bt
            },
            "lookup_mops": {
                "expanse": mops_get_exp,
                "hashbrown": mops_get_hb,
                "btree": mops_get_bt
            }
        }));
    }

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    } else {
        println!("{:#?}", results);
    }
}
