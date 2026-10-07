//! Deterministic cost of the shipped read-modify-write entry points on the
//! concurrent maps: `compare_exchange` and `update` on `SyncExpanseMap`,
//! `SyncExpanseStrMap` and `SyncExpanseBytesMap`, `compare_exchange` on
//! `SyncExpanseBlobMap` (which has no `update`), and the value-to-absent
//! `compare_exchange` on `SyncExpanseBytesMap` that removes a bucket's last
//! entry, via callgrind.
//!
//! This is a second Callgrind binary, apart from `instructions.rs`, on
//! purpose. Arms that call these entry points were first written into
//! `instructions.rs` and moved existing arms there (`sync_blobmap_overwrite`,
//! `sync_blobmap_churn` and `sync_blobmap_insert` by +1.95% to +3.34%,
//! preliminary), because the new call sites change inlining in
//! instantiations the existing arms share. A separate binary has its own
//! instantiations, so the `instructions` arms are not touched (Refs #1395).
//! The `instructions` arms named `sync_*_compare_exchange` and `sync_*_update`
//! measure the `with_exclusive` forms and the `insert` they are built from;
//! the arms here call the entry points themselves.
//!
//! Requires valgrind, which does not support arm64 macOS — the arms run on
//! Linux (the `instruction-counts` CI job). Locally on Linux:
//! `cargo bench -p expanse-trie --bench rmw_instructions`. On other targets
//! `main` runs each arm's body once over the same population. Every arm
//! asserts that all of its operations succeeded, on every target, so an arm
//! that took a different path fails instead of reporting that path's cost.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `core_rmw_instructions` |
//! | `group` | 2 |
//! | `population` | `POP` (50k) keys per arm, above `ROOT_LEAF_CAP`, so every arm walks the tree and none takes the root-leaf short-circuit; `SyncExpanseMap` and `SyncExpanseBlobMap` take 50k random 64-bit keys, `SyncExpanseStrMap` and `SyncExpanseBytesMap` 50k route-shaped ASCII keys (~40 bytes, long shared prefixes) |
//! | `insertion_order` | generator draw order — the population is inserted as drawn, neither sorted nor shuffled; the shuffle is applied to the probe stream, not to the build |
//! | `probes_and_reuse` | 50k (shuffled), reuse 1.0: each present key is exchanged or updated exactly once; `sync_bytesmap_cas_remove` removes each key once, so the map is empty at the end |
//! | `hit_rate` | 100%: every `compare_exchange` carries the value the key holds, so every exchange succeeds (one thread, no concurrent writer), and every `update` replaces a present key |
//! | `miss_gen_method` | None: no arm probes an absent key |
//! | `value_dereference` | `black_box` on every key and expected value passed in, and on the success count returned |
//! | `measured_region` | Build and shuffle outside the region; the map and the probe stream are both leaked with `mem::forget`, so no teardown is counted. One `assert_eq!` on the success count is inside it |
//! | `arm_symmetry` | Internal trie paths; no competitor arm |
//! | `statistics` | iai Callgrind exact counts |
//! | `verdict` | **MEASURED** by the `instruction-counts` job: instruction counts per arm, exact, one thread. They are a regression tripwire for these entry points, not their cost under contention (see *What the arms do not measure*) |
//!
//! # What the arms do not measure
//!
//! One thread on an uncontended map. None of the following runs, so none is
//! in the counts:
//!
//! - a retry or backoff loop, a version-lock conflict, a closed writer gate;
//! - contention on reader registration: `update` reads through `get`, which
//!   registers and drops a throwaway epoch reader on every call, taking the
//!   collector's reader mutex twice. An instruction count prices a locked
//!   instruction as one instruction;
//! - more than one writer slot in a quiesce drain. `sync_bytesmap_cas_remove`
//!   takes the serialised path on every operation (a bucket's last entry),
//!   which stops every writer; here there is only one;
//! - reclamation: the maps are leaked and nothing drains, and
//!   `sync_blobmap_cas` appends 50,000 records to an arena that only grows.
//!
//! The success assertion shows that every operation returned success. It does
//! not show which path returned it: each entry point also succeeds through
//! its serialised fallback. `tests/test_rmw_paths.rs` (feature `occ-stats`)
//! runs the same operations and asserts the fallback counter.
#![allow(missing_docs)]

use expanse_trie::sync::{
    SyncExpanseBlobMap, SyncExpanseBytesMap, SyncExpanseMap, SyncExpanseStrMap,
};
#[cfg(target_os = "linux")]
use iai_callgrind::main;
use iai_callgrind::{
    Callgrind, LibraryBenchmarkConfig, library_benchmark, library_benchmark_group,
};
use std::hint::black_box;

/// Wraps a key for `SyncExpanseStrMap`.
///
/// `new_unchecked`: the validating constructor would put a whole-key scan
/// inside the measured region. Every generator in this file emits ASCII
/// route-shaped keys with no NUL.
#[inline(always)]
fn tk<B: AsRef<[u8]> + ?Sized>(bytes: &B) -> &expanse_trie::strmap::NulFreeStr {
    // SAFETY: the generator in this file emits no NUL bytes.
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

/// Population per arm. Well above `ROOT_LEAF_CAP`, small enough that
/// callgrind (~50x slowdown) stays practical.
const POP: usize = 50_000;
const _: () = assert!(POP > expanse_trie::types::ROOT_LEAF_CAP);

/// Deterministic hasher for the bytes map: `RandomState`'s per-process seed
/// would make bucket placement, and so instruction counts, depend on the run.
type DetHasher = std::hash::BuildHasherDefault<std::collections::hash_map::DefaultHasher>;

fn keys() -> Vec<u64> {
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    (0..POP).map(|_| rng.next()).collect()
}

/// Route-shaped string keys (~40 bytes, long shared prefixes).
fn str_keys() -> Vec<Vec<u8>> {
    (0..POP)
        .map(|i| format!("/api/v2/tenants/{:06}/resources/{:04}", i / 16, i % 16).into_bytes())
        .collect()
}

/// A probe order that is not the build order.
fn shuffled<T>(mut probes: Vec<T>) -> Vec<T> {
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    probes
}

/// The value a map holds for `k`: key-derived, so a probe knows it.
fn value_of(k: u64) -> u64 {
    !k
}

/// The blob payload for `k`: 16 key-derived bytes, above the 7-byte inline
/// limit, so the value lives in the arena.
fn blob_payload(k: u64) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&k.to_le_bytes());
    out[8..].copy_from_slice(&(!k).to_le_bytes());
    out
}

/// The blob 24-bit metadata for `k`; never zero, so no store takes the
/// metadata-free compressed-inline path.
fn blob_meta(k: u64) -> u32 {
    (k as u32 & 0x00FF_FFFF) | 1
}

fn built_sync_map(_dist: &str) -> (SyncExpanseMap, Vec<u64>) {
    let ks = keys();
    let map = SyncExpanseMap::new();
    for &k in &ks {
        map.insert(k, value_of(k));
    }
    (map, shuffled(ks))
}

fn built_sync_blobmap(_dist: &str) -> (SyncExpanseBlobMap, Vec<u64>) {
    let ks = keys();
    let map = SyncExpanseBlobMap::new();
    for &k in &ks {
        map.insert(k, &blob_payload(k), blob_meta(k))
            .expect("blob insert");
    }
    (map, shuffled(ks))
}

/// Each string key paired with the value the map holds for it.
fn built_sync_strmap(_dist: &str) -> (SyncExpanseStrMap, Vec<(Vec<u8>, u64)>) {
    let map = SyncExpanseStrMap::new();
    let mut pairs = Vec::with_capacity(POP);
    for (i, k) in str_keys().into_iter().enumerate() {
        map.insert(tk(&k), i as u64);
        pairs.push((k, i as u64));
    }
    (map, shuffled(pairs))
}

fn built_sync_bytesmap(_dist: &str) -> (SyncExpanseBytesMap<DetHasher>, Vec<(Vec<u8>, u64)>) {
    let map = SyncExpanseBytesMap::with_hasher(DetHasher::default());
    let mut pairs = Vec::with_capacity(POP);
    for (i, k) in str_keys().into_iter().enumerate() {
        map.insert(&k, i as u64);
        pairs.push((k, i as u64));
    }
    (map, shuffled(pairs))
}

// `SyncExpanseMap::compare_exchange` (sync.rs:6573), success path: the
// expected word is the one the key holds, so one optimistic descent stores in
// place.
#[inline(always)]
fn sync_map_cas_body(built: (SyncExpanseMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let mut won = 0u64;
    for &k in &probes {
        let cur = value_of(black_box(k));
        won += u64::from(
            map.compare_exchange(black_box(k), Some(cur), Some(cur.wrapping_add(1)))
                .is_ok(),
        );
    }
    // Teardown stays out of the region: neither the map nor the probe
    // stream (50,000 heap keys on the string arms) is dropped here.
    core::mem::forget(map);
    core::mem::forget(probes);
    // Every operation must take the path the arm is named for: a failed
    // exchange or a missed update is a different, cheaper path.
    assert_eq!(won, POP as u64);
    black_box(won)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_map)]
fn sync_map_cas(built: (SyncExpanseMap, Vec<u64>)) -> u64 {
    sync_map_cas_body(built)
}

// `SyncExpanseMap::update` (sync.rs:6602): a validated read, then
// `compare_exchange` over the word it returned.
#[inline(always)]
fn sync_map_update_rmw_body(built: (SyncExpanseMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let mut hits = 0u64;
    for &k in &probes {
        hits += u64::from(
            map.update(black_box(k), |v| v.map(|x| x.wrapping_add(1)))
                .is_some(),
        );
    }
    core::mem::forget(map);
    core::mem::forget(probes);
    // Every operation must take the path the arm is named for: a failed
    // exchange or a missed update is a different, cheaper path.
    assert_eq!(hits, POP as u64);
    black_box(hits)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_map)]
fn sync_map_update_rmw(built: (SyncExpanseMap, Vec<u64>)) -> u64 {
    sync_map_update_rmw_body(built)
}

// `SyncExpanseStrMap::compare_exchange` (sync.rs:12737), success path.
#[inline(always)]
fn sync_strmap_cas_body(built: (SyncExpanseStrMap, Vec<(Vec<u8>, u64)>)) -> u64 {
    let (map, probes) = built;
    let mut won = 0u64;
    for (k, v) in &probes {
        let cur = black_box(*v);
        won += u64::from(
            map.compare_exchange(black_box(tk(k)), Some(cur), Some(cur.wrapping_add(1)))
                .is_ok(),
        );
    }
    core::mem::forget(map);
    core::mem::forget(probes);
    // Every operation must take the path the arm is named for: a failed
    // exchange or a missed update is a different, cheaper path.
    assert_eq!(won, POP as u64);
    black_box(won)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_sync_strmap)]
fn sync_strmap_cas(built: (SyncExpanseStrMap, Vec<(Vec<u8>, u64)>)) -> u64 {
    sync_strmap_cas_body(built)
}

// `SyncExpanseStrMap::update` (sync.rs:12765).
#[inline(always)]
fn sync_strmap_update_rmw_body(built: (SyncExpanseStrMap, Vec<(Vec<u8>, u64)>)) -> u64 {
    let (map, probes) = built;
    let mut hits = 0u64;
    for (k, _) in &probes {
        hits += u64::from(
            map.update(black_box(tk(k)), |v| v.map(|x| x.wrapping_add(1)))
                .is_some(),
        );
    }
    core::mem::forget(map);
    core::mem::forget(probes);
    // Every operation must take the path the arm is named for: a failed
    // exchange or a missed update is a different, cheaper path.
    assert_eq!(hits, POP as u64);
    black_box(hits)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_sync_strmap)]
fn sync_strmap_update_rmw(built: (SyncExpanseStrMap, Vec<(Vec<u8>, u64)>)) -> u64 {
    sync_strmap_update_rmw_body(built)
}

// `SyncExpanseBytesMap::compare_exchange` (sync.rs:13813), value to value.
#[inline(always)]
fn sync_bytesmap_cas_body(built: (SyncExpanseBytesMap<DetHasher>, Vec<(Vec<u8>, u64)>)) -> u64 {
    let (map, probes) = built;
    let mut won = 0u64;
    for (k, v) in &probes {
        let cur = black_box(*v);
        won += u64::from(
            map.compare_exchange(black_box(k), Some(cur), Some(cur.wrapping_add(1)))
                .is_ok(),
        );
    }
    core::mem::forget(map);
    core::mem::forget(probes);
    // Every operation must take the path the arm is named for: a failed
    // exchange or a missed update is a different, cheaper path.
    assert_eq!(won, POP as u64);
    black_box(won)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_sync_bytesmap)]
fn sync_bytesmap_cas(built: (SyncExpanseBytesMap<DetHasher>, Vec<(Vec<u8>, u64)>)) -> u64 {
    sync_bytesmap_cas_body(built)
}

// `SyncExpanseBytesMap::update` (sync.rs:13841).
#[inline(always)]
fn sync_bytesmap_update_rmw_body(
    built: (SyncExpanseBytesMap<DetHasher>, Vec<(Vec<u8>, u64)>),
) -> u64 {
    let (map, probes) = built;
    let mut hits = 0u64;
    for (k, _) in &probes {
        hits += u64::from(
            map.update(black_box(k), |v| v.map(|x| x.wrapping_add(1)))
                .is_some(),
        );
    }
    core::mem::forget(map);
    core::mem::forget(probes);
    // Every operation must take the path the arm is named for: a failed
    // exchange or a missed update is a different, cheaper path.
    assert_eq!(hits, POP as u64);
    black_box(hits)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_sync_bytesmap)]
fn sync_bytesmap_update_rmw(built: (SyncExpanseBytesMap<DetHasher>, Vec<(Vec<u8>, u64)>)) -> u64 {
    sync_bytesmap_update_rmw_body(built)
}

// `SyncExpanseBytesMap::compare_exchange` (sync.rs:13813), value to absent:
// the key is the last (here, only) entry of its bucket, so the exchange
// removes the bucket, the serialised path #1381 introduced. The 50k keys hash
// to distinct 64-bit values, so every bucket holds one entry.
#[inline(always)]
fn sync_bytesmap_cas_remove_body(
    built: (SyncExpanseBytesMap<DetHasher>, Vec<(Vec<u8>, u64)>),
) -> u64 {
    let (map, probes) = built;
    let mut won = 0u64;
    for (k, v) in &probes {
        won += u64::from(
            map.compare_exchange(black_box(k), Some(black_box(*v)), None)
                .is_ok(),
        );
    }
    core::mem::forget(map);
    core::mem::forget(probes);
    // Every operation must take the path the arm is named for: a failed
    // exchange or a missed update is a different, cheaper path.
    assert_eq!(won, POP as u64);
    black_box(won)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_sync_bytesmap)]
fn sync_bytesmap_cas_remove(built: (SyncExpanseBytesMap<DetHasher>, Vec<(Vec<u8>, u64)>)) -> u64 {
    sync_bytesmap_cas_remove_body(built)
}

// `SyncExpanseBlobMap::compare_exchange` (sync.rs:11590), success path: the
// expected payload and metadata are the ones the key holds; the new ones are
// 16 bytes with non-zero metadata, so the store goes through the arena.
// `SyncExpanseBlobMap` has no `update`.
#[inline(always)]
fn sync_blobmap_cas_body(built: (SyncExpanseBlobMap, Vec<u64>)) -> u64 {
    let (map, probes) = built;
    let mut won = 0u64;
    for &k in &probes {
        let old = blob_payload(k);
        let new = blob_payload(!k);
        won += u64::from(
            map.compare_exchange(
                black_box(k),
                Some((black_box(old.as_slice()), blob_meta(k))),
                Some((black_box(new.as_slice()), blob_meta(!k))),
            )
            .is_ok(),
        );
    }
    core::mem::forget(map);
    core::mem::forget(probes);
    // Every operation must take the path the arm is named for: a failed
    // exchange or a missed update is a different, cheaper path.
    assert_eq!(won, POP as u64);
    black_box(won)
}

#[library_benchmark]
#[bench::random(args = ("random",), setup = built_sync_blobmap)]
fn sync_blobmap_cas(built: (SyncExpanseBlobMap, Vec<u64>)) -> u64 {
    sync_blobmap_cas_body(built)
}

/// `--cache-sim=yes`, as `instructions.rs`: the same columns, so the report
/// reads both binaries alike. Branch simulation is not requested.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn bench_config() -> LibraryBenchmarkConfig {
    let mut config = LibraryBenchmarkConfig::default();
    config.tool(Callgrind::with_args(["--cache-sim=yes"]));
    config
}

library_benchmark_group!(
    name = rmw;
    benchmarks =
        sync_map_cas,
        sync_map_update_rmw,
        sync_strmap_cas,
        sync_strmap_update_rmw,
        sync_bytesmap_cas,
        sync_bytesmap_update_rmw,
        sync_bytesmap_cas_remove,
        sync_blobmap_cas
);

#[cfg(target_os = "linux")]
main!(config = bench_config(); library_benchmark_groups = rmw);

/// Without valgrind, run every arm once and check the count it returns: each
/// exchange succeeds and each update finds its key, so every arm returns
/// `POP`. Not a measurement.
#[cfg(not(target_os = "linux"))]
fn main() {
    // `#[library_benchmark]` turns each arm into a module, so the bodies are
    // called here. Each asserts its own count.
    sync_map_cas_body(built_sync_map("random"));
    sync_map_update_rmw_body(built_sync_map("random"));
    sync_strmap_cas_body(built_sync_strmap("routes"));
    sync_strmap_update_rmw_body(built_sync_strmap("routes"));
    sync_bytesmap_cas_body(built_sync_bytesmap("routes"));
    sync_bytesmap_update_rmw_body(built_sync_bytesmap("routes"));
    sync_bytesmap_cas_remove_body(built_sync_bytesmap("routes"));
    sync_blobmap_cas_body(built_sync_blobmap("random"));
    println!(
        "rmw_instructions: iai-callgrind arms run on Linux only; \
         all 8 arms ran once and met their counts (smoke check, not a measurement)."
    );
}
