//! Micro-benchmarks for Issue #112 large-value architectural optimizations:
//! - Inline vs Heap allocation for small blobs (0..=7 bytes)
//! - Columnar hot-metadata predicate scan selectivity sweep
//! - Slab arena compaction & GC churn

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use expanse_trie::blobmap::ExpanseBlobMap;
use std::collections::BTreeMap;
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

/// Benchmark throughput and allocation comparisons for 0..=7 byte payloads:
/// ExpanseBlobMap (inline value slots, zero heap allocations) vs BTreeMap<u64, Vec<u8>> (heap allocation).
fn bench_inline_vs_heap_small_blobs(c: &mut Criterion) {
    let mut g = c.benchmark_group("inline_vs_heap_small_blobs");
    let payload_sizes = [0usize, 1, 4, 7];

    for &size in &payload_sizes {
        let payload: Vec<u8> = (0..size).map(|i| (i + 1) as u8).collect();
        let num_keys = 10_000u64;

        // Benchmark ExpanseBlobMap insert
        g.bench_with_input(
            BenchmarkId::new("expanse_blobmap_insert", format!("{size}B")),
            &payload,
            |b, p| {
                b.iter(|| {
                    let mut map = ExpanseBlobMap::new();
                    for k in 0..num_keys {
                        map.insert(k, p, 0).unwrap();
                    }
                    black_box(map)
                });
            },
        );

        // Benchmark BTreeMap insert
        g.bench_with_input(
            BenchmarkId::new("btreemap_heap_insert", format!("{size}B")),
            &payload,
            |b, p| {
                b.iter(|| {
                    let mut map = BTreeMap::new();
                    for k in 0..num_keys {
                        map.insert(k, p.clone());
                    }
                    black_box(map)
                });
            },
        );

        // Pre-populate for lookup benchmark
        let mut blob_map = ExpanseBlobMap::new();
        let mut btree_map = BTreeMap::new();
        for k in 0..num_keys {
            blob_map.insert(k, &payload, 0).unwrap();
            btree_map.insert(k, payload.clone());
        }

        // Benchmark ExpanseBlobMap get
        g.bench_with_input(
            BenchmarkId::new("expanse_blobmap_get", format!("{size}B")),
            &payload,
            |b, _| {
                b.iter(|| {
                    let mut sink = 0usize;
                    for k in 0..num_keys {
                        if let Some((view, _)) = blob_map.get(black_box(k)) {
                            sink += view.len();
                        }
                    }
                    black_box(sink)
                });
            },
        );

        // Benchmark BTreeMap get
        g.bench_with_input(
            BenchmarkId::new("btreemap_heap_get", format!("{size}B")),
            &payload,
            |b, _| {
                b.iter(|| {
                    let mut sink = 0usize;
                    for k in 0..num_keys {
                        if let Some(vec) = btree_map.get(&black_box(k)) {
                            sink += vec.len();
                        }
                    }
                    black_box(sink)
                });
            },
        );
    }
    g.finish();
}

/// Benchmark scan latency across selectivity sigma in {0.001, 0.01, 0.05, 0.20, 1.0}.
/// Compares Expanse hot metadata filtering (which evaluates predicates in leaf slots
/// before loading cold arena payload cache lines) vs naive full payload dereferencing.
fn bench_predicate_scan_selectivity_sweep(c: &mut Criterion) {
    let mut g = c.benchmark_group("predicate_scan_selectivity_sweep");
    let total_keys = 50_000u64;

    // Build map with 128-byte arena payloads and metadata in 0..10_000
    let mut map = ExpanseBlobMap::with_chunk_size(16 * 1024 * 1024);
    let mut rng = XorShift(0xDEAD_BEEF_1234_5678);

    for k in 0..total_keys {
        let meta = (rng.next() % 10_000) as u32;
        let payload = vec![(k & 0xFF) as u8; 128];
        map.insert(k, &payload, meta).unwrap();
    }

    // Selectivities sigma and corresponding threshold ceilings (meta <= threshold)
    let selectivities = [
        ("sigma_0.001", 10u32),
        ("sigma_0.01", 100u32),
        ("sigma_0.05", 500u32),
        ("sigma_0.20", 2000u32),
        ("sigma_1.0", 10000u32),
    ];

    for (label, threshold) in selectivities {
        g.bench_function(BenchmarkId::new("columnar_filtered_scan", label), |b| {
            b.iter(|| {
                let mut matches = 0usize;
                let mut byte_sum = 0usize;
                map.scan_filtered(
                    0..=total_keys,
                    |_key, meta| meta <= threshold,
                    |_key, view, _meta| {
                        matches += 1;
                        byte_sum += view.len();
                        true
                    },
                );
                black_box((matches, byte_sum))
            });
        });

        // Baseline: Naive scan where every payload is unconditionally dereferenced
        g.bench_function(BenchmarkId::new("naive_unfiltered_deref", label), |b| {
            b.iter(|| {
                let mut matches = 0usize;
                let mut byte_sum = 0usize;
                for k in 0..total_keys {
                    if let Some((view, meta)) = map.get(k) {
                        if meta <= threshold {
                            matches += 1;
                            byte_sum += view.len();
                        }
                    }
                }
                black_box((matches, byte_sum))
            });
        });
    }
    g.finish();
}

/// Benchmark GC compaction pause times and memory recovery under overwrite / deletion churn.
fn bench_arena_compaction_churn(c: &mut Criterion) {
    let mut g = c.benchmark_group("arena_compaction_churn");
    let churn_sizes = [5_000u64, 20_000u64];

    for &num_keys in &churn_sizes {
        g.bench_with_input(
            BenchmarkId::new("compact_after_50pct_delete", num_keys),
            &num_keys,
            |b, &n| {
                b.iter_batched(
                    || {
                        let mut map = ExpanseBlobMap::with_chunk_size(2 * 1024 * 1024);
                        for k in 0..n {
                            let payload = vec![(k & 0xFF) as u8; 256];
                            map.insert(k, &payload, k as u32).unwrap();
                        }
                        // Delete 50%
                        for k in 0..(n / 2) {
                            map.remove(k);
                        }
                        map
                    },
                    |mut map| {
                        let stats = map.compact().unwrap();
                        black_box(stats)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        g.bench_with_input(
            BenchmarkId::new("compact_after_80pct_delete", num_keys),
            &num_keys,
            |b, &n| {
                b.iter_batched(
                    || {
                        let mut map = ExpanseBlobMap::with_chunk_size(2 * 1024 * 1024);
                        for k in 0..n {
                            let payload = vec![(k & 0xFF) as u8; 256];
                            map.insert(k, &payload, k as u32).unwrap();
                        }
                        // Delete 80%
                        for k in 0..(n * 4 / 5) {
                            map.remove(k);
                        }
                        map
                    },
                    |mut map| {
                        let stats = map.compact().unwrap();
                        black_box(stats)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    g.finish();
}

criterion_group!(
    benches,
    bench_inline_vs_heap_small_blobs,
    bench_predicate_scan_selectivity_sweep,
    bench_arena_compaction_churn
);
criterion_main!(benches);
