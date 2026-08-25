//! Standalone instant comparative lookup & iteration benchmark harness.
//!
//! Evaluates [`ExpanseMap`] and [`ExpanseSet`] directly against:
//! - [`hashbrown::HashMap`] / [`hashbrown::HashSet`] (SwissTable SIMD hash map)
//! - [`std::collections::BTreeMap`] / [`std::collections::BTreeSet`] (Standard B-tree)
//!
//! # Key Distributions
//! - `sequential`: monotonic contiguous keys `0..N`
//! - `random`: 64-bit uniform pseudorandom keys
//! - `clustered`: dense 256-key runs sharing random 56-bit prefixes
//!
//! # Metrics Measured
//! - Point lookup hit latency (`ns/op`) and throughput (`Mops/s`)
//! - Point lookup miss latency (`ns/op`) and throughput (`Mops/s`)
//! - Full iteration latency (`ms`) and throughput (`Mops/s`)
//! - Memory footprint (`bytes/key`)
//!
//! # Usage
//! ```bash
//! cargo run --release -p expanse-trie --example bench_lookup_compare
//! cargo run --release -p expanse-trie --example bench_lookup_compare -- --quick
//! cargo run --release -p expanse-trie --example bench_lookup_compare -- --pop 500000
//! cargo run --release -p expanse-trie --example bench_lookup_compare -- --set
//! ```

use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use hashbrown::{HashMap as HashBrownMap, HashSet as HashBrownSet};
use std::collections::{BTreeMap, BTreeSet};
use std::hint::black_box;
use std::time::Instant;

/// Fast deterministic 64-bit XorShift pseudorandom number generator.
#[derive(Clone, Copy, Debug)]
struct XorShift(u64);

impl XorShift {
    #[inline(always)]
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0x5CA1_AB1E_0001 } else { seed })
    }

    #[inline(always)]
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// Generates $N$ distinct 64-bit keys for the specified distribution class.
fn generate_keys(dist: &str, n: usize, seed: u64) -> Vec<u64> {
    let mut rng = XorShift::new(seed);
    let mut out = Vec::with_capacity(n);
    match dist {
        "sequential" => {
            out.extend(0..n as u64);
        }
        "random" => {
            out.extend((0..n).map(|_| rng.next()));
        }
        "clustered" => {
            let mut base = 0u64;
            for i in 0..n as u64 {
                if i % 256 == 0 {
                    base = rng.next() & !0xFF;
                }
                out.push(base + (i % 256));
            }
        }
        "sparse" => {
            out.extend((0..n as u64).map(|i| i << 40));
        }
        other => panic!("unknown distribution '{other}'"),
    }
    out
}

/// Generates hit and miss probe key sequences.
fn generate_probes(keys: &[u64], probe_count: usize, seed: u64) -> (Vec<u64>, Vec<u64>) {
    let mut rng = XorShift::new(seed);
    let n = keys.len();
    let mut hits = Vec::with_capacity(probe_count);
    let mut misses = Vec::with_capacity(probe_count);

    for _ in 0..probe_count {
        let idx = (rng.next() as usize) % n;
        let hit_k = keys[idx];
        hits.push(hit_k);

        // Perturb the key with high-bit flip and mask to guarantee misses
        let miss_k = hit_k ^ (1u64 << 63) ^ 0x5A5A_5A5A_5A5A_5A5Au64;
        misses.push(miss_k);
    }
    (hits, misses)
}

/// Structural memory estimator for `hashbrown::HashMap<u64, u64>`.
///
/// HashBrown uses a SwissTable layout:
/// - Control bytes array: `capacity + 16` bytes (Group size = 16)
/// - Bucket array: `capacity * (size_of::<u64>() + size_of::<u64>())`
/// - Base struct size: `size_of::<HashMap>()`
fn estimate_hashbrown_map_mem<K, V>(map: &HashBrownMap<K, V>) -> usize {
    let cap = map.capacity();
    if cap == 0 {
        return std::mem::size_of::<HashBrownMap<K, V>>();
    }
    let ctrl_bytes = cap + 16;
    let slot_bytes = cap * std::mem::size_of::<(K, V)>();
    std::mem::size_of::<HashBrownMap<K, V>>() + ctrl_bytes + slot_bytes
}

/// Structural memory estimator for `hashbrown::HashSet<u64>`.
fn estimate_hashbrown_set_mem<T>(set: &HashBrownSet<T>) -> usize {
    let cap = set.capacity();
    if cap == 0 {
        return std::mem::size_of::<HashBrownSet<T>>();
    }
    let ctrl_bytes = cap + 16;
    let slot_bytes = cap * std::mem::size_of::<T>();
    std::mem::size_of::<HashBrownSet<T>>() + ctrl_bytes + slot_bytes
}

/// Structural memory estimator for `std::collections::BTreeMap<u64, u64>`.
///
/// Rust standard library BTreeMap uses B=6 (max 11 elements per leaf/node).
/// Average occupancy is ~70-75% (~8 entries per node).
/// - Leaf node: header (16B) + 11 * (8B key + 8B val) = 192 bytes
/// - Internal node: header (16B) + 11 * (8B key + 8B val) + 12 * 8B child ptrs = 288 bytes
fn estimate_btreemap_mem(n: usize) -> usize {
    if n == 0 {
        return std::mem::size_of::<BTreeMap<u64, u64>>();
    }
    let avg_entries = 8.0f64;
    let num_leaves = (n as f64 / avg_entries).ceil();
    let mut total_nodes = num_leaves;
    let mut cur_level = num_leaves;
    while cur_level > 1.0 {
        cur_level = (cur_level / avg_entries).ceil();
        total_nodes += cur_level;
    }
    let internal_nodes = total_nodes - num_leaves;
    let heap_bytes = (num_leaves * 192.0 + internal_nodes * 288.0) as usize;
    std::mem::size_of::<BTreeMap<u64, u64>>() + heap_bytes
}

/// Structural memory estimator for `std::collections::BTreeSet<u64>`.
fn estimate_btreeset_mem(n: usize) -> usize {
    if n == 0 {
        return std::mem::size_of::<BTreeSet<u64>>();
    }
    let avg_entries = 8.0f64;
    let num_leaves = (n as f64 / avg_entries).ceil();
    let mut total_nodes = num_leaves;
    let mut cur_level = num_leaves;
    while cur_level > 1.0 {
        cur_level = (cur_level / avg_entries).ceil();
        total_nodes += cur_level;
    }
    let internal_nodes = total_nodes - num_leaves;
    // BTreeSet leaf: 16B header + 11 * 8B key = 104 bytes
    // BTreeSet internal: 16B header + 11 * 8B key + 12 * 8B child ptrs = 200 bytes
    let heap_bytes = (num_leaves * 104.0 + internal_nodes * 200.0) as usize;
    std::mem::size_of::<BTreeSet<u64>>() + heap_bytes
}

/// Benchmark measurement results for a container.
#[derive(Clone, Debug)]
struct ContainerResult {
    name: &'static str,
    hit_latency_ns: f64,
    hit_throughput_mops: f64,
    miss_latency_ns: f64,
    miss_throughput_mops: f64,
    iter_latency_ms: f64,
    iter_throughput_mops: f64,
    bytes_per_key: f64,
    is_ordered_iter: bool,
}

/// Benchmarks `ExpanseMap`.
fn bench_expanse_map(keys: &[u64], hit_probes: &[u64], miss_probes: &[u64]) -> ContainerResult {
    let mut map = ExpanseMap::new();
    for &k in keys {
        map.insert(k, !k);
    }
    let mem_used = map.mem_used();
    let bytes_per_key = mem_used as f64 / map.len().max(1) as f64;

    // 1. Point lookup (hit)
    let mut hit_sink = 0u64;
    let t0 = Instant::now();
    for &k in hit_probes {
        if let Some(v) = map.get(black_box(k)) {
            hit_sink = hit_sink.wrapping_add(black_box(v));
        }
    }
    let hit_elapsed = t0.elapsed();
    black_box(hit_sink);

    let hit_ns = hit_elapsed.as_nanos() as f64 / hit_probes.len() as f64;
    let hit_mops = (hit_probes.len() as f64 / hit_elapsed.as_secs_f64()) / 1_000_000.0;

    // 2. Point lookup (miss)
    let mut miss_sink = 0u64;
    let t0 = Instant::now();
    for &k in miss_probes {
        if let Some(v) = map.get(black_box(k)) {
            miss_sink = miss_sink.wrapping_add(black_box(v));
        }
    }
    let miss_elapsed = t0.elapsed();
    black_box(miss_sink);

    let miss_ns = miss_elapsed.as_nanos() as f64 / miss_probes.len() as f64;
    let miss_mops = (miss_probes.len() as f64 / miss_elapsed.as_secs_f64()) / 1_000_000.0;

    // 3. Full ordered iteration
    let mut iter_count = 0usize;
    let mut iter_checksum = 0u64;
    let t0 = Instant::now();
    for (k, v) in map.iter() {
        iter_checksum = iter_checksum.wrapping_add(black_box(k) ^ black_box(v));
        iter_count += 1;
    }
    let iter_elapsed = t0.elapsed();
    black_box((iter_count, iter_checksum));

    let iter_ms = iter_elapsed.as_secs_f64() * 1000.0;
    let iter_mops = (iter_count as f64 / iter_elapsed.as_secs_f64()) / 1_000_000.0;

    ContainerResult {
        name: "ExpanseMap",
        hit_latency_ns: hit_ns,
        hit_throughput_mops: hit_mops,
        miss_latency_ns: miss_ns,
        miss_throughput_mops: miss_mops,
        iter_latency_ms: iter_ms,
        iter_throughput_mops: iter_mops,
        bytes_per_key,
        is_ordered_iter: true,
    }
}

/// Benchmarks `hashbrown::HashMap`.
fn bench_hashbrown_map(keys: &[u64], hit_probes: &[u64], miss_probes: &[u64]) -> ContainerResult {
    let mut map = HashBrownMap::new();
    for &k in keys {
        map.insert(k, !k);
    }
    let total_bytes = estimate_hashbrown_map_mem(&map);
    let bytes_per_key = total_bytes as f64 / map.len().max(1) as f64;

    // 1. Point lookup (hit)
    let mut hit_sink = 0u64;
    let t0 = Instant::now();
    for &k in hit_probes {
        if let Some(&v) = map.get(&black_box(k)) {
            hit_sink = hit_sink.wrapping_add(black_box(v));
        }
    }
    let hit_elapsed = t0.elapsed();
    black_box(hit_sink);

    let hit_ns = hit_elapsed.as_nanos() as f64 / hit_probes.len() as f64;
    let hit_mops = (hit_probes.len() as f64 / hit_elapsed.as_secs_f64()) / 1_000_000.0;

    // 2. Point lookup (miss)
    let mut miss_sink = 0u64;
    let t0 = Instant::now();
    for &k in miss_probes {
        if let Some(&v) = map.get(&black_box(k)) {
            miss_sink = miss_sink.wrapping_add(black_box(v));
        }
    }
    let miss_elapsed = t0.elapsed();
    black_box(miss_sink);

    let miss_ns = miss_elapsed.as_nanos() as f64 / miss_probes.len() as f64;
    let miss_mops = (miss_probes.len() as f64 / miss_elapsed.as_secs_f64()) / 1_000_000.0;

    // 3. Full iteration (unordered)
    let mut iter_count = 0usize;
    let mut iter_checksum = 0u64;
    let t0 = Instant::now();
    for (&k, &v) in map.iter() {
        iter_checksum = iter_checksum.wrapping_add(black_box(k) ^ black_box(v));
        iter_count += 1;
    }
    let iter_elapsed = t0.elapsed();
    black_box((iter_count, iter_checksum));

    let iter_ms = iter_elapsed.as_secs_f64() * 1000.0;
    let iter_mops = (iter_count as f64 / iter_elapsed.as_secs_f64()) / 1_000_000.0;

    ContainerResult {
        name: "hashbrown::HashMap",
        hit_latency_ns: hit_ns,
        hit_throughput_mops: hit_mops,
        miss_latency_ns: miss_ns,
        miss_throughput_mops: miss_mops,
        iter_latency_ms: iter_ms,
        iter_throughput_mops: iter_mops,
        bytes_per_key,
        is_ordered_iter: false,
    }
}

/// Benchmarks `std::collections::BTreeMap`.
fn bench_btreemap(keys: &[u64], hit_probes: &[u64], miss_probes: &[u64]) -> ContainerResult {
    let mut map = BTreeMap::new();
    for &k in keys {
        map.insert(k, !k);
    }
    let total_bytes = estimate_btreemap_mem(map.len());
    let bytes_per_key = total_bytes as f64 / map.len().max(1) as f64;

    // 1. Point lookup (hit)
    let mut hit_sink = 0u64;
    let t0 = Instant::now();
    for &k in hit_probes {
        if let Some(&v) = map.get(&black_box(k)) {
            hit_sink = hit_sink.wrapping_add(black_box(v));
        }
    }
    let hit_elapsed = t0.elapsed();
    black_box(hit_sink);

    let hit_ns = hit_elapsed.as_nanos() as f64 / hit_probes.len() as f64;
    let hit_mops = (hit_probes.len() as f64 / hit_elapsed.as_secs_f64()) / 1_000_000.0;

    // 2. Point lookup (miss)
    let mut miss_sink = 0u64;
    let t0 = Instant::now();
    for &k in miss_probes {
        if let Some(&v) = map.get(&black_box(k)) {
            miss_sink = miss_sink.wrapping_add(black_box(v));
        }
    }
    let miss_elapsed = t0.elapsed();
    black_box(miss_sink);

    let miss_ns = miss_elapsed.as_nanos() as f64 / miss_probes.len() as f64;
    let miss_mops = (miss_probes.len() as f64 / miss_elapsed.as_secs_f64()) / 1_000_000.0;

    // 3. Full ordered iteration
    let mut iter_count = 0usize;
    let mut iter_checksum = 0u64;
    let t0 = Instant::now();
    for (&k, &v) in map.iter() {
        iter_checksum = iter_checksum.wrapping_add(black_box(k) ^ black_box(v));
        iter_count += 1;
    }
    let iter_elapsed = t0.elapsed();
    black_box((iter_count, iter_checksum));

    let iter_ms = iter_elapsed.as_secs_f64() * 1000.0;
    let iter_mops = (iter_count as f64 / iter_elapsed.as_secs_f64()) / 1_000_000.0;

    ContainerResult {
        name: "std::BTreeMap",
        hit_latency_ns: hit_ns,
        hit_throughput_mops: hit_mops,
        miss_latency_ns: miss_ns,
        miss_throughput_mops: miss_mops,
        iter_latency_ms: iter_ms,
        iter_throughput_mops: iter_mops,
        bytes_per_key,
        is_ordered_iter: true,
    }
}

/// Benchmarks `ExpanseSet`.
fn bench_expanse_set(keys: &[u64], hit_probes: &[u64], miss_probes: &[u64]) -> ContainerResult {
    let mut set = ExpanseSet::new();
    for &k in keys {
        set.insert(k);
    }
    let mem_used = set.mem_used();
    let bytes_per_key = mem_used as f64 / set.len().max(1) as f64;

    // 1. Point lookup (hit)
    let mut hit_hits = 0usize;
    let t0 = Instant::now();
    for &k in hit_probes {
        if set.contains(black_box(k)) {
            hit_hits += 1;
        }
    }
    let hit_elapsed = t0.elapsed();
    black_box(hit_hits);

    let hit_ns = hit_elapsed.as_nanos() as f64 / hit_probes.len() as f64;
    let hit_mops = (hit_probes.len() as f64 / hit_elapsed.as_secs_f64()) / 1_000_000.0;

    // 2. Point lookup (miss)
    let mut miss_hits = 0usize;
    let t0 = Instant::now();
    for &k in miss_probes {
        if set.contains(black_box(k)) {
            miss_hits += 1;
        }
    }
    let miss_elapsed = t0.elapsed();
    black_box(miss_hits);

    let miss_ns = miss_elapsed.as_nanos() as f64 / miss_probes.len() as f64;
    let miss_mops = (miss_probes.len() as f64 / miss_elapsed.as_secs_f64()) / 1_000_000.0;

    // 3. Full ordered iteration
    let mut iter_count = 0usize;
    let mut iter_checksum = 0u64;
    let t0 = Instant::now();
    for k in set.iter() {
        iter_checksum = iter_checksum.wrapping_add(black_box(k));
        iter_count += 1;
    }
    let iter_elapsed = t0.elapsed();
    black_box((iter_count, iter_checksum));

    let iter_ms = iter_elapsed.as_secs_f64() * 1000.0;
    let iter_mops = (iter_count as f64 / iter_elapsed.as_secs_f64()) / 1_000_000.0;

    ContainerResult {
        name: "ExpanseSet",
        hit_latency_ns: hit_ns,
        hit_throughput_mops: hit_mops,
        miss_latency_ns: miss_ns,
        miss_throughput_mops: miss_mops,
        iter_latency_ms: iter_ms,
        iter_throughput_mops: iter_mops,
        bytes_per_key,
        is_ordered_iter: true,
    }
}

/// Benchmarks `hashbrown::HashSet`.
fn bench_hashbrown_set(keys: &[u64], hit_probes: &[u64], miss_probes: &[u64]) -> ContainerResult {
    let mut set = HashBrownSet::new();
    for &k in keys {
        set.insert(k);
    }
    let total_bytes = estimate_hashbrown_set_mem(&set);
    let bytes_per_key = total_bytes as f64 / set.len().max(1) as f64;

    // 1. Point lookup (hit)
    let mut hit_hits = 0usize;
    let t0 = Instant::now();
    for &k in hit_probes {
        if set.contains(&black_box(k)) {
            hit_hits += 1;
        }
    }
    let hit_elapsed = t0.elapsed();
    black_box(hit_hits);

    let hit_ns = hit_elapsed.as_nanos() as f64 / hit_probes.len() as f64;
    let hit_mops = (hit_probes.len() as f64 / hit_elapsed.as_secs_f64()) / 1_000_000.0;

    // 2. Point lookup (miss)
    let mut miss_hits = 0usize;
    let t0 = Instant::now();
    for &k in miss_probes {
        if set.contains(&black_box(k)) {
            miss_hits += 1;
        }
    }
    let miss_elapsed = t0.elapsed();
    black_box(miss_hits);

    let miss_ns = miss_elapsed.as_nanos() as f64 / miss_probes.len() as f64;
    let miss_mops = (miss_probes.len() as f64 / miss_elapsed.as_secs_f64()) / 1_000_000.0;

    // 3. Full iteration (unordered)
    let mut iter_count = 0usize;
    let mut iter_checksum = 0u64;
    let t0 = Instant::now();
    for &k in set.iter() {
        iter_checksum = iter_checksum.wrapping_add(black_box(k));
        iter_count += 1;
    }
    let iter_elapsed = t0.elapsed();
    black_box((iter_count, iter_checksum));

    let iter_ms = iter_elapsed.as_secs_f64() * 1000.0;
    let iter_mops = (iter_count as f64 / iter_elapsed.as_secs_f64()) / 1_000_000.0;

    ContainerResult {
        name: "hashbrown::HashSet",
        hit_latency_ns: hit_ns,
        hit_throughput_mops: hit_mops,
        miss_latency_ns: miss_ns,
        miss_throughput_mops: miss_mops,
        iter_latency_ms: iter_ms,
        iter_throughput_mops: iter_mops,
        bytes_per_key,
        is_ordered_iter: false,
    }
}

/// Benchmarks `std::collections::BTreeSet`.
fn bench_btreeset(keys: &[u64], hit_probes: &[u64], miss_probes: &[u64]) -> ContainerResult {
    let mut set = BTreeSet::new();
    for &k in keys {
        set.insert(k);
    }
    let total_bytes = estimate_btreeset_mem(set.len());
    let bytes_per_key = total_bytes as f64 / set.len().max(1) as f64;

    // 1. Point lookup (hit)
    let mut hit_hits = 0usize;
    let t0 = Instant::now();
    for &k in hit_probes {
        if set.contains(&black_box(k)) {
            hit_hits += 1;
        }
    }
    let hit_elapsed = t0.elapsed();
    black_box(hit_hits);

    let hit_ns = hit_elapsed.as_nanos() as f64 / hit_probes.len() as f64;
    let hit_mops = (hit_probes.len() as f64 / hit_elapsed.as_secs_f64()) / 1_000_000.0;

    // 2. Point lookup (miss)
    let mut miss_hits = 0usize;
    let t0 = Instant::now();
    for &k in miss_probes {
        if set.contains(&black_box(k)) {
            miss_hits += 1;
        }
    }
    let miss_elapsed = t0.elapsed();
    black_box(miss_hits);

    let miss_ns = miss_elapsed.as_nanos() as f64 / miss_probes.len() as f64;
    let miss_mops = (miss_probes.len() as f64 / miss_elapsed.as_secs_f64()) / 1_000_000.0;

    // 3. Full ordered iteration
    let mut iter_count = 0usize;
    let mut iter_checksum = 0u64;
    let t0 = Instant::now();
    for &k in set.iter() {
        iter_checksum = iter_checksum.wrapping_add(black_box(k));
        iter_count += 1;
    }
    let iter_elapsed = t0.elapsed();
    black_box((iter_count, iter_checksum));

    let iter_ms = iter_elapsed.as_secs_f64() * 1000.0;
    let iter_mops = (iter_count as f64 / iter_elapsed.as_secs_f64()) / 1_000_000.0;

    ContainerResult {
        name: "std::BTreeSet",
        hit_latency_ns: hit_ns,
        hit_throughput_mops: hit_mops,
        miss_latency_ns: miss_ns,
        miss_throughput_mops: miss_mops,
        iter_latency_ms: iter_ms,
        iter_throughput_mops: iter_mops,
        bytes_per_key,
        is_ordered_iter: true,
    }
}

/// Formats a relative multiplier comparison.
fn format_multiplier(val_a: f64, val_b: f64, higher_is_better: bool) -> String {
    if val_a <= 0.0 || val_b <= 0.0 {
        return "N/A".into();
    }
    let ratio = if higher_is_better {
        val_a / val_b
    } else {
        val_b / val_a
    };

    if ratio >= 1.0 {
        format!("{ratio:.2}x faster")
    } else {
        format!("{:.2}x slower", 1.0 / ratio)
    }
}

/// Formats a memory ratio comparison.
fn format_mem_ratio(expanse_b: f64, other_b: f64) -> String {
    if expanse_b <= 0.0 || other_b <= 0.0 {
        return "N/A".into();
    }
    if expanse_b <= other_b {
        format!("{:.2}x smaller", other_b / expanse_b)
    } else {
        format!("{:.2}x larger", expanse_b / other_b)
    }
}

/// Prints formatted results table and comparative multipliers for a single distribution.
fn print_distribution_report(dist: &str, pop: usize, results: &[ContainerResult]) {
    println!(
        "\n--- Key Distribution: {:<12} (N = {}) -----------------------------------------------------------------",
        dist, pop
    );
    println!(
        "{:<20} | {:>11} | {:>12} | {:>12} | {:>12} | {:>12} | {:>12} | {:>14}",
        "Data Structure",
        "Hit Latency",
        "Hit Tput",
        "Miss Latency",
        "Miss Tput",
        "Iter Latency",
        "Iter Tput",
        "Memory (B/key)"
    );
    println!("{:-<123}", "");

    for r in results {
        let iter_note = if r.is_ordered_iter {
            ""
        } else {
            " (unordered)"
        };
        println!(
            "{:<20} | {:>8.2} ns | {:>7.1} Mops/s | {:>9.2} ns | {:>7.1} Mops/s | {:>9.2} ms | {:>7.1} Mops/s | {:>10.2} B/k{}",
            r.name,
            r.hit_latency_ns,
            r.hit_throughput_mops,
            r.miss_latency_ns,
            r.miss_throughput_mops,
            r.iter_latency_ms,
            r.iter_throughput_mops,
            r.bytes_per_key,
            iter_note
        );
    }

    if results.len() >= 3 {
        let expanse = &results[0];
        let hashbrown = &results[1];
        let btree = &results[2];

        println!(
            "\nComparative Multipliers ({} relative to baselines):",
            expanse.name
        );
        println!(
            "  • Point Lookup (Hit):     {} than {}  |  {} than {}",
            format_multiplier(expanse.hit_latency_ns, hashbrown.hit_latency_ns, false),
            hashbrown.name,
            format_multiplier(expanse.hit_latency_ns, btree.hit_latency_ns, false),
            btree.name,
        );
        println!(
            "  • Point Lookup (Miss):    {} than {}  |  {} than {}",
            format_multiplier(expanse.miss_latency_ns, hashbrown.miss_latency_ns, false),
            hashbrown.name,
            format_multiplier(expanse.miss_latency_ns, btree.miss_latency_ns, false),
            btree.name,
        );
        println!(
            "  • Full Iteration / Scan:  {} vs {}   |  {} vs {}",
            format_multiplier(
                expanse.iter_throughput_mops,
                hashbrown.iter_throughput_mops,
                true
            ),
            hashbrown.name,
            format_multiplier(
                expanse.iter_throughput_mops,
                btree.iter_throughput_mops,
                true
            ),
            btree.name,
        );
        println!(
            "  • Memory Footprint:       {} than {} |  {} than {}",
            format_mem_ratio(expanse.bytes_per_key, hashbrown.bytes_per_key),
            hashbrown.name,
            format_mem_ratio(expanse.bytes_per_key, btree.bytes_per_key),
            btree.name,
        );
    }
}

/// Benchmark configuration options.
struct Config {
    pop: usize,
    dists: Vec<&'static str>,
    is_set: bool,
    is_json: bool,
}

/// Parses command-line arguments.
fn parse_args() -> Config {
    let mut pop = 1_000_000;
    let mut quick = false;
    let mut custom_pop = false;
    let mut dists = Vec::new();
    let mut is_set = false;
    let mut is_json = false;

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--quick" || arg == "-q" {
            quick = true;
        } else if arg == "--json" {
            is_json = true;
        } else if arg == "--pop" || arg == "-n" {
            if i + 1 < args.len() {
                i += 1;
                if let Ok(val) = args[i].replace('_', "").parse::<usize>() {
                    pop = val;
                    custom_pop = true;
                }
            }
        } else if let Some(val_str) = arg.strip_prefix("--pop=") {
            if let Ok(val) = val_str.replace('_', "").parse::<usize>() {
                pop = val;
                custom_pop = true;
            }
        } else if arg == "--dist" || arg == "-d" {
            if i + 1 < args.len() {
                i += 1;
                match args[i].as_str() {
                    "sequential" => dists.push("sequential"),
                    "random" => dists.push("random"),
                    "clustered" => dists.push("clustered"),
                    "sparse" => dists.push("sparse"),
                    "all" => {}
                    other => eprintln!("Warning: unknown distribution '{other}', ignoring"),
                }
            }
        } else if arg == "--set" {
            is_set = true;
        } else if arg == "--help" || arg == "-h" {
            println!("Instant Comparative Benchmark Harness (Expanse vs hashbrown vs BTreeMap)");
            println!(
                "\nUsage: cargo run --release -p expanse-trie --example bench_lookup_compare [OPTIONS]"
            );
            println!("\nOptions:");
            println!("  --pop <N>, -n <N>      Population size (default: 1,000,000)");
            println!("  --quick, -q            Quick test mode (sets population to 10,000)");
            println!(
                "  --dist <D>, -d <D>     Target distribution (sequential, random, clustered, sparse, all)"
            );
            println!("  --set                  Run ExpanseSet comparison instead of ExpanseMap");
            println!("  --json                 Output machine-readable JSON");
            println!("  --help, -h             Print help information");
            std::process::exit(0);
        }
        i += 1;
    }

    if quick && !custom_pop {
        pop = 10_000;
    }

    if dists.is_empty() {
        dists = vec!["sequential", "random", "clustered"];
    }

    Config {
        pop,
        dists,
        is_set,
        is_json,
    }
}

fn print_json(
    pop: usize,
    dists: &[&str],
    summary_rows: &[(&str, Vec<ContainerResult>)],
    is_set: bool,
) {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"system\": {\n");
    out.push_str(&format!("    \"os\": \"{}\",\n", std::env::consts::OS));
    out.push_str(&format!("    \"arch\": \"{}\",\n", std::env::consts::ARCH));
    out.push_str(&format!("    \"pointer_width\": {}\n", usize::BITS));
    out.push_str("  },\n");
    out.push_str(&format!("  \"pop\": {pop},\n"));
    out.push_str(&format!("  \"is_set\": {is_set},\n"));
    out.push_str("  \"distributions\": [");
    for (i, d) in dists.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&format!("\"{d}\""));
    }
    out.push_str("],\n");
    out.push_str("  \"results\": {\n");
    for (d_idx, (dist, res)) in summary_rows.iter().enumerate() {
        out.push_str(&format!("    \"{dist}\": {{\n"));
        for (r_idx, r) in res.iter().enumerate() {
            let key = match r_idx {
                0 => "expanse",
                1 => "hashbrown",
                _ => "btree",
            };
            out.push_str(&format!(
                "      \"{}\": {{\"name\": \"{}\", \"lookup_ns\": {:.2}, \"lookup_mops\": {:.2}, \"hit_latency_ns\": {:.2}, \"hit_throughput_mops\": {:.2}, \"miss_latency_ns\": {:.2}, \"miss_throughput_mops\": {:.2}, \"iter_latency_ms\": {:.2}, \"iter_mops\": {:.2}, \"iter_throughput_mops\": {:.2}, \"bytes_per_key\": {:.2}}}",
                key, r.name, r.hit_latency_ns, r.hit_throughput_mops, r.hit_latency_ns, r.hit_throughput_mops, r.miss_latency_ns, r.miss_throughput_mops, r.iter_latency_ms, r.iter_throughput_mops, r.iter_throughput_mops, r.bytes_per_key
            ));
            if r_idx + 1 < res.len() {
                out.push_str(",\n");
            } else {
                out.push('\n');
            }
        }
        if d_idx + 1 < summary_rows.len() {
            out.push_str("    },\n");
        } else {
            out.push_str("    }\n");
        }
    }
    out.push_str("  }\n");
    out.push_str("}\n");
    println!("{out}");
}

fn main() {
    let config = parse_args();
    let headline = if config.is_set {
        "ExpanseSet vs hashbrown::HashSet vs std::BTreeSet"
    } else {
        "ExpanseMap vs hashbrown::HashMap vs std::BTreeMap"
    };

    if !config.is_json {
        println!(
            "============================================================================================================"
        );
        println!(" Instant Comparative Benchmark Harness: {headline}");
        println!(
            " Population: N = {} keys | Zero statistical warmup overhead (<2s target)",
            config.pop
        );
        println!(
            "============================================================================================================"
        );
    }

    let start_total = Instant::now();
    let probe_count = if config.pop >= 100_000 {
        config.pop.min(1_000_000)
    } else {
        (config.pop * 10).clamp(10_000, 100_000)
    };

    let mut summary_rows = Vec::new();

    for &dist in &config.dists {
        let keys = generate_keys(dist, config.pop, 0x0DDB_1A5E_5EED_0001);
        let (hit_probes, miss_probes) = generate_probes(&keys, probe_count, 0xBEEF_CAFE_1234_5678);

        let results = if config.is_set {
            vec![
                bench_expanse_set(&keys, &hit_probes, &miss_probes),
                bench_hashbrown_set(&keys, &hit_probes, &miss_probes),
                bench_btreeset(&keys, &hit_probes, &miss_probes),
            ]
        } else {
            vec![
                bench_expanse_map(&keys, &hit_probes, &miss_probes),
                bench_hashbrown_map(&keys, &hit_probes, &miss_probes),
                bench_btreemap(&keys, &hit_probes, &miss_probes),
            ]
        };

        if !config.is_json {
            print_distribution_report(dist, config.pop, &results);
        }
        summary_rows.push((dist, results));
    }

    if config.is_json {
        print_json(config.pop, &config.dists, &summary_rows, config.is_set);
        return;
    }

    // Summary matrix across all tested distributions
    println!(
        "\n============================================================================================================"
    );
    println!(" Summary Matrix: Point Lookup Hit Latency (ns/op) & Memory Footprint (B/key)");
    println!(
        "============================================================================================================"
    );
    let name_exp = if config.is_set {
        "ExpanseSet"
    } else {
        "ExpanseMap"
    };
    let name_hb = if config.is_set {
        "hashbrown::HashSet"
    } else {
        "hashbrown::HashMap"
    };
    let name_bt = if config.is_set {
        "std::BTreeSet"
    } else {
        "std::BTreeMap"
    };

    println!(
        "{:<12} | {:<22} | {:<22} | {:<22} | {:<24}",
        "Distribution", name_exp, name_hb, name_bt, "Expanse vs Baselines"
    );
    println!("{:-<108}", "");

    for (dist, res) in &summary_rows {
        let exp = &res[0];
        let hb = &res[1];
        let bt = &res[2];

        let exp_summary = format!(
            "{:.2} ns ({:.2} B/k)",
            exp.hit_latency_ns, exp.bytes_per_key
        );
        let hb_summary = format!("{:.2} ns ({:.2} B/k)", hb.hit_latency_ns, hb.bytes_per_key);
        let bt_summary = format!("{:.2} ns ({:.2} B/k)", bt.hit_latency_ns, bt.bytes_per_key);

        let vs_hb = format_multiplier(exp.hit_latency_ns, hb.hit_latency_ns, false);
        let vs_bt = format_multiplier(exp.hit_latency_ns, bt.hit_latency_ns, false);
        let comp = format!("{vs_hb} vs hash | {vs_bt} vs btree");

        println!(
            "{:<12} | {:<22} | {:<22} | {:<22} | {:<24}",
            dist, exp_summary, hb_summary, bt_summary, comp
        );
    }
    println!(
        "============================================================================================================"
    );
    let total_elapsed = start_total.elapsed();
    println!(
        "Total execution elapsed: {:.2}s\n",
        total_elapsed.as_secs_f64()
    );
}
