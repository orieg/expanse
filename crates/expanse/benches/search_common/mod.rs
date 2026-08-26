//! Shared generators and a microbench helper for the search / inverted-index
//! suite (`docs/benchmarks/search_inverted_index/`).
//!
//! Included via `#[path = "search_common/mod.rs"] mod common;` from each
//! `search_*` harness. It lives in a subdirectory rather than directly under
//! `benches/` so Cargo does not auto-discover it as a phantom bench target.
//!
//! # Posting-list model
//!
//! A posting list is a sorted set of document IDs. Terms in a real corpus
//! follow very different shapes, so four synthetic distributions are modelled,
//! each mapping to a real indexing regime:
//!
//! * `dense` — a contiguous run of docIDs (a term present in every doc of a
//!   segment; the regime Roaring run/bitmap containers own).
//! * `clustered` — bursts of contiguous docIDs separated by gaps (documents
//!   arrive in topical batches; time-partitioned segments).
//! * `sparse` — uniform-random docIDs over a wide universe (a rare term
//!   scattered across a large 64-bit ID space).
//! * `zipfian` — power-law docIDs concentrated on low IDs (recency- or
//!   popularity-skewed assignment; s = 0.99, the YCSB default).
//!
//! `shard` is memory-only: `(tenant << 40) | doc`, the multi-tenant shard-ID
//! layout that forces keys to spread across the high 64-bit lanes.
#![allow(dead_code)]

use expanse_trie::set::ExpanseSet;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rand_distr::{Distribution, Zipf};
use roaring::RoaringTreemap;
use std::hint::black_box;
use std::time::{Duration, Instant};

/// Cluster length for the `clustered` distribution: a burst of this many
/// contiguous docIDs at each randomly-placed base.
pub const CLUSTER_LEN: u64 = 128;

/// Build a single posting list (sorted, de-duplicated docIDs).
///
/// `universe` bounds the docID space for `sparse`/`zipfian`/`clustered`; the
/// density `n / universe` therefore controls how much two independently-seeded
/// lists overlap. `dense` ignores `universe` and returns a contiguous run whose
/// start is offset by `seed` so a pair can be made to overlap deterministically.
pub fn build_list(dist: &str, n: usize, universe: u64, seed: u64) -> Vec<u64> {
    let mut rng = StdRng::seed_from_u64(0x5EED_0000 ^ seed);
    let mut v: Vec<u64> = match dist {
        "dense" => {
            // Start offset is a multiple of n/2 so seed 0 and seed 1 overlap on
            // half their length; the run itself is exactly contiguous.
            let start = (seed % 4) * (n as u64 / 2).max(1);
            (start..start + n as u64).collect()
        }
        "clustered" => {
            let n_clusters = (n as u64).div_ceil(CLUSTER_LEN);
            let base_universe = (universe / CLUSTER_LEN).max(1);
            let mut out = Vec::with_capacity(n);
            for _ in 0..n_clusters {
                let base = rng.gen_range(0..base_universe) * CLUSTER_LEN;
                for j in 0..CLUSTER_LEN {
                    out.push(base + j);
                }
            }
            out
        }
        "sparse" => (0..n).map(|_| rng.gen_range(0..universe)).collect(),
        "zipfian" => {
            let zipf = Zipf::new(universe, 0.99).expect("valid Zipf params");
            (0..n).map(|_| zipf.sample(&mut rng) as u64).collect()
        }
        "shard" => {
            // (tenant << 40) | doc — 16 tenants, contiguous docs per tenant.
            let tenants = 16u64;
            let per = (n as u64).div_ceil(tenants);
            let mut out = Vec::with_capacity(n);
            for t in 0..tenants {
                for d in 0..per {
                    out.push((t << 40) | d);
                }
            }
            out.truncate(n);
            out
        }
        other => panic!("unknown distribution {other}"),
    };
    v.sort_unstable();
    v.dedup();
    v
}

/// Build an [`ExpanseSet`] from a docID slice.
pub fn to_expanse(v: &[u64]) -> ExpanseSet {
    let mut s = ExpanseSet::new();
    for &k in v {
        s.insert(k);
    }
    s
}

/// Build a [`RoaringTreemap`] from a docID slice.
pub fn to_roaring(v: &[u64]) -> RoaringTreemap {
    v.iter().copied().collect()
}

// ------------------------------------------------------------------------
// ExpanseSet application-level Boolean ops (cardinality only).
//
// ExpanseSet has no native set-algebra kernel; these compose the Boolean
// result from the navigation primitives it does expose, which is what an
// engine using it as a posting-list backend must do.
// ------------------------------------------------------------------------

/// Intersection cardinality via a sorted lockstep merge of both iterators.
/// O(|a| + |b|) with cheap iterator steps (no root re-descent) — the right
/// strategy when the two lists are of similar size.
pub fn expanse_and_merge(a: &ExpanseSet, b: &ExpanseSet) -> u64 {
    let mut ia = a.iter();
    let mut ib = b.iter();
    let mut count = 0u64;
    let (mut x, mut y) = (ia.next(), ib.next());
    while let (Some(xv), Some(yv)) = (x, y) {
        match xv.cmp(&yv) {
            core::cmp::Ordering::Equal => {
                count += 1;
                x = ia.next();
                y = ib.next();
            }
            core::cmp::Ordering::Less => x = ia.next(),
            core::cmp::Ordering::Greater => y = ib.next(),
        }
    }
    count
}

/// Intersection cardinality via two-way leapfrog on `next_at_or_after`,
/// driven by the smaller set. Each skip is a stateless O(depth) re-descent, so
/// this only pays off when one list is far smaller than the other (a tiny list
/// leap-frogged through a huge one) — otherwise the merge above is cheaper.
pub fn expanse_and_leapfrog(a: &ExpanseSet, b: &ExpanseSet) -> u64 {
    let (small, big) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let mut count = 0u64;
    let mut probe = match small.first() {
        Some(x) => x,
        None => return 0,
    };
    loop {
        match big.next_at_or_after(probe) {
            None => break,
            Some(hit) if hit == probe => {
                count += 1;
                probe = match small.next_after(probe) {
                    Some(x) => x,
                    None => break,
                };
            }
            Some(hit) => {
                probe = match small.next_at_or_after(hit) {
                    Some(x) => x,
                    None => break,
                };
            }
        }
    }
    count
}

/// Adaptive intersection: leapfrog when the size ratio exceeds 32:1 (a tiny
/// list into a huge one), else a lockstep merge — the choice a query planner
/// using `ExpanseSet` as a posting-list backend would make.
pub fn expanse_and_count(a: &ExpanseSet, b: &ExpanseSet) -> u64 {
    let (lo, hi) = (a.len().min(b.len()), a.len().max(b.len()));
    if lo != 0 && hi / lo >= 32 {
        expanse_and_leapfrog(a, b)
    } else {
        expanse_and_merge(a, b)
    }
}

/// Union cardinality via a sorted merge of both ascending iterators.
pub fn expanse_or_count(a: &ExpanseSet, b: &ExpanseSet) -> u64 {
    let mut ia = a.iter().peekable();
    let mut ib = b.iter().peekable();
    let mut count = 0u64;
    loop {
        match (ia.peek(), ib.peek()) {
            (Some(&x), Some(&y)) => {
                if x == y {
                    ia.next();
                    ib.next();
                } else if x < y {
                    ia.next();
                } else {
                    ib.next();
                }
                count += 1;
            }
            (Some(_), None) => {
                ia.next();
                count += 1;
            }
            (None, Some(_)) => {
                ib.next();
                count += 1;
            }
            (None, None) => break,
        }
    }
    count
}

/// Difference cardinality (|a \ b|): walk `a`, probe each element in `b`.
pub fn expanse_andnot_count(a: &ExpanseSet, b: &ExpanseSet) -> u64 {
    let mut count = 0u64;
    for x in a.iter() {
        if !b.contains(x) {
            count += 1;
        }
    }
    count
}

// ------------------------------------------------------------------------
// Native set-algebra kernels (issue #339): the second arm. These call the
// structural cardinality kernels that descend both tries in lockstep and AND
// bitmap leaves word-parallel, instead of composing the Boolean result from
// navigation primitives element by element.
// ------------------------------------------------------------------------

/// Intersection cardinality via the native structural kernel.
pub fn expanse_native_and_count(a: &ExpanseSet, b: &ExpanseSet) -> u64 {
    a.intersection_len(b)
}

/// Union cardinality via the native structural kernel.
pub fn expanse_native_or_count(a: &ExpanseSet, b: &ExpanseSet) -> u64 {
    a.union_len(b)
}

/// Difference cardinality (|a \ b|) via the native structural kernel.
pub fn expanse_native_andnot_count(a: &ExpanseSet, b: &ExpanseSet) -> u64 {
    a.difference_len(b)
}

/// WAND skip-scan over an [`ExpanseSet`]: stateless O(depth) re-descent per
/// target.
pub fn expanse_skipscan(set: &ExpanseSet, targets: &[u64]) -> u64 {
    let mut sink = 0u64;
    for &t in targets {
        if let Some(v) = set.next_at_or_after(t) {
            sink = sink.wrapping_add(v);
        }
    }
    sink
}

/// WAND skip-scan over a [`RoaringTreemap`]: stateful cursor advance per target.
pub fn roaring_skipscan(tree: &RoaringTreemap, targets: &[u64]) -> u64 {
    let mut it = tree.iter();
    let mut sink = 0u64;
    for &t in targets {
        it.advance_to(t);
        if let Some(v) = it.next() {
            sink = sink.wrapping_add(v);
        }
    }
    sink
}

// ------------------------------------------------------------------------
// Microbench helper: median ns per operation.
// ------------------------------------------------------------------------

/// Time a closure that returns a `u64` sink, reporting the median ns/op over
/// `batches` calibrated batches. Each batch grows its repetition count until it
/// accumulates at least `min_batch`, so both microsecond (small lists) and
/// millisecond (10^7 dense) operations are measured without per-call clock
/// overhead dominating. The sink is `black_box`ed to defeat dead-code
/// elimination.
pub fn median_ns_per_op<F: FnMut() -> u64>(mut f: F, batches: usize, min_batch: Duration) -> f64 {
    let mut sink = 0u64;
    // Warmup: prime caches / branch predictors, out of the measured region.
    for _ in 0..2 {
        sink = sink.wrapping_add(f());
    }
    let mut samples = Vec::with_capacity(batches);
    for _ in 0..batches {
        let mut reps: u64 = 1;
        loop {
            let t = Instant::now();
            for _ in 0..reps {
                sink = sink.wrapping_add(black_box(f()));
            }
            let el = t.elapsed();
            if el >= min_batch || reps >= (1 << 30) {
                samples.push(el.as_nanos() as f64 / reps as f64);
                break;
            }
            // Scale reps toward filling min_batch, at least doubling.
            let factor = (min_batch.as_nanos() as f64 / el.as_nanos().max(1) as f64).ceil();
            reps = ((reps as f64 * factor) as u64).max(reps * 2).max(1);
        }
    }
    black_box(sink);
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples[samples.len() / 2]
}
