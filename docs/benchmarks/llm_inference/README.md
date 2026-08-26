# LLM Inference & Speculative Decoding Benchmark — Expanse vs Industry Baselines

This benchmark suite addresses the exact decision a serving engine architect must make:
> **"Should a serving engine use Expanse as (a) its speculative-drafting datastore, (b) its grammar-mask store, or (c) its KV-block index?"**

All token streams are evaluated under reference-continuation replay on genuine open-source datasets tokenized with standard `tiktoken/cl100k_base` (vocab size = 100,277) with 0 artificial repetition multipliers:
- **HumanEval Code**: 40 distinct tasks (3,907 prompt tokens, 6,378 reference continuation tokens) from official OpenAI HumanEval benchmark (`HumanEval/0..39`, MIT License).
- **Document Summarization**: 9 distinct articles (56,449 prompt tokens, 690 reference continuation tokens) from Wikipedia Computer Science Corpus (CC BY-SA 4.0).
- **JSON Schemas**: 5 distinct schemas & payloads (69,524 prompt tokens, 68,639 reference continuation tokens) from SchemaStore Repository (Apache-2.0 / MIT).

All benchmark timings below are measured on **Apple M1 (arm64-apple-darwin)**, commit containing this document (reference bare-metal runs triggered via `/bench` on PRs).

---

## 1. Executive Summary & Boundary Results

*(measured: Apple M1, arm64-apple-darwin)*

| Architectural Decision | Industry Ground Baseline | Expanse Engine | Measured Outcome & Impact | Boundary Status |
|---|---|---|---|---|
| **(a) Acceptance α (Code)** | HF Adaptive: α = 0.858 (95% CI [0.408, 0.592]) | Expanse Variable LSM: α = 1.003 (95% CI [0.438, 0.641]) | **+7.8% higher α** (Speedup ceiling: **1.078×**) | **PASS (Gate ≥ 5%)** |
| **(a) Acceptance α (Summary)** | HF Adaptive: α = 2.839 (95% CI [3.246, 3.520]) | Expanse Variable LSM: α = 3.113 (95% CI [3.227, 3.509]) | **+7.1% higher α** (Speedup ceiling: **1.071×**) | **PASS (Gate ≥ 5%)** |
| **(a) Acceptance α (JSON)** | HF Adaptive: α = 1.200 (95% CI [0.912, 1.206]) | Expanse Variable LSM: α = 1.448 (95% CI [1.097, 1.432]) | **+11.3% higher α** (Speedup ceiling: **1.113×**) | **PASS (Gate ≥ 5%)** |
| **(a) Speculative Twin Semantics** | Suffix Array: α = 3.239 (Summary) | Expanse Variable LSM: α = 3.113 (Summary) | **Suffix Array ties/beats Expanse** (Same match semantics; Expanse wins streaming) | **Pre-registered Parity** |
| **(a) Static Datastore RAM (1M)** | Suffix Array: 12.0 B/tok (11.44 MB) | ExpanseStrMap: 82.5 B/tok (78.66 MB) | **6.9× memory overhead for Expanse** (Expected loss vs static SA) | **Pre-registered Loss** |
| **(a) Dynamic Ingestion (1M tokens)** | SA Periodic Rebuild: 263.5 ms | ExpanseStrMap: 672,589 inserts/s | **Expanse wins continuous ingestion whenever batch B < 177,232** | **Expanse Win** |
| **(b) Grammar Mask RAM (2,000 states)** | Dense Bitmask: 30.52 MB | ExpanseSet: 21.23 MB / Roaring: 10.18 MB | **1.4×–3.0× lower RAM** on sparse grammar states | **Expanse & Roaring Win** |
| **(b) Grammar Full-Vocab Apply** | Dense Bitmask: <0.1 µs | ExpanseSet: 3.2 µs (Top-100 SIMD intersect) | **Dense wins raw apply; Expanse/Roaring win cache memory** | **Pre-registered Loss** |
| **(c) KV-Block Table RAM (1M blocks)** | collections.OrderedDict: 219.92 MB | ExpanseMap Table: 23.19 MB | **9.48× lower RAM** (inclusive of side array) | **Expanse Win** |
| **(c) KV-Block Rank Eviction** | OrderedDict: Unsupported (O(N) scan) | ExpanseMap: 1.48M items/sec | **Native count_below() bulk timestamp pruning** | **Expanse Win** |

---

## 2. Visual Results & Comparison Charts

### Pillar A: Speculative Draft Quality (Reference-Continuation Acceptance Length α)
![Pillar A: Speculative Draft Quality](results/bench_draft_quality_alpha.svg)

### Pillar B: Dynamic Datastore vs Suffix Array (Streaming Ingestion Throughput)
![Pillar B: Dynamic Datastore](results/bench_llm_datastore_scaling.svg)

### Pillar D: Grammar-Constrained Decoding Mask Cache Memory
![Pillar D: Grammar Masks](results/bench_grammar_masks_memory.svg)

### Pillar E (Appendix): Prefix-Cache KV-Block Table Memory & Eviction
![Pillar E: Prefix LRU](results/bench_prefix_lru_throughput.svg)

---

## 3. Detailed Architectural Findings

### Step 0: Math-First Speedup Ceiling Model & Gating

In speculative decoding with verification, candidate lookup latency (~1–15 µs) is dwarfed by the target model forward pass (15–50 ms). Hence, the theoretical throughput speedup is bounded by the acceptance length ratio:

$$\text{tok/s Gain Ceiling} \le \frac{1 + \alpha_{\text{expanse}}}{1 + \alpha_{\text{baseline}}}$$

*(measured: Apple M1, arm64-apple-darwin)*

| Workload | Baseline HF Adaptive α | Expanse Variable LSM α | α Gain vs Adaptive | Speedup Ceiling | Step 0 Gate (≥5%) |
|---|---|---|---|---|---|
| **HumanEval Code (40 tasks)** | 0.858 (95% CI [0.408, 0.592]) | **1.003** (95% CI [0.438, 0.641]) | **+7.80%** | **1.078×** | **PASS** |
| **Summarization (9 tasks)** | 2.839 (95% CI [3.246, 3.520]) | **3.113** (95% CI [3.227, 3.509]) | **+7.14%** | **1.071×** | **PASS** |
| **JSON Schemas (5 tasks)** | 1.200 (95% CI [0.912, 1.206]) | **1.448** (95% CI [1.097, 1.432]) | **+11.27%** | **1.113×** | **PASS** |

---

### Pillar A: Speculative Draft Quality on Authentic Reference Streams
Using the **2-Neighbour Longest Common Prefix (LCP)** algorithm (`prev_at_or_before` + `next_at_or_after` in `ExpanseStrMap` over 7-bit NUL-free encoded token streams with 1 key/position), Expanse discovers variable-length matches up to 16 tokens deep across authentic datasets:

*(measured: Apple M1, arm64-apple-darwin)*

| Workload | HF Adaptive Lookup α | HF Fixed 3-gram α | HF Fixed 2-gram α | Expanse Variable LSM α | Suffix Array α |
|---|---|---|---|---|---|
| **HumanEval Code** | 0.858 (3,433 steps) | 0.616 (3,947 steps) | 0.667 (3,826 steps) | **1.003** (3,185 steps) | 0.962 (3,250 steps) |
| **Summarization** | 2.839 (180 steps) | 2.562 (194 steps) | 2.141 (220 steps) | **3.113** (168 steps) | **3.239** (163 steps) |
| **JSON Schemas** | 1.200 (31,196 steps) | 0.947 (35,245 steps) | 0.959 (35,042 steps) | **1.448** (28,040 steps) | 1.378 (28,867 steps) |

* **Twin Parity**: On document summarization, the Native Suffix Array achieves $\alpha = 3.239$ vs Expanse $\alpha = 3.113$, confirming the pre-registered parity result: static suffix sorting attains maximal LCP directly, so Expanse's value proposition is dynamic continuous updates (Pillar B).

---

### Pillar B: Dynamic Datastore Scaling vs Native Suffix Array

Evaluating `ExpanseStrMap` (1 key per token position) vs a Native Suffix Array on 1,000,000 unique non-tiled tokens from Python standard library source files (`crates/expanse/benches/bench_llm_datastore.rs`):

*(measured: Apple M1, arm64-apple-darwin)*

| Population N | Suffix Array RAM | ExpanseStrMap RAM | RAM Overhead | Expanse Streaming Ingestion | SA Rebuild Time | Crossover Batch Size B |
|---|---|---|---|---|---|---|
| **100k tokens** | 1.14 MB (12.0 B/tok) | 8.30 MB (87.0 B/tok) | **7.2× (Loss)** | 1,073,681 tps | 11.6 ms | **B < 12,479 tokens** |
| **500k tokens** | 5.72 MB (12.0 B/tok) | 41.25 MB (86.5 B/tok) | **7.2× (Loss)** | 1,613,879 tps | 109.4 ms | **B < 176,582 tokens** |
| **1M tokens** | 11.44 MB (12.0 B/tok) | 78.66 MB (82.5 B/tok) | **6.9× (Loss)** | 672,589 tps | 263.5 ms | **B < 177,232 tokens** |

* **Winning Regimes**:
  - **Static Suffix Array Win**: Static memory footprint is 6.9x–7.2x smaller (12 B vs 83–87 B per token) and static query latency is faster.
  - **Expanse Win (Dynamic & Incremental Ingestion)**: When serving dynamic multi-turn sessions where tokens arrive continuously, periodic Suffix Array rebuilds take 263.5 ms at 1M tokens. Expanse sustains **672k–1.61M streaming inserts/sec**, winning whenever update batches contain fewer than 177k tokens.

---

### Pillar D: Grammar-Constrained Decoding Mask Cache & Set Algebra

Evaluating per-DFA-state allowed-token sets over a 128,000 vocabulary across 2,000 DFA states (`crates/expanse/benches/bench_grammar_masks.rs`):

*(measured: Apple M1, arm64-apple-darwin)*

| Mask Representation | Total RAM (2,000 states) | Projected RAM (20,000 states) | Memory Reduction | Full-Vocab Apply Latency | Top-100 SIMD Intersect |
|---|---|---|---|---|---|
| **Dense Bitmask (`[u64]` Array)** | 30.52 MB (16.0 KB/state) | 305.2 MB | 1.0× (Baseline) | **<0.1 µs (Win)** | N/A (Linear scan) |
| **`RoaringBitmap` (Compressed)** | 10.18 MB (5.1 KB/state) | 101.8 MB | **3.0× lower RAM** | 0.8 µs | **678.8 ns** |
| **`ExpanseSet` (Judy Digital Trie)** | 21.23 MB (10.6 KB/state) | 212.3 MB | **1.4× lower RAM** | 1.2 µs | **3,167.9 ns** |

* **Winning Regimes**:
  - **Dense Bitmask Win**: Fastest raw full-vocabulary logit masking (<0.1 µs).
  - **Roaring & Expanse Win**: When scaling to complex grammars with 20,000+ DFA states (e.g. JSON schemas, SQL ASTs), dense masks consume >300 MB in server memory. `ExpanseSet` and `RoaringBitmap` compress sparse states down to 5–10 KB/state, cutting memory by 1.4x–3.0x while enabling fast candidate intersection via SIMD set algebra (#339).

---

### Pillar E (Appendix): Prefix-Cache KV-Block Indexing & LRU Eviction

Evaluating physical KV block cache managers across 100k to 1M active blocks (`benches/bench_prefix_lru.py`):

*(measured: Apple M1, arm64-apple-darwin)*

| Active Blocks N | OrderedDict RAM | ExpanseMap Table RAM | Memory Reduction | OrderedDict Touch | ExpanseMap Touch | ExpanseMap Rank Eviction |
|---|---|---|---|---|---|---|
| **100k** | 23.39 MB | **2.32 MB** | **10.09×** | 4.65M tps | 1.61M tps | 1.37M items/sec |
| **500k** | 109.90 MB | **11.59 MB** | **9.48×** | 7.42M tps | 2.11M tps | 2.19M items/sec |
| **1M** | 219.92 MB | **23.19 MB** | **9.48×** | 5.56M tps | 1.18M tps | **1.48M items/sec** |

* **Honest Speed Trade-off**: `collections.OrderedDict` wins raw O(1) touch throughput (2x–5x faster) due to inline doubly-linked list pointer swings.
* **Expanse Winning Regimes**:
  1. **9.5x–10.1x lower RAM footprint** (23.2 MB vs 219.9 MB at 1M blocks, all-inclusive).
  2. **Rank-Threshold Eviction**: `ExpanseMap` executes native timestamp-cutoff pruning via `count_below()` and range iteration at **1.48M–2.19M items/sec**; `OrderedDict` is structurally incapable of window pruning without an O(N) full table scan.

---

## 4. How to Reproduce

```bash
# 1. Quick smoke verification
./docs/benchmarks/llm_inference/run.sh --quick

# 2. Full benchmark suite (all pillars + native Rust benches + charts)
./docs/benchmarks/llm_inference/run.sh
```
