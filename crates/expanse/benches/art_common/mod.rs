//! Shared utilities and workload generators for the ART vs Expanse benchmark suite
//! (`docs/benchmarks/art_comparison/`, issue #387).
//!
//! Competitor Twins:
//! - [`ExpanseMap`]: Expanse digital trie with 256-bit SIMD/POPCNT bitmap leaves and 16-byte tagged edges.
//! - [`blart::TreeMap`]: Pure-Rust Adaptive Radix Tree (ART, Leis et al. ICDE 2013) with adaptive inner nodes (Node4/16/48/256) and heap LeafNodes.
//! - [`std::collections::BTreeMap`]: Standard library B-Tree baseline.
//! - [`hashbrown::HashMap`]: Unordered SwissTable baseline.

#![allow(dead_code)]

pub use blart::{Mapped, ToUBE, TreeMap as ArtMap};
pub use expanse_trie::map::ExpanseMap;
pub use hashbrown::HashMap;
pub use std::collections::BTreeMap;

/// Fixed PRNG seed shared across all comparative suites
/// (matches `benches/comparative.rs:40` and `benches/batch_lookup.rs:78`).
pub const SHARED_SEED: u64 = 0x0DDB_1A5E_5EED_0001;

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

/// Miss generation using rejection sampling per AGENTS.md §8.6.
/// Generates `n` keys that are guaranteed absent from `present_keys`.
pub fn gen_rejection_misses(present_keys: &[u64], n: usize, rng: &mut XorShift64) -> Vec<u64> {
    let set: HashMap<u64, ()> = present_keys.iter().map(|&k| (k, ())).collect();
    let mut misses = Vec::with_capacity(n);
    while misses.len() < n {
        let candidate = rng.next();
        if !set.contains_key(&candidate) {
            misses.push(candidate);
        }
    }
    misses
}

/// Helper to wrap u64 into blart's zero-allocation big-endian integer key type.
#[inline]
pub fn art_key(k: u64) -> Mapped<ToUBE, u64> {
    Mapped::new(k)
}
