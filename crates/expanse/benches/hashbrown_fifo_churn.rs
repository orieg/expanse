//! Pillar 7: constant-size FIFO churn against hashbrown (#1695).
//!
//! Builds N keys, then repeatedly inserts a batch of `B` = 128 fresh keys and
//! removes the 128 oldest, holding the population at N, the shape of the
//! churn workload in Barrett's Judy versus hash-table comparison
//! (<https://www.nothings.org/computer/judy/>). Arms: ExpanseMap,
//! hashbrown::HashMap (its default hasher, foldhash) and
//! std::collections::BTreeMap. Reports time per insert+remove pair and live
//! heap bytes per key before and after the churn, since a tree that removes
//! old keys and adds new ones may keep branch shape the remove path does
//! not condense (`docs/benchmarks/remove_retention/`).
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `hashbrown_fifo_churn` |
//! | `group` | 3 |
//! | `population` | N = 100,000 (below the reference host's 30 MiB LLC for every arm) and N = 4,194,304 (above it for every arm); 10,000 and 100,000 under `--quick`. Held constant through the churn |
//! | `insertion_order` | generator draw order — the build and every batch insert the next keys of one stream as drawn, neither sorted nor shuffled; FIFO removal deletes the oldest `B` keys of that stream, so on `sequential` it deletes a contiguous prefix |
//! | `probes_and_reuse` | No lookups. Untimed warm-up of N/B batches (every built key replaced once), then a timed region of 8,192 batches of B = 128 pairs (1,048,576 pairs; 1,024 batches under `--quick`). Each key is inserted once and removed once; none is reused |
//! | `hit_rate` | N/A (every remove deletes a present key, every insert adds an absent one; both asserted outside the clock on the first batch) |
//! | `miss_gen_method` | N/A: fresh keys are the next draws of the population's own generator (`KeyGen`, `random` and `sequential`), so no insert hits a present key (`random` draws are distinct over the stream; asserted) |
//! | `value_dereference` | `black_box` on every `insert` and `remove` return value |
//! | `measured_region` | Clean: key generation, the build and the warm-up are untimed; the timed region is the 8,192 batches alone, and the map is dropped after the clock stops |
//! | `arm_symmetry` | Symmetric: one key stream per cell for all three arms; hashbrown uses its default hasher (foldhash) |
//! | `statistics` | `run_all.py` runs one process per round (`--round K`, 9 rounds; 3 under `--quick`), arm order rotated by `K`; it publishes per-arm round means of ns per pair, the rows as `rounds_raw`, BCa 95% intervals and paired per-round ratios. Live heap bytes per key are exact counts from a `GlobalAlloc` hook, required identical across rounds |
//! | `verdict` | **PASS** `[verified: CODE READ]`: clean timing; rounds and intervals; memory census checked for identity across rounds. |

use expanse_trie::map::ExpanseMap;
use hashbrown::HashMap;
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::{BTreeMap, HashSet};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

#[path = "workload_common/mod.rs"]
mod workload_common;
use workload_common::KeyGen;

/// Counts the bytes every live allocation requested.
struct TrackingAlloc;
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

// SAFETY: Forwards every operation to the System allocator unchanged and only
// records the requested size of each live allocation.
unsafe impl GlobalAlloc for TrackingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: Delegating directly to the System allocator with the caller's layout.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: `ptr` was returned by `alloc` above with this `layout`.
        unsafe { System.dealloc(ptr, layout) };
    }
}

#[global_allocator]
static GLOBAL: TrackingAlloc = TrackingAlloc;

/// Keys inserted, and removed, per batch.
const B: usize = 128;

const DISTS: [&str; 2] = ["random", "sequential"];

const ARMS: [&str; 3] = ["expanse", "hashbrown", "btree"];

fn round_arg(args: &[String]) -> usize {
    match args.iter().position(|a| a == "--round") {
        Some(i) => args
            .get(i + 1)
            .and_then(|v| v.parse().ok())
            .expect("--round takes a non-negative integer"),
        None => 0,
    }
}

/// The three containers behind one interface, so the churn loop is written
/// once and every arm runs the same sequence of calls.
trait Churn {
    fn put(&mut self, k: u64) -> bool;
    fn take(&mut self, k: u64) -> bool;
    fn size(&self) -> usize;
}

impl Churn for ExpanseMap {
    fn put(&mut self, k: u64) -> bool {
        black_box(self.insert(k, k)).is_none()
    }
    fn take(&mut self, k: u64) -> bool {
        black_box(self.remove(k)).is_some()
    }
    fn size(&self) -> usize {
        usize::try_from(self.len()).expect("a population that fits in memory fits in usize")
    }
}

impl Churn for HashMap<u64, u64> {
    fn put(&mut self, k: u64) -> bool {
        black_box(self.insert(k, k)).is_none()
    }
    fn take(&mut self, k: u64) -> bool {
        black_box(self.remove(&k)).is_some()
    }
    fn size(&self) -> usize {
        self.len()
    }
}

impl Churn for BTreeMap<u64, u64> {
    fn put(&mut self, k: u64) -> bool {
        black_box(self.insert(k, k)).is_none()
    }
    fn take(&mut self, k: u64) -> bool {
        black_box(self.remove(&k)).is_some()
    }
    fn size(&self) -> usize {
        self.len()
    }
}

/// One batch: insert `stream[head + n .. head + n + B]`, then remove
/// `stream[head .. head + B]`, the oldest `B` keys present.
#[inline(always)]
fn batch<M: Churn>(m: &mut M, stream: &[u64], head: usize, n: usize) {
    for &k in &stream[head + n..head + n + B] {
        m.put(black_box(k));
    }
    for &k in &stream[head..head + B] {
        m.take(black_box(k));
    }
}

/// Builds, warms up and times one arm; returns (ns per pair, bytes/key after
/// the build, bytes/key after the timed region).
fn run<M: Churn>(mut m: M, stream: &[u64], n: usize, timed_batches: usize) -> (f64, f64, f64) {
    let before = LIVE_BYTES.load(Ordering::SeqCst);
    for &k in &stream[..n] {
        m.put(k);
    }
    let built = (LIVE_BYTES.load(Ordering::SeqCst) - before) as f64 / n as f64;

    // The first batch, checked: every insert adds an absent key and every
    // remove deletes a present one.
    for &k in &stream[n..n + B] {
        assert!(m.put(k), "a churn insert hit a present key");
    }
    for &k in &stream[..B] {
        assert!(m.take(k), "a churn remove missed its key");
    }
    let mut head = B;
    for _ in 1..n / B {
        batch(&mut m, stream, head, n);
        head += B;
    }
    assert_eq!(m.size(), n, "the population drifted during the warm-up");

    let start = Instant::now();
    for _ in 0..timed_batches {
        batch(&mut m, stream, head, n);
        head += B;
    }
    let ns = start.elapsed().as_secs_f64() * 1e9 / (timed_batches * B) as f64;
    assert_eq!(
        m.size(),
        n,
        "the population drifted during the timed region"
    );
    let after = (LIVE_BYTES.load(Ordering::SeqCst) - before) as f64 / n as f64;
    drop(m);
    (ns, built, after)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");
    let round = round_arg(&args);

    let pops: &[usize] = if quick {
        &[10_000, 100_000]
    } else {
        &[100_000, 4_194_304]
    };
    let timed_batches = if quick { 1_024 } else { 8_192 };

    let mut cells = Vec::new();
    for &dist in &DISTS {
        for &n in pops {
            // The stream: the build, then the warm-up's and the timed
            // region's fresh keys, in draw order.
            let len = n + (n / B) * B + timed_batches * B;
            let mut g = KeyGen::new(dist, workload_common::SEED);
            let stream: Vec<u64> = (0..len).map(|_| g.next()).collect();
            assert_eq!(
                stream.iter().collect::<HashSet<_>>().len(),
                len,
                "{dist}: the churn stream repeats a key"
            );

            eprintln!("BENCH_WINDOW begin {dist}/{n}");
            let order: Vec<usize> = (0..ARMS.len()).map(|i| (i + round) % ARMS.len()).collect();
            let mut rows = Vec::new();
            for &arm in &order {
                let (ns, built, after) = match arm {
                    0 => run(ExpanseMap::new(), &stream, n, timed_batches),
                    1 => run(HashMap::new(), &stream, n, timed_batches),
                    _ => run(BTreeMap::new(), &stream, n, timed_batches),
                };
                rows.push(serde_json::json!({
                    "round": round,
                    "arm": ARMS[arm],
                    "ns_per_pair": ns,
                    "bytes_per_key_built": built,
                    "bytes_per_key_churned": after
                }));
            }
            eprintln!("BENCH_WINDOW end {dist}/{n}");
            rows.sort_by_key(|r| ARMS.iter().position(|a| r["arm"] == *a));
            cells.push(serde_json::json!({
                "distribution": dist,
                "population": n,
                "batch": B,
                "timed_pairs": timed_batches * B,
                "rounds_raw": rows
            }));
        }
    }

    let out = serde_json::json!({ "cells": cells });
    if json_mode {
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        println!("{:#?}", out);
    }
}
