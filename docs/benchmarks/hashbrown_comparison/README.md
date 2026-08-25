# Expanse vs. Hashbrown vs. BTreeMap: Empirical Comparative Benchmark Suite

This directory contains the reproducible benchmark suite, raw measurements, methodology specifications, and dual-theme visualization assets comparing **Expanse** (`ExpanseMap`), **Hashbrown / Google SwissTable** (`hashbrown::HashMap`), and **B-Tree** (`std::collections::BTreeMap`).

---

## 1. Architectural Feature Matrix

| Capability / Property | `hashbrown::HashMap` (SwissTable) | `std::collections::BTreeMap` | `expanse::ExpanseMap` |
| :--- | :--- | :--- | :--- |
| **Underlying Data Structure** | Flat 1-Byte Control Array + Buckets | Cache-Oblivious B-Tree | Expanse-Partitioned Digital Trie |
| **Point Query Complexity** | $O(1)$ amortized expected | $O(\log N)$ | $O(k) \le 8$ digit steps |
| **Ordered Traversal & Range Scans** | ❌ **Disqualified** ($O(N \log N)$) | ✅ Supported | ✅ Supported |
| **Sequential Key Memory Density** | $22.28\text{ Bytes / Key}$ | $34.28\text{ Bytes / Key}$ | **$9.38\text{ Bytes / Key}$** ($2.4\times$ to $3.6\times$ smaller) |
| **Key Hashing Required** | ✅ Yes (SipHash / FoldHash) | ❌ No (Direct ordering) | ❌ No (Direct radix prefix slicing) |
| **Dynamic Ingestion Growth Model** | Global Table Doubling (Rehash) | Node Splitting | Local Subexpanse Allocation |

---

## 2. Key Findings & Empirical Results

### Pillar 1: YCSB (Yahoo! Cloud Serving Benchmark) Workloads A–F
Zipfian access distribution ($s = 0.99$, power-law skew) on 500,000 keys.

![YCSB Workloads A-F Throughput](results/bench_ycsb_workloads.svg)

- **Workload E (Short Range Scans):** SwissTable is structurally disqualified because hash tables cannot perform ordered scans without allocating, dumping, and sorting the entire table. `ExpanseMap` and `BTreeMap` execute ordered range queries natively.
- **Read & Update Heavy Workloads (A, B, C, D, F):** `ExpanseMap` delivers $48\text{–}105\text{ Mops/sec}$, outperforming `BTreeMap` ($13\text{–}20\text{ Mops/sec}$) by **$3\times\text{ to }5.8\times$**.

---

### Pillar 2: Memory Footprint (Live Heap Bytes / Key)
Measured via custom `GlobalAlloc` hooks tracking heap allocations at steady state.

![Memory Footprint Bytes Per Key](results/bench_memory_footprint.svg)

| Key Pattern ($N = 100,000$) | `hashbrown` | `BTreeMap` | `ExpanseMap` | Expanse vs. Hashbrown |
| :--- | :--- | :--- | :--- | :--- |
| **Dense Sequential ($0 \dots N$)** | $22.28\text{ B/key}$ | $34.28\text{ B/key}$ | **$9.38\text{ B/key}$** | **$2.37\times$ more compact** |
| **Uniform Random 64-bit** | $22.28\text{ B/key}$ | $27.24\text{ B/key}$ | $30.82\text{ B/key}$ | Comparable (trie branch expansion) |

Expanse's uncompressed bitmap-backed leaf nodes pack sequential and clustered integer keys with near-zero pointer overhead, dropping memory consumption to under 10 bytes per key.

---

### Pillar 3: Ingestion Tail Latency & Rehash Cliffs
Dynamic table expansion from $0 \to 100,000$ keys without pre-allocating capacity, measured via `HdrHistogram`.

![Ingestion Tail Latency Percentiles](results/bench_tail_latency.svg)

---

### Pillar 4: Martin Ankerl & Tessil Key Distributions
Point lookup throughput (Mops/sec) evaluated across standard key geometries:

![Key Distributions Throughput](results/bench_key_distributions.svg)

- **Dense Sequential & Clustered:** Expanse reaches $46.5\text{–}47.8\text{ Mops/sec}$, matching hash table speeds on clustered data and exceeding B-Tree by $4\times\text{ to }5\times$.

---

### Pillar 5: Native Hashbrown Criterion Suite Port
Point query hit/miss and dynamic growth throughput ported from `hashbrown/benches/bench.rs`:

![Native Criterion Port Throughput](results/bench_native_throughput.svg)

---

## 3. Performance Investigation & Optimization Roadmap

During benchmark profiling, two key performance characteristics were identified:

1. **`MapRange` Iterator Cursor Reuse (Range Scan Performance):**
   - **Observation:** `ExpanseMap::iter()` achieves **$26.1\text{ Mops/sec}$** full scan throughput, whereas `ExpanseMap::range()` achieved lower throughput on short range queries in Workload E.
   - **Root Cause:** `MapRange::next()` currently invokes `self.map.next_at_or_after(cur)` on every step, restarting tree descent from the root for each item ($O(L \cdot 8)$ node descents for range length $L$). In contrast, `MapIter` maintains an internal stack cursor (`RawIter<true>`).
   - **Optimization Item:** Refactor `MapRange` to initialize `RawIter` seeked to `start` and bounded at `end`. This will eliminate root restarts and raise range scan throughput by an estimated $10\times\text{–}20\times$.

2. **Sparse Random Key Branch Allocation:**
   - **Observation:** On uniform random 64-bit keys, Expanse consumes $30.8\text{ B/key}$ vs SwissTable's $22.3\text{ B/key}$.
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
