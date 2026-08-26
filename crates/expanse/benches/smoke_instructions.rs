//! Fast Callgrind instruction regression smoke gate ($N = 10,000$).
//!
//! Provides deterministic instruction and cache-miss metrics in <20s for CI
//! gating against regressions.

#![allow(missing_docs)]

use expanse_trie::bytesmap::ExpanseBytesMap;
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use expanse_trie::strmap::ExpanseStrMap;
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

/// Deterministic hasher for the JudyHS cells — `RandomState`'s per-process
/// seed would make bucket placement (and instruction counts) depend on the
/// run, defeating callgrind's exact-reproducibility contract.
type DetHasher = std::hash::BuildHasherDefault<std::collections::hash_map::DefaultHasher>;

/// Route-shaped string keys (~40 bytes, long shared prefixes): the
/// JudySL working shape — prefix chains, suffix leaves, splits — and a
/// realistic JudyHS byte-key distribution.
fn str_keys(_dist: &str) -> Vec<Vec<u8>> {
    (0..POP)
        .map(|i| format!("/api/v2/tenants/{:06}/resources/{:04}", i / 16, i % 16).into_bytes())
        .collect()
}

/// Prebuilt string map plus shuffled probe order so only lookup is measured.
fn built_strmap(dist: &str) -> (ExpanseStrMap, Vec<Vec<u8>>) {
    let ks = str_keys(dist);
    let mut map = ExpanseStrMap::new();
    for (i, k) in ks.iter().enumerate() {
        map.insert(k, i as u64);
    }
    let mut probes = ks;
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    (map, probes)
}

/// Prebuilt byte-string map plus shuffled probe order.
fn built_bytesmap(dist: &str) -> (ExpanseBytesMap<DetHasher>, Vec<Vec<u8>>) {
    let ks = str_keys(dist);
    let mut map = ExpanseBytesMap::with_hasher(DetHasher::default());
    for (i, k) in ks.iter().enumerate() {
        map.insert(k, i as u64);
    }
    let mut probes = ks;
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    (map, probes)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = str_keys)]
fn strmap_insert(ks: Vec<Vec<u8>>) -> u64 {
    let mut map = ExpanseStrMap::new();
    for (i, k) in ks.iter().enumerate() {
        map.insert(black_box(k), black_box(i as u64));
    }
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_strmap)]
fn strmap_get(built: (ExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for k in &probes {
        sink ^= map.get(black_box(k)).unwrap_or(0);
    }
    core::mem::forget(map);
    black_box(sink)
}

// Same-key reinsert (in-place suffix value update), remove (suffix
// disposal + emptied-node pruning), reinsert — the mutation ladder the
// concurrency work routes through disposal helpers.
#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_strmap)]
fn strmap_churn(built: (ExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (mut map, probes) = built;
    let mut sink = 0u64;
    for k in &probes {
        sink ^= map.insert(black_box(k), black_box(7)).unwrap_or(0);
        sink ^= map.remove(black_box(k)).unwrap_or(0);
        map.insert(black_box(k), black_box(9));
    }
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = str_keys)]
fn bytesmap_insert(ks: Vec<Vec<u8>>) -> u64 {
    let mut map = ExpanseBytesMap::with_hasher(DetHasher::default());
    for (i, k) in ks.iter().enumerate() {
        map.insert(black_box(k), black_box(i as u64));
    }
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_bytesmap)]
fn bytesmap_get(built: (ExpanseBytesMap<DetHasher>, Vec<Vec<u8>>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for k in &probes {
        sink ^= map.get(black_box(k)).unwrap_or(0);
    }
    core::mem::forget(map);
    black_box(sink)
}

// Same-key reinsert (in-place value update), remove (bucket
// replacement/removal), reinsert (fresh bucket) — the paths #364
// restructured to publish-replacement-then-dispose.
#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_bytesmap)]
fn bytesmap_churn(built: (ExpanseBytesMap<DetHasher>, Vec<Vec<u8>>)) -> u64 {
    let (mut map, probes) = built;
    let mut sink = 0u64;
    for k in &probes {
        sink ^= map.insert(black_box(k), black_box(7)).unwrap_or(0);
        sink ^= map.remove(black_box(k)).unwrap_or(0);
        map.insert(black_box(k), black_box(9));
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
        map_churn,
        strmap_insert,
        strmap_get,
        strmap_churn,
        bytesmap_insert,
        bytesmap_get,
        bytesmap_churn
);

#[cfg(target_os = "linux")]
main!(library_benchmark_groups = smoke_cost);

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("iai-callgrind instruction benchmarks run on Linux only.");
}
