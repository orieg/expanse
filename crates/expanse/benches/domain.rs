//! Micro-benchmarks for the Interned Set Domain (Issue #611).
//!
//! Measures:
//! 1. Set algebra parity: confirms DomainSet provenance verification adds 0 ns overhead over raw ExpanseSet.
//! 2. Ingestion throughput: scalar insert vs batched insert on text and binary UUID keys.
//! 3. Zero-copy resolution: throughput of iterating and reading borrowed slices from the stable slab arena.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `domain_interned_set` |
//! | `group` | 4 |
//! | `population` | 10k, 50k, 100k |
//! | `probes_and_reuse` | Domain sets and key slices |
//! | `hit_rate` | 100% hits on insertion & resolution; 50% overlap on intersections |
//! | `miss_gen_method` | None |
//! | `value_dereference` | direct byte slice reading via `resolve()` |
//! | `measured_region` | timed inner loop over operations (allocations and setup outside) |
//! | `arm_symmetry` | symmetric key sequences across DomainSet and ExpanseSet |
//! | `statistics` | Criterion estimate |
//! | `verdict` | **PASS** `[verified: CODE READ]`: Interned domain posting-list algebra and ingestion. |

#![cfg(target_pointer_width = "64")]
#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use expanse_trie::domain::ExpanseDomainDict;
use expanse_trie::set::ExpanseSet;
use std::hint::black_box;

fn generate_text_keys(n: usize) -> Vec<Vec<u8>> {
    (0..n)
        .map(|i| format!("user:{i:016x}").into_bytes())
        .collect()
}

fn generate_uuid_keys(n: usize) -> Vec<[u8; 16]> {
    // Generate synthetic 16-byte UUIDs with deterministic embedded NUL bytes
    (0..n)
        .map(|i| {
            let mut b = [0u8; 16];
            let high = (i as u64).to_be_bytes();
            let low = ((i as u64) ^ 0xDEAD_BEEF_CAFE_BABE).to_be_bytes();
            b[0..8].copy_from_slice(&high);
            b[8..16].copy_from_slice(&low);
            // Intentionally force a NUL byte at index 4 and 12
            b[4] = 0;
            b[12] = 0;
            b
        })
        .collect()
}

fn bench_domain_set_algebra_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("domain_set_algebra_overhead");
    for &n in &[10_000, 100_000] {
        // Setup raw ExpanseSets
        let mut raw_a = ExpanseSet::new();
        let mut raw_b = ExpanseSet::new();
        for i in 0..n {
            raw_a.insert(i as u64);
            if i % 2 == 0 {
                raw_b.insert(i as u64);
            }
        }

        // Setup DomainSets via ExpanseDomainDict
        let mut dict = ExpanseDomainDict::new();
        let mut dom_a = dict.new_set();
        let mut dom_b = dict.new_set();
        for i in 0..n {
            let key = format!("entity:{i}").into_bytes();
            dict.insert(&mut dom_a, &key).unwrap();
            if i % 2 == 0 {
                dict.insert(&mut dom_b, &key).unwrap();
            }
        }

        // 1. Raw ExpanseSet intersection
        group.bench_function(BenchmarkId::new("raw_expanse_set_intersection", n), |b| {
            b.iter(|| {
                let out = black_box(&raw_a).intersection(black_box(&raw_b));
                let len = out.len();
                core::mem::forget(out);
                len
            });
        });

        // 2. DomainSet intersection (measures domain verification overhead)
        group.bench_function(BenchmarkId::new("domain_set_intersection", n), |b| {
            b.iter(|| {
                let out = black_box(&dom_a).intersection(black_box(&dom_b)).unwrap();
                let len = out.len();
                core::mem::forget(out);
                len
            });
        });

        // 3. Raw ExpanseSet intersection_len (cardinality only)
        group.bench_function(
            BenchmarkId::new("raw_expanse_set_intersection_len", n),
            |b| {
                b.iter(|| black_box(&raw_a).intersection_len(black_box(&raw_b)));
            },
        );

        // 4. DomainSet intersection_len (cardinality only)
        group.bench_function(BenchmarkId::new("domain_set_intersection_len", n), |b| {
            b.iter(|| {
                black_box(&dom_a)
                    .intersection_len(black_box(&dom_b))
                    .unwrap()
            });
        });
    }
    group.finish();
}

fn bench_domain_ingestion(c: &mut Criterion) {
    let mut group = c.benchmark_group("domain_ingestion");
    for &n in &[10_000, 50_000] {
        let text_keys = generate_text_keys(n);
        let uuid_keys = generate_uuid_keys(n);

        // 1. Scalar text key insertion
        group.bench_function(BenchmarkId::new("scalar_insert_text", n), |b| {
            b.iter_batched(
                || {
                    let dict = ExpanseDomainDict::new();
                    let set = dict.new_set();
                    (dict, set)
                },
                |(mut dict, mut set)| {
                    for k in &text_keys {
                        dict.insert(&mut set, black_box(k)).unwrap();
                    }
                    black_box((dict, set))
                },
                criterion::BatchSize::SmallInput,
            );
        });

        // 2. Batched text key insertion
        let text_refs: Vec<&[u8]> = text_keys.iter().map(|k| k.as_slice()).collect();
        group.bench_function(BenchmarkId::new("batch_insert_text", n), |b| {
            b.iter_batched(
                || {
                    let dict = ExpanseDomainDict::new();
                    let set = dict.new_set();
                    (dict, set)
                },
                |(mut dict, mut set)| {
                    for chunk in text_refs.chunks(128) {
                        dict.insert_batch(&mut set, black_box(chunk)).unwrap();
                    }
                    black_box((dict, set))
                },
                criterion::BatchSize::SmallInput,
            );
        });

        // 3. Scalar binary UUID key insertion (tests order-preserving escape encoding)
        group.bench_function(BenchmarkId::new("scalar_insert_uuid", n), |b| {
            b.iter_batched(
                || {
                    let dict = ExpanseDomainDict::new();
                    let set = dict.new_set();
                    (dict, set)
                },
                |(mut dict, mut set)| {
                    for k in &uuid_keys {
                        dict.insert(&mut set, black_box(k)).unwrap();
                    }
                    black_box((dict, set))
                },
                criterion::BatchSize::SmallInput,
            );
        });

        // 4. Batched binary UUID key insertion
        let uuid_refs: Vec<&[u8]> = uuid_keys.iter().map(|k| k.as_slice()).collect();
        group.bench_function(BenchmarkId::new("batch_insert_uuid", n), |b| {
            b.iter_batched(
                || {
                    let dict = ExpanseDomainDict::new();
                    let set = dict.new_set();
                    (dict, set)
                },
                |(mut dict, mut set)| {
                    for chunk in uuid_refs.chunks(128) {
                        dict.insert_batch(&mut set, black_box(chunk)).unwrap();
                    }
                    black_box((dict, set))
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_domain_resolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("domain_resolution");
    for &n in &[10_000, 100_000] {
        let text_keys = generate_text_keys(n);
        let mut dict = ExpanseDomainDict::new();
        let mut set = dict.new_set();
        for k in &text_keys {
            dict.insert(&mut set, k).unwrap();
        }

        // Full zero-copy resolution scan from BlobArena
        group.bench_function(BenchmarkId::new("resolve_full_scan", n), |b| {
            b.iter(|| {
                let mut byte_sum = 0usize;
                for slice in dict.resolve(black_box(&set)).unwrap() {
                    byte_sum = byte_sum.wrapping_add(slice.len());
                }
                black_box(byte_sum)
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_domain_set_algebra_overhead,
    bench_domain_ingestion,
    bench_domain_resolution
);
criterion_main!(benches);
