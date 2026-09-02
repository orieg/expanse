//! Pillar 5: ART Memory Allocation & Footprint Census
//!
//! Uses a custom GlobalAlloc hook to track exact live heap bytes allocated
//! for ExpanseMap vs blart::TreeMap (Adaptive Radix Tree) vs BTreeMap vs HashMap
//! across population scales: 1k, 10k, 100k, 1M across all key distributions.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `art_memory` |
//! | `group` | 4 |
//! | `population` | 1k to 1M |
//! | `probes_and_reuse` | N/A (Memory) |
//! | `hit_rate` | N/A |
//! | `miss_gen_method` | None |
//! | `value_dereference` | Live bytes tracked |
//! | `measured_region` | Clean GlobalAlloc hook |
//! | `arm_symmetry` | Symmetric keys and PRNG |
//! | `statistics` | Exact byte count |
//! | `verdict` | **PASS** `[verified: CODE READ]`: ART comparison memory footprint benchmark. |

#[path = "art_common/mod.rs"]
mod art_common;

use art_common::{
    ArtMap, BTreeMap, ExpanseMap, HashMap, XorShift64, art_key, gen_clustered, gen_sequential,
    gen_sparse_stride, gen_uniform_random, gen_zipfian,
};
use serde_json::json;
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

struct TrackingAlloc;
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

// SAFETY: Forwards all memory management operations directly to the standard System allocator while recording live allocated bytes.
unsafe impl GlobalAlloc for TrackingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: Delegating directly to the System allocator
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: Delegating directly to the System allocator
        unsafe { System.dealloc(ptr, layout) };
    }
}

#[global_allocator]
static GLOBAL: TrackingAlloc = TrackingAlloc;

fn measure_dist(dist_name: &str, keys: &[u64]) -> serde_json::Value {
    let n = keys.len();

    // 1. ExpanseMap
    let base = LIVE_BYTES.load(Ordering::SeqCst);
    let mut expanse = ExpanseMap::new();
    for &k in keys {
        expanse.insert(k, k.wrapping_mul(3));
    }
    let expanse_bytes = LIVE_BYTES.load(Ordering::SeqCst).saturating_sub(base);
    black_box(&expanse);
    drop(expanse);

    // 2. blart (ART)
    let base = LIVE_BYTES.load(Ordering::SeqCst);
    let mut blart = ArtMap::new();
    for &k in keys {
        let _ = blart.try_insert(art_key(k), k.wrapping_mul(3));
    }
    let blart_bytes = LIVE_BYTES.load(Ordering::SeqCst).saturating_sub(base);
    black_box(&blart);
    drop(blart);

    // 3. BTreeMap
    let base = LIVE_BYTES.load(Ordering::SeqCst);
    let mut btree = BTreeMap::new();
    for &k in keys {
        btree.insert(k, k.wrapping_mul(3));
    }
    let btree_bytes = LIVE_BYTES.load(Ordering::SeqCst).saturating_sub(base);
    black_box(&btree);
    drop(btree);

    // 4. HashMap
    let base = LIVE_BYTES.load(Ordering::SeqCst);
    let mut hash = HashMap::with_capacity(n);
    for &k in keys {
        hash.insert(k, k.wrapping_mul(3));
    }
    let hash_bytes = LIVE_BYTES.load(Ordering::SeqCst).saturating_sub(base);
    black_box(&hash);
    drop(hash);

    let exp_bpk = expanse_bytes as f64 / n as f64;
    let blart_bpk = blart_bytes as f64 / n as f64;
    let btree_bpk = btree_bytes as f64 / n as f64;
    let hash_bpk = hash_bytes as f64 / n as f64;

    json!({
        "distribution": dist_name,
        "population": n,
        "expanse_bytes": expanse_bytes,
        "expanse_bpk": exp_bpk,
        "blart_art_bytes": blart_bytes,
        "blart_art_bpk": blart_bpk,
        "btree_bytes": btree_bytes,
        "btree_bpk": btree_bpk,
        "hashmap_bytes": hash_bytes,
        "hashmap_bpk": hash_bpk,
        "ratio_vs_art": if blart_bpk > 0.0 { exp_bpk / blart_bpk } else { 1.0 },
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    let populations: &[usize] = if quick {
        &[1_000, 10_000, 50_000]
    } else {
        &[1_000, 10_000, 100_000, 1_000_000]
    };

    let mut results = Vec::new();
    let mut rng = XorShift64::new(0);

    for &n in populations {
        let seq = gen_sequential(n);
        results.push(measure_dist("sequential", &seq));

        let clustered = gen_clustered(n, &mut rng);
        results.push(measure_dist("clustered", &clustered));

        let uniform = gen_uniform_random(n, &mut rng);
        results.push(measure_dist("uniform_random", &uniform));

        let sparse = gen_sparse_stride(n);
        results.push(measure_dist("sparse_stride", &sparse));

        let zipf = gen_zipfian(n, 0.99, &mut rng);
        results.push(measure_dist("zipfian", &zipf));
    }

    let output = json!({
        "benchmark": "art_memory",
        "workload_id": "art_memory",
        "quick": quick,
        "results": results,
    });

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("=== ART Benchmark: Memory Footprint Census ===");
        for res in results {
            println!(
                "pop: {:>7} | dist: {:<14} | Expanse: {:>6.2} B/k | blart: {:>6.2} B/k | BTree: {:>6.2} B/k | Hash: {:>6.2} B/k | ratio: {:>4.2}x",
                res["population"],
                res["distribution"].as_str().unwrap(),
                res["expanse_bpk"].as_f64().unwrap(),
                res["blart_art_bpk"].as_f64().unwrap(),
                res["btree_bpk"].as_f64().unwrap(),
                res["hashmap_bpk"].as_f64().unwrap(),
                res["ratio_vs_art"].as_f64().unwrap(),
            );
        }
    }
}
