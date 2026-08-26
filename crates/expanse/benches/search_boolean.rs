//! Pillar 1: Boolean posting-list intersection / union / difference.
//!
//! Compares two ways of evaluating the Boolean algebra a search engine runs
//! over posting lists:
//!
//!   * **`roaring::RoaringTreemap`** — native container-level kernels
//!     (`intersection_len` / `union_len` / `difference_len`) that operate
//!     word-parallel over bitmap containers and gallop over array containers.
//!   * **`expanse_trie::ExpanseSet`** — which has **no native set-algebra
//!     kernel**. The Boolean result is composed at the application level from
//!     the primitives the type does expose: leapfrog `next_at_or_after` for
//!     AND, a dual-iterator merge for OR, and `contains` probing for AND-NOT.
//!     This is exactly what an engine using `ExpanseSet` as a posting-list
//!     backend must do today.
//!
//! Both sides compute a **cardinality** (no result materialization), so the
//! measured quantity is the algebra compute, not allocation.
//!
//! Honesty note (see METHODOLOGY.md, Step 0): this is not a native-vs-native
//! kernel comparison. Roaring is expected to win the dense and symmetric
//! cells decisively because it moves 64 docIDs per word while `ExpanseSet`
//! walks per element. The one cell where per-element leapfrog is plausibly
//! competitive is the **skewed-size AND** (a tiny list probed into a huge
//! one), included explicitly below.
#![allow(missing_docs)]

use serde_json::json;
use std::time::Duration;

#[path = "search_common/mod.rs"]
mod common;
use common::{
    build_list, expanse_and_count, expanse_andnot_count, expanse_or_count, median_ns_per_op,
    to_expanse, to_roaring,
};

fn universe_for(dist: &str, n: usize) -> u64 {
    // Density n/universe ~= 0.5 for the overlapping distributions, so AND has a
    // meaningful result; dense ignores it (contiguous run).
    match dist {
        "dense" => n as u64 * 2,
        "clustered" => n as u64 * 2,
        "sparse" => n as u64 * 2,
        "zipfian" => n as u64 * 2,
        other => panic!("unknown distribution {other}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    let (sizes, batches, min_batch) = if quick {
        (vec![10_000usize, 100_000], 3, Duration::from_millis(30))
    } else {
        (
            vec![10_000usize, 100_000, 1_000_000, 10_000_000],
            5,
            Duration::from_millis(60),
        )
    };
    let dists = ["dense", "clustered", "sparse", "zipfian"];

    let mut results = Vec::new();

    // --- Symmetric cells: |A| == |B| across every distribution & size ---
    for &dist in &dists {
        for &n in &sizes {
            let universe = universe_for(dist, n);
            let a_v = build_list(dist, n, universe, 0);
            let b_v = build_list(dist, n, universe, 1);
            let ea = to_expanse(&a_v);
            let eb = to_expanse(&b_v);
            let ra = to_roaring(&a_v);
            let rb = to_roaring(&b_v);

            let and_card = expanse_and_count(&ea, &eb);
            debug_assert_eq!(and_card, ra.intersection_len(&rb));

            let e_and = median_ns_per_op(|| expanse_and_count(&ea, &eb), batches, min_batch);
            let r_and = median_ns_per_op(|| ra.intersection_len(&rb), batches, min_batch);
            let e_or = median_ns_per_op(|| expanse_or_count(&ea, &eb), batches, min_batch);
            let r_or = median_ns_per_op(|| ra.union_len(&rb), batches, min_batch);
            let e_andnot = median_ns_per_op(|| expanse_andnot_count(&ea, &eb), batches, min_batch);
            let r_andnot = median_ns_per_op(|| ra.difference_len(&rb), batches, min_batch);

            for (op, e_ns, r_ns, card) in [
                ("and", e_and, r_and, and_card),
                ("or", e_or, r_or, expanse_or_count(&ea, &eb)),
                ("andnot", e_andnot, r_andnot, expanse_andnot_count(&ea, &eb)),
            ] {
                results.push(json!({
                    "cell": "symmetric",
                    "distribution": dist,
                    "size": n,
                    "na": a_v.len(),
                    "nb": b_v.len(),
                    "op": op,
                    "card": card,
                    "expanse_ns": e_ns,
                    "roaring_ns": r_ns,
                }));
            }
            if !json_mode {
                eprintln!("  {dist:<10} n={n:>9}  AND exp={e_and:>12.0}ns roar={r_and:>12.0}ns");
            }
        }
    }

    // --- Skewed-size AND: tiny B (0.1% of A) leap-frogged into a huge A ---
    // The one cell where per-element skip-scan is plausibly competitive with a
    // galloping container intersection.
    let skew_sizes = if quick {
        vec![100_000usize]
    } else {
        vec![1_000_000usize, 10_000_000]
    };
    for &n_a in &skew_sizes {
        let n_b = (n_a / 1000).max(16);
        for &dist in &["dense", "zipfian", "sparse"] {
            let universe = universe_for(dist, n_a);
            let a_v = build_list(dist, n_a, universe, 0);
            let b_v = build_list(dist, n_b, universe, 7);
            let ea = to_expanse(&a_v);
            let eb = to_expanse(&b_v);
            let ra = to_roaring(&a_v);
            let rb = to_roaring(&b_v);

            let card = expanse_and_count(&ea, &eb);
            debug_assert_eq!(card, ra.intersection_len(&rb));
            let e_and = median_ns_per_op(|| expanse_and_count(&ea, &eb), batches, min_batch);
            let r_and = median_ns_per_op(|| ra.intersection_len(&rb), batches, min_batch);

            results.push(json!({
                "cell": "skewed",
                "distribution": dist,
                "size": n_a,
                "na": a_v.len(),
                "nb": b_v.len(),
                "op": "and",
                "card": card,
                "expanse_ns": e_and,
                "roaring_ns": r_and,
            }));
            if !json_mode {
                eprintln!(
                    "  skewed {dist:<8} |A|={n_a:>9} |B|={n_b:>7}  AND exp={e_and:>12.0}ns roar={r_and:>12.0}ns"
                );
            }
        }
    }

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    } else {
        eprintln!("\n(pass --json to emit machine-readable results)");
    }
}
