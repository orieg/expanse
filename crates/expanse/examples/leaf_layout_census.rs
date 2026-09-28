//! Layout census for the low-cardinality leaf study (Refs #1257): builds each
//! key shape, checks that the engine's `layout_census()` sums to
//! `mem_used()`, and writes the census as JSON for
//! `scripts/leaf_layout_model.py`, which prices the same trees under each
//! candidate layout. Deterministic byte accounting — no timing, so host load
//! does not enter.
//!
//! Run: `cargo run --release -p expanse-trie --features layout-census --example leaf_layout_census -- --json <path> [--quick | --population N] [--totals-only]`
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `example_leaf_layout_census` |
//! | `group` | 5 |
//! | `population` | 10^6 per shape; the #1257 string case also at 10^7 (`--quick`: 10^5) |
//! | `insertion_order` | generator — each shape is inserted in its generator's order, stated per shape in the JSON |
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

fn str_keys(shape: &str, n: usize) -> Vec<String> {
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
fn record(
    out: &mut Vec<String>,
    totals_only: bool,
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
    if totals_only {
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
    for (i, r) in RULES.iter().enumerate() {
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

    for shape in [
        "sequential",
        "random",
        "decimal8",
        "decimal8_sparse",
        "adversarial",
    ] {
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
        record(
            &mut out,
            totals_only,
            "map",
            shape,
            order,
            m.mem_used(),
            |r| m.layout_census(r),
        );
        record(
            &mut out,
            totals_only,
            "set",
            shape,
            order,
            s.mem_used(),
            |r| s.layout_census(r),
        );
    }

    let mut str_cells = vec![("orders", n), ("orders_sparse", n), ("uuid_hex", n)];
    if !quick {
        str_cells.push(("orders", 10_000_000));
    }
    for (shape, count) in str_cells {
        let keys = str_keys(shape, count);
        let order = if shape == "orders" {
            "sorted"
        } else {
            "generator"
        };
        let mut m = ExpanseStrMap::new();
        for (i, k) in keys.iter().enumerate() {
            m.insert(NulFreeStr::new(k.as_bytes()).unwrap(), i as u64);
        }
        let name = if count == n {
            shape.to_string()
        } else {
            format!("{shape}_{count}")
        };
        record(
            &mut out,
            totals_only,
            "strmap",
            &name,
            order,
            m.mem_used(),
            |r| m.layout_census(r),
        );
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
