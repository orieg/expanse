//! Deterministic arm of the search suite: instruction counts for Boolean
//! cardinality (Pillar 1: AND / OR / AND-NOT, composed and native), AND
//! materialization, and WAND skip-scan (Pillar 2), `ExpanseSet` vs
//! `RoaringTreemap`, via callgrind.
//!
//! Instruction counts are exact and reproducible — the same binary on the same
//! input yields the same number on a loaded laptop and an idle runner alike, so
//! this is the fairest cross-library comparison in the suite: no wall-clock
//! noise, and both sides are measured under one deterministic simulator. Read
//! the numbers as *cost* (retired instructions), not time.
//!
//! Requires valgrind, which does not support arm64 macOS — these run on Linux
//! (the `instruction-counts` CI job). Locally on Linux:
//! `cargo bench -p expanse-trie --bench search_instructions`.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `domain_search_instructions` |
//! | `group` | 4 |
//! | `population` | 1k, 10k, 100k |
//! | `probes_and_reuse` | Postings pairs |
//! | `hit_rate` | Intersection |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | `black_box(count)` |
//! | `measured_region` | Clean (setup in setup) |
//! | `arm_symmetry` | Symmetric |
//! | `statistics` | iai Callgrind exact counts |
//! | `verdict` | **PASS** `[verified: CODE READ]`: Deterministic boolean instructions. |
#![allow(missing_docs)]

use expanse_trie::set::ExpanseSet;
#[cfg(target_os = "linux")]
use iai_callgrind::main;
use iai_callgrind::{library_benchmark, library_benchmark_group};
use roaring::RoaringTreemap;
use std::hint::black_box;

#[path = "search_common/mod.rs"]
mod common;
use common::{
    build_list, expanse_and_count, expanse_and_materialize, expanse_andnot_count,
    expanse_native_and_count, expanse_native_andnot_count, expanse_native_or_count,
    expanse_or_count, expanse_skipscan, roaring_and_materialize, roaring_skipscan, to_expanse,
    to_roaring,
};

/// Population per posting list — small enough that callgrind (~50x slowdown)
/// stays practical, large enough to build a real multi-level trie and multiple
/// Roaring containers.
const POP: usize = 50_000;

fn pair_lists(dist: &str) -> (Vec<u64>, Vec<u64>) {
    let universe = POP as u64 * 2;
    (
        build_list(dist, POP, universe, 0),
        build_list(dist, POP, universe, 1),
    )
}

fn expanse_pair(dist: &str) -> (ExpanseSet, ExpanseSet) {
    let (a, b) = pair_lists(dist);
    (to_expanse(&a), to_expanse(&b))
}

fn roaring_pair(dist: &str) -> (RoaringTreemap, RoaringTreemap) {
    let (a, b) = pair_lists(dist);
    (to_roaring(&a), to_roaring(&b))
}

/// A fixed medium-stride WAND target sequence over the list's span.
fn targets_over(list: &[u64], stride: u64) -> Vec<u64> {
    let (lo, hi) = (*list.first().unwrap(), *list.last().unwrap());
    let mut out = Vec::new();
    let mut t = lo;
    while t <= hi {
        out.push(t);
        t = t.saturating_add(stride);
    }
    out
}

fn expanse_scan(dist: &str) -> (ExpanseSet, Vec<u64>) {
    let list = build_list(dist, POP, POP as u64 * 2, 0);
    let targets = targets_over(&list, 8);
    (to_expanse(&list), targets)
}

fn roaring_scan(dist: &str) -> (RoaringTreemap, Vec<u64>) {
    let list = build_list(dist, POP, POP as u64 * 2, 0);
    let targets = targets_over(&list, 8);
    (to_roaring(&list), targets)
}

// ---- Boolean cardinality: composed vs native, against Roaring ------------
//
// Two Expanse arms per operation, both twinned against the same Roaring
// baseline over the same generated pair:
//
// * `expanse_composed_*` — the Boolean result composed from the navigation
//   primitives (`iter`, `contains`, `next_at_or_after`), which is what a query
//   planner using `ExpanseSet` as a posting-list backend had to do before
//   #339/#347.
// * `expanse_native_*` — the structural kernel (`intersection_len` /
//   `union_len` / `difference_len`) that descends both tries in lockstep and
//   combines bitmap leaves word-parallel.
//
// The two differ by orders of magnitude in the wall-clock artifact
// (`docs/benchmarks/search_inverted_index/results/baseline_boolean.json`), so
// a single row named `expanse_and` cannot stand for both. Naming each arm for
// its kernel is what lets the deterministic counts here be read against the
// wall-clock claim in `docs/BENCHMARKING.md`, which is a claim about the
// native kernel (#417).

#[library_benchmark]
#[bench::dense(args = ("dense",), setup = expanse_pair)]
#[bench::clustered(args = ("clustered",), setup = expanse_pair)]
#[bench::sparse(args = ("sparse",), setup = expanse_pair)]
#[bench::zipfian(args = ("zipfian",), setup = expanse_pair)]
fn expanse_composed_and(pair: (ExpanseSet, ExpanseSet)) -> u64 {
    let (a, b) = pair;
    let n = expanse_and_count(black_box(&a), black_box(&b));
    core::mem::forget((a, b));
    black_box(n)
}

#[library_benchmark]
#[bench::dense(args = ("dense",), setup = expanse_pair)]
#[bench::clustered(args = ("clustered",), setup = expanse_pair)]
#[bench::sparse(args = ("sparse",), setup = expanse_pair)]
#[bench::zipfian(args = ("zipfian",), setup = expanse_pair)]
fn expanse_native_and(pair: (ExpanseSet, ExpanseSet)) -> u64 {
    let (a, b) = pair;
    let n = expanse_native_and_count(black_box(&a), black_box(&b));
    core::mem::forget((a, b));
    black_box(n)
}

#[library_benchmark]
#[bench::dense(args = ("dense",), setup = roaring_pair)]
#[bench::clustered(args = ("clustered",), setup = roaring_pair)]
#[bench::sparse(args = ("sparse",), setup = roaring_pair)]
#[bench::zipfian(args = ("zipfian",), setup = roaring_pair)]
fn roaring_and(pair: (RoaringTreemap, RoaringTreemap)) -> u64 {
    let (a, b) = pair;
    let n = black_box(&a).intersection_len(black_box(&b));
    core::mem::forget((a, b));
    black_box(n)
}

#[library_benchmark]
#[bench::dense(args = ("dense",), setup = expanse_pair)]
#[bench::clustered(args = ("clustered",), setup = expanse_pair)]
#[bench::sparse(args = ("sparse",), setup = expanse_pair)]
#[bench::zipfian(args = ("zipfian",), setup = expanse_pair)]
fn expanse_composed_or(pair: (ExpanseSet, ExpanseSet)) -> u64 {
    let (a, b) = pair;
    let n = expanse_or_count(black_box(&a), black_box(&b));
    core::mem::forget((a, b));
    black_box(n)
}

#[library_benchmark]
#[bench::dense(args = ("dense",), setup = expanse_pair)]
#[bench::clustered(args = ("clustered",), setup = expanse_pair)]
#[bench::sparse(args = ("sparse",), setup = expanse_pair)]
#[bench::zipfian(args = ("zipfian",), setup = expanse_pair)]
fn expanse_native_or(pair: (ExpanseSet, ExpanseSet)) -> u64 {
    let (a, b) = pair;
    let n = expanse_native_or_count(black_box(&a), black_box(&b));
    core::mem::forget((a, b));
    black_box(n)
}

#[library_benchmark]
#[bench::dense(args = ("dense",), setup = roaring_pair)]
#[bench::clustered(args = ("clustered",), setup = roaring_pair)]
#[bench::sparse(args = ("sparse",), setup = roaring_pair)]
#[bench::zipfian(args = ("zipfian",), setup = roaring_pair)]
fn roaring_or(pair: (RoaringTreemap, RoaringTreemap)) -> u64 {
    let (a, b) = pair;
    let n = black_box(&a).union_len(black_box(&b));
    core::mem::forget((a, b));
    black_box(n)
}

#[library_benchmark]
#[bench::dense(args = ("dense",), setup = expanse_pair)]
#[bench::clustered(args = ("clustered",), setup = expanse_pair)]
#[bench::sparse(args = ("sparse",), setup = expanse_pair)]
#[bench::zipfian(args = ("zipfian",), setup = expanse_pair)]
fn expanse_composed_andnot(pair: (ExpanseSet, ExpanseSet)) -> u64 {
    let (a, b) = pair;
    let n = expanse_andnot_count(black_box(&a), black_box(&b));
    core::mem::forget((a, b));
    black_box(n)
}

#[library_benchmark]
#[bench::dense(args = ("dense",), setup = expanse_pair)]
#[bench::clustered(args = ("clustered",), setup = expanse_pair)]
#[bench::sparse(args = ("sparse",), setup = expanse_pair)]
#[bench::zipfian(args = ("zipfian",), setup = expanse_pair)]
fn expanse_native_andnot(pair: (ExpanseSet, ExpanseSet)) -> u64 {
    let (a, b) = pair;
    let n = expanse_native_andnot_count(black_box(&a), black_box(&b));
    core::mem::forget((a, b));
    black_box(n)
}

#[library_benchmark]
#[bench::dense(args = ("dense",), setup = roaring_pair)]
#[bench::clustered(args = ("clustered",), setup = roaring_pair)]
#[bench::sparse(args = ("sparse",), setup = roaring_pair)]
#[bench::zipfian(args = ("zipfian",), setup = roaring_pair)]
fn roaring_andnot(pair: (RoaringTreemap, RoaringTreemap)) -> u64 {
    let (a, b) = pair;
    let n = black_box(&a).difference_len(black_box(&b));
    core::mem::forget((a, b));
    black_box(n)
}

// ---- Boolean AND (materialization) ----------------------------------------
//
// `expanse_and_materialize` is the #348 native direct-emission kernel
// (`ExpanseSet::intersection`), not a composed path — there is no composed
// twin for it in this suite.

#[library_benchmark]
#[bench::dense(args = ("dense",), setup = expanse_pair)]
#[bench::clustered(args = ("clustered",), setup = expanse_pair)]
#[bench::sparse(args = ("sparse",), setup = expanse_pair)]
#[bench::zipfian(args = ("zipfian",), setup = expanse_pair)]
fn expanse_materialize(pair: (ExpanseSet, ExpanseSet)) -> u64 {
    let (a, b) = pair;
    let n = expanse_and_materialize(black_box(&a), black_box(&b));
    core::mem::forget((a, b));
    black_box(n)
}

#[library_benchmark]
#[bench::dense(args = ("dense",), setup = roaring_pair)]
#[bench::clustered(args = ("clustered",), setup = roaring_pair)]
#[bench::sparse(args = ("sparse",), setup = roaring_pair)]
#[bench::zipfian(args = ("zipfian",), setup = roaring_pair)]
fn roaring_materialize(pair: (RoaringTreemap, RoaringTreemap)) -> u64 {
    let (a, b) = pair;
    let n = roaring_and_materialize(black_box(&a), black_box(&b));
    core::mem::forget((a, b));
    black_box(n)
}

// ---- WAND skip-scan -------------------------------------------------------

#[library_benchmark]
#[bench::dense(args = ("dense",), setup = expanse_scan)]
#[bench::clustered(args = ("clustered",), setup = expanse_scan)]
#[bench::sparse(args = ("sparse",), setup = expanse_scan)]
fn expanse_wand(built: (ExpanseSet, Vec<u64>)) -> u64 {
    let (set, targets) = built;
    let sink = expanse_skipscan(black_box(&set), black_box(&targets));
    core::mem::forget((set, targets));
    black_box(sink)
}

#[library_benchmark]
#[bench::dense(args = ("dense",), setup = roaring_scan)]
#[bench::clustered(args = ("clustered",), setup = roaring_scan)]
#[bench::sparse(args = ("sparse",), setup = roaring_scan)]
fn roaring_wand(built: (RoaringTreemap, Vec<u64>)) -> u64 {
    let (tree, targets) = built;
    let sink = roaring_skipscan(black_box(&tree), black_box(&targets));
    core::mem::forget((tree, targets));
    black_box(sink)
}

library_benchmark_group!(
    name = boolean;
    benchmarks = expanse_composed_and, expanse_native_and, roaring_and
);

library_benchmark_group!(
    name = boolean_or;
    benchmarks = expanse_composed_or, expanse_native_or, roaring_or
);

library_benchmark_group!(
    name = boolean_andnot;
    benchmarks = expanse_composed_andnot, expanse_native_andnot, roaring_andnot
);

library_benchmark_group!(
    name = materialize;
    benchmarks = expanse_materialize, roaring_materialize
);

library_benchmark_group!(
    name = wand;
    benchmarks = expanse_wand, roaring_wand
);

#[cfg(target_os = "linux")]
main!(
    library_benchmark_groups = boolean,
    boolean_or,
    boolean_andnot,
    materialize,
    wand
);

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("iai-callgrind instruction benchmarks run on Linux only.");
}
