//! `mem_used` is a property of the population, never of the order it arrived in.
//!
//! The shared benchmark generators sort a population before handing it to any
//! arm, and every comparative suite in this repository builds in that order. On
//! a Fisher–Yates permutation of the same keys the *allocator* footprint moves
//! on both arms — the `masstree_comparison` sensitivity set measured
//! `ExpanseMap` at 16.67 → 23.63 B/key — because glibc `malloc` hands back
//! different spans for the same request sequence in a different order.
//!
//! The engine's own node census must not move: `mem_used` counts the nodes the
//! trie holds, and a digital trie's shape is fixed by the key set, not by the
//! sequence the keys were inserted in. That is what makes `mem_used` the
//! order-invariant instrument of the pair and the allocator census the
//! order-sensitive one, and it is why a sorted/shuffled sensitivity table can
//! report the two side by side and attribute the difference to the allocator.
//!
//! Pinned here rather than in the benchmark crate because `expanse-hot-bench` is
//! detached from the workspace and no CI lane compiles it (#733, AGENTS.md
//! §8.12). Deterministic integer accounting, so these are exact equalities —
//! §8.4's prohibition on hard assertions covers continuous wall-clock estimates,
//! not exact byte counts.

use expanse_trie::bytesmap::ExpanseBytesMap;
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use expanse_trie::strmap::ExpanseStrMap;

/// XorShift64, the generator the comparative suites use.
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

/// A sorted, deduplicated population, exactly as the shared generator produces.
fn population(n: usize) -> Vec<u64> {
    let mut rng = XorShift(SEED);
    let mut v: Vec<u64> = (0..n).map(|_| rng.next()).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Fisher–Yates from a seed derived off the suite seed, matching
/// `expanse_hot_bench::workload::shuffle_in_place`.
fn shuffled(v: &[u64]) -> Vec<u64> {
    let mut out = v.to_vec();
    let mut rng = XorShift(SEED ^ 0x0BDE_B000_0000_0661);
    for j in (1..out.len()).rev() {
        let k = (rng.next() % (j as u64 + 1)) as usize;
        out.swap(j, k);
    }
    out
}

#[test]
fn map_mem_used_is_identical_in_both_insertion_orders() {
    let sorted = population(200_000);
    let shuf = shuffled(&sorted);
    assert_ne!(sorted, shuf, "the permutation must actually reorder");

    let mut a = ExpanseMap::new();
    for (i, k) in sorted.iter().enumerate() {
        a.insert(*k, i as u64);
    }
    let mut b = ExpanseMap::new();
    for k in &shuf {
        // The value is keyed off the key, not the position, so both tries hold
        // the identical key -> value mapping and only the order differs.
        b.insert(*k, *k);
    }

    assert_eq!(a.len(), b.len(), "both tries hold the same population");
    assert_eq!(
        a.mem_used(),
        b.mem_used(),
        "ExpanseMap node census moved with insertion order: sorted {} B, shuffled {} B",
        a.mem_used(),
        b.mem_used()
    );
}

#[test]
fn set_mem_used_is_identical_in_both_insertion_orders() {
    let sorted = population(200_000);
    let shuf = shuffled(&sorted);

    let mut a = ExpanseSet::new();
    for k in &sorted {
        a.insert(*k);
    }
    let mut b = ExpanseSet::new();
    for k in &shuf {
        b.insert(*k);
    }

    assert_eq!(a.len(), b.len());
    assert_eq!(
        a.mem_used(),
        b.mem_used(),
        "ExpanseSet node census moved with insertion order: sorted {} B, shuffled {} B",
        a.mem_used(),
        b.mem_used()
    );
}

/// The string keyspace the `hot_comparison` `short` shape draws from.
fn string_population(n: usize) -> Vec<Vec<u8>> {
    let mut rng = XorShift(SEED);
    let mut v: Vec<Vec<u8>> = (0..n)
        .map(|_| format!("key_{:012x}", rng.next() & 0xFFFF_FFFF_FFFF).into_bytes())
        .collect();
    v.sort();
    v.dedup();
    v
}

fn shuffled_strings(v: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let mut out = v.to_vec();
    let mut rng = XorShift(SEED ^ 0x0BDE_B000_0000_0661);
    for j in (1..out.len()).rev() {
        let k = (rng.next() % (j as u64 + 1)) as usize;
        out.swap(j, k);
    }
    out
}

#[test]
fn strmap_mem_used_is_identical_in_both_insertion_orders() {
    let sorted = string_population(50_000);
    let shuf = shuffled_strings(&sorted);
    assert_ne!(sorted, shuf, "the permutation must actually reorder");

    let mut a = ExpanseStrMap::new();
    for k in &sorted {
        a.insert(k, 1);
    }
    let mut b = ExpanseStrMap::new();
    for k in &shuf {
        b.insert(k, 1);
    }

    assert_eq!(a.len(), b.len());
    assert_eq!(
        a.mem_used(),
        b.mem_used(),
        "ExpanseStrMap node census moved with insertion order: sorted {} B, shuffled {} B",
        a.mem_used(),
        b.mem_used()
    );
}

/// `ExpanseBytesMap` needs its hasher held fixed for this to be a test of
/// insertion order at all.
///
/// The map hashes each key and indexes an `ExpanseMap` by the hash, and its
/// default `std::hash::RandomState` is seeded per instance. Two maps built with
/// `new()` therefore hold two *different key sets* in their inner trie, so their
/// censuses differ whatever order the byte strings arrived in — this test first
/// asserted over `new()` and failed at 4,311,568 B against 4,310,768 B, which
/// was two hash seeds, not two orders. `with_hasher` pins the seed so the only
/// variable left is the order.
#[test]
fn bytesmap_mem_used_is_identical_in_both_insertion_orders() {
    let sorted = string_population(50_000);
    let shuf = shuffled_strings(&sorted);

    // Same `BuildHasher` value on both sides: the inner trie then holds the
    // identical hash key set and only the insertion order differs.
    let seed = std::hash::RandomState::new();
    let mut a = ExpanseBytesMap::with_hasher(seed.clone());
    for k in &sorted {
        a.insert(k, 1);
    }
    let mut b = ExpanseBytesMap::with_hasher(seed);
    for k in &shuf {
        b.insert(k, 1);
    }

    assert_eq!(a.len(), b.len());
    assert_eq!(
        a.mem_used(),
        b.mem_used(),
        "ExpanseBytesMap node census moved with insertion order: sorted {} B, shuffled {} B",
        a.mem_used(),
        b.mem_used()
    );
}
