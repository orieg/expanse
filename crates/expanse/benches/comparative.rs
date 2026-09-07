//! Comparative benchmarking of Expanse vs standard and third-party collections.
//!
//! **The key domain is 32-bit, and that is not neutral.** `RoaringBitmap` is
//! definitionally a `u32` set, so pairing against it honestly means restricting
//! the whole cell to a 32-bit domain. Every cast in this file is a widening
//! `k as u64` — nothing is truncated — but zero-extending a `u32` gives every
//! key four leading zero bytes, so the Expanse arm walks four levels of
//! degenerate single-child structure before it reaches a discriminating byte,
//! at an effective density 2^32 above the nominal one. Narrowing the domain by
//! one bit is arithmetically the same as doubling the population for a
//! structure that partitions by key expanse; the sweep that establishes this is
//! `crates/expanse-hot-bench/src/bin/keyspace_density_probe.rs`. The
//! consequence is a scope limit, not a defect: **no cell in this file is
//! comparable to a 64-bit cell anywhere else in the repository.**
//!
//! **`sparse` here does not mean what it means elsewhere.** The generator is a
//! stride of 1000 inside a 32-bit space; every other suite in this repo means
//! `i << 40` by "sparse". Do not read a cross-suite comparison into the label.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `core_comparative` |
//! | `group` | 2 |
//! | `population` | 10k, 100k |
//! | `insertion_order` | generator draw order — the population is inserted as drawn, neither sorted nor shuffled |
//! | `probes_and_reuse` | `pop` pre-built shuffled probes per cell, wrapping counter (no `%`) |
//! | `hit_rate` | 50% (`contains`, `rank`, map lookup); N/A for `select` (probed by index) and `scan` (full iteration) |
//! | `miss_gen_method` | Same generator as the population, rejected on membership, bounded budget that panics on exhaustion; monotonic `sparse`/`dense` start the candidate stream at offset >= N |
//! | `value_dereference` | `black_box(contains)` / `black_box(get().copied())` |
//! | `measured_region` | Clean: containers, key vectors and probe streams built outside `b.iter`; the insert cell uses `iter_batched(PerIteration)` and returns the container so Drop lands post-batch |
//! | `arm_symmetry` | Key and payload widths symmetric (`u64` keys, `u64` values across Expanse / hashbrown / `BTreeMap`); rank aligned (`count_below(k+1)` is exactly Roaring's inclusive `rank(k)`); one shared probe stream per cell. **Disclosed and NOT eliminated**: the key domain is 32-bit because `RoaringBitmap` is definitionally a `u32` set. Nothing is truncated (every cast widens), but zero-extension gives the Expanse arm four degenerate single-child levels before a discriminating byte, a cost Roaring does not pay — so **no cell here is comparable to a 64-bit cell elsewhere in the repo**. `sparse` here is a stride of 1000 in a 32-bit space, not the `i << 40` the rest of the repo means by the word |
//! | `statistics` | Criterion estimate |
//! | `verdict` | ⚠️ **PARTIAL — probe defects fixed, domain asymmetry disclosed** `[verified: CODE READ + RUN]`: was **DEFECT (Class 2, 5)**. Fixed: 100% hit rate on every lookup cell (now a pre-built, pre-shuffled 50/50 stream), the loop-carried `%` inside every timed window, `HashMap<u32, u32>` against `u64`-keyed rivals (§8.16), and `count_below` vs inclusive `rank` comparing two different quantities. **Not fixed**: "32-bit key truncation" was a mis-diagnosis — there is no truncation — and the real 32-bit *domain* asymmetry is inherent to pairing against Roaring, so it is disclosed in `arm_symmetry`, not removed. |

use criterion::{Criterion, criterion_group, criterion_main};
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use hashbrown::HashMap;
use roaring::RoaringBitmap;
use std::collections::{BTreeMap, HashSet};
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

/// The population generator, resumable from an arbitrary index so absent keys
/// can be drawn from the *same* generator rather than from a transform of a
/// present key (§8.6 miss *shape*).
struct KeyGen<'a> {
    dist: &'a str,
    rng: XorShift,
    i: u64,
    base: u32,
}

impl<'a> KeyGen<'a> {
    fn new(dist: &'a str, seed: u64) -> Self {
        Self::with_offset(dist, seed, 0)
    }

    fn with_offset(dist: &'a str, seed: u64, offset: u64) -> Self {
        Self {
            dist,
            rng: XorShift(seed),
            i: offset,
            base: 0,
        }
    }

    fn next(&mut self) -> u32 {
        let i = self.i;
        let k = match self.dist {
            "sparse" => (i as u32).wrapping_mul(1000),
            "clustered" => {
                if i.is_multiple_of(256) {
                    self.base = (self.rng.next() as u32) & !0xFF;
                }
                self.base + (i % 256) as u32
            }
            "dense" => i as u32,
            _ => unreachable!(),
        };
        self.i += 1;
        k
    }
}

fn keys(dist: &str, n: usize) -> Vec<u32> {
    let mut g = KeyGen::new(dist, 0x0DDB_1A5E_5EED_0001);
    (0..n).map(|_| g.next()).collect()
}

/// `n` distinct keys **absent** from `present`, drawn from the same generator
/// as the population and rejected on membership.
///
/// `sparse` and `dense` are monotonic in the generator index and seed-
/// independent, so a rejection loop starting at index 0 would collide with the
/// entire population and burn the budget; both take the §8.6 "Deterministic
/// Distribution Miss Offsets" carve-out and start at offset >= N. `clustered`
/// draws its cluster bases from the RNG, so a different seed at offset 0 lands
/// in different clusters while keeping the population's shape — misses
/// interleave with the population across the whole expanse rather than sitting
/// past its end.
fn miss_keys(dist: &str, present: &HashSet<u32>, n: usize, seed: u64) -> Vec<u32> {
    let offset = match dist {
        "sparse" | "dense" => present.len() as u64,
        _ => 0,
    };
    let mut g = KeyGen::with_offset(dist, seed, offset);
    let mut seen: HashSet<u32> = HashSet::with_capacity(n * 2);
    let mut out = Vec::with_capacity(n);
    // Bounded so a distribution that cannot yield enough absent keys aborts
    // loudly instead of spinning forever (§8.1).
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

/// In-place Fisher-Yates with a fixed seed, so hits and misses interleave and
/// membership is not branch-predictable from probe position.
fn shuffle(v: &mut [u32], seed: u64) {
    let mut rng = XorShift(seed);
    for i in (1..v.len()).rev() {
        v.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
}

/// A `pop`-long 50% hit / 50% miss probe stream, built once per cell and shared
/// by every arm of that cell. Hits are taken with `step_by(2)` so they cover the
/// whole populated expanse rather than its low half.
fn probes(dist: &str, ks: &[u32]) -> Vec<u32> {
    let n_hits = ks.len() / 2;
    let n_misses = ks.len() - n_hits;
    let present: HashSet<u32> = ks.iter().copied().collect();
    let mut out: Vec<u32> = ks.iter().copied().step_by(2).take(n_hits).collect();
    assert_eq!(out.len(), n_hits, "{dist}: short hit stream");
    out.extend(miss_keys(dist, &present, n_misses, 0x51ED_0FF5_C0FF_EE01));
    shuffle(&mut out, 0xBEEF_CAFE_1234_5678);
    out
}

const DISTS: [&str; 3] = ["sparse", "clustered", "dense"];
const POPS: [usize; 2] = [10_000, 100_000];

fn bench_set_comparative(c: &mut Criterion) {
    for dist in DISTS {
        for pop in POPS {
            let ks = keys(dist, pop);
            let ps = probes(dist, &ks);
            let mut expanse = ExpanseSet::new();
            let mut roaring = RoaringBitmap::new();
            for &k in &ks {
                expanse.insert(k as u64);
                roaring.insert(k);
            }
            // Deterministic invariant: both arms hold the same set, so the
            // select cell's index domain is shared (§8.4 — hard asserts belong
            // on deterministic invariants, never on wall-clock estimates).
            assert_eq!(expanse.len(), roaring.len(), "{dist}/{pop}: arms diverge");
            let n_sel = expanse.len() as usize;

            let mut g = c.benchmark_group(format!("comparative_set_contains/{dist}/{pop}"));
            let mut i = 0usize;
            g.bench_function("expanse", |b| {
                b.iter(|| {
                    let k = ps[i];
                    i += 1;
                    if i == ps.len() {
                        i = 0;
                    }
                    black_box(expanse.contains(black_box(k as u64)))
                })
            });
            let mut i2 = 0usize;
            g.bench_function("roaring", |b| {
                b.iter(|| {
                    let k = ps[i2];
                    i2 += 1;
                    if i2 == ps.len() {
                        i2 = 0;
                    }
                    black_box(roaring.contains(black_box(k)))
                })
            });
            g.finish();

            // Roaring's `rank(k)` counts values **<= k**; `count_below(k)`
            // counts keys **< k**. `count_below(k + 1)` is exactly Roaring's
            // quantity in one descent, so the two arms now return the same
            // number for the same probe instead of comparing two different
            // quantities. The probe is a `u64` widened from a `u32`, so the
            // `+ 1` cannot overflow.
            let mut g = c.benchmark_group(format!("comparative_set_rank/{dist}/{pop}"));
            let mut i = 0usize;
            g.bench_function("expanse", |b| {
                b.iter(|| {
                    let k = ps[i];
                    i += 1;
                    if i == ps.len() {
                        i = 0;
                    }
                    black_box(expanse.count_below(black_box(k as u64 + 1)))
                })
            });
            let mut i2 = 0usize;
            g.bench_function("roaring", |b| {
                b.iter(|| {
                    let k = ps[i2];
                    i2 += 1;
                    if i2 == ps.len() {
                        i2 = 0;
                    }
                    black_box(roaring.rank(black_box(k)))
                })
            });
            g.finish();

            // Select is probed by **rank index**, not by key, so a hit rate does
            // not apply: every index in `0..len` is by definition present. The
            // counter walks the shared index domain asserted above.
            let mut g = c.benchmark_group(format!("comparative_set_select/{dist}/{pop}"));
            let mut i = 0usize;
            g.bench_function("expanse", |b| {
                b.iter(|| {
                    let n = i;
                    i += 1;
                    if i == n_sel {
                        i = 0;
                    }
                    black_box(expanse.by_count(black_box(n as u64)))
                })
            });
            let mut i2 = 0usize;
            g.bench_function("roaring", |b| {
                b.iter(|| {
                    let n = i2;
                    i2 += 1;
                    if i2 == n_sel {
                        i2 = 0;
                    }
                    black_box(roaring.select(black_box(n as u32)))
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
            let ps = probes(dist, &ks);

            // §8.16 payload symmetry: all three arms carry `u64` keys and `u64`
            // values. Before #470's remediation the hashbrown arm was a
            // `HashMap<u32, u32>`, hashing and comparing 4-byte keys in 8-byte
            // entries against 8-byte keys in 16-byte entries.
            let mut expanse = ExpanseMap::new();
            let mut hash: HashMap<u64, u64> = HashMap::new();
            let mut btree: BTreeMap<u64, u64> = BTreeMap::new();
            for &k in &ks {
                expanse.insert(k as u64, k as u64);
                hash.insert(k as u64, k as u64);
                btree.insert(k as u64, k as u64);
            }

            let mut g = c.benchmark_group(format!("comparative_map_lookup/{dist}/{pop}"));
            let mut i = 0usize;
            g.bench_function("expanse", |b| {
                b.iter(|| {
                    let k = ps[i];
                    i += 1;
                    if i == ps.len() {
                        i = 0;
                    }
                    black_box(expanse.get(black_box(k as u64)))
                })
            });
            // `.copied()` so both arms yield `Option<u64>` by value; hashbrown's
            // `get` returns `Option<&u64>`.
            let mut i2 = 0usize;
            g.bench_function("hashbrown", |b| {
                b.iter(|| {
                    let k = ps[i2];
                    i2 += 1;
                    if i2 == ps.len() {
                        i2 = 0;
                    }
                    black_box(hash.get(&black_box(k as u64)).copied())
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
                        let mut m: HashMap<u64, u64> = HashMap::new();
                        for &k in &ks {
                            m.insert(black_box(k as u64), black_box(k as u64));
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
