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
//! | `population` | 50k |
//! | `probes_and_reuse` | 50k (shuffled), reuse 1.0 |
//! | `hit_rate` | 100% |
//! | `miss_gen_method` | None |
//! | `value_dereference` | `black_box` on retrieved values |
//! | `measured_region` | Clean (setup in setup) |
//! | `arm_symmetry` | Internal trie paths |
//! | `statistics` | iai Callgrind exact counts |
//! | `verdict` | **PASS** `[verified: RUN (CI instruction-counts)]`: Canonical instruction reference. |

// The `library_benchmark` macro expands to modules that carry no docs of
// their own; the workspace `missing_docs` lint does not apply to a bench
// harness.
#![allow(missing_docs)]

use expanse_trie::bytesmap::ExpanseBytesMap;
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use expanse_trie::strmap::ExpanseStrMap;
use expanse_trie::{ExpanseBlobMap32, ExpanseMap32, ExpanseSet32, Key32, Value32};
#[cfg(target_os = "linux")]
use iai_callgrind::main;
use iai_callgrind::{
    Callgrind, LibraryBenchmarkConfig, library_benchmark, library_benchmark_group,
};
use std::hint::black_box;

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
        other => panic!("unknown distribution {other}"),
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
        map.insert(k, i as u64);
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
        map.insert(black_box(k), black_box(i as u64));
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
        sink ^= map.get(black_box(k)).unwrap_or(0);
    }
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
        sink ^= map.insert(black_box(k), black_box(7)).unwrap_or(0);
        sink ^= map.remove(black_box(k)).unwrap_or(0);
        map.insert(black_box(k), black_box(9));
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
        map_iterate,
        map_nav,
        set32_insert,
        map32_insert,
        map32_get,
        map32_iterate,
        map32_range,
        map32_for_each_range,
        map32_remove,
        set32_remove,
        blobmap32_scan,
        strmap_insert,
        strmap_get,
        strmap_churn,
        bytesmap_insert,
        bytesmap_get,
        bytesmap_churn
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
