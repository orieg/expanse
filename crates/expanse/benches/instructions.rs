//! Deterministic cost benchmarks: instructions retired, memory accesses
//! and simulated cache behaviour, via callgrind (`docs/BENCHMARKING.md`,
//! issue #1).
//!
//! Why not wall-clock: the measured noise floor of both available
//! environments (CI runners, the development laptop) is ~15-20% at n=2,
//! while every optimization on the roadmap is worth a few percent. A
//! `memcmp` removal that looked like a 7-11% win at n=1 showed no
//! detectable effect at n=2 — that is the failure mode this harness
//! exists to prevent. Callgrind counts are **exact and reproducible**:
//! the same binary on the same input yields the same number on a loaded
//! laptop and an idle runner alike, so a 1% change is legible.
//!
//! Read the numbers as *cost*, not time: fewer instructions or fewer
//! cache misses is strictly better work, but the wall-clock effect
//! depends on how well the machine hides the remaining latency. A
//! wall-clock claim still requires a quiet host (BENCHMARKING.md).
//!
//! Requires valgrind, which does not support arm64 macOS — these run on
//! Linux, in the `instruction-counts` CI job. Locally:
//! `cargo bench --bench instructions` on a Linux host.

// The `library_benchmark` macro expands to modules that carry no docs of
// their own; the workspace `missing_docs` lint does not apply to a bench
// harness.
#![allow(missing_docs)]

use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// Population per benchmark. Small enough that callgrind (~50x slowdown)
/// stays practical, large enough to build a real multi-level trie with
/// branches, leaves and the compression ladder all exercised.
const POP: usize = 50_000;

fn keys(dist: &str) -> Vec<u64> {
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut out = Vec::with_capacity(POP);
    match dist {
        "sequential" => out.extend(0..POP as u64),
        "random" => out.extend((0..POP).map(|_| rng.next())),
        "clustered" => {
            let mut base = 0;
            for i in 0..POP as u64 {
                if i % 256 == 0 {
                    base = rng.next() & !0xFF;
                }
                out.push(base + (i % 256));
            }
        }
        // Small populations that stay in immediates and short leaves —
        // the terminal forms most inserts actually touch.
        "small" => out.extend((0..POP as u64).map(|i| (i % 12) | ((i / 12) << 32))),
        other => panic!("unknown distribution {other}"),
    }
    out
}

/// A prebuilt map plus a probe order that is not the build order (a
/// sequential probe would measure the prefetcher, not the lookup).
fn built_map(dist: &str) -> (ExpanseMap, Vec<u64>) {
    let ks = keys(dist);
    let mut map = ExpanseMap::new();
    for &k in &ks {
        map.insert(k, !k);
    }
    let mut probes = ks;
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    (map, probes)
}

fn built_set(dist: &str) -> (ExpanseSet, Vec<u64>) {
    let ks = keys(dist);
    let mut set = ExpanseSet::new();
    for &k in &ks {
        set.insert(k);
    }
    let mut probes = ks;
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    (set, probes)
}

// ---- Insert: the larger gap vs stock (issue #1) ----------------------

#[library_benchmark]
#[bench::sequential("sequential")]
#[bench::random("random")]
#[bench::clustered("clustered")]
#[bench::small("small")]
fn map_insert(dist: &str) -> u64 {
    let ks = keys(dist);
    let mut map = ExpanseMap::new();
    for &k in &ks {
        map.insert(black_box(k), black_box(!k));
    }
    black_box(map.len())
}

#[library_benchmark]
#[bench::sequential("sequential")]
#[bench::random("random")]
#[bench::clustered("clustered")]
fn set_insert(dist: &str) -> u64 {
    let ks = keys(dist);
    let mut set = ExpanseSet::new();
    for &k in &ks {
        set.insert(black_box(k));
    }
    black_box(set.len())
}

// The fused single-walk `JudyLIns` path the compat layer uses.
#[library_benchmark]
#[bench::random("random")]
fn map_ins_slot(dist: &str) -> u64 {
    let ks = keys(dist);
    let mut map = ExpanseMap::new();
    for &k in &ks {
        let slot = map.ins_slot(black_box(k));
        // SAFETY: valid until the next mutation; written immediately.
        unsafe { slot.as_ptr().write(!k) };
    }
    black_box(map.len())
}

// ---- Lookup ----------------------------------------------------------

#[library_benchmark]
#[bench::sequential("sequential")]
#[bench::random("random")]
#[bench::clustered("clustered")]
fn map_get(dist: &str) -> u64 {
    let (map, probes) = built_map(dist);
    let mut sink = 0u64;
    for &k in &probes {
        sink ^= map.get(black_box(k)).unwrap_or(0);
    }
    black_box(sink)
}

#[library_benchmark]
#[bench::random("random")]
fn set_contains(dist: &str) -> u64 {
    let (set, probes) = built_set(dist);
    let mut hits = 0u64;
    for &k in &probes {
        hits += u64::from(set.contains(black_box(k)));
    }
    black_box(hits)
}

// ---- Remove and ordered navigation -----------------------------------

#[library_benchmark]
#[bench::random("random")]
fn map_remove(dist: &str) -> u64 {
    let (mut map, probes) = built_map(dist);
    let mut removed = 0u64;
    for &k in &probes {
        removed += u64::from(map.remove(black_box(k)).is_some());
    }
    black_box(removed)
}

#[library_benchmark]
#[bench::random("random")]
fn map_iterate(dist: &str) -> u64 {
    let (map, _) = built_map(dist);
    let mut sink = 0u64;
    for (k, v) in map.iter() {
        sink ^= k ^ v;
    }
    black_box(sink)
}

library_benchmark_group!(
    name = cost;
    benchmarks =
        map_insert,
        set_insert,
        map_ins_slot,
        map_get,
        set_contains,
        map_remove,
        map_iterate
);

main!(library_benchmark_groups = cost);
