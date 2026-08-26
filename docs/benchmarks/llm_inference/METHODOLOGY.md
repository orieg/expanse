# LLM Inference & Speculative Decoding Benchmark Methodology — Expanse vs Industry Baselines

## 1. Problem Statement

Modern Large Language Model (LLM) serving systems (vLLM, SGLang, HuggingFace TGI, llama.cpp) are heavily constrained by memory hierarchy bandwidth and speculative verification latency:
1. **Speculative Decoding Acceptance Rate ($\alpha$)**: In prompt-lookup speculative decoding, token candidate generation latency ($\approx 1\text{--}12\,\mu\text{s}$) is dwarfed by the autoregressive model forward pass ($15\text{--}50\,\text{ms}$). The true lever for end-to-end token generation throughput ($\text{tok/s}$) is **mean acceptance length $\alpha$** ($\text{tok/s} \approx \alpha / \text{step\_time}$). Fixed $N$-gram matchers (e.g. HuggingFace PromptLookup 3-gram) discard variable-length matching context, limiting $\alpha$.
2. **Multi-Million Token Datastore Scale**: Scaling prompt lookup or retrieval across $10^5 \to 5\times 10^6$ tokens requires compact, live-mutable indices. Standard Python hash maps (`dict[int, int]`) suffer extreme heap pointer bloat ($90\text{--}130\,\text{B/entry}$), while static sorted arrays (NumPy `searchsorted`) incur prohibitive $O(N)$ reallocation overheads on streaming continuous token ingestion.
3. **Native C++ LLM Engine Integration (`llama.cpp`)**: Dynamic $n$-gram caches in native C++ engines must update on every sampled token and provide low-latency multi-token draft rollouts across long context windows ($4\text{k}\dots 128\text{k}$ tokens).
4. **Prefix-Cache KV-Block Indexing & Eviction**: KV cache block managers (vLLM/SGLang block tables) track active sequence blocks using `OrderedDict` (doubly-linked list + hash table). While offering fast $O(1)$ pointer-swap touches, `OrderedDict` incurs $10\times$ memory bloat and cannot execute rank-threshold eviction without an $O(N)$ full table scan.

This benchmark suite evaluates **Expanse** across four pillars using deterministic synthetic token stream pattern fixtures (Code Generation Patterns, Summary Patterns, and Structured JSON Schemas).

---

## 2. Step 0 — Pre-Registered Hypotheses & Claims Ceiling

Following repository research discipline (`RULE[user_global]`, `docs/BENCHMARKING.md`), claims ceilings, expected wins, and **expected losses** are pre-registered prior to evaluation.

### 2.1 Pre-Registration Matrix

| Pillar / Arm | Metric | Expected Winner | Margin / Rationale |
|---|---|---|---|
| **Pillar 1: Code & JSON ($\alpha$)** | Mean Acceptance Length $\alpha$ | **Expanse LSM** | Win ($\ge +4\%\dots 18\%$ higher $\alpha$ vs fixed 3-gram via variable-length 2-neighbour LCP). |
| **Pillar 1: Lookup Latency** | Candidate Lookup ($\mu\text{s}$) | **HF Fixed 3-Gram** | **Expected Loss** ($\approx 0.5\,\mu\text{s}$ vs $11\text{--}12\,\mu\text{s}$ for Expanse). Negligible vs $20\,\text{ms}$ model forward pass. |
| **Pillar 2: Live Memory** | Index RAM at $10^5\dots 5\times 10^6$ | **ExpanseMap** | Win ($4\times\dots 5.6\times$ lower RAM vs CPython `dict`; competitive with static NumPy). |
| **Pillar 2: Streaming Ingestion** | Dynamic Inserts / sec | **ExpanseMap** | Win ($50\times\dots 5{,}800\times$ faster than Sorted NumPy's single-insert $O(N)$ array reallocations). |
| **Pillar 2: Batched Append & Snapshot Load** | Disk $\to$ Memory / Bulk Array Append | **Sorted NumPy** | **Expected Loss** (NumPy contiguous buffer bulk sort and mmap `np.load` win on bulk arrays). |
| **Pillar 3: C++ llama.cpp Lookup Volume** | Draft Volume Parity | **Tie** | Both stock `unordered_map` and `expanse::str_map` draft equal token volumes ($4{,}000 / 4{,}000$ tokens). |
| **Pillar 3: C++ Lookup Latency** | Point Update & Query Latency | **Stock `unordered_map`** | **Expected Loss** (flat hash probe vs radix trie traversal; sub-$\mu\text{s}$ vs $11\text{--}21\,\mu\text{s}$). |
| **Pillar 4: Prefix-Cache Memory** | Index RAM at $10^5\dots 10^6$ blocks | **ExpanseMap** | Win ($9.5\times\dots 10.1\times$ lower RAM vs `OrderedDict`, inclusive of side array). |
| **Pillar 4: Touch Latency** | LRU `move_to_end` | **`OrderedDict`** | **Expected Loss** ($O(1)$ pointer swing vs trie deletion + re-insertion; $3\times\dots 6\times$ faster). |
| **Pillar 4: Rank Eviction** | Evict Below Timestamp | **ExpanseMap** | Win (`count_below()` + range prune at $2.1\text{M}$ items/s; `OrderedDict` is structurally incapable). |

---

## 3. The Four Pillars

### Pillar 1 — Speculative Draft Quality via Replay Verifier (`benches/bench_draft_quality.py`)
* **Workloads**:
  1. `Synthetic Code Patterns` (simulating function definitions, recurring variable names, loops, returns).
  2. `Synthetic Summary Patterns` (simulating document text with recurring entity key phrases).
  3. `Synthetic JSON Schemas` (simulating nested object schemas, repeated keys, enums).
* **Algorithms**:
  - `HF Fixed 3-Gram`: Sliding window fixed 3-gram exact match.
  - `HF Fixed 2-Gram`: Sliding window fixed 2-gram exact match.
  - `Expanse Fixed 3-Gram`: 21-bit bit-packed integer key trie (`ExpanseMap`).
  - `Expanse Variable-Length LSM`: 7-bit NUL-free encoded reversed context stream with 2-neighbour LCP search (`prev_at_or_before` + `next_at_or_after` in `ExpanseStrMap`).
  - `Dict Multimap Tree`: Python `dict[prefix, list[(token, count)]]` tree baseline.
  - `Expanse Draft Tree`: Multi-candidate continuation tree.
* **Metrics**: Mean Acceptance Length $\alpha$ ($\text{tokens/step}$), Acceptance Rate ($N_{\text{accept}} / N_{\text{draft}}$), Candidate Lookup Latency ($\mu\text{s}$) with 95% bootstrap CIs.

### Pillar 2 — Million-Token Datastore Scale (`benches/bench_datastore_scale.py`)
* **Scale**: $N \in [100\text{k}, 500\text{k}]$ (quick) and $[100\text{k}, 500\text{k}, 1\text{M}, 5\text{M}]$ (full).
* **Competitors**:
  - `CPython dict[int, int]`: Standard Python hash map tracked via `tracemalloc`.
  - `Sorted NumPy Array`: Contiguous `uint64` arrays (`keys` + `values`) searched via `np.searchsorted`, with single-insert, batched-append, and snapshot save/load arms.
  - `ExpanseMap`: Digital trie with internal allocator memory accounting (`.mem_used()`).
* **Metrics**: Total RAM (MB), Bytes per Token (B/entry), Bulk Build Throughput (tps), Continuous Streaming Ingestion Throughput (tps), Snapshot Save/Load Latency (ms).

### Pillar 3 — Native C++ llama.cpp Lookup Decoding (`benches/bench_llama_lookup.cpp`)
* **Scope**: Standalone C++20 harness testing exact `common/ngram-cache.cpp` logic linked against `include/expanse.hpp` and release `libexpanse.so` / `libexpanse.dylib`.
* **Competitors**:
  - Stock `llama.cpp` nested hash map: `std::unordered_map<std::string, std::unordered_map<int32_t, int32_t>>`.
  - Expanse C++20 trie: `expanse::str_map<uint64_t>` with 7-bit NUL-free token encoding.
* **Context Scales**: $4\text{k}, 32\text{k}, 128\text{k}$ tokens.
* **Metrics**: Cache update latency (ns), draft generation latency ($\mu\text{s}$), total tokens drafted (verification of parity).

### Pillar 4 — Prefix-Cache KV-Block Indexing & LRU Eviction (`benches/bench_prefix_lru.py`)
* **Competitors**:
  - `collections.OrderedDict`: Standard vLLM/SGLang block table LRU implementation.
  - `ExpanseMap Ordered Table`: `(monotonic_ts << 32) | block_id` composite key with `block_to_ts` side list.
* **Operations**:
  - Touch (`move_to_end` vs `remove` + `insert`).
  - Oldest LRU Eviction (`popitem(last=False)` vs `first()` + `remove()`).
  - Rank-Threshold Eviction (`count_below(ts_cutoff)`).
* **Metrics**: All-inclusive memory footprint (MB), Touch throughput (ops/s), Eviction throughput (ops/s), Rank-eviction throughput (ops/s).

---

## 4. Benchmark Execution & Reproducibility

```bash
# 1. Quick smoke run (all 4 pillars + chart generation)
./docs/benchmarks/llm_inference/run.sh --quick

# 2. Full scaling run
./docs/benchmarks/llm_inference/run.sh
```
