# Adaptive Radix Tree (ART) vs. Expanse (`ExpanseMap`): Empirical Comparative Methodology

## 1. Executive Summary & Problem Statement

In in-memory database indexing and high-performance associative structures, radix tries are chosen for their $O(k)$ key-length descent, ordered prefix traversal, and absence of catastrophic rehashing.

Two primary architectural philosophies represent the state of the art:

1. **Adaptive Radix Tree (ART)** (Viktor Leis, Alfons Kemper, Thomas Neumann, ICDE 2013, DOI: [`10.1109/ICDE.2013.6544812`](https://doi.org/10.1109/ICDE.2013.6544812)):
   - Partitions by **byte expanse** (1 byte = 8 bits per level).
   - Uses adaptive inner node fanouts (`Node4`, `Node16`, `Node48`, `Node256`) to balance branching density against allocation footprint.
   - Utilizes path compression (pessimistic / optimistic prefix matching) and lazy expansion to collapse one-way pointer chains.
   - Evaluated using the primary Rust twin: **`blart` (v0.5.0, MSRV 1.85)** with `blart::TreeMap<Mapped<ToUBE, u64>, u64>`, providing zero-allocation big-endian integer keys.

2. **Expanse (`ExpanseMap`)**:
   - Clean-room Judy array modernized for current superscalar and SIMD architectures.
   - Compact machine-word descriptors: 16-byte `Edge` holding up to 7 immediate keys/values with 0 heap allocation.
   - Dynamic subexpanse compression: transitions dynamically between Immediates, Linear branches (`BranchL3`, `BranchL7`), POPCNT-indexed Bitmap nodes (`BranchB`, `LeafBitmap1`, `LeafBitmapL`), and Uncompressed branches (`BranchU`).

This benchmark suite delivers a reproducible, empirical head-to-head evaluation of `ExpanseMap` against `blart` (ART), `std::collections::BTreeMap`, and `hashbrown::HashMap`.

---

## 2. Pre-Registration & Expected Losses Matrix

Per `~/.claude/RESEARCH_DISCIPLINES.md` Rules 2 (Pre-registration), 3 (Fair twin with winning regime), and 22 (Engineering plumbing lighter track, novelty tier `Engineering`):

| Workload / Regime | Expected Winner | Primary Mechanism & Structural Rationale |
|---|---|---|
| **Sparse 64-bit Uniform Random Point Lookup ($N = 10^6$)** | **ART (`blart`)** | ART's path compression and lazy expansion skip unpopulated intermediate levels in 2–3 memory hops; Expanse traverses multi-level expanse branches when population spans multiple byte levels. |
| **Uniform Random Dynamic Insertion ($N = 10^6$)** | **ART (`blart`)** | Sparse random inserts into ART require only small Node4/Node16 allocations with minimal branch copying; Expanse manages larger expanse chunk migrations. |
| **Clustered / Sequential Point Lookup** | **`ExpanseMap`** | Expanse bitmap leaves (256-bit POPCNT rank) and uncompressed branches pack contiguous spans with 0 pointer indirection. |
| **Small Payloads ($\le 7$ keys, Immediates)** | **`ExpanseMap`** | Expanse packs up to immediate capacity directly inside the 16-byte `Edge` with **0 heap allocations**; ART always allocates heap nodes (minimum `InnerNode4` = 64 bytes). |
| **Dense Ordered Range Scan ($N = 10^6$)** | **`ExpanseMap`** | Contiguous 64-byte/256-bit leaf array traversals and zero-allocation stack iterator beat ART's node-array pointer chasing. |
| **Dense Integer Memory Footprint ($N = 10^6$)** | **`ExpanseMap`** | *(workloads differ: capi_bench_vs_libjudy vs art_memory)*: `ExpanseMap` achieves 8.56 B/key on sequential and 8.61 B/key on clustered 1M *(measured: reference host, commit 43b46f38, `docs/BENCHMARKING.md:1434–1436`)*; `blart` requires $\ge 40.1$ B/key *(derived: 32 B `LeafNode` + $2072/256$ B `InnerNode256` share, `scripts/art_envelope.py`)*. |
| **Clustered Memory Footprint ($N = 10^6$)** | **`ExpanseMap`** | *(workloads differ: capi_bench_vs_libjudy vs art_memory)*: `ExpanseMap` achieves 8.61 B/key *(measured: reference host, commit 43b46f38, `docs/BENCHMARKING.md:1436`)*; `blart` requires $\ge 42.5$ B/key *(derived: 32 B `LeafNode` + $168/16$ B `InnerNode16` share, `scripts/art_envelope.py`)*. |
| **Sparse Memory Footprint ($N = 10^6$)** | **`ExpanseMap`** | *(workloads differ: capi_bench_vs_libjudy vs art_memory)*: `ExpanseMap` achieves 16.70 B/key on random 1M *(measured: reference host, commit 43b46f38, `docs/BENCHMARKING.md:1435`)*; `blart` requires $\ge 48.0$ B/key *(derived: 32 B `LeafNode` + $64/4$ B `InnerNode4` share, `scripts/art_envelope.py`)*. *(Note: no stride-$2^{32}$ cell is measured in capi; Pillar 4 sparse generator is a distinct workload).* |

### Claims Ceiling
- `ExpanseMap` does **not** claim global dominance over ART.
- The claim ceiling is: *`ExpanseMap` wins dense/clustered point lookup, range scans, and all memory footprint regimes against `blart` (whose separate `LeafNode` allocation imposes a 32 B/key structural floor); ART wins sparse uniform random lookup and sparse insertion throughput via path compression*.

---

## 3. The 5-Pillar Benchmark Suite

### Pillar 1: Point Lookup (100% Hit Rate) (`art_lookup_hit`)
- Point lookup latency across five key geometries: Sequential, Clustered (dense 1024-key Gaussian clusters), Uniform Random (high-entropy 64-bit words), Sparse Stride ($2^{32}$ stride), and Zipfian ($\theta = 0.99$).
- Populations: $10^4, 10^5, 10^6$ keys.

### Pillar 2: Point Lookup (50% Hit / 50% Miss Rate) (`art_lookup_miss`)
- Interleaved lookup stream consisting of 50% present keys and 50% absent keys generated via rejection sampling per `AGENTS.md §8.6`.

### Pillar 3: Dynamic Insertion Throughput (`art_insert`)
- Dynamic table growth from $0 \to N$ keys without pre-allocation.
- Measures only the clean insertion loop, strictly isolating teardown and dropping.

### Pillar 4: Ordered Range Scan & Iteration (`art_scan`)
- Full container in-order iteration (`iter()`) and bounded range scans ($k \in [10, 100, 1000]$ items).

### Pillar 5: Memory Allocation & Footprint Census (`art_memory`)
- Exact live heap accounting using a custom `GlobalAlloc` hook (`TrackingAlloc`) across populations $10^3, 10^4, 10^5, 10^6$.

---

## 4. Workload Shape Audit Tables

### `art_lookup_hit`
| Property | Value |
|---|---|
| `workload_id` | `art_lookup_hit` |
| `group` | 4 |
| `population` | 10k to 1M |
| `probes_and_reuse` | Batch stream |
| `hit_rate` | 100% Hit |
| `miss_gen_method` | None |
| `value_dereference` | `black_box(*val)` |
| `measured_region` | Clean lookup loop |
| `arm_symmetry` | Symmetric keys and PRNG |
| `statistics` | Median of interleaved rounds |
| `verdict` | **PASS** `[verified: CODE READ]`: ART comparison point lookup hit benchmark. |

### `art_lookup_miss`
| Property | Value |
|---|---|
| `workload_id` | `art_lookup_miss` |
| `group` | 4 |
| `population` | 10k to 1M |
| `probes_and_reuse` | 50/50 mixed probe stream |
| `hit_rate` | 50% Hit / 50% Miss |
| `miss_gen_method` | Rejection sampling |
| `value_dereference` | `black_box(*val)` |
| `measured_region` | Clean lookup loop |
| `arm_symmetry` | Symmetric keys and PRNG |
| `statistics` | Median of interleaved rounds |
| `verdict` | **PASS** `[verified: CODE READ]`: ART comparison point lookup 50/50 miss benchmark. |

### `art_insert`
| Property | Value |
|---|---|
| `workload_id` | `art_insert` |
| `group` | 4 |
| `population` | 10k to 1M |
| `probes_and_reuse` | Insertion stream |
| `hit_rate` | Dynamic Growth |
| `miss_gen_method` | None |
| `value_dereference` | Insertion into container |
| `measured_region` | Clean insertion loop |
| `arm_symmetry` | Symmetric keys and PRNG |
| `statistics` | Median of interleaved rounds |
| `verdict` | **PASS** `[verified: CODE READ]`: ART comparison insertion throughput benchmark. |

### `art_scan`
| Property | Value |
|---|---|
| `workload_id` | `art_scan` |
| `group` | 4 |
| `population` | 10k to 1M |
| `probes_and_reuse` | Range scan stream |
| `hit_rate` | Ordered Scan |
| `miss_gen_method` | None |
| `value_dereference` | `black_box(*val)` |
| `measured_region` | Clean scan loop |
| `arm_symmetry` | Symmetric keys and PRNG |
| `statistics` | Median of interleaved rounds |
| `verdict` | **PASS** `[verified: CODE READ]`: ART comparison range scan benchmark. |

### `art_memory`
| Property | Value |
|---|---|
| `workload_id` | `art_memory` |
| `group` | 4 |
| `population` | 1k to 1M |
| `probes_and_reuse` | N/A (Memory) |
| `hit_rate` | N/A |
| `miss_gen_method` | None |
| `value_dereference` | Live bytes tracked |
| `measured_region` | Clean GlobalAlloc hook |
| `arm_symmetry` | Symmetric keys and PRNG |
| `statistics` | Exact byte count |
| `verdict` | **PASS** `[verified: CODE READ]`: ART comparison memory footprint benchmark. |

---

## 5. Amendments after Pre-Registration (2026-09-02)

Per `AGENTS.md §8.12` and Research Disciplines amendment rules, methodological and harness adjustments made after initial pre-registration are explicitly recorded below without modifying §1–§4 in place:

1. **Probe Stream Shuffling in Point Lookups (`art_lookup_hit`, `art_lookup_miss`)**:
   - *Adjustment*: Point lookup probe access was modified from sequential insertion order to a deterministic shuffled probe stream using `PROBE_SHUFFLE_SEED = 0x5EED_511F_F1E0_0001`.
   - *Rationale*: Sequential probe streams introduce CPU hardware branch predictor and prefetch cache-line artifacts (the #454 defect class), distorting true random vs structured access memory latency.
   - *Status*: All point lookup timing verdicts in `README.md` are **measured under amended shape**.

2. **Distribution-Aware Miss Rejection Sampling (`art_lookup_miss`)**:
   - *Adjustment*: Miss generation was updated from uniform random key drawing to same-generator rejection sampling (`gen_distribution_misses`).
   - *Rationale*: Uniform miss keys on structured key distributions (sequential/stride/clustered) terminate prematurely in top-level shallow branches, artificially depressing miss latency.

3. **Statistical Metric Alignment & BCa 95% Confidence Intervals**:
   - *Adjustment*: The benchmark protocol raised sampling to 15 interleaved rounds per cell, recording paired per-round sample series and computing BCa 95% bootstrap confidence intervals (2,000 resamples via `scripts/recompute_and_patch_json.py` / `scripts/bca_bootstrap.py`). Ratios and confidence intervals both evaluate the paired per-round mean ratio, ensuring that every point estimate strictly lies within its interval.

4. **Offline Statistical Verification & Metadata Transcription (`recompute_and_patch_json.py`)**:
   - *Adjustment*: The raw timing samples from the reference-host execution were processed via `scripts/recompute_and_patch_json.py` to derive BCa bootstrap confidence intervals and verify that all point estimates strictly lie within their confidence intervals.
   - *Status*: Host model, kernel version, and load averages (`0.00` start, `0.96` end) were transcribed from the reference-host execution run log into artifact metadata. Zipfian deduplicated unique key counts for lookup pillars (2,911 unique at 10k; 25,144 unique at 100k; 225,853 unique at 1M) were transcribed from the memory census pillar of the same run.
