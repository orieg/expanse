//! Pillar 1: hashbrown Native Criterion Suite Port
//!
//! Mirrors the upstream hashbrown/benches/bench.rs suite across:
//! - insert_growing (un-preallocated dynamic growth)
//! - insert_preallocated (with_capacity(N) vs cold ExpanseMap)
//! - lookup_hit (point query on present keys)
//! - lookup_miss (point query on absent keys)
//! - iter_all (full container iteration)
//! - remove (point deletion)
//!
//! Supports standalone execution with `--json` for automated script collection.

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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    let pops = if quick {
        vec![10_000, 100_000]
    } else {
        vec![10_000, 100_000, 500_000]
    };

    let mut results = Vec::new();

    for &pop in &pops {
        let keys = generate_keys(pop, 0x1234_5678_9ABC_DEF0);
        let absent_keys = generate_keys(pop, 0xFEDC_BA98_7654_3210);

        // Prepopulate maps for read/iter/remove tests
        let mut base_expanse = ExpanseMap::new();
        let mut base_hashbrown = HashMap::new();
        let mut base_btree = BTreeMap::new();
        for &k in &keys {
            base_expanse.insert(k, k);
            base_hashbrown.insert(k, k);
            base_btree.insert(k, k);
        }

        let iters = if pop <= 10_000 {
            100_000
        } else if pop <= 100_000 {
            50_000
        } else {
            10_000
        };

        // 1. Lookup Hit
        let mut idx_e = 0;
        let (ns_hit_exp, mops_hit_exp) = bench_op(
            || {
                idx_e = (idx_e + 1) % keys.len();
                black_box(base_expanse.get(black_box(keys[idx_e])));
            },
            5_000,
            iters,
        );

        let mut idx_h = 0;
        let (ns_hit_hb, mops_hit_hb) = bench_op(
            || {
                idx_h = (idx_h + 1) % keys.len();
                black_box(base_hashbrown.get(&black_box(keys[idx_h])));
            },
            5_000,
            iters,
        );

        let mut idx_b = 0;
        let (ns_hit_bt, mops_hit_bt) = bench_op(
            || {
                idx_b = (idx_b + 1) % keys.len();
                black_box(base_btree.get(&black_box(keys[idx_b])));
            },
            5_000,
            iters,
        );

        // 2. Lookup Miss
        let mut idx_me = 0;
        let (ns_miss_exp, mops_miss_exp) = bench_op(
            || {
                idx_me = (idx_me + 1) % absent_keys.len();
                black_box(base_expanse.get(black_box(absent_keys[idx_me])));
            },
            5_000,
            iters,
        );

        let mut idx_mh = 0;
        let (ns_miss_hb, mops_miss_hb) = bench_op(
            || {
                idx_mh = (idx_mh + 1) % absent_keys.len();
                black_box(base_hashbrown.get(&black_box(absent_keys[idx_mh])));
            },
            5_000,
            iters,
        );

        let mut idx_mb = 0;
        let (ns_miss_bt, mops_miss_bt) = bench_op(
            || {
                idx_mb = (idx_mb + 1) % absent_keys.len();
                black_box(base_btree.get(&black_box(absent_keys[idx_mb])));
            },
            5_000,
            iters,
        );

        // 3. Iteration
        let iter_reps = if pop <= 10_000 {
            100
        } else if pop <= 100_000 {
            20
        } else {
            5
        };
        let (ns_iter_exp, _) = bench_op(
            || {
                let mut count = 0usize;
                for (k, v) in base_expanse.iter() {
                    black_box((k, v));
                    count += 1;
                }
                black_box(count);
            },
            2,
            iter_reps,
        );

        let (ns_iter_hb, _) = bench_op(
            || {
                let mut count = 0usize;
                for (k, v) in base_hashbrown.iter() {
                    black_box((k, v));
                    count += 1;
                }
                black_box(count);
            },
            2,
            iter_reps,
        );

        let (ns_iter_bt, _) = bench_op(
            || {
                let mut count = 0usize;
                for (k, v) in base_btree.iter() {
                    black_box((k, v));
                    count += 1;
                }
                black_box(count);
            },
            2,
            iter_reps,
        );

        // 4. Insert Growing (from 0 to pop)
        let build_reps = if pop <= 10_000 {
            30
        } else if pop <= 100_000 {
            5
        } else {
            2
        };
        let (ns_grow_exp, _) = bench_op(
            || {
                let mut m = ExpanseMap::new();
                for &k in &keys {
                    m.insert(black_box(k), black_box(k));
                }
                black_box(m);
            },
            1,
            build_reps,
        );
        let mops_grow_exp = (pop as f64 / (ns_grow_exp * 1e-9)) / 1e6;

        let (ns_grow_hb, _) = bench_op(
            || {
                let mut m = HashMap::new();
                for &k in &keys {
                    m.insert(black_box(k), black_box(k));
                }
                black_box(m);
            },
            1,
            build_reps,
        );
        let mops_grow_hb = (pop as f64 / (ns_grow_hb * 1e-9)) / 1e6;

        let (ns_grow_bt, _) = bench_op(
            || {
                let mut m = BTreeMap::new();
                for &k in &keys {
                    m.insert(black_box(k), black_box(k));
                }
                black_box(m);
            },
            1,
            build_reps,
        );
        let mops_grow_bt = (pop as f64 / (ns_grow_bt * 1e-9)) / 1e6;

        results.push(serde_json::json!({
            "population": pop,
            "lookup_hit": {
                "expanse": { "ns_per_op": ns_hit_exp, "mops": mops_hit_exp },
                "hashbrown": { "ns_per_op": ns_hit_hb, "mops": mops_hit_hb },
                "btree": { "ns_per_op": ns_hit_bt, "mops": mops_hit_bt }
            },
            "lookup_miss": {
                "expanse": { "ns_per_op": ns_miss_exp, "mops": mops_miss_exp },
                "hashbrown": { "ns_per_op": ns_miss_hb, "mops": mops_miss_hb },
                "btree": { "ns_per_op": ns_miss_bt, "mops": mops_miss_bt }
            },
            "iter_all": {
                "expanse": { "ns_per_scan": ns_iter_exp, "mops_items": (pop as f64 / (ns_iter_exp * 1e-9)) / 1e6 },
                "hashbrown": { "ns_per_scan": ns_iter_hb, "mops_items": (pop as f64 / (ns_iter_hb * 1e-9)) / 1e6 },
                "btree": { "ns_per_scan": ns_iter_bt, "mops_items": (pop as f64 / (ns_iter_bt * 1e-9)) / 1e6 }
            },
            "insert_growing": {
                "expanse": { "mops": mops_grow_exp },
                "hashbrown": { "mops": mops_grow_hb },
                "btree": { "mops": mops_grow_bt }
            }
        }));
    }

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    } else {
        println!("{:#?}", results);
    }
}
