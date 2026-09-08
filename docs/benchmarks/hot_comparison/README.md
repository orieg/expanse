# Expanse vs. HOT (Height Optimized Trie): Empirical Benchmark Suite

Head-to-head evaluation of `ExpanseSet` and `ExpanseMap` — and, for string keys,
`ExpanseStrMap` and `ExpanseBytesMap` (§6) — against **HOT**
([Binna, Zangerle, Pichl, Specht & Leis, SIGMOD 2018](https://dl.acm.org/doi/10.1145/3506692)),
reached through a C++ FFI shim over the reference implementation.

> **Tracking & provenance.** Delivers the HOT arm of
> [#660](https://github.com/orieg/expanse/issues/660); the string-key arms
> (§6) deliver [#693](https://github.com/orieg/expanse/issues/693) and carry
> their own provenance block.
> *(measured: reference host — Intel Core i9-12900F, 8P+8E/24 threads, 30 MiB L3,
> Ubuntu 22.04; HOT [`speedskater/hot`](https://github.com/speedskater/hot) `96bf6fb`,
> ISC; harness commit `0f4fd40c` for the integer arms; **the string arms (§6) were re-measured at `41dc7bfd` for [#723](https://github.com/orieg/expanse/issues/723)** with `run.sh strings`, at load 0.50 / 0.50 / 0.67 / 1.02 and a busy-CPU delta of 1.0-1.02 core-equivalents — the benchmark and nothing else — except §6.3's string insertion-order rows, re-measured at `64f8a3af` ([#772](https://github.com/orieg/expanse/issues/772)); `docs/benchmarks/hot_comparison/run.sh`; benchmark
> shell pinned to CPUs 0-15; both arms built for one ISA target —
> `-C target-cpu=haswell` and `-march=haswell -O3 -std=c++17 -DNDEBUG`; load average
> 0.55 / 0.65 / 0.68 / 0.94 across the run with the host's busy CPU at 1.0
> core-equivalents between every pair of snapshots; 15 rounds per cell, the arm timed first
> alternating per round (§12.1), median reported, BCa 95% bootstrap ratio intervals
> over 2,000 resamples in `results/`)*.
>
> Pre-registration, locked constraints and every amendment: [`METHODOLOGY.md`](METHODOLOGY.md).
> This is internal work; no external peer review has been performed on any claim here.

---

## 1. The headline: there is no single memory answer

Per-key cost for an expanse-partitioned trie is governed by **expanse occupancy**
`λ = N / (populated 2-byte-prefix expanses)`, not by population, and it is a
sawtooth rather than a curve (`METHODOLOGY.md` §9.4). So this suite publishes
memory as a **curve across λ** and refuses to publish a single cell (§9.6).

The reason is visible in the result. **Arm A's winner changes three times.**

![Memory across expanse occupancy](results/chart_memory_curve.svg)

HOT's two lines are flat; Expanse's dip into the shaded band and climb out of it
past the cascade is the whole finding. The shaded region is derived by comparing
the two arms cell by cell, not drawn by eye.


| λ | N | HOT B/key | `ExpanseSet` B/key | winner |
|---:|---:|---:|---:|---|
| 1 | 32,768 | 12.06 | 16.17 | HOT 1.34× |
| 2 | 65,536 | 11.90 | 14.89 | HOT 1.25× |
| 4 | 131,072 | 11.82 | 12.48 | HOT 1.06× |
| 8 | 262,144 | 11.77 | **9.98** | Expanse 1.18× |
| 15 | 491,520 | 11.71 | **8.26** | Expanse 1.42× |
| 23 | 753,664 | 11.69 | **8.11** | Expanse 1.44× |
| 30 | 983,040 | 11.70 | 13.37 | HOT 1.14× |
| 38 | 1,245,184 | 11.77 | 20.70 | HOT 1.76× |
| 46 | 1,507,328 | 11.68 | 21.93 | HOT 1.88× |
| 61 | 1,998,848 | 11.70 | 20.29 | HOT 1.73× |

**Expanse wins only in the band λ ∈ [8, 23].** Outside it, HOT wins — below,
because Expanse has not yet amortized its branch structure; above, because the
`LEAF_CAP = 32` overflow cascade has fired and each key costs its own 16-byte
edge (§9.4).

A single cell would have been true and misleading in either direction: at λ=15
this suite could have published *"Expanse uses 1.42× less memory than HOT"*, and
at λ=46 *"HOT uses 1.88× less memory than Expanse"*. Both are measurements of
the same two systems on the same instrument.

**HOT is flat.** 11.68–12.06 B/key across the entire swept range — a 3% spread
against Expanse's 2.7×. Holding fanout roughly constant by varying discriminative
bits per node is exactly the property its authors claim for it, and on this
instrument it delivers.

### The Expanse curve beyond this table — engine instrument, not the census

The table above is the suite's allocator instrument. The engine's own
deterministic accounting (`mem_used()`, host-independent, no wall clock) covers
a wider λ range and is what locates the teeth; the two instruments are not the
same quantity and are never mixed in one table (§9.3, §9.10.6). Set and map
flavors, uniform random keys, same PRNG and seed *(measured: deterministic
byte accounting; workload: `example_keyspace_density`;
`docs/assets/data/bench_assets.json` → `density_sweep`, commit 86daaddf; full
tables and the node census in `METHODOLOGY.md` §9.10)*:

| λ | cell | `ExpanseSet` B/key | `ExpanseMap<u64,u64>` B/key | where on the curve |
|---:|---|---:|---:|---|
| 15.26 | 1M @64 | 7.92 | 16.70 | the `memory-budget` cell |
| 19.84 | 1.3M @64 | **7.59** | 16.15 | first trough |
| 27.47 | 1.8M @64 | 10.51 | 17.58 | first knee |
| 30.52 | 2M @64 | 13.60 | 19.38 | 35.05% of expanses cascaded (census) |
| 48.83 | 800k @62 | 21.02 | 23.90 | first peak |
| 1,953 | 2M @58 | 8.80 | 18.51 | every level-6 expanse a `BranchU` |
| 4,688 | 1.2M @56 | **6.71** | 15.35 | second trough |
| 7,812 | 2M @56 | 12.98 | 18.74 | second tooth, 34.9% of sub-expanses cascaded |
| 10,547 | 2.7M @56 | 20.98 | 23.76 | second peak |

The curve repeats one byte level down at λ ≈ 256 × `LEAF_CAP`, so the memory
verdict of §1 — Expanse wins in a band and loses outside it — is a verdict
per tooth, not a verdict on "high λ". The census, seed sensitivity, the
`LEAF_CAP = 48` control with its read-path measurement, and the reconciliation
of this suite's 8.27 B/key cell with the 12.62 B/key of §9.3 are in
`METHODOLOGY.md` §9.10.

### Arm B — the value model decides it

| λ | N | HOT B/key | `ExpanseMap` B/key | Expanse advantage |
|---:|---:|---:|---:|---:|
| 1 | 65,536 | 35.88 | 23.87 | 1.50× |
| 8 | 524,288 | 35.74 | 18.99 | 1.88× |
| 23 | 1,507,328 | 35.68 | 16.26 | **2.19×** |
| 46 | 3,014,656 | 35.67 | 24.71 | 1.44× |
| 61 | 3,997,696 | 35.69 | 23.83 | 1.50× |

Expanse wins at every occupancy, and this cell is labelled
**`PASS_categorical_by_design`** rather than a win: HOT reaches its value through
a heap-allocated `std::pair` per entry, ~24 B/key of allocator-visible overhead
before any index structure, while Expanse packs the value into a `ValueSlot`.
That is a property of the value model, not evidence of an architectural
advantage, and §5.2 pre-registered it as such.

---

## 2. Latency at N = 1,000,000

Ratios are HOT ÷ Expanse, so **above 1.000 means Expanse is faster**. Every cell
is gated on the BCa 95% interval, never the point estimate (§8.4); a cell whose
interval spans parity is `BOUNDARY_RESULT` and claims no winner.

![Latency at N=1M](results/chart_latency_1m.svg)

Both exceptions sit against the parity line: `lookup_hit · map · random` is a
`BOUNDARY_RESULT` at 0.986 [0.970, 1.003], and `lookup_miss · set · random` is a
non-scan HOT win at 0.960 [0.953, 0.967]. At 10⁵ the map hit cell is the second
non-scan HOT win, 0.939 [0.888, 0.970].


### Point lookup, 100% hit

| Distribution | Arm | HOT ns | Expanse ns | Ratio | Verdict |
|---|---|---:|---:|---:|---|
| sequential | set | 19.45 | **4.18** | 4.673 | Expanse |
| clustered | set | 24.85 | **7.64** | 3.251 | Expanse |
| sparse | set | 21.83 | **9.85** | 2.422 | Expanse |
| random | set | 36.45 | **36.03** | 1.012 | Expanse |
| sequential | map | 43.40 | **13.81** | 3.451 | Expanse |
| clustered | map | 49.91 | **23.08** | 2.182 | Expanse |
| sparse | map | 43.90 | **10.21** | 4.839 | Expanse |
| **random** | **map** | 59.87 | 61.34 | **0.986** | **`BOUNDARY_RESULT`** |

### Point lookup, 50% hit / 50% rejection-sampled miss

> **Re-measured with the corrected probe builder ([#760](https://github.com/orieg/expanse/issues/760)).**
> The superseded cells drew the hit half of the stream from
> `population[..hits_wanted]`; the sort above confines that to one end of the
> keyspace while the misses span all of it. The builder now strides hits across
> the whole population, and this section is re-measured at `ae0c610d`.
>
> **The earlier disclosure said the ratios would stand because the defect was
> symmetric across both arms. That was wrong, and the re-run is what showed it.**
> Both arms did see the identical probe stream, but the two structures do not
> respond alike to *where* in the keyspace the hits land, so the ratio moved:
> seven `lookup_miss` cells shifted with intervals separated from the superseded
> figures, confirmed by two independent sweeps in the same direction — `set ·
> sparse · 10⁶` from 2.803 to 2.438/2.453 and `map · sparse · 10⁵` from 4.995 to
> 4.378/4.407 among them. Symmetric *in presence* is not symmetric *in
> magnitude*, the caveat §8.10 already carries for teardown cost.
>
> Cross-run spread exceeds any single run's interval
> ([#783](https://github.com/orieg/expanse/issues/783)): two sweeps of identical
> code separate on roughly a sixth of cells. The seven above are the ones **both**
> sweeps moved, in the same direction; a cell only one sweep moved is not claimed.

| Distribution | Arm | HOT ns | Expanse ns | Ratio | Verdict |
|---|---|---:|---:|---:|---|
| sequential | set | 19.24 | **8.16** | 2.374 | Expanse |
| clustered | set | 24.78 | **11.19** | 2.227 | Expanse |
| sparse | set | 21.81 | **9.51** | 2.453 | Expanse |
| **random** | **set** | **36.45** | 38.09 | **0.960** | **HOT** |
| sequential | map | 55.46 | **12.99** | 4.765 | Expanse |
| clustered | map | 62.65 | **18.50** | 3.433 | Expanse |
| sparse | map | 56.31 | **10.00** | 6.296 | Expanse |
| random | map | 76.34 | **62.06** | 1.245 | Expanse |

**The pre-registered uniform-random loss is confirmed on Arm A only on the miss
path, and refuted on Arm B.** §5.1 registered HOT winning uniform-random point
lookup at medium-high confidence, reasoning that random keys discriminate late
and force a deep descent at a fixed 8-bit span while HOT's variable bit selection
bounds height. On the set arm the miss path is a HOT win (0.960 [0.953, 0.967])
and the hit path is a narrow Expanse win (1.012 [1.004, 1.021]) — the registered
direction holds on one of the two. On the map arm it did not hold at all: the hit
cell claims no winner (0.986 [0.970, 1.003]) and the miss cell goes to Expanse
(1.245 [1.224, 1.266]), because HOT's pointer chase to its heap pair costs more
than the descent it saves.

These four cells are the ones §12.1's arm alternation moved most. At `5232af74`,
with HOT timed first in every round and Expanse inheriting its warmed cache, the
map hit cell read 1.399 and the set hit cell 0.998; alternating the arm timed
first puts them at 0.986 and 1.012. The direction of the map hit cell reversed
and it now claims no winner, which is the largest single consequence of the
harness change. **The boundary result is not one run's accident:** two
independent runs of the alternating harness put the map hit cell at 0.993
[0.977, 1.009] and 0.992 [0.977, 1.007], intervals that agree to the third
decimal *(measured: reference host, `5232af74` → `134a0471` → `0f4fd40c`)*.

### Insertion into a cold structure

| Distribution | Arm | HOT ns | Expanse ns | Ratio |
|---|---|---:|---:|---:|
| sequential | set | 58.62 | **4.83** | 12.119 |
| clustered | set | 62.51 | **13.19** | 4.747 |
| sparse | set | 57.72 | **29.17** | 1.977 |
| random | set | 78.11 | **30.83** | 2.532 |
| sequential | map | 72.88 | **12.87** | 5.661 |
| clustered | map | 76.82 | **21.35** | 3.591 |
| sparse | map | 71.94 | **30.70** | 2.344 |
| random | map | 94.84 | **27.15** | 3.504 |

Expanse wins every insertion cell, 1.63×–12.93×. §5.2 registered this as a *weak*
prediction; it landed stronger than registered. Every one of these is a
**sorted-order** cell — the shared generator hands both arms a sorted population
— and §4.1 publishes what the same cells do on a shuffled permutation.

---

### Per-pillar charts

Badges are driven by the cell's **verdict**, not by which bar is shorter: a cell
whose BCa interval spans parity carries a neutral `BOUNDARY` badge and claims no
winner (§8.4).

**Point lookup, 100% hit**

![Point lookup 100% hit](results/chart_lookup_hit.svg)

**Point lookup, 50% hit / 50% miss**

![Point lookup 50/50](results/chart_lookup_miss.svg)

**Insertion into a cold structure**

![Insertion](results/chart_insert.svg)

**Live heap memory, selected occupancies**

![Memory](results/chart_memory.svg)

Read that one against the curve above, not on its own — it samples five
occupancies either side of the cascade, and which side a cell lands on decides
its winner.

---

## 3. Ordered scan is a systematic loss, wider than predicted

**28 of this suite's 30 HOT wins are scan cells.** §5.1 registered HOT winning
short range scans at k=10 and k=100, carried forward from the loss
`art_comparison/` found unpredicted. The measurement is broader than that on two
axes, and both are recorded as **`UNPREDICTED LOSS`**:

- **k=1000 loses too**, which was not registered — `set`/`random`/100k is
  0.521 [0.519, 0.523], and `map`/`random`/100k is 0.407 [0.406, 0.409].
- **`sparse` loses as well as `random`**, also not registered.

| Arm | Dist | N | k=10 | k=100 | k=1000 |
|---|---|---:|---:|---:|---:|
| set | random | 10,000 | 0.535 | 0.431 | 0.424 |
| set | random | 100,000 | 0.766 | 0.554 | 0.521 |
| set | random | 1,000,000 | 0.837 | 0.743 | 0.732 |
| map | random | 10,000 | 0.663 | 0.495 | 0.473 |
| map | random | 100,000 | 0.781 | 0.477 | 0.413 |
| map | random | 1,000,000 | **1.822** | **1.706** | **1.624** |

The `map`/`random`/1M row is the exception and it reverses cleanly: Expanse wins
every scan width there, and by more than it did at `5232af74` (1.835 / 1.719 /
1.615 against 1.414 / 1.402 / 1.517). Scan outcome therefore depends on population as well as
on `k`, which is a second reason this suite does not publish single-population
cells.

![Ordered range scan](results/chart_scan.svg)

Scan on `sequential` and `clustered` is an Expanse win throughout and does not
appear in the loss list.

---

## 4. Scorecard

144 latency cells, 20 memory cells.

| | Count |
|---|---:|
| Expanse wins (CI excludes parity) | 111 |
| HOT wins (CI excludes parity) | 30 |
| `BOUNDARY_RESULT` (interval spans parity) | 3 |

Against the pre-registration:

| Registered | Outcome |
|---|---|
| HOT wins uniform-random point lookup (§5.1, medium-high) | **CONFIRMED** on Arm A's miss path only (0.960), and on Arm B's hit path at 10⁵ (0.939); **REFUTED** on Arm A's hit path (1.012) and on Arm B at 10⁶ (hit `BOUNDARY_RESULT` 0.986, miss 1.245) |
| HOT wins short range scans k=10, k=100 (§5.1, medium-high) | **CONFIRMED**, and wider — see §3 |
| HOT wins sparse-stride memory (§5.1, downgraded to low in §9.5) | **CONFIRMED** as part of the λ story: HOT wins above the cascade |
| Expanse wins Arm B memory (§5.2, high) | **CONFIRMED**, labelled `PASS_categorical_by_design` |
| Expanse wins insertion (§5.2, weak) | **CONFIRMED**, stronger than registered |
| Expanse wins sequential and sparse point lookup (§5.2, medium) | **CONFIRMED** |
| Scan losing at k=1000 and on `sparse` | **UNPREDICTED LOSS** |
| Arm A memory winner flipping three times across λ | **not pre-registered** |

### 4.1 The insert verdicts above are sorted-order verdicts (§12.2)

The shared generator sorts the population before handing it to either arm, and
every cell in §2, §3 and §6 is built in that order. That is not a neutral
choice, and it moves Expanse as well as HOT. The pair below runs the same
population in both orders at N = 10⁶ and is published with **no verdict against
the §5 or §10.7 pre-registrations** — those rows were locked on sorted order,
and reconciling them against a different workload in place is what §8.7 forbids.

**Integer arms, `random`, N = 1,000,000**

| Arm | Order | HOT alloc B/key | Expanse alloc B/key | Expanse `mem_used` B/key | `lookup_hit` HOT ÷ Expanse | `insert` HOT ÷ Expanse |
|---|---|---:|---:|---:|---:|---:|
| set | `sorted` | 11.70 | 13.95 | **13.60** | 1.021 [1.012, 1.030] | 2.532 [2.527, 2.539] |
| set | `shuffled` | 12.06 | 20.33 | **13.60** | 1.009 [1.000, 1.020] | 1.943 [1.933, 1.952] |
| map | `sorted` | 35.71 | 16.67 | **16.70** | 0.990 [0.977, 1.005] | 3.514 [3.505, 3.523] |
| map | `shuffled` | 36.22 | 23.62 | **16.70** | 1.072 [1.056, 1.091] | 2.828 [2.806, 2.851] |

**String arms, `short`, N = 1,000,000**

| Arm | Order | HOT alloc B/key | Expanse alloc B/key | Expanse `mem_used` B/key | `lookup_hit` HOT ÷ Expanse | `insert` HOT ÷ Expanse |
|---|---|---:|---:|---:|---:|---:|
| C · str | `sorted` | 12.23 | 47.83 | **42.77** | 1.268 [1.265, 1.271] | 1.396 [1.355, 1.417] |
| C · str | `shuffled` | 12.72 | 50.61 | **42.77** | 1.283 [1.280, 1.286] | 1.287 [1.280, 1.292] |
| D · map | `sorted` | 36.29 | 47.83 | **42.77** | 1.912 [1.895, 1.942] | 1.558 [1.542, 1.574] |
| D · map | `shuffled` | 36.83 | 50.60 | **42.77** | 1.911 [1.906, 1.916] | 1.773 [1.766, 1.781] |

*(measured: reference host — Intel i9-12900F, 8P+8E / 24 threads, P-core pin;
commit `64f8a3af`; `run.sh strings --only-sensitivity`; 15 rounds per cell,
BCa 95%; busy-CPU delta 1.01 core-equivalents across every phase;
[`results/baseline_string_sensitivity.json`](results/baseline_string_sensitivity.json))*.

These replace the four rows withheld under
[#772](https://github.com/orieg/expanse/issues/772), which were measured at
`0f4fd40c` against the two-allocation `ExpanseStrMap` leaf that
[#723](https://github.com/orieg/expanse/issues/723) replaced. The superseded
Expanse columns (69.16 / 71.99 index, 50.77 `mem_used`) stay registered in
`.github/superseded-figures.json`; the leaf change moved them to 47.83 / 50.61
and 42.77.

**`mem_used` is identical in both orders and the allocator figure is not** —
42.77 either way against 47.83 sorted and 50.61 shuffled. That is the same
split the integer rows show, and the reason the two instruments are published
side by side rather than one standing for the other: a digital trie's node
census is fixed by the key set, and only its allocator footprint depends on
arrival order.
- **`mem_used` is identical in both orders on every arm** — 16.70 B/key
  for the integer map and 42.77 for the string arms — while the allocator
  census moves on both arms. A digital trie's shape is fixed by the key set, not by the sequence
  the keys arrived in; the allocator's is not. That is the invariant
  `crates/expanse/tests/test_mem_used_order_invariant.rs` pins, and it is what
  makes the two columns readable side by side: the difference between them is
  attributable to the allocator, not to the trie.
- **Expanse's own insert cost roughly doubles on a shuffled population** —
  27.10 → 64.75 ns on the integer map — and its allocator footprint
  moves 16.67 → 23.62 B/key. The `masstree_comparison` sensitivity set
  measured `ExpanseMap` at 16.67 → 23.63 B/key on the same shape and population
  *(measured: reference host, `2ce92b7f`)*; this suite's own instrument reads
  16.67 → 23.62, an independent replication of that figure in another suite.
- **HOT moves too**, so the insert ratio narrows rather than reverses:
  3.514 → 2.828 on the integer map. Expanse still wins every insert cell in
  both orders here, unlike the Masstree arm, whose insert ratio flips from 0.760
  to 1.883 across the same pair.
- **`lookup_hit` barely moves**, as expected: the order affects the build, not
  the probe stream, which is shuffled in both cases.
- The mechanism is **unmeasured**. Nothing here attributes the shuffled-order
  cost to page faults, allocator span reuse or node-shape churn; #725's counter
  plan and #737's wrapper are what would.


---

## 5. What this suite does not claim

Stated before the numbers existed (§7) and unchanged by them:

1. **Single-threaded only.** No concurrency claim follows from this suite; HOT's
   ROWEX variant is separate scope.
2. **Integer keys in §1–§4.** Arm A is restricted to a 63-bit domain because
   HOT's inline value payload is 63 bits wide (§9.4), and is labelled
   `hot_set_63bit` throughout. String keys are §6, under their own ceiling
   (§10.9): claims attach to HOT's C-string configuration only, never to keys
   longer than 255 bytes, and an `ExpanseBytesMap` cell is a hash-indexed
   structure against a trie, not a trie comparison.
1. **§1–§4 are single-threaded.** No concurrency claim follows from them. The
   concurrent arm is §6, measured against HOT's ROWEX variant, and it carries
   its own, narrower ceiling (`METHODOLOGY.md` §11.6).
2. **Integer keys.** Arm A is restricted to a 63-bit domain because HOT's inline
   value payload is 63 bits wide (§9.4), and is labelled `hot_set_63bit`
   throughout. No string-key claim.
3. **x86-64 with AVX2 and BMI2 only.** HOT does not build on aarch64.
4. **One HOT implementation at one commit** — `speedskater/hot` `96bf6fb` built as
   documented, not "HOT" as a design, and not the SIGMOD paper's figures, which
   were measured on different hardware with a different harness.
5. **No cross-suite ratio.** A HOT-vs-Expanse ratio here is never set beside an
   ART-vs-Expanse ratio from `art_comparison/` (§8.12).
6. **The memory instrument is bytes held from the C allocator**, not the engine's
   `mem_used()`, because it is the only definition both arms satisfy. The two are
   not the same quantity and the gap is not a constant factor (§9.3) — and on
   the Expanse side it depends on **insertion order**: `hot_memory_curve` sorts
   and deduplicates its keys before inserting, so its Expanse cells carry a
   1.03× gap where a generator-order build of the same keys carries 1.59×
   (§9.10, workload: `hot_instrument_bridge`). Every allocator-instrument cell
   in this suite is a sorted-order cell; the repo's `bytes/key` table is
   generator-order and `mem_used()`, and neither is a re-measurement of the other.

---

## 6. String keys (#693): `ExpanseStrMap` and `ExpanseBytesMap` against HOT's C-string configuration

> **Measured with the corrected probe builder.** The string builder's hit
> sampling was fixed alongside the integer one ([#760](https://github.com/orieg/expanse/issues/760)), and this section's
> artifacts were measured at `7fe02c0b`, which postdates that fix. These cells
> need no re-measurement; §2's integer cells predated it and were re-run.

> *(measured: reference host — Intel Core i9-12900F, 8P+8E/24 threads, 30 MiB L3,
> Ubuntu 22.04 / kernel 6.8; HOT `96bf6fb`; harness commit `7fe02c0b`;
> `docs/benchmarks/hot_comparison/run.sh strings`; benchmark shell pinned to CPUs
> 0-15; both arms `-C target-cpu=haswell` / `-march=haswell -O3 -std=c++17
> -DNDEBUG`; load average 0.57 / 0.57 / 0.98 / 0.99 / 1.00 at start, after the gate,
> after the memory sweep and at the end; 15 rounds per cell, the arm timed first
> alternating per round (§12.1), median reported,
> BCa 95% bootstrap ratio intervals over 2,000 resamples;
> `results/baseline_string_latency.json`, `results/baseline_string_memory.json`;
> gate transcript `results/string_validate.log`)*. Pre-registration:
> [`METHODOLOGY.md` §10](METHODOLOGY.md#10-string-key-arms-693-pre-registration).
> Tables in this section are the output of `scripts/string_tables.py` over those
> two files. Internal work; no external peer review.

Three pairings (§10.2): **Arm C** `hot_str_ptr` — HOT's shipped string
configuration, `HOTSingleThreaded<const char*, IdentityKeyExtractor>`, against
`ExpanseStrMap` storing the same key pointer as its value; **Arm D**
`hot_str_map` — HOT through a heap `std::pair<const char*, uint64_t>` against
`ExpanseStrMap` string → `u64`; **Arm E** `hot_bytes_ptr` — the same HOT
configuration against the **unordered, hash-indexed** `ExpanseBytesMap`, which is
not a trie comparison and has no scan pillar. Five key shapes (§10.5): `short`
(8–16 random alphanumerics), `counter` (`k` + 11 digits), `prefixed` (96 shared
bytes + 24 random), `skewed` (Pareto lengths, 4–192), `beyond` (256 shared bytes
+ 16 random, 272 bytes). The strings are the harness's, one heap allocation
each; the census counts them on neither side (§10.3).

### 6.1 The losses first

**Ordered scan is still a loss in every cell, and it is still the largest loss
in this suite — but it is roughly half the loss it was.** All 72 scan cells
with a HOT column go to HOT; HOT ÷ Expanse runs from 0.637 [0.633, 0.641]
(`counter`, Arm D, k = 10, N = 1M) down to 0.048 [0.048, 0.049] (`skewed`,
Arm C, k = 1000, N = 100k). That is the pre-registered high-confidence loss
(§10.7), **`CONFIRMED`** on all 72.

> **Re-measured after [#722](https://github.com/orieg/expanse/issues/722), and
> the comparison it replaces was not symmetric.** This pillar drove
> `ExpanseStrMap`'s `next_at_or_after` / `next_after` — a fresh root descent
> per element returning a heap-allocated key the arm then discarded — against
> HOT's `lower_bound` plus an **incremental iterator**. A cursor was being
> measured against repeated seeks (§8.3), which is why the loss was read as a
> statement about the surface rather than about the trie. `ExpanseStrMap` now
> has a cursor and both arms are incremental.
>
> **Every one of the 90 scan cells improved, by 1.38× to 11.21× (median
> 2.22×)** — Arm C `beyond` at k = 1000, N = 100k went 314.32 → 28.03 ns.
> Measured as a paired pair of runs differing only in the surface:
> `origin/main` at `82f400b0` against the cursor at `7fe02c0b`, same builder,
> same start distribution, same quiet host. The 135 non-scan cells drifted a
> median 0.7% between those runs (p90 3.2%; a few small-N cells reach 19%),
> which is the floor the comparison resolves; the smallest scan improvement is
> 38%.
>
> **The surface was a cause, not the cause.** HOT still wins 72 of 72. Halving
> Expanse's time did not flip a single cell, which bounds how much of this
> pillar the navigation surface ever explained and leaves the remainder a
> question about the descent rather than the API. The superseded extremes
> (0.375 best, 0.017 worst) are registered in
> `.github/superseded-figures.json`.

| Arm | Shape | N | k=10 | k=100 | k=1000 |
|---|---|---:|---:|---:|---:|
| C · ptr | `counter` | 1,000,000 | 0.507 [0.503, 0.511] | 0.135 [0.132, 0.139] | 0.077 [0.076, 0.078] |
| C · ptr | `prefixed` | 1,000,000 | 0.455 [0.450, 0.459] | 0.173 [0.167, 0.178] | 0.100 [0.097, 0.103] |
| C · ptr | `short` | 1,000,000 | 0.462 [0.458, 0.468] | 0.144 [0.139, 0.149] | 0.083 [0.081, 0.085] |
| C · ptr | `skewed` | 998,150 | 0.369 [0.365, 0.373] | 0.104 [0.101, 0.108] | 0.059 [0.058, 0.061] |
| D · map | `counter` | 1,000,000 | 0.637 [0.633, 0.641] | 0.173 [0.171, 0.176] | 0.093 [0.091, 0.094] |
| D · map | `prefixed` | 1,000,000 | 0.566 [0.561, 0.570] | 0.242 [0.239, 0.246] | 0.131 [0.128, 0.134] |
| D · map | `short` | 1,000,000 | 0.601 [0.597, 0.606] | 0.204 [0.201, 0.206] | 0.112 [0.111, 0.114] |
| D · map | `skewed` | 998,150 | 0.476 [0.473, 0.480] | 0.150 [0.148, 0.152] | 0.080 [0.079, 0.082] |
| C, D | `beyond` | any | Expanse 28–113 ns/element; HOT column withheld (§10.4) | | |

The 10k and 100k rows are in `results/baseline_string_latency.json`; none is
above 0.22.

**Memory ownership on `short` still goes to HOT; `skewed` no longer does.**
§10.7 registered, at low-medium confidence, that Expanse would win the
`ownership` column on `short` and `counter`. On `counter` it does; on `short`
it does not — 36.23 against 47.83, a narrower margin than the one first
published — and `skewed` has since crossed to Expanse, 41.28 against HOT's
41.98 *(workload: `hot_str_ptr`)*:

| Shape (Arm C, N = 1M) | external (exact) | HOT index | Expanse index | **HOT ownership** | **Expanse ownership** | Expanse `mem_used` |
|---|---:|---:|---:|---:|---:|---:|
| `short` | 24.00 (13.00) | 12.23 | 47.83 | **36.23** | 47.83 | 42.77 |
| `skewed` | 29.76 (15.27) | 12.22 | 41.28 | 41.98 | **41.28** | 37.71 |
| `counter` | 24.00 (13.00) | 11.42 | 20.53 | 35.42 | **20.53** | 19.56 |
| `prefixed` | 136.00 (121.00) | 12.23 | 63.49 | 148.23 | **63.49** | 54.77 |
| `beyond` | 280.00 (273.00) | withheld | 47.83 | withheld | 47.83 | 46.78 |

`ExpanseStrMap` holds a 12-byte key in about 48 bytes, and the gate's
allocation counts are still what explains the gap: **115,916** allocations for
100,000 `short` keys against HOT's 4,566 (`results/string_validate.log`), or
roughly one per key against HOT's one per twenty-two. The engine's own
`mem_used` (42.77 B/key) sits 5.1 B/key under the allocator column, which is
the allocator's rounding on those allocations (§9.3 reason 1, now on the string
path). The §10.7 prediction is still **`REFUTED`** on `short`, and it remains a
finding about `ExpanseStrMap`'s leaf representation rather than about HOT.

> **This paragraph is the corrected form; the original figures are superseded,
> not overwritten (§8.7).** When first published it read *a 13-byte key in
> about 69 bytes* and *204,791 allocations*, measured against a leaf that spent
> **two** allocations on every key not resolved inside a terminal 8-byte chunk
> — a `StrSuffix` shell plus a separate byte buffer, each rounded by the
> allocator. [#723](https://github.com/orieg/expanse/issues/723) made the leaf
> a single allocation carrying its bytes inline, and this suite's string arms
> were re-measured on the reference host at the commit that did it. Two
> corrections travel with the number: the shape's mean key length is **12.0**,
> not 13 — the generator is `8 + rng % 9`, and both suites' artifacts report
> 12.0 — and `mem_used`'s shortfall against the allocator column is 5.1 B/key
> here, where the two-allocation leaf made it 18. The superseded figures are
> registered in `.github/superseded-figures.json`.

**The `index` column goes to HOT on every shape, categorically.** HOT holds
11.4–12.2 B/key of index on every representable shape at N = 1M because it
stores an 8-byte pointer plus node bits and nothing else; Expanse holds the key
bytes. Registered in §10.3 as **`PASS_categorical_by_design` in HOT's favour**
and labelled so: the contest is the `ownership` column above, where HOT's
external string table is added back.

**`prefixed` point lookup on Arm C is HOT's, as registered.** 100% hit
0.765 [0.763, 0.767]; 50/50 0.895 [0.885, 0.899] (N = 1M). This is the regime
HOT is designed for — 96 shared bytes that its discriminative-bit selection
skips and `ExpanseStrMap` descends one chunk at a time — and it is the one
place in the string suite where the pre-registered loss on HOT's home ground
landed as predicted. At N = 100,000 the 50/50 cell is 0.993 [0.957, 1.033].

**`ExpanseBytesMap` (Arm E) loses insertion everywhere and most 100%-hit
lookups, and its index is the heaviest thing measured here.** Insert:
0.522 [0.520, 0.523] on `counter`, 0.638 [0.634, 0.642] on `short`,
0.658 [0.655, 0.661] on `skewed`, 0.744 [0.740, 0.747] on `prefixed` (N = 1M).
100% hit: HOT on `prefixed` 0.936 [0.932, 0.939], `short` 0.928 [0.925, 0.930],
`skewed` 0.990 [0.981, 0.995]; Expanse only on `counter` 1.034 [1.030, 1.037].
Its index costs 96.6–102.1 B/key on 12–15-byte keys and 192.7 B/key on
`prefixed` — a hash-trie entry, a boxed collision bucket, the bucket's vector,
and a boxed copy of the key. None of this was pre-registered (§10.7 declined to
predict Arm E); it is reported as `not pre-registered` and it is the largest
per-entry footprint in either HOT suite. Arm E does win every 50/50 cell at
N = 1M (1.104–1.170).

**`UNPREDICTED LOSS`: `counter` 100%-hit lookup on Arm C at small N.** §10.7
registered `counter` lookup as a high-confidence Expanse win. It is one at
N = 1M (1.055 [1.051, 1.059]) and a HOT win at N = 10,000
(0.856 [0.845, 0.868]) and N = 100,000 (0.875 [0.861, 0.887]). The
prediction was stated without a population and was wrong below a million keys.

### 6.2 Where the pre-registration was refuted in Expanse's favour

These are reported with the same prominence as the losses (§6 taxonomy). At
N = 1M, HOT ÷ Expanse:

| Cell | Registered (§10.7) | Measured | Label |
|---|---|---:|---|
| `skewed` 100% hit, Arm C | HOT wins (low) | 1.169 [1.165, 1.173] | **`REFUTED`** |
| `skewed` 100% hit, Arm D | HOT wins (low) | 1.741 [1.737, 1.746] | **`REFUTED`** |
| `skewed` 50/50, Arm C | HOT wins (low) | 1.370 [1.364, 1.375] | **`REFUTED`** |
| `skewed` 50/50, Arm D | HOT wins (low) | 1.969 [1.962, 1.976] | **`REFUTED`** |
| `prefixed` 100% hit, Arm D | HOT wins (medium-high) | 1.072 [1.070, 1.075] | **`REFUTED`** |
| `prefixed` 50/50, Arm D | HOT wins (medium-high) | 1.244 [1.241, 1.248] | **`REFUTED`** |
| `prefixed` insert, Arm C | HOT wins (medium) | 1.210 [1.207, 1.213] | **`REFUTED`** |
| `prefixed` insert, Arm D | HOT wins (medium) | 1.298 [1.293, 1.303] | **`REFUTED`** |

The `skewed` row is the one §10.7 flagged as "registered because the issue
expects it; the mechanism reading does not support it". The mechanism reading
was right and the registration was wrong, in every population and on both
`ExpanseStrMap` arms. On `prefixed`, the loss that held on Arm C reverses on
Arm D exactly as the integer arms' uniform-random loss did (§2): HOT's
per-entry heap pair costs more than the descent it saves.

### 6.3 Confirmed wins, and the rest

`counter` at N = 1M is Expanse's on both `ExpanseStrMap` arms: 100% hit
1.055 [1.051, 1.059] (C) and 1.560 [1.554, 1.568] (D); insert
1.467 [1.463, 1.471] (C) and 1.723 [1.718, 1.727] (D) — **`CONFIRMED`**, with
the small-N caveat of §6.1. `short` 100% hit on Arm C is 1.166 [1.162, 1.170],
**`CONFIRMED`**. Arm D's memory is `PASS_categorical_by_design` for Expanse on
every representable shape (HOT ownership 59.4–172.2 B/key against Expanse
20.5–71.8), as §10.7 registered. The cells §10.7 declined to predict — 50/50
lookups on `short` and `skewed`, insertion on `short` and `skewed`, and all of
Arm E — are `not pre-registered`; at N = 1M the `ExpanseStrMap` ones are
Expanse's (1.14–2.08), the Arm E ones split as described above.

![String keys, latency at N = 1M](results/chart_string_latency.svg)

### 6.4 Memory across the population sweep

![String keys, Arm C ownership across N](results/chart_string_memory_sweep.svg)

| N | `counter` HOT / Expanse | `short` HOT / Expanse | `skewed` HOT / Expanse | `prefixed` HOT / Expanse | `beyond` — / Expanse |
|---:|---:|---:|---:|---:|---:|
| 1,000 | 44.7 / 91.2 | 44.8 / 85.0 | 51.2 / 78.8 | 157.5 / 101.3 | — / 86.2 |
| 10,000 | 36.6 / 27.5 | 37.0 / 54.3 | 42.7 / 47.6 | 149.1 / 70.2 | — / 54.4 |
| 100,000 | 35.5 / 21.1 | 36.1 / 42.5 | 42.0 / 41.3 | 148.1 / 58.0 | — / 42.6 |
| 125,000 | 35.5 / 21.0 | 36.4 / 46.5 | 42.0 / 41.3 | 148.4 / 62.2 | — / 46.6 |
| 150,000 | 35.5 / 20.9 | 36.7 / 49.8 | 42.0 / 41.3 | 148.7 / 65.4 | — / 49.7 |
| 200,000 | 35.5 / 20.8 | 36.5 / 50.5 | 42.0 / 41.3 | 148.5 / 66.0 | — / 50.5 |
| 1,000,000 | 35.4 / 20.5 | 36.2 / 47.8 | 42.0 / 41.3 | 148.2 / 63.5 | — / 47.8 |

Arm C ownership, B/key; the 2k, 5k, 20k and 500k rows are in
`results/baseline_string_memory.json`. **HOT is flat again** — 35.4–36.7 B/key
on every 12–15-byte shape from 10k to 1M, the string analogue of the integer
arms' 11.68–12.06 — and `counter` is the only shape where Expanse's line drops
under it.

**The registered chunk-occupancy hypothesis is consistent with the sweep and
not confirmed by it.** §10.5 predicted a `LEAF_CAP` cascade in the
discriminating chunk map near N ≈ 1.23 × 10⁵ for the random-alphanumeric
shapes and none for `counter`. Between 100k and 150k the Expanse line rises
on exactly those shapes — `short` 64.0 → 68.0 → 71.2, `prefixed`
66.6 → 70.8 → 73.9, `skewed` 42.5 → 46.6 → 49.7 — and `counter` does not
move (21.1 → 21.0 → 20.9). The step is about +11%, not the 2.7× swing of the
integer arm, because stored key bytes dominate the per-key cost. The
single-variable test that would confirm the mechanism — changing the alphabet
width and watching the step move — was not run, so the rows stay on a
population axis and the re-expression against `λ_chunk` §10.5 conditionally
promised is **not** made. Hypothesis, partially supported.

### 6.5 The 255-byte window: a capability finding about HOT

HOT discriminates C-string keys on their first 255 bytes (`MAX_STRING_KEY_LENGTH`,
measured at the boundary: 254- and 255-byte keys differing in their last byte
are two entries, 256- and 300-byte keys are one; §10.10). On `beyond`, whose
272-byte keys share a 256-byte prefix, HOT's `insert` reported 1 of 1,000 keys
new, the trie walked 1, and `lookup` found 1 — no false positive, because HOT
confirms every leaf with a full `strcmp`, but **silent population loss under a
build that reports success**, the same class as the integer arms' §3.1. Every
`beyond` cell therefore publishes the Expanse figure alone with the HOT column
withheld (45 latency cells, 36 memory cells), never a HOT number over a smaller
population. The Expanse side is unrestricted: `ExpanseStrMap` holds all 10⁶
272-byte keys at 47.83 B/key and `ExpanseBytesMap` at 352.26 B/key *(workload:
`hot_string_memory`)*; their 100%-hit lookups at N = 1M take 346.16 ns and
326.92 ns respectively *(workload: `hot_string_latency`)*.

### 6.6 Scorecard

225 latency cells, 180 memory cells. Latency cells with a HOT column: 180.

| | Count |
|---|---:|
| HOT wins (CI excludes parity) | 98 — of which 72 are scan cells |
| Expanse wins (CI excludes parity) | 77 — none is a scan cell |
| `BOUNDARY_RESULT` | 7 |
| HOT column withheld (`beyond`, §10.4) | 45 |

Against §10.7:

| Registered | Outcome |
|---|---|
| HOT wins ordered scan, every k, every shape (high) | **CONFIRMED**, 72 of 72 — still, after [#722](https://github.com/orieg/expanse/issues/722) moved every cell 1.38×–11.21× in Expanse's favour |
| HOT wins `prefixed` point lookup (medium-high) | **CONFIRMED** on Arm C; **REFUTED** on Arm D |
| HOT wins `prefixed` insert (medium) | **REFUTED** on both arms |
| HOT wins the `index` memory column on long and skewed keys (high, categorical) | **CONFIRMED**, `PASS_categorical_by_design` in HOT's favour, on every shape |
| HOT wins `skewed` point lookup (low) | **REFUTED** on both arms, at every population |
| Expanse wins `counter` lookup and insert (high) | **CONFIRMED** at N = 1M; **UNPREDICTED LOSS** on 100%-hit lookup at 10k and 100k |
| Expanse wins `short` 100%-hit lookup, Arm C (medium) | **CONFIRMED** |
| Expanse wins `ownership` memory on `counter` and `short` (low-medium) | **CONFIRMED** on `counter`; **REFUTED** on `short` |
| Expanse wins Arm D memory (high, categorical) | **CONFIRMED**, `PASS_categorical_by_design` |
| `λ_chunk` cascade near N ≈ 1.23 × 10⁵ (hypothesis) | consistent, not confirmed — §6.4 |
## 7. The concurrent arm: the write-concurrency loss, measured

Delivers [#692](https://github.com/orieg/expanse/issues/692): HOT's **ROWEX**
variant (concurrent insert and lookup, no deletion) against `SyncExpanseSet`
and `SyncExpanseMap`. Pre-registration, locked constraint decisions and the
expected-losses matrix are `METHODOLOGY.md` §11; nothing there was edited after
measurement.

> *(measured: reference host — Intel Core i9-12900F, 8P+8E/24 threads, 30 MiB L3,
> Ubuntu 22.04, kernel 6.8; HOT `96bf6fb` with its pinned TBB 2018 `4c73c3b`,
> built from the nested submodule, no system TBB; harness commit `d3bc49c0`;
> `docs/benchmarks/hot_comparison/run.sh --only-concurrent`; benchmark shell
> pinned to CPUs 0–15 and every row records `Cpus_allowed_list 0-15`;
> writers + readers ≤ 16; both arms `-C target-cpu=haswell` / `-march=haswell`,
> both on glibc 2.35 `malloc`; load average 0.40 at start, 6.64 after with 5.83
> cores busy across the sweep — its own threads, which is why it runs last and
> is gated on the start snapshot; 15 rounds per cell, arms interleaved per round,
> medians reported, BCa 95% bootstrap ratio intervals over 2,000 resamples and
> every round in `rounds_raw`; the levels below are **run A** of the replication
> pair §7.6 publishes; `results/baseline_concurrent.json`; workloads `hot_rowex_set_63bit`,
> `hot_rowex_map_64bit`)*.
>
> Both arms are measured **below any external lock**, through their native
> concurrent APIs (§8.16; `METHODOLOGY.md` §11.3 decision 4). Expanse's protocol
> is optimistic lock coupling — one writer mutex, validated readers — and is
> blocking by design (`AGENTS.md` §2.2). Ratios are **Expanse ÷ ROWEX
> throughput**, so, as everywhere in this suite, **above 1.000 means Expanse is
> faster.**

### 7.1 Writer throughput as writer count scales — the pre-registered loss, confirmed and wider than registered

W writers each insert their slice of 2²⁰ fresh keys into a 2²⁰ prefill; fixed
work, so both arms grow by exactly the same population every round.

![Writer throughput vs writer count](results/chart_concurrent_writers.svg)

| W | set: ROWEX M/s | set: Expanse M/s | ratio [BCa 95%] | verdict | map: ROWEX M/s | map: Expanse M/s | ratio [BCa 95%] | verdict |
|--:|---:|---:|---|---|---:|---:|---|---|
| 1 | 5.53 | **8.63** | 1.558 [1.546, 1.575] | Expanse | 2.97 | **5.23** | 1.756 [1.703, 1.798] | Expanse |
| 2 | **9.01** | 5.16 | 0.573 [0.567, 0.577] | **ROWEX** | **4.89** | 3.70 | 0.752 [0.732, 0.767] | **ROWEX** |
| 4 | **15.77** | 4.10 | 0.269 [0.264, 0.277] | **ROWEX** | **8.64** | 3.06 | 0.354 [0.348, 0.360] | **ROWEX** |
| 8 | **26.27** | 3.80 | 0.146 [0.144, 0.151] | **ROWEX** | **13.85** | 2.56 | 0.180 [0.172, 0.186] | **ROWEX** |
| 16 | **35.23** | 2.94 | 0.084 [0.083, 0.085] | **ROWEX** · *not pre-registered (SMT)* | **19.45** | 2.52 | 0.130 [0.126, 0.136] | **ROWEX** · *not pre-registered (SMT)* |

- **Expanse wins with one writer** — 1.56× (set) and 1.76× (map) —
  **`CONFIRMED`** (§11.5.2, medium-high). The concurrent wrappers keep the
  single-threaded insertion win of §2, at a smaller margin than the 2.52× /
  3.55× measured without a wrapper *(different workload: `hot_latency` builds a
  cold structure, this arm inserts into a 2²⁰ prefill — not comparable)*.
- **The crossover is at W = 2**, inside the registered W\* ∈ [2, 4] —
  **`CONFIRMED`** (§11.5.1, medium). ROWEX already wins at two writers on both
  arms, with intervals clear of parity.
- **At W ≥ 4 ROWEX wins by 2.8×–11.9×** — **`CONFIRMED`** (§11.5.1, high) and
  wider than the registration argued for. Expanse's *aggregate* writer
  throughput does not merely plateau at its single-writer rate: it **falls** as
  writers are added — set 8.63 → 5.16 → 4.10 → 3.80 → 2.94 M inserts/s
  (0.34× of one writer at sixteen), map 5.23 → 3.70 → 3.06 → 2.56 → 2.52
  (0.48×). ROWEX scales 4.8× on the set arm and 4.7× on the map arm at W = 8, and 6.4× / 6.6× at W = 16,
  where the sixteen threads occupy both SMT siblings of every P-core.

Every insert on the Expanse side takes the same writer mutex, so the aggregate
is bounded by the single-writer rate by construction; that it falls *below*
that rate is the measured part. Which share of the fall is lock hand-off and
which is the writers' cache-line traffic is **unmeasured** here — this arm
carries no hardware counters (§8.9) — and no mechanism beyond the serialization
itself is claimed.

### 7.2 Readers alongside writers

Eight readers probe a 50/50 stream against the prefill while W writers insert;
W = 0 is the reader-only reference. **The reader window is the writers' fixed
work**, so the two arms' windows differ in length by the writer ratio and the
population grows at different rates inside them — a reader column is not a
fixed-duration measurement on both arms, and W = 0 is the only row where the
two windows are the same length.

![Reader throughput alongside writers](results/chart_concurrent_readers.svg)

| W | set: ROWEX M/s | set: Expanse M/s | ratio [BCa 95%] | verdict | map: ROWEX M/s | map: Expanse M/s | ratio [BCa 95%] | verdict |
|--:|---:|---:|---|---|---:|---:|---|---|
| 0 | 125.73 | **150.31** | 1.220 [1.198, 1.264] | Expanse | 72.96 | **126.95** | 1.707 [1.648, 1.744] | Expanse |
| 1 | **109.74** | 15.42 | 0.140 [0.136, 0.142] | **ROWEX** | **57.49** | 15.94 | 0.271 [0.260, 0.281] | **ROWEX** |
| 2 | **104.16** | 25.64 | 0.247 [0.238, 0.253] | **ROWEX** | **52.98** | 24.93 | 0.467 [0.448, 0.488] | **ROWEX** |
| 4 | **93.00** | 19.63 | 0.216 [0.203, 0.228] | **ROWEX** | **45.81** | 22.85 | 0.493 [0.478, 0.507] | **ROWEX** |
| 8 | **72.21** | 22.57 | 0.311 [0.306, 0.316] | **ROWEX** | **30.56** | 20.63 | 0.663 [0.638, 0.679] | **ROWEX** |

- **Reader-only (W = 0):** Expanse wins on both arms. The map row is
  **`CONFIRMED`** (§11.5.2, medium). The set row was registered as
  `BOUNDARY_RESULT` and landed as an Expanse win with the interval clear of
  parity — recorded as a registered no-winner that resolved in Expanse's
  favour, not as a confirmed prediction.
- **Readers under any writer load: ROWEX wins every cell** — **`CONFIRMED`**
  (§11.5.1, medium-high), and the size at W = 1 is the finding. **One writer
  takes Expanse's eight readers from 150.3 to 15.4 M lookups/s on the set arm
  (0.10× of their reader-only rate) while ROWEX's readers keep 109.7 (0.87×)**;
  on the map arm 127.0 → 15.9 (0.13×) against 73.0 → 57.5 (0.79×). Expanse's
  reader throughput then stays roughly flat as writers are added (15 → 26 → 20
  → 23 set; 16 → 25 → 23 → 21 map) while ROWEX's declines as its writers take
  more of the machine, which is why the ratio narrows toward W = 8 without
  Expanse recovering. **The mechanism of the collapse is unmeasured.** The
  restart share cannot account for a ten-fold drop — it sits in a 1–11% band
  across both runs at every writer count (§7.3) — and `sample_spins` ÷
  `read_ops`, about one wait per lookup there (0.58–1.30 over the two runs),
  is the only counter this suite takes that speaks to it. No
  hardware counter was taken on either arm, so nothing here attributes the fall
  to a cache-line transfer, a futex or a bracket wait (§8.9 principle 1);
  #737's shared `perf stat` wrapper is what would take one.
- **Writers with readers present** *(not registered as a separate row;
  reported)*: the Expanse single writer drops from 8.63 to 2.24 M inserts/s
  (set) and 5.23 to 1.95 (map) when eight readers are probing; ROWEX's from
  5.53 to 3.44 and 2.97 to 2.15. Writer ratios in these cells run 0.645
  [0.636, 0.652] at W = 1 down to 0.091 [0.090, 0.092] at W = 8 (set) and
  0.958 [0.877, 1.033] down to 0.140 [0.133, 0.146] (map) — ROWEX wins every
  one except the map arm at W = 1, which claims no winner; without readers
  Expanse won that cell outright.

### 7.3 Protocol health — event ratios from the diagnostic build

The 64-bit protocol has no `Busy` outcome; its counterpart to the 32-bit Busy
rate is the **restart share** (walk attempts that observed a moved version and
restarted) and the **fallback share** (reads that exhausted 64 restarts and took
the writer mutex), read from the engine's `occ_stats` counters on a **separate
`occ-stats` build** of the same cells, Expanse side only (§11.3, decision 5).
Nothing in this table is a timing. 5 rounds per cell; median with range.

> The table below is the output of `scripts/integer_tables.py` over `results/baseline_concurrent.json` (run 1) and `results/baseline_concurrent_run2.json` (run 2), both runs side by side per `docs/BENCHMARKING.md` rule 18; nothing in it is typed by hand, and a column the artifacts do not carry reads `not recorded`.

| Arm | W | R | run | restart share, median [min, max] | fallback share | `sample_spins` ÷ `read_ops` (ratio of medians) | `locked_reads` ÷ `read_ops` | §11.5.3 |
|---|--:|--:|--:|---|---:|---:|---:|---|
| set | 1 | 8 | 1 | 4.17% [4.01%, 4.22%] | 0.0000% | 0.89 | not recorded | rise with W: `CONFIRMED`; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| set | 1 | 8 | 2 | 1.34% [1.18%, 6.09%] | 0.0000% | 0.58 | not recorded | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| set | 2 | 8 | 1 | 4.61% [3.70%, 5.04%] | 0.0000% | 0.86 | not recorded | rise with W: `CONFIRMED`; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| set | 2 | 8 | 2 | 10.60% [5.71%, 11.26%] | 0.0000% | 1.30 | not recorded | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| set | 4 | 8 | 1 | 5.82% [5.62%, 6.38%] | 0.0000% | 1.00 | not recorded | rise with W: `CONFIRMED`; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| set | 4 | 8 | 2 | 5.16% [4.87%, 6.37%] | 0.0000% | 0.93 | not recorded | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| set | 8 | 8 | 1 | 6.30% [6.07%, 6.59%] | 0.0000% | 1.03 | not recorded | rise with W: `CONFIRMED`; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| set | 8 | 8 | 2 | 5.03% [4.95%, 5.20%] | 0.0000% | 0.99 | not recorded | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 1 | 8 | 1 | 3.60% [3.47%, 4.51%] | 0.0000% | 1.01 | not recorded | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 1 | 8 | 2 | 3.61% [3.58%, 3.75%] | 0.0000% | 1.02 | not recorded | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 2 | 8 | 1 | 4.31% [3.65%, 5.87%] | 0.0000% | 1.00 | not recorded | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 2 | 8 | 2 | 4.96% [4.74%, 5.14%] | 0.0000% | 1.03 | not recorded | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 4 | 8 | 1 | 5.29% [4.29%, 5.60%] | 0.0000% | 1.19 | not recorded | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 4 | 8 | 2 | 5.68% [5.31%, 6.03%] | 0.0000% | 1.16 | not recorded | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 8 | 8 | 1 | 5.01% [4.82%, 5.18%] | 0.0000% | 1.18 | not recorded | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 8 | 8 | 2 | 5.21% [5.15%, 5.51%] | 0.0000% | 1.20 | not recorded | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |

- **No reader ever took the writer mutex**: `read_fallbacks` is zero in every
  round of every cell, so the §11.5.3 starvation falsifier (fallback share
  ≥ 1%) did not fire — **`PASS_categorical_by_design`**, not `CONFIRMED`. A
  fallback needs **64 consecutive failed optimistic walks**, and at the bracket
  lengths a single writer holds, the probability of 64 in a row is negligible by
  construction. The zero is a property of the construction, not a measured
  property of the protocol: the falsifier could not have fired at these writer
  counts whatever the engine did, and a falsifier that cannot fire is not a
  measurement (AGENTS.md §8, C-b). METHODOLOGY §11.8 registers one that can.
- **Whether the restart share rises with W is not settled by two runs.**
  On the set arm run 1 rises monotonically (4.17 → 4.61 → 5.82 → 6.30%,
  `CONFIRMED`) and run 2 does not (1.34 → 10.60 → 5.16 → 5.03%, `REFUTED`);
  on the map arm neither run is monotonic. Under `docs/BENCHMARKING.md` rule
  18 a cell whose two runs disagree is reported as direction-only: the share
  sits in a 1–11% band across both runs and every writer count, and the
  §11.5.3 rise hypothesis is neither `CONFIRMED` nor `REFUTED` across runs.
  The table above carries each run's own verdict; the scorecard carries the
  band.
- The counters account for restarts and for spin iterations in
  `SeqVersion::sample` (0.58–1.30 per read op over the two runs); they do not
  time a spin — the `sample_spin_cycles` counter that does is
  pre-registered in `docs/benchmarks/concurrency/METHODOLOGY.md`. The size
  of the §7.2 reader collapse is therefore **not attributed** by this table —
  the cause beyond the bracket wait itself is unmeasured.

### 7.4 Memory — build-only, single writer, curve across λ

Bytes held from the C allocator after a single-writer build (§11.3, decision 1),
ROWEX against the `Sync*` wrapper, swept across the §9.6 occupancy targets.
Deterministic byte counts, no interval. The census was re-validated with TBB
linked (control allocation returns to zero; every ROWEX cell counted at least
2N allocations on the map arm and N on the set arm — see `rowex_allocs` in the
artifact). **Disclosed blind spot:** `libtbb.so`'s own per-thread state is
allocated through the dynamic linker and is invisible to the link-time
interposition; it is paid once per registering thread and is independent of N.

| λ | set: ROWEX B/key | set: `SyncExpanseSet` B/key | winner | map: ROWEX B/key | map: `SyncExpanseMap` B/key | winner |
|--:|---:|---:|---|---:|---:|---|
| 1 | **12.91** | 14.14 | ROWEX 1.10× | 36.71 | **24.26** | Expanse 1.51× |
| 2 | **12.40** | 13.25 | ROWEX 1.07× | 36.31 | **24.71** | Expanse 1.47× |
| 4 | **12.00** | 12.12 | ROWEX 1.01× | 36.21 | **22.96** | Expanse 1.58× |
| 8 | 11.88 | **9.85** | Expanse 1.21× | 36.16 | **19.36** | Expanse 1.87× |
| 15 | 11.76 | **8.10** | Expanse 1.45× | 36.07 | **16.80** | Expanse 2.15× |
| 23 | 11.73 | **8.12** | Expanse 1.44× | 36.09 | **16.36** | Expanse 2.21× |
| 30 | **11.73** | 14.04 | ROWEX 1.20× | 36.05 | **20.35** | Expanse 1.77× |
| 38 | **11.80** | 21.96 | ROWEX 1.86× | 36.15 | **25.92** | Expanse 1.40× |
| 46 | **11.71** | 23.09 | ROWEX 1.97× | 36.09 | **26.78** | Expanse 1.35× |
| 61 | **11.72** | 21.06 | ROWEX 1.80× | 36.05 | **25.62** | Expanse 1.41× |

- **Set arm:** the §1 story repeats with the concurrent types — ROWEX is flat
  (11.71–12.91 B/key), Expanse wins only in the band λ ∈ [8, 23] and loses on
  both sides of it. Both §11.5 memory rows for the set arm are **`CONFIRMED`**.
- **Map arm:** Expanse wins at every occupancy, 1.35×–2.21× —
  **`CONFIRMED`** and labelled **`PASS_categorical_by_design`**: ROWEX carries
  the same heap `std::pair` per entry as the single-threaded map arm.
- The `SyncExpanseSet` cells differ from §1's `ExpanseSet` cells at the same
  λ in both directions (14.14 against 16.17 at λ = 1; 21.06 against 20.29 at
  λ = 61). The two are different types under the same instrument and the cause
  of the gap is unmeasured; they are not set side by side as one quantity.

### 7.5 Scorecard against the pre-registration

20 throughput cells (10 writer, 10 reader, plus 8 writer-under-reader
sub-cells), 8 health cells, 20 memory cells.

| Registered (`METHODOLOGY.md` §11.5) | Outcome |
|---|---|
| ROWEX wins writer throughput at W ≥ 4 (high) | **CONFIRMED**, 2.8×–11.9× across W = 4, 8 and 16 |
| Crossover writer count W\* ∈ [2, 4] (medium) | **CONFIRMED**, W\* = 2 on both arms |
| ROWEX wins reader throughput under W ≥ 1 (medium-high) | **CONFIRMED**, every cell; 7.1× at W = 1 on the set arm |
| Expanse wins writer throughput at W = 1 (medium-high) | **CONFIRMED**, 1.56× / 1.76× |
| Expanse wins reader-only, map arm (medium) | **CONFIRMED**, 1.71× |
| Reader-only, set arm: `BOUNDARY_RESULT` (low-medium) | registered no-winner; **measured Expanse win** 1.220 [1.198, 1.264] |
| Memory, map arm, all λ (high) | **CONFIRMED**, `PASS_categorical_by_design` |
| Memory, set arm: Expanse wins λ ∈ [8, 23], ROWEX outside (medium) | **CONFIRMED** on both sides |
| Health: fallback share < 1% at all W (falsifier) | **`PASS_categorical_by_design`** — zero fallbacks; a fallback needs 64 consecutive failed walks, which cannot occur at these bracket lengths (§7.3) |
| Health: restart share rises monotonically with W | direction-only (rule 18): set run 1 `CONFIRMED`, run 2 `REFUTED`; map both `REFUTED`; a 1–11% band across both runs (§7.3) |
| W = 16 cells | `not pre-registered`; reported: 0.084 (set), 0.130 (map) |
| Writers with readers present | `not pre-registered`; reported: ROWEX wins every cell but the map arm at W = 1, which claims no winner |

No `UNPREDICTED LOSS`: every cell Expanse lost was registered as a loss.

What this arm does not claim is fixed in `METHODOLOGY.md` §11.6: x86-64 at these
two commits under glibc `malloc` only (no tcmalloc claim); insert and point
lookup on uniform random integer keys only — no deletion, contended-key, scan or
string claim; at most 16 threads on 8 physical performance cores with SMT; and no
peer review.

### 7.6 Between-run spread: the C2 cells are a direction and a range, not a level (#735)

This arm has now been run twice on the reference host **at one commit**,
`64f8a3af`, both under the P-core pin, worst busy-CPU delta 5.7 core-equivalents
across each — the concurrent benchmark's own threads and no non-target process. Publishing the pair at a single commit is what the
earlier pair could not do: the runs at `5232af74` and `134a0471` differed by the
engine as well as by the run, and nothing separated the two. Here the binaries
are identical, so the table below is run-to-run spread on this host and nothing
else.

| Arm | W | R | run A | run B | intervals overlap |
|---|--:|--:|---|---|---|
| set | 0 | 8 | 1.183 [1.158, 1.196] | 1.206 [1.184, 1.268] | yes |
| set | 1 | 8 | 0.138 [0.130, 0.141] | 0.145 [0.138, 0.149] | yes |
| set | 2 | 8 | 0.234 [0.220, 0.242] | 0.281 [0.259, 0.298] | **no** |
| set | 4 | 8 | 0.241 [0.236, 0.246] | 0.247 [0.239, 0.255] | yes |
| set | 8 | 8 | 0.307 [0.297, 0.313] | 0.310 [0.298, 0.321] | yes |
| map | 0 | 8 | 1.780 [1.733, 1.802] | 1.847 [1.788, 1.985] | yes |
| map | 1 | 8 | 0.315 [0.301, 0.326] | 0.309 [0.298, 0.321] | yes |
| map | 2 | 8 | 0.453 [0.424, 0.469] | 0.450 [0.440, 0.464] | yes |
| map | 4 | 8 | 0.524 [0.513, 0.538] | 0.515 [0.502, 0.531] | yes |
| map | 8 | 8 | 0.600 [0.576, 0.622] | 0.681 [0.655, 0.695] | **no** |

**2 of the 10 C2 reader cells moved past their own intervals** — set W = 2 from
0.234 to 0.281 and map W = 8 from 0.600 to 0.681 — and 3 of the 10 C1 writer
cells did the same (`map W = 16`, `set W = 4`, `set W = 8`), 5 of 20 in all.
**Every direction and every verdict held in all 20 cells.** The concurrent
memory cells are byte-identical between the runs, which is the control: a
deterministic census taken by the same code on the same host reproduces
exactly, so the wall-clock spread is not the instrument reading differently.

Five of twenty is the rate `docs/BENCHMARKING.md` rule 18 now records across
this repository's suites — 13 of 72 and 24 of 144 on the single-threaded
sweeps, 4 of 20 on the Masstree concurrent arm. It is the instrument's normal
behaviour, not a fault of this pair.

For the cells that moved, the between-run spread exceeds the within-run
interval, so **a single run's level is not a settled figure**: every citation of
a C2 cell outside this suite states a direction and a range. The
`masstree_comparison` arm found the same thing on its own two runs (README §7,
"Between-run spread"), which is why `docs/BENCHMARKING.md` carries the
replication rule rather than leaving each arm to rediscover it: two runs for a
concurrent cell, the claim ceiling is the union of the two intervals, and a cell
whose runs do not overlap is reported as direction-only.

The health counters move the same way. The restart share is a band, not a
trend: set 4.4 → 7.0 → 5.3 → 6.8% in run A against 5.7 → 4.8 → 5.9 → 6.2% in
run B, map 5.0 → 3.4 → 5.7 → 4.3% against 6.0 → 4.9 → 5.3 → 5.7%. The two runs
do not agree on the ordering of the four values, which is why §7.3 reads the
share as a band between 3% and 7% and grades the registered rise **`REFUTED`**.

What the pair does **not** do is explain the spread. Whether it is scheduling,
frequency, memory placement or something else on this host is unmeasured here —
that is #568's counter plan (`perf c2c`, `xsnp_hitm`, futex counts) and #737's
wrapper, not this table.

The levels quoted in §7.1–§7.5 are run A's; run B is in
`results/baseline_concurrent_run2.json` with its own provenance.
---

## 8. Reproducing

Requires an x86-64 host with AVX2 and BMI2.

```bash
git submodule update --init --depth 1 third_party/hot
docs/benchmarks/hot_comparison/run.sh                    # integer arms, full sweep -> results/
docs/benchmarks/hot_comparison/run.sh --quick            # reduced, -> gitignored results/quick/
docs/benchmarks/hot_comparison/run.sh strings            # string arms (#693) -> results/baseline_string_*.json
docs/benchmarks/hot_comparison/run.sh strings --quick    # reduced, -> gitignored results/quick/
python3 docs/benchmarks/hot_comparison/scripts/generate_charts.py   # every chart, from results/
python3 docs/benchmarks/hot_comparison/scripts/string_tables.py     # the §6 tables, from results/
docs/benchmarks/hot_comparison/run.sh            # full sweep -> results/
docs/benchmarks/hot_comparison/run.sh --quick    # reduced, -> gitignored results/quick/

# The concurrent arm (#692, METHODOLOGY.md §11) additionally needs HOT's
# nested TBB submodule; libtbb is built from it into the cargo build dir.
git -C third_party/hot submodule update --init --depth 1 third-party/tbb
docs/benchmarks/hot_comparison/run.sh --only-concurrent          # -> results/baseline_concurrent.json
docs/benchmarks/hot_comparison/run.sh --only-concurrent --quick  # -> results/quick/
```

The runner takes the host-wide benchmark lock, pins to performance cores, drives
every cell in its own process — HOT's node pool is a process-global `static` and
a warm pool undercounts by up to 3.3× (§9.2) — and snapshots load average at
start, between pillars and at the end. The string runner additionally executes
`hot_string_validate` first and records nothing if it fails (§10.8).
