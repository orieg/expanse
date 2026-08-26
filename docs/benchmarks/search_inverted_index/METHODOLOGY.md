# Search / Inverted-Index Benchmark Methodology — ExpanseSet vs Roaring

## 1. Problem statement

Search engines (Lucene, Tantivy, Quickwit) and OLAP engines (ClickHouse,
Pinot) store document IDs in **posting lists** and evaluate queries as Boolean
algebra over them (`AND`/`OR`/`AND NOT`), with dynamic pruning (WAND / MaxScore)
that skips postings via *advance-to-target*. **Roaring bitmaps** are the de
facto industry standard for these integer sets.

This suite measures where `ExpanseSet` (the Judy1 set: an expanse-partitioned
digital trie with SIMD/SWAR bitmap leaves) stands against Roaring on the three
operations that dominate an inverted index: Boolean algebra, WAND skip-scan,
and memory footprint.

The comparison baseline is **`roaring` 0.10** (the pure-Rust `roaring-rs` crate,
already a dependency), 64-bit variant `RoaringTreemap`, since document IDs here
are 64-bit.

---

## 2. Step 0 — Claims ceiling and expected losses (pre-registered)

This is written **before** the numbers and states, up front, the cells where
`ExpanseSet` is expected to **lose**. This project publishes losing cells
(the hashbrown suite corrected its own claims when a "win" did not reproduce);
the honest framing here matters more than usual because the underlying premise
is asymmetric:

### 2.1 The load-bearing honesty caveat: ExpanseSet has no native set algebra

The issue motivating this suite describes "direct bitwise algebra (AND, OR,
AND NOT, XOR) executed directly over compressed trie edges." **That kernel does
not exist in the code today.** `crates/expanse/src/set.rs` exposes point ops
(`contains`/`insert`/`remove`), ordered navigation (`next_at_or_after`,
`prev_at_or_before`, `first`/`last`, `by_count`, `count_range`), and iteration
(`iter`/`range`) — but **no** `intersection`/`union`/`difference` method, and no
`BitAnd`/`BitOr`/`BitXor` impl.

Therefore Pillar 1 does **not** compare two native set-algebra kernels. It
compares:

* **Roaring** — native, container-level kernels (`intersection_len` etc.) that
  move 64 docIDs per machine word in bitmap containers and gallop over array
  containers; and
* **ExpanseSet** — the Boolean result **composed at the application level** from
  the navigation primitives that do exist (lockstep iterator merge, leapfrog
  `next_at_or_after`, `contains` probing). This is exactly what an engine using
  `ExpanseSet` as a posting-list backend must do today.

**Consequence (expected):** Roaring is expected to win **every symmetric Boolean
cell, often by 1–3 orders of magnitude**, because per-element navigation cannot
match word-parallel container algebra. We report the measured factor rather
than hiding it. The claim ceiling for Pillar 1 is *"here is the real cost of
using ExpanseSet as a posting-list backend today, and how far a native kernel
would have to close the gap"* — **not** "ExpanseSet's set algebra beats
Roaring."

### 2.2 Roaring-rs scope: no run containers

`roaring-rs` 0.10 implements only **array** and **bitmap** containers — it has
**no run containers** and no `run_optimize`. CRoaring (the C library, exposed in
Rust as `croaring`) does have run containers and would compress dense/contiguous
data further. This suite compares against the pure-Rust crate that ships as a
dependency; **it does not claim anything about CRoaring**. A `croaring` arm is
named as future work (it did not build cleanly cross-platform here without
adding a C toolchain dependency to the matrix).

### 2.3 Per-pillar expectation table

| Pillar / cell | Expectation for ExpanseSet | Why |
|---|---|---|
| **P1 AND/OR — dense, symmetric** | **Lose, large margin** | Roaring bitmap containers do word-parallel algebra; Expanse walks per element. |
| **P1 AND/OR — sparse/zipfian, symmetric** | **Lose, smaller margin** | Roaring array-container gallop + word ops vs per-element merge. Zipfian is the closest symmetric cell. |
| **P1 AND — skewed size (tiny B into huge A)** | **Closest cell; possibly competitive** | Leapfrog touches ~\|B\|·depth elements; the candidate cell for parity, reported explicitly. |
| **P2 WAND skip-scan** | **Genuine contest** | `next_at_or_after` is a real O(depth) primitive; Roaring `advance_to` is a warm cursor. Small strides favour the cursor; deep skips over a big list may favour the fixed-depth re-descent. |
| **P2 instruction counts** | **Fairest arm** | Deterministic retired-instruction counts under callgrind — no wall-clock noise, both sides under one simulator. |
| **P3 memory — clustered/dense** | **Competitive at large N** | Bitmap leaves pack contiguous runs; but small N pays trie branch overhead (reported per-size). |
| **P3 memory — sparse 64-bit** | **Expected loss** | Deep low-occupancy trie branches vs Roaring's 2 B/docID array containers. |
| **P3 memory — multi-tenant shard** | **Candidate win** | Shared high-bit prefixes collapse in the trie; Roaring pays a container per populated 2^16 block. |

Any cell whose measured result contradicts this table is reported as-is; the
table is the pre-registration, not the conclusion.

---

## 3. The three pillars

### Pillar 1 — Boolean posting-list algebra (`search_boolean`)
* **Operations:** `AND` (∩), `OR` (∪), `AND NOT` (∖), measured as **cardinality**
  (no result materialization), so the number is algebra compute, not allocation.
* **ExpanseSet strategy:** adaptive — lockstep iterator **merge** when the two
  lists are within 32× in size, **leapfrog** `next_at_or_after` when one is
  ≥32× smaller (a query planner's choice). OR is an iterator merge; AND-NOT
  walks the left list and probes the right with `contains`.
* **Roaring strategy:** native `intersection_len` / `union_len` /
  `difference_len`.
* **Distributions:** dense, clustered, sparse, zipfian (see below). Density is
  set so `n/universe ≈ 0.5` and intersections are non-trivial.
* **Sizes:** 10^4, 10^5, 10^6, 10^7 docIDs per list (quick mode: 10^4, 10^5).
* **Skewed arm:** \|A\| ∈ {10^6, 10^7}, \|B\| = \|A\|/1000, AND only.

### Pillar 2 — WAND dynamic skip-scan (`search_wand`, `search_instructions`)
* **Primitive:** advance to the next docID `≥ target` over a **monotonically
  increasing** target sequence (the block-max WAND pivot advance).
* **ExpanseSet:** `next_at_or_after(target)` — a stateless O(depth) re-descent
  from the trie root per call.
* **Roaring:** `iter().advance_to(target)` — a stateful forward cursor.
* **Regimes:** `shallow` (stride ≈ 1, near-full sweep), `medium` (stride ≈ 64),
  `deep` (few long-range skips). Wall-clock ns/skip in `search_boolean`'s
  sibling `search_wand`; **deterministic retired-instruction counts** per skip
  in `search_instructions` (iai-callgrind, Linux only).

### Pillar 3 — Memory footprint (`search_memory`)
* **Metric:** live resident **heap bits per docID**, via a custom `GlobalAlloc`
  hook that tracks net live bytes (the same technique as the hashbrown memory
  pillar) — the fair apples-to-apples footprint while the set is queryable.
* **Secondary:** Roaring `serialized_size()` (the portable-format figure the
  Roaring literature quotes) and ExpanseSet `mem_used()` (the engine's own live
  accounting) as cross-checks.
* **Distributions:** dense, clustered, sparse, and multi-tenant `shard`
  (`(tenant<<40)|doc`). **Sizes:** 10^4, 10^5, 10^6.

---

## 4. Distributions

| Name | Shape | Real-world analogue |
|---|---|---|
| `dense` | contiguous run `[start, start+n)` | a term in every doc of a segment |
| `clustered` | bursts of 128 contiguous docIDs at random bases | topical / time-partitioned batches |
| `sparse` | uniform-random over `[0, 2n)` | a rare term scattered across the ID space |
| `zipfian` | power-law over `[1, 2n]`, s = 0.99 | recency/popularity-skewed assignment |
| `shard` | `(tenant<<40) \| doc`, 16 tenants (memory only) | multi-tenant shard IDs across the 64-bit space |

All generators are seeded (`StdRng`), so lists are byte-for-byte reproducible.
Correctness is asserted in-harness: every ExpanseSet cardinality is
`debug_assert_eq!` against the Roaring result before timing.

---

## 5. Measurement discipline

Follows `docs/BENCHMARKING.md`:

1. **Wall-clock harnesses** (`search_boolean`, `search_wand`, `search_memory`)
   use a median-of-batches microbench helper: each batch grows its repetition
   count until it accumulates ≥ 30–60 ms, then reports ns/op; the median of 5
   batches (3 in quick mode) is taken. Results pass through `black_box`.
2. **Published wall-clock numbers come only from the quiet reference host**, not
   from CI runners or a laptop under load. System load is snapshotted before and
   between arms; a run is discarded if a non-target process exceeds the
   `docs/BENCHMARKING.md` contention threshold.
3. **Deterministic instruction counts** (`search_instructions`, iai-callgrind)
   are exact and reproducible regardless of host load — the fairest
   cross-library arm. Requires valgrind (Linux; no arm64 macOS support).
4. `rm -rf target/criterion` is not required (these harnesses do not use the
   criterion sampler), but a clean `target` avoids stale-binary confusion.

Every published number is tagged with the measured host and commit. CI ratios
are **not** publishable numbers.

---

## 6. Reproducing

```bash
# From repository root — full suite + charts:
./docs/benchmarks/search_inverted_index/run.sh

# Quick smoke (10^4, 10^5 only):
./docs/benchmarks/search_inverted_index/run.sh --quick

# Deterministic instruction counts (Linux + valgrind):
cargo bench -p expanse-trie --bench search_instructions
```

JSON telemetry and SVG charts are written to
`docs/benchmarks/search_inverted_index/results/`.
