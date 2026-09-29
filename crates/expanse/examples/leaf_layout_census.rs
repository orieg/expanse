//! Layout census for the low-cardinality leaf study (Refs #1257): builds each
//! key shape, checks that the engine's `layout_census()` sums to
//! `mem_used()`, and writes the census as JSON for
//! `scripts/leaf_layout_model.py`, which prices the same trees under each
//! candidate layout. Deterministic byte accounting — no timing, so host load
//! does not enter.
//!
//! Run: `cargo run --release -p expanse-trie --features layout-census --example leaf_layout_census -- --json <path> [--quick | --population N] [--totals-only] [--sosd name=path ...] [--sosd-only] [--downstream] [--encodings] [--mixed] [--rules keys:digits,...] [--dump-keys <dir>]`
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `example_leaf_layout_census` |
//! | `group` | 5 |
//! | `population` | 10^6 per synthetic shape; the #1257 string case also at 10^7 (`--quick`: 10^5); each SOSD dataset whole (`--sosd`); the downstream `orders` and `composite` (uniform and skewed) cells at 10^7 (`--downstream`); both composite distributions under each id encoding, and `orders` as text and transcoded to base 32, at 10^7 (`--encodings`) |
//! | `insertion_order` | generator — each shape is inserted in its generator's order, stated per shape in the JSON; SOSD files in file order, which is sorted; the downstream and `--encodings` cells sorted, and shuffled by a Fisher–Yates permutation from the suite PRNG |
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
use expanse_trie::strmap::{CHUNK_BYTES, ExpanseStrMap, NulFreeStr};
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
        return composite_keys(n, "uniform", ENCODINGS[0]);
    }
    if shape == "composite_skewed" {
        return composite_keys(n, "skewed", ENCODINGS[0]);
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

/// An order-preserving, NUL-free id encoding for the composite shape: every
/// byte is `base_byte + digit`, big-endian at a fixed width, so byte order
/// is numeric order and no byte is NUL.
#[derive(Clone, Copy)]
struct IdEncoding {
    name: &'static str,
    /// Values per byte.
    radix: u64,
    /// Bytes per id.
    width: usize,
    /// The byte for digit 0.
    base_byte: u8,
    /// Widen the id, per name, to the smallest width of at least `width`
    /// that makes the whole key `7 (mod 8)` bytes long: the string map's
    /// terminal chunk then holds seven key bytes and its NUL, so the id's
    /// low byte always sits at word level 2. The width is fixed per name,
    /// so ids stay order-preserving within a name, and extra width is
    /// leading zero digits shared by every id of the name.
    align7: bool,
    /// Write the 5-byte prefix field in this encoding's digits too, at the
    /// fixed width that covers 31 bits (7 base-32 digits), instead of the
    /// report's 7-bit field.
    prefix_too: bool,
}

/// The encodings the census compares (#1257 follow-up). `enc7x10` is the
/// report's own: 7 bits per byte, +1. The rest fit every id below 10^6 (the
/// skewed shape's largest name) in fewer bytes and differ in how many values
/// the low byte takes — which decides whether a range of ids fits one linear
/// leaf (at most `LEAF_CAP` = 32 keys) or cascades into single-key edges.
const ENCODINGS: &[IdEncoding] = &[
    IdEncoding {
        name: "enc7x10",
        radix: 128,
        width: 10,
        base_byte: 0x01,
        align7: false,
        prefix_too: false,
    },
    IdEncoding {
        name: "b32x4",
        radix: 32,
        width: 4,
        base_byte: 0x41,
        align7: false,
        prefix_too: false,
    },
    IdEncoding {
        name: "b16x5",
        radix: 16,
        width: 5,
        base_byte: 0x41,
        align7: false,
        prefix_too: false,
    },
    IdEncoding {
        name: "b255x3",
        radix: 255,
        width: 3,
        base_byte: 0x01,
        align7: false,
        prefix_too: false,
    },
    IdEncoding {
        name: "b32x4a7",
        radix: 32,
        width: 4,
        base_byte: 0x41,
        align7: true,
        prefix_too: false,
    },
    // Full-width ids (#1257 follow-up): every u64 fits (13 digits cover
    // 2^64), the prefix is in the same digits, and the width is widened per
    // name to the 7 (mod 8) alignment, so 13-20 digits.
    IdEncoding {
        name: "b32full",
        radix: 32,
        width: 13,
        base_byte: 0x01,
        align7: true,
        prefix_too: true,
    },
];

impl IdEncoding {
    fn push(self, id: u64, out: &mut Vec<u8>) {
        let width = if self.align7 {
            let mut w = self.width;
            while (out.len() + w) % CHUNK_BYTES != CHUNK_BYTES - 1 {
                w += 1;
            }
            w
        } else {
            self.width
        };
        assert!(
            self.radix
                .checked_pow(self.width as u32)
                .is_none_or(|span| id < span),
            "{}: id {id} does not fit",
            self.name
        );
        // Least significant digit first, then reversed: `radix^i` overflows
        // u64 for the widest ids (32^13 > 2^64).
        let start = out.len();
        let mut v = id;
        for _ in 0..width {
            out.push(self.base_byte + (v % self.radix) as u8);
            v /= self.radix;
        }
        out[start..].reverse();
    }
}

const NAME_ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";

fn random_name(rng: &mut XorShift, min: usize, max: usize) -> Vec<u8> {
    let len = min + (rng.next() % (max - min + 1) as u64) as usize;
    (0..len)
        .map(|_| NAME_ALPHABET[(rng.next() % NAME_ALPHABET.len() as u64) as usize])
        .collect()
}

/// Ids per name for the skewed composite distribution: a power law over
/// name rank, `min(10^6, max(1, floor(A · r^-1.4)))`, with `A` found by
/// bisection so the sizes sum to `n` (any remainder goes to the smallest
/// names). Exponent 1.4 gives, at 10^7 over this generator's 1,158 names, a
/// median of 696 ids, 61% of names under 1,000 and 16 names at 10^5 or more,
/// 3 of them at the cap.
fn skewed_sizes(names: usize, n: usize) -> Vec<u64> {
    const S: f64 = 1.4;
    const CAP: u64 = 1_000_000;
    let size = |a: f64, r: usize| ((a * (r as f64).powf(-S)) as u64).clamp(1, CAP);
    let (mut lo, mut hi) = (1.0f64, 1e12f64);
    for _ in 0..200 {
        let a = (lo * hi).sqrt();
        let total: u64 = (1..=names).map(|r| size(a, r)).sum();
        if total < n as u64 {
            lo = a;
        } else {
            hi = a;
        }
    }
    let mut sizes: Vec<u64> = (1..=names).map(|r| size(lo, r)).collect();
    let mut short = n as u64 - sizes.iter().sum::<u64>();
    let mut r = names;
    while short > 0 {
        r = if r == 0 { names - 1 } else { r - 1 };
        sizes[r] += 1;
        short -= 1;
    }
    sizes
}

/// The downstream composite shape of the #1257 follow-up:
/// `prefix(5 B) ‖ name ‖ '/' ‖ id`, prefix in [`enc7`], names
/// `[A-Za-z0-9_]`, and `'/'` below every name byte. Both distributions are
/// assumptions, not measured traffic:
///
/// - `uniform`: 100 prefixes × 100 names of 1–32 bytes (drawn once, shared
///   by every prefix) × `n / 10_000` sequential ids per name;
/// - `skewed`: 100 prefixes, each with its own 5–20 names of 4–20 bytes,
///   and sequential ids per name sized by [`skewed_sizes`]; ranks are
///   assigned to names by a seeded shuffle, so large names fall anywhere.
///
/// Emitted sorted.
fn composite_keys(n: usize, dist: &str, enc: IdEncoding) -> Vec<Vec<u8>> {
    let mut rng = XorShift(SEED ^ 0xC0);
    // (prefix, name, ids) triples.
    let mut groups: Vec<(u64, Vec<u8>, u64)> = Vec::new();
    match dist {
        "uniform" => {
            let mut names: Vec<Vec<u8>> = Vec::new();
            while names.len() < 100 {
                let name = random_name(&mut rng, 1, 32);
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            let per_name = (n / 10_000).max(1) as u64;
            for prefix in 0..100u64 {
                for name in &names {
                    groups.push((prefix, name.clone(), per_name));
                }
            }
        }
        "skewed" => {
            for prefix in 0..100u64 {
                let count = 5 + (rng.next() % 16) as usize;
                let mut names: Vec<Vec<u8>> = Vec::new();
                while names.len() < count {
                    let name = random_name(&mut rng, 4, 20);
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
                for name in names {
                    groups.push((prefix, name, 0));
                }
            }
            let sizes = shuffled(&skewed_sizes(groups.len(), n), SEED ^ 0x51);
            for (g, size) in groups.iter_mut().zip(sizes) {
                g.2 = size;
            }
        }
        _ => unreachable!("unknown composite distribution {dist}"),
    }
    let mut out = Vec::with_capacity(n);
    for (prefix, name, ids) in &groups {
        for id in 0..*ids {
            let mut k: Vec<u8> = if enc.prefix_too {
                let mut p = Vec::new();
                IdEncoding {
                    width: 7,
                    align7: false,
                    ..enc
                }
                .push(*prefix, &mut p);
                p
            } else {
                enc7(*prefix, 5).collect()
            };
            k.extend_from_slice(name);
            k.push(b'/');
            enc.push(id, &mut k);
            out.push(k);
        }
    }
    // `'/'` sorts below every name byte, so sorting the keys orders names
    // within a prefix; ids are fixed-width, so they sort numerically.
    out.sort_unstable();
    out
}

/// `orders` transcoded (#1257 follow-up): the same keys as `text_keys("orders")`
/// with each fixed-width decimal field rewritten order-preservingly in base-32
/// digits `0x01..=0x20` — the `%03d` tenant in 2 digits, the `%010d` id
/// (below 32^7) padded to the width that makes the key 7 (mod 8) bytes
/// long. `t` + 2 + `:orders:` is 11 bytes, so the id takes 12 digits.
fn orders_b32_keys(n: usize) -> Vec<Vec<u8>> {
    let per_t = (n / 1000).max(1);
    let enc = IdEncoding {
        name: "orders_b32a7",
        radix: 32,
        width: 7,
        base_byte: 0x01,
        align7: true,
        prefix_too: false,
    };
    (0..n)
        .map(|i| {
            let mut k = vec![b't'];
            IdEncoding {
                width: 2,
                align7: false,
                ..enc
            }
            .push((i / per_t) as u64, &mut k);
            k.extend_from_slice(b":orders:");
            enc.push(i as u64, &mut k);
            debug_assert_eq!(k.len() % CHUNK_BYTES, CHUNK_BYTES - 1);
            k
        })
        .collect()
}

/// A mixed `orders` range (#1257 follow-up): each id is either transcoded
/// behind a leading `0x01` marker byte — `0x01` ‖ `t` ‖ 2 base-32 digits ‖
/// `:orders:` ‖ the id padded to the 7 (mod 8) whole-key alignment, which
/// with the marker is 11 digits — or, for a seeded random `escaped_pct`
/// percent of ids, kept as its text key. The marker sorts every encoded key
/// below every text one. Returns (encoded, escaped), each sorted.
fn orders_mixed_keys(n: usize, escaped_pct: u64) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let per_t = (n / 1000).max(1);
    let digits = |v: u64, width: usize, out: &mut Vec<u8>| {
        IdEncoding {
            name: "b32",
            radix: 32,
            width,
            base_byte: 0x01,
            align7: false,
            prefix_too: false,
        }
        .push(v, out);
    };
    let mut rng = XorShift(SEED ^ 0xE5C);
    let (mut encoded, mut escaped) = (Vec::new(), Vec::new());
    for i in 0..n {
        let t = (i / per_t) as u64;
        if rng.next() % 100 < escaped_pct {
            escaped.push(format!("t{t:03}:orders:{i:010}").into_bytes());
        } else {
            let mut k = vec![0x01, b't'];
            digits(t, 2, &mut k);
            k.extend_from_slice(b":orders:");
            digits(i as u64, 11, &mut k);
            debug_assert_eq!(k.len() % CHUNK_BYTES, CHUNK_BYTES - 1);
            encoded.push(k);
        }
    }
    (encoded, escaped)
}

/// Writes `keys` as a u64 count, then each key as a u32 length and its bytes
/// (little-endian); keys may hold any byte but NUL, including newlines.
fn write_keys(path: &std::path::Path, keys: &[Vec<u8>]) {
    let mut buf = Vec::with_capacity(8 + keys.iter().map(|k| 4 + k.len()).sum::<usize>());
    buf.extend_from_slice(&(keys.len() as u64).to_le_bytes());
    for k in keys {
        buf.extend_from_slice(&(k.len() as u32).to_le_bytes());
        buf.extend_from_slice(k);
    }
    std::fs::write(path, buf).expect("write key file");
}

/// The key sets the W3 timing runs on (`benches/leaf_layout_timing.rs`,
/// #1257 follow-up), written by `--dump-keys <dir>` as `<arm>.keys` and
/// `<arm>.miss`, so the timed keys are the censused keys byte for byte.
///
/// Misses are drawn from each shape's own generator and are absent by
/// construction (§8.6: never a fixed transform of a present key):
/// - `orders`: a tenant and an id each drawn uniformly from the shape's own
///   ranges, rejected when that id belongs to that tenant (then it is
///   present), so a miss shares the tenant prefix and diverges in the id;
/// - `composite`: the name's sequential ids continued past its population
///   (ids `per_name..2·per_name`), the offset form §8.6 names for a
///   sequential keyspace.
///
/// One draw of logical misses is encoded both ways, so the text and the
/// encoded arm of a shape probe the same misses.
fn dump_keys(dir: &std::path::Path, n: usize) {
    std::fs::create_dir_all(dir).expect("create key dir");
    const MISSES: usize = 1_000_000;
    let per_t = (n / 1000).max(1);
    let mut rng = XorShift(SEED ^ 0x3155);
    let mut pairs = Vec::with_capacity(MISSES);
    while pairs.len() < MISSES {
        let t = rng.next() % 1000;
        let id = rng.next() % n as u64;
        if id / per_t as u64 != t {
            pairs.push((t, id));
        }
    }
    let text_miss: Vec<Vec<u8>> = pairs
        .iter()
        .map(|&(t, id)| format!("t{t:03}:orders:{id:010}").into_bytes())
        .collect();
    let b32 = |v: u64, width: usize, out: &mut Vec<u8>| {
        IdEncoding {
            name: "b32",
            radix: 32,
            width,
            base_byte: 0x01,
            align7: false,
            prefix_too: false,
        }
        .push(v, out);
    };
    let enc_miss: Vec<Vec<u8>> = pairs
        .iter()
        .map(|&(t, id)| {
            let mut k = vec![b't'];
            b32(t, 2, &mut k);
            k.extend_from_slice(b":orders:");
            b32(id, 12, &mut k);
            k
        })
        .collect();
    write_keys(&dir.join("orders_text.keys"), &str_keys("orders", n));
    write_keys(&dir.join("orders_text.miss"), &text_miss);
    write_keys(&dir.join("orders_b32a7.keys"), &orders_b32_keys(n));
    write_keys(&dir.join("orders_b32a7.miss"), &enc_miss);
    for enc in [ENCODINGS[0], ENCODINGS[4]] {
        let keys = composite_keys(n, "uniform", enc);
        // Misses: every name's ids continued past its population, sampled.
        let per_name = (n / 10_000).max(1) as u64;
        let mut miss = Vec::with_capacity(MISSES);
        let mut rng = XorShift(SEED ^ 0x3156);
        for _ in 0..MISSES {
            // A present key's prefix and name, with an id past the population.
            let k = &keys[(rng.next() % keys.len() as u64) as usize];
            let id_width = if enc.align7 {
                // The aligned width varies per name: recover it from the key.
                let slash = k.iter().rposition(|&b| b == b'/').unwrap();
                k.len() - slash - 1
            } else {
                enc.width
            };
            let mut m = k[..k.len() - id_width].to_vec();
            let id = per_name + rng.next() % per_name;
            IdEncoding {
                width: id_width,
                align7: false,
                ..enc
            }
            .push(id, &mut m);
            miss.push(m);
        }
        let arm = format!("composite_uniform_{}", enc.name);
        write_keys(&dir.join(format!("{arm}.keys")), &keys);
        write_keys(&dir.join(format!("{arm}.miss")), &miss);
    }
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
    // (key bytes, slot level, keys, byte cards) -> (groups, bytes).
    type GroupKey = (u8, u8, usize, [u16; 7]);
    let mut agg: std::collections::BTreeMap<GroupKey, (usize, usize)> =
        std::collections::BTreeMap::new();
    for (g, n) in &c.merge_groups {
        let e = agg
            .entry((g.key_bytes, g.slot_level, g.keys, g.byte_cards))
            .or_insert((0, 0));
        e.0 += n;
        e.1 += n * g.bytes;
    }
    let rows: Vec<String> = agg
        .iter()
        .map(|((kb, slot, keys, cards), (n, bytes))| {
            let cards: Vec<String> = cards[..*kb as usize]
                .iter()
                .map(u16::to_string)
                .collect();
            format!(
                "{{\"key_bytes\": {kb}, \"slot_level\": {slot}, \"keys\": {keys}, \"byte_cards\": [{}], \"count\": {n}, \"bytes\": {bytes}}}",
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
    // The id-encoding comparison of the #1257 follow-up: both composite
    // distributions under every encoding, 10^7 keys (or `--population`),
    // loaded sorted.
    if args.iter().any(|a| a == "--encodings") {
        let dn = args
            .iter()
            .position(|a| a == "--population")
            .map_or(10_000_000, |_| n);
        // Each cell's keys are generated when it runs, so only one key set
        // is resident at a time.
        type KeyGen = Box<dyn Fn() -> Vec<Vec<u8>>>;
        let mut shapes: Vec<(String, KeyGen)> = Vec::new();
        for dist in ["uniform", "skewed"] {
            for &enc in ENCODINGS {
                shapes.push((
                    format!("composite_{dist}_{}_{dn}", enc.name),
                    Box::new(move || composite_keys(dn, dist, enc)),
                ));
            }
        }
        shapes.push((
            format!("orders_text_{dn}"),
            Box::new(move || str_keys("orders", dn)),
        ));
        shapes.push((
            format!("orders_b32a7_{dn}"),
            Box::new(move || orders_b32_keys(dn)),
        ));
        for (shape, keys) in &shapes {
            let keys = keys();
            let m = build_str(&keys);
            record(
                &mut out,
                run,
                "strmap",
                shape,
                "sorted",
                m.mem_used(),
                |r| m.layout_census(r),
            );
            drop(m);
            let mix = shuffled(&keys, SEED ^ 0x5F);
            drop(keys);
            let m = build_str(&mix);
            drop(mix);
            record(
                &mut out,
                run,
                "strmap",
                &format!("{shape}_shuffled"),
                "shuffled",
                m.mem_used(),
                |r| m.layout_census(r),
            );
        }
    }

    // Mixed ranges (#1257 follow-up): transcoded `orders` with 0%, 1%, 10%
    // and 50% of ids escaped to text, as one map, and each part alone so the
    // census can attribute the mixture.
    if args.iter().any(|a| a == "--mixed") {
        let dn = args
            .iter()
            .position(|a| a == "--population")
            .map_or(10_000_000, |_| n);
        for pct in [0u64, 1, 10, 50] {
            let (encoded, escaped) = orders_mixed_keys(dn, pct);
            let mut all = encoded.clone();
            all.extend(escaped.iter().cloned());
            for (part, keys) in [
                ("mixed", &all),
                ("encoded_part", &encoded),
                ("escaped_part", &escaped),
            ] {
                if keys.is_empty() {
                    continue;
                }
                let m = build_str(keys);
                record(
                    &mut out,
                    run,
                    "strmap",
                    &format!("orders_escaped{pct}pct_{part}_{dn}"),
                    "sorted",
                    m.mem_used(),
                    |r| m.layout_census(r),
                );
            }
        }
    }

    if let Some(i) = args.iter().position(|a| a == "--dump-keys") {
        let dn = args
            .iter()
            .position(|a| a == "--population")
            .map_or(10_000_000, |_| n);
        dump_keys(std::path::Path::new(&args[i + 1]), dn);
        return;
    }

    let synthetic = !args
        .iter()
        .any(|a| a == "--sosd-only" || a == "--downstream" || a == "--encodings" || a == "--mixed");

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
        for shape in ["orders", "composite", "composite_skewed"] {
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
