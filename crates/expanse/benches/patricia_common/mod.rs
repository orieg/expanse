//! Shared machinery for the Patricia / radix trie vs Expanse suite
//! (`docs/benchmarks/patricia_comparison/`).
//!
//! Twins (all pinned in `crates/expanse/Cargo.toml`):
//! - [`PatriciaMap`] (`patricia_tree` 0.10.2): byte-labelled radix tree whose
//!   children are a byte-sorted, singly linked sibling list walked linearly.
//! - [`RadixMap`] (`fast_radix_trie` 1.2.0): a fork of `patricia_tree` that
//!   stores each node's child pointers inline in an array. Its child count is a
//!   `u8`, so a node needing 256 children panics (default `realloc` feature) —
//!   every cell passes it through [`build_twin`], which records such a cell as
//!   invalid instead of timing it.
//! - [`QpTrie`] (`qp-trie` 0.8.2): a QP-trie, branching on 4-bit nybbles through
//!   a popcount-indexed sparse child array. It owns its keys, and its iteration
//!   order is not byte-lexicographic (it takes the low nybble of a byte first).
//!
//! Key generators, PRNG and BCa come from `art_common`. Integer keys reach the
//! twins as their big-endian 8 bytes, so byte order is numeric order.

#![allow(dead_code)]

pub use fast_radix_trie::RadixMap;
pub use patricia_tree::PatriciaMap;
pub use qp_trie::Trie as QpTrie;

use crate::art_common::{
    ExpanseMap, PROBE_SHUFFLE_SEED, SHARED_SEED, XorShift64, bca_ci_labeled, dedupe_preserve_order,
    gen_clustered, gen_sequential, gen_sparse_stride, gen_uniform_random, gen_zipfian, median,
    rounds_raw, shuffle,
};
use expanse_trie::strmap::{ExpanseStrMap, NulFreeStr};
use serde_json::{Map, Value, json};
use std::hint::black_box;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Keys
// ---------------------------------------------------------------------------

/// The five `u64` distributions, in the order every harness runs them.
pub const DISTS: [&str; 5] = [
    "sequential",
    "clustered",
    "uniform_random",
    "sparse_stride",
    "zipfian",
];

/// Seed for the random half-split of [`split_half`].
pub const SPLIT_SEED: u64 = 0x5B11_7000_0000_0001;
/// Seed for the string-key generator.
pub const STRING_SEED: u64 = 0x5A71_C1A0_0000_0001;
/// Seed for string-key misses (same generator, independent stream).
pub const STRING_MISS_SEED: u64 = 0x5A71_C1A0_0000_0002;

/// Big-endian key bytes for a `u64`.
#[inline(always)]
pub fn pkey(k: u64) -> [u8; 8] {
    k.to_be_bytes()
}

/// Value stored for a `u64` key, on every arm.
#[inline(always)]
pub fn val(k: u64) -> u64 {
    k.wrapping_mul(3)
}

/// Distinct keys of `n` draws from `dist`, in first-seen (generator) order.
///
/// Every call starts a fresh `SHARED_SEED` stream, so a cell's keys depend on
/// `(dist, n)` alone and `scripts/patricia_envelope.py` can reproduce them.
/// `zipfian` names the key *set* (distinct ranks of `n` Zipf(0.99) draws over
/// `[0, n)`), not a skewed access pattern: probes visit keys uniformly.
pub fn u64_dist(dist: &str, n: usize) -> Vec<u64> {
    let mut rng = XorShift64::new(SHARED_SEED);
    let raw = match dist {
        "sequential" => gen_sequential(n),
        "clustered" => gen_clustered(n, &mut rng),
        "uniform_random" => gen_uniform_random(n, &mut rng),
        "sparse_stride" => gen_sparse_stride(n),
        "zipfian" => gen_zipfian(n, 0.99, &mut rng),
        _ => panic!("unknown distribution {dist}"),
    };
    dedupe_preserve_order(&raw)
}

/// Population and misses for a 50/50 cell with the miss *shape* of a hit
/// (AGENTS.md §8.6): draw `2n` from the generator, split the distinct keys at
/// random into halves, keep one half (in generator order) as the population and
/// use the other as misses. Misses then lie inside the populated range and end
/// at the same depths as hits — a dense generator has no in-range misses
/// otherwise. The population is therefore a random half of the `2n` draw, not
/// the `n`-key distribution the other harnesses use; rows say so.
pub fn split_half(dist: &str, n: usize) -> (Vec<u64>, Vec<u64>) {
    let cand = u64_dist(dist, 2 * n);
    let mut idx: Vec<u64> = (0..cand.len() as u64).collect();
    shuffle(&mut idx, &mut XorShift64::new(SPLIT_SEED));
    let half = cand.len() / 2;
    let mut in_pop = vec![false; cand.len()];
    for &i in &idx[..half] {
        in_pop[i as usize] = true;
    }
    let pop = cand
        .iter()
        .zip(&in_pop)
        .filter(|(_, p)| **p)
        .map(|(k, _)| *k)
        .collect();
    let misses = idx[half..].iter().map(|&i| cand[i as usize]).collect();
    (pop, misses)
}

/// `keys` in a Fisher–Yates permutation under `PROBE_SHUFFLE_SEED`.
pub fn shuffled<T: Clone>(keys: &[T]) -> Vec<T> {
    let mut idx: Vec<u64> = (0..keys.len() as u64).collect();
    shuffle(&mut idx, &mut XorShift64::new(PROBE_SHUFFLE_SEED));
    idx.iter().map(|&i| keys[i as usize].clone()).collect()
}

/// Shared prefix of length `len` for the string keys: `https://example.com/…`,
/// truncated or extended with `seg/` segments. The 35-byte form is
/// `https://example.com/api/v2/objects/`.
pub fn path_prefix(len: usize) -> Vec<u8> {
    let base = b"https://example.com/api/v2/objects/";
    let mut p = base.to_vec();
    while p.len() < len {
        p.extend_from_slice(b"seg/");
    }
    p.truncate(len);
    p
}

/// Prefix lengths swept by the string cells; the key adds 12 hex digits, so
/// the longest key is 252 bytes, inside every twin's 255-byte label limit.
pub const PREFIX_LENS: [usize; 4] = [8, 35, 128, 240];

/// `n` distinct shared-prefix keys: `prefix ++ <12 hex digits of 48 random bits>`.
pub fn gen_paths(n: usize, prefix_len: usize, seed: u64) -> Vec<Vec<u8>> {
    gen_paths_excluding(n, prefix_len, seed, &Default::default())
}

fn gen_paths_excluding(
    n: usize,
    prefix_len: usize,
    seed: u64,
    exclude: &std::collections::HashSet<u64>,
) -> Vec<Vec<u8>> {
    let prefix = path_prefix(prefix_len);
    let mut rng = XorShift64::new(seed);
    let mut seen = std::collections::HashSet::with_capacity(n);
    let mut out = Vec::with_capacity(n);
    let budget = n.saturating_mul(64).saturating_add(1024);
    let mut tries = 0usize;
    while out.len() < n {
        tries += 1;
        assert!(tries <= budget, "could not draw {n} distinct path keys");
        let id = rng.next() & 0xFFFF_FFFF_FFFF;
        if !exclude.contains(&id) && seen.insert(id) {
            let mut k = prefix.clone();
            k.extend_from_slice(format!("{id:012x}").as_bytes());
            out.push(k);
        }
    }
    out
}

/// Same-generator misses for [`gen_paths`], rejected on membership (§8.6): same
/// prefix, same length, ids from an independent stream.
pub fn gen_path_misses(present: &[Vec<u8>], n: usize, prefix_len: usize) -> Vec<Vec<u8>> {
    let ids: std::collections::HashSet<u64> = present.iter().map(|k| path_val(k)).collect();
    gen_paths_excluding(n, prefix_len, STRING_MISS_SEED, &ids)
}

/// Validates every key once, outside any timed region, so the Expanse string
/// arm pays no NUL scan per probe that the twins do not.
pub fn as_nulfree(keys: &[Vec<u8>]) -> Vec<&NulFreeStr> {
    keys.iter()
        .map(|k| NulFreeStr::new(k).expect("generated keys are NUL-free"))
        .collect()
}

// ---------------------------------------------------------------------------
// Twins
// ---------------------------------------------------------------------------

/// The operations every twin exposes, over a stored key type `K`.
pub trait Twin<K>: Sized {
    /// Arm name used in every artifact column.
    const NAME: &'static str;
    /// Largest child count one node can hold, where the crate has a limit
    /// below the 256 a byte can branch into.
    const MAX_CHILDREN: Option<usize> = None;
    /// Empty structure.
    fn empty() -> Self;
    /// Insert, returning the previous value.
    fn put(&mut self, k: &K, v: u64) -> Option<u64>;
    /// Point lookup.
    fn find(&self, k: &K) -> Option<u64>;
    /// Entry count as the structure reports it.
    fn count(&self) -> usize;
    /// Sum of every value, visiting every entry once (a full traversal).
    fn sum_all(&self) -> u64;
    /// Sum of the values of every key starting with `prefix`, through the
    /// crate's public prefix-read API.
    fn sum_prefix(&self, prefix: &[u8]) -> u64;
}

macro_rules! radix_twin {
    ($t:ty, $name:expr, $max:expr) => {
        impl<K: AsRef<[u8]>> Twin<K> for $t {
            const NAME: &'static str = $name;
            const MAX_CHILDREN: Option<usize> = $max;
            fn empty() -> Self {
                <$t>::new()
            }
            #[inline(always)]
            fn put(&mut self, k: &K, v: u64) -> Option<u64> {
                self.insert(k.as_ref(), v)
            }
            #[inline(always)]
            fn find(&self, k: &K) -> Option<u64> {
                self.get(k.as_ref()).copied()
            }
            fn count(&self) -> usize {
                self.len()
            }
            fn sum_all(&self) -> u64 {
                self.values().fold(0u64, |a, v| a.wrapping_add(*v))
            }
            fn sum_prefix(&self, prefix: &[u8]) -> u64 {
                // The only public prefix read on this crate yields an owned,
                // reconstructed key per entry; that cost is part of the API.
                self.iter_prefix(prefix)
                    .fold(0u64, |a, (_, v)| a.wrapping_add(*v))
            }
        }
    };
}
radix_twin!(PatriciaMap<u64>, "patricia_tree", None);
radix_twin!(RadixMap<u64>, "fast_radix_trie", Some(255));

macro_rules! qp_twin {
    ($k:ty) => {
        impl Twin<$k> for QpTrie<$k, u64> {
            const NAME: &'static str = "qp_trie";
            fn empty() -> Self {
                QpTrie::new()
            }
            #[inline(always)]
            fn put(&mut self, k: &$k, v: u64) -> Option<u64> {
                // qp-trie owns its keys: an inline copy for `[u8; 8]`, a heap
                // copy for `Vec<u8>` — the crate's real insertion cost.
                self.insert(k.clone(), v)
            }
            #[inline(always)]
            fn find(&self, k: &$k) -> Option<u64> {
                self.get(&k[..]).copied()
            }
            fn count(&self) -> usize {
                QpTrie::count(self)
            }
            fn sum_all(&self) -> u64 {
                self.values().fold(0u64, |a, v| a.wrapping_add(*v))
            }
            fn sum_prefix(&self, prefix: &[u8]) -> u64 {
                self.iter_prefix(prefix)
                    .fold(0u64, |a, (_, v)| a.wrapping_add(*v))
            }
        }
    };
}
qp_twin!([u8; 8]);
qp_twin!(Vec<u8>);

/// Largest number of distinct next bytes under any shared prefix of `keys`:
/// the child count the widest node of a byte-branching compressed trie over
/// these keys has (`max_fanout` in `scripts/patricia_envelope.py`).
pub fn max_fanout<K: AsRef<[u8]>>(keys: &[K]) -> usize {
    let mut ks: Vec<&[u8]> = keys.iter().map(|k| k.as_ref()).collect();
    ks.sort_unstable();
    ks.dedup();
    // lcp[i] = common prefix length of ks[i - 1] and ks[i]. Under a shared
    // prefix of length d, a node's children are the runs of equal byte d; a
    // new child starts exactly where lcp == d, and the node ends where lcp < d.
    let lcp: Vec<usize> = (1..ks.len())
        .map(|i| {
            ks[i - 1]
                .iter()
                .zip(ks[i])
                .take_while(|(a, b)| a == b)
                .count()
        })
        .collect();
    let deepest = lcp.iter().copied().max().unwrap_or(0);
    let mut best = usize::from(!ks.is_empty());
    for d in 0..=deepest {
        let mut children = usize::from(ks.first().is_some_and(|k| k.len() > d));
        for (i, &l) in lcp.iter().enumerate() {
            let longer = ks[i + 1].len() > d;
            if l < d {
                children = usize::from(longer);
            } else if l == d && longer {
                children += 1;
            }
            best = best.max(children);
        }
    }
    best
}

/// Builds a twin from `keys` (inserted in slice order) and checks it before any
/// timing: no panic, `count() == keys.len()`, and every key reads back its
/// value. A failing twin is returned as `Err(reason)` and the cell records it
/// as invalid — it is never timed and never silently dropped (§8.1).
///
/// A twin with a child-count limit below the key set's widest node is not
/// built at all: `fast_radix_trie`'s failure there is a panic inside its own
/// unsafe node code whose unwind aborts the process, so it cannot be caught.
pub fn build_twin<K: AsRef<[u8]>, T: Twin<K>>(keys: &[K], vals: &[u64]) -> Result<T, String> {
    if let Some(limit) = T::MAX_CHILDREN {
        let widest = max_fanout(keys);
        if widest > limit {
            return Err(format!(
                "not built: the key set needs a node with {widest} children and the crate holds at most {limit} (u8 child count)"
            ));
        }
    }
    let built = catch_unwind(AssertUnwindSafe(|| {
        let mut t = T::empty();
        for (k, &v) in keys.iter().zip(vals) {
            t.put(k, v);
        }
        t
    }));
    let t = match built {
        Ok(t) => t,
        Err(p) => {
            let msg = p
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| p.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic payload".into());
            return Err(format!("panicked during build: {msg}"));
        }
    };
    let lost = keys
        .iter()
        .zip(vals)
        .filter(|(k, v)| t.find(k) != Some(**v))
        .count();
    if t.count() != keys.len() || lost > 0 {
        return Err(format!(
            "count() = {} for {} keys; {lost} keys do not read back",
            t.count(),
            keys.len()
        ));
    }
    Ok(t)
}

/// Expanse `u64` build, checked like [`build_twin`]. A failure here is an
/// engine defect, so it panics rather than being recorded.
pub fn build_expanse(keys: &[u64]) -> ExpanseMap {
    let mut m = ExpanseMap::new();
    for &k in keys {
        m.insert(k, val(k));
    }
    assert_eq!(
        m.len() as usize,
        keys.len(),
        "ExpanseMap population mismatch"
    );
    let lost = keys.iter().filter(|&&k| m.get(k) != Some(val(k))).count();
    assert_eq!(lost, 0, "ExpanseMap lost keys");
    m
}

/// Value stored for a path key on every arm: its 48-bit id, so it does not
/// depend on build order.
pub fn path_val(k: &[u8]) -> u64 {
    let hex = std::str::from_utf8(&k[k.len() - 12..]).expect("hex suffix");
    u64::from_str_radix(hex, 16).expect("hex id")
}

/// Expanse string build, checked like [`build_twin`]; panics on a defect.
pub fn build_expanse_str(keys: &[&NulFreeStr], vals: &[u64]) -> ExpanseStrMap {
    let mut m = ExpanseStrMap::new();
    for (k, &v) in keys.iter().zip(vals) {
        m.insert(k, v);
    }
    let lost = keys
        .iter()
        .zip(vals)
        .filter(|(k, v)| m.get(k) != Some(**v))
        .count();
    assert_eq!(lost, 0, "ExpanseStrMap lost keys");
    m
}

/// Hits a lookup arm returns over `probes`; every valid arm must agree before
/// the cell is timed.
pub fn hit_count<K, T: Twin<K>>(t: &T, probes: &[K]) -> usize {
    probes.iter().filter(|k| t.find(k).is_some()).count()
}

// ---------------------------------------------------------------------------
// Timed kernels (monomorphised per arm; every result reaches `black_box`)
// ---------------------------------------------------------------------------

#[inline(never)]
pub fn pass_expanse_get(m: &ExpanseMap, probes: &[u64], reps: usize) -> Duration {
    let start = Instant::now();
    for _ in 0..reps {
        for &k in probes {
            black_box(m.get(k).unwrap_or(0));
        }
    }
    start.elapsed()
}

#[inline(never)]
pub fn pass_expanse_str_get(m: &ExpanseStrMap, probes: &[&NulFreeStr], reps: usize) -> Duration {
    let start = Instant::now();
    for _ in 0..reps {
        for k in probes {
            black_box(m.get(k).unwrap_or(0));
        }
    }
    start.elapsed()
}

#[inline(never)]
pub fn pass_twin_get<K, T: Twin<K>>(t: &T, probes: &[K], reps: usize) -> Duration {
    let start = Instant::now();
    for _ in 0..reps {
        for k in probes {
            black_box(t.find(k).unwrap_or(0));
        }
    }
    start.elapsed()
}

/// `reps` cold builds; only the insert loops are timed, construction of the
/// empty map precedes each window and the drop follows it.
#[inline(never)]
pub fn pass_expanse_insert(keys: &[u64], reps: usize) -> Duration {
    let mut total = Duration::ZERO;
    for _ in 0..reps {
        let mut m = ExpanseMap::new();
        let start = Instant::now();
        for &k in keys {
            black_box(m.insert(k, val(k)));
        }
        total += start.elapsed();
        black_box(&m);
        drop(m);
    }
    total
}

#[inline(never)]
pub fn pass_twin_insert<K, T: Twin<K>>(keys: &[K], vals: &[u64], reps: usize) -> Duration {
    let mut total = Duration::ZERO;
    for _ in 0..reps {
        let mut t = T::empty();
        let start = Instant::now();
        for (k, &v) in keys.iter().zip(vals) {
            black_box(t.put(k, v));
        }
        total += start.elapsed();
        black_box(&t);
        drop(t);
    }
    total
}

// ---------------------------------------------------------------------------
// Rounds, calibration and statistics
// ---------------------------------------------------------------------------

/// Minimum timed window per pass. The per-arm repetition count is calibrated
/// once, in a discarded warm-up pass, so a 6 ns arm and a 1 µs arm are both
/// timed over windows of at least this length (§8.4 timer and interrupt noise).
pub const MIN_WINDOW: Duration = Duration::from_millis(20);

/// One arm of a cell: a pass closure taking a repetition count, and the number
/// of operations one repetition performs.
pub struct Arm<'a> {
    /// Artifact column name.
    pub name: String,
    /// Operations per repetition (probes, inserts, scanned prefixes).
    pub ops: usize,
    /// Runs `reps` repetitions and returns the timed total.
    pub pass: Box<dyn FnMut(usize) -> Duration + 'a>,
}

/// An arm that was not timed, with the reason (validation failure).
pub struct Invalid {
    /// Artifact column name.
    pub name: String,
    /// Why the arm was excluded.
    pub reason: String,
}

/// Runs `rounds` rounds over `arms`, rotating arm order each round. With
/// `compare`, the first arm is the subject every other arm is compared with;
/// otherwise pairs are added afterwards with [`paired_ratio`]. Returns the
/// row fields: per-arm median ns/op and repetitions, per-twin geometric-mean
/// ratio `subject ns / twin ns` with a BCa 95% interval on the per-round log
/// ratios, the interval's construction label, invalid arms, and `rounds_raw`.
pub fn run_cell(
    mut arms: Vec<Arm<'_>>,
    invalid: &[Invalid],
    rounds: usize,
    compare: bool,
) -> Map<String, Value> {
    assert!(!arms.is_empty(), "a cell needs its subject arm");
    // Warm-up and calibration, discarded.
    let reps: Vec<usize> = arms
        .iter_mut()
        .map(|a| {
            let t = (a.pass)(1);
            let r = (MIN_WINDOW.as_nanos() as f64 / t.as_nanos().max(1) as f64).ceil();
            (r as usize).max(1)
        })
        .collect();
    let m = arms.len();
    let mut ns: Vec<Vec<f64>> = vec![Vec::with_capacity(rounds); m];
    for round in 0..rounds {
        for i in 0..m {
            let a = (round + i) % m;
            let t = (arms[a].pass)(reps[a]);
            let per_op = t.as_nanos() as f64 / (reps[a] * arms[a].ops) as f64;
            assert!(per_op > 0.0, "{}: zero-length timed window", arms[a].name);
            ns[a].push(per_op);
        }
    }

    let mut row = Map::new();
    let mut raw_cols: Vec<(String, Vec<f64>)> = Vec::new();
    for (a, arm) in arms.iter().enumerate() {
        row.insert(format!("{}_ns_op", arm.name), json!(median(ns[a].clone())));
        row.insert(format!("{}_reps", arm.name), json!(reps[a]));
        row.insert(format!("{}_status", arm.name), json!("valid"));
        raw_cols.push((format!("{}_ns", arm.name), ns[a].clone()));
    }
    let subject = arms[0].name.clone();
    let compared = if compare { m } else { 1 };
    for (a, arm) in arms.iter().enumerate().take(compared).skip(1) {
        let logs: Vec<f64> = ns[0]
            .iter()
            .zip(&ns[a])
            .map(|(s, t)| (s / t).ln())
            .collect();
        let (mean, lo, hi, method) = bca_ci_labeled(&logs);
        let key = format!("ratio_{subject}_over_{}", arm.name);
        row.insert(key.clone(), json!(mean.exp()));
        row.insert(format!("{key}_ci"), json!([lo.exp(), hi.exp()]));
        row.insert(format!("{key}_ci_method"), json!(method));
        raw_cols.push((format!("log_{key}"), logs));
    }
    for inv in invalid {
        row.insert(format!("{}_status", inv.name), json!("invalid"));
        row.insert(format!("{}_invalid_reason", inv.name), json!(inv.reason));
    }
    row.insert(
        "ratio_estimator".into(),
        json!("geometric mean of per-round subject/twin ratios; BCa 95% on the log ratios"),
    );
    let cols: Vec<(&str, &[f64])> = raw_cols
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_slice()))
        .collect();
    row.insert("rounds_raw".into(), json!(rounds_raw(&cols)));
    row
}

/// Adds a paired ratio between two per-round series (e.g. one arm's
/// generator-order vs shuffled-order insert, timed in the same rounds), with
/// its BCa interval on the per-round log ratios.
pub fn paired_ratio(row: &mut Map<String, Value>, key: &str, num: &[f64], den: &[f64]) {
    assert_eq!(num.len(), den.len(), "{key}: unpaired columns");
    let logs: Vec<f64> = num.iter().zip(den).map(|(a, b)| (a / b).ln()).collect();
    let (mean, lo, hi, method) = bca_ci_labeled(&logs);
    row.insert(key.into(), json!(mean.exp()));
    row.insert(format!("{key}_ci"), json!([lo.exp(), hi.exp()]));
    row.insert(format!("{key}_ci_method"), json!(method));
}

/// Per-round ns/op series for `arm` from a row built by [`run_cell`].
pub fn series(row: &Map<String, Value>, arm: &str) -> Vec<f64> {
    let col = format!("{arm}_ns");
    row["rounds_raw"]
        .as_array()
        .expect("rounds_raw")
        .iter()
        .map(|r| r[&col].as_f64().expect("round sample"))
        .collect()
}

// ---------------------------------------------------------------------------
// CLI and output
// ---------------------------------------------------------------------------

/// Harness options: `--quick` (smoke populations, 3 rounds), `--json`, and
/// `--pop N` (one population only, so the runner can snapshot load between
/// populations, §8.17).
pub struct Cli {
    /// Populations to run.
    pub pops: Vec<usize>,
    /// Timed rounds per cell (after one discarded warm-up pass).
    pub rounds: usize,
    /// Smoke mode.
    pub quick: bool,
    /// Emit the JSON payload on stdout.
    pub json: bool,
}

/// Parses the harness options; `full` is the full-run population list.
pub fn cli(full: &[usize]) -> Cli {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json = args.iter().any(|a| a == "--json");
    let pop = args.iter().position(|a| a == "--pop").map(|i| {
        args.get(i + 1)
            .expect("--pop needs a value")
            .parse::<usize>()
            .expect("--pop N")
    });
    let pops = match (pop, quick) {
        (Some(n), _) => vec![n],
        (None, true) => vec![10_000, 50_000],
        (None, false) => full.to_vec(),
    };
    Cli {
        pops,
        rounds: if quick { 3 } else { 15 },
        quick,
        json,
    }
}

/// Prints the payload (`--json`) or a console summary of every row.
pub fn emit(benchmark: &str, workload_id: &str, cli: &Cli, rows: Vec<Value>) {
    if cli.json {
        let out = json!({
            "benchmark": benchmark,
            "workload_id": workload_id,
            "twins": ["patricia_tree 0.10.2", "fast_radix_trie 1.2.0", "qp-trie 0.8.2"],
            "quick": cli.quick,
            "rounds": cli.rounds,
            "min_window_ns": MIN_WINDOW.as_nanos() as u64,
            "results": rows,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&out).expect("serialisable payload")
        );
        return;
    }
    println!(
        "=== {benchmark} (quick={}, rounds={}) ===",
        cli.quick, cli.rounds
    );
    for r in &rows {
        let o = r.as_object().expect("row object");
        let head = ["distribution", "prefix_len", "order", "population"]
            .iter()
            .filter_map(|k| o.get(*k).map(|v| format!("{k}={v}")))
            .collect::<Vec<_>>()
            .join(" ");
        println!("  {head}");
        for (k, v) in o {
            let shown = k.ends_with("_ns_op")
                || k.ends_with("bytes_per_key")
                || k.ends_with("_invalid_reason")
                || (k.starts_with("ratio_") && !k.ends_with("_method") && k != "ratio_estimator");
            if shown {
                println!("      {k:52} {v}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Cell builders shared by the lookup harnesses
// ---------------------------------------------------------------------------

/// Adds a validated twin to a lookup cell, or records it as invalid. A twin
/// whose hit count over `probes` differs from the subject's is invalid too.
pub fn push_lookup_twin<'a, K, T: Twin<K>>(
    arms: &mut Vec<Arm<'a>>,
    invalid: &mut Vec<Invalid>,
    built: &'a Result<T, String>,
    probes: &'a [K],
    expected_hits: usize,
) {
    match built {
        Ok(t) => {
            let hits = hit_count(t, probes);
            if hits != expected_hits {
                invalid.push(Invalid {
                    name: T::NAME.into(),
                    reason: format!(
                        "{hits} hits over the probe stream, subject has {expected_hits}"
                    ),
                });
            } else {
                arms.push(Arm {
                    name: T::NAME.into(),
                    ops: probes.len(),
                    pass: Box::new(move |r| pass_twin_get(t, probes, r)),
                });
            }
        }
        Err(e) => invalid.push(Invalid {
            name: T::NAME.into(),
            reason: e.clone(),
        }),
    }
}

/// One `u64` point-lookup cell: every arm is built on its own (no interleaved
/// allocation between arms) from `build_keys` in the given order, validated,
/// and timed over `probes`.
pub fn u64_lookup_cell(build_keys: &[u64], probes: &[u64], rounds: usize) -> Map<String, Value> {
    let bk: Vec<[u8; 8]> = build_keys.iter().map(|&k| pkey(k)).collect();
    let vals: Vec<u64> = build_keys.iter().map(|&k| val(k)).collect();
    let pp: Vec<[u8; 8]> = probes.iter().map(|&k| pkey(k)).collect();

    let e = build_expanse(build_keys);
    let pt: Result<PatriciaMap<u64>, String> = build_twin(&bk, &vals);
    let rx: Result<RadixMap<u64>, String> = build_twin(&bk, &vals);
    let qp: Result<QpTrie<[u8; 8], u64>, String> = build_twin(&bk, &vals);

    let expected = probes.iter().filter(|&&k| e.get(k).is_some()).count();
    let mut arms = vec![Arm {
        name: "expanse".into(),
        ops: probes.len(),
        pass: Box::new(|r| pass_expanse_get(&e, probes, r)),
    }];
    let mut invalid = Vec::new();
    push_lookup_twin(&mut arms, &mut invalid, &pt, &pp, expected);
    push_lookup_twin(&mut arms, &mut invalid, &rx, &pp, expected);
    push_lookup_twin(&mut arms, &mut invalid, &qp, &pp, expected);
    let mut row = run_cell(arms, &invalid, rounds, true);
    row.insert("expected_hits".into(), json!(expected));
    row.insert("probes".into(), json!(probes.len()));
    row
}
