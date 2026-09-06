//! String-key workload generator — the reusable scaffolding #693 exists to add.
//!
//! Every string arm in this crate — HOT's (#693) and Masstree's (#661) — asks
//! this module for its population and probe stream rather than rolling its own. The disciplines are the integer generator's
//! ([`crate::workload`]): one PRNG and seed for every pillar and both sides
//! (§8.3), misses rejection-sampled from the **same** generator and rejected
//! on membership (§8.6), and a shuffled probe stream (§9.8).
//!
//! Two properties are specific to strings and are locked in METHODOLOGY §10:
//!
//! - **Every key is its own NUL-terminated heap allocation** ([`KeyStr`]), as
//!   HOT's own string benchmark allocates its keys. HOT stores the pointer;
//!   Expanse copies the bytes. The census counts these allocations on neither
//!   side (§10.3), and the memory pillar publishes them as a separate column.
//! - **Probe strings are separate allocations from population strings**, even
//!   for hits. HOT confirms a leaf with `strcmp` through the stored pointer; a
//!   probe that *is* the stored pointer would hand that compare an operand it
//!   had just read. A real lookup key arrives in its own buffer.

use std::os::raw::c_char;

use crate::workload::XorShift;

/// The 62 ASCII alphanumerics — 5.95 bits per byte, NUL-free by construction.
const ALNUM: &[u8; 62] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// One key: NUL-free bytes followed by a terminating NUL, in a single heap
/// allocation. [`KeyStr::bytes`] is the Expanse view (no NUL);
/// [`KeyStr::as_ptr`] is the HOT view (C string).
pub struct KeyStr(Box<[u8]>);

impl KeyStr {
    /// Copies `bytes` (which must be NUL-free) into a fresh allocation with a
    /// terminating NUL.
    pub fn new(bytes: &[u8]) -> Self {
        debug_assert!(!bytes.contains(&0), "string keys are NUL-free");
        let mut v = Vec::with_capacity(bytes.len() + 1);
        v.extend_from_slice(bytes);
        v.push(0);
        Self(v.into_boxed_slice())
    }

    /// The key bytes, without the terminator.
    #[inline]
    pub fn bytes(&self) -> &[u8] {
        &self.0[..self.0.len() - 1]
    }

    /// Key length in bytes, without the terminator.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len() - 1
    }

    /// Whether the key is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The NUL-terminated C string HOT reads.
    #[inline]
    pub fn as_ptr(&self) -> *const c_char {
        self.0.as_ptr().cast()
    }

    /// The pointer as a 64-bit word — the value Arm C and Arm E store on both
    /// sides (§10.2), so the two arms' sinks can be compared for equality.
    #[inline]
    pub fn word(&self) -> u64 {
        self.0.as_ptr() as u64
    }

    /// Bytes this key costs as a C string: `len + 1`.
    #[inline]
    pub fn c_len(&self) -> usize {
        self.0.len()
    }
}

/// Key shapes (METHODOLOGY §10.5).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StrDist {
    /// Random alphanumeric bytes, length uniform in `8..=16`.
    Short,
    /// `k` followed by the 11-digit zero-padded decimal of `i` — 12 bytes,
    /// monotone; the string analogue of `sequential`.
    Counter,
    /// A 96-byte alphanumeric prefix drawn once, then 24 random bytes: 120 bytes.
    Prefixed,
    /// Random alphanumeric bytes, length Pareto(α = 1.2, x_min = 4) truncated
    /// at [`SKEWED_MAX_LEN`].
    Skewed,
    /// A 256-byte prefix drawn once, then 16 random bytes: 272 bytes. Every
    /// key exceeds HOT's 255-byte window; every discriminating byte lies past
    /// it. Designed to fail the §10.4 predicate for its whole population.
    Beyond,
}

/// Truncation of the `skewed` length law. A generator parameter, not a HOT
/// accommodation: a run with a longer tail reports its representable fraction
/// (§10.4) rather than trimming the tail.
pub const SKEWED_MAX_LEN: usize = 192;

impl StrDist {
    /// The name used in workload IDs and result rows.
    pub fn name(self) -> &'static str {
        match self {
            StrDist::Short => "short",
            StrDist::Counter => "counter",
            StrDist::Prefixed => "prefixed",
            StrDist::Skewed => "skewed",
            StrDist::Beyond => "beyond",
        }
    }

    /// Parses a result-row name.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "short" => StrDist::Short,
            "counter" => StrDist::Counter,
            "prefixed" => StrDist::Prefixed,
            "skewed" => StrDist::Skewed,
            "beyond" => StrDist::Beyond,
            _ => return None,
        })
    }

    /// All shapes, in the order the suite reports them.
    pub const ALL: [StrDist; 5] = [
        StrDist::Short,
        StrDist::Counter,
        StrDist::Prefixed,
        StrDist::Skewed,
        StrDist::Beyond,
    ];
}

fn alnum(rng: &mut XorShift) -> u8 {
    ALNUM[((rng.next() >> 11) % 62) as usize]
}

fn fill_alnum(rng: &mut XorShift, out: &mut Vec<u8>, n: usize) {
    for _ in 0..n {
        out.push(alnum(rng));
    }
}

/// Uniform in `[0, 1)` from the top 53 bits of a draw.
fn unit(rng: &mut XorShift) -> f64 {
    (rng.next() >> 11) as f64 / (1u64 << 53) as f64
}

/// Pareto(α = 1.2, x_min = 4) length, truncated at `max_len`.
fn skewed_len(rng: &mut XorShift, max_len: usize) -> usize {
    let u = unit(rng);
    let l = 4.0 * (1.0 - u).powf(-1.0 / 1.2);
    (l.floor() as usize).clamp(4, max_len)
}

/// The per-shape generator state: the prefix shapes draw their prefix once.
struct Gen {
    dist: StrDist,
    prefix: Vec<u8>,
    max_len: usize,
}

impl Gen {
    fn new(dist: StrDist, rng: &mut XorShift, max_len: usize) -> Self {
        let mut prefix = Vec::new();
        match dist {
            StrDist::Prefixed => fill_alnum(rng, &mut prefix, 96),
            StrDist::Beyond => fill_alnum(rng, &mut prefix, 256),
            _ => {}
        }
        Self {
            dist,
            prefix,
            max_len,
        }
    }

    /// The `i`-th population key. `counter` is a function of `i`; the random
    /// shapes advance the PRNG.
    fn key(&self, rng: &mut XorShift, i: u64, out: &mut Vec<u8>) {
        out.clear();
        match self.dist {
            StrDist::Short => {
                let n = 8 + (rng.next() % 9) as usize;
                fill_alnum(rng, out, n);
            }
            StrDist::Counter => {
                out.extend_from_slice(format!("k{i:011}").as_bytes());
            }
            StrDist::Prefixed => {
                out.extend_from_slice(&self.prefix);
                fill_alnum(rng, out, 24);
            }
            StrDist::Skewed => {
                let n = skewed_len(rng, self.max_len);
                fill_alnum(rng, out, n);
            }
            StrDist::Beyond => {
                out.extend_from_slice(&self.prefix);
                fill_alnum(rng, out, 16);
            }
        }
    }
}

/// A population and a probe stream drawn against it.
pub struct StringWorkload {
    /// Distinct keys, sorted byte-lexicographically, one allocation each.
    pub population: Vec<KeyStr>,
    /// Shuffled probe stream at the requested hit rate; every probe is its own
    /// allocation, including hits.
    pub probes: Vec<KeyStr>,
    /// Number of probes that are hits.
    pub hits: usize,
    /// The shape.
    pub dist: StrDist,
}

impl StringWorkload {
    /// Mean key length in bytes, without terminators.
    pub fn mean_len(&self) -> f64 {
        let total: usize = self.population.iter().map(KeyStr::len).sum();
        total as f64 / self.population.len().max(1) as f64
    }

    /// Exact external key storage: `Σ (len_i + 1)` over the population.
    pub fn key_bytes(&self) -> usize {
        self.population.iter().map(KeyStr::c_len).sum()
    }

    /// Fraction of the population a competitor can represent, for a predicate
    /// on key length — evaluated against this workload rather than assumed
    /// from the shape. HOT's window (`hot_comparison` §10.4) and Masstree's
    /// `MASSTREE_MAXKEYLEN` (`masstree_comparison` §3.4) are both such
    /// predicates.
    pub fn representable_fraction(&self, can_key: impl Fn(usize) -> bool) -> f64 {
        let ok = self.population.iter().filter(|k| can_key(k.len())).count();
        ok as f64 / self.population.len().max(1) as f64
    }

    /// Number of population keys a competitor cannot represent, for a
    /// predicate on key length.
    pub fn not_representable(&self, can_key: impl Fn(usize) -> bool) -> usize {
        self.population.iter().filter(|k| !can_key(k.len())).count()
    }

    /// Fraction of the population HOT can discriminate (§10.4), evaluated
    /// against this workload rather than assumed from the shape.
    pub fn hot_representable_fraction(&self) -> f64 {
        let ok = self
            .population
            .iter()
            .filter(|k| crate::hot_can_key(k.len()))
            .count();
        ok as f64 / self.population.len().max(1) as f64
    }

    /// Number of population keys HOT cannot discriminate.
    pub fn hot_not_representable(&self) -> usize {
        self.population
            .iter()
            .filter(|k| !crate::hot_can_key(k.len()))
            .count()
    }
}

/// Builds the population only (no probes), for the memory pillar.
pub fn build_population(dist: StrDist, n: usize) -> StringWorkload {
    build_with(dist, n, 0.0, false, SKEWED_MAX_LEN)
}

/// Builds a population and a probe stream at `hit_rate`.
pub fn build(dist: StrDist, n: usize, hit_rate: f64) -> StringWorkload {
    build_with(dist, n, hit_rate, true, SKEWED_MAX_LEN)
}

/// Full-parameter constructor. `max_len` is the `skewed` truncation.
pub fn build_with(
    dist: StrDist,
    n: usize,
    hit_rate: f64,
    with_probes: bool,
    max_len: usize,
) -> StringWorkload {
    assert!(
        (0.0..=1.0).contains(&hit_rate),
        "hit_rate must be a fraction"
    );
    let mut rng = XorShift::new(XorShift::SEED);
    let g = Gen::new(dist, &mut rng, max_len);

    let mut buf = Vec::with_capacity(300);
    let mut population: Vec<KeyStr> = Vec::with_capacity(n);
    for i in 0..n as u64 {
        g.key(&mut rng, i, &mut buf);
        population.push(KeyStr::new(&buf));
    }
    population.sort_by(|a, b| a.bytes().cmp(b.bytes()));
    population.dedup_by(|a, b| a.bytes() == b.bytes());

    let want = population.len();
    let mut probes: Vec<KeyStr> = Vec::new();
    let mut hits = 0usize;
    if with_probes {
        probes.reserve(want);
        let hits_wanted = (want as f64 * hit_rate).round() as usize;
        for i in 0..want {
            if i < hits_wanted {
                // A hit is a fresh copy of a present key, never the stored
                // allocation itself (module docs).
                probes.push(KeyStr::new(population[i].bytes()));
                hits += 1;
            } else {
                // Same generator, same shape, rejected on membership (§8.6).
                // `counter` draws its index from [N, 4N) so candidate
                // generation does not collide with every inserted key.
                let mut guard = 0u32;
                loop {
                    let idx = want as u64 + (rng.next() % (want as u64 * 3));
                    g.key(&mut rng, idx, &mut buf);
                    if population
                        .binary_search_by(|k| k.bytes().cmp(&buf))
                        .is_err()
                    {
                        probes.push(KeyStr::new(&buf));
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
        // Fisher-Yates from the same PRNG (§9.8): the population is sorted, so
        // hits drawn by index would otherwise arrive in ascending key order.
        for j in (1..probes.len()).rev() {
            let k = (rng.next() % (j as u64 + 1)) as usize;
            probes.swap(j, k);
        }
    }

    StringWorkload {
        population,
        probes,
        hits,
        dist,
    }
}

/// A concurrent string arm's workload (`masstree_comparison` §5, MC2): a
/// prefill with its probe stream, and a stream of fresh keys for the writers —
/// the string twin of [`crate::workload::build_concurrent`].
pub struct ConcurrentStringWorkload {
    /// Prefill and probes, exactly as [`build`] produces them.
    pub base: StringWorkload,
    /// For each probe, whether it was drawn from the prefill. Readers verify
    /// that a prefill key is always found; a miss may become a hit as writers
    /// land, identically on both arms.
    pub probe_is_prefill: Vec<bool>,
    /// Fresh keys absent from the prefill, drawn from the same shape generator
    /// and rejected on membership (§8.6), in insertion order. Writers take
    /// contiguous slices.
    pub new_keys: Vec<KeyStr>,
}

/// Builds the concurrent string workload.
///
/// The prefill and probes come from [`build`] with `hit_rate`. The fresh keys
/// come from the **same shape generator** — the prefix shapes re-derive the
/// population's prefix from the suite seed, `counter` continues its index past
/// the prefill — driven by a distinct seed derived from the suite seed so the
/// fresh stream does not replay the prefill draws; every candidate is still
/// rejected on membership rather than trusted to be absent.
pub fn build_concurrent(
    dist: StrDist,
    n_prefill: usize,
    m_new: usize,
    hit_rate: f64,
) -> ConcurrentStringWorkload {
    let base = build(dist, n_prefill, hit_rate);
    let probe_is_prefill = base
        .probes
        .iter()
        .map(|p| {
            base.population
                .binary_search_by(|k| k.bytes().cmp(p.bytes()))
                .is_ok()
        })
        .collect();

    // The population's generator state is reproduced so the prefix shapes
    // share their prefix with the prefill; only the content draws differ.
    let mut seed_rng = XorShift::new(XorShift::SEED);
    let g = Gen::new(dist, &mut seed_rng, SKEWED_MAX_LEN);
    let mut rng = XorShift::new(XorShift::SEED ^ 0x5EED_C0DE_0000_0661);
    let want = base.population.len() as u64;
    let mut buf = Vec::with_capacity(300);
    let mut new_keys: Vec<KeyStr> = Vec::with_capacity(m_new);
    // Fresh keys must also be distinct among themselves, or two writers would
    // race on one key and the arms would grow by different amounts; `counter`
    // draws its index at random and repeats it often enough for this to matter.
    let mut seen: std::collections::BTreeSet<Vec<u8>> = std::collections::BTreeSet::new();
    let mut guard = 0u32;
    while new_keys.len() < m_new {
        let idx = want + (rng.next() % (want.max(1) * 3));
        g.key(&mut rng, idx, &mut buf);
        let in_prefill = base
            .population
            .binary_search_by(|k| k.bytes().cmp(&buf))
            .is_ok();
        if in_prefill || !seen.insert(buf.clone()) {
            guard += 1;
            assert!(
                guard < 10_000_000,
                "fresh-key sampling exhausted for {dist:?} at n={n_prefill}"
            );
            continue;
        }
        new_keys.push(KeyStr::new(&buf));
    }
    ConcurrentStringWorkload {
        base,
        probe_is_prefill,
        new_keys,
    }
}
