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

struct KeyGen<'a> {
    dist: &'a str,
    rng: XorShift,
    i: u64,
    base: u64,
}

impl<'a> KeyGen<'a> {
    fn new(dist: &'a str, seed: u64) -> Self {
        Self {
            dist,
            rng: XorShift(seed),
            i: 0,
            base: 0,
        }
    }

    fn next(&mut self) -> u64 {
        let k = match self.dist {
            "sequential" => self.i,
            "random" => self.rng.next(),
            "clustered" => {
                if self.i.is_multiple_of(256) {
                    self.base = self.rng.next() & !0xFF;
                }
                self.base + (self.i % 256)
            }
            "sparse" => self.i << 40,
            _ => unreachable!(),
        };
        self.i += 1;
        k
    }
}

/// The TESTING.md key-distribution classes.
fn keys(dist: &str, n: usize) -> Vec<u64> {
    let mut g = KeyGen::new(dist, 0x0DDB_1A5E_5EED_0001);
    (0..n).map(|_| g.next()).collect()
}

/// `n` distinct keys that are absent from `present`, drawn from the same
/// generator as the population and rejected on membership (AGENTS.md §8.6).
fn generate_miss_keys(dist: &str, present: &HashSet<u64>, n: usize, seed: u64) -> Vec<u64> {
    let mut g = KeyGen::new(dist, seed);
    let mut seen: HashSet<u64> = HashSet::with_capacity(n * 2);
    let mut out = Vec::with_capacity(n);
    let budget = n.saturating_mul(64).saturating_add(1024);
    for _ in 0..budget {
        if out.len() == n {
            return out;
        }
        let c = g.next();
        if !present.contains(&c) && seen.insert(c) {
            out.push(c);
        }
    }
    panic!("{dist}: could not draw {n} distinct absent keys within budget");
}

/// Probe sets: present keys (hits) and realistic absent keys (misses).
fn probes(dist: &str, keys: &[u64]) -> (Vec<u64>, Vec<u64>) {
    let mut rng = XorShift(0xBEEF_CAFE_1234_5678);
    let hits: Vec<u64> = (0..4096)
        .map(|_| keys[(rng.next() as usize) % keys.len()])
        .collect();
    let present: HashSet<u64> = keys.iter().copied().collect();
    let misses = generate_miss_keys(dist, &present, 4096, 0x51ED_0FF5_C0FF_EE01);
    (hits, misses)
}

const DISTS: [&str; 4] = ["sequential", "random", "clustered", "sparse"];
const POPS: [usize; 2] = [10_000, 1_000_000];

fn bench_set_lookup(c: &mut Criterion) {
    for dist in DISTS {
        for pop in POPS {
            let ks = keys(dist, pop);
            let (hits, misses) = probes(dist, &ks);
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
            let (hits, _) = probes(dist, &ks);
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
            b.iter_batched(
                ExpanseSet::new,
                |mut s| {
                    for &k in &ks {
                        s.insert(black_box(k));
                    }
                    black_box(s)
                },
                criterion::BatchSize::PerIteration,
            )
        });
        g.bench_function("btree", |b| {
            b.iter_batched(
                BTreeSet::new,
                |mut s| {
                    for &k in &ks {
                        s.insert(black_box(k));
                    }
                    black_box(s)
                },
                criterion::BatchSize::PerIteration,
            )
        });
        g.bench_function("hash", |b| {
            b.iter_batched(
                HashSet::new,
                |mut s| {
                    for &k in &ks {
                        s.insert(black_box(k));
                    }
                    black_box(s)
                },
                criterion::BatchSize::PerIteration,
            )
        });
        g.finish();
    }
}

/// Wide probe sets for cold-DRAM measurement (>LLC working set).
fn cold_probes(keys: &[u64], probe_count: usize) -> (Vec<u64>, Vec<u64>) {
    let mut rng = XorShift(0xFEED_FACE_CAFE_BEEF);
    let hits: Vec<u64> = (0..probe_count)
        .map(|_| keys[(rng.next() as usize) % keys.len()])
        .collect();
    let present: HashSet<u64> = keys.iter().copied().collect();
    let misses = generate_miss_keys("random", &present, probe_count, 0xCAFE_BABE_9876_5432);
    (hits, misses)
}

/// Cold-DRAM random point lookup benchmark.
///
/// Designed to strictly exceed the reference host's 30 MiB Last-Level Cache (LLC)
/// across all evaluated data structures:
/// - `ExpanseMap` @ 16.70 B/key (gated) × 4,000,000 keys ≈ 66.8 MiB (~2.2× LLC)
/// - `std::BTreeMap` @ ~36.5 B/key × 4,000,000 keys ≈ 146.0 MiB (~4.8× LLC)
/// - `hashbrown::HashMap` @ ~33.2 B/key × 4,000,000 keys ≈ 132.8 MiB (~4.4× LLC)
/// - Probe array: 2,097,152 keys × 8 B = 16.8 MiB
///
/// Total working set (>83 MiB) guarantees 100% DRAM traversal stalls.
fn bench_cold_dram_lookup(c: &mut Criterion) {
    let pop = 4_000_000;
    let ks = keys("random", pop);
    // 2,097,152 probes (16.8 MiB) randomly distributed across the 4M key expanse.
    let (hits, misses) = cold_probes(&ks, 2_097_152);

    let mut expanse_map = ExpanseMap::new();
    let mut btree_map = BTreeMap::new();
    let mut hash_map = HashMap::new();
    for &k in &ks {
        expanse_map.insert(k, !k);
        btree_map.insert(k, !k);
        hash_map.insert(k, !k);
    }

    let mut g = c.benchmark_group(format!("map_get_cold_dram/random/{pop}"));
    g.sample_size(10);
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
