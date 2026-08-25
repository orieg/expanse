//! Fast Callgrind instruction regression smoke gate ($N = 10,000$).
//!
//! Provides deterministic instruction and cache-miss metrics in <20s for CI
//! gating against regressions.

#![allow(missing_docs)]

use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
#[cfg(target_os = "linux")]
use iai_callgrind::main;
use iai_callgrind::{library_benchmark, library_benchmark_group};
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

/// Scaled-down population for fast CI smoke runs (<20s under Callgrind).
const POP: usize = 10_000;

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
        other => panic!("unknown distribution {other}"),
    }
    out
}

/// Prebuilt map plus shuffled probe order so only lookup is measured.
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

/// Prebuilt set plus shuffled probe order so only lookup is measured.
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

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = keys)]
#[bench::random(args = ("random",), setup = keys)]
#[bench::clustered(args = ("clustered",), setup = keys)]
fn map_insert(ks: Vec<u64>) -> u64 {
    let mut map = ExpanseMap::new();
    for &k in &ks {
        map.insert(black_box(k), black_box(!k));
    }
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = built_map)]
#[bench::random(args = ("random",), setup = built_map)]
#[bench::clustered(args = ("clustered",), setup = built_map)]
fn map_get(built: (ExpanseMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for &k in &probes {
        sink ^= map.get(black_box(k)).unwrap_or(0);
    }
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = keys)]
#[bench::random(args = ("random",), setup = keys)]
#[bench::clustered(args = ("clustered",), setup = keys)]
fn set_insert(ks: Vec<u64>) -> u64 {
    let mut set = ExpanseSet::new();
    for &k in &ks {
        set.insert(black_box(k));
    }
    let n = set.len();
    core::mem::forget(set);
    black_box(n)
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = built_set)]
#[bench::random(args = ("random",), setup = built_set)]
#[bench::clustered(args = ("clustered",), setup = built_set)]
fn set_contains(built: (ExpanseSet, Vec<u64>)) -> u64 {
    let (set, probes) = built;
    let mut hits = 0u64;
    for &k in &probes {
        hits += u64::from(set.contains(black_box(k)));
    }
    core::mem::forget(set);
    black_box(hits)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = keys)]
fn map_ins_slot(ks: Vec<u64>) -> u64 {
    let mut map = ExpanseMap::new();
    for &k in &ks {
        let slot = map.ins_slot(black_box(k));
        // SAFETY: valid until the next mutation; written immediately.
        unsafe { slot.as_ptr().write(!k) };
    }
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_map)]
fn map_churn(built: (ExpanseMap, Vec<u64>)) -> u64 {
    let (mut map, probes) = built;
    let mut sink = 0u64;
    for &k in &probes {
        sink ^= map.insert(black_box(k), black_box(!k)).unwrap_or(0);
        map.insert(black_box(k ^ 1), k);
        sink ^= u64::from(map.remove(black_box(k ^ 1)).is_some());
    }
    core::mem::forget(map);
    black_box(sink)
}

library_benchmark_group!(
    name = smoke_cost;
    benchmarks =
        map_insert,
        map_get,
        set_insert,
        set_contains,
        map_ins_slot,
        map_churn
);

#[cfg(target_os = "linux")]
main!(library_benchmark_groups = smoke_cost);

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("iai-callgrind instruction benchmarks run on Linux only.");
}
