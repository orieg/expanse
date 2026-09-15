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
12.205 [12.181, 12.271] in run 2 (`results/baseline_latency.json` is run 1). §5.2 registered this as a *weak*
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
1.518 (`results/baseline_latency.json` is run 1). At `5232af74`, with HOT timed
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

| Arm | W | R | run | restart share, median [min, max] | fallback share | `sample_spins` ÷ `read_ops` (ratio of medians) | `locked_reads` ÷ `read_ops` | unconditional lock share | handoffs ÷ write | branch replacements ÷ write | deep-cascade share | root-rewrite share | spin time ÷ reader wall | §11.5.3 |
|---|--:|--:|--:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| set | 1 | 8 | 1 | 9.23% [9.12%, 9.46%] | 0.0000% | 2.52 | 0.00% | 0.00% | 0.000 | 0.034 | 1.71% | 0.00% | 52.76% | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| set | 1 | 8 | 2 | 13.39% [12.57%, 30.84%] | 0.0000% | 3.36 | 0.00% | 0.00% | 0.000 | 0.034 | 1.71% | 0.00% | 57.33% | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| set | 2 | 8 | 1 | 6.05% [5.85%, 6.08%] | 0.0000% | 1.77 | 0.00% | 0.00% | 0.127 | 0.034 | 1.71% | 0.00% | 48.48% | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| set | 2 | 8 | 2 | 5.89% [5.76%, 6.05%] | 0.0000% | 1.81 | 0.00% | 0.00% | 0.124 | 0.034 | 1.71% | 0.00% | 49.23% | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| set | 4 | 8 | 1 | 6.72% [5.80%, 8.02%] | 0.0000% | 1.85 | 0.00% | 0.00% | 0.280 | 0.034 | 1.71% | 0.00% | 48.36% | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| set | 4 | 8 | 2 | 8.30% [8.01%, 9.97%] | 0.0000% | 2.21 | 0.00% | 0.00% | 0.472 | 0.034 | 1.71% | 0.00% | 50.86% | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| set | 8 | 8 | 1 | 12.03% [11.39%, 12.87%] | 0.0000% | 2.39 | 0.00% | 0.00% | 0.546 | 0.034 | 1.71% | 0.00% | 52.14% | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| set | 8 | 8 | 2 | 12.01% [11.46%, 12.37%] | 0.0000% | 2.52 | 0.00% | 0.00% | 0.532 | 0.034 | 1.71% | 0.00% | 51.73% | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 1 | 8 | 1 | 10.15% [9.74%, 24.83%] | 0.0000% | 3.38 | 0.00% | 0.00% | 0.000 | 0.057 | 2.84% | 0.00% | 58.74% | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 1 | 8 | 2 | 12.17% [11.88%, 23.81%] | 0.0000% | 3.64 | 0.00% | 0.00% | 0.000 | 0.057 | 2.84% | 0.00% | 59.91% | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 2 | 8 | 1 | 5.31% [5.25%, 6.30%] | 0.0000% | 2.19 | 0.00% | 0.00% | 0.128 | 0.057 | 2.84% | 0.00% | 52.47% | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 2 | 8 | 2 | 5.49% [5.25%, 5.54%] | 0.0000% | 2.16 | 0.00% | 0.00% | 0.137 | 0.057 | 2.84% | 0.00% | 52.00% | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 4 | 8 | 1 | 7.36% [6.81%, 10.53%] | 0.0000% | 2.54 | 0.00% | 0.00% | 0.381 | 0.057 | 2.84% | 0.00% | 54.06% | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 4 | 8 | 2 | 6.97% [6.41%, 7.34%] | 0.0000% | 2.55 | 0.00% | 0.00% | 0.385 | 0.057 | 2.84% | 0.00% | 52.85% | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 8 | 8 | 1 | 5.75% [5.37%, 6.07%] | 0.0000% | 2.36 | 0.00% | 0.00% | 0.326 | 0.057 | 2.84% | 0.00% | 51.26% | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 8 | 8 | 2 | 6.99% [6.62%, 7.84%] | 0.0000% | 2.59 | 0.00% | 0.00% | 0.288 | 0.057 | 2.84% | 0.00% | 53.11% | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |

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

The health counters move the same way, and §7.3 reads them as a band: the
two runs do not agree on the ordering of the restart share across writer
counts, so the registered rise is direction-only (run 1 monotonic on the set
arm, run 2 not).

**A second pair, at `a1982ff2`, for [#568](https://github.com/orieg/expanse/issues/568)
Step 0.** The concurrent sweep was re-taken twice with a load snapshot per
cell (foreign share ≤ 0.02 core-equivalents throughout) and health rows that
carry the attribution counters and are summed from per-thread shards. Against
the union of the `64f8a3af` pair, 24 of the 28 C1/C2 ratio cells overlap; the
four that do not are all on the set arm — `C1 set W=4` (0.247–0.270 →
0.273–0.304), `C2 set W=4 R=8 writer` (0.154–0.170 → 0.173–0.192), `C2 set W=8
R=8 reader` (0.297–0.321 → 0.258–0.279) and `C2 set W=8 R=8 writer`
(0.086–0.099 → 0.100–0.109). Within the new pair **14 of 28 cells separate**,
the widest `C2 set W=1 R=8 reader` (0.254 → 0.141); **no direction and no
verdict moved** in either pair. §7.3's table is the new pair; §7.1, §7.2, §7.4
and §7.5 keep the `64f8a3af` levels, which the new pair replicates in
direction everywhere and in level on 24 of 28 cells. Both runs are in
`results/baseline_concurrent.json` and `results/baseline_concurrent_run2.json`
with their own provenance.

What explains the spread was the open question this table left, and #568's
Step 0 has now measured the mechanism it left open — readers spending most of
their time on the writer's open tree-level bracket, the writer's coherence
cost with readers present — in
[`docs/benchmarks/concurrency/README.md`](../concurrency/README.md). The
run-to-run spread itself is still unattributed there too.
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
