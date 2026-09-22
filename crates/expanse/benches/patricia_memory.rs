//! Patricia trie vs Expanse: live-heap census.
//!
//! Counts the bytes each structure holds on the heap after a cold build, with
//! one GlobalAlloc hook for both arms (requested `Layout::size`; allocator
//! overhead excluded on both). `u64` keys over the five `art_common`
//! distributions, plus shared-prefix path keys (`ExpanseStrMap` vs
//! `PatriciaMap`). The Patricia arm is built in generator order and again in a
//! Fisher–Yates permutation, and both counts are recorded: a Patricia tree's
//! node set is fixed by its key set, so the two should agree, and the harness
//! records whether they do rather than assuming it. The expected Patricia
//! count is computed independently by `scripts/patricia_envelope.py`.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `patricia_memory` |
//! | `group` | 4 |
//! | `population` | 1k to 1M |
//! | `insertion_order` | both — generator draw order (ascending on `sequential` and `sparse_stride`) and a Fisher–Yates permutation under `PROBE_SHUFFLE_SEED`; both recorded per row |
//! | `probes_and_reuse` | N/A (memory) |
//! | `hit_rate` | N/A |
//! | `miss_gen_method` | None |
//! | `value_dereference` | None; live bytes read from the TrackingAlloc counter |
//! | `measured_region` | Build loop only; each structure dropped before the next is built |
//! | `arm_symmetry` | Identical key sets and values, one allocator hook for all arms |
//! | `statistics` | Exact deterministic byte count |
//! | `verdict` | **PENDING** `[unmeasured]`: no committed run yet. |

#[path = "art_common/mod.rs"]
mod art_common;
#[path = "patricia_common/mod.rs"]
mod patricia_common;

use art_common::{
    ExpanseMap, PROBE_SHUFFLE_SEED, SHARED_SEED, XorShift64, dedupe_preserve_order, gen_clustered,
    gen_sequential, gen_sparse_stride, gen_uniform_random, gen_zipfian, shuffle,
};
use expanse_trie::strmap::ExpanseStrMap;
use patricia_common::{PatriciaMap, STRING_SEED, as_nulfree, gen_prefixed_paths, pkey};
use serde_json::json;
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

struct TrackingAlloc;
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_ALLOCS: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards every call to the System allocator unchanged; the counters
// are bookkeeping only and never affect the returned pointer or layout.
unsafe impl GlobalAlloc for TrackingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: caller upholds GlobalAlloc::alloc's contract; delegated as is.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            LIVE_ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        LIVE_ALLOCS.fetch_sub(1, Ordering::Relaxed);
        // SAFETY: `ptr` came from `alloc` above with this `layout`.
        unsafe { System.dealloc(ptr, layout) };
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: caller upholds GlobalAlloc::realloc's contract; delegated as is.
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if !new.is_null() {
            LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
            LIVE_BYTES.fetch_add(new_size, Ordering::Relaxed);
        }
        new
    }
}

#[global_allocator]
static GLOBAL: TrackingAlloc = TrackingAlloc;

fn snapshot() -> (usize, usize) {
    (
        LIVE_BYTES.load(Ordering::SeqCst),
        LIVE_ALLOCS.load(Ordering::SeqCst),
    )
}

fn delta(base: (usize, usize)) -> (usize, usize) {
    let now = snapshot();
    (now.0.saturating_sub(base.0), now.1.saturating_sub(base.1))
}

fn patricia_u64(keys: &[u64]) -> (usize, usize) {
    let base = snapshot();
    let mut p = PatriciaMap::new();
    for &k in keys {
        p.insert(pkey(k), k.wrapping_mul(3));
    }
    let d = delta(base);
    black_box(&p);
    d
}

fn measure_u64(dist: &str, raw: &[u64]) -> serde_json::Value {
    let keys = dedupe_preserve_order(raw);
    let n = keys.len();
    let mut shuffled = keys.clone();
    shuffle(&mut shuffled, &mut XorShift64::new(PROBE_SHUFFLE_SEED));

    let base = snapshot();
    let mut e = ExpanseMap::new();
    for &k in &keys {
        e.insert(k, k.wrapping_mul(3));
    }
    let (e_bytes, e_allocs) = delta(base);
    let e_mem_used = e.mem_used();
    black_box(&e);
    drop(e);

    let (p_bytes, p_allocs) = patricia_u64(&keys);
    let (p_bytes_shuf, p_allocs_shuf) = patricia_u64(&shuffled);

    json!({
        "key_type": "u64",
        "distribution": dist,
        "population": n,
        "raw_draws": raw.len(),
        "expanse_live_bytes": e_bytes,
        "expanse_live_allocs": e_allocs,
        "expanse_mem_used": e_mem_used,
        "expanse_bytes_per_key": e_bytes as f64 / n as f64,
        "patricia_live_bytes": p_bytes,
        "patricia_live_allocs": p_allocs,
        "patricia_bytes_per_key": p_bytes as f64 / n as f64,
        "patricia_live_bytes_shuffled": p_bytes_shuf,
        "patricia_live_allocs_shuffled": p_allocs_shuf,
        "patricia_order_invariant": p_bytes == p_bytes_shuf && p_allocs == p_allocs_shuf,
    })
}

fn measure_paths(n: usize) -> serde_json::Value {
    let mut rng = XorShift64::new(STRING_SEED);
    let keys = gen_prefixed_paths(n, &mut rng);
    let nf = as_nulfree(&keys);

    let base = snapshot();
    let mut e = ExpanseStrMap::new();
    for (i, k) in nf.iter().enumerate() {
        e.insert(k, i as u64);
    }
    let (e_bytes, e_allocs) = delta(base);
    let e_mem_used = e.mem_used();
    black_box(&e);
    drop(e);

    let base = snapshot();
    let mut p = PatriciaMap::new();
    for (i, k) in keys.iter().enumerate() {
        p.insert(k, i as u64);
    }
    let (p_bytes, p_allocs) = delta(base);
    black_box(&p);
    drop(p);

    let key_bytes: usize = keys.iter().map(Vec::len).sum();
    json!({
        "key_type": "prefixed_path",
        "distribution": "prefixed_path",
        "population": n,
        "mean_key_len": key_bytes as f64 / n as f64,
        "expanse_live_bytes": e_bytes,
        "expanse_live_allocs": e_allocs,
        "expanse_mem_used": e_mem_used,
        "expanse_bytes_per_key": e_bytes as f64 / n as f64,
        "patricia_live_bytes": p_bytes,
        "patricia_live_allocs": p_allocs,
        "patricia_bytes_per_key": p_bytes as f64 / n as f64,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");
    let populations: &[usize] = if quick {
        &[1_000, 10_000]
    } else {
        &[1_000, 10_000, 100_000, 1_000_000]
    };

    let mut results = Vec::new();
    let mut rng = XorShift64::new(SHARED_SEED);
    for &n in populations {
        results.push(measure_u64("sequential", &gen_sequential(n)));
        results.push(measure_u64("clustered", &gen_clustered(n, &mut rng)));
        results.push(measure_u64(
            "uniform_random",
            &gen_uniform_random(n, &mut rng),
        ));
        results.push(measure_u64("sparse_stride", &gen_sparse_stride(n)));
        results.push(measure_u64("zipfian", &gen_zipfian(n, 0.99, &mut rng)));
        results.push(measure_paths(n));
    }

    let output = json!({
        "benchmark": "patricia_memory",
        "workload_id": "patricia_memory",
        "competitor": "patricia_tree 0.10.2",
        "quick": quick,
        "results": results,
    });
    if json_mode {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("=== patricia_memory (quick={quick}) ===");
        for r in &results {
            println!(
                "  pop={:8} | {:15} | Expanse {:7.2} B/key | Patricia {:7.2} B/key | order-invariant {}",
                r["population"],
                r["distribution"].as_str().unwrap(),
                r["expanse_bytes_per_key"].as_f64().unwrap(),
                r["patricia_bytes_per_key"].as_f64().unwrap(),
                r.get("patricia_order_invariant")
                    .map_or("n/a".to_string(), |v| v.to_string()),
            );
        }
    }
}
