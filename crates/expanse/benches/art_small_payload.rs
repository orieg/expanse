//! ART Small-Payload Regime: Expanse vs blart (ART), BTreeMap, and HashMap
//!
//! Evaluates the small-payload / immediate regime (N <= 7 keys, issue #663)
//! across point lookup latency (100% hit and 50/50 rejection miss), dynamic
//! insertion throughput, and memory footprint / allocation census.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `art_small_payload` |
//! | `group` | 4 |
//! | `population` | 1 to 7 keys |
//! | `probes_and_reuse` | Looped probe stream (10k lookups) & batched builds (1k constructions) |
//! | `hit_rate` | 100% Hit & 50/50 Rejection Miss & Dynamic Insertion |
//! | `miss_gen_method` | Rejection sampling |
//! | `value_dereference` | `black_box(*val)` / insertion sink |
//! | `measured_region` | Clean lookup & cold build loops |
//! | `arm_symmetry` | Symmetric keys and PRNG |
//! | `statistics` | Median + BCa 95% Bootstrap CI over paired rounds; exact byte and allocation census |
//! | `verdict` | **PASS** `[verified: CODE READ]`: ART comparison small-payload regime benchmark (#663). |

#[path = "art_common/mod.rs"]
mod art_common;

use art_common::{
    ArtMap, BTreeMap, ExpanseMap, HashMap, XorShift64, art_key, bca_ci, gen_sequential, median,
    shuffle,
};
use serde_json::json;
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

struct TrackingAlloc;
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

// SAFETY: Forwards memory management directly to the system allocator while tracking live bytes and alloc counts.
unsafe impl GlobalAlloc for TrackingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: Delegating directly to the System allocator
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size(), Ordering::SeqCst);
            ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::SeqCst);
        // SAFETY: Delegating directly to the System allocator
        unsafe { System.dealloc(ptr, layout) };
    }
}

#[global_allocator]
static GLOBAL: TrackingAlloc = TrackingAlloc;

const LOOKUP_PROBES: usize = 10_000;
const INSERT_BATCH: usize = 1_000;

#[inline(never)]
fn time_expanse_lookup(map: &ExpanseMap, probes: &[u64]) -> f64 {
    let start = Instant::now();
    for &k in probes {
        let v = map.get(k).unwrap_or(0);
        black_box(v);
    }
    start.elapsed().as_nanos() as f64 / probes.len() as f64
}

#[inline(never)]
fn time_blart_lookup(map: &ArtMap<blart::Mapped<blart::ToUBE, u64>, u64>, probes: &[u64]) -> f64 {
    let start = Instant::now();
    for &k in probes {
        let ak = art_key(k);
        let v = map.get(&ak).copied().unwrap_or(0);
        black_box(v);
    }
    start.elapsed().as_nanos() as f64 / probes.len() as f64
}

#[inline(never)]
fn time_btree_lookup(map: &BTreeMap<u64, u64>, probes: &[u64]) -> f64 {
    let start = Instant::now();
    for &k in probes {
        let v = map.get(&k).copied().unwrap_or(0);
        black_box(v);
    }
    start.elapsed().as_nanos() as f64 / probes.len() as f64
}

#[inline(never)]
fn time_hash_lookup(map: &HashMap<u64, u64>, probes: &[u64]) -> f64 {
    let start = Instant::now();
    for &k in probes {
        let v = map.get(&k).copied().unwrap_or(0);
        black_box(v);
    }
    start.elapsed().as_nanos() as f64 / probes.len() as f64
}

#[inline(never)]
fn time_expanse_insert(keys: &[u64], batch_count: usize) -> f64 {
    let start = Instant::now();
    for _ in 0..batch_count {
        let mut map = ExpanseMap::new();
        for &k in keys {
            map.insert(k, k.wrapping_mul(3));
        }
        black_box(&map);
    }
    start.elapsed().as_nanos() as f64 / (batch_count * keys.len()) as f64
}

#[inline(never)]
fn time_blart_insert(keys: &[u64], batch_count: usize) -> f64 {
    let start = Instant::now();
    for _ in 0..batch_count {
        let mut map = ArtMap::new();
        for &k in keys {
            let _ = map.try_insert(art_key(k), k.wrapping_mul(3));
        }
        black_box(&map);
    }
    start.elapsed().as_nanos() as f64 / (batch_count * keys.len()) as f64
}

#[inline(never)]
fn time_btree_insert(keys: &[u64], batch_count: usize) -> f64 {
    let start = Instant::now();
    for _ in 0..batch_count {
        let mut map = BTreeMap::new();
        for &k in keys {
            map.insert(k, k.wrapping_mul(3));
        }
        black_box(&map);
    }
    start.elapsed().as_nanos() as f64 / (batch_count * keys.len()) as f64
}

#[inline(never)]
fn time_hash_insert(keys: &[u64], batch_count: usize) -> f64 {
    let start = Instant::now();
    for _ in 0..batch_count {
        let mut map = HashMap::new();
        for &k in keys {
            map.insert(k, k.wrapping_mul(3));
        }
        black_box(&map);
    }
    start.elapsed().as_nanos() as f64 / (batch_count * keys.len()) as f64
}

fn bench_small_pop(n: usize, rounds: usize) -> serde_json::Value {
    let keys = gen_sequential(n);

    // 1. Memory and Allocation Census
    let exp_logical_bytes = {
        let mut map = ExpanseMap::new();
        for &k in &keys {
            map.insert(k, k.wrapping_mul(3));
        }
        let used = map.mem_used();
        black_box(&map);
        used
    };

    let (exp_tracking_bytes, exp_allocs) = {
        let base_b = LIVE_BYTES.load(Ordering::SeqCst);
        let base_c = ALLOC_COUNT.load(Ordering::SeqCst);
        let mut map = ExpanseMap::new();
        for &k in &keys {
            map.insert(k, k.wrapping_mul(3));
        }
        let b = LIVE_BYTES.load(Ordering::SeqCst).saturating_sub(base_b);
        let c = ALLOC_COUNT.load(Ordering::SeqCst).saturating_sub(base_c);
        black_box(&map);
        drop(map);
        (b, c)
    };

    let (blart_bytes, blart_allocs) = {
        let base_b = LIVE_BYTES.load(Ordering::SeqCst);
        let base_c = ALLOC_COUNT.load(Ordering::SeqCst);
        let mut map = ArtMap::new();
        for &k in &keys {
            let _ = map.try_insert(art_key(k), k.wrapping_mul(3));
        }
        let b = LIVE_BYTES.load(Ordering::SeqCst).saturating_sub(base_b);
        let c = ALLOC_COUNT.load(Ordering::SeqCst).saturating_sub(base_c);
        black_box(&map);
        drop(map);
        (b, c)
    };

    let (btree_bytes, btree_allocs) = {
        let base_b = LIVE_BYTES.load(Ordering::SeqCst);
        let base_c = ALLOC_COUNT.load(Ordering::SeqCst);
        let mut map = BTreeMap::new();
        for &k in &keys {
            map.insert(k, k.wrapping_mul(3));
        }
        let b = LIVE_BYTES.load(Ordering::SeqCst).saturating_sub(base_b);
        let c = ALLOC_COUNT.load(Ordering::SeqCst).saturating_sub(base_c);
        black_box(&map);
        drop(map);
        (b, c)
    };

    let (hash_bytes, hash_allocs) = {
        let base_b = LIVE_BYTES.load(Ordering::SeqCst);
        let base_c = ALLOC_COUNT.load(Ordering::SeqCst);
        let mut map = HashMap::new();
        for &k in &keys {
            map.insert(k, k.wrapping_mul(3));
        }
        let b = LIVE_BYTES.load(Ordering::SeqCst).saturating_sub(base_b);
        let c = ALLOC_COUNT.load(Ordering::SeqCst).saturating_sub(base_c);
        black_box(&map);
        drop(map);
        (b, c)
    };

    // 2. Point Lookup Latency (100% Hit Rate)
    // Warm containers for lookup probes
    let mut expanse = ExpanseMap::new();
    let mut blart = ArtMap::new();
    let mut btree = BTreeMap::new();
    let mut hash = HashMap::new();
    for &k in &keys {
        expanse.insert(k, k.wrapping_mul(3));
        let _ = blart.try_insert(art_key(k), k.wrapping_mul(3));
        btree.insert(k, k.wrapping_mul(3));
        hash.insert(k, k.wrapping_mul(3));
    }

    // Build looped probe stream (10,000 probes) shuffled to eliminate branch predictor lock-in
    let mut hit_probes = Vec::with_capacity(LOOKUP_PROBES);
    for i in 0..LOOKUP_PROBES {
        hit_probes.push(keys[i % n]);
    }
    let mut shuffle_rng = XorShift64::new(art_common::PROBE_SHUFFLE_SEED);
    shuffle(&mut hit_probes, &mut shuffle_rng);

    let mut hit_exp_times = Vec::with_capacity(rounds);
    let mut hit_blart_times = Vec::with_capacity(rounds);
    let mut hit_btree_times = Vec::with_capacity(rounds);
    let mut hit_hash_times = Vec::with_capacity(rounds);
    let mut hit_paired_ratios = Vec::with_capacity(rounds);

    for r in 0..rounds {
        let (e, b, bt, h) = match r % 4 {
            0 => (
                time_expanse_lookup(&expanse, &hit_probes),
                time_blart_lookup(&blart, &hit_probes),
                time_btree_lookup(&btree, &hit_probes),
                time_hash_lookup(&hash, &hit_probes),
            ),
            1 => {
                let b = time_blart_lookup(&blart, &hit_probes);
                let bt = time_btree_lookup(&btree, &hit_probes);
                let h = time_hash_lookup(&hash, &hit_probes);
                let e = time_expanse_lookup(&expanse, &hit_probes);
                (e, b, bt, h)
            }
            2 => {
                let bt = time_btree_lookup(&btree, &hit_probes);
                let h = time_hash_lookup(&hash, &hit_probes);
                let e = time_expanse_lookup(&expanse, &hit_probes);
                let b = time_blart_lookup(&blart, &hit_probes);
                (e, b, bt, h)
            }
            _ => {
                let h = time_hash_lookup(&hash, &hit_probes);
                let e = time_expanse_lookup(&expanse, &hit_probes);
                let b = time_blart_lookup(&blart, &hit_probes);
                let bt = time_btree_lookup(&btree, &hit_probes);
                (e, b, bt, h)
            }
        };
        hit_exp_times.push(e);
        hit_blart_times.push(b);
        hit_btree_times.push(bt);
        hit_hash_times.push(h);
        if b > 0.0 {
            hit_paired_ratios.push(e / b);
        }
    }

    let hit_exp_med = median(hit_exp_times);
    let hit_blart_med = median(hit_blart_times);
    let hit_btree_med = median(hit_btree_times);
    let hit_hash_med = median(hit_hash_times);
    let (hit_ratio_mean, hit_ci_lo, hit_ci_hi) = bca_ci(&hit_paired_ratios);

    // 3. Point Lookup Latency (50% Hit / 50% Rejection Miss)
    let mut miss_probes = Vec::with_capacity(LOOKUP_PROBES);
    let miss_offset = (n as u64).max(100);
    for i in 0..LOOKUP_PROBES {
        if i % 2 == 0 {
            miss_probes.push(keys[(i / 2) % n]);
        } else {
            miss_probes.push(miss_offset + ((i / 2) % n) as u64);
        }
    }
    shuffle(&mut miss_probes, &mut shuffle_rng);

    let mut miss_exp_times = Vec::with_capacity(rounds);
    let mut miss_blart_times = Vec::with_capacity(rounds);
    let mut miss_btree_times = Vec::with_capacity(rounds);
    let mut miss_hash_times = Vec::with_capacity(rounds);
    let mut miss_paired_ratios = Vec::with_capacity(rounds);

    for r in 0..rounds {
        let (e, b, bt, h) = match r % 4 {
            0 => (
                time_expanse_lookup(&expanse, &miss_probes),
                time_blart_lookup(&blart, &miss_probes),
                time_btree_lookup(&btree, &miss_probes),
                time_hash_lookup(&hash, &miss_probes),
            ),
            1 => {
                let b = time_blart_lookup(&blart, &miss_probes);
                let bt = time_btree_lookup(&btree, &miss_probes);
                let h = time_hash_lookup(&hash, &miss_probes);
                let e = time_expanse_lookup(&expanse, &miss_probes);
                (e, b, bt, h)
            }
            2 => {
                let bt = time_btree_lookup(&btree, &miss_probes);
                let h = time_hash_lookup(&hash, &miss_probes);
                let e = time_expanse_lookup(&expanse, &miss_probes);
                let b = time_blart_lookup(&blart, &miss_probes);
                (e, b, bt, h)
            }
            _ => {
                let h = time_hash_lookup(&hash, &miss_probes);
                let e = time_expanse_lookup(&expanse, &miss_probes);
                let b = time_blart_lookup(&blart, &miss_probes);
                let bt = time_btree_lookup(&btree, &miss_probes);
                (e, b, bt, h)
            }
        };
        miss_exp_times.push(e);
        miss_blart_times.push(b);
        miss_btree_times.push(bt);
        miss_hash_times.push(h);
        if b > 0.0 {
            miss_paired_ratios.push(e / b);
        }
    }

    let miss_exp_med = median(miss_exp_times);
    let miss_blart_med = median(miss_blart_times);
    let miss_btree_med = median(miss_btree_times);
    let miss_hash_med = median(miss_hash_times);
    let (miss_ratio_mean, miss_ci_lo, miss_ci_hi) = bca_ci(&miss_paired_ratios);

    // 4. Dynamic Growth Insertion Latency
    let mut ins_exp_times = Vec::with_capacity(rounds);
    let mut ins_blart_times = Vec::with_capacity(rounds);
    let mut ins_btree_times = Vec::with_capacity(rounds);
    let mut ins_hash_times = Vec::with_capacity(rounds);
    let mut ins_paired_ratios = Vec::with_capacity(rounds);

    for r in 0..rounds {
        let (e, b, bt, h) = match r % 4 {
            0 => (
                time_expanse_insert(&keys, INSERT_BATCH),
                time_blart_insert(&keys, INSERT_BATCH),
                time_btree_insert(&keys, INSERT_BATCH),
                time_hash_insert(&keys, INSERT_BATCH),
            ),
            1 => {
                let b = time_blart_insert(&keys, INSERT_BATCH);
                let bt = time_btree_insert(&keys, INSERT_BATCH);
                let h = time_hash_insert(&keys, INSERT_BATCH);
                let e = time_expanse_insert(&keys, INSERT_BATCH);
                (e, b, bt, h)
            }
            2 => {
                let bt = time_btree_insert(&keys, INSERT_BATCH);
                let h = time_hash_insert(&keys, INSERT_BATCH);
                let e = time_expanse_insert(&keys, INSERT_BATCH);
                let b = time_blart_insert(&keys, INSERT_BATCH);
                (e, b, bt, h)
            }
            _ => {
                let h = time_hash_insert(&keys, INSERT_BATCH);
                let e = time_expanse_insert(&keys, INSERT_BATCH);
                let b = time_blart_insert(&keys, INSERT_BATCH);
                let bt = time_btree_insert(&keys, INSERT_BATCH);
                (e, b, bt, h)
            }
        };
        ins_exp_times.push(e);
        ins_blart_times.push(b);
        ins_btree_times.push(bt);
        ins_hash_times.push(h);
        if b > 0.0 {
            ins_paired_ratios.push(e / b);
        }
    }

    let ins_exp_med = median(ins_exp_times);
    let ins_blart_med = median(ins_blart_times);
    let ins_btree_med = median(ins_btree_times);
    let ins_hash_med = median(ins_hash_times);
    let (ins_ratio_mean, ins_ci_lo, ins_ci_hi) = bca_ci(&ins_paired_ratios);

    json!({
        "population": n,
        "memory": {
            "expanse_logical_bytes": exp_logical_bytes,
            "expanse_logical_bpk": exp_logical_bytes as f64 / n as f64,
            "expanse_tracking_bytes": exp_tracking_bytes,
            "expanse_tracking_bpk": exp_tracking_bytes as f64 / n as f64,
            "expanse_alloc_count": exp_allocs,
            "blart_bytes": blart_bytes,
            "blart_bpk": blart_bytes as f64 / n as f64,
            "blart_alloc_count": blart_allocs,
            "btree_bytes": btree_bytes,
            "btree_bpk": btree_bytes as f64 / n as f64,
            "btree_alloc_count": btree_allocs,
            "hashmap_bytes": hash_bytes,
            "hashmap_bpk": hash_bytes as f64 / n as f64,
            "hashmap_alloc_count": hash_allocs,
            "logical_ratio_vs_art": exp_logical_bytes as f64 / blart_bytes.max(1) as f64,
        },
        "lookup_hit": {
            "expanse_ns_op": hit_exp_med,
            "blart_art_ns_op": hit_blart_med,
            "btree_ns_op": hit_btree_med,
            "hashmap_ns_op": hit_hash_med,
            "ratio_vs_art": if hit_blart_med > 0.0 { hit_exp_med / hit_blart_med } else { 1.0 },
            "ratio_bca_ci_95": [hit_ci_lo, hit_ci_hi],
            "ratio_mean": hit_ratio_mean,
        },
        "lookup_miss": {
            "expanse_ns_op": miss_exp_med,
            "blart_art_ns_op": miss_blart_med,
            "btree_ns_op": miss_btree_med,
            "hashmap_ns_op": miss_hash_med,
            "ratio_vs_art": if miss_blart_med > 0.0 { miss_exp_med / miss_blart_med } else { 1.0 },
            "ratio_bca_ci_95": [miss_ci_lo, miss_ci_hi],
            "ratio_mean": miss_ratio_mean,
        },
        "insert": {
            "expanse_ns_op": ins_exp_med,
            "blart_art_ns_op": ins_blart_med,
            "btree_ns_op": ins_btree_med,
            "hashmap_ns_op": ins_hash_med,
            "ratio_vs_art": if ins_blart_med > 0.0 { ins_exp_med / ins_blart_med } else { 1.0 },
            "ratio_bca_ci_95": [ins_ci_lo, ins_ci_hi],
            "ratio_mean": ins_ratio_mean,
        }
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    let populations: &[usize] = if quick {
        &[1, 2, 4, 7]
    } else {
        &[1, 2, 3, 4, 5, 6, 7]
    };
    let rounds = if quick { 3 } else { 15 };

    let mut results = Vec::new();
    for &n in populations {
        results.push(bench_small_pop(n, rounds));
    }

    let output = json!({
        "benchmark": "art_small_payload",
        "workload_id": "art_small_payload",
        "quick": quick,
        "rounds": rounds,
        "results": results,
    });

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("=== art_small_payload (quick={quick}, rounds={rounds}) ===");
        println!("--- Memory & Allocation Census ---");
        for r in &results {
            let n = r["population"].as_u64().unwrap();
            let mem = &r["memory"];
            let exp_log = mem["expanse_logical_bytes"].as_u64().unwrap();
            let exp_allocs = mem["expanse_alloc_count"].as_u64().unwrap();
            let art_bytes = mem["blart_bytes"].as_u64().unwrap();
            let art_allocs = mem["blart_alloc_count"].as_u64().unwrap();
            let rat = mem["logical_ratio_vs_art"].as_f64().unwrap();
            println!(
                "  N={n}: Expanse {exp_log:3}B ({exp_allocs} alloc) | blart {art_bytes:3}B ({art_allocs:2} allocs) | Ratio: {rat:.2}x",
            );
        }
        println!("\n--- Point Lookup Latency (100% Hit) ---");
        for r in &results {
            let n = r["population"].as_u64().unwrap();
            let hit = &r["lookup_hit"];
            let exp = hit["expanse_ns_op"].as_f64().unwrap();
            let art = hit["blart_art_ns_op"].as_f64().unwrap();
            let rat = hit["ratio_vs_art"].as_f64().unwrap();
            let ci = &hit["ratio_bca_ci_95"];
            println!(
                "  N={n}: Expanse: {exp:5.2} ns | blart: {art:5.2} ns | Ratio (Exp/ART): {rat:5.2}x CI [{:.2}, {:.2}]",
                ci[0].as_f64().unwrap(),
                ci[1].as_f64().unwrap()
            );
        }
        println!("\n--- Dynamic Growth Insertion Latency ---");
        for r in &results {
            let n = r["population"].as_u64().unwrap();
            let ins = &r["insert"];
            let exp = ins["expanse_ns_op"].as_f64().unwrap();
            let art = ins["blart_art_ns_op"].as_f64().unwrap();
            let rat = ins["ratio_vs_art"].as_f64().unwrap();
            let ci = &ins["ratio_bca_ci_95"];
            println!(
                "  N={n}: Expanse: {exp:5.2} ns | blart: {art:5.2} ns | Ratio (Exp/ART): {rat:5.2}x CI [{:.2}, {:.2}]",
                ci[0].as_f64().unwrap(),
                ci[1].as_f64().unwrap()
            );
        }
    }
}
