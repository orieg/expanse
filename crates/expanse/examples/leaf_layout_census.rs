//! Layout census for the low-cardinality leaf study (Refs #1257): builds each
//! key shape, checks that the engine's `layout_census()` sums to
//! `mem_used()`, and writes the census as JSON for
//! `scripts/leaf_layout_model.py`, which prices the same trees under each
//! candidate layout. Deterministic byte accounting — no timing, so host load
//! does not enter.
//!
//! Run: `cargo run --release -p expanse-trie --features layout-census --example leaf_layout_census -- --json <path> [--quick | --population N] [--totals-only] [--sosd name=path ...] [--sosd-only] [--downstream] [--rules keys:digits,...]`
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `example_leaf_layout_census` |
//! | `group` | 5 |
//! | `population` | 10^6 per synthetic shape; the #1257 string case also at 10^7 (`--quick`: 10^5); each SOSD dataset whole (`--sosd`); the downstream `orders` and `composite` cells at 10^7 (`--downstream`) |
//! | `insertion_order` | generator — each shape is inserted in its generator's order, stated per shape in the JSON; SOSD files in file order, which is sorted; the downstream cells sorted, and shuffled by a Fisher–Yates permutation from the suite PRNG |
//! | `probes_and_reuse` | N/A (Memory) |
//! | `hit_rate` | N/A |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | `mem_used()` accounting |
//! | `measured_region` | Clean |
//! | `arm_symmetry` | One engine build; candidate layouts are priced from its census by the model, not built |
//! | `statistics` | Exact byte count |
//! | `verdict` | **PENDING**: census instrument for #1257; no layout verdict rests on it alone. |

use expanse_trie::census::{LayoutCensus, MergeRule};
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use expanse_trie::strmap::{ExpanseStrMap, NulFreeStr};
use std::fmt::Write as _;

/// The suite PRNG (`bytes_per_key.rs`).
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

/// Merge rules the census reports groups for: a digit bound at the decimal
/// (10) and hex (16) alphabets, and key bounds at 2×, 4× and 8× `LEAF_CAP`.
const RULES: &[MergeRule] = &[
    MergeRule {
        max_keys: 128,
        max_digits: 10,
    },
    MergeRule {
        max_keys: 64,
        max_digits: 16,
    },
    MergeRule {
        max_keys: 128,
        max_digits: 16,
    },
    MergeRule {
        max_keys: 256,
        max_digits: 16,
    },
];

/// ASCII `%0Wd` of `i`, big-endian in a word: the decimal digits a
/// fixed-width numeric string puts in a word map's chunk.
fn decimal_word(i: u64, width: usize) -> u64 {
    let s = format!("{i:0width$}");
    let mut b = [0u8; 8];
    b[8 - width..].copy_from_slice(s.as_bytes());
    u64::from_be_bytes(b)
}

/// The adversarial word shape: groups of 64 keys below a 3-byte prefix whose
/// decoded byte takes 16 or 17 values in turn — one each side of a 16-digit
/// rule — so half the groups qualify and half do not, at the same key count.
fn adversarial(n: usize) -> Vec<u64> {
    let mut out = Vec::with_capacity(n);
    let mut g = 0u64;
    while out.len() < n {
        let digits = if g.is_multiple_of(2) { 16 } else { 17 };
        let mut i = 0;
        while i < 64 && out.len() < n {
            let d = (i % digits) as u64 * 15;
            let low = (i / digits) as u64;
            out.push((g << 24) | (d << 16) | low);
            i += 1;
        }
        g += 1;
    }
    out
}

fn u64_keys(shape: &str, n: usize) -> Vec<u64> {
    let mut rng = XorShift(SEED);
    match shape {
        "sequential" => (0..n as u64).collect(),
        "random" => (0..n).map(|_| rng.next()).collect(),
        "decimal8" => (0..n as u64).map(|i| decimal_word(i, 8)).collect(),
        "decimal8_sparse" => {
            // Uniform 8-digit numbers: decimal bytes, but the low digits are
            // far from dense at this population.
            (0..n)
                .map(|_| decimal_word(rng.next() % 100_000_000, 8))
                .collect()
        }
        "adversarial" => adversarial(n),
        _ => unreachable!("unknown shape {shape}"),
    }
}

fn str_keys(shape: &str, n: usize) -> Vec<Vec<u8>> {
    if shape == "composite" {
        return composite_keys(n);
    }
    text_keys(shape, n)
        .into_iter()
        .map(String::into_bytes)
        .collect()
}

fn text_keys(shape: &str, n: usize) -> Vec<String> {
    let mut rng = XorShift(SEED);
    match shape {
        // #1257: `t%03d:orders:%010d`, 1000 tenants in order, one global
        // counter — sorted by construction.
        "orders" => {
            let per_t = (n / 1000).max(1);
            (0..n)
                .map(|i| format!("t{:03}:orders:{i:010}", i / per_t))
                .collect()
        }
        // Random 10-digit order numbers under the same prefix scheme.
        "orders_sparse" => (0..n)
            .map(|i| {
                format!(
                    "t{:03}:orders:{:010}",
                    i % 1000,
                    rng.next() % 10_000_000_000
                )
            })
            .collect(),
        // Version-4-style UUIDs as lowercase hex text: 16 byte values.
        "uuid_hex" => (0..n)
            .map(|_| {
                let a = rng.next();
                let b = rng.next();
                format!(
                    "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
                    a >> 32,
                    (a >> 16) & 0xFFFF,
                    a & 0xFFF,
                    0x8000 | (b >> 48) & 0x3FFF,
                    b & 0xFFFF_FFFF_FFFF
                )
            })
            .collect(),
        _ => unreachable!("unknown shape {shape}"),
    }
}

/// `v` in `width` bytes of 7 bits each, big-endian, each byte offset by +1:
/// every byte lies in `0x01..=0x80`, byte order is numeric order at a fixed
/// width, and no byte is NUL, so the key is a valid `ExpanseStrMap` key.
fn enc7(v: u64, width: usize) -> impl Iterator<Item = u8> {
    (0..width)
        .rev()
        .map(move |i| ((v >> (7 * i)) & 0x7F) as u8 + 1)
}

/// The downstream composite shape of the #1257 follow-up:
/// `prefix(5 B) ‖ name ‖ '/' ‖ id(10 B)`, prefix and id in [`enc7`], names
/// `[A-Za-z0-9_]{1,32}`, and `'/'` below every name byte. The distribution
/// is this harness's choice, not the report's: 100 prefixes × 100 names
/// (drawn once from the suite PRNG and shared by every prefix) × sequential
/// ids from 0, `n / 10_000` per name. Emitted sorted.
fn composite_keys(n: usize) -> Vec<Vec<u8>> {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";
    let mut rng = XorShift(SEED ^ 0xC0);
    let mut names: Vec<Vec<u8>> = Vec::new();
    while names.len() < 100 {
        let len = 1 + (rng.next() % 32) as usize;
        let name: Vec<u8> = (0..len)
            .map(|_| ALPHABET[(rng.next() % ALPHABET.len() as u64) as usize])
            .collect();
        if !names.contains(&name) {
            names.push(name);
        }
    }
    // `'/'` sorts below every name byte, so byte order of the names is the
    // order of the keys that carry them.
    names.sort();
    let per_name = (n / 10_000).max(1) as u64;
    let mut out = Vec::with_capacity(n);
    for prefix in 0..100u64 {
        for name in &names {
            for id in 0..per_name {
                let mut k: Vec<u8> = enc7(prefix, 5).collect();
                k.extend_from_slice(name);
                k.push(b'/');
                k.extend(enc7(id, 10));
                out.push(k);
            }
        }
    }
    debug_assert!(out.windows(2).all(|w| w[0] < w[1]), "composite keys sorted");
    out
}

/// A Fisher–Yates permutation from the suite PRNG (§8.12.4's shuffled order).
fn shuffled<T: Clone>(v: &[T], seed: u64) -> Vec<T> {
    let mut out = v.to_vec();
    let mut rng = XorShift(seed);
    for i in (1..out.len()).rev() {
        let j = (rng.next() % (i as u64 + 1)) as usize;
        out.swap(i, j);
    }
    out
}

fn build_str(keys: &[Vec<u8>]) -> ExpanseStrMap {
    let mut m = ExpanseStrMap::new();
    for (i, k) in keys.iter().enumerate() {
        m.insert(NulFreeStr::new(k).unwrap(), i as u64);
    }
    m
}

fn json_census(c: &LayoutCensus) -> String {
    fn map2<K: std::fmt::Debug>(m: &std::collections::BTreeMap<K, usize>) -> String {
        let rows: Vec<String> = m
            .iter()
            .map(|(k, v)| {
                let k = format!("{k:?}").replace(['(', ')'], "");
                format!("[{k}, {v}]")
            })
            .collect();
        format!("[{}]", rows.join(", "))
    }
    format!(
        "{{\"keys\": {}, \"bytes\": {}, \"root_leaves\": {}, \"linear_leaves\": {}, \
         \"immediates\": {}, \"bitmap_leaves\": {}, \"bitmap_leaf_value_subarrays\": {}, \
         \"branch_l3\": {}, \"branch_l7\": {}, \"branch_b\": {}, \"branch_b_subarrays\": {}, \
         \"branch_u\": {}, \"branches_by_level_fanout\": {}, \"str_nodes\": {}, \
         \"str_node_shell_bytes\": {}, \"suffix_leaves\": {}, \"suffix_bytes\": {}}}",
        c.keys,
        c.bytes,
        map2(&c.root_leaves),
        map2(&c.linear_leaves),
        map2(&c.immediates),
        map2(&c.bitmap_leaves),
        map2(&c.bitmap_leaf_value_subarrays),
        c.branch_l3,
        c.branch_l7,
        c.branch_b,
        map2(&c.branch_b_subarrays),
        c.branch_u,
        map2(&c.branches_by_level_fanout),
        c.str_nodes,
        c.str_node_shell_bytes,
        map2(&c.suffix_leaves),
        c.suffix_bytes,
    )
}

/// Merge groups keyed by what a projected form's size depends on — key
/// bytes, key count and per-byte cardinalities — with `count` groups and
/// `bytes` the total they replace (the census keys them by subtree size too).
fn json_groups(c: &LayoutCensus) -> String {
    let mut agg: std::collections::BTreeMap<(u8, usize, [u16; 7]), (usize, usize)> =
        std::collections::BTreeMap::new();
    for (g, n) in &c.merge_groups {
        let e = agg
            .entry((g.key_bytes, g.keys, g.byte_cards))
            .or_insert((0, 0));
        e.0 += n;
        e.1 += n * g.bytes;
    }
    let rows: Vec<String> = agg
        .iter()
        .map(|((kb, keys, cards), (n, bytes))| {
            let cards: Vec<String> = cards[..*kb as usize]
                .iter()
                .map(u16::to_string)
                .collect();
            format!(
                "{{\"key_bytes\": {kb}, \"keys\": {keys}, \"byte_cards\": [{}], \"count\": {n}, \"bytes\": {bytes}}}",
                cards.join(", ")
            )
        })
        .collect();
    format!("[{}]", rows.join(", "))
}

/// One shape's record: the census, then the groups under every rule.
/// What a run records: the merge rules, and whether only byte totals.
#[derive(Clone, Copy)]
struct Run<'a> {
    rules: &'a [MergeRule],
    totals_only: bool,
}

fn record(
    out: &mut Vec<String>,
    run: Run<'_>,
    engine: &str,
    shape: &str,
    order: &str,
    mem_used: usize,
    census: impl Fn(Option<MergeRule>) -> LayoutCensus,
) {
    let base = census(None);
    assert_eq!(
        base.bytes, mem_used,
        "{engine}/{shape}: census does not sum to mem_used"
    );
    if run.totals_only {
        eprintln!(
            "{engine:6} {shape:16} n={:>9} mem_used={mem_used:>11}",
            base.keys
        );
        out.push(format!(
            "{{\"engine\": \"{engine}\", \"shape\": \"{shape}\", \"keys\": {}, \
             \"mem_used\": {mem_used}}}",
            base.keys
        ));
        return;
    }
    let mut rules = String::new();
    for (i, r) in run.rules.iter().enumerate() {
        let c = census(Some(*r));
        assert_eq!(
            c.bytes, mem_used,
            "{engine}/{shape}: census under a rule moved"
        );
        if i > 0 {
            rules.push_str(", ");
        }
        write!(
            rules,
            "{{\"max_keys\": {}, \"max_digits\": {}, \"groups\": {}}}",
            r.max_keys,
            r.max_digits,
            json_groups(&c)
        )
        .unwrap();
    }
    eprintln!(
        "{engine:6} {shape:16} n={:>9} mem_used={:>11} B/key={:.4}",
        base.keys,
        mem_used,
        mem_used as f64 / base.keys as f64
    );
    out.push(format!(
        "{{\"engine\": \"{engine}\", \"shape\": \"{shape}\", \"insertion_order\": \"{order}\", \
         \"mem_used\": {mem_used}, \"census\": {}, \"rules\": [{rules}]}}",
        json_census(&base)
    ));
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    // Byte totals only, for a build whose layout differs from the census's
    // (a diagnostic arm, a patched constant): what the model's prediction
    // for that layout is checked against.
    let totals_only = args.iter().any(|a| a == "--totals-only");
    let json = args
        .iter()
        .position(|a| a == "--json")
        .map(|i| args[i + 1].clone());
    let n = args
        .iter()
        .position(|a| a == "--population")
        .map(|i| args[i + 1].parse::<usize>().expect("--population N"))
        .unwrap_or(if quick { 100_000 } else { 1_000_000 });
    let mut out = Vec::new();
    // `--rules 128:10,128:128` replaces the default rule list: `max_keys:max_digits`.
    let rules: Vec<MergeRule> = args.iter().position(|a| a == "--rules").map_or_else(
        || RULES.to_vec(),
        |i| {
            args[i + 1]
                .split(',')
                .map(|r| {
                    let (k, d) = r.split_once(':').expect("--rules keys:digits,...");
                    MergeRule {
                        max_keys: k.parse().expect("max_keys"),
                        max_digits: d.parse().expect("max_digits"),
                    }
                })
                .collect()
        },
    );
    let run = Run {
        rules: &rules,
        totals_only,
    };

    // Real sorted `u64` keys (SOSD, Kipf et al. 2019): `--sosd name=path`,
    // repeatable, reads the benchmark's binary format — a little-endian u64
    // count, then that many little-endian u64 keys — and censuses the whole
    // file as map and set. `--sosd-only` skips the synthetic shapes.
    let sosd: Vec<(String, String)> = args
        .windows(2)
        .filter(|w| w[0] == "--sosd")
        .map(|w| {
            let (name, path) = w[1].split_once('=').expect("--sosd name=path");
            (name.to_string(), path.to_string())
        })
        .collect();
    for (name, path) in &sosd {
        let bytes = std::fs::read(path).expect("read SOSD file");
        let count = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
        assert_eq!(bytes.len(), 8 + 8 * count, "{path}: not an SOSD u64 file");
        let keys: Vec<u64> = bytes[8..]
            .as_chunks::<8>()
            .0
            .iter()
            .map(|c| u64::from_le_bytes(*c))
            .collect();
        drop(bytes);
        let shape = format!("sosd_{name}");
        let mut m = ExpanseMap::new();
        for (i, &k) in keys.iter().enumerate() {
            m.insert(k, i as u64);
        }
        record(&mut out, run, "map", &shape, "sorted", m.mem_used(), |r| {
            m.layout_census(r)
        });
        drop(m);
        let mut s = ExpanseSet::new();
        for &k in &keys {
            s.insert(k);
        }
        record(&mut out, run, "set", &shape, "sorted", s.mem_used(), |r| {
            s.layout_census(r)
        });
    }
    let synthetic = !args
        .iter()
        .any(|a| a == "--sosd-only" || a == "--downstream");

    for shape in [
        "sequential",
        "random",
        "decimal8",
        "decimal8_sparse",
        "adversarial",
    ] {
        if !synthetic {
            break;
        }
        let keys = u64_keys(shape, n);
        let order = if shape == "random" || shape == "decimal8_sparse" {
            "generator"
        } else {
            "sorted"
        };
        let mut m = ExpanseMap::new();
        let mut s = ExpanseSet::new();
        for &k in &keys {
            m.insert(k, k);
            s.insert(k);
        }
        record(&mut out, run, "map", shape, order, m.mem_used(), |r| {
            m.layout_census(r)
        });
        record(&mut out, run, "set", shape, order, s.mem_used(), |r| {
            s.layout_census(r)
        });
    }

    // The downstream cells of the #1257 follow-up: `orders` and `composite`
    // at 10^7 (or `--population`), each loaded sorted and shuffled, then — from
    // the sorted load — a seeded random half removed (`_churned`), beside a
    // fresh sorted build of the same survivors (`_survivors`). The difference
    // between those two is what a removal phase leaves behind.
    if args.iter().any(|a| a == "--downstream") {
        let dn = args
            .iter()
            .position(|a| a == "--population")
            .map_or(10_000_000, |_| n);
        for shape in ["orders", "composite"] {
            let keys = str_keys(shape, dn);
            let m = build_str(&keys);
            let base = format!("{shape}_{dn}");
            record(
                &mut out,
                run,
                "strmap",
                &format!("{base}_sorted"),
                "sorted",
                m.mem_used(),
                |r| m.layout_census(r),
            );
            drop(m);
            let mix = shuffled(&keys, SEED ^ 0x5F);
            let m = build_str(&mix);
            drop(mix);
            record(
                &mut out,
                run,
                "strmap",
                &format!("{base}_shuffled"),
                "shuffled",
                m.mem_used(),
                |r| m.layout_census(r),
            );
            drop(m);
            let mut m = build_str(&keys);
            let mut rng = XorShift(SEED ^ 0xDE1);
            let mut survivors = Vec::with_capacity(keys.len() / 2);
            for k in &keys {
                if rng.next() & 1 == 1 {
                    m.remove(NulFreeStr::new(k).unwrap());
                } else {
                    survivors.push(k.clone());
                }
            }
            record(
                &mut out,
                run,
                "strmap",
                &format!("{base}_churned"),
                "sorted, then a random half removed",
                m.mem_used(),
                |r| m.layout_census(r),
            );
            drop(m);
            let m = build_str(&survivors);
            record(
                &mut out,
                run,
                "strmap",
                &format!("{base}_survivors"),
                "sorted",
                m.mem_used(),
                |r| m.layout_census(r),
            );
        }
    }

    let mut str_cells = vec![("orders", n), ("orders_sparse", n), ("uuid_hex", n)];
    if !quick {
        str_cells.push(("orders", 10_000_000));
    }
    if !synthetic {
        str_cells.clear();
    }
    for (shape, count) in str_cells {
        let keys = str_keys(shape, count);
        let order = if shape == "orders" {
            "sorted"
        } else {
            "generator"
        };
        let m = build_str(&keys);
        let name = if count == n {
            shape.to_string()
        } else {
            format!("{shape}_{count}")
        };
        record(&mut out, run, "strmap", &name, order, m.mem_used(), |r| {
            m.layout_census(r)
        });
    }

    let commit = std::env::var("EXPANSE_COMMIT").unwrap_or_else(|_| "unknown".into());
    let doc = format!(
        "{{\"instrument\": \"leaf_layout_census\", \"commit\": \"{commit}\", \
         \"population\": {n}, \"seed\": \"{SEED:#x}\", \"records\": [\n{}\n]}}\n",
        out.join(",\n")
    );
    match json {
        Some(path) => std::fs::write(&path, doc).expect("write census JSON"),
        None => print!("{doc}"),
    }
}
