//! Comparative benchmarking of Expanse vs standard and third-party collections.

use criterion::{Criterion, criterion_group, criterion_main};
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use hashbrown::HashMap;
use roaring::RoaringBitmap;
use std::collections::BTreeMap;
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

fn keys(dist: &str, n: usize) -> Vec<u32> {
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut out = Vec::with_capacity(n);
    match dist {
        "sparse" => {
            for i in 0..n {
                out.push((i as u32).wrapping_mul(1000));
            }
        }
        "clustered" => {
            let mut base = 0;
            for i in 0..n {
                if i % 256 == 0 {
                    base = (rng.next() as u32) & !0xFF;
                }
                out.push(base + (i % 256) as u32);
            }
        }
        "dense" => {
            for i in 0..n {
                out.push(i as u32);
            }
        }
        _ => unreachable!(),
    }
    out
}

const DISTS: [&str; 3] = ["sparse", "clustered", "dense"];
const POPS: [usize; 2] = [10_000, 100_000];

fn bench_set_comparative(c: &mut Criterion) {
    for dist in DISTS {
        for pop in POPS {
            let ks = keys(dist, pop);
            let mut expanse = ExpanseSet::new();
            let mut roaring = RoaringBitmap::new();
            for &k in &ks {
                expanse.insert(k as u64);
                roaring.insert(k);
            }

            let mut g = c.benchmark_group(format!("comparative_set_contains/{dist}/{pop}"));
            let mut i = 0;
            g.bench_function("expanse", |b| {
                b.iter(|| {
                    i = (i + 1) % ks.len();
                    black_box(expanse.contains(black_box(ks[i] as u64)))
                })
            });
            let mut i2 = 0;
            g.bench_function("roaring", |b| {
                b.iter(|| {
                    i2 = (i2 + 1) % ks.len();
                    black_box(roaring.contains(black_box(ks[i2])))
                })
            });
            g.finish();

            let mut g = c.benchmark_group(format!("comparative_set_rank/{dist}/{pop}"));
            let mut i = 0;
            g.bench_function("expanse", |b| {
                b.iter(|| {
                    i = (i + 1) % ks.len();
                    black_box(expanse.count_below(black_box(ks[i] as u64)))
                })
            });
            let mut i2 = 0;
            g.bench_function("roaring", |b| {
                b.iter(|| {
                    i2 = (i2 + 1) % ks.len();
                    black_box(roaring.rank(black_box(ks[i2])))
                })
            });
            g.finish();

            let mut g = c.benchmark_group(format!("comparative_set_select/{dist}/{pop}"));
            let mut i = 0;
            g.bench_function("expanse", |b| {
                b.iter(|| {
                    i = (i + 1) % ks.len();
                    black_box(expanse.by_count(black_box(i as u64)))
                })
            });
            let mut i2 = 0;
            g.bench_function("roaring", |b| {
                b.iter(|| {
                    i2 = (i2 + 1) % ks.len();
                    black_box(roaring.select(black_box(i2 as u32)))
                })
            });
            g.finish();
        }
    }
}

fn bench_map_comparative(c: &mut Criterion) {
    for dist in DISTS {
        for pop in POPS {
            let ks = keys(dist, pop);

            let mut expanse = ExpanseMap::new();
            let mut hash = HashMap::new();
            let mut btree = BTreeMap::new();
            for &k in &ks {
                expanse.insert(k as u64, k as u64);
                hash.insert(k, k);
                btree.insert(k as u64, k as u64);
            }

            let mut g = c.benchmark_group(format!("comparative_map_lookup/{dist}/{pop}"));
            let mut i = 0;
            g.bench_function("expanse", |b| {
                b.iter(|| {
                    i = (i + 1) % ks.len();
                    black_box(expanse.get(black_box(ks[i] as u64)))
                })
            });
            let mut i2 = 0;
            g.bench_function("hashbrown", |b| {
                b.iter(|| {
                    i2 = (i2 + 1) % ks.len();
                    black_box(hash.get(&black_box(ks[i2])))
                })
            });
            g.finish();

            let mut g = c.benchmark_group(format!("comparative_map_scan/{dist}/{pop}"));
            g.bench_function("expanse", |b| {
                b.iter(|| {
                    let mut count = 0;
                    for (k, v) in expanse.iter() {
                        black_box((k, v));
                        count += 1;
                    }
                    black_box(count)
                })
            });
            g.bench_function("btree", |b| {
                b.iter(|| {
                    let mut count = 0;
                    for (k, v) in btree.iter() {
                        black_box((k, v));
                        count += 1;
                    }
                    black_box(count)
                })
            });
            g.finish();

            // Whole-build insert cell (#375): each iteration constructs the
            // container and inserts the FULL key set, so the dist/pop labels
            // describe what is actually measured. `iter_batched` keeps the
            // key-Vec clone in setup and, by returning the built container
            // (plus the consumed Vec), hands both to criterion's post-batch
            // drop — teardown stays outside the timed routine.
            // `BatchSize::PerIteration` bounds live memory to one container.
            // (Before #375 this cell timed ONE insert into a freshly
            // allocated empty container per iteration, comparing "allocate a
            // hashtable" vs "write a tagged word" under fictional dist/pop
            // labels.)
            let mut g = c.benchmark_group(format!("comparative_map_insert/{dist}/{pop}"));
            g.throughput(criterion::Throughput::Elements(pop as u64));
            g.bench_function("expanse", |b| {
                b.iter_batched(
                    || ks.clone(),
                    |ks| {
                        let mut m = ExpanseMap::new();
                        for &k in &ks {
                            m.insert(black_box(k as u64), black_box(k as u64));
                        }
                        (m, ks)
                    },
                    criterion::BatchSize::PerIteration,
                )
            });
            g.bench_function("hashbrown", |b| {
                b.iter_batched(
                    || ks.clone(),
                    |ks| {
                        let mut m = HashMap::new();
                        for &k in &ks {
                            m.insert(black_box(k), black_box(k));
                        }
                        (m, ks)
                    },
                    criterion::BatchSize::PerIteration,
                )
            });
            g.finish();
        }
    }
}

criterion_group!(benches, bench_set_comparative, bench_map_comparative);
criterion_main!(benches);
