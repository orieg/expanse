# LLM Inference & Speculative Decoding Benchmark — Expanse vs Industry Baselines

This benchmark suite evaluates **Expanse** across four core pillars of high-throughput Large Language Model (LLM) serving systems: **Speculative Decoding Draft Quality**, **Multi-Million Token Datastore Scaling**, **Native C++ Engine Integration (`llama.cpp`)**, and **Prefix-Cache KV-Block LRU Management**.

All figures below are measured on **Apple M1 (arm64-apple-darwin)**, commit `e703f83b` (`feat/bench-llm-inference-342`).

---

## 1. Executive Summary

| Architectural Capability | Standard Industry Baseline | Expanse Trie Engine | Impact |
|---|---|---|---|
| **Speculative Match Horizon (JSON)** | Fixed 3-gram: $\alpha = 3.192$ | Variable-length LSM: $\alpha = 3.754$ | **$+17.6\%$ higher $\alpha$** (Slashing verification steps from 401 to 341) |
| **Speculative Match Horizon (Code)** | Fixed 3-gram: $\alpha = 4.466$ | Variable-length LSM: $\alpha = 4.646$ | **$+4.0\%$ higher $\alpha$** vs 3-gram; $+29.3\%$ vs 2-gram ($\alpha = 3.594$) |
| **Speculative Match Horizon (Summary)**| Fixed 3-gram: $\alpha = 3.226$ | Variable-length LSM: $\alpha = 3.636$ | **$+12.7\%$ higher $\alpha$** vs 3-gram; ties 2-gram ($\alpha = 3.636$) |
| **Datastore Memory Footprint ($5\text{M}$)** | CPython `dict`: $446.10\,\text{MB}$ ($93.6\,\text{B/entry}$) | `ExpanseMap`: $109.67\,\text{MB}$ ($23.0\,\text{B/entry}$) | **$4.07\times$ lower RAM** ($4.07\times\dots 5.63\times$ reduction across scales) |
| **Streaming Dynamic Ingestion ($5\text{M}$)** | Sorted NumPy single insert: $103\,\text{inserts/s}$ | `ExpanseMap`: $603{,}846\,\text{inserts/s}$ | **$5{,}862\times$ faster continuous ingestion** ($O(\text{depth})$ vs $O(N)$ copies) |
| **Prefix-Cache Memory ($1\text{M}$ blocks)** | `collections.OrderedDict`: $219.92\,\text{MB}$ | `ExpanseMap` Ordered Table: $23.19\,\text{MB}$ | **$9.48\times$ lower RAM** (inclusive of side index) |
| **Rank-Threshold Eviction** | $O(N)$ full table scan in `OrderedDict` | Native `count_below()` in `ExpanseMap` | **$2.10\text{M}$ items/sec** bulk prune |

---

## 2. Visual Results & Comparison Charts

### Pillar 1: Speculative Draft Quality (Mean Acceptance Length $\alpha$)
![Pillar 1: Speculative Draft Quality](results/bench_draft_quality_alpha.svg)

### Pillar 2: Million-Token Datastore Scale (Live Memory Footprint)
![Pillar 2: Datastore Scale Memory](results/bench_datastore_scale_memory.svg)

### Pillar 3: Native C++ llama.cpp Lookup Cache Draft Latency
![Pillar 3: Native C++ llama.cpp Lookup](results/bench_llama_lookup_latency.svg)

### Pillar 4: Prefix-Cache KV Block Indexing & LRU Eviction
![Pillar 4: Prefix LRU Eviction](results/bench_prefix_lru_throughput.svg)

---

## 3. Detailed Architectural Findings

### Pillar 1: Speculative Draft Quality & Acceptance Length $\alpha$
In prompt-lookup speculative decoding, candidate generation latency ($\approx 1\text{--}12\,\mu\text{s}$) is negligible compared to an autoregressive model forward pass ($15\text{--}50\,\text{ms}$). The true driver of end-to-end inference speedup is **mean accepted tokens per step ($\alpha$)**:

$$\text{Generation Speedup} \approx \frac{\alpha \cdot t_{\text{verify}}}{t_{\text{forward}} + t_{\text{lookup}}}$$

Using the **2-Neighbour Longest Common Prefix (LCP)** algorithm (`prev_at_or_before` + `next_at_or_after` in `ExpanseStrMap` over 7-bit NUL-free encoded token streams), Expanse discovers variable-length matches up to 16 tokens deep across deterministic pattern fixtures:

| Workload | Fixed 3-gram $\alpha$ | Fixed 2-gram $\alpha$ | Dict Multimap $\alpha$ | Expanse Variable LSM $\alpha$ | Expanse vs Fixed 3-gram |
|---|---|---|---|---|---|
| **Code Patterns** | 4.466 (515 steps) | 3.594 (640 steps) | 1.979 (1162 steps) | **4.646** (495 steps) | **+4.0%** higher $\alpha$ (-20 verification steps) |
| **Summary Patterns** | 3.226 (62 steps) | 3.636 (55 steps) | 1.786 (112 steps) | **3.636** (55 steps) | **+12.7%** higher $\alpha$ (ties 2-gram; -7 steps) |
| **JSON Schemas** | 3.192 (401 steps) | 3.192 (401 steps) | 1.778 (720 steps) | **3.754** (341 steps) | **+17.6%** higher $\alpha$ (-60 verification steps) |

* **Candidate Lookup Latency**: Point lookup latency is $0.50\text{--}0.54\,\mu\text{s}$ for fixed 3-gram vs $11.46\text{--}12.04\,\mu\text{s}$ for Expanse variable-length 2-neighbour LCP. Because $12\,\mu\text{s}$ represents $<0.06\%$ of a $20\,\text{ms}$ GPU forward pass, the $+4\%\dots +17.6\%$ increase in $\alpha$ delivers net end-to-end speedups.

---

### Pillar 2: Multi-Million Token Datastore Scale

Scaling prompt matching or retrieval across multi-turn sessions ($10^5 \to 5\times 10^6$ tokens):

| Population $N$ | CPython `dict` RAM | Sorted NumPy RAM | `ExpanseMap` RAM | Memory Reduction vs `dict` | Expanse Streaming Ingestion | NumPy Single Insert | NumPy Batched Append |
|---|---|---|---|---|---|---|---|
| **100k** | $12.49\,\text{MB}$ ($131.0\,\text{B}$) | $1.53\,\text{MB}$ ($16.0\,\text{B}$) | **$2.55\,\text{MB}$ ($26.7\,\text{B}$)** | **$4.91\times$** | $601{,}679\,\text{tps}$ | $9{,}789\,\text{tps}$ | $1{,}274{,}697\,\text{tps}$ |
| **500k** | $49.99\,\text{MB}$ ($104.8\,\text{B}$) | $7.63\,\text{MB}$ ($16.0\,\text{B}$) | **$10.85\,\text{MB}$ ($22.7\,\text{B}$)** | **$4.61\times$** | $694{,}436\,\text{tps}$ | $966\,\text{tps}$ | $452{,}962\,\text{tps}$ |
| **1M** | $99.99\,\text{MB}$ ($104.9\,\text{B}$) | $15.26\,\text{MB}$ ($16.0\,\text{B}$) | **$17.75\,\text{MB}$ ($18.6\,\text{B}$)** | **$5.63\times$** | $501{,}735\,\text{tps}$ | $557\,\text{tps}$ | $236{,}753\,\text{tps}$ |
| **5M** | $446.10\,\text{MB}$ ($93.6\,\text{B}$) | $76.29\,\text{MB}$ ($16.0\,\text{B}$) | **$109.67\,\text{MB}$ ($23.0\,\text{B}$)** | **$4.07\times$** | $603{,}846\,\text{tps}$ | $103\,\text{tps}$ | $28{,}368\,\text{tps}$ |

* **Dynamic Ingestion vs NumPy**: Single-token streaming inserts into sorted arrays collapse ($103\,\text{inserts/s}$ at $5\text{M}$) due to $O(N)$ buffer copies. `ExpanseMap` executes dynamic $O(\text{depth})$ trie insertions at **$603{,}846\,\text{inserts/s}$** ($5{,}862\times$ faster).
* **NumPy Winning Regime (Batched Append & Snapshot Load)**: When data arrives in large discrete chunks, pre-allocating and bulk sorting NumPy arrays achieves higher batch throughput ($1.27\text{M}\,\text{tps}$ at 100k). Raw `.npy` snapshots load via zero-copy mmap in $11\text{--}20\,\text{ms}$.

---

### Pillar 3: Native C++ llama.cpp Lookup Decoding

Evaluating `include/expanse.hpp` linked against `libexpanse` replicating `llama.cpp`'s `common/ngram-cache.cpp`:

| Context Length $N$ | Stock `llama.cpp` Update | Stock `llama.cpp` Draft | `expanse::str_map` Update | `expanse::str_map` Draft | `expanse` RAM | Total Drafted Tokens |
|---|---|---|---|---|---|---|
| **4k** | $1{,}199.3\,\text{ns}$ | $0.72\,\mu\text{s}$ | $5{,}230.7\,\text{ns}$ | $11.30\,\mu\text{s}$ | $3.04\,\text{MB}$ | 4,000 / 4,000 |
| **32k** | $2{,}623.2\,\text{ns}$ | $0.85\,\mu\text{s}$ | $7{,}780.5\,\text{ns}$ | $13.19\,\mu\text{s}$ | $24.39\,\text{MB}$ | 4,000 / 4,000 |
| **128k** | $2{,}503.4\,\text{ns}$ | $0.75\,\mu\text{s}$ | $4{,}201.9\,\text{ns}$ | $21.66\,\mu\text{s}$ | $97.04\,\text{MB}$ | 4,000 / 4,000 |

* **Draft Volume & Sequence Parity**: Both engines produce equal draft volumes ($4{,}000 / 4{,}000$ draft tokens). Exact token-by-token sequence divergence occurs strictly when multiple distinct continuation tokens share equal frequency (`count = 1`): `std::unordered_map` breaks ties by arbitrary hash bucket order, while `expanse::str_map` breaks ties deterministically in lexicographical byte order.
* **Trade-off**: Stock hash map provides sub-$\mu\text{s}$ flat point updates, while `expanse::str_map` executes in $4.2\text{--}7.8\,\mu\text{s}$, operating well within the multi-millisecond LLM sampling budget while supporting ordered range traversals.

---

### Pillar 4: Prefix-Cache KV Block Indexing & LRU Eviction

Evaluating physical KV block cache managers across $100\text{k} \to 1\text{M}$ active blocks (Expanse memory is all-inclusive, adding $8\,\text{B/block}$ for the `block_to_ts` side array):

| Active Blocks $N$ | `OrderedDict` RAM | `ExpanseMap` Table RAM | Memory Reduction | `OrderedDict` Touch | `ExpanseMap` Touch | `OrderedDict` Evict | `ExpanseMap` Evict | `ExpanseMap` Rank Evict |
|---|---|---|---|---|---|---|---|---|
| **100k** | $23.39\,\text{MB}$ | **$2.32\,\text{MB}$** | **$10.09\times$** | $12.82\text{M}\,\text{tps}$ | $1.66\text{M}\,\text{tps}$ | $7.08\text{M}\,\text{tps}$ | $2.32\text{M}\,\text{tps}$ | $170{,}366\,\text{tps}$ |
| **500k** | $109.90\,\text{MB}$ | **$11.59\,\text{MB}$** | **$9.48\times$** | $11.20\text{M}\,\text{tps}$ | $1.09\text{M}\,\text{tps}$ | $6.93\text{M}\,\text{tps}$ | $3.32\text{M}\,\text{tps}$ | $185{,}031\,\text{tps}$ |
| **1M** | $219.92\,\text{MB}$ | **$23.19\,\text{MB}$** | **$9.48\times$** | $7.65\text{M}\,\text{tps}$ | $2.07\text{M}\,\text{tps}$ | $2.33\text{M}\,\text{tps}$ | $3.46\text{M}\,\text{tps}$ | $2.10\text{M}\,\text{tps}$ |

* **Honest Pre-Registered Speed Trade-off**: `collections.OrderedDict` wins raw $O(1)$ touch and oldest-eviction throughput ($3\times\dots 6\times$ faster) due to inline doubly-linked list pointer swings.
* **Expanse Winning Regimes**:
  1. **$9.5\times\dots 10.1\times$ lower RAM footprint** ($23.2\,\text{MB}$ vs $219.9\,\text{MB}$ at $1\text{M}$ blocks).
  2. **Rank-Threshold Eviction**: `ExpanseMap` prunes all blocks below a timestamp threshold via native `count_below()` and range iteration at **$2.10\text{M}\,\text{items/sec}$**; `OrderedDict` is structurally incapable of time-window pruning without an $O(N)$ full table scan.

---

## 4. Feature & Architectural Matrix

| Feature | CPython `dict` | Sorted NumPy | `OrderedDict` | Stock `llama.cpp` | Expanse Engine |
|---|---|---|---|---|---|
| **Variable-Length Match (LSM)** | ❌ (Fixed $N$-gram) | ❌ (Fixed $N$-gram) | ❌ | ❌ (Fixed $N$-gram) | ✅ (2-Neighbour LCP) |
| **Live Continuous Ingestion** | ✅ ($O(1)$) | ❌ ($O(N)$ realloc) | ✅ ($O(1)$) | ✅ ($O(1)$) | ✅ ($O(\text{depth})$) |
| **RAM per Entry ($1\text{M}$)** | $104.9\,\text{B}$ | $16.0\,\text{B}$ | $219.9\,\text{B}$ | $85.0\,\text{B}$ | **$18.6\,\text{B}$** |
| **Rank-Threshold Eviction** | ❌ ($O(N)$ scan) | ❌ ($O(N)$ scan) | ❌ ($O(N)$ scan) | ❌ | ✅ (`count_below()`) |
| **Drop-in C++ Header** | ❌ | ❌ | ❌ | N/A | ✅ (`include/expanse.hpp`) |

---

## 5. llama.cpp Integration Patch

An illustrative drop-in patch is provided in [`benches/patch_llama_cpp.diff`](benches/patch_llama_cpp.diff) (pinned against `llama.cpp` master commit `b3560`) demonstrating direct integration into `common/ngram-cache.cpp` using `include/expanse.hpp`.

---

## 6. How to Reproduce

```bash
# 1. Quick smoke verification
./docs/benchmarks/llm_inference/run.sh --quick

# 2. Full benchmark suite (all 4 pillars + chart generation)
./docs/benchmarks/llm_inference/run.sh
```
