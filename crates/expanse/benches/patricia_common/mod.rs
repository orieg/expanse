//! Shared helpers for the Patricia trie vs Expanse suite
//! (`docs/benchmarks/patricia_comparison/`). Key generators, PRNG, BCa and
//! `rounds_raw` come from `art_common`, so both suites draw identical keys.
//!
//! Competitor twin: [`patricia_tree::PatriciaMap`] 0.10.2 — a byte-labelled
//! Patricia (compressed radix) tree after Morrison, J. ACM 1968. Each node is
//! one allocation holding its label, optional value, first child and next
//! sibling; children form a byte-sorted singly linked list. Integer keys are
//! fed as their big-endian 8 bytes, so byte order equals numeric order on both
//! arms.

#![allow(dead_code)]

pub use patricia_tree::PatriciaMap;

use crate::art_common::XorShift64;
use expanse_trie::strmap::NulFreeStr;

/// Big-endian key bytes for a `u64` (the encoding the Patricia arm stores).
#[inline(always)]
pub fn pkey(k: u64) -> [u8; 8] {
    k.to_be_bytes()
}

/// Dedicated seed for the string-key generator.
pub const STRING_SEED: u64 = 0x5A71_C1A0_0000_0001;

/// Shared-prefix path keys: `https://example.com/api/v2/objects/<12 hex>`.
///
/// A 34-byte prefix common to every key, then 48 random bits. This is the
/// regime a Patricia tree compresses to one node, and the regime the suite
/// pre-registers as the twin's plausible win (AGENTS.md §8.3, C-b). Keys are
/// distinct and NUL-free by construction.
pub fn gen_prefixed_paths(n: usize, rng: &mut XorShift64) -> Vec<Vec<u8>> {
    let mut seen = std::collections::HashSet::with_capacity(n);
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        let id = rng.next() & 0xFFFF_FFFF_FFFF;
        if seen.insert(id) {
            out.push(format!("https://example.com/api/v2/objects/{id:012x}").into_bytes());
        }
    }
    out
}

/// Same-generator misses for [`gen_prefixed_paths`], rejected on membership
/// (AGENTS.md §8.6 miss shape): same prefix, same length, same id width.
pub fn gen_prefixed_path_misses(present: &[Vec<u8>], n: usize, seed: u64) -> Vec<Vec<u8>> {
    let set: std::collections::HashSet<&[u8]> = present.iter().map(|k| k.as_slice()).collect();
    let mut rng = XorShift64::new(seed);
    let mut out = Vec::with_capacity(n);
    let mut seen = std::collections::HashSet::with_capacity(n);
    let budget = n.saturating_mul(64).saturating_add(1024);
    let mut tries = 0;
    while out.len() < n {
        tries += 1;
        assert!(
            tries <= budget,
            "could not draw {n} distinct absent path keys"
        );
        let id = rng.next() & 0xFFFF_FFFF_FFFF;
        let k = format!("https://example.com/api/v2/objects/{id:012x}").into_bytes();
        if !set.contains(k.as_slice()) && seen.insert(id) {
            out.push(k);
        }
    }
    out
}

/// Validates every key once, outside any timed region, so the Expanse arm's
/// timed loop does not pay a NUL scan per probe that the Patricia arm does not.
pub fn as_nulfree(keys: &[Vec<u8>]) -> Vec<&NulFreeStr> {
    keys.iter()
        .map(|k| NulFreeStr::new(k).expect("generated keys are NUL-free"))
        .collect()
}

// ---------------------------------------------------------------------------
// Timed point-lookup kernels shared by `patricia_lookup_hit` and
// `patricia_lookup_miss`. Each returns ns/op over one pass of `probes`; every
// result reaches `black_box` (AGENTS.md §8.6).
// ---------------------------------------------------------------------------

use crate::art_common::{ExpanseMap, bca_ci, median, rounds_raw};
use std::hint::black_box;
use std::time::Instant;

#[inline(never)]
pub fn time_expanse_get(map: &ExpanseMap, probes: &[u64]) -> f64 {
    let start = Instant::now();
    for &k in probes {
        black_box(map.get(k).unwrap_or(0));
    }
    start.elapsed().as_nanos() as f64 / probes.len() as f64
}

/// Probes are pre-encoded big-endian outside the timed region, matching the
/// Expanse arm, which receives its keys ready to use.
#[inline(never)]
pub fn time_patricia_get(map: &PatriciaMap<u64>, probes: &[[u8; 8]]) -> f64 {
    let start = Instant::now();
    for k in probes {
        black_box(map.get(k).copied().unwrap_or(0));
    }
    start.elapsed().as_nanos() as f64 / probes.len() as f64
}

/// Builds both arms over `keys`, then times `probes` for `rounds` rounds,
/// alternating which arm runs first, and returns the result row.
pub fn lookup_row(
    dist: &str,
    keys: &[u64],
    probes: &[u64],
    hit_pct: u32,
    rounds: usize,
) -> serde_json::Value {
    let mut e = ExpanseMap::new();
    let mut p = PatriciaMap::new();
    for &k in keys {
        e.insert(k, k.wrapping_mul(3));
        p.insert(pkey(k), k.wrapping_mul(3));
    }
    let pprobes: Vec<[u8; 8]> = probes.iter().map(|&k| pkey(k)).collect();

    let mut et = Vec::with_capacity(rounds);
    let mut pt = Vec::with_capacity(rounds);
    let mut ratios = Vec::with_capacity(rounds);
    for round in 0..rounds {
        let (a, b) = if round % 2 == 0 {
            let a = time_expanse_get(&e, probes);
            (a, time_patricia_get(&p, &pprobes))
        } else {
            let b = time_patricia_get(&p, &pprobes);
            (time_expanse_get(&e, probes), b)
        };
        et.push(a);
        pt.push(b);
        if b > 0.0 {
            ratios.push(a / b);
        }
    }
    let (r, lo, hi) = bca_ci(&ratios);
    serde_json::json!({
        "distribution": dist,
        "population": keys.len(),
        "probes": probes.len(),
        "hit_rate_pct": hit_pct,
        "expanse_ns_op": median(et.clone()),
        "patricia_ns_op": median(pt.clone()),
        "ratio_expanse_over_patricia": r,
        "ratio_ci": [lo, hi],
        "ratio_ci_method": ci_method(ratios.len()),
        "rounds_raw": rounds_raw(&[
            ("expanse_ns", &et),
            ("patricia_ns", &pt),
            ("ratio_expanse_over_patricia", &ratios),
        ]),
    })
}

/// Prints the console table shared by the lookup harnesses.
pub fn print_rows(title: &str, rows: &[serde_json::Value]) {
    println!("=== {title} ===");
    for r in rows {
        let ci = &r["ratio_ci"];
        println!(
            "  pop={:8} | {:15} | Expanse {:8.2} ns | Patricia {:8.2} ns | Exp/Pat {:6.3} [{:.3}, {:.3}]",
            r["population"],
            r["distribution"].as_str().unwrap_or("?"),
            r["expanse_ns_op"].as_f64().unwrap_or(f64::NAN),
            r["patricia_ns_op"].as_f64().unwrap_or(f64::NAN),
            r["ratio_expanse_over_patricia"]
                .as_f64()
                .unwrap_or(f64::NAN),
            ci[0].as_f64().unwrap_or(f64::NAN),
            ci[1].as_f64().unwrap_or(f64::NAN),
        );
    }
}

/// Parses `--quick` / `--json` and returns (populations, rounds, quick, json).
pub fn cli() -> (&'static [usize], usize, bool, bool) {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json = args.iter().any(|a| a == "--json");
    if quick {
        (&[10_000, 50_000], 3, quick, json)
    } else {
        (&[10_000, 100_000, 1_000_000], 15, quick, json)
    }
}

/// `art_common::bca_ci` returns the plain mean as a zero-width interval below
/// three samples; label that case instead of calling it BCa (AGENTS.md §8.4).
pub fn ci_method(samples: usize) -> &'static str {
    if samples >= 3 { "bca" } else { "degenerate" }
}
