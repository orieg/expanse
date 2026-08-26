//! Pillar 2 (deterministic arm): instruction counts for Boolean AND and WAND
//! skip-scan, `ExpanseSet` vs `RoaringTreemap`, via callgrind.
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
    build_list, expanse_and_count, expanse_skipscan, roaring_skipscan, to_expanse, to_roaring,
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

// ---- Boolean AND (cardinality) --------------------------------------------

#[library_benchmark]
#[bench::dense(args = ("dense",), setup = expanse_pair)]
#[bench::clustered(args = ("clustered",), setup = expanse_pair)]
#[bench::sparse(args = ("sparse",), setup = expanse_pair)]
#[bench::zipfian(args = ("zipfian",), setup = expanse_pair)]
fn expanse_and(pair: (ExpanseSet, ExpanseSet)) -> u64 {
    let (a, b) = pair;
    let n = expanse_and_count(black_box(&a), black_box(&b));
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
    benchmarks = expanse_and, roaring_and
);

library_benchmark_group!(
    name = wand;
    benchmarks = expanse_wand, roaring_wand
);

#[cfg(target_os = "linux")]
main!(library_benchmark_groups = boolean, wand);

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("iai-callgrind instruction benchmarks run on Linux only.");
}
