# Expanse vs. Hashbrown vs. BTreeMap: Empirical Comparative Benchmark Suite

This directory contains the reproducible benchmark suite, raw measurements, methodology specifications, and dual-theme visualization assets comparing **Expanse** (`ExpanseMap`), **Hashbrown / Google SwissTable** (`hashbrown::HashMap`), and **B-Tree** (`std::collections::BTreeMap`).

---

## 1. Architectural Feature Matrix

| Capability / Property | `hashbrown::HashMap` (SwissTable) | `std::collections::BTreeMap` | `expanse::ExpanseMap` |
| :--- | :--- | :--- | :--- |
| **Underlying Data Structure** | Flat 1-Byte Control Array + Buckets | Cache-Oblivious B-Tree | Expanse-Partitioned Digital Trie |
| **Point Query Complexity** | $O(1)$ amortized expected | $O(\log N)$ | $O(k) \le 8$ digit steps |
| **Ordered Traversal & Range Scans** | ❌ **Disqualified** ($O(N \log N)$) | ✅ Supported | ✅ Supported |
| **Sequential Key Memory Density** | $35.7\text{ Bytes / Key}$ | $34.3\text{ Bytes / Key}$ | **$8.7\text{ Bytes / Key}$** ($3.9\times\text{ to }4.1\times$ smaller) |
| **Key Hashing Required** | ✅ Yes (SipHash / FoldHash) | ❌ No (Direct ordering) | ❌ No (Direct radix prefix slicing) |
| **Dynamic Ingestion Growth Model** | Global Table Doubling (Rehash) | Node Splitting | Local Subexpanse Allocation |

---

## 2. Key Findings & Empirical Results

### Pillar 1: YCSB (Yahoo! Cloud Serving Benchmark) Workloads A–F
Zipfian access distribution ($s = 0.99$, power-law skew) on 500,000 keys.

![YCSB Workloads A-F Throughput](results/bench_ycsb_workloads.svg)

- **Workload E (Short Range Scans):** SwissTable is structurally disqualified because hash tables cannot perform ordered scans without allocating, dumping, and sorting the entire table. `ExpanseMap` and `BTreeMap` execute ordered range queries natively.
- **Read & Update Heavy Workloads (A, B, C, D, F):** `ExpanseMap` delivers $15\text{–}36\text{ Mops/sec}$, consistently beating `BTreeMap` ($3\text{–}7\text{ Mops/sec}$) by **$2.5\times\text{ to }6.5\times$**.
- **Workload D (Read Latest / Ingestion):** `ExpanseMap` reaches **$28.7\text{ Mops/sec}$**, outperforming `hashbrown` ($15.6\text{ Mops/sec}$) by **$1.84\times$**.

---

### Pillar 2: Memory Footprint (Live Heap Bytes / Key)
Measured via custom `GlobalAlloc` hooks tracking heap allocations at steady state ($N = 100,000$).

![Memory Footprint Bytes Per Key](results/bench_memory_footprint.svg)

| Key Pattern ($N = 100,000$) | `hashbrown` | `BTreeMap` | `ExpanseMap` | Expanse vs. Hashbrown |
| :--- | :--- | :--- | :--- | :--- |
| **Dense Sequential ($0 \dots N$)** | $35.7\text{ B/key}$ | $34.3\text{ B/key}$ | **$8.7\text{ B/key}$** | **$4.1\times$ more compact** |
| **Uniform Random 64-bit** | $35.7\text{ B/key}$ | $27.2\text{ B/key}$ | $31.8\text{ B/key}$ | Comparable (trie branch expansion) |

Expanse's uncompressed bitmap-backed leaf nodes pack sequential and clustered integer keys with near-zero pointer overhead, dropping memory consumption to under 9 bytes per key.

---

### Pillar 3: Ingestion Tail Latency & Rehash Cliffs
Dynamic table expansion from $0 \to 100,000$ keys without pre-allocating capacity, measured via `HdrHistogram`.

![Ingestion Tail Latency Percentiles](results/bench_tail_latency.svg)

---

### Pillar 4: Martin Ankerl & Tessil Key Distributions
Point lookup throughput (Mops/sec) evaluated across standard key geometries ($N = 500,000$):

![Key Distributions Throughput](results/bench_key_distributions.svg)

| Key Geometry ($N = 500,000$) | `hashbrown` | `BTreeMap` | `ExpanseMap` | Expanse Win |
| :--- | :--- | :--- | :--- | :--- |
| **Sparse Clustered / Stride** | $28.8\text{ Mops/s}$ | $15.5\text{ Mops/s}$ | **$96.2\text{ Mops/s}$** | **$3.3\times$ faster than Hashbrown, $6.2\times$ vs BTree** |
| **Dense Sequential ($0 \dots N$)** | $46.5\text{ Mops/s}$ | $16.2\text{ Mops/s}$ | **$73.0\text{ Mops/s}$** | **$1.6\times$ faster than Hashbrown, $4.5\times$ vs BTree** |
| **Zipfian Skewed ($s = 0.99$)** | $36.1\text{ Mops/s}$ | $4.1\text{ Mops/s}$ | **$18.3\text{ Mops/s}$** | **$4.5\times$ faster than BTree** |
| **Uniform Random 64-bit** | $23.2\text{ Mops/s}$ | $3.4\text{ Mops/s}$ | **$6.7\text{ Mops/s}$** | **$2.0\times$ faster than BTree** |

---

### Pillar 5: Native Hashbrown Criterion Suite Port
Point query hit/miss and dynamic growth throughput ported from `hashbrown/benches/bench.rs` ($N = 100,000$):

![Native Criterion Port Throughput](results/bench_native_throughput.svg)

---

## 3. Performance Investigation & Optimization Roadmap

During benchmark profiling, two key performance characteristics were identified:

1. **`MapRange` & `SetRange` Iterator Cursor Seeking (Implemented):**
   - **Optimization:** Refactored `MapRange` and `SetRange` to wrap `RawIter` with direct target descent seeking (`RawIter::from_tree_range`, `RawIter::from_root_leaf_range`).
   - **Result:** Eliminates $O(L \cdot 8)$ root restarts, allowing bounded range scans to stream contiguous leaf elements in $O(1)$ amortized time per key without heap allocation.

2. **Sparse Random Key Branch Allocation:**
   - **Observation:** On uniform random 64-bit keys, Expanse consumes $31.8\text{ B/key}$ vs SwissTable's $35.7\text{ B/key}$ under allocator churn.
   - **Root Cause:** In high-entropy random distributions without shared prefixes, 64-bit keys create separate branch paths across levels 7 down to 0 with low occupancy per branch.
   - **Optimization Item:** Evaluate adaptive narrow pointer promotion / linear leaf inline key packing for sparse subexpanses.

---

## 4. How to Reproduce

All benchmarks can be reproduced with a single command:

```bash
# From repository root
./docs/benchmarks/hashbrown_comparison/run.sh

# Or quick verification mode:
./docs/benchmarks/hashbrown_comparison/run.sh --quick
```

### Directory Structure

```
docs/benchmarks/hashbrown_comparison/
├── README.md                      # Consolidated report and overview
├── METHODOLOGY.md                 # Rigor, isolation rules, and hardware setup
├── run.sh                         # 1-command reproduction runner
├── scripts/
│   ├── theme.py                   # Shared dual-theme CSS and SVG template module
│   ├── run_all.py                 # Master benchmark orchestrator
│   └── generate_charts.py         # Dual-theme SVG visualizer
└── results/                       # Raw JSON telemetry and generated SVGs
    ├── baseline_native.json
    ├── baseline_ycsb.json
    ├── baseline_tail_latency.json
    ├── baseline_distributions.json
    ├── baseline_memory.json
    ├── bench_native_throughput.svg
    ├── bench_ycsb_workloads.svg
    ├── bench_tail_latency.svg
    ├── bench_key_distributions.svg
    └── bench_memory_footprint.svg
```
