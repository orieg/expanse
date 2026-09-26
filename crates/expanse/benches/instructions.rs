//! Deterministic cost benchmarks: instructions retired, memory accesses
//! and simulated cache behaviour, via callgrind (`docs/BENCHMARKING.md`,
//! issue #1).
//!
//! Why not wall-clock: the measured noise floor of both available
//! environments (CI runners, the development laptop) is ~15-20% at n=2,
//! while every optimization on the roadmap is worth a few percent. A
//! `memcmp` removal that looked like a 7-11% win at n=1 showed no
//! detectable effect at n=2 — that is the failure mode this harness
//! exists to prevent. Callgrind counts are **exact and reproducible**:
//! the same binary on the same input yields the same number on a loaded
//! laptop and an idle runner alike, so a 1% change is legible.
//!
//! Read the numbers as *cost*, not time: fewer instructions or fewer
//! cache misses is strictly better work, but the wall-clock effect
//! depends on how well the machine hides the remaining latency. A
//! wall-clock claim still requires a quiet host (BENCHMARKING.md).
//!
//! Requires valgrind, which does not support arm64 macOS — these run on
//! Linux, in the `instruction-counts` CI job. Locally:
//! `cargo bench --bench instructions` on a Linux host.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `core_instructions` |
//! | `group` | 2 |
//! | `population` | 50k; the `sync_*` arms' `leaf` variants build `LEAF_POP` (24) keys, below `ROOT_LEAF_CAP`, so the map or set stays a root leaf; `sync_strmap_insert_sorted` builds `UUID_POP` (20k) UUIDv4 strings; the remove-retention arms: `*_remove_partial` builds 200k random 60-bit keys, `*_rebuild_drained` and `*_compact_drained` the same tree drained to 62.5k by those removes, the `set_subtree_*` arms 64,512 one-key prefixes plus `SUBTREE_E` (1,024) driven level-6 expanses of 25–33 keys |
//! | `insertion_order` | generator draw order — the population is inserted as drawn, neither sorted nor shuffled; the shuffle in this file is applied to the probe stream, not to the build. Exception: `sync_strmap_insert_sorted` inserts its keys sorted ascending, the order #1162 reported |
//! | `probes_and_reuse` | 50k (shuffled), reuse 1.0; `leaf` variants cycle their 24 keys to 50k probes (reuse ≈ 2,083); the concurrent count arms take the first `COUNT_OPS` (1,000); `*_remove_partial` removes 137,500 keys in a Fisher–Yates order; `*_rebuild_drained` clones the drained tree once (62,500 keys) and drops the drained one; `*_compact_drained` compacts it in place once; the `set_subtree_*` arms make one operation per driven expanse, or `OSC_CYCLES` (8) cycles of 2 × band operations per expanse |
//! | `hit_rate` | 100% |
//! | `miss_gen_method` | None for reads; the concurrent count arms write absent keys drawn from the population's distribution and rejected on membership (`fresh_keys`) |
//! | `value_dereference` | `black_box` on retrieved values |
//! | `measured_region` | Clean (setup in setup) |
//! | `arm_symmetry` | Internal trie paths |
//! | `statistics` | iai Callgrind exact counts |
//! | `verdict` | **PASS** `[verified: RUN (CI instruction-counts)]`: Canonical instruction reference. |

// The `library_benchmark` macro expands to modules that carry no docs of
// their own; the workspace `missing_docs` lint does not apply to a bench
// harness.
#![allow(missing_docs)]

use expanse_trie::blobmap::ExpanseBlobMap;
use expanse_trie::bytesmap::ExpanseBytesMap;
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use expanse_trie::strmap::ExpanseStrMap;
use expanse_trie::sync::{
    SyncExpanseBlobMap, SyncExpanseBytesMap, SyncExpanseMap, SyncExpanseSet, SyncExpanseStrMap,
};
use expanse_trie::{ExpanseBlobMap32, ExpanseMap32, ExpanseSet32, Key32, Value32};
#[cfg(target_os = "linux")]
use iai_callgrind::main;
use iai_callgrind::{
    Callgrind, LibraryBenchmarkConfig, library_benchmark, library_benchmark_group,
};
use std::hint::black_box;

/// Wraps a key for `ExpanseStrMap`.
///
/// `new_unchecked`: the validating constructor would put a whole-key scan
/// inside the measured region, and the arm would measure the check instead of
/// the descent. Every generator in this file emits route-shaped ASCII.
#[inline(always)]
fn tk<B: AsRef<[u8]> + ?Sized>(bytes: &B) -> &expanse_trie::strmap::NulFreeStr {
    // SAFETY: generators in this file emit no NUL bytes.
    unsafe { expanse_trie::strmap::NulFreeStr::new_unchecked(bytes.as_ref()) }
}

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

/// Population per benchmark. Small enough that callgrind (~50x slowdown)
/// stays practical, large enough to build a real multi-level trie with
/// branches, leaves and the compression ladder all exercised.
const POP: usize = 50_000;

fn keys(dist: &str) -> Vec<u64> {
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut out = Vec::with_capacity(POP);
    match dist {
        "sequential" => out.extend(0..POP as u64),
        "random" => out.extend((0..POP).map(|_| rng.next())),
        "clustered" => {
            let mut base = 0;
            for i in 0..POP as u64 {
                if i % 256 == 0 {
                    base = rng.next() & !0xFF;
                }
                out.push(base + (i % 256));
            }
        }
        // Small populations that stay in immediates and short leaves —
        // the terminal forms most inserts actually touch.
        "small" => out.extend((0..POP as u64).map(|i| (i % 12) | ((i / 12) << 32))),
        "dense_leaf" => {
            // Generates runs of exactly 32 keys sharing prefixes, creating bitmap leaves (pop > 25).
            for _ in 0..(POP / 32) {
                let prefix = rng.next() & !0xFF;
                for j in 0..32 {
                    out.push(prefix | (j as u64));
                }
            }
        }
        "linear_leaf" => {
            // Generates runs of exactly 15 keys sharing prefixes, creating linear leaves in the 16-element SIMD vector scan band.
            for _ in 0..(POP / 15) {
                let prefix = rng.next() & !0xFF;
                for j in 0..15 {
                    out.push(prefix | (j as u64));
                }
            }
        }
        // Uniform keys confined to one top byte (#1144): every key shares
        // `digit(key, 8)`, so the concurrent map's per-top-digit dirty mask
        // has one bit for the whole population.
        "one_top_byte" => out.extend((0..POP).map(|_| ONE_TOP_BYTE | (rng.next() >> 8))),
        other => panic!("unknown distribution {other}"),
    }
    out
}

/// The top byte every `one_top_byte` key carries.
const ONE_TOP_BYTE: u64 = 0xA5 << 56;

/// `POP` keys absent from `keys(dist)`, drawn from the same distribution
/// (§8.6 miss shape): `sequential` continues past the population, the others
/// draw from a second seed and reject present keys. A write of one of these
/// lands where a write of a new population key would.
fn fresh_keys(dist: &str) -> Vec<u64> {
    let present: std::collections::HashSet<u64> = keys(dist).into_iter().collect();
    let mut rng = XorShift(0x5EED_F2E5_0000_0002);
    let mut out = Vec::with_capacity(POP);
    while out.len() < POP {
        let k = match dist {
            "sequential" => (POP + out.len()) as u64,
            "random" => rng.next(),
            "one_top_byte" => ONE_TOP_BYTE | (rng.next() >> 8),
            other => panic!("no fresh-key generator for {other}"),
        };
        if !present.contains(&k) {
            out.push(k);
        }
    }
    out
}

/// A prebuilt map plus a probe order that is not the build order (a
/// sequential probe would measure the prefetcher, not the lookup).
fn built_map(dist: &str) -> (ExpanseMap, Vec<u64>) {
    let ks = keys(dist);
    let mut map = ExpanseMap::new();
    for &k in &ks {
        map.insert(k, !k);
    }
    let mut probes = ks;
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    (map, probes)
}

fn built_set(dist: &str) -> (ExpanseSet, Vec<u64>) {
    let ks = keys(dist);
    let mut set = ExpanseSet::new();
    for &k in &ks {
        set.insert(k);
    }
    let mut probes = ks;
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    (set, probes)
}

// ---- Insert: the larger gap vs stock (issue #1) ----------------------
//
// `keys()` runs in `setup`, not in the body: 50k xorshift steps plus a
// `Vec` growth were being counted as insert work. And the built structure
// is leaked rather than dropped, because a full teardown at the end of
// the body is a different code path measured under the insert label —
// per-function profiles showed `free_subtree` inside the lookup arms,
// which is how this was found. Each benchmark is its own process.

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = keys)]
#[bench::random(args = ("random",), setup = keys)]
#[bench::clustered(args = ("clustered",), setup = keys)]
#[bench::small(args = ("small",), setup = keys)]
#[bench::dense_leaf(args = ("dense_leaf",), setup = keys)]
#[bench::linear_leaf(args = ("linear_leaf",), setup = keys)]
fn map_insert(ks: Vec<u64>) -> u64 {
    let mut map = ExpanseMap::new();
    for &k in &ks {
        map.insert(black_box(k), black_box(!k));
    }
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = keys)]
#[bench::random(args = ("random",), setup = keys)]
#[bench::clustered(args = ("clustered",), setup = keys)]
#[bench::dense_leaf(args = ("dense_leaf",), setup = keys)]
#[bench::linear_leaf(args = ("linear_leaf",), setup = keys)]
fn set_insert(ks: Vec<u64>) -> u64 {
    let mut set = ExpanseSet::new();
    for &k in &ks {
        set.insert(black_box(k));
    }
    let n = set.len();
    core::mem::forget(set);
    black_box(n)
}

// The fused single-walk `JudyLIns` path the compat layer uses.
#[library_benchmark]
#[bench::random(args = ("random",), setup = keys)]
fn map_ins_slot(ks: Vec<u64>) -> u64 {
    let mut map = ExpanseMap::new();
    for &k in &ks {
        let slot = map.ins_slot(black_box(k));
        // SAFETY: valid until the next mutation; written immediately.
        unsafe { slot.as_ptr().write(!k) };
    }
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

// ---- Lookup ----------------------------------------------------------

// `setup =` keeps the build out of the measured region — without it these
// counted the build too, and the "lookup" number was mostly insert.

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = built_map)]
#[bench::random(args = ("random",), setup = built_map)]
#[bench::clustered(args = ("clustered",), setup = built_map)]
#[bench::dense_leaf(args = ("dense_leaf",), setup = built_map)]
#[bench::linear_leaf(args = ("linear_leaf",), setup = built_map)]
fn map_get(built: (ExpanseMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for &k in &probes {
        sink ^= map.get(black_box(k)).unwrap_or(0);
    }
    // Leaked: taking the map by value dropped it here, so every "lookup"
    // count included a full `free_subtree` walk. On the random arm that
    // was ~10% of the reported number.
    core::mem::forget(map);
    black_box(sink)
}

// The batched descent (#430) over the same map and the same probe order as
// `map_get`, so the two arms are directly comparable.
//
// A HIGHER count here is the expected shape, not a regression. Batching
// overlaps dependent misses across independent lookups rather than removing
// work; per `docs/BENCHMARKING.md` ("Which instrument fits the change") the
// instrument that decides it is wall clock. This arm reports what the overlap
// costs in retired instructions; it is not the verdict.
#[library_benchmark]
#[bench::random(args = ("random",), setup = built_map)]
fn map_get_batch(built: (ExpanseMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    // Stack-resident and hoisted out of the loop: the measured region is the
    // descent, not an allocation.
    let mut out = [None::<u64>; 256];
    let mut sink = 0u64;
    for chunk in probes.chunks(256) {
        map.get_batch(chunk, &mut out[..chunk.len()]);
        for v in &out[..chunk.len()] {
            sink ^= v.unwrap_or(0);
        }
    }
    // Leaked for the same reason as `map_get`.
    core::mem::forget(map);
    black_box(sink)
}

// Set-flavor twin of `map_get_batch`, against `set_contains`.
#[library_benchmark]
#[bench::random(args = ("random",), setup = built_set)]
fn set_contains_batch(built: (ExpanseSet, Vec<u64>)) -> u64 {
    let (set, probes) = built;
    let mut out = [false; 256];
    let mut hits = 0u64;
    for chunk in probes.chunks(256) {
        hits += set.contains_batch(chunk, &mut out[..chunk.len()]) as u64;
    }
    core::mem::forget(set);
    black_box(hits)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_set)]
fn set_contains(built: (ExpanseSet, Vec<u64>)) -> u64 {
    let (set, probes) = built;
    let mut hits = 0u64;
    for &k in &probes {
        hits += u64::from(set.contains(black_box(k)));
    }
    // Leaked — see `map_get`. Teardown was ~8% of this arm.
    core::mem::forget(set);
    black_box(hits)
}

// ---- Steady-state churn ------------------------------------------------

// Measured region: ONLY the mixed op loop — upsert an existing key,
// insert a fresh neighbour, remove it again. Build is in `setup`; the
// structure is leaked (rule 0). This is the arm the matrix lacked twice
// over: capacity-classed growth was tuned with no benchmark crossing
// class boundaries in steady state, and the locate_slot fix could not
// show its upsert effect because every insert arm inserts each key once,
// fresh. Fresh neighbours (`k ^ 1`) collide with existing random keys
// with probability ~n²/2⁶⁴ — negligible, and deterministic either way.
#[library_benchmark]
#[bench::random(args = ("random",), setup = built_map)]
fn map_churn(built: (ExpanseMap, Vec<u64>)) -> u64 {
    let (mut map, probes) = built;
    let mut sink = 0u64;
    for &k in &probes {
        // Upsert of a present key: the steady-state store pattern.
        sink ^= map.insert(black_box(k), black_box(!k)).unwrap_or(0);
        // Insert + remove of a fresh neighbour: crosses whatever
        // capacity-class boundary the local terminal sits at, both ways.
        map.insert(black_box(k ^ 1), k);
        sink ^= u64::from(map.remove(black_box(k ^ 1)).is_some());
    }
    core::mem::forget(map);
    black_box(sink)
}

// ---- Remove and ordered navigation -----------------------------------

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_map)]
fn map_remove(built: (ExpanseMap, Vec<u64>)) -> u64 {
    let (mut map, probes) = built;
    let mut removed = 0u64;
    for &k in &probes {
        removed += u64::from(map.remove(black_box(k)).is_some());
    }
    black_box(removed)
}

// Set-flavor twin of `map_remove`: every key removed in shuffled order, so
// the set drains to empty; the drop of the emptied set is inside the arm
// as it is there.
#[library_benchmark]
#[bench::random(args = ("random",), setup = built_set)]
fn set_remove(built: (ExpanseSet, Vec<u64>)) -> u64 {
    let (mut set, probes) = built;
    let mut removed = 0u64;
    for &k in &probes {
        removed += u64::from(set.remove(black_box(k)));
    }
    black_box(removed)
}

// Drain-and-refill and empty-tree oscillation (#1119): the workloads where
// a tree returns its allocator's blocks when it empties and has to carve
// them again on the next insert. `*_oscillate` inserts and removes one key
// at a time from empty, so the tree empties on every cycle; `*_refill`
// drains the built tree by `remove` and inserts every key again;
// `*_clear_refill` does the same through `clear`. Leaked, so the drop is
// outside the arm and only the cycle is measured.
#[library_benchmark]
#[bench::random(args = ("random",), setup = keys)]
fn map_oscillate(ks: Vec<u64>) -> u64 {
    let mut map = ExpanseMap::new();
    let mut sink = 0u64;
    for &k in &ks {
        map.insert(black_box(k), black_box(k));
        sink ^= map.remove(black_box(k)).unwrap_or(0);
    }
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = keys)]
fn set_oscillate(ks: Vec<u64>) -> u64 {
    let mut set = ExpanseSet::new();
    let mut sink = 0u64;
    for &k in &ks {
        set.insert(black_box(k));
        sink += u64::from(set.remove(black_box(k)));
    }
    core::mem::forget(set);
    black_box(sink)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_map)]
fn map_refill(built: (ExpanseMap, Vec<u64>)) -> u64 {
    let (mut map, probes) = built;
    let mut sink = 0u64;
    for &k in &probes {
        sink ^= map.remove(black_box(k)).unwrap_or(0);
    }
    for &k in &probes {
        map.insert(black_box(k), black_box(k));
    }
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_set)]
fn set_refill(built: (ExpanseSet, Vec<u64>)) -> u64 {
    let (mut set, probes) = built;
    let mut sink = 0u64;
    for &k in &probes {
        sink += u64::from(set.remove(black_box(k)));
    }
    for &k in &probes {
        set.insert(black_box(k));
    }
    core::mem::forget(set);
    black_box(sink)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_map)]
fn map_clear_refill(built: (ExpanseMap, Vec<u64>)) -> u64 {
    let (mut map, probes) = built;
    map.clear();
    for &k in &probes {
        map.insert(black_box(k), black_box(k));
    }
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_set)]
fn set_clear_refill(built: (ExpanseSet, Vec<u64>)) -> u64 {
    let (mut set, probes) = built;
    set.clear();
    for &k in &probes {
        set.insert(black_box(k));
    }
    let n = set.len();
    core::mem::forget(set);
    black_box(n)
}

// ---- Remove retention (docs/benchmarks/remove_retention/METHODOLOGY.md) ----
//
// The arms the remove-retention pre-registration names (§7.2–§7.4). They were
// added before any condensing code exists, so `main` carries a base-side count
// for each. Every tree is built in `setup` and leaked, so neither the build nor
// the drop is measured.

/// `set_remove_partial` / `map_remove_partial` population: 200,000 uniform
/// random keys at a 60-bit width, λ = 200,000 / 4,096 = 48.8 keys per 2-byte
/// expanse (the headline λ_N of METHODOLOGY §3).
const PARTIAL_N: usize = 200_000;
/// Keys the partial-remove arms keep: 62,500, λ = 15.3 (the headline λ_M).
const PARTIAL_M: usize = 62_500;
/// Key width of the partial-remove population.
const PARTIAL_BITS: u32 = 60;

/// The partial-remove population in generator order (distinct keys, drawn as
/// `remove_retention.rs` draws `random@60`) and the `PARTIAL_N - PARTIAL_M`
/// keys to remove, in a Fisher–Yates order.
fn partial_keys() -> (Vec<u64>, Vec<u64>) {
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mask = (1u64 << PARTIAL_BITS) - 1;
    let mut seen = std::collections::HashSet::with_capacity(PARTIAL_N);
    let mut all = Vec::with_capacity(PARTIAL_N);
    while all.len() < PARTIAL_N {
        let k = rng.next() & mask;
        if seen.insert(k) {
            all.push(k);
        }
    }
    let mut perm = all.clone();
    let mut prng = XorShift(0x5EED_0DE1_E7E5_0002);
    for i in (1..perm.len()).rev() {
        perm.swap(i, (prng.next() % (i as u64 + 1)) as usize);
    }
    perm.truncate(PARTIAL_N - PARTIAL_M);
    (all, perm)
}

fn built_partial_set(_: &str) -> (ExpanseSet, Vec<u64>) {
    let (all, removals) = partial_keys();
    let mut set = ExpanseSet::new();
    for &k in &all {
        set.insert(k);
    }
    (set, removals)
}

fn built_partial_map(_: &str) -> (ExpanseMap, Vec<u64>) {
    let (all, removals) = partial_keys();
    let mut map = ExpanseMap::new();
    for &k in &all {
        map.insert(k, !k);
    }
    (map, removals)
}

// Remove 137,500 of 200,000 keys in shuffled order, down to 62,500: drains
// cascaded expanses below `LEAF_CAP` (METHODOLOGY §7.2). Counted per remove.
#[library_benchmark]
#[bench::random60(args = ("random60",), setup = built_partial_set)]
fn set_remove_partial(built: (ExpanseSet, Vec<u64>)) -> u64 {
    let (mut set, removals) = built;
    let mut removed = 0u64;
    for &k in &removals {
        removed += u64::from(set.remove(black_box(k)));
    }
    core::mem::forget(set);
    black_box(removed)
}

#[library_benchmark]
#[bench::random60(args = ("random60",), setup = built_partial_map)]
fn map_remove_partial(built: (ExpanseMap, Vec<u64>)) -> u64 {
    let (mut map, removals) = built;
    let mut removed = 0u64;
    for &k in &removals {
        removed += u64::from(map.remove(black_box(k)).is_some());
    }
    core::mem::forget(map);
    black_box(removed)
}

/// The `*_remove_partial` set after its removes: 62,500 keys left in a tree
/// grown to 200,000 (`docs/benchmarks/remove_retention/README.md`, "Allocator
/// census and rebuild").
fn drained_partial_set(arg: &str) -> ExpanseSet {
    let (mut set, removals) = built_partial_set(arg);
    for &k in &removals {
        assert!(set.remove(k));
    }
    set
}

fn drained_partial_map(arg: &str) -> ExpanseMap {
    let (mut map, removals) = built_partial_map(arg);
    for &k in &removals {
        assert!(map.remove(k).is_some());
    }
    map
}

// The rebuild arm of the remove-retention census: `clone()` the drained tree
// (the set through `from_sorted_iter`, the map by an ascending insert of every
// entry) and drop the drained one, so the arm counts the copy and the free of
// the old tree. The copy is leaked. Counted per surviving key (62,500).
#[library_benchmark]
#[bench::random60(args = ("random60",), setup = drained_partial_set)]
fn set_rebuild_drained(drained: ExpanseSet) -> u64 {
    let rebuilt = black_box(&drained).clone();
    drop(drained);
    let n = rebuilt.len();
    core::mem::forget(rebuilt);
    black_box(n)
}

#[library_benchmark]
#[bench::random60(args = ("random60",), setup = drained_partial_map)]
fn map_rebuild_drained(drained: ExpanseMap) -> u64 {
    let rebuilt = black_box(&drained).clone();
    drop(drained);
    let n = rebuilt.len();
    core::mem::forget(rebuilt);
    black_box(n)
}

// The compact arm of the remove-retention suite (METHODOLOGY §12.7): the same
// drained tree as `*_rebuild_drained`, compacted in place, so the arm counts
// the ordered walk, the bottom-up build of the new tree and the drop of the
// old one. The compacted tree is leaked. Counted per surviving key (62,500);
// G-cost compares it with the rebuild arm of the same run.
#[library_benchmark]
#[bench::random60(args = ("random60",), setup = drained_partial_set)]
fn set_compact_drained(drained: ExpanseSet) -> u64 {
    let mut set = drained;
    black_box(&mut set).compact();
    let n = set.len();
    core::mem::forget(set);
    black_box(n)
}

#[library_benchmark]
#[bench::random60(args = ("random60",), setup = drained_partial_map)]
fn map_compact_drained(drained: ExpanseMap) -> u64 {
    let mut map = drained;
    black_box(&mut map).compact();
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

/// Level-6 expanses the subtree arms drive (METHODOLOGY §7.3, §7.4: E).
const SUBTREE_E: usize = 1_024;
/// Spacing of the driven expanses among the 65,536 2-byte prefixes.
const SUBTREE_STRIDE: usize = 65_536 / SUBTREE_E;
/// The `H1` arm's threshold, `LEAF_CAP - 1` (METHODOLOGY §5.2).
const SUBTREE_T_H1: usize = expanse_trie::types::LEAF_CAP - 1;
/// The `wide` arm's threshold, `LEAF_CAP - 8` (METHODOLOGY §5.2).
const SUBTREE_T_WIDE: usize = expanse_trie::types::LEAF_CAP - 8;
/// Full insert/remove cycles per expanse in the oscillation arm.
const OSC_CYCLES: usize = 8;

/// Key `j` of driven expanse `e`: 2-byte prefix `e * SUBTREE_STRIDE`, a third
/// byte distinct for every `j < 256` (so a cascaded expanse is a branch of
/// single-key children), and 40 low bits from `rng`.
fn subtree_key(e: usize, j: usize, rng: &mut XorShift) -> u64 {
    let prefix = (e * SUBTREE_STRIDE) as u64;
    let third = ((j * 37 + e) & 0xFF) as u64;
    (prefix << 48) | (third << 40) | (rng.next() & ((1u64 << 40) - 1))
}

/// A set whose top two levels are `BranchU` (every one of the 65,536 2-byte
/// prefixes populated) with `SUBTREE_E` driven expanses. Each driven expanse
/// draws `drawn` keys, has the first `insert` of them inserted fresh, and is then
/// drained, highest `j` first, to `keep` keys. Every other prefix holds one
/// key. Returns the set and, per driven expanse, its `drawn` keys in `j` order.
fn subtree_set(drawn: usize, insert: usize, keep: usize) -> (ExpanseSet, Vec<Vec<u64>>) {
    assert!(keep <= insert && insert <= drawn && drawn <= 256);
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut set = ExpanseSet::new();
    for p in 0..65_536usize {
        if p % SUBTREE_STRIDE != 0 {
            set.insert(((p as u64) << 48) | (rng.next() & ((1u64 << 48) - 1)));
        }
    }
    let mut expanses = Vec::with_capacity(SUBTREE_E);
    for e in 0..SUBTREE_E {
        let ks: Vec<u64> = (0..drawn).map(|j| subtree_key(e, j, &mut rng)).collect();
        for &k in &ks[..insert] {
            assert!(set.insert(k));
        }
        for &k in ks[keep..insert].iter().rev() {
            assert!(set.remove(k));
        }
        expanses.push(ks);
    }
    (set, expanses)
}

/// Oscillation input: the band's lower edge, `LEAF_CAP + 1 - band` keys.
fn osc_setup(band: usize) -> (ExpanseSet, Vec<Vec<u64>>, usize) {
    let cap1 = expanse_trie::types::LEAF_CAP + 1;
    let (set, ex) = subtree_set(cap1, cap1, cap1);
    (set, ex, band)
}

// METHODOLOGY §7.3: every driven expanse cascaded at `LEAF_CAP + 1` keys, then
// `OSC_CYCLES` cycles per expanse of removing its highest `band` keys and
// inserting them again. `band2` crosses `LEAF_CAP + 1 ↔ LEAF_CAP - 1` (H = 1),
// `band9` crosses `LEAF_CAP + 1 ↔ LEAF_CAP - 8` (H = 8). Counted per operation:
// `OSC_CYCLES × SUBTREE_E × 2 × band`.
#[library_benchmark]
#[bench::band2(args = (2,), setup = osc_setup)]
#[bench::band9(args = (9,), setup = osc_setup)]
fn set_subtree_boundary_oscillate(built: (ExpanseSet, Vec<Vec<u64>>, usize)) -> u64 {
    let (mut set, expanses, band) = built;
    let mut sink = 0u64;
    for _ in 0..OSC_CYCLES {
        for ks in &expanses {
            let lo = ks.len() - band;
            for &k in ks[lo..].iter().rev() {
                sink += u64::from(set.remove(black_box(k)));
            }
            for &k in &ks[lo..] {
                sink += u64::from(set.insert(black_box(k)));
            }
        }
    }
    core::mem::forget(set);
    black_box(sink)
}

/// Split pair input: every driven expanse a fresh leaf of `pop` keys (built by
/// insertion only), and the key each measured insert adds.
fn split_leaf_setup(pop: usize) -> (ExpanseSet, Vec<u64>) {
    let (set, ex) = subtree_set(pop + 1, pop, pop);
    let adds = ex.iter().map(|ks| ks[pop]).collect();
    (set, adds)
}

fn insert_each(built: (ExpanseSet, Vec<u64>)) -> u64 {
    let (mut set, adds) = built;
    let mut sink = 0u64;
    for &k in &adds {
        sink += u64::from(set.insert(black_box(k)));
    }
    core::mem::forget(set);
    black_box(sink)
}

// METHODOLOGY §7.4, C_split: one insert into each of `SUBTREE_E` full
// `LEAF_CAP`-key leaves, so every insert cascades its expanse.
#[library_benchmark]
#[bench::e1024(args = (expanse_trie::types::LEAF_CAP,), setup = split_leaf_setup)]
fn set_subtree_split(built: (ExpanseSet, Vec<u64>)) -> u64 {
    insert_each(built)
}

// The control of `set_subtree_split`: one insert into each of `SUBTREE_E`
// `LEAF_CAP - 1`-key leaves, the same slot class, no cascade.
#[library_benchmark]
#[bench::e1024(args = (expanse_trie::types::LEAF_CAP - 1,), setup = split_leaf_setup)]
fn set_subtree_split_control(built: (ExpanseSet, Vec<u64>)) -> u64 {
    insert_each(built)
}

/// Condense pair input: each driven expanse cascaded at `LEAF_CAP + 1` keys
/// and drained to `pop` keys (a `BranchB` of single-key children), and the
/// key each measured remove takes out.
fn condense_setup(pop: usize) -> (ExpanseSet, Vec<u64>) {
    let cap1 = expanse_trie::types::LEAF_CAP + 1;
    let (set, ex) = subtree_set(cap1, cap1, pop);
    let takes = ex.iter().map(|ks| ks[pop - 1]).collect();
    (set, takes)
}

fn remove_each(built: (ExpanseSet, Vec<u64>)) -> u64 {
    let (mut set, takes) = built;
    let mut sink = 0u64;
    for &k in &takes {
        sink += u64::from(set.remove(black_box(k)));
    }
    core::mem::forget(set);
    black_box(sink)
}

// METHODOLOGY §7.4, C_condense: one remove from each of `SUBTREE_E` drained
// expanses at `T + 1` keys, so under the arm with threshold T every remove
// lands on T and condenses. `h1` is T = `LEAF_CAP - 1`, `wide` is
// T = `LEAF_CAP - 8`; the input matching a build's arm is the one its
// C_condense reads.
#[library_benchmark]
#[bench::h1(args = (SUBTREE_T_H1 + 1,), setup = condense_setup)]
#[bench::wide(args = (SUBTREE_T_WIDE + 1,), setup = condense_setup)]
fn set_subtree_condense(built: (ExpanseSet, Vec<u64>)) -> u64 {
    remove_each(built)
}

// The control of `set_subtree_condense`: the same remove from expanses at
// `T + 2` keys, which lands on `T + 1`, not an evaluation point of either arm.
#[library_benchmark]
#[bench::h1(args = (SUBTREE_T_H1 + 2,), setup = condense_setup)]
#[bench::wide(args = (SUBTREE_T_WIDE + 2,), setup = condense_setup)]
fn set_subtree_condense_control(built: (ExpanseSet, Vec<u64>)) -> u64 {
    remove_each(built)
}

// Snapshot by deep copy (#1103): `Clone` rebuilds by ordered iteration, so
// an arm measures iteration plus an ascending insert per key. The original
// is built in `setup`, and both it and the copy are leaked (see `map_get`).
#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = built_map)]
#[bench::random(args = ("random",), setup = built_map)]
fn map_clone(built: (ExpanseMap, Vec<u64>)) -> u64 {
    let (map, _) = built;
    let copy = black_box(&map).clone();
    let n = copy.len();
    core::mem::forget(copy);
    core::mem::forget(map);
    black_box(n)
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = built_set)]
#[bench::random(args = ("random",), setup = built_set)]
fn set_clone(built: (ExpanseSet, Vec<u64>)) -> u64 {
    let (set, _) = built;
    let copy = black_box(&set).clone();
    let n = copy.len();
    core::mem::forget(copy);
    core::mem::forget(set);
    black_box(n)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_map)]
fn map_iterate(built: (ExpanseMap, Vec<u64>)) -> u64 {
    let (map, _) = built;
    let mut sink = 0u64;
    for (k, v) in map.iter() {
        sink ^= k ^ v;
    }
    // Leaked — see `map_get`.
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_map)]
#[bench::sequential(args = ("sequential",), setup = built_map)]
#[bench::clustered(args = ("clustered",), setup = built_map)]
fn map_nav(built: (ExpanseMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for &k in &probes {
        if let Some((next_k, next_v)) = map.next_at_or_after(black_box(k)) {
            sink ^= next_k ^ next_v;
        }
    }
    // Leaked — see `map_get`.
    core::mem::forget(map);
    black_box(sink)
}

// The strict predecessor of a present key. Whenever the probe is the smallest
// key in its terminal, the walk leaves that subtree and descends a sibling to
// its maximum: the backtracking path that `next_at_or_after` on a present key
// never takes, and the one an optimistic ordered read must validate (#900).
// The three key shapes place terminal boundaries differently.
#[library_benchmark]
#[bench::random(args = ("random",), setup = built_map)]
#[bench::sequential(args = ("sequential",), setup = built_map)]
#[bench::clustered(args = ("clustered",), setup = built_map)]
fn map_prev(built: (ExpanseMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for &k in &probes {
        if let Some((prev_k, prev_v)) = map.prev_before(black_box(k)) {
            sink ^= prev_k ^ prev_v;
        }
    }
    // Leaked — see `map_get`.
    core::mem::forget(map);
    black_box(sink)
}

// A full forward scan through the stateful cursor (#1142): one `next` per
// entry, the path kept between steps. The single-threaded reference for a
// concurrent batch cursor; no other arm covers `MapCursor`.
#[library_benchmark]
#[bench::random(args = ("random",), setup = built_map)]
#[bench::sequential(args = ("sequential",), setup = built_map)]
#[bench::clustered(args = ("clustered",), setup = built_map)]
fn map_cursor_scan(built: (ExpanseMap, Vec<u64>)) -> u64 {
    let (map, _probes) = built;
    let mut sink = 0u64;
    let mut n = 0usize;
    let mut cur = map.cursor();
    while let Some((k, v)) = cur.next() {
        sink ^= k ^ v;
        n += 1;
    }
    assert_eq!(n, POP, "the scan visits every entry once");
    // Leaked — see `map_get`.
    core::mem::forget(map);
    black_box(sink)
}

// Rank from each present key (#1144): `nav::count_below` sums sibling
// `pop0` down the descent. The §6 prerequisite for any change to the
// concurrent count path; `one_top_byte` is that issue's worst case for the
// concurrent fold and is measured here on the plain engine as its control.
#[library_benchmark]
#[bench::random(args = ("random",), setup = built_map)]
#[bench::sequential(args = ("sequential",), setup = built_map)]
#[bench::one_top_byte(args = ("one_top_byte",), setup = built_map)]
fn map_count_below(built: (ExpanseMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for &k in &probes {
        sink = sink.wrapping_add(map.count_below(black_box(k)));
    }
    // Leaked — see `map_get`.
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_set)]
#[bench::sequential(args = ("sequential",), setup = built_set)]
#[bench::one_top_byte(args = ("one_top_byte",), setup = built_set)]
fn set_count_below(built: (ExpanseSet, Vec<u64>)) -> u64 {
    let (set, probes) = built;
    let mut sink = 0u64;
    for &k in &probes {
        sink = sink.wrapping_add(set.count_below(black_box(k)));
    }
    core::mem::forget(set);
    black_box(sink)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_map)]
#[bench::sequential(args = ("sequential",), setup = built_map)]
#[bench::clustered(args = ("clustered",), setup = built_map)]
fn map_range(built: (ExpanseMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    // 100 key-WIDTH windows of 100 key units — NOT 100 elements each. The
    // yield depends entirely on the distribution, and was measured (structural
    // counters in `RawIter`, POP = 50 000) as: `random` 1.00 element/window
    // (the probe key itself; density is ~2.7e-15 per key unit, so a 100-wide
    // window holds nothing else — this cell is a seek + one terminating
    // advance, not a scan), `sequential` 101.00, `clustered` 85.90.
    // Only the sequential and clustered cells exercise real scan streaming.
    for &start in probes.iter().take(100) {
        let end = start.saturating_add(100);
        for (k, v) in map.range(black_box(start..=end)) {
            sink ^= k ^ v;
        }
    }
    // Leaked — see `map_get`.
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_set)]
#[bench::sequential(args = ("sequential",), setup = built_set)]
#[bench::clustered(args = ("clustered",), setup = built_set)]
fn set_range(built: (ExpanseSet, Vec<u64>)) -> u64 {
    let (set, probes) = built;
    let mut sink = 0u64;
    // 100 key-WIDTH windows of 100 key units — see the yield note on
    // `map_range`: the `random` cell yields one element per window and so
    // measures seek, not scan streaming.
    for &start in probes.iter().take(100) {
        let end = start.saturating_add(100);
        for k in set.range(black_box(start..=end)) {
            sink ^= k;
        }
    }
    // Leaked — see `map_get`.
    core::mem::forget(set);
    black_box(sink)
}

#[library_benchmark]
#[bench::sensor_timestamps()]
fn set32_insert() -> ExpanseSet32 {
    let mut set = ExpanseSet32::new();
    for i in 0..10_000 {
        set.insert(black_box(1_700_000_000 + i as Key32));
    }
    black_box(set)
}

#[library_benchmark]
#[bench::sensor_timestamps()]
fn map32_insert() -> ExpanseMap32 {
    let mut map = ExpanseMap32::new();
    for i in 0..10_000 {
        map.insert(black_box(1_700_000_000 + i as Key32), i as Value32);
    }
    black_box(map)
}

// Build in `setup` — the same isolation rule as the 64-bit cells (#375).
// Until #375 the builds ran inside the measured region of `map32_get` and
// `blobmap32_scan`, so those cells counted insert work under a get/scan
// label. The first run after this change therefore shows a large one-time
// instruction-count DROP on both cells: build work leaving the measured
// region, not a real get/scan optimization.

/// Prebuilt 32-bit map — the probe stream is regenerated in the body (it
/// is 500 multiply-and-mask steps, negligible next to the lookups).
fn built_map32(_dist: &str) -> ExpanseMap32 {
    let mut map = ExpanseMap32::new();
    for i in 0..500 {
        map.insert((i * 100_007) & 0x1FFF_FFFF, i);
    }
    map
}

/// Prebuilt 32-bit blob map with IPv4-route-shaped keys.
fn built_blobmap32(_dist: &str) -> ExpanseBlobMap32 {
    let mut blobmap = ExpanseBlobMap32::new();
    for i in 0..2_000 {
        let ip = (10 << 24) | ((i as Key32 / 256) << 16) | ((i as Key32 % 256) << 8);
        blobmap
            .insert(ip, &[0xAA, 0xBB, 0xCC], (i % 16) as u16)
            .unwrap();
    }
    blobmap
}

#[library_benchmark]
#[bench::can_dispatch(args = ("can_dispatch",), setup = built_map32)]
fn map32_get(map: ExpanseMap32) -> u64 {
    let mut sum = 0u64;
    for i in 0..500 {
        if let Some(v) = map.get(black_box((i * 100_007) & 0x1FFF_FFFF)) {
            sum += v as u64;
        }
    }
    // Leaked — see `map_get`: dropping the map here would measure teardown
    // under the lookup label.
    core::mem::forget(map);
    black_box(sum)
}

/// Prebuilt 2,000-entry 32-bit map in three key shapes. Ordered iteration
/// is the cell these exist for, and the shapes decide how deep the trie is:
/// sequential packs into few leaves, clustered spreads runs across
/// expanses, and uniform random builds the deepest paths.
fn keys32(dist: &str) -> Vec<Key32> {
    let mut x: u64 = 0x2545_F491_4F6C_DD1D;
    (0..2_000u32)
        .map(|i| match dist {
            "random" => {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x >> 16) as Key32
            }
            "clustered" => (i / 8) * 4096 + (i % 8),
            _ => 1_000 + i,
        })
        .collect()
}

fn built_map32_dist(dist: &str) -> ExpanseMap32 {
    let mut map = ExpanseMap32::new();
    for (i, k) in keys32(dist).into_iter().enumerate() {
        map.insert(k, (i * 3) as Value32);
    }
    map
}

fn built_map32_remove(dist: &str) -> (ExpanseMap32, Vec<Key32>) {
    let ks = keys32(dist);
    let mut map = ExpanseMap32::new();
    for (i, &k) in ks.iter().enumerate() {
        map.insert(k, (i * 3) as Value32);
    }
    let mut probes = ks;
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    (map, probes)
}

fn built_set32_dist(dist: &str) -> ExpanseSet32 {
    let mut set = ExpanseSet32::new();
    for k in keys32(dist) {
        set.insert(k);
    }
    set
}

fn built_set32_remove(dist: &str) -> (ExpanseSet32, Vec<Key32>) {
    let ks = keys32(dist);
    let mut set = ExpanseSet32::new();
    for &k in &ks {
        set.insert(k);
    }
    let mut probes = ks;
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    (set, probes)
}

// Full ordered iteration over a 32-bit map: the stack walk's own cell
// (#614). Before it, each key cost a fresh root descent through `first_ge`.
#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = built_map32_dist)]
#[bench::clustered(args = ("clustered",), setup = built_map32_dist)]
#[bench::random(args = ("random",), setup = built_map32_dist)]
fn map32_iterate(map: ExpanseMap32) -> u64 {
    let mut sink = 0u64;
    for (k, v) in map.iter() {
        sink = sink.wrapping_add(u64::from(k) ^ u64::from(v));
    }
    // Leaked — see `map_get`.
    core::mem::forget(map);
    black_box(sink)
}

// Ordered navigation on the 32-bit map from each present key, shuffled:
// `next_at_or_after` lands on the probe itself, and `prev_before` crosses to a
// sibling subtree whenever the probe is its terminal's smallest key. The
// 32-bit twins of `map_nav` and `map_prev`, which the concurrent 32-bit
// ordered reads (#900) are measured against.
#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = built_map32_remove)]
#[bench::clustered(args = ("clustered",), setup = built_map32_remove)]
#[bench::random(args = ("random",), setup = built_map32_remove)]
fn map32_nav(built: (ExpanseMap32, Vec<Key32>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for &k in &probes {
        if let Some((nk, nv)) = map.next_at_or_after(black_box(k)) {
            sink ^= u64::from(nk) ^ u64::from(nv);
        }
    }
    // Leaked — see `map_get`.
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = built_map32_remove)]
#[bench::clustered(args = ("clustered",), setup = built_map32_remove)]
#[bench::random(args = ("random",), setup = built_map32_remove)]
fn map32_prev(built: (ExpanseMap32, Vec<Key32>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for &k in &probes {
        if let Some((pk, pv)) = map.prev_before(black_box(k)) {
            sink ^= u64::from(pk) ^ u64::from(pv);
        }
    }
    // Leaked — see `map_get`.
    core::mem::forget(map);
    black_box(sink)
}

// A bounded range walk over the same maps, through the `RawIter32` cursor.
//
// This is NOT the walk the C ABI takes: `expanse_map_for_each_range` reaches
// `trie32::map_for_each_range`, a separate recursive descent that shares this
// shape and none of its code. The comment here used to claim it served that
// entry point, and the claim held long enough for the cursor to lose its
// per-key popcount (#686) while the C ABI path kept one — a −15.29% host win
// next to 0.00% on device. `map32_for_each_range` below covers the other side.
#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = built_map32_dist)]
#[bench::clustered(args = ("clustered",), setup = built_map32_dist)]
#[bench::random(args = ("random",), setup = built_map32_dist)]
fn map32_range(map: ExpanseMap32) -> u64 {
    let mut sink = 0u64;
    for (k, v) in map.range(black_box(0)..=black_box(Key32::MAX / 2)) {
        sink = sink.wrapping_add(u64::from(k) ^ u64::from(v));
    }
    // Leaked — see `map_get`.
    core::mem::forget(map);
    black_box(sink)
}

// The same bounded range, walked the way the 32-bit C ABI walks it (#614).
//
// `expanse_map_for_each_range` -> `ExpanseMap32::try_for_each_range` ->
// `trie32::map_for_each_range`: a recursive descent that visits a bitmap
// leaf's digits directly, where the arm above streams a `LeafCur32` cursor.
// The embedded suite's range aggregation is this path, so an optimisation
// measured only on `map32_range` says nothing about what the device runs.
//
// Same map, same bounds, same sink as `map32_range` so the two are readable
// side by side — the difference between them is the walk, not the workload.
#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = built_map32_dist)]
#[bench::clustered(args = ("clustered",), setup = built_map32_dist)]
#[bench::random(args = ("random",), setup = built_map32_dist)]
fn map32_for_each_range(map: ExpanseMap32) -> u64 {
    let mut sink = 0u64;
    map.try_for_each_range(black_box(0), black_box(Key32::MAX / 2), |k, v| {
        sink = sink.wrapping_add(u64::from(k) ^ u64::from(v));
        true
    });
    // Leaked — see `map_get`.
    core::mem::forget(map);
    black_box(sink)
}

// Ordered iteration and a bounded range over a 32-bit SET.
//
// The set walk had no arm at all: the suite measured `set32_insert` and
// `set32_remove` and nothing that walked one in order. `LeafCur32`'s
// `Kind::SetImmed` branch is reachable only from here, so any change to it was
// unmeasurable — which is the same shape of gap that let the C ABI range walk
// keep a per-key popcount (#690).
#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = built_set32_dist)]
#[bench::clustered(args = ("clustered",), setup = built_set32_dist)]
#[bench::random(args = ("random",), setup = built_set32_dist)]
fn set32_iterate(set: ExpanseSet32) -> u64 {
    let mut sink = 0u64;
    for k in set.iter() {
        sink = sink.wrapping_add(u64::from(k));
    }
    // Leaked — see `map_get`.
    core::mem::forget(set);
    black_box(sink)
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = built_set32_dist)]
#[bench::clustered(args = ("clustered",), setup = built_set32_dist)]
#[bench::random(args = ("random",), setup = built_set32_dist)]
fn set32_range(set: ExpanseSet32) -> u64 {
    let mut sink = 0u64;
    for k in set.range(black_box(0)..=black_box(Key32::MAX / 2)) {
        sink = sink.wrapping_add(u64::from(k));
    }
    // Leaked — see `map_get`.
    core::mem::forget(set);
    black_box(sink)
}

// Scattered and sequential removals on ExpanseMap32 and ExpanseSet32:
// guards 32-bit removal descent and demotion unrolls (#617).
#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = built_map32_remove)]
#[bench::clustered(args = ("clustered",), setup = built_map32_remove)]
#[bench::random(args = ("random",), setup = built_map32_remove)]
fn map32_remove(built: (ExpanseMap32, Vec<Key32>)) -> u64 {
    let (mut map, probes) = built;
    let mut removed = 0u64;
    for &k in &probes {
        removed += u64::from(map.remove(black_box(k)).is_some());
    }
    black_box(removed)
}

#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = built_set32_remove)]
#[bench::clustered(args = ("clustered",), setup = built_set32_remove)]
#[bench::random(args = ("random",), setup = built_set32_remove)]
fn set32_remove(built: (ExpanseSet32, Vec<Key32>)) -> u64 {
    let (mut set, probes) = built;
    let mut removed = 0u64;
    for &k in &probes {
        removed += u64::from(set.remove(black_box(k)));
    }
    black_box(removed)
}

#[library_benchmark]
#[bench::ipv4_routes(args = ("ipv4_routes",), setup = built_blobmap32)]
fn blobmap32_scan(blobmap: ExpanseBlobMap32) -> usize {
    let mut count = 0;
    blobmap.scan_filtered(
        black_box(10 << 24),
        black_box((10 << 24) | 0x00FF_FFFF),
        |_k, meta| meta < 8,
        |_k, _view, _meta| count += 1,
    );
    // Leaked — see `map_get`.
    core::mem::forget(blobmap);
    black_box(count)
}

/// Deterministic hasher for the JudyHS cells — `RandomState`'s per-process
/// seed would make bucket placement (and instruction counts) depend on the
/// run, defeating callgrind's exact-reproducibility contract.
type DetHasher = std::hash::BuildHasherDefault<std::collections::hash_map::DefaultHasher>;

/// Route-shaped string keys (~40 bytes, long shared prefixes): the
/// JudySL working shape — prefix chains, suffix leaves, splits — and a
/// realistic JudyHS byte-key distribution.
fn str_keys(_dist: &str) -> Vec<Vec<u8>> {
    (0..POP)
        .map(|i| format!("/api/v2/tenants/{:06}/resources/{:04}", i / 16, i % 16).into_bytes())
        .collect()
}

/// Prebuilt string map plus shuffled probe order so only lookup is measured.
fn built_strmap(dist: &str) -> (ExpanseStrMap, Vec<Vec<u8>>) {
    let ks = str_keys(dist);
    let mut map = ExpanseStrMap::new();
    for (i, k) in ks.iter().enumerate() {
        map.insert(tk(k), i as u64);
    }
    let mut probes = ks;
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    (map, probes)
}

/// Prebuilt byte-string map plus shuffled probe order.
fn built_bytesmap(dist: &str) -> (ExpanseBytesMap<DetHasher>, Vec<Vec<u8>>) {
    let ks = str_keys(dist);
    let mut map = ExpanseBytesMap::with_hasher(DetHasher::default());
    for (i, k) in ks.iter().enumerate() {
        map.insert(k, i as u64);
    }
    let mut probes = ks;
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    (map, probes)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = str_keys)]
fn strmap_insert(ks: Vec<Vec<u8>>) -> u64 {
    let mut map = ExpanseStrMap::new();
    for (i, k) in ks.iter().enumerate() {
        map.insert(black_box(tk(k)), black_box(i as u64));
    }
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_strmap)]
fn strmap_get(built: (ExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for k in &probes {
        sink ^= map.get(black_box(tk(k))).unwrap_or(0);
    }
    core::mem::forget(map);
    black_box(sink)
}

/// Shared-prefix path keys for the prefix-scan arms:
/// `https://example.com/api/v2/objects/` plus 12 hex digits of a 48-bit id,
/// first-seen order, drawn from the `docs/benchmarks/patricia_comparison/`
/// suite's `STRING_SEED`.
///
/// - `paths`: uniform ids — that suite's string workload at this file's
///   population. Bytes 32..40 of a key hold the id's top 20 bits, so at
///   `POP = 50_000` the expected keys per 20-bit bucket is λ = 0.048, and 4.7%
///   of keys share a bucket and sit under a child node.
/// - `paths_dense`: the top 20 bits are restricted to every 20th bucket, which
///   puts λ at 0.954 — the suite's 1M cell, where 61.5% of keys sit under a
///   child node. The terminal-chunk path that dominates the 1M cells is nearly
///   absent from `paths`; this arm exercises it at the same population.
fn path_keys(dist: &str) -> Vec<Vec<u8>> {
    let dense = match dist {
        "paths" => false,
        "paths_dense" => true,
        other => panic!("unknown path-key distribution {other}"),
    };
    let mut rng = XorShift(0x5A71_C1A0_0000_0001);
    let mut seen = std::collections::HashSet::with_capacity(POP);
    let mut out = Vec::with_capacity(POP);
    while out.len() < POP {
        let r = rng.next();
        let id = if dense {
            // 52,428 buckets × stride 20 stays below 2^20 and spans every
            // leading hex digit, so all 64 scan prefixes stay populated.
            let bucket = (r >> 28) % 52_428 * 20;
            (bucket << 28) | (r & 0xFFF_FFFF)
        } else {
            r & 0xFFFF_FFFF_FFFF
        };
        if seen.insert(id) {
            out.push(format!("https://example.com/api/v2/objects/{id:012x}").into_bytes());
        }
    }
    out
}

/// The 64 prefixes the suite's prefix-scan cell walks: the shared prefix plus
/// two hex digits `{:02x}` of `4 * i`, each covering about 1/256 of the keys.
fn scan_prefixes() -> Vec<Vec<u8>> {
    (0..64u32)
        .map(|i| format!("https://example.com/api/v2/objects/{:02x}", (i * 4) as u8).into_bytes())
        .collect()
}

/// Entries the 64 prefixes yield over `path_keys` — the ops counts
/// `scripts/perf_report.py` divides by. `built_path_strmap` asserts them, so a
/// registered count cannot drift from its generator.
fn prefix_scan_entries(dist: &str) -> u64 {
    match dist {
        "paths" => 12_547,
        "paths_dense" => 12_544,
        other => panic!("unknown path-key distribution {other}"),
    }
}

/// Sums the values of every key under `prefix`: one seek, then cursor steps
/// until the first key outside the prefix. Returns (entries, value sum).
#[inline(always)]
fn strmap_prefix_sum(map: &mut ExpanseStrMap, prefix: &[u8]) -> (u64, u64) {
    let mut cur = map.cursor_at_or_after(tk(prefix));
    let (mut n, mut sum) = (0u64, 0u64);
    while let Some((k, slot)) = cur.next() {
        if !k.starts_with(prefix) {
            break;
        }
        // SAFETY: `slot` is the map's live value word, valid until the next
        // structural mutation; the cursor's `&mut` borrow of `map` rules that
        // out for as long as the slot is read.
        sum ^= unsafe { slot.as_ptr().read() };
        n += 1;
    }
    (n, sum)
}

/// Prebuilt shared-prefix map and the scan prefixes; setup also walks every
/// prefix once and checks the entry count, outside the measured region.
fn built_path_strmap(dist: &str) -> (ExpanseStrMap, Vec<Vec<u8>>) {
    let mut map = ExpanseStrMap::new();
    for (i, k) in path_keys(dist).iter().enumerate() {
        map.insert(tk(k), i as u64);
    }
    let prefixes = scan_prefixes();
    let entries: u64 = prefixes
        .iter()
        .map(|p| strmap_prefix_sum(&mut map, p).0)
        .sum();
    assert_eq!(
        entries,
        prefix_scan_entries(dist),
        "prefix-scan population drifted from its registered ops count"
    );
    for p in &prefixes {
        let want = strmap_prefix_sum(&mut map, p);
        let mut cur = map.cursor_prefix(tk(p));
        let (mut n, mut sum) = (0u64, 0u64);
        while let Some((_, slot)) = cur.next() {
            // SAFETY: as in `strmap_prefix_sum`.
            sum ^= unsafe { slot.as_ptr().read() };
            n += 1;
        }
        assert_eq!(
            (n, sum),
            want,
            "cursor_prefix disagrees with the bounded walk"
        );
    }
    (map, prefixes)
}

// The `patricia_comparison` prefix-scan cell (#1096) as a deterministic arm:
// 64 seeks with `cursor_at_or_after` and a bounded cursor walk each, over the
// suite's shared-prefix path keys. Ops = entries yielded, so the report reads
// instructions per scanned entry; the 64 seeks are part of that figure.
// `paths_dense` is the same walk in the 1M cell's key-sharing regime.
#[library_benchmark]
#[bench::paths(args = ("paths",), setup = built_path_strmap)]
#[bench::paths_dense(args = ("paths_dense",), setup = built_path_strmap)]
fn strmap_prefix_scan(built: (ExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (mut map, prefixes) = built;
    let mut sink = 0u64;
    for p in &prefixes {
        let (n, sum) = strmap_prefix_sum(&mut map, black_box(p));
        sink ^= n ^ sum;
    }
    core::mem::forget(map);
    black_box(sink)
}

// The same 64 prefix scans through `cursor_prefix`, which ends each walk at
// the prefix boundary itself: no per-entry `starts_with` in the caller and no
// key built past the prefix. Ops = entries yielded, as for
// `strmap_prefix_scan`, so the two arms read on one scale.
#[library_benchmark]
#[bench::paths(args = ("paths",), setup = built_path_strmap)]
#[bench::paths_dense(args = ("paths_dense",), setup = built_path_strmap)]
fn strmap_prefix_bounded(built: (ExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (map, prefixes) = built;
    let mut sink = 0u64;
    for p in &prefixes {
        let mut cur = map.cursor_prefix(tk(black_box(p)));
        let (mut n, mut sum) = (0u64, 0u64);
        while let Some((_, slot)) = cur.next() {
            // SAFETY: as in `strmap_prefix_sum`.
            sum ^= unsafe { slot.as_ptr().read() };
            n += 1;
        }
        sink ^= n ^ sum;
    }
    core::mem::forget(map);
    black_box(sink)
}

// The fixed per-prefix cost of `strmap_prefix_scan` on its own: 64 seeks,
// each followed by the one step that yields the first entry. Ops = seeks, so
// the report reads instructions per prefix — the intercept of the prefix-scan
// cells, which the per-entry figure of `strmap_prefix_scan` folds in.
#[library_benchmark]
#[bench::paths(args = ("paths",), setup = built_path_strmap)]
#[bench::paths_dense(args = ("paths_dense",), setup = built_path_strmap)]
fn strmap_prefix_seek(built: (ExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (map, prefixes) = built;
    let mut sink = 0u64;
    for p in &prefixes {
        let mut cur = map.cursor_at_or_after(tk(black_box(p)));
        if let Some((k, slot)) = cur.next() {
            // SAFETY: `slot` is the map's live value word, valid until the
            // next structural mutation, which the cursor's borrow rules out.
            sink ^= k.len() as u64 ^ unsafe { slot.as_ptr().read() };
        }
    }
    core::mem::forget(map);
    black_box(sink)
}

// An unbounded cursor walk over every key of the path-key maps: the per-entry
// cost of `StrCursor::next` with no seek and no prefix compare. Ops = POP.
#[library_benchmark]
#[bench::paths(args = ("paths",), setup = built_path_strmap)]
#[bench::paths_dense(args = ("paths_dense",), setup = built_path_strmap)]
fn strmap_cursor_scan(built: (ExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (map, _) = built;
    let (mut n, mut sink) = (0u64, 0u64);
    let mut cur = map.cursor();
    while let Some((k, slot)) = cur.next() {
        // SAFETY: as in `strmap_prefix_seek`.
        sink ^= k.len() as u64 ^ unsafe { slot.as_ptr().read() };
        n += 1;
    }
    drop(cur);
    assert_eq!(n, POP as u64, "cursor walk lost keys");
    core::mem::forget(map);
    black_box(sink)
}

// Same-key reinsert (in-place suffix value update), remove (suffix
// disposal + emptied-node pruning), reinsert — the mutation ladder the
// concurrency work routes through disposal helpers.
#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_strmap)]
fn strmap_churn(built: (ExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (mut map, probes) = built;
    let mut sink = 0u64;
    for k in &probes {
        sink ^= map.insert(black_box(tk(k)), black_box(7)).unwrap_or(0);
        sink ^= map.remove(black_box(tk(k))).unwrap_or(0);
        map.insert(black_box(tk(k)), black_box(9));
    }
    core::mem::forget(map);
    black_box(sink)
}

// The string-map twins of `map_oscillate`, `map_refill` and
// `map_clear_refill` (#1119).
#[library_benchmark]
#[bench::routes(args = ("routes",), setup = str_keys)]
fn strmap_oscillate(ks: Vec<Vec<u8>>) -> u64 {
    let mut map = ExpanseStrMap::new();
    let mut sink = 0u64;
    for (i, k) in ks.iter().enumerate() {
        map.insert(black_box(tk(k)), black_box(i as u64));
        sink ^= map.remove(black_box(tk(k))).unwrap_or(0);
    }
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_strmap)]
fn strmap_refill(built: (ExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (mut map, probes) = built;
    let mut sink = 0u64;
    for k in &probes {
        sink ^= map.remove(black_box(tk(k))).unwrap_or(0);
    }
    for (i, k) in probes.iter().enumerate() {
        map.insert(black_box(tk(k)), black_box(i as u64));
    }
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_strmap)]
fn strmap_clear_refill(built: (ExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (mut map, probes) = built;
    map.clear();
    for (i, k) in probes.iter().enumerate() {
        map.insert(black_box(tk(k)), black_box(i as u64));
    }
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

/// The first 1,000 keys of `str_keys` (18 slab pages), drained and refilled
/// 50 times: a tree of a few pages that empties repeatedly, where returning
/// its blocks on every empty and carving them again would show (#1119).
fn small_strmap(_dist: &str) -> (ExpanseStrMap, Vec<Vec<u8>>) {
    let mut ks = str_keys("routes");
    ks.truncate(1_000);
    let mut map = ExpanseStrMap::new();
    for (i, k) in ks.iter().enumerate() {
        map.insert(tk(k), i as u64);
    }
    (map, shuffled_bytes(ks))
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = small_strmap)]
fn strmap_refill_small(built: (ExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (mut map, probes) = built;
    let mut sink = 0u64;
    for _ in 0..50 {
        for k in &probes {
            sink ^= map.remove(black_box(tk(k))).unwrap_or(0);
        }
        for (i, k) in probes.iter().enumerate() {
            map.insert(black_box(tk(k)), black_box(i as u64));
        }
    }
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = str_keys)]
fn bytesmap_insert(ks: Vec<Vec<u8>>) -> u64 {
    let mut map = ExpanseBytesMap::with_hasher(DetHasher::default());
    for (i, k) in ks.iter().enumerate() {
        map.insert(black_box(k), black_box(i as u64));
    }
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_bytesmap)]
fn bytesmap_get(built: (ExpanseBytesMap<DetHasher>, Vec<Vec<u8>>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for k in &probes {
        sink ^= map.get(black_box(k)).unwrap_or(0);
    }
    core::mem::forget(map);
    black_box(sink)
}

// Same-key reinsert (in-place value update), remove (bucket
// replacement/removal), reinsert (fresh bucket) — the paths #364
// restructured to publish-replacement-then-dispose.
#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_bytesmap)]
fn bytesmap_churn(built: (ExpanseBytesMap<DetHasher>, Vec<Vec<u8>>)) -> u64 {
    let (mut map, probes) = built;
    let mut sink = 0u64;
    for k in &probes {
        sink ^= map.insert(black_box(k), black_box(7)).unwrap_or(0);
        sink ^= map.remove(black_box(k)).unwrap_or(0);
        map.insert(black_box(k), black_box(9));
    }
    core::mem::forget(map);
    black_box(sink)
}

// Remove of every present key, shuffled: bucket removal and, as the hash
// trie empties, terminal demotion. `sync_bytesmap_remove` is its twin.
#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_bytesmap)]
fn bytesmap_remove(built: (ExpanseBytesMap<DetHasher>, Vec<Vec<u8>>)) -> u64 {
    let (mut map, probes) = built;
    let mut removed = 0u64;
    for k in &probes {
        removed += u64::from(map.remove(black_box(k)).is_some());
    }
    core::mem::forget(map);
    black_box(removed)
}

// Insert over a present key, and nothing else: `bytesmap_churn` folds this
// step in with a remove and a fresh insert, so a change to the present-key
// path (the membership probe ahead of the slot insert, #929) moves only part
// of that arm and cannot be read off it. Every probe is a hit; the population
// does not change.
#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_bytesmap)]
fn bytesmap_overwrite(built: (ExpanseBytesMap<DetHasher>, Vec<Vec<u8>>)) -> u64 {
    let (mut map, probes) = built;
    let mut sink = 0u64;
    for k in &probes {
        sink ^= map.insert(black_box(k), black_box(7)).unwrap_or(0);
    }
    core::mem::forget(map);
    black_box(sink)
}

// ---- Blob map (#929) ------------------------------------------------------
//
// The plain `ExpanseBlobMap` over `keys("random")`, with the payload and
// metadata the `sync_blobmap_*` arms use (`blob_payload`, `blob_meta`, further
// down): 16 bytes, above the 7-byte inline limit, and non-zero 24-bit
// metadata, so every value is an arena record and no insert takes the
// compressed-inline path. Each `sync_blobmap_*` arm is the same body through
// the wrapper, so the difference between a pair is the wrapper's cost.
// `blobmap_insert_inline` is the exception: a 7-byte payload, stored in the
// slot word with no arena allocation.
//
// Every insert result is checked with `expect`, in setup and in the measured
// bodies alike, as the `sync_blobmap_*` arms do: a rejected insert
// (`MetaOverflow`, `AllocationFailed`) would otherwise leave an arm measuring
// an error return under an insert label.

/// Prebuilt blob map plus shuffled probe order.
fn built_blobmap(dist: &str) -> (ExpanseBlobMap, Vec<u64>) {
    let ks = keys(dist);
    let mut map = ExpanseBlobMap::new();
    for &k in &ks {
        map.insert(k, &blob_payload(k), blob_meta(k))
            .expect("blob insert");
    }
    (map, shuffled(ks))
}

/// The inline blob arm's payload for `k`: 7 key-derived bytes, the largest
/// payload the slot word holds.
fn blob_payload_inline(k: u64) -> [u8; 7] {
    let mut out = [0u8; 7];
    out.copy_from_slice(&k.to_le_bytes()[..7]);
    out
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = keys)]
fn blobmap_insert(ks: Vec<u64>) -> u64 {
    let mut map = ExpanseBlobMap::new();
    for &k in &ks {
        map.insert(
            black_box(k),
            black_box(&blob_payload(k)),
            black_box(blob_meta(k)),
        )
        .expect("blob insert");
    }
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

// Inline payloads carry no metadata field, so `hot_meta` is passed as 0.
#[library_benchmark]
#[bench::random(args = ("random",), setup = keys)]
fn blobmap_insert_inline(ks: Vec<u64>) -> u64 {
    let mut map = ExpanseBlobMap::new();
    for &k in &ks {
        map.insert(
            black_box(k),
            black_box(&blob_payload_inline(k)),
            black_box(0),
        )
        .expect("blob insert");
    }
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

// The payload is dereferenced, not just located: the sink folds in a payload
// byte and the length along with the metadata, so the arena line is read.
#[library_benchmark]
#[bench::random(args = ("random",), setup = built_blobmap)]
fn blobmap_get(built: (ExpanseBlobMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for &k in &probes {
        if let Some((view, meta)) = map.get(black_box(k)) {
            let bytes = view.as_bytes();
            sink ^= u64::from(meta)
                ^ bytes.len() as u64
                ^ u64::from(bytes.first().copied().unwrap_or(0));
        }
    }
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_blobmap)]
fn blobmap_remove(built: (ExpanseBlobMap, Vec<u64>)) -> u64 {
    let (mut map, probes) = built;
    let mut removed = 0u64;
    for &k in &probes {
        removed += u64::from(map.remove(black_box(k)));
    }
    core::mem::forget(map);
    black_box(removed)
}

// Insert over a present key with a payload of the same size: the index walk
// that reads the old slot, a fresh arena record, the index walk that stores
// the new slot, and the old record counted as garbage. The arena is
// append-only, so the old bytes stay stranded until a compaction; this arm
// runs none.
#[library_benchmark]
#[bench::random(args = ("random",), setup = built_blobmap)]
fn blobmap_overwrite(built: (ExpanseBlobMap, Vec<u64>)) -> u64 {
    let (mut map, probes) = built;
    for &k in &probes {
        map.insert(
            black_box(k),
            black_box(&blob_payload(!k)),
            black_box(blob_meta(!k)),
        )
        .expect("blob replace");
    }
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

// Same-key replace, remove, reinsert: `sync_blobmap_churn`'s ladder on the
// plain map.
#[library_benchmark]
#[bench::random(args = ("random",), setup = built_blobmap)]
fn blobmap_churn(built: (ExpanseBlobMap, Vec<u64>)) -> u64 {
    let (mut map, probes) = built;
    let mut sink = 0u64;
    for &k in &probes {
        map.insert(
            black_box(k),
            black_box(&blob_payload(!k)),
            black_box(blob_meta(!k)),
        )
        .expect("blob replace");
        sink ^= u64::from(map.remove(black_box(k)));
        map.insert(
            black_box(k),
            black_box(&blob_payload(k)),
            black_box(blob_meta(k)),
        )
        .expect("blob reinsert");
    }
    core::mem::forget(map);
    black_box(sink)
}

// ---- Concurrent wrappers, one thread (#568) -----------------------------
//
// The same operations through `SyncExpanseMap` / `SyncExpanseSet` on a
// single thread: the uncontended writer mutex, the tree-level bracket,
// every per-node version bracket, deferred reclamation and the epoch
// advance every 32 writes — the `OCC=true` monomorph of the engine, which
// no other arm reaches (`version_begin_if::<false>` compiles the brackets
// out of the plain arms). One thread means no lock is ever contended and
// the advance interval is a constant, so the count is exact. A change to
// the bracket scope (#568) is measured here; the plain arms above are its
// control for the single-threaded engine. `sync_map_get` is the
// optimistic reader (pin, sample, hand-over-hand walk) with no writer.

fn shuffled(ks: Vec<u64>) -> Vec<u64> {
    let mut probes = ks;
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    probes
}

fn built_sync_map(dist: &str) -> (SyncExpanseMap, Vec<u64>) {
    let ks = keys(dist);
    let map = SyncExpanseMap::new();
    for &k in &ks {
        map.insert(k, !k);
    }
    (map, shuffled(ks))
}

/// Keys in the `leaf` variants of the `sync_*` arms: below `ROOT_LEAF_CAP`, so
/// the map or set stays a root leaf throughout, including the churn arms'
/// transient extra key. The root-leaf path is what stage (a) of the #1086
/// class-1 plan converts (`docs/ARCHITECTURE.md` §4.2); every tree-population
/// arm bypasses it. The churn arms' inserts of a present key overwrite in
/// place; their transient key `k ^ 1` sorts next to `k`, and the keys are
/// uniform, so those inserts land at a uniform position in the leaf and shift
/// half of it on average.
const LEAF_POP: usize = 24;
const _: () = assert!(LEAF_POP < expanse_trie::types::ROOT_LEAF_CAP);

/// `LEAF_POP` keys drawn as `keys("random")` draws them, and a probe stream
/// that cycles over them in shuffled order until it is `POP` long, so a
/// `leaf` variant divides by the same op count as its `random` twin.
fn leaf_keys_and_probes() -> (Vec<u64>, Vec<u64>) {
    let ks: Vec<u64> = keys("random").into_iter().take(LEAF_POP).collect();
    let probes: Vec<u64> = ks.iter().copied().cycle().take(POP).collect();
    (ks, shuffled(probes))
}

fn built_sync_map_leaf(_: &str) -> (SyncExpanseMap, Vec<u64>) {
    let (ks, probes) = leaf_keys_and_probes();
    let map = SyncExpanseMap::new();
    for &k in &ks {
        map.insert(k, !k);
    }
    // Built by inserts alone, a map promotes its root leaf to a tree only
    // when an insert takes it past `ROOT_LEAF_CAP`.
    assert_eq!(
        map.len(),
        LEAF_POP as u64,
        "a leaf variant must measure a root leaf"
    );
    (map, probes)
}

fn built_sync_set_leaf(_: &str) -> (SyncExpanseSet, Vec<u64>) {
    let (ks, probes) = leaf_keys_and_probes();
    let set = SyncExpanseSet::new();
    for &k in &ks {
        set.insert(k);
    }
    // As for the map.
    assert_eq!(
        set.len(),
        LEAF_POP as u64,
        "a leaf variant must measure a root leaf"
    );
    (set, probes)
}

fn built_sync_set(dist: &str) -> (SyncExpanseSet, Vec<u64>) {
    let ks = keys(dist);
    let set = SyncExpanseSet::new();
    for &k in &ks {
        set.insert(k);
    }
    (set, shuffled(ks))
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = keys)]
fn sync_map_insert(ks: Vec<u64>) -> u64 {
    let map = SyncExpanseMap::new();
    for &k in &ks {
        map.insert(black_box(k), black_box(!k));
    }
    let n = map.len();
    // Leaked, as every insert arm is: teardown is a different path.
    core::mem::forget(map);
    black_box(n)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = keys)]
fn sync_set_insert(ks: Vec<u64>) -> u64 {
    let set = SyncExpanseSet::new();
    for &k in &ks {
        set.insert(black_box(k));
    }
    let n = set.len();
    core::mem::forget(set);
    black_box(n)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_map)]
#[bench::leaf(args = ("leaf",), setup = built_sync_map_leaf)]
fn sync_map_get(built: (SyncExpanseMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let rd = map.reader();
    let mut sink = 0u64;
    for &k in &probes {
        sink ^= rd.get(black_box(k)).unwrap_or(0);
    }
    // Both leaked: the reader's deregistration and the map's teardown are
    // other paths (see `map_get`).
    core::mem::forget(rd);
    core::mem::forget(map);
    black_box(sink)
}

// Ordered navigation on the concurrent map before #900: `with_locked` is the
// only route, and it runs each call with every writer excluded. The
// optimistic ordered reads #900 adds are compared against this cell and
// against `map_prev`.
#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_map)]
fn sync_map_prev_locked(built: (SyncExpanseMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for &k in &probes {
        if let Some((pk, pv)) = map.with_locked(|m| m.prev_before(black_box(k))) {
            sink ^= pk ^ pv;
        }
    }
    // Leaked — see `map_get`.
    core::mem::forget(map);
    black_box(sink)
}

// The optimistic ordered read (#900): the probes of `sync_map_prev_locked`,
// through a reader handle that does not exclude writers. Prediction P12.3
// (`docs/benchmarks/concurrency/METHODOLOGY.md` §12.3) compares the two.
#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_map)]
fn sync_map_prev(built: (SyncExpanseMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let rd = map.reader();
    let mut sink = 0u64;
    for &k in &probes {
        if let Some((pk, pv)) = rd.prev_before(black_box(k)) {
            sink ^= pk ^ pv;
        }
    }
    // Both leaked — see `sync_map_get`.
    core::mem::forget(rd);
    core::mem::forget(map);
    black_box(sink)
}

// A full ascending scan through the optimistic ordered reads (#1142): one
// `next_after` per entry, each a pin, a version sample and a descent from the
// root. The per-element baseline a concurrent batch cursor is predicted
// against; `map_cursor_scan` is its single-threaded twin.
#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_map)]
#[bench::sequential(args = ("sequential",), setup = built_sync_map)]
#[bench::clustered(args = ("clustered",), setup = built_sync_map)]
fn sync_map_next_after_scan(built: (SyncExpanseMap, Vec<u64>)) -> u64 {
    let (map, _probes) = built;
    let rd = map.reader();
    let mut sink = 0u64;
    let mut n = 0usize;
    let mut at = rd.first();
    while let Some((k, v)) = at {
        sink ^= k ^ v;
        n += 1;
        at = rd.next_after(black_box(k));
    }
    assert_eq!(n, POP, "the scan visits every entry once");
    // Both leaked — see `sync_map_get`.
    core::mem::forget(rd);
    core::mem::forget(map);
    black_box(sink)
}

// ---- Concurrent counts under a writer (#1144) -----------------------------
//
// Counts on the concurrent map go through `with_locked`, which folds the
// ancestor `pop0` an optimistic writer left stale (the subtrees under the
// top digits marked dirty since the last fold). Three arms over one probe
// stream and one stream of absent keys:
//
// - `sync_map_count_locked`: `count_below` through `with_locked` per probe,
//   no writes, so the dirty mask stays clean after setup's fold;
// - `sync_map_write_twin`: insert an absent key and remove it, per probe;
// - `sync_map_count_after_write`: insert it, count, remove it.
//
// Callgrind counts are additive, so the combined arm minus its two twins is
// the fold's cost (each count refolds the digits the previous step's remove
// and this step's insert marked). `one_top_byte` puts every key under one
// top digit, the issue's worst case. `COUNT_OPS` steps per arm, over the
// 50k-key map.

/// Steps per concurrent count arm. Each `one_top_byte` count after a write
/// refolds the whole tree, so a full `POP` pass would dominate the job; the
/// three arms share this one stream, which keeps the subtraction exact.
const COUNT_OPS: usize = 1_000;

fn built_sync_map_count(dist: &str) -> (SyncExpanseMap, Vec<u64>, Vec<u64>) {
    let (map, mut probes) = built_sync_map(dist);
    let mut fresh = fresh_keys(dist);
    probes.truncate(COUNT_OPS);
    fresh.truncate(COUNT_OPS);
    // Setup ends with a fold, so every arm starts from a clean mask.
    assert_eq!(map.with_locked(|m| m.len()), POP as u64);
    (map, probes, fresh)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_map_count)]
#[bench::sequential(args = ("sequential",), setup = built_sync_map_count)]
#[bench::one_top_byte(args = ("one_top_byte",), setup = built_sync_map_count)]
fn sync_map_count_locked(built: (SyncExpanseMap, Vec<u64>, Vec<u64>)) -> u64 {
    let (map, probes, _fresh) = built;
    let mut sink = 0u64;
    for &k in &probes {
        sink = sink.wrapping_add(map.with_locked(|m| m.count_below(black_box(k))));
    }
    // Leaked — see `sync_map_get`.
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_map_count)]
#[bench::sequential(args = ("sequential",), setup = built_sync_map_count)]
#[bench::one_top_byte(args = ("one_top_byte",), setup = built_sync_map_count)]
fn sync_map_write_twin(built: (SyncExpanseMap, Vec<u64>, Vec<u64>)) -> u64 {
    let (map, _probes, fresh) = built;
    let mut sink = 0u64;
    for &f in &fresh {
        sink ^= map.insert(black_box(f), f).unwrap_or(0);
        sink ^= map.remove(black_box(f)).unwrap_or(0);
    }
    assert_eq!(map.len(), POP as u64);
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_map_count)]
#[bench::sequential(args = ("sequential",), setup = built_sync_map_count)]
#[bench::one_top_byte(args = ("one_top_byte",), setup = built_sync_map_count)]
fn sync_map_count_after_write(built: (SyncExpanseMap, Vec<u64>, Vec<u64>)) -> u64 {
    let (map, probes, fresh) = built;
    let mut sink = 0u64;
    for (&k, &f) in probes.iter().zip(&fresh) {
        sink ^= map.insert(black_box(f), f).unwrap_or(0);
        sink = sink.wrapping_add(map.with_locked(|m| m.count_below(black_box(k))));
        sink ^= map.remove(black_box(f)).unwrap_or(0);
    }
    assert_eq!(map.len(), POP as u64);
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_set)]
#[bench::leaf(args = ("leaf",), setup = built_sync_set_leaf)]
fn sync_set_contains(built: (SyncExpanseSet, Vec<u64>)) -> u64 {
    let (set, probes) = built;
    let rd = set.reader();
    let mut hits = 0u64;
    for &k in &probes {
        hits += u64::from(rd.contains(black_box(k)));
    }
    core::mem::forget(rd);
    core::mem::forget(set);
    black_box(hits)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_map)]
#[bench::leaf(args = ("leaf",), setup = built_sync_map_leaf)]
fn sync_map_churn(built: (SyncExpanseMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let n0 = map.len();
    let mut sink = 0u64;
    for &k in &probes {
        sink ^= map.insert(black_box(k), black_box(!k)).unwrap_or(0);
        map.insert(black_box(k ^ 1), k);
        sink ^= u64::from(map.remove(black_box(k ^ 1)).is_some());
    }
    // Churn leaves the population as it found it, so a `leaf` variant stays
    // at `LEAF_POP` keys, a root leaf, from its setup to here.
    assert_eq!(map.len(), n0);
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_map)]
fn sync_map_remove(built: (SyncExpanseMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let mut removed = 0u64;
    for &k in &probes {
        removed += u64::from(map.remove(black_box(k)).is_some());
    }
    core::mem::forget(map);
    black_box(removed)
}

// The read-modify-write a caller builds from the conditional publish: a
// validated read, then `compare_exchange` over the word it returned. One
// thread, so every compare succeeds and every op is one read and one
// optimistic descent that stores in place.
#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_map)]
fn sync_map_compare_exchange(built: (SyncExpanseMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let rd = map.reader();
    let mut won = 0u64;
    for &k in &probes {
        let cur = rd.get(black_box(k));
        let next = cur.map(|v| v.wrapping_add(1));
        won += u64::from(map.compare_exchange(black_box(k), cur, next).is_ok());
    }
    // Both leaked — see `sync_map_get`.
    core::mem::forget(rd);
    core::mem::forget(map);
    black_box(won)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_set)]
#[bench::leaf(args = ("leaf",), setup = built_sync_set_leaf)]
fn sync_set_churn(built: (SyncExpanseSet, Vec<u64>)) -> u64 {
    let (set, probes) = built;
    let n0 = set.len();
    let mut sink = 0u64;
    for &k in &probes {
        sink ^= u64::from(set.insert(black_box(k)));
        set.insert(black_box(k ^ 1));
        sink ^= u64::from(set.remove(black_box(k ^ 1)));
    }
    // Churn leaves the population as it found it, so a `leaf` variant stays
    // at `LEAF_POP` keys, a root leaf, from its setup to here.
    assert_eq!(set.len(), n0);
    core::mem::forget(set);
    black_box(sink)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_set)]
fn sync_set_remove(built: (SyncExpanseSet, Vec<u64>)) -> u64 {
    let (set, probes) = built;
    let mut removed = 0u64;
    for &k in &probes {
        removed += u64::from(set.remove(black_box(k)));
    }
    core::mem::forget(set);
    black_box(removed)
}

// ---- The string wrapper's reader on `short` keys (#730) ------------------
//
// `short` is the key shape of the `masstree_conc_str` cell and of the
// `writer_scaling` `str` arm: random alphanumerics, 8 to 16 bytes. The
// route-shaped `str_keys` above share long prefixes and cross more sub-tries
// per key, so they are a different descent. `sync_strmap_get_short` minus
// `strmap_get_short` is the wrapper's per-probe instruction overhead on that
// shape at POP = 50,000 (pin, version sample, validated cascade), and a lower
// bound for a 2^20 population: trie depth, and with it the validations per
// hop, grows with population. One thread, no writer, so no restart is taken.

/// `short` string keys: random alphanumerics, 8..=16 bytes, the file's
/// XorShift, duplicates dropped in draw order so the population is exactly
/// `POP` distinct keys. NUL-free by construction.
fn short_keys(_dist: &str) -> Vec<Vec<u8>> {
    const ALNUM: &[u8; 62] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0730);
    let mut seen = std::collections::HashSet::with_capacity(POP);
    let mut out = Vec::with_capacity(POP);
    while out.len() < POP {
        let n = 8 + (rng.next() % 9) as usize;
        let k: Vec<u8> = (0..n).map(|_| ALNUM[(rng.next() % 62) as usize]).collect();
        if seen.insert(k.clone()) {
            out.push(k);
        }
    }
    out
}

/// A probe order that is not the build order, for string keys.
fn shuffled_bytes(ks: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut probes = ks;
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    probes
}

fn built_strmap_short(dist: &str) -> (ExpanseStrMap, Vec<Vec<u8>>) {
    let ks = short_keys(dist);
    let mut map = ExpanseStrMap::new();
    for (i, k) in ks.iter().enumerate() {
        map.insert(tk(k), i as u64);
    }
    (map, shuffled_bytes(ks))
}

fn built_sync_strmap_short(dist: &str) -> (SyncExpanseStrMap, Vec<Vec<u8>>) {
    let ks = short_keys(dist);
    let map = SyncExpanseStrMap::new();
    for (i, k) in ks.iter().enumerate() {
        map.insert(tk(k), i as u64);
    }
    (map, shuffled_bytes(ks))
}

#[library_benchmark]
#[bench::short(args = ("short",), setup = built_strmap_short)]
fn strmap_get_short(built: (ExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for k in &probes {
        sink ^= map.get(black_box(tk(k))).unwrap_or(0);
    }
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::short(args = ("short",), setup = built_sync_strmap_short)]
fn sync_strmap_get_short(built: (SyncExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (map, probes) = built;
    let rd = map.reader();
    let mut sink = 0u64;
    for k in &probes {
        sink ^= rd.get(black_box(tk(k))).unwrap_or(0);
    }
    // Both leaked — see `sync_map_get`.
    core::mem::forget(rd);
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::short(args = ("short",), setup = short_keys)]
fn sync_strmap_insert_short(ks: Vec<Vec<u8>>) -> u64 {
    let map = SyncExpanseStrMap::new();
    for (i, k) in ks.iter().enumerate() {
        map.insert(black_box(tk(k)), black_box(i as u64));
    }
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

// The `strmap_churn` ladder through the wrapper: same-key reinsert, remove,
// reinsert, each under the writer mutex and the tree-level bracket.
#[library_benchmark]
#[bench::short(args = ("short",), setup = built_sync_strmap_short)]
fn sync_strmap_churn_short(built: (SyncExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for k in &probes {
        sink ^= map.insert(black_box(tk(k)), black_box(7)).unwrap_or(0);
        sink ^= map.remove(black_box(tk(k))).unwrap_or(0);
        map.insert(black_box(tk(k)), black_box(9));
    }
    core::mem::forget(map);
    black_box(sink)
}

// ---- The coarse-mutex wrappers' mutations, one thread (#929) -------------
//
// `SyncExpanseStrMap`, `SyncExpanseBytesMap` and `SyncExpanseBlobMap` route
// every insert and remove through `Shared::write`: the writer mutex, the
// tree-level version bracket, deferred reclamation through the collector.
// One thread, so no lock is contended and the count is exact. The string and
// byte-string arms run the `str_keys` population their plain twins
// (`strmap_*`, `bytesmap_*`) run, so each difference is the wrapper's cost
// on that path. The blob arms' plain twins are `blobmap_*`; their payloads
// are 16 bytes, above the 7-byte inline limit, with non-zero metadata, so
// every value lives in the arena.
//
// The `get` arms are the optimistic readers (pin, sample, validated walk) with
// no writer, as `sync_map_get` is. The `overwrite` arms are the present-key
// insert alone, which the churn arms fold in with a remove and a reinsert.

fn built_sync_strmap(dist: &str) -> (SyncExpanseStrMap, Vec<Vec<u8>>) {
    let ks = str_keys(dist);
    let map = SyncExpanseStrMap::new();
    for (i, k) in ks.iter().enumerate() {
        map.insert(tk(k), i as u64);
    }
    (map, shuffled_bytes(ks))
}

fn built_sync_bytesmap(dist: &str) -> (SyncExpanseBytesMap<DetHasher>, Vec<Vec<u8>>) {
    let ks = str_keys(dist);
    let map = SyncExpanseBytesMap::with_hasher(DetHasher::default());
    for (i, k) in ks.iter().enumerate() {
        map.insert(k, i as u64);
    }
    (map, shuffled_bytes(ks))
}

/// The blob arms' payload for `k`: 16 key-derived bytes, an arena payload.
fn blob_payload(k: u64) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&k.to_le_bytes());
    out[8..].copy_from_slice(&(!k).to_le_bytes());
    out
}

/// The blob arms' 24-bit metadata for `k`; never zero, so no insert takes the
/// metadata-free compressed-inline path.
fn blob_meta(k: u64) -> u32 {
    (k as u32 & 0x00FF_FFFF) | 1
}

fn built_sync_blobmap(dist: &str) -> (SyncExpanseBlobMap, Vec<u64>) {
    let ks = keys(dist);
    let map = SyncExpanseBlobMap::new();
    for &k in &ks {
        map.insert(k, &blob_payload(k), blob_meta(k))
            .expect("blob insert");
    }
    (map, shuffled(ks))
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = str_keys)]
fn sync_strmap_insert(ks: Vec<Vec<u8>>) -> u64 {
    let map = SyncExpanseStrMap::new();
    for (i, k) in ks.iter().enumerate() {
        map.insert(black_box(tk(k)), black_box(i as u64));
    }
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_sync_strmap)]
fn sync_strmap_remove(built: (SyncExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (map, probes) = built;
    let mut removed = 0u64;
    for k in &probes {
        removed += u64::from(map.remove(black_box(tk(k))).is_some());
    }
    core::mem::forget(map);
    black_box(removed)
}

// `strmap_churn`'s ladder through the wrapper.
#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_sync_strmap)]
fn sync_strmap_churn(built: (SyncExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for k in &probes {
        sink ^= map.insert(black_box(tk(k)), black_box(7)).unwrap_or(0);
        sink ^= map.remove(black_box(tk(k))).unwrap_or(0);
        map.insert(black_box(tk(k)), black_box(9));
    }
    core::mem::forget(map);
    black_box(sink)
}

// ---- Ascending UUID text keys through the string wrapper (#1162) ---------
//
// The reported workload: canonical lowercase UUIDv4 strings, sorted, inserted
// in order through `SyncExpanseStrMap::insert` on one thread. Ascending order
// makes the engine's optimistic insert fall back to the exclusive path
// repeatedly (linear branches filling one digit at a time, prefix splits),
// and that path used to refold the whole dirty root sub-map on each fallback:
// a per-insert cost that grows with the population. `UUID_POP` is smaller
// than `POP` because the arm is also run against the base commit, where the
// load is quadratic.

/// Population of the ascending-UUID arm.
const UUID_POP: usize = 20_000;

/// `UUID_POP` distinct canonical UUIDv4 strings from a SplitMix64 stream,
/// sorted ascending. NUL-free by construction.
fn uuid_keys_sorted(_dist: &str) -> Vec<Vec<u8>> {
    fn splitmix64(mut z: u64) -> u64 {
        z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    let mut out: Vec<Vec<u8>> = (0..UUID_POP as u64)
        .map(|i| {
            let hi = (splitmix64(i << 1) & 0xFFFF_FFFF_FFFF_0FFF) | 0x4000;
            let lo = (splitmix64((i << 1) | 1) & 0x3FFF_FFFF_FFFF_FFFF) | (1 << 63);
            format!(
                "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
                hi >> 32,
                (hi >> 16) & 0xFFFF,
                hi & 0xFFFF,
                lo >> 48,
                lo & 0xFFFF_FFFF_FFFF
            )
            .into_bytes()
        })
        .collect();
    out.sort();
    out.dedup();
    assert_eq!(out.len(), UUID_POP, "uuid_keys_sorted drew a duplicate");
    out
}

#[library_benchmark]
#[bench::uuid(args = ("uuid",), setup = uuid_keys_sorted)]
fn sync_strmap_insert_sorted(ks: Vec<Vec<u8>>) -> u64 {
    let map = SyncExpanseStrMap::new();
    for (i, k) in ks.iter().enumerate() {
        map.insert(black_box(tk(k)), black_box(i as u64));
    }
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = str_keys)]
fn sync_bytesmap_insert(ks: Vec<Vec<u8>>) -> u64 {
    let map = SyncExpanseBytesMap::with_hasher(DetHasher::default());
    for (i, k) in ks.iter().enumerate() {
        map.insert(black_box(k), black_box(i as u64));
    }
    let n = map.len();
    core::mem::forget(map);
    black_box(n)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_sync_bytesmap)]
fn sync_bytesmap_remove(built: (SyncExpanseBytesMap<DetHasher>, Vec<Vec<u8>>)) -> u64 {
    let (map, probes) = built;
    let mut removed = 0u64;
    for k in &probes {
        removed += u64::from(map.remove(black_box(k)).is_some());
    }
    core::mem::forget(map);
    black_box(removed)
}

// `bytesmap_churn`'s ladder through the wrapper.
#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_sync_bytesmap)]
fn sync_bytesmap_churn(built: (SyncExpanseBytesMap<DetHasher>, Vec<Vec<u8>>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for k in &probes {
        sink ^= map.insert(black_box(k), black_box(7)).unwrap_or(0);
        sink ^= map.remove(black_box(k)).unwrap_or(0);
        map.insert(black_box(k), black_box(9));
    }
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_sync_bytesmap)]
fn sync_bytesmap_get(built: (SyncExpanseBytesMap<DetHasher>, Vec<Vec<u8>>)) -> u64 {
    let (map, probes) = built;
    let rd = map.reader();
    let mut sink = 0u64;
    for k in &probes {
        sink ^= rd.get(black_box(k)).unwrap_or(0);
    }
    // Both leaked — see `sync_map_get`.
    core::mem::forget(rd);
    core::mem::forget(map);
    black_box(sink)
}

// `bytesmap_overwrite` through the wrapper.
#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_sync_bytesmap)]
fn sync_bytesmap_overwrite(built: (SyncExpanseBytesMap<DetHasher>, Vec<Vec<u8>>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for k in &probes {
        sink ^= map.insert(black_box(k), black_box(7)).unwrap_or(0);
    }
    core::mem::forget(map);
    black_box(sink)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = keys)]
fn sync_blobmap_insert(ks: Vec<u64>) -> u64 {
    let map = SyncExpanseBlobMap::new();
    for &k in &ks {
        map.insert(
            black_box(k),
            black_box(&blob_payload(k)),
            black_box(blob_meta(k)),
        )
        .expect("blob insert");
    }
    let n = map.with_locked(|m| m.len());
    core::mem::forget(map);
    black_box(n)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_blobmap)]
fn sync_blobmap_remove(built: (SyncExpanseBlobMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let mut removed = 0u64;
    for &k in &probes {
        removed += u64::from(map.remove(black_box(k)));
    }
    core::mem::forget(map);
    black_box(removed)
}

// Same-key replace (a new arena payload, the old one recorded as garbage),
// remove, reinsert: the blob wrapper's mutation ladder.
#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_blobmap)]
fn sync_blobmap_churn(built: (SyncExpanseBlobMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let mut sink = 0u64;
    for &k in &probes {
        map.insert(
            black_box(k),
            black_box(&blob_payload(!k)),
            black_box(blob_meta(!k)),
        )
        .expect("blob replace");
        sink ^= u64::from(map.remove(black_box(k)));
        map.insert(
            black_box(k),
            black_box(&blob_payload(k)),
            black_box(blob_meta(k)),
        )
        .expect("blob reinsert");
    }
    core::mem::forget(map);
    black_box(sink)
}

// `blobmap_get` through the wrapper's zero-copy read: one pin per probe, as
// the other readers' `get` takes, then the validated walk and a borrow of the
// epoch-pinned arena bytes. `BlobReader::get` is not used: it copies the
// payload into a fresh `Vec`, and the arm would count the allocator.
#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_blobmap)]
fn sync_blobmap_get(built: (SyncExpanseBlobMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let mut rd = map.reader();
    let mut sink = 0u64;
    for &k in &probes {
        let guard = rd.pin();
        if let Some((view, meta)) = guard.get(black_box(k)) {
            let bytes = view.as_bytes();
            sink ^= u64::from(meta)
                ^ bytes.len() as u64
                ^ u64::from(bytes.first().copied().unwrap_or(0));
        }
    }
    // Both leaked — see `sync_map_get`.
    core::mem::forget(rd);
    core::mem::forget(map);
    black_box(sink)
}

// `blobmap_overwrite` through the wrapper.
#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_blobmap)]
fn sync_blobmap_overwrite(built: (SyncExpanseBlobMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    for &k in &probes {
        map.insert(
            black_box(k),
            black_box(&blob_payload(!k)),
            black_box(blob_meta(!k)),
        )
        .expect("blob replace");
    }
    let n = map.with_locked(|m| m.len());
    core::mem::forget(map);
    black_box(n)
}

/// Callgrind simulator settings for this harness.
///
/// **`--cache-sim=yes` is stated here, not inherited.** iai-callgrind's runner
/// already defaults it on (`iai-callgrind-runner` `defaults::CACHE_SIM = true`),
/// which is why the L1/LL/RAM hit counts `scripts/perf_report.py` renders have
/// been in every PR comment all along. Passing it explicitly changes no number
/// and costs nothing; it makes the harness say which instrument it uses instead
/// of depending on a dependency default that a version bump could flip.
///
/// The simulated cache is fixed by the runner and is **not this machine's**:
/// I1 and D1 are 32 KiB 8-way, LL is 8 MiB 16-way, 64-byte lines. Fixed sizes
/// are what make the counts comparable across hosts, and they are also why a
/// question about a real last-level cache — where the L3 cliff sits on the
/// reference host, for instance — cannot be answered here. That needs hardware
/// counters (`scripts/perf_counters.py`).
///
/// **`--branch-sim=yes` is opt-in through `EXPANSE_BRANCH_SIM=1`.** It has no
/// runner default, and it adds a branch-predictor simulation on top of
/// callgrind's own slowdown, so the regression pass — gated on instructions
/// retired, needing no branch column — leaves it off and measures exactly what
/// it measured before. Instruction counts are unaffected either way.
///
/// An unrecognised value is fatal rather than ignored. A mistyped
/// `EXPANSE_BRANCH_SIM=yes` that quietly produced a run with no branch columns
/// would be a run published as a misprediction measurement that never simulated
/// a predictor (`AGENTS.md` section 8.1).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn bench_config() -> LibraryBenchmarkConfig {
    let mut config = LibraryBenchmarkConfig::default();
    let mut args = vec!["--cache-sim=yes"];
    let requested = match std::env::var("EXPANSE_BRANCH_SIM") {
        Ok(v) => v,
        Err(std::env::VarError::NotPresent) => String::new(),
        Err(e) => panic!("EXPANSE_BRANCH_SIM is set but unreadable: {e}"),
    };
    match requested.as_str() {
        "" | "0" => {}
        "1" => args.push("--branch-sim=yes"),
        other => panic!(
            "EXPANSE_BRANCH_SIM={other} is not a recognised value: use 1 to add the \
             branch-predictor simulation, or 0 / unset to leave it off"
        ),
    }
    config.tool(Callgrind::with_args(args));
    config
}

library_benchmark_group!(
    name = cost;
    benchmarks =
        map_insert,
        set_insert,
        map_ins_slot,
        map_get,
        set_contains,
        map_get_batch,
        set_contains_batch,
        map_churn,
        map_remove,
        set_remove,
        map_oscillate,
        set_oscillate,
        map_refill,
        set_refill,
        map_clear_refill,
        set_clear_refill,
        set_remove_partial,
        map_remove_partial,
        set_rebuild_drained,
        map_rebuild_drained,
        set_compact_drained,
        map_compact_drained,
        set_subtree_boundary_oscillate,
        set_subtree_split,
        set_subtree_split_control,
        set_subtree_condense,
        set_subtree_condense_control,
        map_iterate,
        map_clone,
        set_clone,
        map_nav,
        map_prev,
        map_cursor_scan,
        map_count_below,
        set_count_below,
        set32_insert,
        map32_insert,
        map32_get,
        map32_iterate,
        map32_nav,
        map32_prev,
        map32_range,
        map32_for_each_range,
        set32_iterate,
        set32_range,
        map32_remove,
        set32_remove,
        blobmap32_scan,
        strmap_insert,
        strmap_get,
        strmap_churn,
        strmap_oscillate,
        strmap_refill,
        strmap_clear_refill,
        strmap_refill_small,
        strmap_prefix_scan,
        strmap_prefix_bounded,
        strmap_prefix_seek,
        strmap_cursor_scan,
        bytesmap_insert,
        bytesmap_get,
        bytesmap_churn,
        sync_map_insert,
        sync_set_insert,
        sync_map_get,
        sync_map_prev_locked,
        sync_map_prev,
        sync_map_next_after_scan,
        sync_map_count_locked,
        sync_map_write_twin,
        sync_map_count_after_write,
        sync_set_contains,
        sync_map_churn,
        sync_map_remove,
        sync_map_compare_exchange,
        sync_set_churn,
        sync_set_remove,
        strmap_get_short,
        sync_strmap_get_short,
        sync_strmap_insert_short,
        sync_strmap_churn_short,
        sync_strmap_insert,
        sync_strmap_remove,
        sync_strmap_churn,
        sync_strmap_insert_sorted,
        sync_bytesmap_insert,
        sync_bytesmap_remove,
        sync_bytesmap_churn,
        sync_blobmap_insert,
        sync_blobmap_remove,
        sync_blobmap_churn,
        bytesmap_remove,
        bytesmap_overwrite,
        blobmap_insert,
        blobmap_insert_inline,
        blobmap_get,
        blobmap_remove,
        blobmap_overwrite,
        blobmap_churn,
        sync_bytesmap_get,
        sync_bytesmap_overwrite,
        sync_blobmap_get,
        sync_blobmap_overwrite
);

library_benchmark_group!(
    name = range_cost;
    benchmarks =
        map_range,
        set_range
);

#[cfg(target_os = "linux")]
main!(config = bench_config(); library_benchmark_groups = cost, range_cost);

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("iai-callgrind instruction benchmarks run on Linux only.");
}
