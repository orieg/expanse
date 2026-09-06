# Expanse vs. Adaptive Radix Tree (ART): Empirical Benchmark Suite

This benchmark suite delivers a reproducible, empirical head-to-head evaluation of **`ExpanseMap`** against the **Adaptive Radix Tree (ART)**, evaluated using pure-Rust **`blart` (v0.5.0)**, alongside **`std::collections::BTreeMap`** and **`hashbrown::HashMap`**.

> **Tracking & Provenance.** Delivers the ART comparison arm of [#387](https://github.com/orieg/expanse/issues/387) and closes the undelivered ART baseline tracking gap from [#122](https://github.com/orieg/expanse/issues/122). All measurements below were captured with full population scaling ($N \in [10\text{k}, 100\text{k}, 1\text{M}]$ for latency; $N \in [1\text{k}, 10\text{k}, 100\text{k}, 1\text{M}]$ for memory census) under isolated execution *(measured: reference host — Intel Core i9-12900F, 8P+8E/24 threads, 30 MiB L3, Ubuntu 22.04, Linux 6.8.0-136-generic; harness commit `b447dbc`; `docs/benchmarks/art_comparison/run.sh` on the host; load average 0.90 at start, 1.25 at end transcribed from run log; 15 rounds/cell, median; BCa 95% CIs in results/)*.
>
> **Amendment & Comparability Disclosure.** Due to the probe-order shuffle amendment and same-distribution miss generator fixes, timing figures from the initial reference-host run before the probe-shuffle amendment are not comparable. A prior exploratory sweep on an Apple Silicon laptop showed ART 1.71× on uniform-random lookup; that run had no load snapshot, coincided with co-resident cargo/ESP-IDF builds, is classified contaminated per `BENCHMARKING.md` rule 2, and carries no timing claim.

---

## 1. Executive Summary Scorecard ($N = 1,000,000$)

```
========================================================================================================
 Workload / Regime                         Pre-Reg Outcome    Observed Winner     Delta / Ratio
========================================================================================================
 Dense Memory Footprint (1M seq)           ExpanseMap         Expanse             Expanse 4.63x less RAM
 Clustered Memory Footprint (1M)           ExpanseMap         Expanse             Expanse 4.58x less RAM
 Sparse Memory Footprint (1M stride)       ExpanseMap         Expanse             Expanse 2.45x less RAM
 Uniform Random Memory (1M)                not pre-reg        Expanse             Expanse 2.36x less RAM
 Zipfian Memory (1M draws, 226k keys)      not pre-reg        Expanse             Expanse 4.35x less RAM
--------------------------------------------------------------------------------------------------------
 Sequential Point Lookup (1M hit)          ExpanseMap         Expanse             Expanse 2.64x faster
 Clustered Point Lookup (1M hit)           ExpanseMap         Expanse             Expanse 1.98x faster
 Sparse Stride Point Lookup (1M hit)       not pre-reg        Expanse             Expanse 3.22x faster
 Zipfian Point Lookup (1M hit)             not pre-reg        Expanse             Expanse 1.86x faster
 Uniform Random Point Lookup (1M hit)      blart (ART)        Expanse             Expanse 1.51x faster
--------------------------------------------------------------------------------------------------------
 Dynamic Growth Insert (1M seq)            not pre-reg        Expanse             Expanse 4.82x faster
 Dynamic Growth Insert (1M clustered)      not pre-reg        Expanse             Expanse 3.99x faster
 Dynamic Growth Insert (1M random)         blart (ART)        Expanse             Expanse 1.69x faster
 Dynamic Growth Insert (1M stride)         not pre-reg        Expanse             Expanse 1.82x faster
 Dynamic Growth Insert (1M Zipfian)        not pre-reg        BOUNDARY_RESULT     0.93x [0.93, 0.94]
--------------------------------------------------------------------------------------------------------
 Full In-Order Iteration (1M random)       not pre-reg        Expanse             Expanse 10.85x faster
 Full In-Order Iteration (1M seq)          ExpanseMap         Expanse             Expanse 1.88x faster
 Full In-Order Iteration (1M clustered)    ExpanseMap         Expanse             Expanse 1.89x faster
 Range Scan k=1000 (1M seq)                ExpanseMap         Expanse             Expanse 1.89x faster
 Range Scan k=1000 (1M clustered)          ExpanseMap         Expanse             Expanse 1.90x faster
 Range Scan k=1000 (1M random)             not pre-reg        Expanse             Expanse 9.60x faster
 Range Scan k=100 (1M seq)                 ExpanseMap         Expanse             Expanse 1.71x faster
 Range Scan k=100 (1M clustered)           ExpanseMap         Expanse             Expanse 1.75x faster
 Range Scan k=100 (1M random)              not pre-reg        Expanse             Expanse 7.58x faster
 Range Scan k=10 (1M seq)                  not pre-reg        Expanse             Expanse 1.53x faster
 Range Scan k=10 (1M clustered)            not pre-reg        Expanse             Expanse 1.50x faster
========================================================================================================
```

### Key Architectural Insights

1. **Memory Footprint: Structural 4.6× Advantage for Expanse**:
   - `blart` (v0.5.0) heap-allocates a 32-byte `LeafNode<K, V>` (`value` 8B, `key` 8B, `prev` 8B, `next` 8B) for every inserted entry. With inner node sharing, this imposes a strict structural floor of $\ge 40.1$ B/key on dense keys.
   - `ExpanseMap` packs 256 keys into 64-byte `LeafBitmap1` descriptors with contiguous `ValueSlot` arrays, achieving **8.66 B/key** on sequential keys (4.63× less memory). *(Note: 8.66 B/key reflects `TrackingAlloc` layout bytes, compared to the 8.56 B/key `JudyLMemUsed` C ABI accounting figure; workloads differ: capi_bench_vs_libjudy vs art_memory)*.
   - In the original Leis et al. 2013 paper model (Section V, Table IV), ART achieved 8.1 B/key by assuming values embedded directly inside 8-byte pointer slots without separate leaf nodes. `blart` does not implement that inline-value model.
   - The sparse-stride envelope row is projected-from-fit (fitted to the measured 16.39 B/key anchor) and excluded from the contradiction-rule confirmed count, matching `results/contradiction_rule.json`.

2. **Point Lookup: Expanse Wins Structured Keys; Random Refuted in Expanse's Favor**:
   - POPCNT-indexed bitmap leaves and contiguous chunk memory enable Expanse to achieve **17.85 ns** on sequential and **14.55 ns** on sparse stride lookups.
   - On uniform random 1M keys, Expanse achieved **43.87 ns** vs ART's **66.39 ns** (1.51× faster), refuting the pre-registered ART win.

3. **Dynamic Growth & Insertion**:
   - `ExpanseMap` achieves **12.23 ns/insert** on sequential keys vs `blart`'s **58.43 ns/insert** (4.82× faster), benefiting from localized subexpanse allocations compared to per-entry leaf node heap allocations.
   - On uniform random 1M insert, Expanse achieves **57.83 ns** vs ART's **97.31 ns** (1.69× faster), refuting the pre-registered ART win.

4. **Range Scan & In-Order Iteration**:
   - On 1M random key iteration, `ExpanseMap`'s stack-based zero-allocation iterator scans at **5.59 ns/element** vs `blart`'s **61.05 ns/element** (10.85× faster).
   - For short range scans ($k=10$, 100,000 starts per timed window so the descent is measured and not one warm path), Expanse leads by 1.53×: **10.46 ns/element** against `blart`'s **16.07 ns** *(workload: art_scan)*.

5. **Unmeasured Regimes**:
   - Small payloads ($\le 7$ keys, Immediates): **Not measured in this suite** (tracked in [#387](https://github.com/orieg/expanse/issues/387)).

---

## 2. Benchmark Visualizations

### Memory Footprint Census (Bytes / Key)
![Memory Footprint](results/chart_memory.svg)

### Point Lookup Latency (100% Hit Rate)
![Point Lookup Hit](results/chart_lookup_hit.svg)

### Point Lookup Latency (50% Hit / 50% Rejection Miss Rate)
![Point Lookup Miss](results/chart_lookup_miss.svg)

### Dynamic Insertion Throughput (ns / Insert)
![Dynamic Insertion](results/chart_insert.svg)

### Ordered Scan & In-Order Iteration Latency (ns / Element)
![Ordered Scan & Iteration](results/chart_scan.svg)

---

## 3. Detailed Results Tables ($N = 1,000,000$)

### Pillar 1: Point Lookup Latency (100% Hit Rate, ns/op)

| Key Distribution | `ExpanseMap` | `blart` (ART) | `BTreeMap` | `hashbrown` | Ratio (Exp/ART) | Verdict / Status |
|---|---:|---:|---:|---:|---:|---|
| **Sequential** | **17.85 ns** | 47.08 ns | 161.30 ns | 8.51 ns | 0.38x [0.37, 0.38] | **CONFIRMED** *(Expanse 2.64×)* |
| **Clustered** | **26.55 ns** | 52.24 ns | 162.41 ns | 8.49 ns | 0.51x [0.50, 0.51] | **CONFIRMED** *(Expanse 1.98×)* |
| **Sparse Stride** | **14.55 ns** | 46.90 ns | 161.96 ns | 8.63 ns | 0.31x [0.31, 0.31] | **Expanse 3.22×** *(not pre-registered)* |
| **Zipfian (1M draws over 225,853 unique keys)** | **9.77 ns** | 18.30 ns | 57.53 ns | 3.06 ns | 0.54x [0.52, 0.55] | **Expanse 1.86×** *(not pre-registered)* |
| **Uniform Random** | **43.87 ns** | 66.39 ns | 152.13 ns | 8.52 ns | 0.66x [0.66, 0.66] | **REFUTED in Expanse's favour (1.51×)** *(pre-registered: ART win)* |

### Pillar 2: Point Lookup Latency (50% Hit / 50% Miss, ns/op)

| Key Distribution | `ExpanseMap` | `blart` (ART) | `BTreeMap` | `hashbrown` | Ratio (Exp/ART) | Verdict / Status |
|---|---:|---:|---:|---:|---:|---|
| **Sequential** | **14.02 ns** | 30.77 ns | 80.15 ns | 9.62 ns | 0.46x [0.45, 0.46] | **Expanse 2.19×** *(not pre-registered)* |
| **Clustered** | **20.64 ns** | 32.16 ns | 105.29 ns | 9.62 ns | 0.64x [0.63, 0.64] | **Expanse 1.57×** *(not pre-registered)* |
| **Sparse Stride** | **12.57 ns** | 28.54 ns | 79.44 ns | 8.85 ns | 0.44x [0.44, 0.44] | **Expanse 2.27×** *(not pre-registered)* |
| **Zipfian (1M draws over 225,853 unique keys)** | **11.39 ns** | 16.14 ns | 42.99 ns | 9.66 ns | 0.69x [0.67, 0.71] | **Expanse 1.44×** *(not pre-registered)* |
| **Uniform Random** | **46.63 ns** | 58.12 ns | 150.69 ns | 9.67 ns | 0.80x [0.80, 0.81] | **BOUNDARY_RESULT** (0.80× [0.80, 0.81]) |

### Pillar 3: Dynamic Insertion Latency (ns/op)

| Key Distribution | `ExpanseMap` | `blart` (ART) | `BTreeMap` | `hashbrown` | Ratio (Exp/ART) | Verdict / Status |
|---|---:|---:|---:|---:|---:|---|
| **Sequential** | **12.23 ns** | 58.43 ns | 45.86 ns | 21.66 ns | 0.21x [0.20, 0.21] | **Expanse 4.82×** *(not pre-registered)* |
| **Clustered** | **14.83 ns** | 58.79 ns | 44.96 ns | 22.04 ns | 0.25x [0.25, 0.25] | **Expanse 3.99×** *(not pre-registered)* |
| **Uniform Random** | **57.83 ns** | 97.31 ns | 108.96 ns | 22.17 ns | 0.59x [0.59, 0.59] | **REFUTED in Expanse's favour (1.69×)** *(pre-registered: ART win)* |
| **Sparse Stride** | **31.61 ns** | 57.63 ns | 45.88 ns | 21.99 ns | 0.55x [0.55, 0.55] | **Expanse 1.82×** *(not pre-registered)* |
| **Zipfian (225,853 distinct keys)** | **53.27 ns** | 57.04 ns | 80.47 ns | 9.69 ns | 0.93x [0.93, 0.94] | **BOUNDARY_RESULT** (0.93× [0.93, 0.94]) |

### Pillar 4: Ordered Range Scan & Full In-Order Iteration (ns/element)

| Operation & Distribution | `ExpanseMap` | `blart` (ART) | `BTreeMap` | Ratio (Exp/ART) | Verdict / Status |
|---|---:|---:|---:|---:|---|
| **Full Iteration (Sequential)** | **3.14 ns** | 5.90 ns | 5.53 ns | 0.53x [0.53, 0.53] | **CONFIRMED** *(Expanse 1.88×)* |
| **Full Iteration (Clustered)** | **3.24 ns** | 6.14 ns | 5.69 ns | 0.53x [0.53, 0.53] | **CONFIRMED** *(Expanse 1.89×)* |
| **Full Iteration (Uniform Random)** | **5.59 ns** | 61.05 ns | 11.60 ns | 0.09x [0.09, 0.09] | **Expanse 10.85×** *(not pre-registered)* |
| **Range Scan k=1000 (Sequential)** | **2.92 ns** | 5.66 ns | 4.97 ns | 0.53x [0.52, 0.54] | **CONFIRMED** *(Expanse 1.89×)* |
| **Range Scan k=1000 (Clustered)** | **3.04 ns** | 5.87 ns | 5.14 ns | 0.53x [0.52, 0.54] | **CONFIRMED** *(Expanse 1.90×)* |
| **Range Scan k=1000 (Uniform Random)** | **5.45 ns** | 52.50 ns | 9.93 ns | 0.10x [0.10, 0.11] | **Expanse 9.60×** *(not pre-registered)* |
| **Range Scan k=100 (Sequential)** | **3.98 ns** | 6.94 ns | 7.04 ns | 0.58x [0.58, 0.60] | **CONFIRMED** *(Expanse 1.71×)* |
| **Range Scan k=100 (Clustered)** | **4.16 ns** | 7.37 ns | 7.26 ns | 0.57x [0.56, 0.58] | **CONFIRMED** *(Expanse 1.75×)* |
| **Range Scan k=100 (Uniform Random)** | **6.47 ns** | 49.28 ns | 11.32 ns | 0.13x [0.13, 0.13] | **Expanse 7.58×** *(not pre-registered)* |
| **Range Scan k=10 (Sequential)** | **10.46 ns** | 16.07 ns | 23.25 ns | 0.65x [0.65, 0.66] | **Expanse 1.53×** *(not pre-registered)* |
| **Range Scan k=10 (Clustered)** | **11.44 ns** | 17.23 ns | 23.62 ns | 0.67x [0.66, 0.67] | **Expanse 1.50×** *(not pre-registered)* |
| **Range Scan k=10 (Uniform Random)** | **13.98 ns** | 37.26 ns | 22.73 ns | 0.37x [0.37, 0.38] | **Expanse 2.67×** *(not pre-registered)* |

### Pillar 5: Live Heap Memory Allocation Census Across Population Scaling (Bytes / Key)

| Population ($N$) | Key Distribution | `ExpanseMap` | `blart` (ART) | `BTreeMap` | `hashbrown` | Expanse vs ART | Verdict / Status |
|---|---|---:|---:|---:|---:|---:|---|
| 1,000 | **Sequential** | **77.87 B/k** | 40.35 B/k | 34.37 B/k | 34.83 B/k | ART 1.93x less RAM | **LOSS at N=1k** (ART 1.93x less RAM, not pre-registered; §2 covers N=10⁶) |
| 1,000 | **Clustered** | **77.87 B/k** | 40.35 B/k | 34.37 B/k | 34.83 B/k | ART 1.93x less RAM | **LOSS at N=1k** (ART 1.93x less RAM, not pre-registered; §2 covers N=10⁶) |
| 1,000 | **Sparse Stride** | **78.38 B/k** | 40.35 B/k | 34.37 B/k | 34.83 B/k | ART 1.94x less RAM | **LOSS at N=1k** (ART 1.94x less RAM, not pre-registered; §2 covers N=10⁶) |
| 1,000 | **Uniform Random** | **58.31 B/k** | 58.75 B/k | 27.65 B/k | 34.83 B/k | Expanse 1.01x less RAM | **Expanse 1.01x less RAM** *(not pre-registered)* |
| 1,000 | **Zipfian (354 unique)** | **198.99 B/k** | 51.62 B/k | 26.58 B/k | 24.63 B/k | ART 3.86x less RAM | **LOSS at N=1k** (ART 3.86x less RAM, not pre-registered; §2 covers N=10⁶) |
| 10,000 | **Sequential** | **15.44 B/k** | 40.16 B/k | 34.27 B/k | 27.85 B/k | Expanse 2.60x less RAM | **CONFIRMED** *(Expanse 2.60×)* |
| 10,000 | **Clustered** | **16.62 B/k** | 40.18 B/k | 34.74 B/k | 27.85 B/k | Expanse 2.42x less RAM | **CONFIRMED** *(Expanse 2.42×)* |
| 10,000 | **Sparse Stride** | **22.45 B/k** | 40.16 B/k | 34.27 B/k | 27.85 B/k | Expanse 1.79x less RAM | **CONFIRMED** *(Expanse 1.79×)* |
| 10,000 | **Uniform Random** | **35.94 B/k** | 54.22 B/k | 27.04 B/k | 27.85 B/k | Expanse 1.51x less RAM | **Expanse 1.51×** *(not pre-registered)* |
| 10,000 | **Zipfian (2,911 unique)** | **29.65 B/k** | 51.79 B/k | 27.17 B/k | 23.93 B/k | Expanse 1.75x less RAM | **Expanse 1.75×** *(not pre-registered)* |
| 100,000 | **Sequential** | **9.38 B/k** | 40.14 B/k | 34.28 B/k | 22.28 B/k | Expanse 4.28x less RAM | **CONFIRMED** *(Expanse 4.28×)* |
| 100,000 | **Clustered** | **9.61 B/k** | 40.19 B/k | 34.62 B/k | 22.28 B/k | Expanse 4.18x less RAM | **CONFIRMED** *(Expanse 4.18×)* |
| 100,000 | **Sparse Stride** | **17.09 B/k** | 40.14 B/k | 34.28 B/k | 22.28 B/k | Expanse 2.35x less RAM | **CONFIRMED** *(Expanse 2.35×)* |
| 100,000 | **Uniform Random** | **30.49 B/k** | 57.61 B/k | 27.13 B/k | 22.28 B/k | Expanse 1.89x less RAM | **Expanse 1.89×** *(not pre-registered)* |
| 100,000 | **Zipfian (25,144 unique)** | **14.14 B/k** | 51.71 B/k | 27.32 B/k | 22.16 B/k | Expanse 3.66x less RAM | **Expanse 3.66×** *(not pre-registered)* |
| 1,000,000 | **Sequential** | **8.66 B/k** | 40.13 B/k | 34.28 B/k | 35.65 B/k | Expanse 4.63x less RAM | **CONFIRMED** *(Expanse 4.63×)* |
| 1,000,000 | **Clustered** | **8.77 B/k** | 40.18 B/k | 34.89 B/k | 35.65 B/k | Expanse 4.58x less RAM | **CONFIRMED** *(Expanse 4.58×)* |
| 1,000,000 | **Sparse Stride** | **16.39 B/k** | 40.13 B/k | 34.28 B/k | 35.65 B/k | Expanse 2.45x less RAM | **CONFIRMED** *(Expanse 2.45×)* |
| 1,000,000 | **Uniform Random** | **23.54 B/k** | 55.62 B/k | 27.13 B/k | 35.65 B/k | Expanse 2.36x less RAM | **Expanse 2.36×** *(not pre-registered)* |
| 1,000,000 | **Zipfian (225,846 unique)** | **11.96 B/k** | 52.00 B/k | 27.12 B/k | 19.73 B/k | Expanse 4.35x less RAM | **Expanse 4.35×** *(not pre-registered)* |

---

## 4. Reproducing These Results

To execute the entire 5-pillar benchmark suite and regenerate the charts on the reference host:

```bash
# 1. Run the full benchmark sweep and generate SVG charts
docs/benchmarks/art_comparison/run.sh

# 2. Run a fast smoke test (reduced populations, gitignored scratch output)
docs/benchmarks/art_comparison/run.sh --quick
```

