//! Interleave-width sweep for the batched descent (issue #430).
//!
//! **What this measures, and why it is wall clock.** Batching does not remove
//! work — it overlaps stalls. `docs/BENCHMARKING.md` ("Which instrument fits
//! the change") puts that in the second row: the instrument is wall clock on
//! the reference host, and a flat or slightly higher instruction count is the
//! expected shape, not a regression. Callgrind's job on this change is only
//! to show the single-key arms did not move.
//!
//! **The sweep is the point.** Too narrow and the dependent miss chains do
//! not overlap; too wide and the interleave exceeds the core's
//! outstanding-miss budget (L1 fill buffers / L2 MSHRs) while still paying
//! the bookkeeping. Neither bound is a number this repo can assert without
//! measuring, so the width ships as a swept parameter and
//! `expanse_trie::get::BATCH_WIDTH` is whatever the sweep supports.
//!
//! **Populations.**
//!
//! * `cold_dram` — 4,000,000 random keys, the population and probe
//!   construction of `compare.rs`'s `bench_cold_dram_lookup`: `ExpanseMap` at
//!   ~16.7 B/key is ~66.8 MiB against the reference host's 30 MiB LLC, so the
//!   descent's loads reach DRAM. Note this working set is also far past STLB
//!   reach, so a result here mixes DRAM latency with page-walk cost (#431);
//!   the two are not separated by this harness.
//! * `warm` — 100,000 keys, a wholly cache-resident control. There are no
//!   miss chains to overlap here, so batching can only cost. An arm that
//!   improves `cold_dram` while regressing `warm` is a trade to state, not a
//!   win.
//!
//! **The twin.** `scalar` is the same probe slice through
//! `ExpanseMap::get` / `ExpanseSet::contains` in a loop, writing the same
//! outputs to the same buffer. `batch/1` is the batched driver reduced to one
//! lane — a different arm from `scalar`, and the one that isolates the
//! driver's own bookkeeping from the interleaving.
//!
//! Run:
//! ```text
//! cargo bench -p expanse-trie --bench batch_lookup
//! ```
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `core_batch_lookup` |
//! | `group` | 2 |
//! | `population` | 100k, 4M |
//! | `probes_and_reuse` | 4M (CHUNK 1024 rolling) |
//! | `hit_rate` | 50% |
//! | `miss_gen_method` | **DEGENERATE XOR**: `k ^ (1<<63) ^ 0xA5` alternating `i % 2` (L72) |
//! | `value_dereference` | `black_box(&out[..CHUNK])` |
//! | `measured_region` | Clean rolling offset |
//! | `arm_symmetry` | Symmetric (scalar vs batch lanes) |
//! | `statistics` | Criterion estimate |
//! | `verdict` | ✅ **RESOLVED in #470 — was DEFECT (Class 3)** `[verified: CODE READ]`: Fixed high-bit XOR alternating pattern corrupts batched descent depth (though #467 confirmed not the root of the 40% jump). |

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use std::collections::HashSet;
use std::hint::black_box;

/// XorShift64 with the seeds `compare.rs` uses, so the two harnesses build
/// the same populations and probe sets (AGENTS.md §8.3).
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

fn random_keys(n: usize) -> Vec<u64> {
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    (0..n).map(|_| rng.next()).collect()
}

/// Draws `n` distinct keys absent from `present` using an independent PRNG stream.
fn generate_miss_keys(present: &HashSet<u64>, n: usize, seed: u64) -> Vec<u64> {
    let mut rng = XorShift(seed);
    let mut seen: HashSet<u64> = HashSet::with_capacity(n * 2);
    let mut out = Vec::with_capacity(n);
    let budget = n.saturating_mul(64).saturating_add(1024);
    for _ in 0..budget {
        if out.len() == n {
            return out;
        }
        let c = rng.next();
        if !present.contains(&c) && seen.insert(c) {
            out.push(c);
        }
    }
    panic!("could not draw {n} distinct absent keys within budget");
}

/// A 50% hit / 50% miss probe stream, interleaved so no interleave width sees
/// a uniform run of one or the other (AGENTS.md §8.6).
fn mixed_probes(keys: &[u64], probe_count: usize) -> Vec<u64> {
    let mut rng = XorShift(0xFEED_FACE_CAFE_BEEF);
    let n_hits = probe_count / 2;
    let n_misses = probe_count - n_hits;
    let present: HashSet<u64> = keys.iter().copied().collect();
    let misses = generate_miss_keys(&present, n_misses, 0x51ED_0FF5_C0FF_EE01);

    let mut out = Vec::with_capacity(probe_count);
    for i in 0..n_hits {
        let hit = keys[(rng.next() as usize) % keys.len()];
        out.push(hit);
        if i < misses.len() {
            out.push(misses[i]);
        }
    }
    out
}

/// Probes consumed per timed iteration. Large enough that the driver spends
/// its time at full width rather than priming and draining, small enough that
/// the slice itself is not the working set.
const CHUNK: usize = 1024;

// Widths swept: 1, 2, 3, 4, 6, 8, 10, 12, 16, 32. `1` is the driver's own
// control; the upper end is past any plausible outstanding-miss budget on the
// reference host, which is the point — the sweep has to contain the turn, not
// stop short of it. They are literals in the `*_width_arm!` invocations
// because `W` is a const generic parameter.

/// Rolling offset so consecutive iterations read different probes; without it
/// the probe slice itself goes resident and the arm stops measuring the trie.
#[inline(always)]
fn advance(off: &mut usize, len: usize) -> usize {
    *off += CHUNK;
    if *off + CHUNK > len {
        *off = 0;
    }
    *off
}

macro_rules! map_width_arm {
    ($g:expr, $map:expr, $probes:expr, $out:expr, $($w:literal),*) => {$({
        let mut off = 0usize;
        $g.bench_with_input(BenchmarkId::new("batch", $w), &$w, |b, _| {
            b.iter(|| {
                let o = advance(&mut off, $probes.len());
                $map.get_batch_width::<$w>(&$probes[o..o + CHUNK], &mut $out[..CHUNK]);
                black_box(&$out[..CHUNK]);
            })
        });
    })*};
}

macro_rules! set_width_arm {
    ($g:expr, $set:expr, $probes:expr, $out:expr, $($w:literal),*) => {$({
        let mut off = 0usize;
        $g.bench_with_input(BenchmarkId::new("batch", $w), &$w, |b, _| {
            b.iter(|| {
                let o = advance(&mut off, $probes.len());
                let n = $set.contains_batch_width::<$w>(&$probes[o..o + CHUNK], &mut $out[..CHUNK]);
                black_box(n);
            })
        });
    })*};
}

fn map_sweep(c: &mut Criterion, label: &str, pop: usize, probe_count: usize) {
    let ks = random_keys(pop);
    let probes = mixed_probes(&ks, probe_count);
    let mut map = ExpanseMap::new();
    for &k in &ks {
        map.insert(k, !k);
    }
    let mut out = vec![None; CHUNK];

    let mut g = c.benchmark_group(format!("map_get_batch/{label}/{pop}"));
    g.throughput(Throughput::Elements(CHUNK as u64));
    g.sample_size(20);

    // The twin: identical probes, identical outputs, single-key path.
    let mut off = 0usize;
    g.bench_function("scalar", |b| {
        b.iter(|| {
            let o = advance(&mut off, probes.len());
            for (i, &k) in probes[o..o + CHUNK].iter().enumerate() {
                out[i] = map.get(black_box(k));
            }
            black_box(&out[..CHUNK]);
        })
    });

    map_width_arm!(g, map, probes, out, 1, 2, 3, 4, 6, 8, 10, 12, 16, 32);
    g.finish();
}

fn set_sweep(c: &mut Criterion, label: &str, pop: usize, probe_count: usize) {
    let ks = random_keys(pop);
    let probes = mixed_probes(&ks, probe_count);
    let mut set = ExpanseSet::new();
    for &k in &ks {
        set.insert(k);
    }
    let mut out = vec![false; CHUNK];

    let mut g = c.benchmark_group(format!("set_contains_batch/{label}/{pop}"));
    g.throughput(Throughput::Elements(CHUNK as u64));
    g.sample_size(20);

    let mut off = 0usize;
    g.bench_function("scalar", |b| {
        b.iter(|| {
            let o = advance(&mut off, probes.len());
            let mut n = 0usize;
            for (i, &k) in probes[o..o + CHUNK].iter().enumerate() {
                let hit = set.contains(black_box(k));
                out[i] = hit;
                n += usize::from(hit);
            }
            black_box(n);
        })
    });

    set_width_arm!(g, set, probes, out, 1, 2, 3, 4, 6, 8, 10, 12, 16, 32);
    g.finish();
}

/// 4M keys, ~66.8 MiB of trie against a 30 MiB LLC — the arm the mechanism
/// is aimed at. Probe array is 2,097,152 × 8 B = 16.8 MiB, matching
/// `compare.rs`'s cold-DRAM arm.
fn bench_cold_dram(c: &mut Criterion) {
    map_sweep(c, "cold_dram", 4_000_000, 2_097_152);
    set_sweep(c, "cold_dram", 4_000_000, 2_097_152);
}

/// 100k keys — the whole structure is cache-resident, so there is nothing to
/// overlap. This arm exists to price the batched path where it cannot win.
fn bench_warm(c: &mut Criterion) {
    map_sweep(c, "warm", 100_000, 262_144);
    set_sweep(c, "warm", 100_000, 262_144);
}

criterion_group!(benches, bench_cold_dram, bench_warm);
criterion_main!(benches);
