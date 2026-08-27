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
                    if let Some((view, meta)) = map.get(k)
                        && meta <= threshold
                    {
                        matches += 1;
                        byte_sum += view.len();
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
/// This bench is the **warm-arena** measurement: it deliberately keeps the
/// arena small (~14.6 MiB) so the whole working set stays L3-resident (the
/// reference host's L3 is 30 MiB), isolating the speedup that skipping a payload
/// *touch* yields when the payload is only an L3 hit, not a cold-DRAM fetch. It
/// therefore does not reach the RFC `>10× at σ≤0.05` target — with a warm
/// working set a skipped payload saves ~7 ns (L3), not ~80 ns (DRAM), so the
/// columnar arm is traversal-floor bounded near ~1.4×.
///
/// The **cold-DRAM** regime — a `>`LLC arena where skipped payloads are real
/// DRAM fetches — is measured by [`bench_predicate_scan_cold_dram_large`]. That
/// became possible once the `ArenaMeta` encoding (#287) lifted the former 16 MiB
/// `ArenaShort` size ceiling, so an arena can exceed the LLC while every entry
/// keeps its hot metadata. See §10.3 of `docs/design/large-values.md`.
///
/// `N × (PAYLOAD + 8-byte record header)`, 16-byte aligned; the defaults fill it
/// to ~14.6 MiB — chosen to stay L3-resident, not by any encoding limit.
fn bench_predicate_scan_cold_dram_sweep(c: &mut Criterion) {
    /// Payload bytes per entry (16 cache lines) — large enough that the payload
    /// fetch, not the slot/metadata read, dominates a matched entry's cost.
    const PAYLOAD: usize = 1024;
    /// Key count sized so the arena stays L3-resident (warm): 14_000 ×
    /// align16(1024 + 8) = ~14.6 MiB, comfortably under the 30 MiB reference L3.
    const N: u64 = 14_000;
    /// Metadata is drawn uniformly from `[0, META_RANGE)`; a `meta <= threshold`
    /// predicate then selects `threshold / META_RANGE` of the entries.
    const META_RANGE: u32 = 10_000;
    /// Reference LLC used only for the logged arena/LLC ratio (reference host L3).
    const LLC_BYTES: f64 = 30.0 * 1024.0 * 1024.0;

    let record = (PAYLOAD + 8).div_ceil(16) * 16; // 8-byte header, 16B aligned
    let arena_bytes = N as usize * record;
    eprintln!(
        "cold_dram_sweep config: N={N} payload={PAYLOAD}B arena~{:.1} MiB (warm: ref LLC 30 MiB => arena/LLC ~{:.2}x, payloads stay L3-resident; the >LLC cold-DRAM regime is measured by cold_dram_large)",
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

/// Cold-DRAM predicate-scan sweep over a **> LLC** arena — the RFC §10.3
/// re-benchmark, sized so a skipped payload is a real ~80 ns DRAM fetch.
///
/// [`bench_predicate_scan_cold_dram_sweep`] above is capped at a ~14.6 MiB arena
/// whose working set stays L3-resident (reference host L3 = 30 MiB), so skipped
/// payloads are only ~L3 hits — bounding the columnar advantage to ~1.4×. This
/// bench sizes the arena to ~9× the LLC. Keys are inserted in **shuffled** order
/// so an ordered key-scan touches arena offsets in random order (defeating the
/// hardware prefetcher that would otherwise stream a sequential arena and hide
/// the DRAM latency).
///
/// **Metadata degeneration fixed in #285 Phase 1.** Before the uniform
/// `ArenaMeta` encoding, arena payloads past 16 MiB spilled to a metadata-less
/// `ArenaLong` slot (`hot_meta = 0`), so over a > LLC arena a `meta <= threshold`
/// predicate matched ~94% of entries regardless of σ and the columnar pushdown
/// degenerated to "touch every payload." `ArenaMeta` carries 24-bit metadata
/// across the whole arena, so the filter now works: the logged match-rate should
/// track σ (e.g. ~0.1% at σ=0.001), not the old ~94%. The bench keeps logging the
/// realized match-rate so the fix stays visible; the columnar-vs-naive timing is
/// the RFC §10.3 measurement (its ceiling is still traversal-bound; see the
/// design doc for the BlobLeafVector work that targets ≥10×).
fn bench_predicate_scan_cold_dram_large(c: &mut Criterion) {
    /// Payload bytes per entry (16 cache lines); the touched `view[0]` is one
    /// cold cache line per matched entry.
    const PAYLOAD: usize = 1024;
    /// ~272 MiB arena (262_144 × align16(1032) = 260 MiB) ≈ 9× the 30 MiB LLC.
    const N: u64 = 262_144;
    /// `meta <= threshold` selects `threshold / META_RANGE` of entries. Under the
    /// uniform `ArenaMeta` encoding every arena entry carries metadata, so the
    /// realized selectivity tracks this ratio across the whole arena.
    const META_RANGE: u32 = 10_000;
    /// Reference LLC, for the logged arena/LLC ratio (reference host L3).
    const LLC_BYTES: f64 = 30.0 * 1024.0 * 1024.0;
    /// Golden-ratio multiplier for a cheap deterministic per-key metadata.
    const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

    let record = (PAYLOAD + 8).div_ceil(16) * 16;
    let arena_bytes = N as usize * record;

    // Shuffled insertion order (Fisher-Yates) so key value is decorrelated from
    // arena offset: an ascending-key scan then reads payloads in random order.
    let mut order: Vec<u64> = (0..N).collect();
    let mut rng = XorShift(0x0BAD_F00D_C0FF_EE11);
    for i in (1..order.len()).rev() {
        let j = (rng.next() % (i as u64 + 1)) as usize;
        order.swap(i, j);
    }

    // 64 MiB chunks. Every arena entry uses the uniform `ArenaMeta` encoding, so
    // all N keys carry 24-bit metadata regardless of where they land (the #285
    // Phase 1 fix — previously entries past 16 MiB lost their metadata).
    let mut map = ExpanseBlobMap::with_chunk_size(64 * 1024 * 1024);
    for &k in &order {
        let meta = (k.wrapping_mul(GOLDEN) % META_RANGE as u64) as u32;
        let payload = vec![(k & 0xFF) as u8; PAYLOAD];
        map.insert(k, &payload, meta).unwrap();
    }

    let selectivities = [
        ("sigma_0.001", 10u32),
        ("sigma_0.05", 500u32),
        ("sigma_0.20", 2000u32),
        ("sigma_1.0", 10000u32),
    ];

    // Log realized match-rate vs the σ a working filter yields. With the uniform
    // ArenaMeta encoding these now agree (the filter works); a rate far above σ
    // would signal a metadata regression.
    eprintln!(
        "cold_dram_large config: N={N} payload={PAYLOAD}B arena~{:.0} MiB (ref LLC 30 MiB => arena/LLC ~{:.1}x, cold-DRAM regime reachable)",
        arena_bytes as f64 / (1024.0 * 1024.0),
        arena_bytes as f64 / LLC_BYTES,
    );
    for (label, threshold) in selectivities {
        let mut matches = 0usize;
        map.scan_filtered(
            0..=N,
            |_k, meta| meta <= threshold,
            |_k, _v, _m| {
                matches += 1;
                true
            },
        );
        eprintln!(
            "  {label}: columnar matches={matches} ({:.1}% of N) — a working hot-meta filter would match ~{:.1}%",
            100.0 * matches as f64 / N as f64,
            100.0 * threshold as f64 / META_RANGE as f64,
        );
    }

    let mut g = c.benchmark_group("predicate_scan_cold_dram_large");
    // Fewer samples: each iteration scans 262k × 1KiB cold payloads (~tens of ms).
    g.sample_size(10);

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
                        byte_sum += view[0] as u64;
                        matches += 1;
                        true
                    },
                );
                black_box((matches, byte_sum))
            });
        });

        // Naive row store: every entry's payload cache line is loaded before the
        // predicate is evaluated (payload traffic is independent of σ).
        g.bench_function(BenchmarkId::new("naive_row_deref", label), |b| {
            b.iter(|| {
                let mut matches = 0usize;
                let mut byte_sum = 0u64;
                map.scan_filtered(
                    0..=N,
                    |_key, _meta| true,
                    |_key, view, meta| {
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
    bench_predicate_scan_cold_dram_large,
    bench_arena_compaction_churn
);
criterion_main!(benches);
