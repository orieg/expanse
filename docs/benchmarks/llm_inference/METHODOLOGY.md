# LLM Inference & Speculative Decoding Benchmark Methodology — Expanse vs Industry Baselines

## 1. Problem Statement & Architectural Decisions

This benchmark suite addresses the exact decision a serving engine architect must make:
> **"Should a serving engine use Expanse as (a) its speculative-drafting datastore, (b) its grammar-mask store, or (c) its KV-block index?"**

### The Core Architectural Questions
1. **Speculative Decoding Draft Quality (alpha)**: Per-step draft overhead in the harness (~2–61 µs, including incremental re-indexing — an upper bound on pure lookup cost) is negligible compared to the target model forward pass (15–50 ms). Throughput gains are strictly driven by **macro mean accepted tokens per step (alpha)**. Can variable-length suffix matching via a digital trie raise alpha over fixed N-gram PromptLookup policies and HuggingFace adaptive N -> 1 fallback on authentic reference streams?
2. **Dynamic Speculative Datastore Scaling**: Static contiguous window indices provide compact memory (12.0 B/token) and fast lookup, but require $O(N L \log N)$ full rebuilds on update. Does Expanse's $O(\text{depth})$ incremental insertion win on continuous streaming ingestion, and at what update batch size $B_{\text{crossover}}$ does static index periodic rebuild become slower?
3. **Grammar-Constrained Decoding Masks**: When serving constrained generation with 10,000+ DFA states (e.g. JSON schemas, SQL ASTs), dense bitmasks require >300 MB of cache memory. Can sparse set representations (`RoaringBitmap` and `ExpanseSet`) compress mask cache memory while maintaining fast candidate filtering via SIMD set algebra?
4. **Prefix-Cache KV-Block Table Indexing (Appendix)**: Does `ExpanseMap` deliver memory density and efficient timestamp rank eviction (`count_below()`) over `collections.OrderedDict`?

---

## 2. Step 0 — Math-First Theoretical Speedup Ceiling Model & Gating

In speculative decoding with verification:
$$\text{Throughput} = \frac{1 + \alpha}{T_{\text{verify}} + T_{\text{propose}}}$$

Because $T_{\text{propose}} \ll T_{\text{verify}}$ (e.g. 35 µs << 20,000 µs, representing < 0.2% of step time), the theoretical throughput speedup ceiling is strictly bounded by:
$$\text{tok/s Gain Ceiling} \le \frac{1 + \alpha_{\text{expanse}}}{1 + \alpha_{\text{baseline}}}$$

**Gating Rule (Research Discipline Rule 1 / B-9)**:
- A claim PASSES iff the **BCa 95% bootstrap CI lower bound of the paired per-task ceiling gain $\ge 5.0\%$ floor**, NOT iff the point estimate $\ge 5.0\%$.
- Paired per-task ceiling gain for task $i$:
  $$\text{gain}_i = \frac{\alpha_{i, \text{expanse}} - \alpha_{i, \text{adaptive}}}{1 + \alpha_{i, \text{adaptive}}}$$
- If the 95% CI lower bound is $< 5.0\%$ or spans zero, the outcome is recorded as `BOUNDARY_RESULT` or `INTERMEDIATE_floor_within_ci`, and Pillar C is skipped with the boundary result published.

---

## 3. Expected-Outcome Matrix (reconciled post-hoc)

Directional expectations for each cell were stated before measurement, but the
matrix below was **reconciled with the measured results afterwards** — the
exact figures in it are observed outcomes, not an independent pre-registration
(kept honest per the repo's research-discipline rules: a backfilled matrix
must not be presented as pre-registered).

*(measured: Apple M1 laptop, arm64-apple-darwin; results commit `86b1503c` — single-shot wall-clock, informational)*

| Pillar / Arm | Metric | Expected Winner | Expected Outcome / Pre-Registered Loss |
|---|---|---|---|
| **Step 0 / Pillar A** | Macro Reference-continuation α vs HF Adaptive | **HF Adaptive & Expanse** | Boundary: Code gains $+2.63\%$ tok/s ceiling (<5% gate), Summary in dead heat (-0.03%), JSON clears on small N=5 (+8.82%). |
| **Pillar A: Draft-Step Overhead** | Per-step draft overhead (propose + acceptance + re-indexing, Python harness — not pure lookup) | **HF PromptLookup** | **Expected Loss**: ~2–4 µs for HF vs ~30–61 µs for Expanse/Static Index (either way negligible vs 20 ms forward pass; Pillar B's native bench owns pure lookup latency). |
| **Pillar B: Static Memory** | Datastore RAM at 1M tokens | **Static Window Index** | **Expected Loss**: Static index uses 12.0 B/tok (11.44 MB); ExpanseStrMap uses 259.2 B/tok (247.23 MB, 21.6× overhead). |
| **Pillar B: Static Search** | Static Longest Match Latency | **Static Window Index** | **Expected Loss**: Contiguous array binary search beats pointer-chasing trie descent. |
| **Pillar B: Incremental Update** | Streaming Tokens / sec | **ExpanseStrMap** | **Expected Win**: 452k–621k streaming inserts/sec vs 163.1 ms static full rebuilds. |
| **Pillar B: Crossover Curve** | Batch Update Frequency $B$ | **ExpanseStrMap** | **Expected Win**: Expanse wins whenever update batches contain $B < 73{,}859$ tokens (at 1M scale). |
| **Pillar D: Full-Vocab Apply** | Mask Apply Latency | **Dense Bitmask** | **Expected Loss** (by construction). The previously published <0.1 µs figure came from a dead-code-eliminated loop; measurement pending re-run. |
| **Pillar D: Mask Cache RAM** | RAM across DFA states | **RoaringBitmap** | **Expected Win**: Dense uses 30.6 MB (2,000 states); Roaring (11.5 MB) wins 2.66× lower RAM. |
| **Pillar D: Top-100 Intersect** | Candidate ∩ Allowed Set | **RoaringBitmap & ExpanseSet** | Fast candidate filtering (0.6–1.9 µs) via native SIMD set algebra (#339). |
| **Pillar E: Prefix-Cache RAM** | Block Table RAM (1M blocks) | **ExpanseMap Table** | **Expected Win**: 9.48x lower RAM vs `OrderedDict` (23.2 MB vs 219.9 MB) — cross-accounting comparison (tracemalloc peak vs native bytes + idealized side map); see README caveat. |
| **Pillar E: Touch Throughput** | LRU Touch (`move_to_end`) | **OrderedDict** | **Expected Loss**: O(1) doubly-linked list pointer swings beat trie mutation (15.28M vs 2.04M tps at 1M in the committed run). |
| **Pillar E: Rank Eviction** | Evict Below Timestamp | **ExpanseMap** | Native `count_below()` prunes at **1.98M–2.16M items/sec**. The OrderedDict cell is definitional, not measured: monotonic touch timestamps make pop-until-cutoff O(evicted), so a symmetric twin is pending before this counts as a measured win. |

---

## 4. Workloads & Authentic Reference Datasets

1. **HumanEval Code Generations**: 40 distinct tasks (3,907 prompt tokens, 6,378 reference tokens) tokenized via `tiktoken/cl100k_base` from OpenAI HumanEval benchmark (`HumanEval/0..39`, MIT License).
2. **Document Summarization**: 9 distinct articles (56,449 prompt tokens, 690 reference tokens) tokenized via `tiktoken/cl100k_base` from Wikipedia Computer Science Corpus (CC BY-SA 4.0).
3. **Structured JSON Schemas**: 5 distinct schemas & payloads (69,524 prompt tokens, 68,639 reference tokens) tokenized via `tiktoken/cl100k_base` from SchemaStore Repository (Apache-2.0 / MIT).
4. **Multi-Document Datastore Corpus**: 1,000,000 unique sequential uint32 tokens tokenized from Python standard library source files via `scripts/build_corpus.py` (0 tiling, 0 repetition).
5. **Grammar DFA States**: 2,000 synthetic DFA state masks over a 128,000 vocabulary, generated in-bench by a deterministic LCG tier model (40% sparse k=20 / 35% medium k=1,280 / 25% dense k=12,800) in `crates/expanse/benches/bench_grammar_masks.rs`.

---

## 5. Execution & Reproducibility

```bash
# 1. Quick smoke verification
./docs/benchmarks/llm_inference/run.sh --quick

# 2. Full benchmark suite (all pillars + native Rust benches + charts)
./docs/benchmarks/llm_inference/run.sh
```
