//! Comparative Micro-benchmarks for 32-Bit Embedded Expanse vs Industry Primitives.
//!
//! Evaluates `ExpanseSet32`, `ExpanseMap32`, and `ExpanseBlobMap32` against:
//! - `std::collections::BTreeMap<u32, u32>`
//! - `hashbrown::HashMap<u32, u32>`
//!
//! Across real embedded workloads (Sensor Buffers, CAN-bus dispatch, IPv4 routing, OTA chunks).

use std::collections::BTreeMap;
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use expanse_trie::{ExpanseBlobMap32, ExpanseMap32, ExpanseSet32, Key32};
use hashbrown::HashMap;

fn bench_sensor_indexing(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedded_sensor_timestamps");
    let n = 10_000;
    let keys: Vec<Key32> = (0..n).map(|i| 1_700_000_000 + i as Key32).collect();

    // 1. ExpanseSet32 Insert
    group.bench_function(BenchmarkId::new("expanse_set32", n), |b| {
        b.iter(|| {
            let mut set = ExpanseSet32::new();
            for &k in &keys {
                set.insert(black_box(k));
            }
            black_box(set)
        });
    });

    // 2. BTreeSet Insert
    group.bench_function(BenchmarkId::new("btreeset_u32", n), |b| {
        b.iter(|| {
            let mut set = std::collections::BTreeSet::new();
            for &k in &keys {
                set.insert(black_box(k));
            }
            black_box(set)
        });
    });

    group.finish();
}

fn bench_sparse_can_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedded_can_dispatch");
    let n = 500;
    let keys: Vec<Key32> = (0..n).map(|i| (i * 100_007) & 0x1FFF_FFFF).collect();

    // Pre-populate map
    let mut exp_map = ExpanseMap32::new();
    let mut hash_map = HashMap::new();
    let mut btree_map = BTreeMap::new();

    for (idx, &k) in keys.iter().enumerate() {
        exp_map.insert(k, idx as u32);
        hash_map.insert(k, idx as u32);
        btree_map.insert(k, idx as u32);
    }

    // Lookup benchmarks
    group.bench_function(BenchmarkId::new("expanse_map32_get", n), |b| {
        b.iter(|| {
            let mut sum = 0u64;
            for &k in &keys {
                if let Some(v) = exp_map.get(black_box(k)) {
                    sum += v as u64;
                }
            }
            black_box(sum)
        });
    });

    group.bench_function(BenchmarkId::new("hashbrown_get", n), |b| {
        b.iter(|| {
            let mut sum = 0u64;
            for &k in &keys {
                if let Some(&v) = hash_map.get(black_box(&k)) {
                    sum += v as u64;
                }
            }
            black_box(sum)
        });
    });

    group.bench_function(BenchmarkId::new("btreemap_get", n), |b| {
        b.iter(|| {
            let mut sum = 0u64;
            for &k in &keys {
                if let Some(&v) = btree_map.get(black_box(&k)) {
                    sum += v as u64;
                }
            }
            black_box(sum)
        });
    });

    group.finish();
}

fn bench_blobmap32_predicate_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedded_blobmap_predicate_scan");
    let n = 2_000;

    let mut blob_map = ExpanseBlobMap32::new();
    for i in 0..n {
        blob_map.insert(i as Key32, b"payload_bytes_sample", (i % 100) as u16);
    }

    group.bench_function(BenchmarkId::new("columnar_hot_meta_scan", n), |b| {
        b.iter(|| {
            let mut matched = 0usize;
            blob_map.scan_filtered(
                black_box(100),
                black_box(1900),
                |_k, meta| meta > 80,
                |_k, _view, _meta| {
                    matched += 1;
                },
            );
            black_box(matched)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_sensor_indexing,
    bench_sparse_can_dispatch,
    bench_blobmap32_predicate_scan
);
criterion_main!(benches);
