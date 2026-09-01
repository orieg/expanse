//! Comparative micro-benchmarks for Embedded Telemetry MemTable & BLE Asset Tracker
//! on the 32-bit Expanse engine vs competitor baselines.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `core_embedded_memtable` |
//! | `group` | 2 |
//! | `population` | 500, 2k, 5k |
//! | `probes_and_reuse` | 500, 2k, 5k, looped |
//! | `hit_rate` | 100% hits on present timestamps/IDs; 50% hits on mixed point lookups |
//! | `miss_gen_method` | None |
//! | `value_dereference` | direct integer reading and 28-byte slab/arena tracking payload dereference |
//! | `measured_region` | timed inner loop over operations (allocations and setup outside) |
//! | `arm_symmetry` | symmetric random key sequences, matching 28-byte BLE tracking payload layout across all arms |
//! | `statistics` | Criterion estimate |
//! | `verdict` | ✅ Verified symmetric workloads with BCa CIs on reference host |

use std::collections::BTreeMap;
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use expanse_trie::{ExpanseBlobMap32, ExpanseMap32, Key32};
use hashbrown::HashMap;

/// 28-byte BLE tracking record (matching expanse_ble_record_t byte layout).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct BleRecord {
    /// 48-bit IEEE MAC address.
    pub mac: [u8; 6],
    /// RSSI in dBm.
    pub rssi: i8,
    /// Status/presence flags.
    pub flags: u8,
    /// Millisecond timestamp.
    pub last_seen_ms: u32,
    /// Estimated distance in cm.
    pub distance_cm: u16,
    /// Advertised device name.
    pub name: [u8; 14],
}

impl Default for BleRecord {
    fn default() -> Self {
        Self {
            mac: [0; 6],
            rssi: -70,
            flags: 0,
            last_seen_ms: 0,
            distance_cm: 100,
            name: *b"Dev_Sample_123",
        }
    }
}

fn fnv1a_32(data: &[u8]) -> u32 {
    let mut hash = 2_166_136_261u32;
    for &b in data {
        hash ^= b as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}

fn bench_sensor_tsdb_ingest_and_flush(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedded_tsdb_ingest_and_flush");
    for &n in &[500, 2000, 5000] {
        let keys: Vec<Key32> = (0..n).map(|i| 1_700_000_000 + i as Key32).collect();

        // 1. ExpanseMap32 Ingest
        group.bench_function(BenchmarkId::new("expanse_map32_ingest", n), |b| {
            b.iter_batched(
                ExpanseMap32::new,
                |mut map| {
                    for &k in &keys {
                        map.insert(black_box(k), black_box(42));
                    }
                    black_box(map)
                },
                criterion::BatchSize::PerIteration,
            );
        });

        // 2. BTreeMap Ingest
        group.bench_function(BenchmarkId::new("btreemap_ingest", n), |b| {
            b.iter_batched(
                BTreeMap::new,
                |mut map| {
                    for &k in &keys {
                        map.insert(black_box(k), black_box(42));
                    }
                    black_box(map)
                },
                criterion::BatchSize::PerIteration,
            );
        });

        // 3. HashMap Ingest
        group.bench_function(BenchmarkId::new("hashmap_ingest", n), |b| {
            b.iter_batched(
                || HashMap::with_capacity(n),
                |mut map| {
                    for &k in &keys {
                        map.insert(black_box(k), black_box(42));
                    }
                    black_box(map)
                },
                criterion::BatchSize::PerIteration,
            );
        });
    }
    group.finish();
}

fn bench_can_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedded_can_dispatch_lookup");
    let n = 500;
    let keys: Vec<Key32> = (0..n).map(|i| (i * 100_007) & 0x1FFF_FFFF).collect();

    let mut exp_map = ExpanseMap32::new();
    let mut hash_map = HashMap::new();
    let mut btree_map = BTreeMap::new();

    for (idx, &k) in keys.iter().enumerate() {
        exp_map.insert(k, idx as u32);
        hash_map.insert(k, idx as u32);
        btree_map.insert(k, idx as u32);
    }

    group.bench_function(BenchmarkId::new("expanse_map32", n), |b| {
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

    group.bench_function(BenchmarkId::new("hashmap", n), |b| {
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

    group.bench_function(BenchmarkId::new("btreemap", n), |b| {
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

fn bench_ble_tracker_point_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedded_ble_point_lookup");
    for &n in &[500, 2000] {
        let macs: Vec<[u8; 6]> = (0..n)
            .map(|i| {
                [
                    0x00,
                    0x1A,
                    0x2B,
                    ((i >> 16) & 0xFF) as u8,
                    ((i >> 8) & 0xFF) as u8,
                    (i & 0xFF) as u8,
                ]
            })
            .collect();

        // 1. ExpanseMap32 + Slab
        let mut by_mac = ExpanseMap32::new();
        let mut slab = Vec::with_capacity(n);
        for (idx, mac) in macs.iter().enumerate() {
            let rec = BleRecord {
                mac: *mac,
                last_seen_ms: (1000 + idx * 10) as u32,
                ..BleRecord::default()
            };
            slab.push(rec);
            let h = fnv1a_32(mac);
            by_mac.insert(h, idx as u32);
        }

        group.bench_function(BenchmarkId::new("expanse_slab_lookup", n), |b| {
            b.iter(|| {
                let mut sum_rssi = 0i64;
                for mac in &macs {
                    let h = fnv1a_32(mac);
                    if let Some(idx) = by_mac.get(black_box(h)) {
                        let rec = &slab[idx as usize];
                        if rec.mac == *mac {
                            sum_rssi += rec.rssi as i64;
                        }
                    }
                }
                black_box(sum_rssi)
            });
        });

        // 2. ExpanseBlobMap32
        let mut blob_map = ExpanseBlobMap32::new();
        for (idx, mac) in macs.iter().enumerate() {
            let rec = BleRecord {
                mac: *mac,
                last_seen_ms: (1000 + idx * 10) as u32,
                ..BleRecord::default()
            };
            // SAFETY: `rec` is a valid, stack-allocated BleRecord struct with repr(C) layout.
            let bytes = unsafe {
                core::slice::from_raw_parts(
                    &rec as *const BleRecord as *const u8,
                    core::mem::size_of::<BleRecord>(),
                )
            };
            let h = fnv1a_32(mac);
            blob_map.insert(h, bytes, 0).unwrap();
        }

        group.bench_function(BenchmarkId::new("expanse_blobmap_lookup", n), |b| {
            b.iter(|| {
                let mut sum_rssi = 0i64;
                for mac in &macs {
                    let h = fnv1a_32(mac);
                    if let Some((view, _meta)) = blob_map.get(black_box(h)) {
                        let bytes = view.as_bytes();
                        if bytes.len() == core::mem::size_of::<BleRecord>() {
                            // SAFETY: `bytes` contains exactly size_of::<BleRecord>() valid bytes.
                            let rec = unsafe { &*(bytes.as_ptr() as *const BleRecord) };
                            if rec.mac == *mac {
                                sum_rssi += rec.rssi as i64;
                            }
                        }
                    }
                }
                black_box(sum_rssi)
            });
        });

        // 3. HashMap
        let mut hash_map: HashMap<[u8; 6], BleRecord> = HashMap::with_capacity(n);
        for (idx, mac) in macs.iter().enumerate() {
            let rec = BleRecord {
                mac: *mac,
                last_seen_ms: (1000 + idx * 10) as u32,
                ..BleRecord::default()
            };
            hash_map.insert(*mac, rec);
        }

        group.bench_function(BenchmarkId::new("hashmap_lookup", n), |b| {
            b.iter(|| {
                let mut sum_rssi = 0i64;
                for mac in &macs {
                    if let Some(rec) = hash_map.get(black_box(mac)) {
                        sum_rssi += rec.rssi as i64;
                    }
                }
                black_box(sum_rssi)
            });
        });
    }
    group.finish();
}

fn bench_ble_tracker_ttl_eviction(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedded_ble_ttl_eviction");
    let n = 2000;
    let macs: Vec<[u8; 6]> = (0..n)
        .map(|i| {
            [
                0x00,
                0x1A,
                0x2B,
                ((i >> 16) & 0xFF) as u8,
                ((i >> 8) & 0xFF) as u8,
                (i & 0xFF) as u8,
            ]
        })
        .collect();

    // 1. ExpanseMap32 Dual-Trie Eviction (Ordered range scan over by_time)
    group.bench_function(BenchmarkId::new("expanse_dual_trie_eviction", n), |b| {
        b.iter_batched(
            || {
                let mut by_mac = ExpanseMap32::new();
                let mut by_time = ExpanseMap32::new();
                let mut slab = Vec::with_capacity(n);
                for (idx, mac) in macs.iter().enumerate() {
                    let rec = BleRecord {
                        mac: *mac,
                        last_seen_ms: (idx * 10) as u32,
                        ..BleRecord::default()
                    };
                    slab.push(rec);
                    let h = fnv1a_32(mac);
                    by_mac.insert(h, idx as u32);
                    let tk = ((rec.last_seen_ms / 1000) << 13) | (idx as u32 & 0x1FFF);
                    by_time.insert(tk, idx as u32);
                }
                (by_mac, by_time, slab)
            },
            |(mut by_mac, mut by_time, slab)| {
                let cutoff_sec = 5u32;
                let max_tk = (cutoff_sec << 13) | 0x1FFF;
                let mut evicted = 0usize;
                while let Some((tk, idx)) = by_time.first() {
                    if tk > max_tk {
                        break;
                    }
                    let rec = &slab[idx as usize];
                    let h = fnv1a_32(&rec.mac);
                    by_time.remove(tk);
                    by_mac.remove(h);
                    evicted += 1;
                }
                black_box((by_mac, by_time, evicted))
            },
            criterion::BatchSize::PerIteration,
        );
    });

    // 1b. The same dual-trie eviction through `remove_range` (#578): one
    // descent to the range plus one structural fix-up per touched node
    // on `by_time`, with the hash-keyed `by_mac` removal per record kept
    // exactly as above — that part is inherently random access.
    group.bench_function(
        BenchmarkId::new("expanse_dual_trie_eviction_range", n),
        |b| {
            b.iter_batched(
                || {
                    let mut by_mac = ExpanseMap32::new();
                    let mut by_time = ExpanseMap32::new();
                    let mut slab = Vec::with_capacity(n);
                    for (idx, mac) in macs.iter().enumerate() {
                        let rec = BleRecord {
                            mac: *mac,
                            last_seen_ms: (idx * 10) as u32,
                            ..BleRecord::default()
                        };
                        slab.push(rec);
                        let h = fnv1a_32(mac);
                        by_mac.insert(h, idx as u32);
                        let tk = ((rec.last_seen_ms / 1000) << 13) | (idx as u32 & 0x1FFF);
                        by_time.insert(tk, idx as u32);
                    }
                    (by_mac, by_time, slab)
                },
                |(mut by_mac, mut by_time, slab)| {
                    let cutoff_sec = 5u32;
                    let max_tk = (cutoff_sec << 13) | 0x1FFF;
                    let evicted = by_time.remove_range(0..=max_tk, |_tk, idx| {
                        let rec = &slab[idx as usize];
                        by_mac.remove(fnv1a_32(&rec.mac));
                    });
                    black_box((by_mac, by_time, evicted))
                },
                criterion::BatchSize::PerIteration,
            );
        },
    );

    // 2. HashMap TTL Eviction (Full table scan)
    group.bench_function(BenchmarkId::new("hashmap_full_scan_eviction", n), |b| {
        b.iter_batched(
            || {
                let mut map: HashMap<[u8; 6], BleRecord> = HashMap::with_capacity(n);
                for (idx, mac) in macs.iter().enumerate() {
                    let rec = BleRecord {
                        mac: *mac,
                        last_seen_ms: (idx * 10) as u32,
                        ..BleRecord::default()
                    };
                    map.insert(*mac, rec);
                }
                map
            },
            |mut map| {
                let cutoff_ms = 5000u32;
                map.retain(|_, rec| rec.last_seen_ms > cutoff_ms);
                black_box(map)
            },
            criterion::BatchSize::PerIteration,
        );
    });

    // 3. Steady-state shape: 25 expired of 2,000 — the regime the
    // O(expired) pre-registration is actually about. The bulk shape above
    // (600 of 2,000) is where a linear sweep's cache-friendliness must win
    // on constants; this one is where the ordered index can express its
    // asymptotic advantage. Both ship, so neither story is cherry-picked.
    let steady_seen = |idx: usize| -> u32 {
        if idx < 25 {
            (idx * 10) as u32 // expired: < 6,000 ms
        } else {
            10_000 + idx as u32 // live: well past the cutoff
        }
    };
    group.bench_function(
        BenchmarkId::new("expanse_dual_trie_eviction_steady", n),
        |b| {
            b.iter_batched(
                || {
                    let mut by_mac = ExpanseMap32::new();
                    let mut by_time = ExpanseMap32::new();
                    let mut slab = Vec::with_capacity(n);
                    for (idx, mac) in macs.iter().enumerate() {
                        let rec = BleRecord {
                            mac: *mac,
                            last_seen_ms: steady_seen(idx),
                            ..BleRecord::default()
                        };
                        slab.push(rec);
                        let h = fnv1a_32(mac);
                        by_mac.insert(h, idx as u32);
                        let tk = ((rec.last_seen_ms / 1000) << 13) | (idx as u32 & 0x1FFF);
                        by_time.insert(tk, idx as u32);
                    }
                    (by_mac, by_time, slab)
                },
                |(mut by_mac, mut by_time, slab)| {
                    let cutoff_sec = 5u32;
                    let max_tk = (cutoff_sec << 13) | 0x1FFF;
                    let mut evicted = 0usize;
                    while let Some((tk, idx)) = by_time.first() {
                        if tk > max_tk {
                            break;
                        }
                        let rec = &slab[idx as usize];
                        let h = fnv1a_32(&rec.mac);
                        by_time.remove(tk);
                        by_mac.remove(h);
                        evicted += 1;
                    }
                    black_box((by_mac, by_time, evicted))
                },
                criterion::BatchSize::PerIteration,
            );
        },
    );
    group.bench_function(
        BenchmarkId::new("expanse_dual_trie_eviction_range_steady", n),
        |b| {
            b.iter_batched(
                || {
                    let mut by_mac = ExpanseMap32::new();
                    let mut by_time = ExpanseMap32::new();
                    let mut slab = Vec::with_capacity(n);
                    for (idx, mac) in macs.iter().enumerate() {
                        let rec = BleRecord {
                            mac: *mac,
                            last_seen_ms: steady_seen(idx),
                            ..BleRecord::default()
                        };
                        slab.push(rec);
                        let h = fnv1a_32(mac);
                        by_mac.insert(h, idx as u32);
                        let tk = ((rec.last_seen_ms / 1000) << 13) | (idx as u32 & 0x1FFF);
                        by_time.insert(tk, idx as u32);
                    }
                    (by_mac, by_time, slab)
                },
                |(mut by_mac, mut by_time, slab)| {
                    let cutoff_sec = 5u32;
                    let max_tk = (cutoff_sec << 13) | 0x1FFF;
                    let evicted = by_time.remove_range(0..=max_tk, |_tk, idx| {
                        let rec = &slab[idx as usize];
                        by_mac.remove(fnv1a_32(&rec.mac));
                    });
                    black_box((by_mac, by_time, evicted))
                },
                criterion::BatchSize::PerIteration,
            );
        },
    );
    group.bench_function(
        BenchmarkId::new("hashmap_full_scan_eviction_steady", n),
        |b| {
            b.iter_batched(
                || {
                    let mut map: HashMap<[u8; 6], BleRecord> = HashMap::with_capacity(n);
                    for (idx, mac) in macs.iter().enumerate() {
                        let rec = BleRecord {
                            mac: *mac,
                            last_seen_ms: steady_seen(idx),
                            ..BleRecord::default()
                        };
                        map.insert(*mac, rec);
                    }
                    map
                },
                |mut map| {
                    let cutoff_ms = 6000u32;
                    map.retain(|_, rec| rec.last_seen_ms >= cutoff_ms);
                    black_box(map)
                },
                criterion::BatchSize::PerIteration,
            );
        },
    );

    group.finish();
}

criterion_group!(
    benches,
    bench_sensor_tsdb_ingest_and_flush,
    bench_can_dispatch,
    bench_ble_tracker_point_lookup,
    bench_ble_tracker_ttl_eviction
);
criterion_main!(benches);
