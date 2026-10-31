//! Workload helpers shared by the core crate's benches and examples: the
//! XorShift64 generator, the TESTING.md key-distribution generator
//! ([`KeyGen`]), rejection-sampled absent keys (AGENTS.md §8.6) and the two
//! median reductions.
//!
//! Loaded with `#[path = "workload_common/mod.rs"]` from `benches/` and
//! `#[path = "../benches/workload_common/mod.rs"]` from `examples/`, so Cargo
//! does not register it as a target. It holds generators only, no timing, so
//! it is not a harness and carries no workload shape table.
//!
//! The harnesses this replaced seeded the same step in different ways, and
//! every committed figure they produced was drawn from one of those seed
//! paths. Each path is therefore its own constructor, never a unified one:
//!
//! | Constructor | Seed path | Used by |
//! |---|---|---|
//! | `XorShift(seed)` | the seed as given; 0 stays 0 | most harnesses |
//! | [`XorShift::nonzero_or`] | 0 replaced by a per-harness fallback | `hashbrown_*`, `bench_lookup_compare` |
//! | [`XorShift::odd`] | `seed \| 1` | `reclaim_stall`, `strmap_poll_stall` |
//!
//! `tests/test_workload_common.rs` pins the first keys of every path against
//! the per-file copies as they stood before they were shared.

#![allow(dead_code)]

use std::collections::HashSet;
use std::hash::Hash;

/// The suites' shared population seed.
pub const SEED: u64 = 0x0DDB_1A5E_5EED_0001;

/// Marsaglia's XorShift64 with the (13, 7, 17) triple.
#[derive(Clone, Copy, Debug)]
pub struct XorShift(pub u64);

impl XorShift {
    /// Seeds with `seed`, replacing 0 (a fixed point of the step) by
    /// `fallback`.
    pub fn nonzero_or(seed: u64, fallback: u64) -> Self {
        Self(if seed == 0 { fallback } else { seed })
    }

    /// Seeds with `seed | 1`, so no seed is the fixed point 0.
    pub fn odd(seed: u64) -> Self {
        Self(seed | 1)
    }

    /// The next draw.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// The next draw truncated to its low 32 bits.
    pub fn next_u32(&mut self) -> u32 {
        self.next() as u32
    }

    /// The next draw's top 53 bits as a uniform `f64` in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// The TESTING.md key-distribution classes (`sequential`, `random`,
/// `clustered`, `sparse`) as one generator, resumable from any index so
/// absent keys can be drawn from the population's own generator rather than
/// from a transform of a present key (AGENTS.md §8.6 miss shape).
pub struct KeyGen<'a> {
    dist: &'a str,
    rng: XorShift,
    i: u64,
    base: u64,
}

impl<'a> KeyGen<'a> {
    /// From index 0, with a raw seed.
    pub fn new(dist: &'a str, seed: u64) -> Self {
        Self::with_offset(dist, seed, 0)
    }

    /// From index `offset`, with a raw seed.
    pub fn with_offset(dist: &'a str, seed: u64, offset: u64) -> Self {
        Self::from_rng(dist, XorShift(seed), offset)
    }

    /// From index `offset`, with a generator seeded by the caller.
    pub fn from_rng(dist: &'a str, rng: XorShift, offset: u64) -> Self {
        Self {
            dist,
            rng,
            i: offset,
            base: 0,
        }
    }

    /// The key at the current index.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> u64 {
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
            other => panic!("unknown distribution '{other}'"),
        };
        self.i += 1;
        k
    }
}

/// `n` keys of `dist` from [`SEED`].
pub fn keys(dist: &str, n: usize) -> Vec<u64> {
    let mut g = KeyGen::new(dist, SEED);
    (0..n).map(|_| g.next()).collect()
}

/// `n` distinct candidates from `draw` that are absent from `present`, or
/// `None` if a budget of `64 n + 1024` draws does not yield them. The caller
/// panics, so an exhausted stream aborts loudly (AGENTS.md §8.1).
pub fn absent<T: Copy + Eq + Hash>(
    present: &HashSet<T>,
    n: usize,
    mut draw: impl FnMut() -> T,
) -> Option<Vec<T>> {
    let mut seen: HashSet<T> = HashSet::with_capacity(n * 2);
    let mut out = Vec::with_capacity(n);
    let budget = n.saturating_mul(64).saturating_add(1024);
    for _ in 0..budget {
        if out.len() == n {
            return Some(out);
        }
        let c = draw();
        if !present.contains(&c) && seen.insert(c) {
            out.push(c);
        }
    }
    None
}

/// `n` distinct keys absent from `present`, drawn from `g` and rejected on
/// membership (AGENTS.md §8.6). `g` is positioned by the caller; for the
/// monotonic distributions it starts at an index `>= present.len()`.
pub fn miss_keys_from(g: &mut KeyGen<'_>, present: &HashSet<u64>, n: usize) -> Vec<u64> {
    let dist = g.dist;
    absent(present, n, || g.next())
        .unwrap_or_else(|| panic!("{dist}: could not draw {n} distinct absent keys within budget"))
}

/// [`miss_keys_from`] on a raw-seeded generator resumed at `present.len()`.
pub fn miss_keys(dist: &str, present: &HashSet<u64>, n: usize, seed: u64) -> Vec<u64> {
    miss_keys_from(
        &mut KeyGen::with_offset(dist, seed, present.len() as u64),
        present,
        n,
    )
}

/// Median, averaging the two middle samples of an even count; 0 when empty.
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

/// Median as the upper-middle sample of an even count (the reduction
/// `crates/expanse-capi/examples/bench_vs_libjudy.rs` uses).
pub fn median_upper(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}
