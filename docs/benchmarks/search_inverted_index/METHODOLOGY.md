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

> **Update (#339, superseding this pre-registration).** The native kernel this
> section says does *not* exist has since landed: `ExpanseSet::intersection_len`
> / `union_len` / `difference_len` (+ `&` `|` `-` `^`), a structural lockstep
> descent (`crates/expanse/src/algebra.rs`). Pillar 1 now measures **composed
> vs native vs roaring** per cell. The pre-registered expectation below —
> "Roaring wins every symmetric cell, often by 1–3 orders of magnitude" — held
> for the **composed** arm (4×–1413×) and is **refuted for the native arm**,
> which is within 3.84× on every symmetric cell and faster than Roaring on 15 of
> 48 symmetric cells (per the committed #348-run artifact,
> `results/baseline_boolean.json`), and faster on the dense/zipfian skewed AND
> (see README §Pillar 1). The text below is preserved as the original
> pre-registration.
>
> **Update (#348, second resolution — materialization).** #339 measured
> *cardinality* only; its materializing ops still merged the two iterators and
> re-inserted each key. #348 makes materialization structural (direct emission
> from the lockstep walk). Pillar 1 now carries a materializing arm measured in
> three ways (v2 direct emission, v1 ordered-merge + insert, roaring bitmap); the
> cardinality cells are unchanged (this pillar's kernel did not change in #348).
>
> Pre-registered targets for #348 *(target, not measured)* were: dense 1e7
> ≤ 1.2× roaring, clustered 1e7 ≤ 1.5×, no symmetric cell > 2×, each materialized
> cell within 2× of the corresponding cardinality cell. **Measured outcome: all
> four targets are MISSED.** v2 materialization at N=10⁷ vs roaring is dense AND
> 2.6×, clustered AND 7.1×, sparse AND 11.7×, zipfian AND 1.4×; v2/cardinality at
> N=10⁷ is 2.2×–6.0× (building the result tree costs more than counting it). The
> honest result: **v2 direct emission is 7×–225× faster at N=10⁷ than the v1
> insert-based path it replaces** (3.2× floor across the full 10⁴–10⁷ sweep,
> zipfian AND at 10⁴) — that is the #348 deliverable ("materializing ops are not
> structural" → now they are) — but it does not reach roaring on the symmetric
> cells, which need the level-2 65,536-key bitmap leaf that #348 explicitly left
> out of scope (a new node form with its own density-crossover design note). The
> gap is dispatch granularity + dependent loads, identical to the cardinality
> pillar's.
>
> **Frozen negative result (#348).** A prefetch + SIMD `BranchU × BranchU`
> presence-mask step was implemented and parity-tested but **dropped**: it caused
> a controlled +61 % dense-cardinality regression at N=10⁷ (native AND
> 56.5 µs → 90.9 µs; reference host, `4d720b0c` vs `c129836d`, composed/roaring
> controls identical to 4 s.f.), because `BranchU × BranchU` is by definition the
> high-overlap regime (both ≥ 192/256 populated) where a presence-mask skip has
> nothing to skip. Prefetch rode the same commit and was not isolated. Details in
> `ALGORITHMS.md` §3.4.

The issue motivating this suite describes "direct bitwise algebra (AND, OR,
AND NOT, XOR) executed directly over compressed trie edges." **That kernel did
not exist in the code when this suite was first written (#337); it landed in
#339.** `crates/expanse/src/set.rs` exposes point ops
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

> **Update (#340).** The **P2 WAND skip-scan** row is resolved: the pre-registered
> "deep skips may favour the fixed-depth re-descent" was refuted for the stateless
> `next_at_or_after`, and #340 adds a stateful cursor arm that reuses its descent
> path. See the Pillar 2 update note in §3 and the measured three-arm table in the
> README. The row above is preserved as the original pre-registration.

---

## 3. The three pillars

### Pillar 1 — Boolean posting-list algebra (`search_boolean`)
* **Operations:** `AND` (∩), `OR` (∪), `AND NOT` (∖), measured as **cardinality**
  (no result materialization), so the number is algebra compute, not allocation,
  **and** — since #348 — as **materialization** (the result set built: v2 direct
  emission vs v1 ordered-merge + insert vs roaring bitmap), so the cost of
  producing the result is measured too.
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

> **Update (#610 Resolved) — k-Way Aggregate Set Algebra.**
> The pairwise set-algebra kernels (#339 cardinality, #348 direct-emission materialization)
> required folding multi-predicate queries two operands at a time, allocating intermediate tries
> and re-walking them. Issue #610 adds $k$-way aggregate set algebra:
> `intersection_len_many`, `union_len_many`, `intersection_many`, `union_many` over `&[&ExpanseSet]`
> and `&[&ExpanseSet32]`.
>
> - **Novelty tier claimed:** `Engineering` (per Rule 18 / `GEMINI.md §1.7`).
> - **Primary hypothesis:** Direct $k$-way structural lockstep trie descent eliminates intermediate
>   allocations and achieves higher prune selectivity than pairwise cascades for $k \ge 3$,
>   beating the pairwise-composed Expanse baseline and matching/beating `roaring::MultiOps`
>   on structured posting-list intersections.
> - **Verification outcome (measured: Apple M-series, commit 109dc311):**
>   - **AND speedup over pairwise:** Mean 21.86× speedup, BCa 95% bootstrap CI [2.44×, 56.98×].
>     The CI lower bound ($2.44 \ge 1.0$) **PASSES** the pre-registered floor.
>     On dense cells ($k = 5, N = 100,000$; workload: domain_search_boolean), $k$-way intersection achieves 12,072 ns vs 10,737,958 ns
>     pairwise (>800× speedup due to avoiding intermediate materialized tries).
>   - **OR speedup over pairwise:** Mean 2.72× speedup, BCa 95% bootstrap CI [2.02×, 3.63×].
>     The CI lower bound ($2.02 \ge 1.0$) **PASSES** the pre-registered floor.
>   - **Comparison against `roaring::MultiOps`:** On clustered posting lists ($k = 5, N = 100,000$; workload: domain_search_boolean),
>     $k$-way intersection executes in 205,502 ns vs 938,253 ns for `roaring::MultiOps` (4.56× lower latency).
>   - **Expected losses confirmed:** $k = 2$ general walker overhead is eliminated by fast-pathing to
>     pairwise (`intersection(b)` / `union(b)`).
> - **Competitor twin:** `roaring::MultiOps` over `RoaringTreemap` at identical PRNG seed and key streams.

### Pillar 2 — WAND dynamic skip-scan (`search_wand`, `search_instructions`)

> **Update (#340, resolving the P2 pre-registration).** The Step-0 hypothesis
> (§2.3) — that Expanse's fixed-depth re-descent could beat Roaring's warm cursor
> on deep skips — was **refuted** for the *stateless* `next_at_or_after`, which
> re-descends from the root every call and lost every regime by 2×–4×. #340 adds
> the missing piece: a **stateful cursor** (`ExpanseSet::cursor().advance_to`)
> that keeps its descent path and re-descends only from the deepest ancestor
> whose expanse still covers the next target — a leaf-local search for near skips.
> The harness now measures **three arms** (stateless / cursor / Roaring); the
> stateless numbers are kept alongside for comparison. The original
> pre-registration and the two-arm framing are preserved below.

* **Primitive:** advance to the next docID `≥ target` over a **monotonically
  increasing** target sequence (the block-max WAND pivot advance).
* **ExpanseSet (stateless):** `next_at_or_after(target)` — a stateless O(depth)
  re-descent from the trie root per call.
* **ExpanseSet (cursor, #340):** `cursor().advance_to(target)` — a stateful
  forward cursor reusing the held descent path; leaf-local for near targets,
  O(levels crossed) for mid targets, root re-descent only for far/cross-expanse
  targets. Answers the same "smallest key `≥ target`" query as the stateless arm,
  so the two arms' sinks are bit-identical.
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
