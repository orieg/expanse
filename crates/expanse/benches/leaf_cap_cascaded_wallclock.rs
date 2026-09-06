//! Wall-clock twin of `leaf_cap_cascaded.rs`: `ExpanseSet::contains` and
//! `ExpanseMap::get`, hit and miss, over the same 1,000,000 keys at 63 bits
//! (λ = 30.52), in the `compare.rs` `set_lookup` shape (#715).
//!
//! `docs/BENCHMARKING.md` rule 16: a random point-lookup claim is decided by
//! wall clock and hardware counters, with intervals; the Callgrind arm beside
//! this one publishes the instruction count as cost, not verdict. This harness
//! is the wall-clock half of the `LEAF_CAP` 32-vs-48 pair in the cascaded
//! regime. On its own it measures whatever build it was compiled into; the
//! pair is produced by
//! `docs/benchmarks/hot_comparison/scripts/leaf_cap_cascaded_wallclock.sh`,
//! which builds the shipped cap and a `LEAF_CAP = 48` build-time patch,
//! interleaves them A/B/A/B under the host-wide bench lock and the P-core pin
//! (`scripts/bench_pin.sh`, AGENTS.md §8.4), opens with a same-build A/A repeat
//! so the drift of one binary between rounds is bounded before any
//! between-build difference is read, and harvests BCa 95% intervals through
//! `scripts/bench_baseline.py`. Do not run the pair on a shared host or
//! without the pin: the reference host's E-cores are 1.576× slower on this
//! very arm, and a migration is a phantom regression of that size.
//!
//! Same generator, seed, width and miss sampling as the Callgrind twin, so the
//! two instruments describe one structure. 65,536 probes per case, cycled;
//! the population (13.6 MB at cap 32, 7.0 MB at cap 48, set flavor) is what
//! the probes walk, and it is the object under test.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `core_leaf_cap_cascaded_wallclock` |
//! | `group` | 2 |
//! | `population` | 1,000,000 uniform random keys masked to 63 bits (λ = N / 2^(63−48) = 30.52) |
//! | `probes_and_reuse` | 65,536 hit probes (drawn from the population under a fixed seed) and 65,536 miss probes, cycled by criterion; reuse > 1 |
//! | `hit_rate` | 100% on the `hit` arms, 0% on the `miss` arms (separate arms) |
//! | `miss_gen_method` | Rejection-sampled from the same XorShift64 generator at the same 63-bit width under a second seed, rejected on membership and deduplicated (§8.6) |
//! | `value_dereference` | `black_box(contains(k))` / `black_box(get(k))` |
//! | `measured_region` | Clean (build and probe generation before the group; `b.iter` holds one lookup) |
//! | `arm_symmetry` | Set and map arms probe bit-identical key vectors; across a cap-32 / cap-48 pair the only variable is `LEAF_CAP` (build-time patch), the same binary layout otherwise |
//! | `statistics` | criterion, n = 100 samples per arm; BCa 95% via `scripts/bench_baseline.py` |
//! | `verdict` | **PASS** `[verified: RUN (fa1704d4, hybrid reference host, P-core pinned)]`: five-run pair published in `docs/benchmarks/hot_comparison/results/leaf_cap_cascaded_contains_cap{32,48}_{a,repeat,b}.json`; cap 48 at 0.82–0.86× of cap 32 on every arm, same-build repeat within 0.5% (METHODOLOGY §9.10.5). |

#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use std::collections::HashSet;
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

const SEED: u64 = 0x0DDB_1A5E_5EED_0001;
const MISS_SEED: u64 = 0x51ED_0FF5_C0FF_EE01;
const HIT_PICK_SEED: u64 = 0xBEEF_CAFE_1234_5678;

const POP: usize = 1_000_000;
const BITS: u32 = 63;
const MASK: u64 = (1u64 << BITS) - 1;
const PROBES: usize = 65_536;

fn keys() -> Vec<u64> {
    let mut rng = XorShift(SEED);
    (0..POP).map(|_| rng.next() & MASK).collect()
}

/// `n` distinct keys absent from `present`, same generator and width,
/// rejected on membership (AGENTS.md §8.6).
fn miss_keys(present: &HashSet<u64>, n: usize) -> Vec<u64> {
    let mut rng = XorShift(MISS_SEED);
    let mut seen: HashSet<u64> = HashSet::with_capacity(n * 2);
    let mut out = Vec::with_capacity(n);
    let budget = n.saturating_mul(64).saturating_add(1024);
    for _ in 0..budget {
        if out.len() == n {
            return out;
        }
        let c = rng.next() & MASK;
        if !present.contains(&c) && seen.insert(c) {
            out.push(c);
        }
    }
    panic!("could not draw {n} distinct absent keys within budget");
}

/// (hits, misses): present keys picked under a fixed seed, absent keys
/// rejection-sampled — the `compare.rs` `probes` shape.
fn probes(ks: &[u64]) -> (Vec<u64>, Vec<u64>) {
    let mut rng = XorShift(HIT_PICK_SEED);
    let hits: Vec<u64> = (0..PROBES)
        .map(|_| ks[(rng.next() as usize) % ks.len()])
        .collect();
    let present: HashSet<u64> = ks.iter().copied().collect();
    let misses = miss_keys(&present, PROBES);
    (hits, misses)
}

fn bench_cascaded_lookup(c: &mut Criterion) {
    let ks = keys();
    let (hits, misses) = probes(&ks);
    let mut set = ExpanseSet::new();
    let mut map = ExpanseMap::new();
    for &k in &ks {
        set.insert(k);
        map.insert(k, !k);
    }

    let mut g = c.benchmark_group("set_lookup_cascaded/random/1000000_63bit");
    for (case, probe) in [("hit", &hits), ("miss", &misses)] {
        let mut i = 0;
        g.bench_with_input(BenchmarkId::new("expanse", case), probe, |b, p| {
            b.iter(|| {
                i = (i + 1) % p.len();
                black_box(set.contains(black_box(p[i])))
            })
        });
    }
    g.finish();

    let mut g = c.benchmark_group("map_lookup_cascaded/random/1000000_63bit");
    for (case, probe) in [("hit", &hits), ("miss", &misses)] {
        let mut i = 0;
        g.bench_with_input(BenchmarkId::new("expanse", case), probe, |b, p| {
            b.iter(|| {
                i = (i + 1) % p.len();
                black_box(map.get(black_box(p[i])))
            })
        });
    }
    g.finish();

    // Leaked deliberately: teardown of a 1M-key structure is not part of any
    // arm, and the process exits right after (`docs/BENCHMARKING.md`,
    // measured-region hygiene).
    core::mem::forget(set);
    core::mem::forget(map);
}

criterion_group!(benches, bench_cascaded_lookup);
criterion_main!(benches);
