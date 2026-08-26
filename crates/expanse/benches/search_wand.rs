//! Pillar 2: WAND dynamic skip-scan.
//!
//! Block-max WAND and MaxScore evaluators do not read every posting: they
//! advance the current cursor to the next candidate `>= target` and skip the
//! gaps. This harness measures that primitive on a monotonically increasing
//! target sequence:
//!
//!   * **`ExpanseSet::next_at_or_after(target)`** — a stateless O(depth)
//!     re-descent from the trie root on every call.
//!   * **`RoaringTreemap::iter().advance_to(target)`** — a stateful cursor
//!     that advances forward from its current position.
//!
//! The architectural trade this exposes: when targets are dense (small
//! strides, near a full sweep) Roaring's incremental cursor wins because it
//! never re-descends; when targets are sparse (large strides, deep skips over
//! a big list) Expanse's fixed O(depth) skip is expected to win because it does
//! not scan the skipped blocks. Both regimes are measured below.
#![allow(missing_docs)]

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use roaring::RoaringTreemap;
use serde_json::json;
use std::hint::black_box;
use std::time::Duration;

#[path = "search_common/mod.rs"]
mod common;
use common::{
    build_list, expanse_skipscan, median_ns_per_op, roaring_skipscan, to_expanse, to_roaring,
};

/// A monotonically increasing target sequence with mean stride `avg_stride`,
/// spanning `[lo, hi]`.
fn make_targets(lo: u64, hi: u64, avg_stride: u64, seed: u64) -> Vec<u64> {
    let mut rng = StdRng::seed_from_u64(0x3A2D ^ seed);
    let mut out = Vec::new();
    let mut t = lo;
    let span = avg_stride.max(1);
    while t <= hi {
        out.push(t);
        // stride uniform in [1, 2*avg] => mean ~= avg_stride.
        t = t.saturating_add(rng.gen_range(1..=2 * span));
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    let (sizes, batches, min_batch) = if quick {
        (vec![100_000usize], 3, Duration::from_millis(30))
    } else {
        (
            vec![1_000_000usize, 10_000_000],
            5,
            Duration::from_millis(60),
        )
    };
    let list_dists = ["dense", "clustered", "sparse"];

    let mut results = Vec::new();

    for &dist in &list_dists {
        for &n in &sizes {
            let universe = n as u64 * 2;
            let list = build_list(dist, n, universe, 0);
            let lo = *list.first().unwrap();
            let hi = *list.last().unwrap();
            let set = to_expanse(&list);
            let tree: RoaringTreemap = to_roaring(&list);

            // Regimes by mean stride relative to the docID span.
            let span = hi - lo;
            let regimes = [
                ("shallow", 1u64),                // near-full sweep
                ("medium", 64),                   // moderate skips
                ("deep", (span / 2000).max(256)), // few, long-range skips
            ];

            for (regime, avg_stride) in regimes {
                let targets = make_targets(lo, hi, avg_stride, n as u64);
                // sanity: identical results from both cursors.
                debug_assert_eq!(
                    expanse_skipscan(&set, &targets),
                    roaring_skipscan(&tree, &targets)
                );
                let t = targets.len().max(1) as f64;
                let e = median_ns_per_op(
                    || black_box(expanse_skipscan(&set, &targets)),
                    batches,
                    min_batch,
                ) / t;
                let r = median_ns_per_op(
                    || black_box(roaring_skipscan(&tree, &targets)),
                    batches,
                    min_batch,
                ) / t;

                results.push(json!({
                    "list_dist": dist,
                    "size": n,
                    "regime": regime,
                    "avg_stride": avg_stride,
                    "skips": targets.len(),
                    "expanse_ns_per_skip": e,
                    "roaring_ns_per_skip": r,
                }));
                if !json_mode {
                    eprintln!(
                        "  {dist:<10} n={n:>9} {regime:<8} skips={:>9}  exp={e:>8.2}ns roar={r:>8.2}ns",
                        targets.len()
                    );
                }
            }
        }
    }

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    } else {
        eprintln!("\n(pass --json to emit machine-readable results)");
    }
}
