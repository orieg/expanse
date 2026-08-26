# LLM Inference & Speculative Decoding Benchmark Methodology — Expanse vs Industry Baselines

## 1. Problem Statement

Modern Large Language Model (LLM) serving systems (vLLM, SGLang, HuggingFace TGI, llama.cpp) are heavily constrained by memory hierarchy bandwidth and speculative verification latency:
1. **Speculative Decoding Acceptance Rate ($\alpha$)**: In prompt-lookup speculative decoding, token candidate generation latency ($\approx 1\text{--}10\,\mu\text{s}$) is dwarfed by the autoregressive model forward pass ($15\text{--}50\,\text{ms}$). The true lever for end-to-end token generation throughput ($\text{tok/s}$) is **mean acceptance length $\alpha$** ($\text{tok/s} \approx \alpha / \text{step\_time}$). Fixed $N$-gram matchers (e.g. HuggingFace PromptLookup 3-gram) discard variable-length matching context, limiting $\alpha$.
2. **Multi-Million Token Datastore Scale**: Scaling prompt lookup or retrieval across $10^5 \to 10^7$ tokens requires compact, live-mutable indices. Standard Python hash maps (`dict[int, int]`) suffer extreme heap pointer bloat ($130\text{--}150\,\text{B/entry}$), while static sorted arrays (NumPy `searchsorted`) incur prohibitive $O(N)$ reallocation overheads on streaming continuous token ingestion.
3. **Native C++ LLM Engine Integration (`llama.cpp`)**: Dynamic $n$-gram caches in native C++ engines must update on every sampled token and provide low-latency multi-token draft rollouts across long context windows ($4\text{k}\dots 128\text{k}$ tokens).
4. **Prefix-Cache KV-Block Indexing & Eviction**: KV cache block managers (vLLM/SGLang block tables) track active sequence blocks using `OrderedDict` (doubly-linked list + hash table). While offering fast $O(1)$ pointer-swap touches, `OrderedDict` incurs $15\times$ memory bloat and cannot execute rank-threshold eviction without an $O(N)$ full table scan.

This benchmark suite evaluates **Expanse** across four pillars using realistic, deterministic token streams (Code Generation, Summarization, and Structured JSON Extraction).

---

## 2. Step 0 — Pre-Registered Hypotheses & Claims Ceiling

Following repository research discipline (`RULE[user_global]`, `docs/BENCHMARKING.md`), claims ceilings, expected wins, and **expected losses** are pre-registered prior to evaluation.

### 2.1 Pre-Registration Matrix

| Pillar / Arm | Metric | Expected Winner | Margin / Rationale |
|---|---|---|---|
| **Pillar 1: Code Gen ($\alpha$)** | Mean Acceptance Length $\alpha$ | **Expanse LSM** | Win ($\ge +5\%\dots 15\%$ higher $\alpha$ vs fixed 3-gram via variable-length 2-neighbour LCP). |
| **Pillar 1: Lookup Latency** | Candidate Lookup ($\mu\text{s}$) | **HF Fixed 3-Gram** | **Expected Loss** ($\approx 0.7\,\mu\text{s}$ vs $7\text{--}11\,\mu\text{s}$ for Expanse). Negligible vs $20\,\text{ms}$ model forward pass. |
| **Pillar 2: Live Memory** | Index RAM at $10^5\dots 10^7$ | **ExpanseMap** | Win ($5\times\dots 8\times$ lower RAM vs CPython `dict`; competitive with static NumPy). |
| **Pillar 2: Streaming Ingestion** | Dynamic Inserts / sec | **ExpanseMap** | Win ($50\times\dots 100\times$ faster than Sorted NumPy's $O(N)$ array reallocations). |
| **Pillar 2: Snapshot Load** | Disk $\to$ Memory Latency | **Sorted NumPy** | **Expected Loss** (`np.load` raw buffer mmap is faster than Expanse's heap deserialization). |
| **Pillar 3: C++ llama.cpp Lookup** | Draft Candidate Parity | **Tie (Exact Parity)** | Both stock `unordered_map` and `expanse::str_map` produce 100% identical draft rollouts. |
| **Pillar 3: C++ Lookup Latency** | Point Update & Query Latency | **Stock `unordered_map`** | **Expected Loss** (flat hash probe vs radix trie traversal; sub-$\mu\text{s}$ vs $9\,\mu\text{s}$). |
| **Pillar 4: Prefix-Cache Memory** | Index RAM at $10^5$ blocks | **ExpanseMap** | Win ($14\times\dots 15\times$ lower RAM vs `OrderedDict`). |
| **Pillar 4: Touch Latency** | LRU `move_to_end` | **`OrderedDict`** | **Expected Loss** ($O(1)$ pointer swing vs trie deletion + re-insertion). |
| **Pillar 4: Rank Eviction** | Evict Below Timestamp | **ExpanseMap** | Win (`count_below()` + range prune; `OrderedDict` is structurally incapable). |

---

## 3. The Four Pillars

### Pillar 1 — Speculative Draft Quality via Replay Verifier (`benches/bench_draft_quality.py`)
* **Workloads**:
  1. `HumanEval Code Generation` (high context reuse, recurring signatures, types, loops).
  2. `CNN/DailyMail Document Summarization` (recurring key phrases, entities).
  3. `Structured JSON Schema Extraction` (repeated schema keys, object types, enums).
* **Algorithms**:
  - `HF Fixed 3-Gram`: Sliding window fixed 3-gram exact match.
  - `HF Fixed 2-Gram`: Sliding window fixed 2-gram exact match.
  - `Expanse Fixed 3-Gram`: 21-bit bit-packed integer key trie (`ExpanseMap`).
  - `Expanse Variable-Length LSM`: 7-bit NUL-free encoded reversed context stream with 2-neighbour LCP search (`prev_at_or_before` + `next_at_or_after` in `ExpanseStrMap`).
  - `Dict Multimap Tree`: Python `dict[prefix, list[(token, count)]]` tree baseline.
  - `Expanse Draft Tree`: Multi-candidate continuation tree.
* **Metrics**: Mean Acceptance Length $\alpha$ ($\text{tokens/step}$), Acceptance Rate ($N_{\text{accept}} / N_{\text{draft}}$), Candidate Lookup Latency ($\mu\text{s}$).

### Pillar 2 — Million-Token Datastore Scale (`benches/bench_datastore_scale.py`)
* **Scale**: $N \in [10\text{k}, 50\text{k}, 100\text{k}]$ (quick) and $[100\text{k}, 500\text{k}, 1\text{M}, 5\text{M}]$ (full).
* **Competitors**:
  - `CPython dict[int, int]`: Standard Python hash map tracked via `tracemalloc` + `pympler.asizeof`.
  - `Sorted NumPy Array`: Contiguous `uint64` arrays (`keys` + `values`) searched via `np.searchsorted`.
  - `ExpanseMap`: Digital trie with internal allocator memory accounting (`.mem_used()`).
* **Metrics**: Total RAM (MB), Bytes per Token (B/entry), Bulk Build Throughput (tps), Continuous Streaming Ingestion Throughput (tps).

### Pillar 3 — Native C++ llama.cpp Lookup Decoding (`benches/bench_llama_lookup.cpp`)
* **Scope**: Standalone C++20 harness testing exact `common/ngram-cache.cpp` logic linked against `include/expanse.hpp` and release `libexpanse.so` / `libexpanse.dylib`.
* **Competitors**:
  - Stock `llama.cpp` nested hash map: `std::unordered_map<std::string, std::unordered_map<int32_t, int32_t>>`.
  - Expanse C++20 trie: `expanse::str_map<uint64_t>` with 7-bit NUL-free token encoding.
* **Context Scales**: $4\text{k}, 16\text{k}, 32\text{k}, 128\text{k}$ tokens.
* **Metrics**: Cache update latency (ns), draft generation latency ($\mu\text{s}$), total tokens drafted (verification of parity).

### Pillar 4 — Prefix-Cache KV-Block Indexing & LRU Eviction (`benches/bench_prefix_lru.py`)
* **Competitors**:
  - `collections.OrderedDict`: Standard vLLM/SGLang block table LRU implementation.
  - `ExpanseMap Ordered Table`: `(monotonic_ts << 32) | block_id` composite key.
* **Operations**:
  - Touch (`move_to_end` vs `remove` + `insert`).
  - Oldest LRU Eviction (`popitem(last=False)` vs `first()` + `remove()`).
  - Rank-Threshold Eviction (`count_below(ts_cutoff)`).
* **Metrics**: Memory footprint (MB), Touch throughput (ops/s), Eviction throughput (ops/s).

---

## 4. Benchmark Execution & Reproducibility

```bash
# 1. Quick smoke run (all 4 pillars + chart generation)
./docs/benchmarks/llm_inference/run.sh --quick

# 2. Full scaling run
./docs/benchmarks/llm_inference/run.sh

# 3. Direct component execution
PYTHONPATH=bindings/python python3 docs/benchmarks/llm_inference/benches/bench_draft_quality.py
PYTHONPATH=bindings/python python3 docs/benchmarks/llm_inference/benches/bench_datastore_scale.py
./docs/benchmarks/llm_inference/benches/bench_llama_lookup
PYTHONPATH=bindings/python python3 docs/benchmarks/llm_inference/benches/bench_prefix_lru.py
python3 docs/benchmarks/llm_inference/scripts/generate_charts.py
```
