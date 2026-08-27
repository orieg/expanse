# Expanse vs. Hashbrown vs. BTreeMap: Empirical Comparative Methodology

## 1. Executive Summary & Problem Statement

In systems programming, associative containers are typically chosen based on two broad archetypes:
1. **Hash Tables (e.g., `hashbrown::HashMap` / Google SwissTable):** Unordered, flat contiguous control-byte arrays offering $O(1)$ expected amortized lookups via SIMD group probes (`_mm_cmpeq_epi8`), but suffering from periodic global rehash cliffs, hash computation overhead, high memory slack space ($1.14\times$ to $2\times$ over-allocation), and an inability to perform ordered iteration or range queries.
2. **Ordered Search Trees (e.g., `std::collections::BTreeMap` / Cache-Oblivious B-Trees):** Ordered, node-based branching structures offering $O(\log N)$ point queries, prefix navigation, and range scans, but hindered by pointer chasing, low branching density per cache line, and high heap allocation churn.

**Expanse (`ExpanseMap`)** introduces an alternative architectural paradigm: **Expanse-Partitioned Digital Tries** modernized with SIMD/SWAR parallel bit-twiddling, zero-byte vector detection, and class-sized adaptive subexpanses.

This methodology document specifies the reproduction harness, metrics, statistical rigor, and hardware isolation standards used to evaluate `ExpanseMap`, `hashbrown::HashMap`, and `std::collections::BTreeMap`.

---

## 2. The 5-Pillar Benchmark Suite

### Pillar 1: Native Criterion Suite Port
- **Source:** Direct port of upstream `hashbrown/benches/bench.rs`.
- **Operations:**
  - `insert_growing`: Dynamic table expansion from $0 \to N$ keys without pre-allocation.
  - `lookup_hit`: Point queries on 100% present keys.
  - `lookup_miss`: Point queries on 100% absent keys.
  - `iter_all`: Linear scan of all elements.
- **Population Bands:** $10^4, 10^5, 5 \times 10^5$ keys.

### Pillar 2: YCSB (Yahoo! Cloud Serving Benchmark) Workloads A–F
- **Access Distribution:** Zipfian skew ($s = 0.99$, power-law distribution typical of real-world key-value stores and database engines).
- **Standard Workloads:**
  - **Workload A (Update Heavy):** 50% Read, 50% Update.
  - **Workload B (Read Heavy):** 95% Read, 5% Update.
  - **Workload C (Read Only):** 100% Read.
  - **Workload D (Read Latest):** 95% Read, 5% Insert Latest.
  - **Workload E (Short Range Scans):** 95% Short Range Scan (10–50 items), 5% Insert. *Note: `hashbrown` is disqualified on Workload E due to lack of ordered traversal.*
  - **Workload F (Read-Modify-Write):** 50% Read, 50% RMW.

### Pillar 3: Dynamic Growth Tail Latency & Rehash Cliffs
- **Instrumentation:** High-dynamic-range histogram (`HdrHistogram`, 3 significant figures, nanosecond resolution up to 10 seconds).
- **Protocol:** Un-preallocated continuous insertion of $10^6$ keys. Every individual insert is timed with `std::time::Instant` and recorded.
- **Metrics:** $P_{50}, P_{75}, P_{90}, P_{95}, P_{99}, P_{99.9}, P_{99.99}$, and $\text{Max}$ latency.
- **Objective:** Reveal the latency cliff of table-doubling reallocations in SwissTable vs. localized subexpanse allocations in Expanse.

### Pillar 4: Martin Ankerl & Tessil Key Distribution Suite
- **Key Geometries:**
  - **Uniform Random 64-bit (`uniform`):** High entropy pseudo-random keys.
  - **Dense Sequential (`sequential`):** Monotonically increasing keys $0 \dots N-1$.
  - **Sparse Clustered / Stride (`clustered`):** Clustered burst sequences with sparse strides.
  - **Zipfian Skewed (`zipfian`):** Heavily biased hot keys.

### Pillar 5: Runtime Memory Allocation Profiler
- **Instrumentation:** Custom `GlobalAlloc` hook tracking exact heap allocations and deallocations in bytes.
- **Metric:** Live Heap Bytes / Key ($\text{Bytes} / N$) at steady state across population scales $10^3, 10^4, 10^5, 5 \times 10^5$ (the populations in `results/baseline_memory.json`).

---

## 3. Experimental Isolation & Measurement Discipline

To eliminate thermal throttling, core migration, and CPU frequency scaling noise:
1. **Pipelining & Optimization:** All harnesses are compiled under `--profile bench` with `opt-level = 3`, `lto = "thin"`, and `codegen-units = 1`.
2. **Black Box Optimization Fence:** All benchmark operations pass keys, values, and results through `std::hint::black_box` to prevent compiler dead-code elimination.
3. **No Setup Contamination:** Container creation, RNG generation, and buffer prep are strictly separated from measurement loops.

---

## 4. Reproducing the Benchmarks

To execute the entire suite and regenerate all charts:

```bash
# 1. Clone or switch to repository
cd /path/to/expanse

# 2. Run reproduction script
./docs/benchmarks/hashbrown_comparison/run.sh

# Or run quick smoke test
./docs/benchmarks/hashbrown_comparison/run.sh --quick
```

Results and SVG graphs are written to:
`docs/benchmarks/hashbrown_comparison/results/`
