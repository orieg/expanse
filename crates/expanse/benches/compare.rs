//! Speed benches per `docs/BENCHMARKING.md`: lookup latency (hit and miss
//! measured separately) and insert throughput, per key-distribution class
//! and population size, for `ExpanseSet`/`ExpanseMap` against the std
//! baselines (`BTreeSet`/`BTreeMap`, `HashSet`/`HashMap`).
//!
//! Discipline reminders (see the doc): numbers from a loaded machine are
//! not publishable; regression comparisons need interleaved A/B arms; the
//! C-libjudy comparison arrives with the capi bench harness. Memory
//! (bytes/key) is measured by `examples/bytes_per_key.rs`, not here.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
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

/// The TESTING.md key-distribution classes.
fn keys(dist: &str, n: usize) -> Vec<u64> {
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut out = Vec::with_capacity(n);
    match dist {
        "sequential" => out.extend(0..n as u64),
        "random" => out.extend((0..n).map(|_| rng.next())),
        "clustered" => {
            // Dense runs of 256 at random 64-bit bases.
            let mut base = 0;
            for i in 0..n as u64 {
                if i % 256 == 0 {
                    base = rng.next() & !0xFF;
                }
                out.push(base + (i % 256));
            }
        }
        "sparse" => out.extend((0..n as u64).map(|i| i << 40)),
        _ => unreachable!(),
    }
    out
}

/// Probe sets: present keys (hits) and perturbed keys (mostly misses).
fn probes(keys: &[u64]) -> (Vec<u64>, Vec<u64>) {
    let mut rng = XorShift(0xBEEF_CAFE_1234_5678);
    let hits: Vec<u64> = (0..4096)
        .map(|_| keys[(rng.next() as usize) % keys.len()])
        .collect();
    let misses: Vec<u64> = hits.iter().map(|k| k ^ (1 << 63) ^ 0x5A).collect();
    (hits, misses)
}

const DISTS: [&str; 4] = ["sequential", "random", "clustered", "sparse"];
const POPS: [usize; 2] = [10_000, 1_000_000];

fn bench_set_lookup(c: &mut Criterion) {
    for dist in DISTS {
        for pop in POPS {
            let ks = keys(dist, pop);
            let (hits, misses) = probes(&ks);
            let mut expanse = ExpanseSet::new();
            let mut btree = BTreeSet::new();
            let mut hash = HashSet::new();
            for &k in &ks {
                expanse.insert(k);
                btree.insert(k);
                hash.insert(k);
            }
            let mut g = c.benchmark_group(format!("set_lookup/{dist}/{pop}"));
            for (case, probe) in [("hit", &hits), ("miss", &misses)] {
                let mut i = 0;
                g.bench_with_input(BenchmarkId::new("expanse", case), probe, |b, p| {
                    b.iter(|| {
                        i = (i + 1) % p.len();
                        black_box(expanse.contains(black_box(p[i])))
                    })
                });
                g.bench_with_input(BenchmarkId::new("btree", case), probe, |b, p| {
                    b.iter(|| {
                        i = (i + 1) % p.len();
                        black_box(btree.contains(&black_box(p[i])))
                    })
                });
                g.bench_with_input(BenchmarkId::new("hash", case), probe, |b, p| {
                    b.iter(|| {
                        i = (i + 1) % p.len();
                        black_box(hash.contains(&black_box(p[i])))
                    })
                });
            }
            g.finish();
        }
    }
}

fn bench_map_lookup(c: &mut Criterion) {
    for dist in DISTS {
        for pop in POPS {
            let ks = keys(dist, pop);
            let (hits, _) = probes(&ks);
            let mut expanse = ExpanseMap::new();
            let mut btree = BTreeMap::new();
            let mut hash = HashMap::new();
            for &k in &ks {
                expanse.insert(k, !k);
                btree.insert(k, !k);
                hash.insert(k, !k);
            }
            let mut g = c.benchmark_group(format!("map_get/{dist}/{pop}"));
            let mut i = 0;
            g.bench_with_input(BenchmarkId::new("expanse", "hit"), &hits, |b, p| {
                b.iter(|| {
                    i = (i + 1) % p.len();
                    black_box(expanse.get(black_box(p[i])))
                })
            });
            g.bench_with_input(BenchmarkId::new("btree", "hit"), &hits, |b, p| {
                b.iter(|| {
                    i = (i + 1) % p.len();
                    black_box(btree.get(&black_box(p[i])))
                })
            });
            g.bench_with_input(BenchmarkId::new("hash", "hit"), &hits, |b, p| {
                b.iter(|| {
                    i = (i + 1) % p.len();
                    black_box(hash.get(&black_box(p[i])))
                })
            });
            g.finish();
        }
    }
}

fn bench_insert(c: &mut Criterion) {
    for dist in DISTS {
        let pop = 100_000;
        let ks = keys(dist, pop);
        let mut g = c.benchmark_group(format!("set_insert_build/{dist}/{pop}"));
        g.sample_size(10);
        g.bench_function("expanse", |b| {
            b.iter(|| {
                let mut s = ExpanseSet::new();
                for &k in &ks {
                    s.insert(black_box(k));
                }
                black_box(s.len())
            })
        });
        g.bench_function("btree", |b| {
            b.iter(|| {
                let mut s = BTreeSet::new();
                for &k in &ks {
                    s.insert(black_box(k));
                }
                black_box(s.len())
            })
        });
        g.bench_function("hash", |b| {
            b.iter(|| {
                let mut s = HashSet::new();
                for &k in &ks {
                    s.insert(black_box(k));
                }
                black_box(s.len())
            })
        });
        g.finish();
    }
}

/// Wide probe sets for cold-DRAM measurement (exceeding L1/L2 and L3 cache).
fn cold_probes(keys: &[u64], probe_count: usize) -> (Vec<u64>, Vec<u64>) {
    let mut rng = XorShift(0xFEED_FACE_CAFE_BEEF);
    let hits: Vec<u64> = (0..probe_count)
        .map(|_| keys[(rng.next() as usize) % keys.len()])
        .collect();
    let misses: Vec<u64> = hits.iter().map(|k| k ^ (1 << 63) ^ 0xA5).collect();
    (hits, misses)
}

fn bench_cold_dram_lookup(c: &mut Criterion) {
    let pop = 1_000_000;
    let ks = keys("random", pop);
    // 524_288 probes (4 MiB of keys) randomly drawn across the 1M key population
    // to measure cold DRAM traversal latency outside L1/L2 cache.
    let (hits, misses) = cold_probes(&ks, 524_288);

    let mut expanse_map = ExpanseMap::new();
    let mut btree_map = BTreeMap::new();
    let mut hash_map = HashMap::new();
    for &k in &ks {
        expanse_map.insert(k, !k);
        btree_map.insert(k, !k);
        hash_map.insert(k, !k);
    }

    let mut g = c.benchmark_group(format!("map_get_cold_dram/random/{pop}"));
    g.sample_size(15);
    for (case, probe) in [("hit", &hits), ("miss", &misses)] {
        let mut i = 0;
        g.bench_with_input(BenchmarkId::new("expanse", case), probe, |b, p| {
            b.iter(|| {
                i = (i + 1) % p.len();
                black_box(expanse_map.get(black_box(p[i])))
            })
        });
        g.bench_with_input(BenchmarkId::new("btree", case), probe, |b, p| {
            b.iter(|| {
                i = (i + 1) % p.len();
                black_box(btree_map.get(&black_box(p[i])))
            })
        });
        g.bench_with_input(BenchmarkId::new("hash", case), probe, |b, p| {
            b.iter(|| {
                i = (i + 1) % p.len();
                black_box(hash_map.get(&black_box(p[i])))
            })
        });
    }
    g.finish();
}

fn bench_iter(c: &mut Criterion) {
    for dist in DISTS {
        for pop in [10_000, 1_000_000] {
            let ks = keys(dist, pop);
            let mut expanse = ExpanseMap::new();
            let mut btree = BTreeMap::new();
            for &k in &ks {
                expanse.insert(k, !k);
                btree.insert(k, !k);
            }
            let mut g = c.benchmark_group(format!("map_iter/{dist}/{pop}"));
            g.sample_size(10);
            g.bench_function("expanse", |b| {
                b.iter(|| {
                    let mut sum = 0u64;
                    for (k, v) in expanse.iter() {
                        sum = sum.wrapping_add(k ^ v);
                    }
                    black_box(sum)
                })
            });
            g.bench_function("btree", |b| {
                b.iter(|| {
                    let mut sum = 0u64;
                    for (&k, &v) in btree.iter() {
                        sum = sum.wrapping_add(k ^ v);
                    }
                    black_box(sum)
                })
            });
            g.finish();
        }
    }
}

criterion_group!(
    benches,
    bench_set_lookup,
    bench_map_lookup,
    bench_cold_dram_lookup,
    bench_insert,
    bench_iter
);
criterion_main!(benches);
