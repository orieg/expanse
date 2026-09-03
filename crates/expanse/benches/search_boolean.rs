//! Pillar 1: Boolean posting-list intersection / union / difference.
//!
//! Compares two ways of evaluating the Boolean algebra a search engine runs
//! over posting lists:
//!
//!   * **`roaring::RoaringTreemap`** — native container-level kernels
//!     (`intersection_len` / `union_len` / `difference_len`) that operate
//!     word-parallel over bitmap containers and gallop over array containers.
//!   * **`expanse_trie::ExpanseSet`** — native structural set-algebra kernels
//!     (issue #339 cardinality, #348 materialization): a lockstep walk of both
//!     tries that skips absent subtrees, `AND`s bitmap leaves word-parallel,
//!     and — for the materializing ops — emits the result tree directly.
//!
//! Two quantities are measured per cell:
//!
//!   * **Cardinality** (`*_count`): the Boolean population, no result built —
//!     `ExpanseSet::intersection_len` etc. vs roaring's `*_len`.
//!   * **Materialization** (`*_materialize`, #348): the result *set* built —
//!     `ExpanseSet::intersection` (v2 direct emission) and the pre-#348
//!     ordered-merge + per-key `insert` path (v1) vs roaring `&`/`|`/`-`
//!     returning a bitmap.
//!
//! Honesty note (see METHODOLOGY.md, Step 0): roaring moves 64 docIDs per word
//! and owns the dense and symmetric cells; `ExpanseSet`'s 256-key bitmap leaf
//! pays more edge decodes per op. The cells where the structural walk is
//! plausibly competitive are the **skewed-size AND** (a tiny list probed into a
//! huge one) and the **skewed-dense / Zipfian** shapes.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `domain_search_boolean` |
//! | `group` | 4 |
//! | `population` | Synthetic postings |
//! | `probes_and_reuse` | Postings sets |
//! | `hit_rate` | Intersection |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | `black_box(result)` |
//! | `measured_region` | Clean |
//! | `arm_symmetry` | Symmetric (Roaring vs Expanse) |
//! | `statistics` | Raw ms (no CI) |
//! | `verdict` | **PASS** `[verified: CODE READ]`: Boolean index evaluation. |
#![allow(missing_docs)]

use serde_json::json;
use std::time::Duration;

#[path = "search_common/mod.rs"]
mod common;
use common::{
    build_list, expanse_and_count, expanse_and_materialize, expanse_and_materialize_v1,
    expanse_andnot_count, expanse_andnot_materialize, expanse_andnot_materialize_v1,
    expanse_kway_and_count, expanse_kway_and_materialize, expanse_kway_or_count,
    expanse_kway_or_materialize, expanse_native_and_count, expanse_native_andnot_count,
    expanse_native_or_count, expanse_or_count, expanse_or_materialize, expanse_or_materialize_v1,
    expanse_pairwise_and_count, expanse_pairwise_or_count, median_ns_per_op,
    roaring_and_materialize, roaring_andnot_materialize, roaring_multiops_and_count,
    roaring_multiops_and_materialize, roaring_multiops_or_count, roaring_multiops_or_materialize,
    roaring_or_materialize, to_expanse, to_roaring,
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
            assert_eq!(and_card, ra.intersection_len(&rb));
            // Native kernels must agree with the composed path and Roaring.
            assert_eq!(expanse_native_and_count(&ea, &eb), and_card);
            assert_eq!(expanse_native_or_count(&ea, &eb), ra.union_len(&rb));
            assert_eq!(
                expanse_native_andnot_count(&ea, &eb),
                ra.difference_len(&rb)
            );

            let e_and = median_ns_per_op(|| expanse_and_count(&ea, &eb), batches, min_batch);
            let n_and = median_ns_per_op(|| expanse_native_and_count(&ea, &eb), batches, min_batch);
            let r_and = median_ns_per_op(|| ra.intersection_len(&rb), batches, min_batch);
            let e_or = median_ns_per_op(|| expanse_or_count(&ea, &eb), batches, min_batch);
            let n_or = median_ns_per_op(|| expanse_native_or_count(&ea, &eb), batches, min_batch);
            let r_or = median_ns_per_op(|| ra.union_len(&rb), batches, min_batch);
            let e_andnot = median_ns_per_op(|| expanse_andnot_count(&ea, &eb), batches, min_batch);
            let n_andnot =
                median_ns_per_op(|| expanse_native_andnot_count(&ea, &eb), batches, min_batch);
            let r_andnot = median_ns_per_op(|| ra.difference_len(&rb), batches, min_batch);

            // --- Materializing arm (#348): build the result set, not the
            // cardinality. v2 = native direct emission; v1 = the pre-#348
            // ordered-merge + per-key insert; roaring returns a bitmap. Every
            // arm must produce the same cardinality as the AND/OR/AND-NOT
            // cardinality cells.
            assert_eq!(expanse_and_materialize(&ea, &eb), and_card);
            assert_eq!(expanse_and_materialize_v1(&ea, &eb), and_card);
            assert_eq!(roaring_and_materialize(&ra, &rb), and_card);
            assert_eq!(expanse_or_materialize(&ea, &eb), expanse_or_count(&ea, &eb));
            assert_eq!(
                expanse_andnot_materialize(&ea, &eb),
                expanse_andnot_count(&ea, &eb)
            );
            let m_and = median_ns_per_op(|| expanse_and_materialize(&ea, &eb), batches, min_batch);
            let m_and_v1 =
                median_ns_per_op(|| expanse_and_materialize_v1(&ea, &eb), batches, min_batch);
            let rm_and = median_ns_per_op(|| roaring_and_materialize(&ra, &rb), batches, min_batch);
            let m_or = median_ns_per_op(|| expanse_or_materialize(&ea, &eb), batches, min_batch);
            let m_or_v1 =
                median_ns_per_op(|| expanse_or_materialize_v1(&ea, &eb), batches, min_batch);
            let rm_or = median_ns_per_op(|| roaring_or_materialize(&ra, &rb), batches, min_batch);
            let m_andnot =
                median_ns_per_op(|| expanse_andnot_materialize(&ea, &eb), batches, min_batch);
            let m_andnot_v1 = median_ns_per_op(
                || expanse_andnot_materialize_v1(&ea, &eb),
                batches,
                min_batch,
            );
            let rm_andnot =
                median_ns_per_op(|| roaring_andnot_materialize(&ra, &rb), batches, min_batch);

            for (op, e_ns, n_ns, r_ns, card, mat_ns, mat_v1_ns, rmat_ns) in [
                (
                    "and", e_and, n_and, r_and, and_card, m_and, m_and_v1, rm_and,
                ),
                (
                    "or",
                    e_or,
                    n_or,
                    r_or,
                    expanse_or_count(&ea, &eb),
                    m_or,
                    m_or_v1,
                    rm_or,
                ),
                (
                    "andnot",
                    e_andnot,
                    n_andnot,
                    r_andnot,
                    expanse_andnot_count(&ea, &eb),
                    m_andnot,
                    m_andnot_v1,
                    rm_andnot,
                ),
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
                    "expanse_native_ns": n_ns,
                    "roaring_ns": r_ns,
                    "expanse_materialize_ns": mat_ns,
                    "expanse_materialize_v1_ns": mat_v1_ns,
                    "roaring_materialize_ns": rmat_ns,
                }));
            }
            if !json_mode {
                eprintln!(
                    "  {dist:<10} n={n:>9}  AND card: native={n_and:>11.0}ns roar={r_and:>11.0}ns  |  AND mat: v2={m_and:>11.0}ns v1={m_and_v1:>11.0}ns roar={rm_and:>11.0}ns"
                );
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
            assert_eq!(card, ra.intersection_len(&rb));
            assert_eq!(expanse_native_and_count(&ea, &eb), card);
            let e_and = median_ns_per_op(|| expanse_and_count(&ea, &eb), batches, min_batch);
            let n_and = median_ns_per_op(|| expanse_native_and_count(&ea, &eb), batches, min_batch);
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
                "expanse_native_ns": n_and,
                "roaring_ns": r_and,
            }));
            if !json_mode {
                eprintln!(
                    "  skewed {dist:<8} |A|={n_a:>9} |B|={n_b:>7}  AND composed={e_and:>12.0}ns native={n_and:>12.0}ns roar={r_and:>12.0}ns"
                );
            }
        }
    }

    // --- k-way aggregate set algebra (#610): k in [2, 3, 5, 8, 16] ---
    let k_values = if quick {
        vec![2usize, 3, 5]
    } else {
        vec![2usize, 3, 5, 8, 16]
    };
    let k_sizes = if quick {
        vec![10_000usize, 100_000]
    } else {
        vec![10_000usize, 100_000, 1_000_000]
    };

    for &dist in &dists {
        for &n in &k_sizes {
            let universe = universe_for(dist, n);
            for &k in &k_values {
                let lists: Vec<Vec<u64>> = (0..k)
                    .map(|seed| build_list(dist, n, universe, seed as u64))
                    .collect();
                let expanse_sets: Vec<_> = lists.iter().map(|l| to_expanse(l)).collect();
                let roaring_sets: Vec<_> = lists.iter().map(|l| to_roaring(l)).collect();
                let e_refs: Vec<&expanse_trie::set::ExpanseSet> = expanse_sets.iter().collect();
                let r_refs: Vec<&roaring::RoaringTreemap> = roaring_sets.iter().collect();

                let k_and_card = expanse_kway_and_count(&e_refs);
                let r_and_card = roaring_multiops_and_count(&r_refs);
                assert_eq!(k_and_card, r_and_card);
                let k_or_card = expanse_kway_or_count(&e_refs);
                let r_or_card = roaring_multiops_or_count(&r_refs);
                assert_eq!(k_or_card, r_or_card);

                let k_and_ns =
                    median_ns_per_op(|| expanse_kway_and_count(&e_refs), batches, min_batch);
                let pair_and_ns =
                    median_ns_per_op(|| expanse_pairwise_and_count(&e_refs), batches, min_batch);
                let r_and_ns =
                    median_ns_per_op(|| roaring_multiops_and_count(&r_refs), batches, min_batch);

                let k_or_ns =
                    median_ns_per_op(|| expanse_kway_or_count(&e_refs), batches, min_batch);
                let pair_or_ns =
                    median_ns_per_op(|| expanse_pairwise_or_count(&e_refs), batches, min_batch);
                let r_or_ns =
                    median_ns_per_op(|| roaring_multiops_or_count(&r_refs), batches, min_batch);

                let k_mat_and_ns =
                    median_ns_per_op(|| expanse_kway_and_materialize(&e_refs), batches, min_batch);
                let r_mat_and_ns = median_ns_per_op(
                    || roaring_multiops_and_materialize(&r_refs),
                    batches,
                    min_batch,
                );

                let k_mat_or_ns =
                    median_ns_per_op(|| expanse_kway_or_materialize(&e_refs), batches, min_batch);
                let r_mat_or_ns = median_ns_per_op(
                    || roaring_multiops_or_materialize(&r_refs),
                    batches,
                    min_batch,
                );

                results.push(json!({
                    "cell": "kway",
                    "distribution": dist,
                    "size": n,
                    "k": k,
                    "op": "and",
                    "card": k_and_card,
                    "expanse_kway_ns": k_and_ns,
                    "expanse_pairwise_ns": pair_and_ns,
                    "roaring_ns": r_and_ns,
                    "expanse_materialize_ns": k_mat_and_ns,
                    "roaring_materialize_ns": r_mat_and_ns,
                }));

                results.push(json!({
                    "cell": "kway",
                    "distribution": dist,
                    "size": n,
                    "k": k,
                    "op": "or",
                    "card": k_or_card,
                    "expanse_kway_ns": k_or_ns,
                    "expanse_pairwise_ns": pair_or_ns,
                    "roaring_ns": r_or_ns,
                    "expanse_materialize_ns": k_mat_or_ns,
                    "roaring_materialize_ns": r_mat_or_ns,
                }));

                if !json_mode {
                    eprintln!(
                        "  kway {dist:<10} k={k:>2} n={n:>8}  AND: kway={k_and_ns:>10.0}ns pair={pair_and_ns:>10.0}ns roar={r_and_ns:>10.0}ns  |  OR: kway={k_or_ns:>10.0}ns pair={pair_or_ns:>10.0}ns roar={r_or_ns:>10.0}ns"
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
