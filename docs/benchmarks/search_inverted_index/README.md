# Search / Inverted-Index Benchmark: ExpanseSet vs Roaring

Reproducible suite comparing **`ExpanseSet`** (Judy1 — an expanse-partitioned
digital trie with SIMD/SWAR bitmap leaves) against **Roaring bitmaps**
(`roaring` 0.10, `RoaringTreemap`, 64-bit) on the operations that dominate a
search-engine inverted index: Boolean posting-list algebra, WAND dynamic
skip-scan, and memory footprint.

> **Read [`METHODOLOGY.md`](METHODOLOGY.md) first — especially Step 0.** The
> headline caveat is load-bearing: **`ExpanseSet` has no native set-algebra
> kernel.** The Boolean results below are the cost of *composing* AND/OR/AND-NOT
> from the set's navigation primitives (merge, leapfrog, `contains`), which is
> what an engine using `ExpanseSet` as a posting-list backend must do today —
> not a native-kernel-vs-native-kernel comparison. Roaring is expected to win
> the Boolean pillar, often by orders of magnitude, and it does. This suite
> publishes those losses.

---

## 1. Architectural feature matrix

| Capability / property | `roaring::RoaringTreemap` | `expanse::ExpanseSet` |
| :--- | :--- | :--- |
| **Underlying structure** | 32-bit-keyed map of 2^16 containers (array / bitmap) | Expanse-partitioned digital trie, SIMD/SWAR bitmap leaves |
| **Native Boolean algebra** | ✅ `intersection_len` / `union_len` / `difference_len`, `&` `\|` `-` `^` | ❌ **none** — composed from navigation primitives |
| **Skip-scan / advance-to-target** | ✅ `iter().advance_to(n)` (stateful cursor) | ✅ `next_at_or_after(n)` (stateless O(depth) re-descent) |
| **Ordered iteration / range** | ✅ | ✅ `iter` / `range` |
| **Rank / select** | ✅ `rank` / `select` | ✅ `count_below` / `by_count` |
| **Run containers (dense RLE)** | ❌ not in `roaring-rs` 0.10 (CRoaring only) | n/a (trie compresses runs structurally) |
| **64-bit docIDs** | via `RoaringTreemap` (map of 32-bit bitmaps) | native |
| **Serialization** | ✅ portable format | ❌ none today (`mem_used` accounting only) |

---

## 2. Key findings

*(measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, Ubuntu 22.04 / kernel 6.8, commit 29f86ddc; full non-quick suite via `run.sh`, host idle. Deterministic instruction counts (`search_instructions`, iai-callgrind) require valgrind and run in the `instruction-counts` CI job, not on this host.)*

<!-- RESULTS:START -->

**Summary of who wins each pillar (measured, not projected):**

| Pillar | Winner | Margin | Notes |
|---|---|---|---|
| **1. Boolean AND / OR / AND-NOT** | **Roaring** | 4× – 1406× | Every cell. `ExpanseSet` has no native algebra kernel; per-element composition cannot match word-parallel containers. Zipfian is the closest distribution. |
| **2. WAND skip-scan** | **Roaring** | 2× – 4× | Every cell. Expanse's `next_at_or_after` cost is notably *flat* (~13.6 ns dense, independent of stride/size) — a real O(depth) property — but the warm cursor still wins. |
| **3. Memory (bits/docID)** | **Mixed** | see below | Roaring wins most cells; **Expanse wins `shard` @ 10^5 (1.4× more compact)** and **ties dense/shard @ 10^6** (within ~4%). Small-N and sparse are Expanse losses. |

### Pillar 1 — Boolean posting-list algebra

![Boolean AND latency](results/bench_boolean_and.svg)

Roaring wins **every** Boolean cell — the expected result (METHODOLOGY §2.1):
`ExpanseSet` exposes no `intersection`/`union`/`difference`, so AND is an
adaptive iterator-merge/leapfrog, OR an iterator merge, and AND-NOT a `contains`
probe — all per-element, against Roaring's word-parallel containers. The gap
widens with size (more elements to walk one-at-a-time) and is smallest on
`zipfian`, where heavy skew forces Roaring through many small array containers
too.

**AND (symmetric, |A| = |B|):**

| Distribution | Size | Expanse | Roaring | Expanse vs Roaring |
|---|--:|--:|--:|---|
| dense | 10,000 | 9.41 µs | 523 ns | 18× slower |
| dense | 100,000 | 90.65 µs | 1.02 µs | 89× slower |
| dense | 1,000,000 | 900.24 µs | 4.57 µs | 197× slower |
| dense | 10,000,000 | 9.04 ms | 38.91 µs | 232× slower |
| clustered | 1,000,000 | 1.79 ms | 15.62 µs | 115× slower |
| clustered | 10,000,000 | 18.11 ms | 154.26 µs | 117× slower |
| sparse | 1,000,000 | 8.06 ms | 15.58 µs | 517× slower |
| sparse | 10,000,000 | 80.18 ms | 153.71 µs | 522× slower |
| zipfian | 10,000 | 13.55 µs | 3.77 µs | **4× slower** (closest) |
| zipfian | 1,000,000 | 2.75 ms | 267.18 µs | 10× slower |
| zipfian | 10,000,000 | 25.17 ms | 3.06 ms | 8× slower |

**OR** and **AND-NOT** follow the same shape and are the largest losses
(materializing the union/difference cardinality walks even more elements): OR
ranges from 8× (zipfian 10^4) to **798× slower** (dense 10^7); AND-NOT from 8×
(zipfian 10^4) to **1406× slower** (dense 10^7). Full per-size tables are in
[`results/baseline_boolean.json`](results/baseline_boolean.json).

**Skewed-size AND (|B| = |A|/1000)** — the cell where per-element leapfrog was
hypothesised to be competitive (METHODOLOGY §2.3). It is the **closest cell in
the whole Boolean pillar** on `zipfian` (2× slower), but on `dense`/`sparse`
Roaring's tiny-B container intersection is still 80×–386× faster:

| Distribution | \|A\| | \|B\| | Expanse | Roaring | Expanse vs Roaring |
|---|--:|--:|--:|--:|---|
| dense | 1,000,000 | 1,000 | 31.06 µs | 306 ns | 101× slower |
| zipfian | 266,218 | 733 | 60.77 µs | 30.31 µs | **2× slower** (closest) |
| sparse | 786,944 | 999 | 102.86 µs | 1.29 µs | 80× slower |
| dense | 10,000,000 | 10,000 | 299.12 µs | 775 ns | 386× slower |
| zipfian | 2,415,102 | 6,491 | 375.78 µs | 232.17 µs | **2× slower** |
| sparse | 7,867,812 | 9,997 | 2.58 ms | 9.27 µs | 279× slower |

> **Verdict:** as a posting-list Boolean backend today, `ExpanseSet` is not
> competitive with Roaring. Closing this requires the native trie-edge algebra
> kernel the motivating issue describes — the numbers above are the size of
> that gap.

### Pillar 2 — WAND dynamic skip-scan

![WAND skip-scan ns per skip](results/bench_wand_skipscan.svg)

A genuine contest — Expanse's `next_at_or_after` is a real O(depth) skip — but
Roaring's stateful `advance_to` cursor wins every regime by **2× – 4×**. The
Step 0 hypothesis that deep, long-range skips over a large list would favour the
fixed-depth re-descent is **refuted**: Roaring skips whole containers via its
outer map just as cheaply. The one architectural point in Expanse's favour is
**stride-independence** — dense skip cost is a flat ~13.6 ns whether advancing
by 1 or by 5,000 — whereas Roaring's cursor cost grows slightly with stride.

| List dist | Size | Regime | skips | Expanse | Roaring | Expanse vs Roaring |
|---|--:|---|--:|--:|--:|---|
| dense | 1,000,000 | shallow | 666,641 | 13.6 ns | 7.0 ns | 2× slower |
| dense | 1,000,000 | deep | 1,967 | 13.8 ns | 7.3 ns | 2× slower |
| dense | 10,000,000 | shallow | 6,666,150 | 13.7 ns | 6.9 ns | 2× slower |
| dense | 10,000,000 | deep | 2,047 | 14.5 ns | 8.9 ns | 2× slower |
| clustered | 10,000,000 | shallow | 13,332,173 | 20.9 ns | 6.0 ns | 3× slower |
| clustered | 10,000,000 | deep | 2,005 | 34.2 ns | 16.1 ns | 2× slower |
| sparse | 10,000,000 | shallow | 13,332,249 | 17.1 ns | 6.2 ns | 3× slower |
| sparse | 10,000,000 | deep | 2,005 | 18.2 ns | 10.1 ns | 2× slower |

Deterministic retired-instruction counts for the same skip-scan
(`search_instructions`, iai-callgrind) are the noise-free cross-check; they
require valgrind and run in the `instruction-counts` CI job.

### Pillar 3 — Memory footprint (live-heap bits per docID)

![Memory bits per docID](results/bench_memory_bits.svg)

The one pillar with Expanse wins. Roaring is more compact at small populations
(the trie pays fixed branch overhead that Roaring's flat containers do not), but
**the gap closes sharply with N**: at 10^6 dense the two are within 4% (1.10 vs
1.06 bits/docID = 0.137 vs 0.132 B/docID — Expanse's figure lands inside the
0.07–0.36 B/docID range the issue cites), and Expanse is **more compact than
Roaring on `shard` at 10^5** because shared high-bit tenant prefixes collapse in
the trie while Roaring allocates a container per populated 2^16 block. The
engine's own `mem_used()` accounting (excluding allocator overhead) is roughly
half the tracked heap — e.g. 0.52 bits/docID on dense 10^6.

| Distribution | Size | Expanse (heap) | Roaring (heap) | Roaring (serialized) | Expanse vs Roaring heap |
|---|--:|--:|--:|--:|---|
| dense | 10,000 | 52.84 | 6.91 | 6.58 | 8× slower |
| dense | 100,000 | 6.39 | 1.35 | 1.31 | 5× slower |
| dense | 1,000,000 | 1.10 | 1.06 | 1.05 | ~tie (1.04×) |
| clustered | 1,000,000 | 4.06 | 2.59 | 2.58 | 2× slower |
| sparse | 1,000,000 | 7.21 | 2.60 | 2.58 | 3× slower |
| shard | 10,000 | 69.02 | 28.70 | 16.26 | 2× slower |
| shard | 100,000 | 7.48 | 10.73 | 10.51 | **1.4× more compact** |
| shard | 1,000,000 | 1.22 | 1.07 | 1.05 | ~tie (1.15×) |

Full per-size data (including 10^4/10^5 rows for every distribution) is in
[`results/baseline_memory.json`](results/baseline_memory.json).

<!-- RESULTS:END -->disclosure

* **Pillar 1 is not a native-kernel comparison.** `ExpanseSet` has no
  `intersection`/`union`/`difference` method. The measured cost is
  application-level composition (adaptive merge/leapfrog for AND, iterator merge
  for OR, `contains` probe for AND-NOT). A native trie-edge algebra kernel — the
  feature the motivating issue describes but that does **not** exist in the code
  — is what would be needed to close the gap. The numbers quantify how large
  that gap is today.
* **Baseline is `roaring-rs` 0.10, not CRoaring.** `roaring-rs` has only array
  and bitmap containers (no run containers). CRoaring's run containers would
  compress dense/contiguous data further; this suite makes **no** claim about
  CRoaring. A `croaring` arm is future work.
* **Cardinality, not materialization.** Pillar 1 measures set-operation
  *cardinality* (no result set is built), isolating algebra compute from
  allocation on both sides.
* **Wall-clock numbers are single-host, idle.** Load was snapshotted before and
  between arms (see the run log). The deterministic instruction-count arm is the
  noise-free cross-check and lives in CI.
* **No synthetic peer review.** These are direct measurements; no LLM panel
  reviewed them.

---

## 4. How to reproduce

```bash
# Full suite + charts (from repository root):
./docs/benchmarks/search_inverted_index/run.sh

# Quick smoke (10^4, 10^5 only):
./docs/benchmarks/search_inverted_index/run.sh --quick

# Deterministic instruction counts (Linux + valgrind):
cargo bench -p expanse-trie --bench search_instructions
```

### Directory structure

```
docs/benchmarks/search_inverted_index/
├── README.md                    # This report
├── METHODOLOGY.md               # Rigor, Step 0 claims ceiling, expected losses
├── run.sh                       # 1-command reproduction runner
├── scripts/
│   ├── theme.py                 # Shared dual-theme SVG styling
│   ├── run_all.py               # Master orchestrator (3 wall-clock benches + charts)
│   └── generate_charts.py       # Dual-theme SVG generator (self-labels wins/losses)
└── results/                     # Raw JSON telemetry + generated SVGs
    ├── baseline_boolean.json
    ├── baseline_wand.json
    ├── baseline_memory.json
    ├── bench_boolean_and.svg
    ├── bench_wand_skipscan.svg
    └── bench_memory_bits.svg
```

The criterion/custom harnesses live in `crates/expanse/benches/search_*.rs`
(`search_boolean`, `search_wand`, `search_memory`, `search_instructions`) with
shared generators in `crates/expanse/benches/search_common/`.
