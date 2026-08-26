//! Bench harness for the #295 scalar-metadata-sidecar POC.
//!
//! Gated behind the non-default `poc-meta-sidecar` feature (see Cargo.toml
//! `required-features`), so it never builds in the default CI path and cannot
//! perturb the existing `large_values` benches. Run on a quiet dedicated host
//! per `docs/BENCHMARKING.md` (interleaved A/B arms, load snapshots):
//!
//! ```text
//! cargo bench -p expanse-trie --features poc-meta-sidecar --bench poc_sidecar_295
//! ```
//!
//! ## What it measures (RFC §5.6)
//!
//! * `sidecar_cold_dram` — the pre-registered parity test. Same 260 MiB / 262k
//!   / 1 KiB shuffled-insert cold-DRAM arena as
//!   `large_values::bench_predicate_scan_cold_dram_large` (the Phase-1
//!   baseline). Three arms per σ: `phase1_in_slot` (shipped 24-bit in-slot
//!   `ArenaMeta` read), `sidecar` (warm decoupled `[u32;1]` array read), and
//!   `naive_row_deref` (touch every payload — the payload-fetch baseline that
//!   Phase 1 measured at ~10.3×/~22×). Metadata is kept ≤ 24-bit so the
//!   Phase-1 arm can express it, isolating the *mechanism* (in-slot vs sidecar)
//!   at parity.
//! * `sidecar_cold_dram_xllc` — the H3 residency-cliff arm, σ ∈ {0.001, 0.05},
//!   three cells sizing the sidecar `meta[]` array across the reference 30 MiB
//!   L3: N=1M/K=3/256 B (12 MiB, warm boundary), N=3M/K=3/256 B (36 MiB,
//!   straddling), N=6M/K=4/128 B (96 MiB, ≈3.2× L3, spilled). Payload size is
//!   dropped so every arena stays under the shipped 1 GiB cap (a declared
//!   scaled proxy for a 1 KiB / multi-GiB arena), so the cliff is *measured*.
//! * `sidecar_compaction` — compaction time, sidecar (rewrite dense `offsets`)
//!   vs Phase-1 (rewrite a trie value slot per live record).
//! * `sidecar_write_path` — build (insert) throughput, sidecar vs Phase-1.
//! * `inverted_index` — `intersection` / `intersection_len` query latency and
//!   ts range-query (exact vs bucketed) latency.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use expanse_trie::blobmap::ExpanseBlobMap;
use expanse_trie::poc_sidecar::{InvertedIndex, SidecarBlobMap};
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

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

/// Cold-DRAM predicate scan: Phase-1 in-slot vs sidecar vs payload-fetch
/// baseline, over a > LLC arena (pre-registered expectation: phase1 ≈ sidecar).
fn bench_sidecar_cold_dram(c: &mut Criterion) {
    const PAYLOAD: usize = 1024;
    const N: u64 = 262_144; // 260 MiB arena ≈ 8.7× the 30 MiB reference LLC.
    const META_RANGE: u32 = 10_000; // ≤ 24-bit so Phase 1 can hold it.

    // Shuffled insertion order so an ascending-key scan hits arena offsets
    // randomly (defeats the prefetcher), matching the Phase-1 cold-DRAM harness.
    let mut order: Vec<u64> = (0..N).collect();
    let mut rng = XorShift(0x0BAD_F00D_C0FF_EE11);
    for i in (1..order.len()).rev() {
        let j = (rng.next() % (i as u64 + 1)) as usize;
        order.swap(i, j);
    }

    let mut phase1 = ExpanseBlobMap::with_chunk_size(64 * 1024 * 1024);
    let mut sidecar = SidecarBlobMap::<1>::with_chunk_size(64 * 1024 * 1024);
    for &k in &order {
        let meta = (k.wrapping_mul(GOLDEN) % META_RANGE as u64) as u32;
        let payload = vec![(k & 0xFF) as u8; PAYLOAD];
        phase1.insert(k, &payload, meta).unwrap();
        sidecar.insert(k, &payload, [meta]).unwrap();
    }

    let selectivities = [
        ("sigma_0.001", 10u32),
        ("sigma_0.05", 500u32),
        ("sigma_0.20", 2000u32),
        ("sigma_1.0", 10000u32),
    ];

    let mut g = c.benchmark_group("sidecar_cold_dram");
    g.sample_size(10);

    for (label, threshold) in selectivities {
        g.bench_function(BenchmarkId::new("phase1_in_slot", label), |b| {
            b.iter(|| {
                let mut matches = 0usize;
                let mut byte_sum = 0u64;
                phase1.scan_filtered(
                    0..=N,
                    |_k, meta| meta <= threshold,
                    |_k, view, _m| {
                        byte_sum += view.as_bytes()[0] as u64;
                        matches += 1;
                        true
                    },
                );
                black_box((matches, byte_sum))
            });
        });

        g.bench_function(BenchmarkId::new("sidecar", label), |b| {
            b.iter(|| {
                let mut matches = 0usize;
                let mut byte_sum = 0u64;
                sidecar.scan_filtered(
                    0..=N,
                    |_k, cols| cols[0] <= threshold,
                    |_k, payload, _c| {
                        byte_sum += payload[0] as u64;
                        matches += 1;
                        true
                    },
                );
                black_box((matches, byte_sum))
            });
        });

        // Payload-fetch baseline (touch every payload) — the ~10.3×/~22×
        // reference. Measured on the sidecar map (identical arena to Phase 1).
        g.bench_function(BenchmarkId::new("naive_row_deref", label), |b| {
            b.iter(|| {
                let mut matches = 0usize;
                let mut byte_sum = 0u64;
                sidecar.scan_filtered(
                    0..=N,
                    |_k, _c| true,
                    |_k, payload, cols| {
                        byte_sum += payload[0] as u64;
                        if cols[0] <= threshold {
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

/// One `>`LLC cell (RFC §5.6 H3): builds a Phase-1 map and a `K`-column sidecar
/// over the same N shuffled keys / `payload_bytes` payloads, then benches the
/// three arms at σ ∈ {0.001, 0.05}. `K` and `payload_bytes` are chosen per cell
/// (see [`bench_sidecar_cold_dram_xllc`]) so the sidecar `meta[]` array
/// (`4·K·N` bytes) straddles or exceeds the reference 30 MiB L3 while the
/// payload arena stays under the shipped 1 GiB `MAX_ARENA_CAPACITY` cap.
fn xllc_cell<const K: usize>(
    g: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    n: u64,
    payload_bytes: usize,
) {
    const META_RANGE: u32 = 10_000; // ≤ 24-bit so the Phase-1 arm can hold col 0.

    // Shuffled insertion order: handles become insert-ordered, decorrelated from
    // key order, so an ascending-key scan reads BOTH the payload arena AND the
    // sidecar meta[] in permuted order (defeats the prefetcher on each stream).
    let mut order: Vec<u64> = (0..n).collect();
    let mut rng = XorShift(0x0BAD_F00D_C0FF_EE11 ^ n ^ (K as u64));
    for i in (1..order.len()).rev() {
        let j = (rng.next() % (i as u64 + 1)) as usize;
        order.swap(i, j);
    }

    let mut phase1 = ExpanseBlobMap::with_chunk_size(64 * 1024 * 1024);
    let mut sidecar = SidecarBlobMap::<K>::with_chunk_size(64 * 1024 * 1024);
    for &k in &order {
        let meta = (k.wrapping_mul(GOLDEN) % META_RANGE as u64) as u32;
        let payload = vec![(k & 0xFF) as u8; payload_bytes];
        phase1.insert(k, &payload, meta).unwrap();
        // Column 0 is the predicate field; the rest are filler so meta[] is the
        // full 4·K bytes/entry the residency cliff depends on.
        let mut cols = [0u32; K];
        cols[0] = meta;
        for (c, slot) in cols.iter_mut().enumerate().skip(1) {
            *slot = (k % (c as u64 + 2)) as u32;
        }
        sidecar.insert(k, &payload, cols).unwrap();
    }
    let record = (payload_bytes + 8).div_ceil(16) * 16;
    eprintln!(
        "xllc N={n} K={K}: sidecar meta[] ≈ {:.1} MiB ({:.2}× L3), payload arena ≈ {:.0} MiB ({payload_bytes} B payloads)",
        (n as usize * 4 * K) as f64 / 1048576.0,
        (n as usize * 4 * K) as f64 / (30.0 * 1048576.0),
        (n as usize * record) as f64 / 1048576.0
    );

    for (label, threshold) in [("sigma_0.001", 10u32), ("sigma_0.05", 500u32)] {
        let id = format!("{n}_{label}");
        g.bench_function(BenchmarkId::new("phase1_in_slot", &id), |b| {
            b.iter(|| {
                let mut matches = 0usize;
                let mut byte_sum = 0u64;
                phase1.scan_filtered(
                    0..=n,
                    |_k, meta| meta <= threshold,
                    |_k, view, _m| {
                        byte_sum += view.as_bytes()[0] as u64;
                        matches += 1;
                        true
                    },
                );
                black_box((matches, byte_sum))
            });
        });
        g.bench_function(BenchmarkId::new("sidecar", &id), |b| {
            b.iter(|| {
                let mut matches = 0usize;
                let mut byte_sum = 0u64;
                sidecar.scan_filtered(
                    0..=n,
                    |_k, cols| cols[0] <= threshold,
                    |_k, payload, _c| {
                        byte_sum += payload[0] as u64;
                        matches += 1;
                        true
                    },
                );
                black_box((matches, byte_sum))
            });
        });
        g.bench_function(BenchmarkId::new("naive_row_deref", &id), |b| {
            b.iter(|| {
                let mut matches = 0usize;
                let mut byte_sum = 0u64;
                sidecar.scan_filtered(
                    0..=n,
                    |_k, _c| true,
                    |_k, payload, cols| {
                        byte_sum += payload[0] as u64;
                        if cols[0] <= threshold {
                            matches += 1;
                        }
                        true
                    },
                );
                black_box((matches, byte_sum))
            });
        });
    }
}

/// The `>`LLC arm (RFC §5.6 H3 — the residency cliff, *measured* not predicted).
///
/// The sidecar's warm-read advantage holds only while its per-entry
/// `meta: [u32; K]` array is LLC-resident. Because record handles are assigned
/// in **insert** order, a **key-ordered** scan reads `meta[rid]` in a permuted
/// order, so once that array exceeds the LLC it becomes a *second* cold-DRAM
/// stream that Phase-1 in-slot metadata (which rides in the trie leaf the walk
/// already touches) never pays.
///
/// A literal 1 KiB payload at these N would put the arena at multiple GiB, over
/// the shipped 1 GiB `MAX_ARENA_CAPACITY` cap (which is Phase-1 code, out of
/// scope to edit) — so this is a **declared scaled proxy**: payload size is
/// dropped to keep every arena `<` 1 GiB while `K` is chosen so the `meta[]`
/// array (the cliff driver, `4·K·N` bytes) straddles / exceeds the 30 MiB L3.
/// Three cells (all reference L3 = 30 MiB):
/// * N = 1M, `K = 3`, 256 B → `meta[]` ≈ 12 MiB (< L3, warm boundary), arena ≈ 272 MiB.
/// * N = 3M, `K = 3`, 256 B → `meta[]` ≈ 36 MiB (≈ 1.2× L3, straddling), arena ≈ 816 MiB.
/// * N = 6M, `K = 4`, 128 B → `meta[]` ≈ 96 MiB (≈ 3.2× L3, spilled), arena ≈ 896 MiB.
///
/// The Phase-1 arm carries a single 24-bit in-slot field (it cannot express
/// `K > 1`); the sidecar predicate reads column 0 only, so both scan the same
/// keys/payloads and the only difference is metadata-read locality — exactly
/// what H3 isolates. σ ∈ {0.001, 0.05} as pre-registered.
fn bench_sidecar_cold_dram_xllc(c: &mut Criterion) {
    let mut g = c.benchmark_group("sidecar_cold_dram_xllc");
    g.sample_size(10);
    xllc_cell::<3>(&mut g, 1_000_000, 256);
    xllc_cell::<3>(&mut g, 3_000_000, 256);
    xllc_cell::<4>(&mut g, 6_000_000, 128);
    g.finish();
}

/// Compaction cost after a 50% delete churn: sidecar (dense offset rewrite, no
/// trie writes) vs Phase-1 (per-record trie value-slot rewrite).
fn bench_sidecar_compaction(c: &mut Criterion) {
    let mut g = c.benchmark_group("sidecar_compaction");
    let sizes = [20_000u64, 100_000u64];

    for &n in &sizes {
        g.bench_with_input(BenchmarkId::new("phase1", n), &n, |b, &n| {
            b.iter_batched(
                || {
                    let mut map = ExpanseBlobMap::with_chunk_size(8 * 1024 * 1024);
                    for k in 0..n {
                        let payload = vec![(k & 0xFF) as u8; 256];
                        map.insert(k, &payload, k as u32).unwrap();
                    }
                    for k in 0..(n / 2) {
                        map.remove(k);
                    }
                    map
                },
                |mut map| black_box(map.compact().unwrap()),
                criterion::BatchSize::LargeInput,
            );
        });

        g.bench_with_input(BenchmarkId::new("sidecar", n), &n, |b, &n| {
            b.iter_batched(
                || {
                    let mut map = SidecarBlobMap::<1>::with_chunk_size(8 * 1024 * 1024);
                    for k in 0..n {
                        let payload = vec![(k & 0xFF) as u8; 256];
                        map.insert(k, &payload, [k as u32]).unwrap();
                    }
                    for k in 0..(n / 2) {
                        map.remove(k);
                    }
                    map
                },
                |mut map| black_box(map.compact().unwrap()),
                criterion::BatchSize::LargeInput,
            );
        });
    }
    g.finish();
}

/// Write-path (build) throughput: sidecar vs Phase-1 over identical inserts.
fn bench_sidecar_write_path(c: &mut Criterion) {
    let mut g = c.benchmark_group("sidecar_write_path");
    const N: u64 = 50_000;

    g.bench_function("phase1_insert", |b| {
        b.iter(|| {
            let mut map = ExpanseBlobMap::with_chunk_size(8 * 1024 * 1024);
            for k in 0..N {
                let payload = vec![(k & 0xFF) as u8; 64];
                map.insert(k, &payload, (k % 10_000) as u32).unwrap();
            }
            black_box(map.len())
        });
    });

    g.bench_function("sidecar_insert", |b| {
        b.iter(|| {
            let mut map = SidecarBlobMap::<1>::with_chunk_size(8 * 1024 * 1024);
            for k in 0..N {
                let payload = vec![(k & 0xFF) as u8; 64];
                map.insert(k, &payload, [(k % 10_000) as u32]).unwrap();
            }
            black_box(map.len())
        });
    });
    g.finish();
}

/// Inverted-index query latency: set intersection (materialize + count) and ts
/// range query (exact vs bucketed).
fn bench_inverted_index(c: &mut Criterion) {
    const N: u64 = 262_144;
    let mut idx = InvertedIndex::new(12); // 4096-wide ts buckets
    let mut rng = XorShift(0x5EED);
    let mut ts_of: Vec<u32> = vec![0; N as usize];
    for k in 0..N {
        let tenant = (rng.next() % 64) as u32;
        let status = (rng.next() % 4) as u32;
        let ts = (rng.next() % 2_000_000) as u32;
        idx.insert(k, tenant, status, ts);
        ts_of[k as usize] = ts;
    }

    let mut g = c.benchmark_group("inverted_index");

    g.bench_function("intersection_materialize_tenant_status", |b| {
        b.iter(|| black_box(idx.query_tenant_status(3, 1).len()));
    });
    g.bench_function("intersection_len_tenant_status", |b| {
        b.iter(|| black_box(idx.count_tenant_status(3, 1)));
    });

    // ts range covering ~5% of the domain.
    let (lo, hi) = (500_000u32, 600_000u32);
    g.bench_function("ts_range_exact", |b| {
        b.iter(|| black_box(idx.query_ts_range_exact(lo, hi).len()));
    });
    let ts_lookup = |k: u64| ts_of[k as usize];
    g.bench_function("ts_range_bucketed", |b| {
        b.iter(|| black_box(idx.query_ts_range_bucketed(lo, hi, ts_lookup).len()));
    });
    g.finish();
}

criterion_group!(
    benches,
    bench_sidecar_cold_dram,
    bench_sidecar_cold_dram_xllc,
    bench_sidecar_compaction,
    bench_sidecar_write_path,
    bench_inverted_index
);
criterion_main!(benches);
