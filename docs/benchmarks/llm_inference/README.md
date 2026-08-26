# LLM Inference & Speculative Decoding Benchmark — Expanse vs Industry Baselines

This benchmark suite evaluates **Expanse** across four core pillars of high-throughput Large Language Model (LLM) serving systems: **Speculative Decoding Draft Quality**, **Multi-Million Token Datastore Scaling**, **Native C++ Engine Integration (`llama.cpp`)**, and **Prefix-Cache KV-Block LRU Management**.

All figures below are measured on **Apple M1 (arm64-apple-darwin)**, commit containing this document.

---

## 1. Executive Summary

*(measured: Apple M1, arm64-apple-darwin)*

| Architectural Capability | Standard Industry Baseline | Expanse Trie Engine | Impact |
|---|---|---|---|
| **Speculative Match Horizon (JSON)** | Fixed 3-gram: alpha = 3.192 | Variable-length LSM: alpha = 3.754 | **+17.6% higher alpha** (Slashing verification steps from 401 to 341) |
| **Speculative Match Horizon (Code)** | Fixed 3-gram: alpha = 4.466 | Variable-length LSM: alpha = 4.646 | **+4.0% higher alpha** vs 3-gram; +29.3% vs 2-gram (alpha = 3.594) |
| **Speculative Match Horizon (Summary)**| Fixed 3-gram: alpha = 3.226 | Variable-length LSM: alpha = 3.636 | **+12.7% higher alpha** vs 3-gram; ties 2-gram (alpha = 3.636) |
| **Datastore Memory Footprint (5M)** | CPython dict: 446.10 MB (93.6 B/entry) | ExpanseMap: 109.67 MB (23.0 B/entry) | **4.07x lower RAM** (4.07x–5.63x reduction across scales) |
| **Ingestion vs Batched NumPy (Scaling Curve)** | Batched NumPy: 2.49M (100k) to 44.9k (5M) | ExpanseMap: 752k (100k) to 610k (5M) | **0.30x at 100k (NumPy win), 0.95x at 500k, 4.37x at 1M, 13.6x at 5M** |
| **Streaming vs Single-Insert NumPy (5M)** | Single insert NumPy: 105 inserts/s | ExpanseMap: 610,046 inserts/s | **5,810x faster point insertion** (O(depth) vs O(N) array copy) |
| **Native C++ llama.cpp Parity** | Stock std::unordered_map: 4000/4000 | expanse::str_map: 4000/4000 | **100.0% exact token sequence match** with deterministic tie-breaking |
| **Prefix-Cache Memory (1M blocks)** | collections.OrderedDict: 219.92 MB | ExpanseMap Table: 23.19 MB | **9.48x lower RAM** (inclusive of side index) |
| **Rank-Threshold Eviction** | O(N) full table scan in OrderedDict | Native count_below() in ExpanseMap | **2.08M items/sec** bulk prune |

---

## 2. Visual Results & Comparison Charts

### Pillar 1: Speculative Draft Quality (Mean Acceptance Length alpha)
![Pillar 1: Speculative Draft Quality](results/bench_draft_quality_alpha.svg)

### Pillar 2: Million-Token Datastore Scale (Live Memory Footprint)
![Pillar 2: Datastore Scale Memory](results/bench_datastore_scale_memory.svg)

### Pillar 3: Native C++ llama.cpp Lookup Cache Draft Latency
![Pillar 3: Native C++ llama.cpp Lookup](results/bench_llama_lookup_latency.svg)

### Pillar 4: Prefix-Cache KV Block Indexing & LRU Eviction
![Pillar 4: Prefix LRU Eviction](results/bench_prefix_lru_throughput.svg)

---

## 3. Detailed Architectural Findings

### Pillar 1: Speculative Draft Quality & Acceptance Length alpha
In prompt-lookup speculative decoding, candidate generation latency (~0.5–12 µs) is negligible compared to an autoregressive model forward pass (15–50 ms). The true driver of end-to-end inference speedup is **mean accepted tokens per step (alpha)**:

$$\text{Generation Speedup} \approx \frac{\alpha \cdot t_{\text{verify}}}{t_{\text{forward}} + t_{\text{lookup}}}$$

Using the **2-Neighbour Longest Common Prefix (LCP)** algorithm (`prev_at_or_before` + `next_at_or_after` in `ExpanseStrMap` over 7-bit NUL-free encoded token streams), Expanse discovers variable-length matches up to 16 tokens deep across deterministic pattern fixtures:

*(measured: Apple M1, arm64-apple-darwin)*

| Workload | Fixed 3-gram alpha | Fixed 2-gram alpha | Dict Multimap Chain alpha | Expanse Variable LSM alpha | Expanse vs Fixed 3-gram |
|---|---|---|---|---|---|
| **Code Patterns** | 4.466 (515 steps) | 3.594 (640 steps) | 3.859 (596 steps) | **4.646** (495 steps) | **+4.0%** higher alpha (-20 verification steps) |
| **Summary Patterns** | 3.226 (62 steps) | 3.636 (55 steps) | 3.226 (62 steps) | **3.636** (55 steps) | **+12.7%** higher alpha (ties 2-gram; -7 steps) |
| **JSON Schemas** | 3.192 (401 steps) | 3.192 (401 steps) | 3.192 (401 steps) | **3.754** (341 steps) | **+17.6%** higher alpha (-60 verification steps) |

* **Candidate Lookup Latency**: Point lookup latency is 0.50–0.58 µs for fixed 3-gram vs 10.38–11.58 µs for Expanse variable-length 2-neighbour LCP. Because 11 µs represents <0.06% of a 20 ms GPU forward pass, the +4% to +17.6% increase in alpha delivers net end-to-end speedups.

---

### Pillar 2: Multi-Million Token Datastore Scale

Scaling prompt matching or retrieval across multi-turn sessions (100k to 5M tokens):

*(measured: Apple M1, arm64-apple-darwin)*

| Population N | CPython dict RAM | Sorted NumPy RAM | ExpanseMap RAM | Memory Reduction vs dict | Expanse Streaming Ingestion | NumPy Single Insert | NumPy Batched Append |
|---|---|---|---|---|---|---|---|
| **100k** | 12.49 MB (131.0 B) | 1.53 MB (16.0 B) | **2.55 MB (26.7 B)** | **4.91x** | 751,800 tps | 16,840 tps | 2,485,913 tps |
| **500k** | 49.99 MB (104.8 B) | 7.63 MB (16.0 B) | **10.85 MB (22.7 B)** | **4.61x** | 639,591 tps | 1,154 tps | 671,025 tps |
| **1M** | 99.99 MB (104.9 B) | 15.26 MB (16.0 B) | **17.75 MB (18.6 B)** | **5.63x** | 986,218 tps | 476 tps | 225,409 tps |
| **5M** | 446.10 MB (93.6 B) | 76.29 MB (16.0 B) | **109.67 MB (23.0 B)** | **4.07x** | 610,046 tps | 105 tps | 44,900 tps |

* **Ingestion Scaling Curve vs Batched NumPy**: At 100k tokens, pre-allocated contiguous NumPy array append + sort achieves higher throughput (2.49M vs 752k tps, 0.30x). As token population scales, NumPy's chunk sort costs grow while Expanse sustains flat O(depth) throughput: Expanse ties NumPy at 500k (640k vs 671k tps, 0.95x), wins 4.37x at 1M (986k vs 225k tps), and wins **13.6x at 5M** (610k vs 44.9k tps). Single-token streaming inserts into sorted arrays collapse entirely (105 inserts/s at 5M, a **5,810x** Expanse win).
* **Snapshot Save & Load (Expected Loss)**: Sorted NumPy `.npy` arrays load via zero-copy mmap in 9.8–15.2 ms. Deserializing raw keys and reconstructing an `ExpanseMap` takes 23.8 ms at 100k and 1,342 ms at 5M, confirming the pre-registered snapshot loading loss.

---

### Pillar 3: Native C++ llama.cpp Lookup Decoding

Evaluating `include/expanse.hpp` linked against release `libexpanse` replicating `llama.cpp`'s `common/ngram-cache.cpp` with deterministic lowest-token tie-breaking:

*(measured: Apple M1, arm64-apple-darwin)*

| Context Length N | Stock llama.cpp Update | Stock llama.cpp Draft | expanse::str_map Update | expanse::str_map Draft | expanse RAM | Sequence Match Rate |
|---|---|---|---|---|---|---|
| **4k** | 591.3 ns | 0.54 µs | 1,011.0 ns | 3.21 µs | 0.61 MB | **100.0%** (4,000 / 4,000) |
| **32k** | 1,915.0 ns | 0.42 µs | 1,485.6 ns | 3.95 µs | 5.20 MB | **100.0%** (4,000 / 4,000) |
| **128k** | 2,259.2 ns | 1.13 µs | 1,695.3 ns | 4.56 µs | 19.72 MB | **100.0%** (4,000 / 4,000) |

* **100.0% Exact Sequence Parity**: With identical deterministic tie-breaking (lowest token ID), `expanse::str_map` produces 100.0% exact token-for-token sequence match with stock `llama.cpp` across all context lengths (4k, 32k, 128k).
* **Throughput & Scaling**: At 128k context, `expanse::str_map` update latency is **1,695 ns** (outperforming stock hash map's 2,259 ns) while drafting candidates in 4.56 µs within a 19.72 MB footprint.

---

### Pillar 4: Prefix-Cache KV Block Indexing & LRU Eviction

Evaluating physical KV block cache managers across 100k to 1M active blocks (Expanse memory is all-inclusive, adding 8 B/block for the `block_to_ts` side array):

*(measured: Apple M1, arm64-apple-darwin)*

| Active Blocks N | OrderedDict RAM | ExpanseMap Table RAM | Memory Reduction | OrderedDict Touch | ExpanseMap Touch | OrderedDict Evict | ExpanseMap Evict | ExpanseMap Rank Evict |
|---|---|---|---|---|---|---|---|---|
| **100k** | 23.39 MB | **2.32 MB** | **10.09x** | 16.25M tps | 1.69M tps | 8.10M tps | 3.71M tps | 1.45M tps |
| **500k** | 109.90 MB | **11.59 MB** | **9.48x** | 19.83M tps | 2.13M tps | 9.37M tps | 3.53M tps | 2.21M tps |
| **1M** | 219.92 MB | **23.19 MB** | **9.48x** | 8.63M tps | 1.81M tps | 4.82M tps | 3.29M tps | 2.08M tps |

* **Honest Pre-Registered Speed Trade-off**: `collections.OrderedDict` wins raw O(1) touch and oldest-eviction throughput (2x–8x faster) due to inline doubly-linked list pointer swings.
* **Expanse Winning Regimes**:
  1. **9.5x–10.1x lower RAM footprint** (23.2 MB vs 219.9 MB at 1M blocks).
  2. **Rank-Threshold Eviction**: `ExpanseMap` prunes all blocks below a timestamp threshold via native `count_below()` and range iteration at **2.08M items/sec**; `OrderedDict` is structurally incapable of time-window pruning without an O(N) full table scan.

---

## 4. Feature & Architectural Matrix

| Feature | CPython dict | Sorted NumPy | OrderedDict | Stock llama.cpp | Expanse Engine |
|---|---|---|---|---|---|
| **Variable-Length Match (LSM)** | ❌ (Fixed N-gram) | ❌ (Fixed N-gram) | ❌ | ❌ (Fixed N-gram) | ✅ (2-Neighbour LCP) |
| **Live Continuous Ingestion** | ✅ (O(1)) | ❌ (O(N) realloc) | ✅ (O(1)) | ✅ (O(1)) | ✅ (O(depth)) |
| **RAM per Entry (1M)** | 104.9 B | 16.0 B | 219.9 B | 85.0 B | **18.6 B** |
| **Rank-Threshold Eviction** | ❌ (O(N) scan) | ❌ (O(N) scan) | ❌ (O(N) scan) | ❌ | ✅ (count_below()) |
| **Drop-in C++ Header** | ❌ | ❌ | ❌ | N/A | ✅ (include/expanse.hpp) |

---

## 5. llama.cpp Integration Snippet

An illustrative integration snippet is provided in [`benches/patch_llama_cpp.diff`](benches/patch_llama_cpp.diff) demonstrating how `common/ngram-cache.cpp` can integrate `expanse::str_map` via `include/expanse.hpp`.

---

## 6. How to Reproduce

```bash
# 1. Quick smoke verification
./docs/benchmarks/llm_inference/run.sh --quick

# 2. Full benchmark suite (all 4 pillars + chart generation)
./docs/benchmarks/llm_inference/run.sh
```
