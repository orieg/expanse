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
> ISC; harness commit `ae9e716e` for the integer arms (§1–§4), measured in two runs on the same host with the committed artifacts as run 1 and `results/baseline_latency_run2.json`, `baseline_memory_curve_run2.json` and `baseline_sensitivity_run2.json` as run 2 — load average 0.01 / 0.29 / 0.35 / 0.69 at the start, after the sensitivity set, after the memory sweep and at the end of run 1, 1.02 / 1.02 / 1.02 / 1.00 for run 2, and at most 0.05 core-equivalents of busy CPU outside the benchmark between any two snapshots of either run; **the string arms (§6, and §4.1's string insertion-order rows) were re-measured twice at `b868fb2e`** with `run.sh strings`, after [#941](https://github.com/orieg/expanse/issues/941) moved string-key NUL validation out of the timed loops — load average 1.21 / 1.21 / 1.09 / 1.06 / 1.00 at the start, after the gate, after the insertion-order set, after the memory sweep and at the end of run 1, 1.00 at every snapshot of run 2, and at most 0.01 core-equivalents of busy CPU outside the benchmark between any two snapshots of either run; the string tables are run 1, with run 2 quoted beside them; `docs/benchmarks/hot_comparison/run.sh`; benchmark
> shell pinned to CPUs 0-15; both arms built for one ISA target —
> `-C target-cpu=haswell` and `-march=haswell -O3 -std=c++17 -DNDEBUG`; 15 rounds per cell, the arm timed first
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

The reason is visible in the result. **Arm A's winner changes twice**: HOT below λ ≈ 8, Expanse from λ ≈ 8 through 23, and HOT again from λ ≈ 30. Arm B's never changes.

![Memory across expanse occupancy](results/chart_memory_curve.svg)

HOT's two lines are flat; Expanse's dip into the shaded band and climb out of it
past the cascade is the whole finding. The shaded region is derived by comparing
the two arms cell by cell, not drawn by eye.

| λ | N | HOT B/key | `ExpanseSet` B/key | winner |
|---:|---:|---:|---:|---|
| 1 | 32,768 | 12.06 | 17.05 | HOT 1.41× |
| 2 | 65,536 | 11.90 | 14.84 | HOT 1.25× |
| 4 | 131,072 | 11.82 | 12.45 | HOT 1.05× |
| 8 | 262,144 | 11.77 | **9.97** | Expanse 1.18× |
| 15 | 491,520 | 11.71 | **8.50** | Expanse 1.38× |
| 23 | 753,664 | 11.69 | **8.52** | Expanse 1.37× |
| 30 | 983,040 | 11.70 | 13.58 | HOT 1.16× |
| 38 | 1,245,184 | 11.77 | 20.69 | HOT 1.76× |
| 46 | 1,507,328 | 11.68 | 21.91 | HOT 1.88× |
| 61 | 1,998,848 | 11.70 | 20.28 | HOT 1.73× |

*(measured: reference host, `ae9e716e`; deterministic allocator census, identical in both runs;
[`results/baseline_memory_curve.json`](results/baseline_memory_curve.json); workload: `hot_memory_curve`)*

> **Correction: the Expanse column is re-measured.** Against the superseded `0f4fd40c` census it
> moved at λ ≈ 1, 15, 23 and 30 — 16.17 → 17.05, 8.27 → 8.50 (the table printed that cell as
> 8.26), 8.11 → 8.52 and 13.37 → 13.58 B/key — and by less than 0.05 B/key at every other λ.
> HOT's column is unchanged to the digit, and no cell changed winner. Which engine change between
> the two commits moved the Expanse side is not isolated here; the cause is unmeasured.

**Expanse wins only in the band λ ∈ [8, 23].** Outside it, HOT wins — below,
because Expanse has not yet amortized its branch structure; above, because the
`LEAF_CAP = 32` overflow cascade has fired and each key costs its own 16-byte
edge (§9.4).

A single cell would have been true and misleading in either direction: at λ=15
this suite could have published *"Expanse uses 1.38× less memory than HOT"*, and
at λ=46 *"HOT uses 1.88× less memory than Expanse"*. Both are measurements of
the same two systems on the same instrument.

**HOT is flat.** 11.68–12.06 B/key across the entire swept range — a 3% spread
against Expanse's 2.6×. Holding fanout roughly constant by varying discriminative
bits per node is exactly the property its authors claim for it, and on this
instrument it delivers.

### The Expanse curve beyond this table — engine instrument, not the census

The table above is the suite's allocator instrument. The engine's own
deterministic accounting (`mem_used()`, host-independent, no wall clock) covers
a wider λ range and is what locates the teeth; the two instruments are not the
same quantity and are never mixed in one table (§9.3, §9.10.6). Set and map
flavors, uniform random keys, same PRNG and seed *(measured: deterministic
byte accounting; workload: `example_keyspace_density`;
`docs/assets/data/bench_assets.json` → `density_sweep`, commit 66a355f9; the full
tables and node census in `METHODOLOGY.md` §9.10 were taken at 86daaddf, before
the capacity-class ladder of #826)*:

| λ | cell | `ExpanseSet` B/key | `ExpanseMap<u64,u64>` B/key | where on the curve |
|---:|---|---:|---:|---|
| 15.26 | 1M @64 | 8.21 | 17.58 | the `memory-budget` cell |
| 19.84 | 1.3M @64 | **7.96** | 17.26 | first trough |
| 27.47 | 1.8M @64 | 10.72 | 18.23 | first knee |
| 30.52 | 2M @64 | 13.74 | 19.78 | 35.05% of expanses cascaded (census) |
| 48.83 | 800k @62 | 21.02 | 23.90 | first peak |
| 1,953 | 2M @58 | 8.81 | 18.52 | every level-6 expanse a `BranchU` |
| 4,688 | 1.2M @56 | **7.08** | 16.47 | second trough |
| 7,812 | 2M @56 | 13.11 | 19.15 | second tooth, 34.9% of sub-expanses cascaded |
| 10,547 | 2.7M @56 | 20.99 | 23.78 | second peak |

The curve repeats one byte level down at λ ≈ 256 × `LEAF_CAP`, so the memory
verdict of §1 — Expanse wins in a band and loses outside it — is a verdict
per tooth, not a verdict on "high λ". The census, seed sensitivity, the
`LEAF_CAP = 48` control with its read-path measurement, and the reconciliation
of this suite's λ = 15 cell as measured at `0f4fd40c` with the 12.62 B/key of §9.3 are in
`METHODOLOGY.md` §9.10.

### Arm B — the value model decides it

| λ | N | HOT B/key | `ExpanseMap` B/key | Expanse advantage |
|---:|---:|---:|---:|---:|
| 1 | 65,536 | 35.88 | 24.86 | 1.44× |
| 8 | 524,288 | 35.74 | 19.02 | 1.88× |
| 23 | 1,507,328 | 35.68 | 17.57 | **2.03×** |
| 46 | 3,014,656 | 35.67 | 24.72 | 1.44× |
| 61 | 3,997,696 | 35.69 | 23.83 | 1.50× |

> **Correction: re-measured at `ae9e716e`.** The superseded `0f4fd40c` map column read 23.87 at
> λ ≈ 1 and 16.26 at λ ≈ 23, where it now reads 24.86 and 17.57 B/key; the largest advantage is
> 2.03× at λ ≈ 23, not the 2.19× first published. At λ ≈ 15 and 30, off this table, the column
> moved 16.72 → 17.84 and 19.39 → 19.98. HOT's column is unchanged; the cause of the Expanse move
> is unmeasured *(measured: reference host, `ae9e716e`; workload: `hot_memory_curve`)*.

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

Two cells at this population go to HOT, and both sit near the parity line:
`lookup_hit · map · random` at 0.946 [0.934, 0.960] and `lookup_miss · set · random`
at 0.983 [0.976, 0.991]. At 10⁵ the map hit cell is the third non-scan HOT win,
0.928 [0.874, 0.954]. The second run puts the three at 0.954 [0.941, 0.968],
0.980 [0.971, 0.988] and 0.928 [0.890, 0.945], so each is a HOT win in both
*(measured: reference host, `ae9e716e` twice; `results/baseline_latency.json` is
run 1; workload: `hot_latency`)*.

### Point lookup, 100% hit

| Distribution | Arm | HOT ns | Expanse ns | Ratio | Verdict |
|---|---|---:|---:|---:|---|
| sequential | set | 19.22 | **4.18** | 4.675 | Expanse |
| clustered | set | 24.81 | **7.74** | 3.221 | Expanse |
| sparse | set | 22.50 | **10.18** | 2.410 | Expanse |
| random | set | 36.55 | **35.28** | 1.038 | Expanse |
| sequential | map | 43.57 | **14.12** | 3.380 | Expanse |
| clustered | map | 49.85 | **23.27** | 2.154 | Expanse |
| sparse | map | 44.36 | **10.26** | 4.801 | Expanse |
| **random** | **map** | **60.12** | 64.13 | **0.946** | **HOT** |

> **Correction: `lookup_hit · map · random` at 10⁶ is a HOT win, not a `BOUNDARY_RESULT`.**
> It was published at 0.986 [0.970, 1.003] from the `ae0c610d` artifact, and two earlier runs
> of the alternating harness put it at 0.993 [0.977, 1.009] and 0.992 [0.977, 1.007]; those
> figures are superseded. Both runs at `ae9e716e` exclude parity on HOT's side, 0.946
> [0.934, 0.960] and 0.954 [0.941, 0.968]. Between the superseded artifact and the two runs
> Expanse's median moved 61.34 → 64.13 and 63.85 ns and HOT's 59.87 → 60.12 and 60.27 ns;
> which engine change moved the Expanse side is not isolated, and the cause is unmeasured
> *(workload: `hot_latency`)*.

### Point lookup, 50% hit / 50% rejection-sampled miss

> **Re-measured with the corrected probe builder ([#760](https://github.com/orieg/expanse/issues/760)).**
> The superseded cells drew the hit half of the stream from
> `population[..hits_wanted]`; the sort above confines that to one end of the
> keyspace while the misses span all of it. The builder now strides hits across
> the whole population; this section was re-measured at `ae0c610d`, and again with §1–§4 at `ae9e716e`.
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
| sequential | set | 19.28 | **8.13** | 2.380 | Expanse |
| clustered | set | 24.79 | **11.24** | 2.212 | Expanse |
| sparse | set | 22.46 | **9.59** | 2.525 | Expanse |
| **random** | **set** | **36.49** | 37.09 | **0.983** | **HOT** |
| sequential | map | 55.74 | **13.17** | 4.710 | Expanse |
| clustered | map | 62.70 | **18.55** | 3.430 | Expanse |
| sparse | map | 56.11 | **10.04** | 6.231 | Expanse |
| random | map | 76.76 | **64.79** | 1.200 | Expanse |

**The pre-registered uniform-random loss is confirmed on one path of each arm —
Arm A's miss path and Arm B's hit path — and refuted on the other.** §5.1 registered HOT winning uniform-random point
lookup at medium-high confidence, reasoning that random keys discriminate late
and force a deep descent at a fixed 8-bit span while HOT's variable bit selection
bounds height. On the set arm the miss path is a HOT win (0.983 [0.976, 0.991])
and the hit path an Expanse win (1.038 [1.028, 1.048]). On the map arm the hit
path is a HOT win (0.946 [0.934, 0.960]) and the miss path goes to Expanse
(1.200 [1.184, 1.218]). The second run agrees on all four directions — 0.980,
1.032, 0.954 and 1.201. Why the map arm's HOT win sits on the hit path and not
the miss path is unmeasured; no counter in this suite attributes it.

These four cells are the ones §12.1's arm alternation moved most. At `5232af74`,
with HOT timed first in every round and Expanse inheriting its warmed cache, the
map hit cell read 1.399 and the set hit cell 0.998. Alternating the arm timed
first reversed the map hit cell's direction, which is the largest single
consequence of the harness change *(measured: reference host, `5232af74` →
`134a0471` → `0f4fd40c`)*. The `BOUNDARY_RESULT` it then carried is superseded by
the correction under the 100% hit table.

### Insertion into a cold structure

| Distribution | Arm | HOT ns | Expanse ns | Ratio |
|---|---|---:|---:|---:|
| sequential | set | 58.74 | **4.83** | 12.146 |
| clustered | set | 62.53 | **12.01** | 5.208 |
| sparse | set | 57.85 | **27.93** | 2.070 |
| random | set | 78.24 | **29.50** | 2.647 |
| sequential | map | 73.20 | **11.98** | 6.103 |
| clustered | map | 77.21 | **20.53** | 3.759 |
| sparse | map | 72.36 | **29.65** | 2.438 |
| random | map | 94.53 | **26.89** | 3.515 |

Expanse wins all 24 insertion cells across the three populations in both runs —
1.735 [1.706, 1.758] to 12.146 [12.084, 12.185] in run 1, 1.639 [1.310, 2.034] to
12.205 [12.181, 12.271] in run 2 (`results/baseline_latency.json` is run 1 and `results/baseline_latency_run2.json` run 2). §5.2 registered this as a *weak*
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

**28 of this suite's 31 HOT wins are scan cells.** §5.1 registered HOT winning
short range scans at k=10 and k=100, carried forward from the loss
`art_comparison/` found unpredicted. The measurement is broader than that on two
axes, and both are recorded as **`UNPREDICTED LOSS`**:

- **k=1000 loses too**, which was not registered — `set`/`random`/100k is
  0.520 [0.517, 0.524] and 0.508 [0.506, 0.510] in the two runs, and
  `map`/`random`/100k is 0.392 [0.389, 0.396] and 0.395 [0.391, 0.397]. The set
  cell's two intervals do not overlap, so its level is a range across runs, not
  a figure (`docs/BENCHMARKING.md` rule 18).
- **`sparse` loses as well as `random`**, also not registered.

| Arm | Dist | N | k=10 | k=100 | k=1000 |
|---|---|---:|---:|---:|---:|
| set | random | 10,000 | 0.518 | 0.424 | 0.419 |
| set | random | 100,000 | 0.741 | 0.556 | 0.520 |
| set | random | 1,000,000 | 0.851 | 0.745 | 0.722 |
| map | random | 10,000 | 0.616 | 0.507 | 0.472 |
| map | random | 100,000 | 0.699 | 0.441 | 0.392 |
| map | random | 1,000,000 | **1.702** | **1.631** | **1.525** |

The `map`/`random`/1M row is the exception and it reverses cleanly: Expanse wins
every scan width there in both runs, 1.702 / 1.631 / 1.525 and 1.712 / 1.632 /
1.518 (`results/baseline_latency.json` is run 1 and `results/baseline_latency_run2.json` run 2). At `5232af74`, with HOT timed
first in every round, the row read 1.414 / 1.402 / 1.517, with no interval
published beside it. Scan outcome therefore depends on population as well as
on `k`, which is a second reason this suite does not publish single-population
cells.

![Ordered range scan](results/chart_scan.svg)

Scan on `sequential` and `clustered` is an Expanse win throughout and does not
appear in the loss list.

> **Correction: `map`/`sparse`/1M at k = 1000 is an Expanse win, not a `BOUNDARY_RESULT`.**
> It was published from the `ae0c610d` artifact at 1.003 [0.984, 1.027], which is superseded;
> both runs at `ae9e716e` exclude parity, 1.026 [1.009, 1.054] and 1.030 [1.012, 1.059]. The
> other 13 `sparse` scan cells that go to HOT are HOT wins in both runs, so the `sparse` bullet
> above stands. The cause of the move is unmeasured *(workload: `hot_latency`)*.

---

## 4. Scorecard

144 latency cells, 20 memory cells.

| | Count |
|---|---:|
| Expanse wins (CI excludes parity) | 112 |
| HOT wins (CI excludes parity) | 31 |
| `BOUNDARY_RESULT` (interval spans parity) | 1 |

Both runs at `ae9e716e` give this scorecard, and no cell changes class between
them; 17 of the 144 cells have run-1 and run-2 intervals that do not overlap,
none of them with a winner change. **Correction:** the superseded `ae0c610d`
scorecard was 111 / 30 / 3. The two cells that changed class are the corrections
in §2 and §3, and both runs confirm each *(measured: reference host, `ae9e716e`
twice; workload: `hot_latency`)*.

Against the pre-registration:

| Registered | Outcome |
|---|---|
| HOT wins uniform-random point lookup (§5.1, medium-high) | **CONFIRMED** on Arm A's miss path (0.983) and on Arm B's hit path at 10⁶ (0.946) and 10⁵ (0.928); **REFUTED** on Arm A's hit path (1.038) and on Arm B's miss path at 10⁶ (1.200) |
| HOT wins short range scans k=10, k=100 (§5.1, medium-high) | **CONFIRMED**, and wider — see §3 |
| HOT wins sparse-stride memory (§5.1, downgraded to low in §9.5) | **CONFIRMED** as part of the λ story: HOT wins above the cascade |
| Expanse wins Arm B memory (§5.2, high) | **CONFIRMED**, labelled `PASS_categorical_by_design` |
| Expanse wins insertion (§5.2, weak) | **CONFIRMED**, stronger than registered |
| Expanse wins sequential and sparse point lookup (§5.2, medium) | **CONFIRMED** |
| Scan losing at k=1000 and on `sparse` | **UNPREDICTED LOSS** |
| Arm A memory winner changing twice across λ | **not pre-registered** |

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
| set | `sorted` | 11.70 | 14.13 | **13.74** | 1.033 [1.022, 1.043] | 2.645 [2.640, 2.650] |
| set | `shuffled` | 12.06 | 20.62 | **13.74** | 1.036 [1.025, 1.048] | 1.982 [1.976, 1.988] |
| map | `sorted` | 35.71 | 17.84 | **17.58** | 0.948 [0.935, 0.963] | 3.536 [3.528, 3.545] |
| map | `shuffled` | 36.22 | 24.75 | **17.58** | 1.012 [0.999, 1.028] | 2.770 [2.748, 2.792] |

*(measured: reference host — Intel i9-12900F, CPUs 0-15; commit `ae9e716e`, two runs;
`run.sh`; 15 rounds per cell, BCa 95%;
[`results/baseline_sensitivity.json`](results/baseline_sensitivity.json) is run 1 and [`results/baseline_sensitivity_run2.json`](results/baseline_sensitivity_run2.json) run 2;
workload: `hot_set_63bit` and `hot_map_64bit`)*. Run 2 reproduces the memory
columns exactly and agrees on the direction of seven of the eight latency cells.
The eighth, `map` · `shuffled` · `lookup_hit`, spans parity in run 1 at
1.012 [0.999, 1.028] and excludes it in run 2 at 1.016 [1.002, 1.030]: **its
winner is not established** (`docs/BENCHMARKING.md` rule 18), and it claims none.

> **Correction:** these rows replace the superseded `0f4fd40c` integer rows, which read
> 13.95 / 20.33 / 13.60 B/key on the set and 16.67 / 23.62 / 16.70 on the map. The
> `map` · `sorted` · `lookup_hit` cell was published there as a `BOUNDARY_RESULT` at
> 0.990 [0.977, 1.005]; both runs at `ae9e716e` put it on HOT's side, 0.948 [0.935, 0.963]
> and 0.954 [0.941, 0.968], the same direction as §2's sorted-order cell. The cause of the
> move is unmeasured.

**String arms, `short`, N = 1,000,000**

| Arm | Order | HOT alloc B/key | Expanse alloc B/key | Expanse `mem_used` B/key | `lookup_hit` HOT ÷ Expanse | `insert` HOT ÷ Expanse |
|---|---|---:|---:|---:|---:|---:|
| C · str | `sorted` | 12.23 | 48.19 | **43.25** | 1.228 [1.225, 1.232] | 1.331 [1.313, 1.349] |
| C · str | `shuffled` | 12.72 | 50.95 | **43.25** | 1.242 [1.237, 1.246] | 1.258 [1.250, 1.266] |
| D · map | `sorted` | 36.29 | 48.19 | **43.25** | 1.856 [1.851, 1.860] | 1.494 [1.478, 1.510] |
| D · map | `shuffled` | 36.83 | 50.93 | **43.25** | 1.878 [1.872, 1.885] | 1.720 [1.716, 1.724] |

*(measured: reference host — Intel i9-12900F, 8P+8E / 24 threads, P-core pin;
commit `b868fb2e`, two runs; `run.sh strings`; 15 rounds per cell, BCa 95%;
busy CPU 1.00 core-equivalents across the insertion-order phase of each run,
none of it outside the benchmark;
[`results/baseline_string_sensitivity.json`](results/baseline_string_sensitivity.json)
is run 1 and [`results/baseline_string_sensitivity_run2.json`](results/baseline_string_sensitivity_run2.json) run 2; workloads: `hot_str_ptr`, `hot_str_map`)*. The census columns are
identical in run 2, and its ratios, row by row, are 1.227 [1.222, 1.231] /
1.333 [1.316, 1.349], 1.228 [1.200, 1.245] / 1.252 [1.241, 1.260],
1.876 [1.869, 1.882] / 1.492 [1.475, 1.509] and 1.878 [1.872, 1.884] /
1.715 [1.709, 1.722], every winner the same.

> **Correction: these rows are re-measured.** They were published from
> `64f8a3af` at 47.83 / 50.61 / 50.60 B/key allocator and 42.77 `mem_used`, with
> `lookup_hit` / `insert` ratios 1.268 / 1.396, 1.283 / 1.287, 1.912 / 1.558 and
> 1.911 / 1.773. The Expanse census moved with §6.1's and HOT's columns are
> unchanged; no cell changed winner, and the cause of the census move is
> unmeasured.

These replace the four rows withheld under
[#772](https://github.com/orieg/expanse/issues/772), which were measured at
`0f4fd40c` against the two-allocation `ExpanseStrMap` leaf that
[#723](https://github.com/orieg/expanse/issues/723) replaced. The superseded
Expanse columns (69.16 / 71.99 index, 50.77 `mem_used`) stay registered in
`.github/superseded-figures.json`; after the leaf change they read 48.19 / 50.95
and 43.25 at `b868fb2e`.

**`mem_used` is identical in both orders and the allocator figure is not** —
43.25 either way against 48.19 sorted and 50.95 shuffled (50.93 on Arm D). That is the same
split the integer rows show, and the reason the two instruments are published
side by side rather than one standing for the other: a digital trie's node
census is fixed by the key set, and only its allocator footprint depends on
arrival order.
- **`mem_used` is identical in both orders on every arm** — 17.58 B/key
  for the integer map and 43.25 for the string arms — while the allocator
  census moves on both arms. A digital trie's shape is fixed by the key set, not by the sequence
  the keys arrived in; the allocator's is not. That is the invariant
  `crates/expanse/tests/test_mem_used_order_invariant.rs` pins, and it is what
  makes the two columns readable side by side: the difference between them is
  attributable to the allocator, not to the trie.
- **Expanse's own insert cost more than doubles on a shuffled population** —
  26.83 → 65.88 ns on the integer map in run 1 and 26.95 → 65.40 in run 2 — and
  its allocator footprint moves 17.84 → 24.75 B/key in both. The
  `masstree_comparison` sensitivity set measured `ExpanseMap` at 16.67 → 23.63
  B/key on the same shape and population *(measured: reference host, `2ce92b7f`)*,
  and this suite read 16.67 → 23.62 at `0f4fd40c`. The two suites no longer
  measure one engine commit, so the figures are not comparable as a replication,
  and which engine change moved this suite's Expanse column is unmeasured.
- **HOT moves too**, so the insert ratio narrows rather than reverses:
  3.536 [3.528, 3.545] → 2.770 [2.748, 2.792] on the integer map (3.505 → 2.801
  in run 2). Expanse still wins every insert cell in
  both orders here, unlike the Masstree arm, whose insert ratio flips from 0.760
  to 1.883 across the same pair.
- **`lookup_hit` moves far less than `insert`** — 1.033 → 1.036 on
  the set, 0.948 → 1.012 on the map, whose shuffled cell's winner is not
  established (above). The order affects the build, not the probe stream, which
  is shuffled in both cases.
- The mechanism is **unmeasured**. Nothing here attributes the shuffled-order
  cost to page faults, allocator span reuse or node-shape churn; #725's counter
  plan and #737's wrapper are what would.

---

## 5. What this suite does not claim

Stated before the numbers existed (`METHODOLOGY.md` §7), as amended by §10.9
for the string arms and §11.6 for the concurrent arm:

1. **§1–§4 and §6 are single-threaded.** No concurrency claim follows from
   them. The concurrent arm is §7, measured against HOT's ROWEX variant, and it
   carries its own, narrower ceiling (`METHODOLOGY.md` §11.6).
2. **Integer keys in §1–§4.** Arm A is restricted to a 63-bit domain because
   HOT's inline value payload is 63 bits wide (§9.4), and is labelled
   `hot_set_63bit` throughout. String keys are §6, under their own ceiling
   (`METHODOLOGY.md` §10.9): claims attach to HOT's C-string configuration only,
   never to keys longer than 254 bytes, and an `ExpanseBytesMap` cell is a
   hash-indexed structure against a trie, not a trie comparison.
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

> **Measured twice at `b868fb2e`, after key validation left the timed loops.**
> Since [#810](https://github.com/orieg/expanse/issues/810) the string maps take
> a NUL-free key type, and the harness had adapted by validating each key inside
> the timed lookup, insert and scan loops while HOT received the pointer
> unchecked. [#941](https://github.com/orieg/expanse/issues/941) validates every
> key once, at construction, and this section was re-measured on a head that
> includes it. The `7fe02c0b` artifacts it replaces predate #810, so neither
> measurement carries the per-call validation. Both runs, and the `7fe02c0b`
> artifact, agree on the winner of every N = 1M cell and every scan cell; five
> 100%-hit lookup cells at N = 10,000 and 100,000 do not, and §6.6 names them.
> The probe builder carries the [#760](https://github.com/orieg/expanse/issues/760)
> hit-sampling fix in all three.

> *(measured: reference host — Intel Core i9-12900F, 8P+8E/24 threads, 30 MiB L3,
> Ubuntu 22.04 / kernel 6.8; HOT `96bf6fb`; harness commit `b868fb2e`, two runs;
> `docs/benchmarks/hot_comparison/run.sh strings`; benchmark shell pinned to CPUs
> 0-15; both arms `-C target-cpu=haswell` / `-march=haswell -O3 -std=c++17
> -DNDEBUG`; load average 1.21 / 1.21 / 1.09 / 1.06 / 1.00 at start, after the
> gate, after the insertion-order set, after the memory sweep and at the end of
> run 1, and 1.00 at every snapshot of run 2; busy CPU 0.99–1.00 core-equivalents
> between snapshots, of which at most 0.01 outside the benchmark; 15 rounds per
> cell, the arm timed first alternating per round (§12.1), median reported,
> BCa 95% bootstrap ratio intervals over 2,000 resamples;
> `results/baseline_string_latency.json`, `results/baseline_string_memory.json`
> and gate transcript `results/string_validate.log` are run 1)*. Pre-registration:
> [`METHODOLOGY.md` §10](METHODOLOGY.md#10-string-key-arms-693-pre-registration).
> Tables in this section are curated from `scripts/string_tables.py` over those
> files; where run 2 is quoted, it follows run 1. Internal work; no external peer
> review.

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
with a HOT column go to HOT, in both runs; HOT ÷ Expanse runs from
0.646 [0.643, 0.650] (`counter`, Arm D, k = 10, N = 1M) down to
0.050 [0.049, 0.050] (`skewed`, Arm C, k = 1000, N = 100k), and run 2 reads
0.646 [0.643, 0.649] and 0.049 [0.049, 0.050] on the same two cells
*(workloads: `hot_str_ptr`, `hot_str_map`)*. That is the pre-registered
high-confidence loss (§10.7), **`CONFIRMED`** on all 72.

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
| C · ptr | `counter` | 1,000,000 | 0.508 [0.502, 0.513] | 0.136 [0.132, 0.139] | 0.079 [0.078, 0.080] |
| C · ptr | `prefixed` | 1,000,000 | 0.464 [0.459, 0.469] | 0.178 [0.172, 0.184] | 0.103 [0.100, 0.106] |
| C · ptr | `short` | 1,000,000 | 0.454 [0.448, 0.460] | 0.145 [0.139, 0.151] | 0.085 [0.082, 0.087] |
| C · ptr | `skewed` | 998,150 | 0.382 [0.378, 0.387] | 0.111 [0.107, 0.115] | 0.064 [0.062, 0.066] |
| D · map | `counter` | 1,000,000 | 0.646 [0.643, 0.650] | 0.177 [0.175, 0.179] | 0.093 [0.092, 0.094] |
| D · map | `prefixed` | 1,000,000 | 0.581 [0.577, 0.584] | 0.250 [0.246, 0.253] | 0.135 [0.133, 0.137] |
| D · map | `short` | 1,000,000 | 0.598 [0.594, 0.601] | 0.204 [0.201, 0.206] | 0.117 [0.115, 0.118] |
| D · map | `skewed` | 998,150 | 0.496 [0.493, 0.499] | 0.159 [0.157, 0.161] | 0.086 [0.085, 0.088] |
| C, D | `beyond` | any | Expanse 27–108 ns/element in both runs; HOT column withheld (§10.4) | | |

Run 1; run 2 is within 0.01 of every N = 1M cell above. The 10k and 100k rows
are in `results/baseline_string_latency.json`: at k = 10 they run 0.245–0.551
(0.245–0.545 in run 2), and at k = 100 and k = 1000 none is above 0.18 in either
run. **Correction:** this sentence previously read that none of the 10k and 100k
rows was above 0.22; that holds only at k = 100 and k = 1000, and the k = 10
cells were above it in the `7fe02c0b` artifact as well.

**Memory ownership on `short` still goes to HOT; `skewed` goes narrowly to
Expanse.** §10.7 registered, at low-medium confidence, that Expanse would win the
`ownership` column on `short` and `counter`. On `counter` it does; on `short` it
does not — 36.23 against 48.19 — and on `skewed` Expanse holds 41.71 against
HOT's 41.98 *(workload: `hot_str_ptr`)*:

| Shape (Arm C, N = 1M) | external (exact) | HOT index | Expanse index | **HOT ownership** | **Expanse ownership** | Expanse `mem_used` |
|---|---:|---:|---:|---:|---:|---:|
| `short` | 24.00 (13.00) | 12.23 | 48.19 | **36.23** | 48.19 | 43.25 |
| `skewed` | 29.76 (15.27) | 12.22 | 41.71 | 41.98 | **41.71** | 38.18 |
| `counter` | 24.00 (13.00) | 11.42 | 20.54 | 35.42 | **20.54** | 19.56 |
| `prefixed` | 136.00 (121.00) | 12.23 | 64.01 | 148.23 | **64.01** | 55.25 |
| `beyond` | 280.00 (273.00) | withheld | 48.19 | withheld | 48.19 | 47.25 |

*(Deterministic allocator census; the `ExpanseStrMap` arms are identical in both
runs.)*

> **Correction: the Expanse columns are re-measured.** Against the `7fe02c0b`
> census, the allocator column moved 47.83 → 48.19 B/key on `short` and
> `beyond`, 41.28 → 41.71 on `skewed`, 63.49 → 64.01 on `prefixed` and
> 20.53 → 20.54 on `counter`; `mem_used` moved 42.77 → 43.25 on `short`,
> 37.71 → 38.18 on `skewed`, 54.77 → 55.25 on `prefixed` and 46.78 → 47.25 on
> `beyond` (the superseded figure first in each pair), and held at 19.56 on
> `counter`. Allocations for 10⁶ keys fell
> 1,065,010 → 1,049,567 on `short` and 160,627 → 141,423 on `counter`. HOT's
> columns are unchanged to the digit, and no cell changed winner. Which engine
> change between the two commits moved the Expanse side is not isolated here;
> the cause is unmeasured.

`ExpanseStrMap` holds a 12-byte key in about 48 bytes, and the gate's
allocation counts show where they go: **111,244** allocations for 100,000
`short` keys against HOT's 4,566 (`results/string_validate.log`, identical in
both runs; the superseded `7fe02c0b` transcript read 115,916), or roughly one per key
against HOT's one per twenty-two. The engine's own `mem_used` (43.25 B/key)
sits 4.9 B/key under the allocator column, which is the allocator's rounding on
those allocations (§9.3 reason 1, now on the string path). The §10.7 prediction
is still **`REFUTED`** on `short`, and it remains a finding about
`ExpanseStrMap`'s leaf representation rather than about HOT.

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
> 12.0 — and `mem_used`'s shortfall against the allocator column was 5.1 B/key
> after that change (4.9 at `b868fb2e`), where the two-allocation leaf made it
> 18. The superseded figures are registered in `.github/superseded-figures.json`.

**The `index` column goes to HOT on every shape, categorically.** HOT holds
11.4–12.2 B/key of index on every representable shape at N = 1M because it
stores an 8-byte pointer plus node bits and nothing else; Expanse holds the key
bytes. Registered in §10.3 as **`PASS_categorical_by_design` in HOT's favour**
and labelled so: the contest is the `ownership` column above, where HOT's
external string table is added back.

**`prefixed` point lookup on Arm C is HOT's at N = 1M and N = 10,000, as
registered, and not at N = 100,000.** At N = 1M, 100% hit 0.797 [0.790, 0.800]
and 50/50 0.930 [0.928, 0.933]; run 2 0.798 [0.795, 0.800] and
0.933 [0.930, 0.937] *(workload: `hot_str_ptr`)*. This is the regime HOT is
designed for — 96 shared bytes that its discriminative-bit selection skips and
`ExpanseStrMap` descends one chunk at a time — and it is the one place in the
string suite where the pre-registered loss on HOT's home ground landed as
predicted. At N = 100,000 the 50/50 cell spans parity, 1.026 [0.989, 1.067]
(run 2 1.032 [0.995, 1.074]), and the 100%-hit cell goes to Expanse in both
runs, 1.063 [1.024, 1.108] and 1.049 [1.010, 1.095] — **`REFUTED`** at that
population. **Correction:** the `7fe02c0b` artifact had that 100%-hit cell at
parity, 1.018 [0.982, 1.058]; both runs here put it on Expanse's side, and the
cause of the move is unmeasured.

**`ExpanseBytesMap` (Arm E) loses insertion at every population and most
100%-hit lookups at N = 1M, and its index is the heaviest thing measured here.**
Insert at N = 1M: 0.515 [0.514, 0.517] on `counter`, 0.639 [0.633, 0.660] on
`short`, 0.656 [0.652, 0.662] on `skewed`, 0.742 [0.741, 0.743] on `prefixed`
(run 2 0.520 [0.518, 0.521], 0.637 [0.632, 0.652], 0.656 [0.651, 0.669],
0.742 [0.740, 0.749]). 100% hit: HOT on `prefixed` 0.933 [0.928, 0.948],
`short` 0.937 [0.933, 0.940], `skewed` 0.989 [0.986, 0.993]; Expanse only on
`counter` 1.020 [1.017, 1.024] (run 2 0.930 [0.926, 0.934],
0.937 [0.934, 0.939], 0.986 [0.984, 0.989] and 1.017 [1.014, 1.020], the same
winners) *(workload: `hot_bytes_ptr`)*. Its index costs 97.7–103.2 B/key on
12–15-byte keys and 193.7–193.8 B/key on `prefixed` at N = 1M across the two
runs — a hash-trie entry, a boxed collision bucket, the bucket's vector, and a
boxed copy of the key. None of this was pre-registered (§10.7 declined to
predict Arm E); it is reported as `not pre-registered` and it is the largest
per-entry footprint in either HOT suite. Arm E does win every 50/50 cell at
N = 1M (1.117–1.215 across the two runs).

**Arm E's census is the one memory figure in this section that differs between
runs.** All 60 of its cells differ between run 1 and run 2 — by at most 2.90% in
any cell, at small N, and by at most 0.08 B/key at N = 1M (`short` 97.74 against
97.66) — while every `ExpanseStrMap` and HOT cell is identical in both. Its
figures are therefore given as the range the two runs span; the cause of the
difference is unmeasured. Against the `7fe02c0b` artifact its N = 1M index moved
96.61 → 97.74 B/key on `short` and 96.61 → 97.77 on `counter` (run 1), and its
allocations for 10⁶ keys fell from about 3,105,900 to about 3,097,300; the
superseded range, 96.6–102.1 and 192.7 B/key, is registered in
`.github/superseded-figures.json`.

**`UNPREDICTED LOSS`: `counter` 100%-hit lookup on Arm C below a million keys.**
§10.7 registered `counter` lookup as a high-confidence Expanse win. It is one at
N = 1M (1.067 [1.063, 1.071]; run 2 1.061 [1.048, 1.075]) and a HOT win at
N = 10,000 (0.855 [0.845, 0.866]; run 2 0.848 [0.837, 0.861]) and N = 100,000
(0.864 [0.836, 0.875]; run 2 0.880 [0.862, 0.889]). The prediction was stated
without a population and was wrong below a million keys. On Arm D the same cell
is an Expanse win at N = 100,000 and 1M in both runs and spans parity at
N = 10,000 (1.003 [0.989, 1.019]; run 2 1.055 [0.963, 1.186]). **Correction:**
the `7fe02c0b` artifact had that Arm D cell as an Expanse win,
1.057 [1.045, 1.094]; the cause of the move is unmeasured.

### 6.2 Where the pre-registration was refuted in Expanse's favour

These are reported with the same prominence as the losses (§6 taxonomy). At
N = 1M, HOT ÷ Expanse:

| Cell | Registered (§10.7) | Run 1 | Run 2 | Label |
|---|---|---:|---:|---|
| `skewed` 100% hit, Arm C | HOT wins (low) | 1.222 [1.218, 1.228] | 1.234 [1.221, 1.260] | **`REFUTED`** |
| `skewed` 100% hit, Arm D | HOT wins (low) | 1.798 [1.793, 1.803] | 1.795 [1.790, 1.801] | **`REFUTED`** |
| `skewed` 50/50, Arm C | HOT wins (low) | 1.381 [1.375, 1.386] | 1.376 [1.370, 1.382] | **`REFUTED`** |
| `skewed` 50/50, Arm D | HOT wins (low) | 2.000 [1.993, 2.006] | 1.993 [1.987, 1.999] | **`REFUTED`** |
| `prefixed` 100% hit, Arm D | HOT wins (medium-high) | 1.112 [1.110, 1.115] | 1.113 [1.110, 1.115] | **`REFUTED`** |
| `prefixed` 50/50, Arm D | HOT wins (medium-high) | 1.266 [1.262, 1.269] | 1.265 [1.260, 1.269] | **`REFUTED`** |
| `prefixed` insert, Arm C | HOT wins (medium) | 1.336 [1.334, 1.339] | 1.336 [1.334, 1.338] | **`REFUTED`** |
| `prefixed` insert, Arm D | HOT wins (medium) | 1.431 [1.428, 1.433] | 1.431 [1.429, 1.434] | **`REFUTED`** |

*(workloads: `hot_str_ptr`, `hot_str_map`)*

The `skewed` row is the one §10.7 flagged as "registered because the issue
expects it; the mechanism reading does not support it". The mechanism reading
was right and the registration was wrong, in every population and on both
`ExpanseStrMap` arms. `prefixed` insertion is refuted at every population on
both arms. On `prefixed` point lookup, the loss that holds on Arm C at N = 1M
reverses on Arm D at N = 100,000 and 1M, and spans parity there at N = 10,000
in both runs; that HOT's per-entry heap pair is what reverses it is a
hypothesis, not a measurement. **Correction:** this paragraph previously
likened the reversal to the integer arms' uniform-random loss reversing on the
map arm (§2); since §2 was re-measured at `ae9e716e` the integer map's
uniform-random hit cell is a HOT win, so the likeness is withdrawn.

### 6.3 Confirmed wins, and the rest

`counter` at N = 1M is Expanse's on both `ExpanseStrMap` arms: 100% hit
1.067 [1.063, 1.071] (C) and 1.565 [1.560, 1.571] (D); insert
1.406 [1.403, 1.409] (C) and 1.661 [1.655, 1.667] (D) — **`CONFIRMED`**, with
the small-N caveats of §6.1 (run 2 1.061 [1.048, 1.075], 1.573 [1.566, 1.579],
1.403 [1.399, 1.407] and 1.661 [1.655, 1.667]). `short` 100% hit on Arm C is
1.243 [1.239, 1.246] (run 2 1.232 [1.228, 1.236]), **`CONFIRMED`**, and an
Expanse win at N = 10,000 and 100,000 in both runs too. Arm D's memory is
`PASS_categorical_by_design` for Expanse on every representable shape from
N = 5,000 (at N = 1M, HOT ownership 59.4–172.2 B/key against Expanse
20.5–64.0), as §10.7 registered; at N = 1,000 HOT's ownership is the smaller on
`short`, `counter` and `skewed`, and at N = 2,000 on `short`, in both runs and
in the `7fe02c0b` census *(workload: `hot_str_map`)*. The cells §10.7 declined
to predict — 50/50 lookups on `short` and `skewed`, insertion on `short` and
`skewed`, and all of Arm E — are `not pre-registered`; at N = 1M the
`ExpanseStrMap` ones are Expanse's (1.33–2.15 across both runs), the Arm E ones
split as described above.

![String keys, latency at N = 1M](results/chart_string_latency.svg)

### 6.4 Memory across the population sweep

![String keys, Arm C ownership across N](results/chart_string_memory_sweep.svg)

| N | `counter` HOT / Expanse | `short` HOT / Expanse | `skewed` HOT / Expanse | `prefixed` HOT / Expanse | `beyond` — / Expanse |
|---:|---:|---:|---:|---:|---:|
| 1,000 | 44.7 / 90.8 | 44.8 / 86.4 | 51.2 / 80.3 | 157.5 / 102.5 | — / 87.0 |
| 10,000 | 36.6 / 27.5 | 37.0 / 54.4 | 42.7 / 47.9 | 149.1 / 70.6 | — / 54.5 |
| 100,000 | 35.5 / 21.1 | 36.1 / 43.6 | 41.9 / 37.0 | 148.1 / 59.3 | — / 43.6 |
| 125,000 | 35.5 / 21.0 | 36.4 / 47.0 | 42.2 / 40.6 | 148.4 / 63.0 | — / 47.0 |
| 150,000 | 35.5 / 20.9 | 36.7 / 50.1 | 42.5 / 43.6 | 148.7 / 66.0 | — / 50.0 |
| 200,000 | 35.5 / 20.8 | 36.5 / 51.2 | 42.3 / 44.6 | 148.5 / 66.9 | — / 51.2 |
| 1,000,000 | 35.4 / 20.5 | 36.2 / 48.2 | 42.0 / 41.7 | 148.2 / 64.0 | — / 48.2 |

Arm C ownership, B/key, identical in both runs *(workload: `hot_str_ptr`)*; the
2k, 5k, 20k, 50k and 500k rows are in `results/baseline_string_memory.json`.
**HOT is flat again** — 35.4–37.0 B/key on `short` and `counter` and 41.8–42.7
on `skewed` from 10k to 1M, the string analogue of the integer arms'
11.68–12.06 — and Expanse's line drops under it on `counter` from N = 5,000 and
on `skewed` at 50k–125k and at 1M. **Correction:** this paragraph previously
gave HOT's range as 35.4–36.7 B/key and named `counter` as the only shape under
it; `skewed` dips under HOT in the `7fe02c0b` census as well.

**The registered chunk-occupancy hypothesis is consistent with the sweep and
not confirmed by it.** §10.5 predicted a `LEAF_CAP` cascade in the
discriminating chunk map near N ≈ 1.23 × 10⁵ for the random-alphanumeric
shapes and none for `counter`. Between 100k and 150k the Expanse line rises
on exactly those shapes — `short` 43.6 → 47.0 → 50.1, `prefixed`
59.3 → 63.0 → 66.0, `skewed` 37.0 → 40.6 → 43.6, `beyond` 43.6 → 47.0 → 50.0 —
and `counter` does not move (21.1 → 21.0 → 20.9). The step is +11% to +18%, not
the 2.6× swing of the integer arm (§1). That stored key bytes dilute it is a
reading, not a measurement. **Correction:** the rows quoted here previously were
figures from before [#723](https://github.com/orieg/expanse/issues/723) that
disagreed with the table above them, and put the step at about +11%. The
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
272-byte keys at 48.19 B/key, identically in both runs, and `ExpanseBytesMap`
at 353.36–353.42 B/key *(workload: `hot_string_memory`)*; their 100%-hit
lookups at N = 1M take 334.77 ns and 329.95 ns respectively, and 333.87 ns and
329.59 ns in run 2 *(workload: `hot_string_latency`)*. The figures previously
printed here (47.83 and 352.26 B/key; 346.16 and 326.92 ns) are superseded.

### 6.6 Scorecard

225 latency cells, 180 memory cells. Latency cells with a HOT column: 180.

| | Count, each run |
|---|---:|
| HOT wins (CI excludes parity) | 96 — of which 72 are scan cells |
| Expanse wins (CI excludes parity) | 77 — none is a scan cell |
| `BOUNDARY_RESULT` | 7 |
| HOT column withheld (`beyond`, §10.4) | 45 |

The counts are identical in the two runs but not cell for cell. Six of the seven
`BOUNDARY_RESULT` cells are the same in both — Arm C `prefixed` 50/50 at
N = 100,000; Arm D `counter` 100% hit, `prefixed` 100% hit and `prefixed` 50/50
at N = 10,000; Arm E `prefixed` 50/50 at N = 10,000 and 100,000 — and the
seventh is Arm E `short` 100% hit at N = 10,000 in run 1 and Arm E `skewed`
100% hit at N = 10,000 in run 2, each a HOT win in the other run.

> **Correction: the scorecard is re-counted.** It previously read 98 HOT wins,
> which with 77 and 7 summed to 182 of 180 cells; the `7fe02c0b` artifact holds
> 98 / 77 / 5. Against that artifact three cells changed class in both runs —
> Arm C `prefixed` 100% hit at N = 100,000 (`BOUNDARY_RESULT` → Expanse), Arm D
> `prefixed` 100% hit at N = 10,000 (HOT → `BOUNDARY_RESULT`) and Arm D `counter`
> 100% hit at N = 10,000 (Expanse → `BOUNDARY_RESULT`) — and none at N = 1M or on
> any scan cell. The causes are unmeasured.

Against §10.7:

| Registered | Outcome |
|---|---|
| HOT wins ordered scan, every k, every shape (high) | **CONFIRMED**, 72 of 72 in both runs — still, after [#722](https://github.com/orieg/expanse/issues/722) moved every cell 1.38×–11.21× in Expanse's favour |
| HOT wins `prefixed` point lookup (medium-high) | **CONFIRMED** on Arm C at N = 10k and 1M; **REFUTED** on Arm C's 100%-hit cell at 100k; **REFUTED** on Arm D at 100k and 1M, `BOUNDARY_RESULT` at 10k |
| HOT wins `prefixed` insert (medium) | **REFUTED** on both arms, at every population |
| HOT wins the `index` memory column on long and skewed keys (high, categorical) | **CONFIRMED**, `PASS_categorical_by_design` in HOT's favour, on every shape |
| HOT wins `skewed` point lookup (low) | **REFUTED** on both arms, at every population |
| Expanse wins `counter` lookup and insert (high) | **CONFIRMED** at N = 1M; **UNPREDICTED LOSS** on Arm C's 100%-hit lookup at 10k and 100k; `BOUNDARY_RESULT` on Arm D's at 10k |
| Expanse wins `short` 100%-hit lookup, Arm C (medium) | **CONFIRMED**, at every population |
| Expanse wins `ownership` memory on `counter` and `short` (low-medium) | **CONFIRMED** on `counter`; **REFUTED** on `short` |
| Expanse wins Arm D memory (high, categorical) | **CONFIRMED**, `PASS_categorical_by_design`, from N = 5,000 |
| `λ_chunk` cascade near N ≈ 1.23 × 10⁵ (hypothesis) | consistent, not confirmed — §6.4 |
## 7. The concurrent arm: HOT-ROWEX against the multi-writer engine

Delivers [#692](https://github.com/orieg/expanse/issues/692): HOT's **ROWEX**
variant (concurrent insert and lookup, no deletion) against `SyncExpanseSet`
and `SyncExpanseMap`. Pre-registration, locked constraint decisions and the
expected-losses matrix are `METHODOLOGY.md` §11; nothing there was edited after
measurement.

> *(measured: reference host — Intel Core i9-12900F, 8P+8E/24 threads, 30 MiB L3,
> Ubuntu 22.04, kernel 6.8; HOT `96bf6fb` with its pinned TBB 2018 `4c73c3b`,
> built from the nested submodule, no system TBB; harness commit `6f8d6ba5`,
> two runs; `docs/benchmarks/hot_comparison/run.sh --only-concurrent`; benchmark
> shell pinned to CPUs 0–15 and every row records `Cpus_allowed_list 0-15`;
> writers + readers ≤ 16; both arms `-C target-cpu=haswell` / `-march=haswell`,
> both on glibc 2.35 `malloc`; load average 1.13 and 0.95 at the two runs'
> starts and at most 5.76 and 6.29 during them — the sweep's own threads — with a
> load snapshot per cell putting foreign busy CPU at no more than
> 0.03 core-equivalents in any cell of either run; 15 rounds per throughput
> cell and 5 per health cell, arms interleaved per round, medians reported,
> BCa 95% bootstrap ratio intervals over 2,000 resamples and every round in
> `rounds_raw`; the levels in §7.1, §7.2 and §7.4 are **run 1**, §7.3 carries
> both runs and §7.6 sets every ratio cell of the two side by side;
> `results/baseline_concurrent.json`, `results/baseline_concurrent_run2.json`;
> workloads `hot_rowex_set_63bit`, `hot_rowex_map_64bit`)*.
>
> Both arms are measured **below any external lock**, through their native
> concurrent APIs (§8.16; `METHODOLOGY.md` §11.3 decision 4). `SyncExpanseSet`
> and `SyncExpanseMap` admit concurrent writers through optimistic lock
> coupling over per-node version words (`docs/ARCHITECTURE.md` §4.2), and the
> protocol is blocking by design (`AGENTS.md` §2.2). Ratios are **Expanse ÷
> ROWEX throughput**, so, as everywhere in this suite, **above 1.000 means
> Expanse is faster.**
>
> **Re-measured after [#949](https://github.com/orieg/expanse/pull/949)
> (AGENTS.md §8.7).** This section previously published two runs at
> `b868fb2e`. After #928, the 64-bit `SyncExpanseMap` reader in the Masstree
> suite's `masstree_concurrent` binary paid store-to-load forwarding blocks in
> `sync::walk_validated` (`ld_blocks.store_forward` per read, measured in #949
> on that binary's readers-only cell), and #949 inlined the function. This
> arm's `SyncExpanseSet` and `SyncExpanseMap` readers go through the same
> function, and code generation is chosen per binary, so the published reader
> cells were stale until re-measured; both runs here are at `6f8d6ba5`, which
> contains #949. The `b868fb2e` figures are superseded, and §7.2 states for
> every reader cell whether it moved. The single-writer engine's figures before
> those are superseded too; the `a1982ff2` pair among them is kept at
> `results/step0/`, the data `docs/benchmarks/concurrency/README.md` §3–§5 and
> §8 read.

### 7.1 Writer throughput as writer count scales — ROWEX wins from four writers

W writers each insert their slice of 2²⁰ fresh keys into a 2²⁰ prefill; fixed
work, so both arms grow by exactly the same population every round.

![Writer throughput vs writer count](results/chart_concurrent_writers.svg)

| W | set: ROWEX M/s | set: Expanse M/s | ratio [BCa 95%] | verdict | map: ROWEX M/s | map: Expanse M/s | ratio [BCa 95%] | verdict |
|--:|---:|---:|---|---|---:|---:|---|---|
| 1 | 5.33 | **7.36** | 1.382 [1.367, 1.398] | Expanse | 2.95 | **3.88** | 1.347 [1.296, 1.413] | Expanse |
| 2 | **8.75** | 8.58 | 0.981 [0.972, 0.995] | **ROWEX** | 5.04 | **5.65** | 1.111 [1.067, 1.148] | Expanse |
| 4 | **15.29** | 9.94 | 0.660 [0.651, 0.672] | **ROWEX** | **8.75** | 7.74 | 0.898 [0.874, 0.924] | **ROWEX** |
| 8 | **24.97** | 13.50 | 0.546 [0.531, 0.560] | **ROWEX** | **15.04** | 10.94 | 0.715 [0.693, 0.733] | **ROWEX** |
| 16 | **33.50** | 14.84 | 0.456 [0.444, 0.483] | **ROWEX** · *not pre-registered (SMT)* | **21.11** | 10.18 | 0.480 [0.468, 0.489] | **ROWEX** · *not pre-registered (SMT)* |

- **Expanse wins with one writer** — 1.382 [1.367, 1.398] (set) and
  1.347 [1.296, 1.413] (map) in run 1, 1.386 [1.366, 1.402] and
  1.337 [1.283, 1.405] in run 2 — **`CONFIRMED`** (§11.5.2, medium-high).
- **At two writers the arms split.** The map arm is Expanse's in both runs,
  1.111 [1.067, 1.148] and 1.102 [1.056, 1.147] — **`REFUTED`** in
  Expanse's favour, since §11.5.1 registered ROWEX or `BOUNDARY_RESULT`. The set
  arm is ROWEX's in run 1, 0.981 [0.972, 0.995], and claims no winner in
  run 2, 0.984 [0.974, 1.011]: the runs disagree on a winner, so under
  `docs/BENCHMARKING.md` rule 18 the cell is direction-only. Both outcomes lie
  inside the registered row, which is **`CONFIRMED`** on the set arm.
- **The crossover lies inside the registered W\* ∈ [2, 4]** — **`CONFIRMED`**
  (§11.5.1, medium): W\* = 4 on the map arm in both runs; on the set arm W\* = 2
  in run 1 and 4 in run 2, where W = 2 claims no winner.
- **At W ≥ 4 ROWEX wins every cell, by 1.09×–2.23×** across W = 4, 8 and 16 in
  both runs — **`CONFIRMED`** (§11.5.1, high). Expanse's aggregate writer
  throughput rises with writer count to eight on both arms — set
  7.36 → 8.58 → 9.94 → 13.50 → 14.84 M inserts/s, map
  3.88 → 5.65 → 7.74 → 10.94 → 10.18 in run 1 — reaching 2.01–2.02× (set) and
  2.62–2.65× (map) its one-writer rate at sixteen across the two runs. ROWEX
  scales 4.66–4.69× (set) and 4.93–5.10× (map) at W = 8 and 6.26–6.29× /
  7.16–7.32× at W = 16, where the sixteen threads occupy both SMT siblings of
  every P-core.
- **Against the `b868fb2e` pair this section previously published**, no writer
  cell's interval in either run lies clear of both earlier intervals, and no
  C1 verdict changed in both runs, so no writer level is claimed to have moved
  (`docs/BENCHMARKING.md` rule 18). The set arm's W = 2 cell was
  direction-only then and is direction-only now, with the runs' verdicts in the
  other order.

What limits Expanse's writer scaling, and what separates it from ROWEX's, is
**unmeasured** here: this arm carries no hardware counters (§8.9), and no
mechanism is claimed.

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
| 0 | 126.79 | **148.97** | 1.194 [1.174, 1.249] | Expanse | 70.34 | **104.84** | 1.522 [1.480, 1.580] | Expanse |
| 1 | 109.53 | **136.08** | 1.231 [1.209, 1.243] | Expanse | 55.38 | **97.32** | 1.769 [1.718, 1.824] | Expanse |
| 2 | 102.74 | **124.14** | 1.214 [1.205, 1.231] | Expanse | 51.75 | **92.96** | 1.817 [1.770, 1.887] | Expanse |
| 4 | 91.85 | **108.56** | 1.205 [1.185, 1.228] | Expanse | 44.30 | **82.21** | 1.865 [1.825, 1.906] | Expanse |
| 8 | 70.57 | **81.56** | 1.143 [1.104, 1.172] | Expanse | 30.37 | **62.97** | 2.090 [2.022, 2.160] | Expanse |

- **Reader-only (W = 0):** Expanse wins on both arms. The map row is
  **`CONFIRMED`** (§11.5.2, medium), 1.522 [1.480, 1.580] and
  1.486 [1.389, 1.543]. The set row was registered as `BOUNDARY_RESULT`
  and landed as an Expanse win in both runs, 1.194 [1.174, 1.249] and
  1.172 [1.131, 1.199] — recorded as a registered no-winner that resolved
  in Expanse's favour, not as a confirmed prediction.
- **Readers under writer load: Expanse wins every cell in both runs** —
  1.143–1.243 on the set arm and 1.769–2.090 on the map arm across W = 1, 2,
  4 and 8 — so the registered ROWEX win (§11.5.1, medium-high) is **`REFUTED`**
  in Expanse's favour on all eight cells. With one writer, Expanse's eight
  readers keep 0.91× (set) and 0.93× (map) of their reader-only rate
  and ROWEX's keep 0.86× and 0.78–0.79×; with eight writers,
  0.54–0.55× and 0.60× against 0.55–0.56× and 0.42–0.43× (the two runs'
  range). Every reader cell's two intervals overlap (§7.6). What
  sets these reader levels is **unmeasured**: no hardware counter was taken on
  either arm (§8.9).
- **Writers with readers present** *(not registered as a separate row;
  reported)*: with eight readers probing, the Expanse single writer runs at
  3.94 M inserts/s (set) and 2.84 (map), against 7.36 and 3.88 without
  readers; ROWEX's at 3.42 and 2.22, against 5.33 and 2.95 (run 1).
  Expanse wins the W = 1 writer cell on both arms — set
  1.187 [1.148, 1.324] and 1.145 [1.085, 1.243], map
  1.460 [1.312, 1.625] and 1.243 [1.151, 1.403] — and the map arm at
  W = 2, 1.095 [1.046, 1.154] and 1.081 [1.047, 1.116]. The set arm's
  W = 2 writer cell is ROWEX's in both runs, 0.934 [0.912, 0.960] and
  0.926 [0.906, 0.944]; the `b868fb2e` pair previously published it as
  direction-only (`BOUNDARY_RESULT` in run 1, ROWEX in run 2), so its verdict is
  settled in both new runs, while both new intervals overlap the earlier run 2's
  and no change in level is claimed. ROWEX wins every writer cell at W ≥ 4:
  0.586–0.723 (set) and 0.716–0.923 (map).

**The reader cells against the `b868fb2e` pair (AGENTS.md §8.7).** The
readers were re-measured because #949 changed the function they call (see the
note at the top of this section). Every earlier figure in the third column is
superseded, and each new run is set against both of the earlier runs:

| Arm | W | previously published at `b868fb2e` (run 1; run 2) | `6f8d6ba5` run 1 | `6f8d6ba5` run 2 | new run clear of both earlier intervals |
|---|--:|---|---|---|---|
| set | 0 | previously 1.197 [1.153, 1.224]; 1.194 [1.190, 1.198] | 1.194 [1.174, 1.249] | 1.172 [1.131, 1.199] | neither |
| set | 1 | previously 1.260 [1.254, 1.267]; 1.274 [1.258, 1.286] | 1.231 [1.209, 1.243] | 1.243 [1.219, 1.254] | run 1 |
| set | 2 | previously 1.218 [1.209, 1.236]; 1.235 [1.216, 1.251] | 1.214 [1.205, 1.231] | 1.227 [1.216, 1.241] | neither |
| set | 4 | previously 1.219 [1.198, 1.241]; 1.164 [1.142, 1.187] | 1.205 [1.185, 1.228] | 1.189 [1.168, 1.210] | neither |
| set | 8 | previously 1.157 [1.136, 1.178]; 1.164 [1.130, 1.191] | 1.143 [1.104, 1.172] | 1.153 [1.132, 1.173] | neither |
| map | 0 | previously 1.533 [1.490, 1.597]; 1.526 [1.479, 1.592] | 1.522 [1.480, 1.580] | 1.486 [1.389, 1.543] | neither |
| map | 1 | previously 1.763 [1.722, 1.808]; 1.801 [1.753, 1.842] | 1.769 [1.718, 1.824] | 1.781 [1.734, 1.830] | neither |
| map | 2 | previously 1.754 [1.714, 1.800]; 1.772 [1.723, 1.823] | 1.817 [1.770, 1.887] | 1.816 [1.783, 1.850] | neither |
| map | 4 | previously 1.844 [1.800, 1.899]; 1.848 [1.812, 1.895] | 1.865 [1.825, 1.906] | 1.841 [1.739, 1.899] | neither |
| map | 8 | previously 2.075 [1.987, 2.147]; 2.061 [1.991, 2.141] | 2.090 [2.022, 2.160] | 2.087 [2.014, 2.158] | neither |

**No HOT reader cell moved, and no reader verdict changed.** In 9 of the 10
cells both new intervals overlap both earlier ones. In the set arm's W = 1
cell run 1's interval lies below both earlier intervals, while run 2's upper
bound touches the lower bound of the earlier run 1 (1.254). Both new point
estimates sit below both earlier ones, but the two new runs do not both clear
the earlier pair, so under `docs/BENCHMARKING.md` rule 18 that cell is not
claimed to have moved. The ratios above, not the per-arm levels, are what this
compares; the levels carry no interval. Between the same two commits the
Masstree suite's integer reader cells moved up in both runs
(`masstree_comparison` README §2); why those moved and these did not is
**unmeasured**: no counter was taken on this arm, and code generation is
chosen per binary.

### 7.3 Protocol health — event ratios from the diagnostic build

The 64-bit protocol has no `Busy` outcome; its counterpart to the 32-bit Busy
rate is the **restart share** (walk attempts that observed a moved version and
restarted) and the **fallback share** (reads that exhausted 64 restarts and took
the writer mutex), read from the engine's `occ_stats` counters on a **separate
`occ-stats` build** of the same cells, Expanse side only (§11.3, decision 5).
Nothing in this table is a timing. 5 rounds per cell; median with range.

> The table below is the output of `scripts/integer_tables.py` over `results/baseline_concurrent.json` (run 1) and `results/baseline_concurrent_run2.json` (run 2), both runs side by side per `docs/BENCHMARKING.md` rule 18; nothing in it is typed by hand, and a column the artifacts do not carry reads `not recorded`.

| Arm | W | R | run | restart share, median [min, max] | fallback share | `sample_spins` ÷ `read_ops` (ratio of medians) | `locked_reads` ÷ `read_ops` | unconditional lock share | handoffs ÷ write | branch replacements ÷ write | deep-cascade share | root-rewrite share | spin time ÷ reader wall | §11.5.3 |
|---|--:|--:|--:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| set | 1 | 8 | 1 | 0.02% [0.02%, 0.02%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.034 | 1.71% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| set | 1 | 8 | 2 | 0.02% [0.02%, 0.02%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.034 | 1.71% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 9 in a round) |
| set | 2 | 8 | 1 | 0.03% [0.02%, 0.03%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.035 | 1.71% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 2 in a round) |
| set | 2 | 8 | 2 | 0.03% [0.02%, 0.03%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.035 | 1.71% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 1 in a round) |
| set | 4 | 8 | 1 | 0.03% [0.03%, 0.03%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.036 | 1.71% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 1 in a round) |
| set | 4 | 8 | 2 | 0.03% [0.03%, 0.03%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.036 | 1.71% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| set | 8 | 8 | 1 | 0.05% [0.04%, 0.05%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.038 | 1.71% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 1 in a round) |
| set | 8 | 8 | 2 | 0.05% [0.04%, 0.05%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.038 | 1.71% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 1 in a round) |
| map | 1 | 8 | 1 | 0.18% [0.17%, 0.23%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.057 | 2.84% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 2 in a round) |
| map | 1 | 8 | 2 | 0.18% [0.18%, 0.18%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.057 | 2.84% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 9 in a round) |
| map | 2 | 8 | 1 | 0.27% [0.25%, 0.28%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.058 | 2.84% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 9 in a round) |
| map | 2 | 8 | 2 | 0.27% [0.26%, 0.32%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.058 | 2.84% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 36 in a round) |
| map | 4 | 8 | 1 | 0.35% [0.34%, 0.37%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.061 | 2.84% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 16 in a round) |
| map | 4 | 8 | 2 | 0.37% [0.35%, 0.43%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.061 | 2.84% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 15 in a round) |
| map | 8 | 8 | 1 | 0.46% [0.45%, 0.49%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.067 | 2.84% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 4 in a round) |
| map | 8 | 8 | 2 | 0.49% [0.43%, 0.50%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.067 | 2.84% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 3 in a round) |

- **The restart share rises with W on both arms, in both runs** —
  **`CONFIRMED`** (§11.5.3), and the two runs agree on the ordering, so the rise
  is settled on this pair rather than direction-only. It sits at
  0.02%–0.05% of walks on the set arm and 0.18%–0.49% on the map arm.
- **Readers did take the writer mutex, and the starvation falsifier did not
  fire.** A fallback was recorded in 14 of the 16 cells: at most 9 in a
  round on the set arm (W = 1, run 2), and at most 36 on the map arm (W = 2,
  run 2). The median fallback share rounds to 0.0000% in every cell, far below
  the 1% line of §11.5.3, so the second half is a measured **`CONFIRMED`**
  wherever a fallback occurred. In the 2 cells where no round recorded one —
  the set arm at W = 1 in run 1 and at W = 4 in run 2 — the zero is
  **`PASS_categorical_by_design`**: a fallback needs **64 consecutive failed
  optimistic walks**, so the falsifier could not have fired there, and a
  falsifier that cannot fire is not a measurement (AGENTS.md §8).
  METHODOLOGY §11.8 registers one that can.
- `sample_spins` reads zero at the median of every cell (four rounds are
  non-zero, all on the map arm, at most 8 spins in a round), spin time ÷ reader
  wall reads 0.00%, and the unconditional lock share reads 0.00%. These are event counts from the
  counting build; nothing in this table attributes a §7.2 reader level to
  them.

### 7.4 Memory — build-only, single writer, curve across λ

Bytes held from the C allocator after a single-writer build (§11.3, decision 1),
ROWEX against the `Sync*` wrapper, swept across the §9.6 occupancy targets.
Deterministic byte counts, no interval, byte-identical in both runs. Every ROWEX
cell counted at least 2N allocations on the map arm and N on the set arm (see
`hot_allocs` in the artifact). **Disclosed blind spot:** `libtbb.so`'s own
per-thread state is allocated through the dynamic linker and is invisible to the
link-time interposition; it is paid once per registering thread and is
independent of N.

| λ | set: ROWEX B/key | set: `SyncExpanseSet` B/key | winner | map: ROWEX B/key | map: `SyncExpanseMap` B/key | winner |
|--:|---:|---:|---|---:|---:|---|
| 1 | **12.91** | 15.82 | ROWEX 1.22× | 36.71 | **25.51** | Expanse 1.44× |
| 2 | **12.40** | 13.59 | ROWEX 1.10× | 36.31 | **24.85** | Expanse 1.46× |
| 4 | **12.00** | 12.28 | ROWEX 1.02× | 36.21 | **23.03** | Expanse 1.57× |
| 8 | 11.88 | **9.96** | Expanse 1.19× | 36.16 | **19.44** | Expanse 1.86× |
| 15 | 11.76 | **8.73** | Expanse 1.35× | 36.07 | **17.95** | Expanse 2.01× |
| 23 | 11.73 | **8.81** | Expanse 1.33× | 36.09 | **17.67** | Expanse 2.04× |
| 30 | **11.73** | 14.31 | ROWEX 1.22× | 36.05 | **20.93** | Expanse 1.72× |
| 38 | **11.80** | 21.98 | ROWEX 1.86× | 36.15 | **25.95** | Expanse 1.39× |
| 46 | **11.71** | 23.07 | ROWEX 1.97× | 36.09 | **26.78** | Expanse 1.35× |
| 61 | **11.72** | 21.06 | ROWEX 1.80× | 36.05 | **25.60** | Expanse 1.41× |

- **Set arm:** the §1 story repeats with the concurrent types — ROWEX is flat
  (11.71–12.91 B/key), Expanse wins only in the band λ ∈ [8, 23]
  (1.19×–1.35×) and loses on both sides of it (1.02×–1.22× below,
  1.22×–1.97× above). Both §11.5 memory rows for the set arm are
  **`CONFIRMED`**.
- **Map arm:** Expanse wins at every occupancy, 1.35×–2.04× —
  **`CONFIRMED`** and labelled **`PASS_categorical_by_design`**: ROWEX carries
  the same heap `std::pair` per entry as the single-threaded map arm.
- The `SyncExpanseSet` cells differ from §1's `ExpanseSet` cells at the same
  λ in both directions (15.82 against 17.05 at λ = 1; 21.06 against 20.28 at
  λ = 61) *(workloads differ: `hot_rowex_set_63bit` vs `hot_memory_curve`,
  harness commits `6f8d6ba5` and `ae9e716e`)*. The two are different types
  under the same instrument and the cause of the gap is unmeasured; they are
  not set side by side as one quantity.

### 7.5 Scorecard against the pre-registration

20 throughput cells (10 writer, 10 reader, plus 8 writer-under-reader
sub-cells), 8 health cells per run, 20 memory cells; two runs of every
wall-clock cell.

| Registered (`METHODOLOGY.md` §11.5) | Outcome |
|---|---|
| ROWEX wins writer throughput at W ≥ 4 (high) | **CONFIRMED**, 1.09×–2.23× across W = 4, 8 and 16, both runs |
| ROWEX wins or `BOUNDARY_RESULT` at W = 2 (medium) | set **CONFIRMED** (ROWEX in run 1, `BOUNDARY_RESULT` in run 2; direction-only); map **REFUTED** in Expanse's favour, 1.102–1.111 |
| Crossover writer count W\* ∈ [2, 4] (medium) | **CONFIRMED**: W\* = 4 on the map arm; 2 or 4 on the set arm |
| ROWEX wins reader throughput under W ≥ 1 (medium-high) | **REFUTED** in Expanse's favour on all 8 cells in both runs, 1.143–2.090 |
| Expanse wins writer throughput at W = 1 (medium-high) | **CONFIRMED**, 1.382–1.386 (set) / 1.337–1.347 (map) |
| Expanse wins reader-only, map arm (medium) | **CONFIRMED**, 1.486–1.522 |
| Reader-only, set arm: `BOUNDARY_RESULT` (low-medium) | registered no-winner; **measured Expanse win** 1.194 [1.174, 1.249] and 1.172 [1.131, 1.199] |
| Memory, map arm, all λ (high) | **CONFIRMED**, `PASS_categorical_by_design` |
| Memory, set arm: Expanse wins λ ∈ [8, 23], ROWEX outside (medium) | **CONFIRMED** on both sides |
| Health: fallback share < 1% at all W (falsifier) | did not fire: **`CONFIRMED`** in the 14 cells where a fallback was recorded, **`PASS_categorical_by_design`** in the 2 where none was (§7.3) |
| Health: restart share rises monotonically with W | **CONFIRMED** on both arms in both runs (§7.3) |
| W = 16 cells | `not pre-registered`; reported: 0.456 [0.444, 0.483] and 0.448 [0.439, 0.458] (set), 0.480 [0.468, 0.489] and 0.468 [0.455, 0.478] (map) |
| Writers with readers present | `not pre-registered`; reported: Expanse wins at W = 1 on both arms and at W = 2 on the map arm, ROWEX wins the set arm from W = 2 and the map arm from W = 4, both runs |

No `UNPREDICTED LOSS`: every cell Expanse lost was registered as a loss.

What this arm does not claim is fixed in `METHODOLOGY.md` §11.6: x86-64 at these
two commits under glibc `malloc` only (no tcmalloc claim); insert and point
lookup on uniform random integer keys only — no deletion, contended-key, scan or
string claim; at most 16 threads on 8 physical performance cores with SMT; and no
peer review.

### 7.6 Between-run spread: two runs at one commit (#735)

The arm was run twice on the reference host **at one commit**, `6f8d6ba5`, both
under the P-core pin with a load snapshot per cell. The binaries are identical,
so the table below is run-to-run spread on this host and nothing else
*(workloads: `hot_rowex_set_63bit`, `hot_rowex_map_64bit`)*.

| Arm | cell | W | R | run 1 | run 2 | intervals overlap | verdict |
|---|---|--:|--:|---|---|---|---|
| set | C1 writer | 1 | 0 | 1.382 [1.367, 1.398] | 1.386 [1.366, 1.402] | yes | Expanse |
| set | C1 writer | 2 | 0 | 0.981 [0.972, 0.995] | 0.984 [0.974, 1.011] | yes | runs disagree (ROWEX / `BOUNDARY_RESULT`) — direction-only |
| set | C1 writer | 4 | 0 | 0.660 [0.651, 0.672] | 0.667 [0.660, 0.677] | yes | ROWEX |
| set | C1 writer | 8 | 0 | 0.546 [0.531, 0.560] | 0.560 [0.545, 0.579] | yes | ROWEX |
| set | C1 writer | 16 | 0 | 0.456 [0.444, 0.483] | 0.448 [0.439, 0.458] | yes | ROWEX |
| set | C2 reader | 0 | 8 | 1.194 [1.174, 1.249] | 1.172 [1.131, 1.199] | yes | Expanse |
| set | C2 reader | 1 | 8 | 1.231 [1.209, 1.243] | 1.243 [1.219, 1.254] | yes | Expanse |
| set | C2 writer | 1 | 8 | 1.187 [1.148, 1.324] | 1.145 [1.085, 1.243] | yes | Expanse |
| set | C2 reader | 2 | 8 | 1.214 [1.205, 1.231] | 1.227 [1.216, 1.241] | yes | Expanse |
| set | C2 writer | 2 | 8 | 0.934 [0.912, 0.960] | 0.926 [0.906, 0.944] | yes | ROWEX |
| set | C2 reader | 4 | 8 | 1.205 [1.185, 1.228] | 1.189 [1.168, 1.210] | yes | Expanse |
| set | C2 writer | 4 | 8 | 0.723 [0.710, 0.734] | 0.719 [0.702, 0.737] | yes | ROWEX |
| set | C2 reader | 8 | 8 | 1.143 [1.104, 1.172] | 1.153 [1.132, 1.173] | yes | Expanse |
| set | C2 writer | 8 | 8 | 0.589 [0.577, 0.613] | 0.586 [0.577, 0.596] | yes | ROWEX |
| map | C1 writer | 1 | 0 | 1.347 [1.296, 1.413] | 1.337 [1.283, 1.405] | yes | Expanse |
| map | C1 writer | 2 | 0 | 1.111 [1.067, 1.148] | 1.102 [1.056, 1.147] | yes | Expanse |
| map | C1 writer | 4 | 0 | 0.898 [0.874, 0.924] | 0.915 [0.892, 0.942] | yes | ROWEX |
| map | C1 writer | 8 | 0 | 0.715 [0.693, 0.733] | 0.726 [0.699, 0.747] | yes | ROWEX |
| map | C1 writer | 16 | 0 | 0.480 [0.468, 0.489] | 0.468 [0.455, 0.478] | yes | ROWEX |
| map | C2 reader | 0 | 8 | 1.522 [1.480, 1.580] | 1.486 [1.389, 1.543] | yes | Expanse |
| map | C2 reader | 1 | 8 | 1.769 [1.718, 1.824] | 1.781 [1.734, 1.830] | yes | Expanse |
| map | C2 writer | 1 | 8 | 1.460 [1.312, 1.625] | 1.243 [1.151, 1.403] | yes | Expanse |
| map | C2 reader | 2 | 8 | 1.817 [1.770, 1.887] | 1.816 [1.783, 1.850] | yes | Expanse |
| map | C2 writer | 2 | 8 | 1.095 [1.046, 1.154] | 1.081 [1.047, 1.116] | yes | Expanse |
| map | C2 reader | 4 | 8 | 1.865 [1.825, 1.906] | 1.841 [1.739, 1.899] | yes | Expanse |
| map | C2 writer | 4 | 8 | 0.923 [0.888, 0.956] | 0.901 [0.864, 0.935] | yes | ROWEX |
| map | C2 reader | 8 | 8 | 2.090 [2.022, 2.160] | 2.087 [2.014, 2.158] | yes | Expanse |
| map | C2 writer | 8 | 8 | 0.716 [0.690, 0.741] | 0.756 [0.727, 0.780] | yes | ROWEX |

**None of the 28 ratio cells moved past its own interval**, and **in 1 cell
the runs disagree on a winner**: set C1 W = 2, ROWEX's in run 1 and
`BOUNDARY_RESULT` in run 2, with overlapping intervals. That cell is
direction-only; every other cell has the same verdict in both runs. The concurrent memory cells are byte-identical between the runs, which is
the control: a deterministic census taken by the same code on the same host
reproduces exactly, so the wall-clock spread is not the instrument reading
differently.

Per `docs/BENCHMARKING.md` rule 18 the claim ceiling on a concurrent cell is
the union of its two runs' intervals, and a citation of a cell outside this
suite states a direction and a range. The `masstree_comparison` arm's pair at
the same commit is in its README §7.

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
