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

/// Predicate-scan sweep with a payload-touching baseline — the falsifiable
/// variant of [`bench_predicate_scan_selectivity_sweep`] designed to test the
/// RFC §10.3 DRAM-traffic premise.
///
/// The warm-payload sweep above cannot show the columnar advantage because
/// (a) its 50k×128B = 6.4 MiB arena fits in the L3 cache, so skipping a payload
/// saves no DRAM fetch, and (b) its "naive" baseline only reads `view.len()`
/// (a slot field), never touching payload bytes — so both arms avoid cold
/// payload cache lines and the pushdown has nothing to win.
///
/// This variant fixes (b): both arms use the identical `scan_filtered`
/// traversal, so trie-walk cost cancels and the *only* difference is payload
/// touches:
///   - `columnar_hotmeta_filter`: predicate on hot metadata; the payload is
///     dereferenced (one byte read, forcing a cache-line load) only for the σ
///     fraction of entries that match.
///   - `naive_row_deref`: models a row store whose predicate field lives in the
///     payload — every entry's payload is dereferenced before the predicate is
///     applied, so payload traffic is independent of σ.
///
/// It **cannot** fix (a) on the current engine. The RFC premise needs the arena
/// working set to exceed the host LLC so the skipped payloads are cache-*cold*
/// (a real DRAM fetch, not an L3 hit). But the 64-bit `ExpanseBlobMap` arena is
/// hard-capped at **16 MiB** by the 24-bit `ArenaShort` value-slot offset (see
/// `blobmap.rs` "Capacity limits"; the wider `ArenaLong`/`External` encodings
/// that would lift it are unimplemented). 16 MiB is *smaller* than a typical
/// server LLC (honeycomb's L3 is 30 MiB), so the entire arena is forced
/// L3-resident and payload skips save an L3 hit, not a DRAM fetch. The
/// `>10× at σ≤0.05` target is therefore not reachable on this structure until
/// the wide-offset arena lands — this bench measures the *warm-arena* speedup
/// that isolating the payload touch actually yields, near the ceiling.
///
/// `N × (PAYLOAD + 8-byte record header)`, 16-byte aligned, must stay under the
/// 16 MiB ceiling; the defaults fill it to ~14.6 MiB.
fn bench_predicate_scan_cold_dram_sweep(c: &mut Criterion) {
    /// Payload bytes per entry (16 cache lines) — large enough that the payload
    /// fetch, not the slot/metadata read, dominates a matched entry's cost.
    const PAYLOAD: usize = 1024;
    /// Key count sized so the arena fills close to (but under) the 16 MiB
    /// `ArenaShort` ceiling: 14_000 × align16(1024 + 8) = ~14.6 MiB.
    const N: u64 = 14_000;
    /// Metadata is drawn uniformly from `[0, META_RANGE)`; a `meta <= threshold`
    /// predicate then selects `threshold / META_RANGE` of the entries.
    const META_RANGE: u32 = 10_000;
    /// Reference LLC used only for the logged arena/LLC ratio (honeycomb L3).
    const LLC_BYTES: f64 = 30.0 * 1024.0 * 1024.0;

    let record = (PAYLOAD + 8).div_ceil(16) * 16; // 8-byte header, 16B aligned
    let arena_bytes = N as usize * record;
    eprintln!(
        "cold_dram_sweep config: N={N} payload={PAYLOAD}B arena~{:.1} MiB (ArenaShort ceiling 16 MiB; ref LLC 30 MiB => arena/LLC ~{:.2}x, so payloads stay L3-resident — cold-DRAM regime is unreachable until wide-offset arena lands)",
        arena_bytes as f64 / (1024.0 * 1024.0),
        arena_bytes as f64 / LLC_BYTES,
    );

    let mut g = c.benchmark_group("predicate_scan_cold_dram_sweep");

    let mut map = ExpanseBlobMap::with_chunk_size(16 * 1024 * 1024);
    let mut rng = XorShift(0xDEAD_BEEF_1234_5678);
    for k in 0..N {
        let meta = (rng.next() % META_RANGE as u64) as u32;
        // Distinct per-key payload bytes so reads cannot be constant-folded.
        let payload = vec![(k & 0xFF) as u8; PAYLOAD];
        map.insert(k, &payload, meta).unwrap();
    }

    let selectivities = [
        ("sigma_0.001", 10u32),
        ("sigma_0.01", 100u32),
        ("sigma_0.05", 500u32),
        ("sigma_0.20", 2000u32),
        ("sigma_1.0", 10000u32),
    ];

    for (label, threshold) in selectivities {
        // Columnar hot-metadata pushdown: payload touched only on a match.
        g.bench_function(BenchmarkId::new("columnar_hotmeta_filter", label), |b| {
            b.iter(|| {
                let mut matches = 0usize;
                let mut byte_sum = 0u64;
                map.scan_filtered(
                    0..=N,
                    |_key, meta| meta <= threshold,
                    |_key, view, _meta| {
                        // Force the cold payload cache line to load.
                        byte_sum += view[0] as u64;
                        matches += 1;
                        true
                    },
                );
                black_box((matches, byte_sum))
            });
        });

        // Naive row store: predicate lives in the payload, so every entry's
        // payload cache line is loaded before the predicate is evaluated.
        g.bench_function(BenchmarkId::new("naive_row_deref", label), |b| {
            b.iter(|| {
                let mut matches = 0usize;
                let mut byte_sum = 0u64;
                map.scan_filtered(
                    0..=N,
                    |_key, _meta| true,
                    |_key, view, meta| {
                        // Unconditional payload load (row must be read to test).
                        byte_sum += view[0] as u64;
                        if meta <= threshold {
                            matches += 1;
                        }
                        true
                    },
                );
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
    bench_predicate_scan_cold_dram_sweep,
    bench_arena_compaction_churn
);
criterion_main!(benches);
