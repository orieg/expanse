//! The shared workload generator for every pillar.
//!
//! Centralised deliberately. Several of this suite's defects were the kind a
//! shared generator makes once and fixes once: a probe stream that arrived in
//! sorted order, a population that silently shrank, and a keyspace folded to fit
//! a competitor. Each pillar asks this module for a workload rather than rolling
//! its own.

/// XorShift64 — the generator the rest of the repo's comparative suites use, so
/// key streams are reproducible across arms, pillars and runs (§8.3).
pub struct XorShift(u64);

impl XorShift {
    /// The seed `crates/expanse/examples/bytes_per_key.rs` uses, so this suite's
    /// Expanse cells are comparable to the repo's committed table.
    pub const SEED: u64 = 0x0DDB_1A5E_5EED_0001;

    /// Seeds the generator.
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Next raw 64-bit draw.
    #[allow(clippy::should_implement_trait)] // a PRNG step, not an iterator
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// Key distribution shapes, matching `art_comparison/` so the two suites'
/// *Expanse* columns are relatable.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Dist {
    /// `0..n` — dense and contiguous.
    Sequential,
    /// Random 256-key runs.
    Clustered,
    /// `i << 40` — one isolated key per expanse.
    Sparse,
    /// Full-width draws. The only shape whose per-key cost moves with density
    /// (§9.5), so its cells carry λ.
    Random,
}

impl Dist {
    /// The name used in workload IDs and result rows.
    pub fn name(self) -> &'static str {
        match self {
            Dist::Sequential => "sequential",
            Dist::Clustered => "clustered",
            Dist::Sparse => "sparse",
            Dist::Random => "random",
        }
    }
}

/// A population and a probe stream drawn against it.
pub struct Workload {
    /// Distinct keys inserted, sorted.
    pub population: Vec<u64>,
    /// Shuffled probe stream at the requested hit rate.
    pub probes: Vec<u64>,
    /// Keyspace width the keys were drawn from — 63 on Arm A, 64 on Arm B.
    pub keyspace_bits: u32,
}

impl Workload {
    /// Occupancy of a populated 2-byte-prefix expanse.
    ///
    /// The axis the memory pillar publishes against (§9.6), and the reason Arm A's
    /// restricted domain stays comparable to Arm B's: halving the keyspace is
    /// exactly a doubling of density. Only meaningful for [`Dist::Random`]; the
    /// other shapes have construction-fixed occupancy (§9.5).
    pub fn lambda(&self) -> f64 {
        let expanses = 2f64.powi(16 - (64 - self.keyspace_bits as i32));
        self.population.len() as f64 / expanses
    }
}

/// Builds a population and a probe stream.
///
/// `hit_rate` is the fraction of probes drawn from the population; the rest are
/// **rejection-sampled from the same generator** and rejected on membership
/// (§8.6). A miss is never a transform of a present key — a transformed miss
/// lands in a different expanse, shares no prefix with its parent, and so
/// terminates at a systematically different depth than a hit, which averages two
/// different descents into one number.
///
/// The probe stream is shuffled. The population is sorted, so hits drawn by index
/// would arrive in ascending key order and hand an ordered trie cache and
/// prefetch behaviour no real point-lookup workload provides (§9.8).
pub fn build(dist: Dist, n: usize, keyspace_bits: u32, hit_rate: f64) -> Workload {
    assert!(
        (0.0..=1.0).contains(&hit_rate),
        "hit_rate must be a fraction"
    );
    assert!(
        (1..=64).contains(&keyspace_bits),
        "keyspace_bits out of range"
    );
    let mask = if keyspace_bits >= 64 {
        u64::MAX
    } else {
        (1u64 << keyspace_bits) - 1
    };

    let mut rng = XorShift::new(XorShift::SEED);
    let mut population: Vec<u64> = Vec::with_capacity(n);
    let mut base = 0u64;
    for i in 0..n as u64 {
        let k = match dist {
            Dist::Sequential => i,
            Dist::Sparse => i << 40,
            Dist::Clustered => {
                if i % 256 == 0 {
                    base = rng.next() & !0xFF;
                }
                base.wrapping_add(i % 256)
            }
            Dist::Random => rng.next(),
        };
        population.push(k & mask);
    }
    population.sort_unstable();
    population.dedup();

    let want = population.len();
    let hits_wanted = (want as f64 * hit_rate).round() as usize;
    let mut probes = Vec::with_capacity(want);
    for i in 0..want {
        if i < hits_wanted {
            probes.push(population[i % want]);
        } else {
            // Same generator, same distribution, rejected on membership. The
            // offset for the structured shapes starts beyond the population so
            // candidate generation does not collide with every inserted key and
            // exhaust the sampling budget at large N (§8.6).
            let mut guard = 0u32;
            loop {
                let c = match dist {
                    Dist::Sequential => want as u64 + (rng.next() % (want as u64 * 4)),
                    Dist::Sparse => (want as u64 + (rng.next() % (want as u64 * 4))) << 40,
                    _ => rng.next(),
                } & mask;
                if population.binary_search(&c).is_err() {
                    probes.push(c);
                    break;
                }
                guard += 1;
                assert!(
                    guard < 10_000,
                    "miss sampling exhausted for {dist:?} at n={n}: the keyspace is \
                     too dense to reject-sample a miss from the same generator"
                );
            }
        }
    }

    // Fisher-Yates from the same PRNG, so the stream stays reproducible.
    for j in (1..probes.len()).rev() {
        let k = (rng.next() % (j as u64 + 1)) as usize;
        probes.swap(j, k);
    }

    Workload {
        population,
        probes,
        keyspace_bits,
    }
}
