//! Pillar 3: Memory footprint & compression efficiency.
//!
//! Live resident heap bytes per docID for `ExpanseSet` vs `RoaringTreemap`,
//! measured with a custom `GlobalAlloc` hook (the same technique as the
//! hashbrown memory pillar), across the four posting-list shapes plus the
//! multi-tenant shard-ID layout.
//!
//! Two numbers are reported per structure:
//!   * **live heap bits/docID** — the resident cost while the set is in RAM and
//!     queryable (the fair apples-to-apples footprint).
//!   * **serialized bits/docID** — for Roaring only, `serialized_size()`, the
//!     canonical portable-format compression figure the Roaring literature
//!     quotes. `ExpanseSet` has no serialize path, so `mem_used()` (the
//!     engine's own live accounting) is reported alongside the allocator total
//!     as a cross-check.
//!
//! Scope: `roaring-rs` 0.10 has only array and bitmap containers — no run
//! containers. On dense/clustered data CRoaring's run containers would compress
//! further; this suite does not claim a win over CRoaring, only over the
//! pure-Rust `roaring` crate that ships as a dependency here. See METHODOLOGY.
#![allow(missing_docs)]

use expanse_trie::set::ExpanseSet;
use roaring::RoaringTreemap;
use serde_json::json;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

#[path = "search_common/mod.rs"]
mod common;
use common::build_list;

struct TrackingAlloc;
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

// SAFETY: Forwards every allocation to the System allocator while recording the
// net live byte count; the recording is a relaxed atomic and never touches the
// returned pointer.
unsafe impl GlobalAlloc for TrackingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: delegating directly to the System allocator.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: delegating directly to the System allocator.
        unsafe { System.dealloc(ptr, layout) };
    }
}

#[global_allocator]
static GLOBAL: TrackingAlloc = TrackingAlloc;

fn measure(dist: &str, n: usize) -> serde_json::Value {
    let universe = n as u64 * 2;
    let docids = build_list(dist, n, universe, 0);
    let actual = docids.len() as f64;

    // ExpanseSet live heap.
    let before = LIVE_BYTES.load(Ordering::SeqCst);
    let mut set = ExpanseSet::new();
    for &k in &docids {
        set.insert(k);
    }
    let after = LIVE_BYTES.load(Ordering::SeqCst);
    let exp_heap = after.saturating_sub(before);
    let exp_mem_used = set.mem_used();
    drop(set);

    // RoaringTreemap live heap.
    let before = LIVE_BYTES.load(Ordering::SeqCst);
    let mut tree = RoaringTreemap::new();
    for &k in &docids {
        tree.insert(k);
    }
    let after = LIVE_BYTES.load(Ordering::SeqCst);
    let roar_heap = after.saturating_sub(before);
    let roar_serialized = tree.serialized_size();
    drop(tree);

    let bits = |bytes: usize| (bytes as f64 * 8.0) / actual;
    json!({
        "distribution": dist,
        "size": n,
        "actual": docids.len(),
        "expanse_heap_bits_per_docid": bits(exp_heap),
        "expanse_mem_used_bits_per_docid": bits(exp_mem_used),
        "roaring_heap_bits_per_docid": bits(roar_heap),
        "roaring_serialized_bits_per_docid": bits(roar_serialized),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    let sizes = if quick {
        vec![10_000usize, 100_000]
    } else {
        vec![10_000usize, 100_000, 1_000_000]
    };
    let dists = ["dense", "clustered", "sparse", "shard"];

    let mut results = Vec::new();
    for &dist in &dists {
        for &n in &sizes {
            let row = measure(dist, n);
            if !json_mode {
                eprintln!(
                    "  {dist:<10} n={n:>9}  expanse={:>7.3} roaring(heap)={:>7.3} roaring(ser)={:>7.3} bits/docID",
                    row["expanse_heap_bits_per_docid"].as_f64().unwrap(),
                    row["roaring_heap_bits_per_docid"].as_f64().unwrap(),
                    row["roaring_serialized_bits_per_docid"].as_f64().unwrap(),
                );
            }
            results.push(row);
        }
    }

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    } else {
        eprintln!("\n(pass --json to emit machine-readable results)");
    }
}
