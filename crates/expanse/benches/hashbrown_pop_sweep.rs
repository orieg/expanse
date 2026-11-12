//! Pillar 6: log-spaced population sweep against hashbrown (#1694).
//!
//! Point lookup (hit and miss) and live heap bytes per key for ExpanseMap vs
//! hashbrown::HashMap (its default hasher, foldhash) vs
//! std::collections::BTreeMap, from 16 keys to 4M keys with four points per
//! decade, so that a crossover can be located between two adjacent measured
//! populations rather than inferred across a decade. The points include
//! `ROOT_LEAF_CAP` (31, `crates/expanse/src/types.rs`) and its neighbours,
//! below which a map is a single root leaf.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `hashbrown_pop_sweep` |
//! | `group` | 3 |
//! | `population` | `POPS`: 16, 24, 31, 32, 48, 64, then four log-spaced points per decade from 100 to 3,162,278, and 4,194,304 draws (26 points; eight from 16 to 1,000 under `--quick`). `rand22_bswap` draws with repetition, so its large populations hold fewer distinct keys than draws; bytes/key divides by the distinct count |
//! | `insertion_order` | generator draw order — the population is inserted as drawn, neither sorted nor shuffled |
//! | `probes_and_reuse` | 1,048,576 probes per pass (65,536 under `--quick`), drawn uniformly with replacement from the inserted keys by the suite PRNG (`XorShift(0x5EED_5EEB_0000_0001)`), never in draw order; the stream is cycled |
//! | `hit_rate` | 100% on the hit pass (`hit_mops`), 0% on the miss pass (`miss_mops`) |
//! | `miss_gen_method` | Same generator as the population (`KeyGen` of the distribution, seed `0x0BAD_5EED_0000_0002`), resumed at index N and rejected on membership (`miss_keys_from`); `min(N, 65,536)` distinct misses, drawn into the probe stream uniformly with replacement |
//! | `value_dereference` | `black_box(get)` |
//! | `measured_region` | Clean: each arm's build is untimed and its two lookup passes are timed separately; key, probe and miss generation are outside the clock, and the round's maps are dropped after its cell |
//! | `arm_symmetry` | Symmetric: one key set, one probe stream and one miss stream per cell for all three arms; hashbrown uses its default hasher (foldhash) |
//! | `statistics` | `run_all.py` runs one process per round (`--round K`, 9 rounds; 3 under `--quick`), arm order rotated by `K`; it publishes per-arm round means, the rows as `rounds_raw`, BCa 95% intervals and paired per-round ratios per cell. Live heap bytes per key are exact counts from a `GlobalAlloc` hook (requested `Layout::size`), recorded by every round and required identical across them |
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
use workload_common::{KeyGen, XorShift, miss_keys_from};

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

/// The populations: below, at and just above `ROOT_LEAF_CAP` (31), 48 and
/// 64, then four per decade, `round(10^(k/4))`, from 100 to 3,162,278, and
/// 2^22 last.
const POPS: [usize; 26] = [
    16, 24, 31, 32, 48, 64, 100, 178, 316, 562, 1_000, 1_778, 3_162, 5_623, 10_000, 17_783, 31_623,
    56_234, 100_000, 177_828, 316_228, 562_341, 1_000_000, 1_778_279, 3_162_278, 4_194_304,
];
const POPS_QUICK: [usize; 8] = [16, 24, 31, 32, 48, 64, 100, 1_000];

const DISTS: [&str; 3] = ["random", "sequential", "rand22_bswap"];

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

/// `count` draws, uniform with replacement, from `from`.
fn sample(from: &[u64], count: usize, seed: u64) -> Vec<u64> {
    let mut rng = XorShift(seed);
    (0..count)
        .map(|_| from[(rng.next() % from.len() as u64) as usize])
        .collect()
}

/// The three containers behind one interface, so each arm runs the same
/// build and the same two lookup passes.
trait Sweep: Default {
    fn put(&mut self, k: u64);
    fn find(&self, k: u64) -> Option<u64>;
}

impl Sweep for ExpanseMap {
    fn put(&mut self, k: u64) {
        self.insert(k, k);
    }
    #[inline(always)]
    fn find(&self, k: u64) -> Option<u64> {
        self.get(k)
    }
}

impl Sweep for HashMap<u64, u64> {
    fn put(&mut self, k: u64) {
        self.insert(k, k);
    }
    #[inline(always)]
    fn find(&self, k: u64) -> Option<u64> {
        self.get(&k).copied()
    }
}

impl Sweep for BTreeMap<u64, u64> {
    fn put(&mut self, k: u64) {
        self.insert(k, k);
    }
    #[inline(always)]
    fn find(&self, k: u64) -> Option<u64> {
        self.get(&k).copied()
    }
}

/// One timed pass of `probes` against `map`; returns Mops/s.
fn lookup<M: Sweep>(map: &M, probes: &[u64]) -> f64 {
    let start = Instant::now();
    for &k in probes {
        black_box(map.find(black_box(k)));
    }
    (probes.len() as f64 / start.elapsed().as_secs_f64()) / 1e6
}

/// Builds `M` from `keys` (untimed), then times the hit and the miss pass;
/// returns (live heap bytes of the built map, hit Mops/s, miss Mops/s).
fn cell<M: Sweep>(keys: &[u64], hits: &[u64], misses: &[u64]) -> (usize, f64, f64) {
    let before = LIVE_BYTES.load(Ordering::SeqCst);
    let mut m = M::default();
    for &k in keys {
        m.put(k);
    }
    let bytes = LIVE_BYTES.load(Ordering::SeqCst) - before;
    let hit = lookup(&m, hits);
    let miss = lookup(&m, misses);
    drop(m);
    (bytes, hit, miss)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");
    let round = round_arg(&args);

    let pops: &[usize] = if quick { &POPS_QUICK } else { &POPS };
    let probe_count = if quick { 65_536 } else { 1_048_576 };

    let mut cells = Vec::new();
    for &dist in &DISTS {
        for &pop in pops {
            // One load window per cell, read from stderr by
            // `scripts/bench_windowed.py` (AGENTS.md section 8.17, #1214).
            eprintln!("BENCH_WINDOW begin {dist}/{pop}");
            let mut g = KeyGen::new(dist, workload_common::SEED);
            let keys: Vec<u64> = (0..pop).map(|_| g.next()).collect();
            let present: HashSet<u64> = keys.iter().copied().collect();
            let distinct = present.len();
            let mut mg = KeyGen::from_rng(dist, XorShift(0x0BAD_5EED_0000_0002), pop as u64);
            let misses = miss_keys_from(&mut mg, &present, pop.min(65_536));
            drop(present);
            let hit_probes = sample(&keys, probe_count, 0x5EED_5EEB_0000_0001);
            let miss_probes = sample(&misses, probe_count, 0x5EED_5EEB_0000_0002);

            let order: Vec<usize> = (0..ARMS.len()).map(|i| (i + round) % ARMS.len()).collect();
            let mut rows = Vec::new();
            for &arm in &order {
                let (bytes, hit, miss) = match arm {
                    0 => cell::<ExpanseMap>(&keys, &hit_probes, &miss_probes),
                    1 => cell::<HashMap<u64, u64>>(&keys, &hit_probes, &miss_probes),
                    _ => cell::<BTreeMap<u64, u64>>(&keys, &hit_probes, &miss_probes),
                };
                rows.push(serde_json::json!({
                    "round": round,
                    "arm": ARMS[arm],
                    "hit_mops": hit,
                    "miss_mops": miss,
                    "live_bytes": bytes,
                    "bytes_per_key": bytes as f64 / distinct as f64
                }));
            }
            rows.sort_by_key(|r| ARMS.iter().position(|a| r["arm"] == *a));
            cells.push(serde_json::json!({
                "distribution": dist,
                "population": pop,
                "distinct_keys": distinct,
                "rounds_raw": rows
            }));
            eprintln!("BENCH_WINDOW end {dist}/{pop}");
        }
    }

    let out = serde_json::json!({ "probes_per_pass": probe_count, "cells": cells });
    if json_mode {
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        println!("{:#?}", out);
    }
}
