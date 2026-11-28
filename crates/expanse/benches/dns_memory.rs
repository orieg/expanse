//! DNS-shaped keys: live heap after the build, and the engine's node census.
//!
//! `docs/benchmarks/dns_names/METHODOLOGY.md` §3, operation `dns_memory`, and
//! the exact prediction E1. Each arm is built alone from the same names, and
//! the bytes its build left live are read from a `GlobalAlloc` hook that
//! records the requested `Layout::size` of every allocation: the structure
//! and the keys it owns, nothing else. The Expanse arms also report
//! `mem_used()` and the `stats()` node census, which the suite runner compares
//! with `scripts/dns_name_bounds.py`.
//!
//! Nothing is timed. The counts are exact and identical on every run.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `dns_memory` |
//! | `group` | 4 |
//! | `population` | 1k, 10k, 100k and 1M synthetic names (`dns_common::World`); 1k and 10k under `--quick` |
//! | `insertion_order` | both: `sorted` is ascending key order, `shuffled` a Fisher–Yates permutation from the suite PRNG; one row per order. The Expanse node census does not depend on the order; the allocator census of `BTreeMap` does |
//! | `probes_and_reuse` | None: a census, no probe |
//! | `hit_rate` | n/a (no lookup) |
//! | `miss_gen_method` | n/a (no lookup) |
//! | `value_dereference` | n/a; every key is read back once after the census, outside it |
//! | `measured_region` | Nothing is timed. The byte window opens on an empty structure and closes when the last insert returns; names, keys and the build order exist before it opens and are not counted |
//! | `arm_symmetry` | Identical names, values and build order; `u64` payload on every arm. Each arm is charged the keys it stores: `BTreeMap` and `qp-trie` own one `Vec<u8>` per key, cloned at its exact length; the Expanse arms store key bytes in their nodes |
//! | `statistics` | Exact byte and allocation counts, one per cell, no interval |
//! | `verdict` | **PASS** `[verified: CODE READ]`: requested bytes of the build alone, per arm, in both orders. |

#[path = "dns_common/mod.rs"]
mod dns_common;

use dns_common::{ORDERS, QpTrie, World, cli, emit, nul_free};
use expanse_trie::ExpanseOrderedBytesMap;
use expanse_trie::strmap::{ExpanseStrMap, NulFreeStr};
use serde_json::{Value, json};
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Counts the bytes every live allocation requested, and the live allocations.
struct TrackingAlloc;
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_ALLOCS: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards every operation to the System allocator unchanged and only
// records the requested size of each live allocation.
unsafe impl GlobalAlloc for TrackingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: delegating to the System allocator with the caller's layout.
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
        // SAFETY: `ptr` was returned by `alloc` above with this `layout`.
        unsafe { System.dealloc(ptr, layout) };
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: `ptr` was returned by this allocator with `layout`, and the
        // caller guarantees `new_size` is valid for its alignment.
        let out = unsafe { System.realloc(ptr, layout, new_size) };
        if !out.is_null() {
            LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
            LIVE_BYTES.fetch_add(new_size, Ordering::Relaxed);
        }
        out
    }
}

#[global_allocator]
static GLOBAL: TrackingAlloc = TrackingAlloc;

/// Bytes and allocations `build` left live, and what it built.
fn census<T>(build: impl FnOnce() -> T) -> (T, usize, usize) {
    let (b0, a0) = (
        LIVE_BYTES.load(Ordering::SeqCst),
        LIVE_ALLOCS.load(Ordering::SeqCst),
    );
    let built = build();
    let bytes = LIVE_BYTES.load(Ordering::SeqCst) - b0;
    let allocs = LIVE_ALLOCS.load(Ordering::SeqCst) - a0;
    (built, bytes, allocs)
}

fn row(n: usize, order: &str, arm: &str, bytes: usize, allocs: usize) -> Value {
    json!({
        "population": n,
        "order": order,
        "arm": arm,
        "requested_bytes": bytes,
        "allocations": allocs,
        "bytes_per_name": bytes as f64 / n as f64,
    })
}

fn with_engine(mut r: Value, m: &ExpanseStrMap) -> Value {
    let stats = m.stats();
    let o = r.as_object_mut().expect("row object");
    o.insert("mem_used".into(), json!(m.mem_used()));
    o.insert("str_nodes".into(), json!(stats.str_nodes));
    o.insert("suffix_leaves".into(), json!(stats.suffix_leaves));
    r
}

fn main() {
    let cli = cli();
    let mut rows = Vec::new();
    for &n in &cli.pops {
        let w = World::new(n);
        let stored: usize = w.keys_nul.iter().map(Vec::len).sum();
        for order in ORDERS {
            let build = w.build_order(order);

            let (m, bytes, allocs) = census(|| {
                let mut m = ExpanseOrderedBytesMap::new();
                for &i in &build {
                    m.insert(&w.keys_nul[i], i as u64);
                }
                m
            });
            assert!((0..n).all(|i| m.get(&w.keys_nul[i]) == Some(i as u64)));
            rows.push(with_engine(
                row(n, order, "expanse_ordered", bytes, allocs),
                m.as_inner(),
            ));
            drop(m);

            let (m, bytes, allocs) = census(|| {
                let mut m = ExpanseStrMap::new();
                for &i in &build {
                    let key = NulFreeStr::new(&w.keys_direct[i]).expect("key_direct is NUL-free");
                    m.insert(key, i as u64);
                }
                m
            });
            assert!((0..n).all(|i| m.get(nul_free(&w.keys_direct[i])) == Some(i as u64)));
            rows.push(with_engine(
                row(n, order, "expanse_direct", bytes, allocs),
                &m,
            ));
            drop(m);

            let (m, bytes, allocs) = census(|| {
                let mut m: BTreeMap<Vec<u8>, u64> = BTreeMap::new();
                for &i in &build {
                    m.insert(w.keys_nul[i].clone(), i as u64);
                }
                m
            });
            assert!((0..n).all(|i| m.get(w.keys_nul[i].as_slice()) == Some(&(i as u64))));
            rows.push(row(n, order, "btreemap", bytes, allocs));
            drop(m);

            let (m, bytes, allocs) = census(|| {
                let mut m: QpTrie<Vec<u8>, u64> = QpTrie::new();
                for &i in &build {
                    m.insert(w.keys_nul[i].clone(), i as u64);
                }
                m
            });
            assert!((0..n).all(|i| m.get(&w.keys_nul[i][..]) == Some(&(i as u64))));
            rows.push(row(n, order, "qp_trie", bytes, allocs));
            drop(m);
        }
        for r in rows.iter_mut().rev().take(8) {
            r.as_object_mut()
                .expect("row object")
                .insert("key_nul_bytes".into(), json!(stored));
        }
    }
    emit("dns_memory", &cli, "memory", rows);
}
