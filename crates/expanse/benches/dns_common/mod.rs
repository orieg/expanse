//! Shared machinery for the DNS-shaped key suite (`docs/benchmarks/dns_names/`).
//!
//! Names, key forms, probe classes, arm builds and the one-round timing step
//! of `docs/benchmarks/dns_names/METHODOLOGY.md` §2 and §3. The name generator
//! and both key encodings reproduce `scripts/dns_name_bounds.py`;
//! `crates/expanse/tests/test_dns_name_bounds.rs` pins the same reproduction
//! against the script's constants.
//!
//! Every harness of the suite runs one round per process (`--round K`): the
//! suite runner starts one process per round and forms the per-round ratios
//! and their intervals itself, so no round reuses memory an earlier round
//! freed.

#![allow(dead_code)]

use expanse_trie::ExpanseOrderedBytesMap;
use expanse_trie::strmap::{ExpanseStrMap, NulFreeStr};
pub use qp_trie::Trie as QpTrie;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::time::Duration;

// ---------------------------------------------------------------------------
// PRNG and seeds
// ---------------------------------------------------------------------------

/// `scripts/dns_name_bounds.py::NAME_SEED`: the population stream.
pub const NAME_SEED: u64 = 0x0D05_5EED_0000_0001;
/// Stream for the label and the parent of a `miss_under_sld` probe.
pub const UNDER_SEED: u64 = 0x0D05_5EED_0000_0002;
/// Stream for `encloser` probes.
pub const ENCLOSER_SEED: u64 = 0x0D05_5EED_0000_0003;
/// Stream for the uniformly drawn half of the population used as hits.
pub const HIT_SEED: u64 = 0x0D05_5EED_0000_0004;
/// Stream for the `shuffled` insertion order.
pub const ORDER_SEED: u64 = 0x0D05_5EED_0000_0005;
/// Stream for the probe-order permutation.
pub const PROBE_SEED: u64 = 0x0D05_5EED_0000_0006;

const TLDS: [&[u8]; 6] = [b"com", b"net", b"org", b"io", b"de", b"uk"];
const HOSTS: [&[u8]; 3] = [b"www", b"mail", b"ns1"];
const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";

/// `scripts/patricia_envelope.py::xorshift`.
pub struct XorShift(pub u64);

impl XorShift {
    /// Next 64-bit draw.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Uniform index in `[0, n)`.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Fisher–Yates permutation of `0..n` under `seed`.
pub fn permutation(n: usize, seed: u64) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..n).collect();
    let mut rng = XorShift(seed);
    for i in (1..n).rev() {
        idx.swap(i, rng.below(i + 1));
    }
    idx
}

// ---------------------------------------------------------------------------
// Names and key forms
// ---------------------------------------------------------------------------

/// A name as its labels in wire order (`www`, `example`, `com`).
pub type Name = Vec<Vec<u8>>;
/// A key encoding: [`key_nul`] or [`key_direct`].
pub type Encoder = fn(&[Vec<u8>]) -> Vec<u8>;

/// `key_nul`: reversed lowercased labels, each `0x00`-terminated. The
/// generator's labels hold no octet below `0x30`, so the script's escape of
/// `0x00` and `0x01` never applies; it is kept so the form is the script's.
pub fn key_nul(labels: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for label in labels.iter().rev() {
        for &b in label {
            match b.to_ascii_lowercase() {
                0 => out.extend_from_slice(&[1, 1]),
                1 => out.extend_from_slice(&[1, 2]),
                b => out.push(b),
            }
        }
        out.push(0);
    }
    out.shrink_to_fit();
    out
}

/// `key_direct`: the NUL-free form, `0x01`-terminated labels, label octets
/// `0x00..=0x02` escaped.
pub fn key_direct(labels: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for label in labels.iter().rev() {
        for &b in label {
            match b.to_ascii_lowercase() {
                b @ 0..=2 => out.extend_from_slice(&[2, b + 2]),
                b => out.push(b),
            }
        }
        out.push(1);
    }
    out.shrink_to_fit();
    out
}

/// One label by the generator's rule: 3 to 16 characters of `[a-z0-9]`.
fn gen_label(rng: &mut XorShift) -> Vec<u8> {
    let r = rng.next();
    let length = 3 + (r >> 8) % 14;
    (0..length)
        .map(|_| ALPHABET[rng.below(ALPHABET.len())])
        .collect()
}

/// The registration stream of `scripts/dns_name_bounds.py::gen_names`: each
/// step yields `<sld>.<tld>` and its first 0 to 3 host names.
struct Registrations {
    rng: XorShift,
    seen: HashSet<(Vec<u8>, Vec<u8>)>,
}

impl Registrations {
    fn new() -> Self {
        Self {
            rng: XorShift(NAME_SEED),
            seen: HashSet::new(),
        }
    }

    /// The names of the next registration whose `<sld>.<tld>` is new.
    fn next_names(&mut self) -> Vec<Name> {
        loop {
            let r = self.rng.next();
            let tld = TLDS[(r % TLDS.len() as u64) as usize].to_vec();
            let length = 3 + (r >> 8) % 14;
            let hosts = ((r >> 16) % 4) as usize;
            let sld: Vec<u8> = (0..length)
                .map(|_| ALPHABET[self.rng.below(ALPHABET.len())])
                .collect();
            if !self.seen.insert((sld.clone(), tld.clone())) {
                continue;
            }
            let mut out = vec![vec![sld.clone(), tld.clone()]];
            for h in &HOSTS[..hosts] {
                out.push(vec![h.to_vec(), sld.clone(), tld.clone()]);
            }
            return out;
        }
    }
}

/// An `encloser` probe: the full key and the prefix lengths of the names
/// tried, longest first. `cuts[..tries]` are the lookups one probe performs;
/// the last one is the present `<sld>.<tld>`.
#[derive(Clone)]
pub struct EncloserProbe {
    /// Full probe key in one form.
    pub key: Vec<u8>,
    /// Prefix lengths, longest first.
    pub cuts: [u32; 4],
    /// Lookups this probe performs: k + 1.
    pub tries: usize,
    /// Value stored under the closest encloser.
    pub answer: u64,
}

/// Population and probe classes for one population size (METHODOLOGY §3).
pub struct World {
    /// Population size.
    pub n: usize,
    /// `key_nul` of every name, in generator order; the value of name `i` is `i`.
    pub keys_nul: Vec<Vec<u8>>,
    /// `key_direct` of every name, in generator order.
    pub keys_direct: Vec<Vec<u8>>,
    /// `hit`: indices of a uniformly drawn half of the population.
    pub hit_idx: Vec<usize>,
    /// `miss_new_sld`: names from the continuation of the stream.
    pub new_sld: Vec<Name>,
    /// `miss_under_sld`: one new label in front of a present two-label name.
    pub under_sld: Vec<Name>,
    /// `encloser`: (name, index of its closest encloser, k).
    pub encloser: Vec<(Name, usize, usize)>,
}

impl World {
    /// Generates the population of `n` names and `n / 2` probes per class.
    pub fn new(n: usize) -> Self {
        assert!(n >= 4, "population too small for the probe classes");
        let half = n / 2;
        let mut reg = Registrations::new();
        let mut names: Vec<Name> = Vec::with_capacity(n + 4);
        while names.len() < n {
            names.extend(reg.next_names());
        }
        names.truncate(n);
        // Continuation of the same stream: every later registration has a
        // second-level name the population does not hold.
        let mut new_sld: Vec<Name> = Vec::with_capacity(half + 4);
        while new_sld.len() < half {
            new_sld.extend(reg.next_names());
        }
        new_sld.truncate(half);
        drop(reg);

        let keys_nul: Vec<Vec<u8>> = names.iter().map(|nm| key_nul(nm)).collect();
        let keys_direct: Vec<Vec<u8>> = names.iter().map(|nm| key_direct(nm)).collect();
        let present: HashSet<&[u8]> = keys_nul.iter().map(|k| k.as_slice()).collect();
        assert_eq!(present.len(), n, "generated names are distinct");
        for nm in &new_sld {
            assert!(
                !present.contains(key_nul(nm).as_slice()),
                "a miss_new_sld probe is present"
            );
        }
        let parents: Vec<usize> = (0..n).filter(|&i| names[i].len() == 2).collect();

        let mut rng = XorShift(UNDER_SEED);
        let mut drawn: HashSet<Vec<u8>> = HashSet::new();
        let mut under_sld = Vec::with_capacity(half);
        while under_sld.len() < half {
            let parent = &names[parents[rng.below(parents.len())]];
            let name = vec![gen_label(&mut rng), parent[0].clone(), parent[1].clone()];
            let key = key_nul(&name);
            if present.contains(key.as_slice()) || !drawn.insert(key) {
                continue;
            }
            under_sld.push(name);
        }

        let mut rng = XorShift(ENCLOSER_SEED);
        let mut drawn: HashSet<Vec<u8>> = HashSet::new();
        let mut encloser = Vec::with_capacity(half);
        while encloser.len() < half {
            let pi = parents[rng.below(parents.len())];
            let parent = &names[pi];
            let k = 1 + rng.below(3);
            // The label next to the present name is drawn first, then the
            // k - 1 labels in front of it.
            let mut front: Vec<Vec<u8>> = (0..k).map(|_| gen_label(&mut rng)).collect();
            front.reverse();
            let inner = vec![front[k - 1].clone(), parent[0].clone(), parent[1].clone()];
            let inner_key = key_nul(&inner);
            if present.contains(inner_key.as_slice()) {
                continue;
            }
            let mut name = front;
            name.push(parent[0].clone());
            name.push(parent[1].clone());
            if !drawn.insert(key_nul(&name)) {
                continue;
            }
            encloser.push((name, pi, k));
        }

        let hit_idx: Vec<usize> = permutation(n, HIT_SEED).into_iter().take(half).collect();
        drop(present);
        Self {
            n,
            keys_nul,
            keys_direct,
            hit_idx,
            new_sld,
            under_sld,
            encloser,
        }
    }

    /// Build order as indices into the population: `sorted` is ascending key
    /// order, `shuffled` a Fisher–Yates permutation under `ORDER_SEED`.
    pub fn build_order(&self, order: &str) -> Vec<usize> {
        match order {
            "sorted" => {
                let mut idx: Vec<usize> = (0..self.n).collect();
                idx.sort_unstable_by(|&a, &b| self.keys_nul[a].cmp(&self.keys_nul[b]));
                // Both key forms encode the same canonical order; a build
                // that is sorted in one and not in the other is a defect.
                assert!(
                    idx.windows(2)
                        .all(|w| self.keys_direct[w[0]] < self.keys_direct[w[1]]),
                    "key_direct does not sort as key_nul"
                );
                idx
            }
            "shuffled" => permutation(self.n, ORDER_SEED),
            _ => panic!("unknown insertion order {order}"),
        }
    }

    /// A 50% hit probe stream: the `hit` half with `misses`, in a fixed
    /// permutation, as `key_direct` keys when `direct` and `key_nul` otherwise.
    pub fn get_probes(&self, misses: &[Name], direct: bool) -> Vec<Vec<u8>> {
        let (keys, enc): (&[Vec<u8>], Encoder) = if direct {
            (&self.keys_direct, key_direct)
        } else {
            (&self.keys_nul, key_nul)
        };
        let mut all: Vec<Vec<u8>> = self.hit_idx.iter().map(|&i| keys[i].clone()).collect();
        all.extend(misses.iter().map(|nm| enc(nm)));
        let perm = permutation(all.len(), PROBE_SEED);
        perm.iter().map(|&i| all[i].clone()).collect()
    }

    /// The miss probes of one class in the form `enc` gives, permuted.
    pub fn miss_probes(&self, misses: &[Name], enc: Encoder) -> Vec<Vec<u8>> {
        let perm = permutation(misses.len(), PROBE_SEED);
        perm.iter().map(|&i| enc(&misses[i])).collect()
    }

    /// The `encloser` probes in the form `enc` gives, permuted.
    pub fn encloser_probes(&self, enc: Encoder) -> Vec<EncloserProbe> {
        let perm = permutation(self.encloser.len(), PROBE_SEED);
        perm.iter()
            .map(|&i| {
                let (name, parent, k) = &self.encloser[i];
                let mut cuts = [0u32; 4];
                for (j, cut) in cuts.iter_mut().enumerate().take(k + 1) {
                    // Dropping the j leftmost labels leaves a prefix of the
                    // reversed key.
                    *cut = enc(&name[j..]).len() as u32;
                }
                EncloserProbe {
                    key: enc(name),
                    cuts,
                    tries: k + 1,
                    answer: *parent as u64,
                }
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Arms
// ---------------------------------------------------------------------------

/// `key` as a `NulFreeStr` without the validating scan.
///
/// Every `key_direct` key is validated once when its arm is built
/// ([`build_direct`]) or when its probe stream is checked
/// ([`assert_nul_free`]); a prefix of a NUL-free key is NUL-free.
#[inline(always)]
pub fn nul_free(key: &[u8]) -> &NulFreeStr {
    debug_assert!(!key.contains(&0));
    // SAFETY: the caller's keys come from `key_direct`, which emits no 0x00
    // octet, and every probe stream is checked by `assert_nul_free` before it
    // is timed.
    unsafe { NulFreeStr::new_unchecked(key) }
}

/// Panics unless every key is NUL-free; run on each `key_direct` probe stream
/// outside the timed region.
pub fn assert_nul_free(keys: &[Vec<u8>]) {
    assert!(
        keys.iter().all(|k| NulFreeStr::new(k).is_some()),
        "a key_direct probe holds a NUL"
    );
}

/// `ExpanseOrderedBytesMap` over `key_nul`, checked: population and every
/// key reading back its value.
pub fn build_ordered(w: &World, order: &[usize]) -> ExpanseOrderedBytesMap {
    let mut m = ExpanseOrderedBytesMap::new();
    for &i in order {
        assert_eq!(m.insert(&w.keys_nul[i], i as u64), None);
    }
    assert_eq!(m.len() as usize, w.n, "expanse_ordered population");
    let lost = (0..w.n)
        .filter(|&i| m.get(&w.keys_nul[i]) != Some(i as u64))
        .count();
    assert_eq!(lost, 0, "expanse_ordered lost keys");
    m
}

/// `ExpanseStrMap` over `key_direct`, checked.
pub fn build_direct(w: &World, order: &[usize]) -> ExpanseStrMap {
    let mut m = ExpanseStrMap::new();
    for &i in order {
        let key = NulFreeStr::new(&w.keys_direct[i]).expect("key_direct is NUL-free");
        assert_eq!(m.insert(key, i as u64), None);
    }
    assert_eq!(m.len() as usize, w.n, "expanse_direct population");
    let lost = (0..w.n)
        .filter(|&i| m.get(nul_free(&w.keys_direct[i])) != Some(i as u64))
        .count();
    assert_eq!(lost, 0, "expanse_direct lost keys");
    m
}

/// `BTreeMap<Vec<u8>, u64>` over `key_nul`, checked.
pub fn build_btree(w: &World, order: &[usize]) -> BTreeMap<Vec<u8>, u64> {
    let mut m = BTreeMap::new();
    for &i in order {
        assert_eq!(m.insert(w.keys_nul[i].clone(), i as u64), None);
    }
    assert_eq!(m.len(), w.n, "btreemap population");
    let lost = (0..w.n)
        .filter(|&i| m.get(w.keys_nul[i].as_slice()) != Some(&(i as u64)))
        .count();
    assert_eq!(lost, 0, "btreemap lost keys");
    m
}

/// `qp_trie::Trie<Vec<u8>, u64>` over `key_nul`, checked.
pub fn build_qp(w: &World, order: &[usize]) -> QpTrie<Vec<u8>, u64> {
    let mut m = QpTrie::new();
    for &i in order {
        assert_eq!(m.insert(w.keys_nul[i].clone(), i as u64), None);
    }
    assert_eq!(m.count(), w.n, "qp_trie population");
    let lost = (0..w.n)
        .filter(|&i| m.get(&w.keys_nul[i][..]) != Some(&(i as u64)))
        .count();
    assert_eq!(lost, 0, "qp_trie lost keys");
    m
}

// ---------------------------------------------------------------------------
// One round
// ---------------------------------------------------------------------------

/// Minimum timed window per arm, the `MIN_WINDOW` of `benches/patricia_common`.
pub const MIN_WINDOW: Duration = Duration::from_millis(20);

/// One arm of a cell: its name and a pass taking a repetition count and
/// returning the timed total.
pub struct Arm<'a> {
    /// Arm name in every artifact row.
    pub name: &'static str,
    /// Runs `reps` repetitions of the probe loop.
    pub pass: Box<dyn FnMut(usize) -> Duration + 'a>,
}

/// Times every arm once, in the order rotated by `round`. Each arm gets one
/// discarded warm-up pass, from which its repetition count is calibrated to
/// [`MIN_WINDOW`], then one timed pass. Returns the round's rows.
pub fn time_round(mut arms: Vec<Arm<'_>>, ops: usize, round: usize) -> Vec<Value> {
    let m = arms.len();
    let mut rows: Vec<(usize, Value)> = Vec::with_capacity(m);
    for i in 0..m {
        let a = (round + i) % m;
        let warm = (arms[a].pass)(1);
        let reps =
            ((MIN_WINDOW.as_nanos() as f64 / warm.as_nanos().max(1) as f64).ceil() as usize).max(1);
        let t = (arms[a].pass)(reps);
        let ns = t.as_nanos() as f64 / (reps * ops) as f64;
        assert!(ns > 0.0, "{}: zero-length timed window", arms[a].name);
        rows.push((
            a,
            json!({
                "round": round,
                "arm": arms[a].name,
                "ns_per_op": ns,
                "reps": reps,
                "position": i,
            }),
        ));
    }
    rows.sort_by_key(|(a, _)| *a);
    rows.into_iter().map(|(_, r)| r).collect()
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// Populations of a full run (METHODOLOGY §3).
pub const POPS: [usize; 4] = [1_000, 10_000, 100_000, 1_000_000];
/// Populations of a `--quick` smoke run.
pub const POPS_QUICK: [usize; 2] = [1_000, 10_000];
/// Insertion orders, both measured (AGENTS.md §8.12.4).
pub const ORDERS: [&str; 2] = ["sorted", "shuffled"];

/// Harness options.
pub struct Cli {
    /// Populations to run.
    pub pops: Vec<usize>,
    /// Round index; rotates the arm order.
    pub round: usize,
    /// Smoke mode.
    pub quick: bool,
    /// Print the JSON payload on stdout.
    pub json: bool,
}

fn arg_value(args: &[String], flag: &str) -> Option<usize> {
    args.iter().position(|a| a == flag).map(|i| {
        args.get(i + 1)
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| panic!("{flag} takes a non-negative integer"))
    })
}

/// Parses `--quick`, `--json`, `--round K` and `--pop N`.
pub fn cli() -> Cli {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let pops = match arg_value(&args, "--pop") {
        Some(n) => vec![n],
        None if quick => POPS_QUICK.to_vec(),
        None => POPS.to_vec(),
    };
    Cli {
        pops,
        round: arg_value(&args, "--round").unwrap_or(0),
        quick,
        json: args.iter().any(|a| a == "--json"),
    }
}

/// Prints the payload: JSON with `--json`, otherwise one line per row.
pub fn emit(workload_id: &str, cli: &Cli, cell_key: &str, cells: Vec<Value>) {
    let out = json!({
        "benchmark": workload_id,
        "workload_id": workload_id,
        "twins": ["std BTreeMap", "qp-trie 0.8.2"],
        "quick": cli.quick,
        "round": cli.round,
        "min_window_ns": MIN_WINDOW.as_nanos() as u64,
        cell_key: cells,
    });
    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&out).expect("serialisable payload")
        );
        return;
    }
    println!(
        "=== {workload_id} (quick={}, round={}) ===",
        cli.quick, cli.round
    );
    for c in out[cell_key].as_array().expect("cell list") {
        let o = c.as_object().expect("cell object");
        let head = o
            .iter()
            .filter(|(k, _)| *k != "rounds_raw")
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!("  {head}");
        for r in c["rounds_raw"].as_array().into_iter().flatten() {
            println!("      {r}");
        }
    }
}
