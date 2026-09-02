//! Shared utilities and workload generators for the ART vs Expanse benchmark suite
//! (`docs/benchmarks/art_comparison/`, issue #387).
//!
//! Competitor Twins:
//! - [`ExpanseMap`]: Expanse digital trie with 256-bit SIMD/POPCNT bitmap leaves and 16-byte tagged edges.
//! - [`blart::TreeMap`]: Pure-Rust Adaptive Radix Tree (ART, Leis et al. ICDE 2013) with adaptive inner nodes (Node4/16/48/256) and heap LeafNodes.
//! - [`std::collections::BTreeMap`]: Standard library B-Tree baseline.
//! - [`hashbrown::HashMap`]: Unordered SwissTable baseline.

#![allow(dead_code, unused_imports)]

pub use blart::{Mapped, ToUBE, TreeMap as ArtMap};
pub use expanse_trie::map::ExpanseMap;
pub use hashbrown::HashMap;
pub use std::collections::BTreeMap;
use std::collections::HashSet;

/// Fixed PRNG seed shared across all comparative suites
/// (matches `benches/comparative.rs:40` and `benches/batch_lookup.rs:78`).
pub const SHARED_SEED: u64 = 0x0DDB_1A5E_5EED_0001;

/// Secondary PRNG seed for independent miss generation
/// (matches `bench_vs_libjudy.rs:451`).
pub const MISS_SEED: u64 = 0x51ED_0FF5_C0FF_EE01;

/// Dedicated PRNG seed for deterministic probe stream shuffling
/// (eliminates insertion-order cache and branch predictor artifacts).
pub const PROBE_SHUFFLE_SEED: u64 = 0x5EED_511F_F1E0_0001;

/// Deterministic XorShift64 PRNG for identical pseudo-random key generation across arms.
#[derive(Clone)]
pub struct XorShift64(u64);

impl XorShift64 {
    pub fn new(seed: u64) -> Self {
        Self(if seed == 0 { SHARED_SEED } else { seed })
    }

    #[inline]
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform integer in `[0, n)`.
    #[inline]
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// In-place Fisher-Yates shuffle with fixed PRNG so probe access pattern
/// does not match sequential insertion order while remaining 100% deterministic.
pub fn shuffle(v: &mut [u64], rng: &mut XorShift64) {
    for i in (1..v.len()).rev() {
        let j = (rng.next() % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
}

/// Median of a series of samples (for interleaved round timing per §8.4).
pub fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n == 0 {
        0.0
    } else if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

/// Mean of a slice of samples.
pub fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

// ---------------------------------------------------------------------------
// BCa Bootstrap Confidence Interval (AGENTS.md §8.4 / Rule 1.1)
// ---------------------------------------------------------------------------

const RESAMPLES: usize = 2000;

fn norm_cdf(x: f64) -> f64 {
    let z = x.abs() / core::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.5 * z);
    let poly = -z * z - 1.265_512_23
        + t * (1.000_023_68
            + t * (0.374_091_96
                + t * (0.096_784_18
                    + t * (-0.186_288_06
                        + t * (0.278_868_07
                            + t * (-1.135_203_98
                                + t * (1.488_515_87 + t * (-0.822_152_23 + t * 0.170_872_77))))))));
    let erfc_z = t * poly.exp();
    if x >= 0.0 {
        1.0 - 0.5 * erfc_z
    } else {
        0.5 * erfc_z
    }
}

fn norm_ppf(p: f64) -> f64 {
    assert!(p > 0.0 && p < 1.0, "probability must be in (0, 1), got {p}");
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239e0,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838e0,
        -2.549_732_539_343_734e0,
        4.374_664_141_464_968e0,
        2.938_163_982_698_783e0,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996e0,
        3.754_408_661_907_416e0,
    ];
    const P_LOW: f64 = 0.02425;
    if p < P_LOW {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= 1.0 - P_LOW {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    }
}

/// Computes (point_estimate, ci_lower, ci_upper) at 95% confidence using BCa bootstrap.
pub fn bca_ci(data: &[f64]) -> (f64, f64, f64) {
    let n = data.len();
    if n < 3 {
        let avg = if n > 0 {
            data.iter().sum::<f64>() / n as f64
        } else {
            0.0
        };
        return (avg, avg, avg);
    }
    let theta_hat = data.iter().sum::<f64>() / n as f64;

    let mut rng = XorShift64::new(0x0BCA_0BCA_0BCA_0001);
    let mut boot: Vec<f64> = (0..RESAMPLES)
        .map(|_| {
            let s: f64 = (0..n).map(|_| data[(rng.next() % n as u64) as usize]).sum();
            s / n as f64
        })
        .collect();
    boot.sort_by(f64::total_cmp);

    let less = boot.iter().filter(|b| **b < theta_hat).count();
    let prop_less = (less as f64 / RESAMPLES as f64).clamp(1e-6, 1.0 - 1e-6);
    let z0 = norm_ppf(prop_less);

    let total: f64 = data.iter().sum();
    let jack: Vec<f64> = data
        .iter()
        .map(|d| (total - d) / (n as f64 - 1.0))
        .collect();
    let jack_mean = jack.iter().sum::<f64>() / n as f64;
    let num: f64 = jack.iter().map(|j| (jack_mean - j).powi(3)).sum();
    let denom: f64 = jack.iter().map(|j| (jack_mean - j).powi(2)).sum();
    let a = if denom.abs() < 1e-12 {
        0.0
    } else {
        num / (6.0 * denom.powf(1.5))
    };

    let z_alpha = norm_ppf(0.025);
    let z_1_alpha = norm_ppf(0.975);

    let a1 = norm_cdf(z0 + (z0 + z_alpha) / (1.0 - a * (z0 + z_alpha)));
    let a2 = norm_cdf(z0 + (z0 + z_1_alpha) / (1.0 - a * (z0 + z_1_alpha)));

    let idx_lo = ((a1 * RESAMPLES as f64).round() as usize).clamp(0, RESAMPLES - 1);
    let idx_hi = ((a2 * RESAMPLES as f64).round() as usize).clamp(0, RESAMPLES - 1);

    (theta_hat, boot[idx_lo], boot[idx_hi])
}

// ---------------------------------------------------------------------------
// Key Distribution Generators
// ---------------------------------------------------------------------------

/// Sequential keys: `0 .. n`.
pub fn gen_sequential(n: usize) -> Vec<u64> {
    (0..n as u64).collect()
}

/// Clustered keys: dense clusters of 1024 keys separated by sparse random gaps in 64-bit space.
pub fn gen_clustered(n: usize, rng: &mut XorShift64) -> Vec<u64> {
    let mut keys = Vec::with_capacity(n);
    let cluster_size = 1024;
    while keys.len() < n {
        let base = (rng.next() & 0x00FF_FFFF_FFFF_0000) ^ ((keys.len() as u64) << 32);
        let count = (n - keys.len()).min(cluster_size);
        for i in 0..count {
            keys.push(base.wrapping_add(i as u64));
        }
    }
    keys
}

/// Uniform random 64-bit keys (high entropy across full machine-word range).
pub fn gen_uniform_random(n: usize, rng: &mut XorShift64) -> Vec<u64> {
    let mut keys = Vec::with_capacity(n);
    for _ in 0..n {
        keys.push(rng.next());
    }
    keys
}

/// Sparse stride keys: stride of 2^32 across 64-bit key space.
pub fn gen_sparse_stride(n: usize) -> Vec<u64> {
    let mut keys = Vec::with_capacity(n);
    for i in 0..n {
        keys.push((i as u64) << 32);
    }
    keys
}

/// Zipfian power-law distributed keys (skew parameter theta = 0.99).
pub fn gen_zipfian(n: usize, theta: f64, rng: &mut XorShift64) -> Vec<u64> {
    let mut keys = Vec::with_capacity(n);
    let mut sum = 0.0;
    let mut cdf = Vec::with_capacity(n);
    for i in 1..=n {
        sum += 1.0 / (i as f64).powf(theta);
        cdf.push(sum);
    }
    for item in cdf.iter_mut() {
        *item /= sum;
    }
    for _ in 0..n {
        let p = (rng.next() as f64) / (u64::MAX as f64);
        let idx = match cdf.binary_search_by(|v| v.partial_cmp(&p).unwrap()) {
            Ok(i) => i,
            Err(i) => i.min(n - 1),
        };
        keys.push(idx as u64);
    }
    keys
}

/// Deduplicates keys while preserving first-seen order.
pub fn dedupe_preserve_order(keys: &[u64]) -> Vec<u64> {
    let mut seen = HashSet::with_capacity(keys.len());
    let mut out = Vec::with_capacity(keys.len());
    for &k in keys {
        if seen.insert(k) {
            out.push(k);
        }
    }
    out
}

/// Same-distribution rejection-sampling miss generation per AGENTS.md §8.6.
/// Guarantees that miss keys belong to the SAME structural distribution
/// (dense sequential runs, clusters, strides, random) as the population
/// without trivial shallow root termination.
pub fn gen_distribution_misses(
    dist: &str,
    present_keys: &[u64],
    n: usize,
    _rng: &mut XorShift64,
) -> Vec<u64> {
    let present: HashSet<u64> = present_keys.iter().copied().collect();
    let mut misses = Vec::with_capacity(n);

    match dist {
        "sequential" => {
            // Rejection walks the dense generator immediately following the populated block
            let start = present_keys.len() as u64;
            for i in 0..n {
                let candidate = start + i as u64;
                assert!(!present.contains(&candidate));
                misses.push(candidate);
            }
        }
        "sparse_stride" => {
            let start = present_keys.len() as u64;
            for i in 0..n {
                let candidate = (start + i as u64) << 32;
                assert!(!present.contains(&candidate));
                misses.push(candidate);
            }
        }
        "clustered" => {
            let mut miss_rng = XorShift64::new(MISS_SEED);
            let mut seen = HashSet::with_capacity(n * 2);
            let budget = n.saturating_mul(64).saturating_add(1024);
            let mut count = 0;
            while misses.len() < n && count < budget {
                count += 1;
                let cluster_keys = gen_clustered(n.min(1024), &mut miss_rng);
                for k in cluster_keys {
                    if misses.len() == n {
                        break;
                    }
                    if !present.contains(&k) && seen.insert(k) {
                        misses.push(k);
                    }
                }
            }
            if misses.len() < n {
                panic!("clustered: could not draw {n} distinct absent keys");
            }
        }
        "uniform_random" => {
            let mut miss_rng = XorShift64::new(MISS_SEED);
            let mut seen = HashSet::with_capacity(n * 2);
            let budget = n.saturating_mul(64).saturating_add(1024);
            let mut count = 0;
            while misses.len() < n && count < budget {
                count += 1;
                let c = miss_rng.next();
                if !present.contains(&c) && seen.insert(c) {
                    misses.push(c);
                }
            }
            if misses.len() < n {
                panic!("uniform_random: could not draw {n} distinct absent keys");
            }
        }
        "zipfian" => {
            let start = present_keys.len() as u64;
            let mut miss_rng = XorShift64::new(MISS_SEED);
            let zipf = gen_zipfian(n * 2, 0.99, &mut miss_rng);
            let mut seen = HashSet::with_capacity(n);
            for k in zipf {
                let candidate = k.wrapping_add(start);
                if !present.contains(&candidate) && seen.insert(candidate) {
                    misses.push(candidate);
                    if misses.len() == n {
                        break;
                    }
                }
            }
            if misses.len() < n {
                for i in 0..n - misses.len() {
                    let candidate = start.wrapping_add(1_000_000).wrapping_add(i as u64);
                    if !present.contains(&candidate) {
                        misses.push(candidate);
                    }
                }
            }
        }
        _ => panic!("unknown distribution {dist}"),
    }

    misses
}

/// Helper to wrap u64 into blart's zero-allocation big-endian integer key type.
#[inline]
pub fn art_key(k: u64) -> Mapped<ToUBE, u64> {
    Mapped::new(k)
}
