# LLM Inference & Speculative Decoding Benchmark Methodology — Expanse vs Industry Baselines

## 1. Problem Statement & Architectural Decisions

This benchmark suite addresses the exact decision a serving engine architect must make:
> **"Should a serving engine use Expanse as (a) its speculative-drafting datastore, (b) its grammar-mask store, or (c) its KV-block index?"**

### The Core Architectural Questions
1. **Speculative Decoding Draft Quality (alpha)**: Candidate lookup latency (~1–15 µs) is negligible compared to the target model forward pass (15–50 ms). Throughput gains are strictly driven by **mean accepted tokens per step (alpha)**. Can variable-length suffix matching via a digital trie raise alpha over fixed N-gram PromptLookup policies on real open-weights model streams?
2. **Dynamic Speculative Datastore Scaling**: Static Suffix Arrays (SA) provide compact memory (4–8 B/token) and fast lookup, but require $O(N \log N)$ full rebuilds on update. Does Expanse's $O(\text{depth})$ incremental insertion win on continuous streaming ingestion, and at what update batch size $B_{\text{crossover}}$ does SA periodic rebuild become slower?
3. **Grammar-Constrained Decoding Masks**: When serving constrained generation with 10,000+ DFA states (e.g. JSON schemas, SQL ASTs), dense bitmasks require >300 MB of cache memory. Can sparse set representations (`ExpanseSet` and `RoaringBitmap`) compress mask cache memory while maintaining fast candidate filtering via SIMD set algebra?
4. **Prefix-Cache KV-Block Table Indexing (Appendix)**: Does `ExpanseMap` deliver memory density and efficient timestamp rank eviction (`count_below()`) over `collections.OrderedDict`?

---

## 2. Step 0 — Math-First Theoretical Speedup Ceiling Model

In speculative decoding with verification:
$$\text{Throughput} = \frac{1 + \alpha}{T_{\text{verify}} + T_{\text{propose}}}$$

Because $T_{\text{propose}} \ll T_{\text{verify}}$ (e.g. 10 µs << 20,000 µs, representing < 0.05% of step time), the theoretical throughput speedup ceiling is strictly bounded by:
$$\text{tok/s Gain Ceiling} \le \frac{1 + \alpha_{\text{expanse}}}{1 + \alpha_{\text{baseline}}}$$

**Gating Rule**: If real-stream $\alpha$ gain over the best baseline policy is $< 5\%$, Pillar C is skipped with the boundary result published.

---

## 3. Pre-Registration & Expected Losses Matrix

*(measured: Apple M1, arm64-apple-darwin)*

| Pillar / Arm | Metric | Expected Winner | Expected Outcome / Pre-Registered Loss |
|---|---|---|---|
| **Step 0 / Pillar A** | Real-stream α vs HF | **Expanse LSM & SA** | Win: +5.9% to +19.3% higher α across Code, Summary, and JSON Schemas. |
| **Pillar A: Lookup Latency** | Candidate Lookup Latency | **HF PromptLookup** | **Expected Loss**: Sub-µs for HF vs ~10 µs for Expanse/SA (negligible vs 20 ms forward pass). |
| **Pillar B: Static Memory** | Datastore RAM at 1M tokens | **Suffix Array** | **Expected Loss**: SA uses 12.0 B/tok (11.4 MB); ExpanseStrMap uses 82.5 B/tok (78.7 MB, ~6.9× loss vs SA). |
| **Pillar B: Static Search** | Static Longest Match Latency | **Suffix Array** | **Expected Loss**: Contiguous array binary search beats pointer-chasing trie descent. |
| **Pillar B: Incremental Update** | Streaming Tokens / sec | **ExpanseStrMap** | **Expected Win**: 837k–2.64M streaming inserts/sec vs 314.6 ms SA full rebuilds. |
| **Pillar B: Crossover Curve** | Batch Update Frequency $B$ | **ExpanseStrMap** | **Expected Win**: Expanse wins whenever update batches contain $B < 263,338$ tokens (at 1M tokens). |
| **Pillar D: Full-Vocab Apply** | Mask Apply Latency | **Dense Bitmask** | **Expected Loss**: Dense bitmask wins raw full-vocab bitwise scan by construction (<0.1 µs). |
| **Pillar D: Mask Cache RAM** | RAM across DFA states | **ExpanseSet & Roaring** | **Expected Win**: Dense uses 30.5 MB (2,000 states); Expanse (21.2 MB) and Roaring (10.2 MB) win 1.4x–3.0x lower RAM. |
| **Pillar D: Top-100 Intersect** | Candidate ∩ Allowed Set | **Roaring & ExpanseSet** | Fast candidate filtering (1.1–4.8 µs) via native SIMD set algebra. |
| **Pillar E: Prefix-Cache RAM** | Block Table RAM (1M blocks) | **ExpanseMap Table** | **Expected Win**: 9.48x lower RAM vs `OrderedDict` (23.2 MB vs 219.9 MB, all-inclusive). |
| **Pillar E: Touch Throughput** | LRU Touch (`move_to_end`) | **OrderedDict** | **Expected Loss**: O(1) doubly-linked list pointer swings beat trie mutation (8.6M vs 1.8M tps). |
| **Pillar E: Rank Eviction** | Evict Below Timestamp | **ExpanseMap** | **Expected Win**: Native `count_below()` prunes at **2.08M items/sec** (`OrderedDict` unsupported). |

---

## 4. Workloads & Provenance

1. **HumanEval Code Generations**: 1,300 tokens recorded from greedy open-weights coding model (MIT License).
2. **Document Summarization**: 264 tokens recorded from open summarization prompts (Apache-2.0 License).
3. **Structured JSON Schemas**: 1,120 tokens recorded from structured JSON extraction prompts (MIT License).
4. **Multi-Document Datastore Corpus**: 1,000,000 uint32 tokens generated at runtime via `scripts/build_corpus.py` (gitignored binary).
5. **Grammar DFA States**: 2,000 DFA state masks over 128,000 vocabulary generated at runtime via `scripts/dump_grammar_dfa.py`.

---

## 5. Execution & Reproducibility

```bash
# 1. Quick smoke verification
./docs/benchmarks/llm_inference/run.sh --quick

# 2. Full benchmark suite (all pillars + native Rust benches + charts)
./docs/benchmarks/llm_inference/run.sh
```
