# Expanse vs. Masstree: Empirical Benchmark Suite

Head-to-head evaluation of `ExpanseMap`, `ExpanseStrMap`, `SyncExpanseMap` and
`SyncExpanseStrMap` against **Masstree**
([Mao, Kohler & Morris, EuroSys 2012](https://doi.org/10.1145/2168836.2168855)),
the trie of B+-trees, reached through a C++ FFI shim over the reference
implementation. The last of the three SOTA arms
[#387](https://github.com/orieg/expanse/issues/387) filed, and the second,
independent route to the write-concurrency loss
[#692](https://github.com/orieg/expanse/issues/692) measured against HOT-ROWEX.

> **Tracking & provenance.** Delivers
> [#661](https://github.com/orieg/expanse/issues/661).
> *(measured: reference host — Intel Core i9-12900F, 8P+8E/24 threads, 30 MiB L3, Ubuntu 22.04, kernel 6.8; Masstree [`kohler/masstree-beta`](https://github.com/kohler/masstree-beta) `1119842`, MIT with a publicity clause; harness commit `2ce92b7f`; `docs/benchmarks/masstree_comparison/run.sh --concurrent`; benchmark shell pinned to CPUs 0-15 and every concurrent row records `Cpus_allowed_list 0-15`; both arms built for one ISA target — `-C target-cpu=haswell` and `-march=haswell -O3 -std=c++17 -DNDEBUG`, assertions off, superpages on, glibc 2.35 `malloc`; load average 0.92 / 0.92 / 0.93 / 0.94 / 0.96 / 1.08 / 1.02 across the single-threaded phases with the host's busy CPU at 1.0 core-equivalents between every pair of snapshots, and 1.02 at the concurrent sweep's start, 5.68 after it with 4.13 cores busy across it — its own threads, which is why it runs last; frequency driver `intel_pstate` in `powersave`, transparent huge pages `madvise`, P-cores `0-15` with SMT and E-cores `16-23` outside the pin; 15 rounds per wall-clock cell, the arm timed first alternating per round, per-arm medians reported beside a mean-of-rounds ratio with its BCa 95% bootstrap interval over 2,000 resamples, every round's samples in `rounds_raw`; `results/baseline_*.json`; gate transcript `results/validate.log`)*
>
> Pre-registration, locked constraints and every amendment:
> [`METHODOLOGY.md`](METHODOLOGY.md). Every table below is the output of
> `scripts/tables.py` over `results/`; no number in a table is typed by hand.
> Ratios are **Masstree ÷ Expanse for latency and Expanse ÷ Masstree for
> throughput, so above 1.000 always means Expanse is faster**; memory is
> deterministic and carries no interval. This is internal work; no external
> peer review has been performed on any claim here.

---

## 1. The losses first

**Write concurrency is a loss from two writers on integer keys and from one
writer on string keys, and it is wider than the pre-registration argued
for.** `SyncExpanseMap` serializes every writer on one mutex (optimistic lock
coupling, blocking by design — `AGENTS.md` §2.2); Masstree's per-node locks
admit them. With eight writers inserting 2²⁰ fresh keys into a 2²⁰ prefill
Masstree sustains 31.68 M inserts/s against Expanse's 2.60, ratio
0.082 [0.079, 0.084]; at sixteen, 0.075 [0.070, 0.079] — sixteen threads on eight physical P-cores
with SMT, where Masstree's own plateau (31.7 → 34.4 M/s) is the sibling
ceiling *(workload: `masstree_conc_map_64bit`)*. On `short` string keys the loss starts at one
writer — 0.883 [0.792, 0.992] — and reaches 0.019 [0.017, 0.022] at sixteen,
where the Expanse string writers fall to 0.52 M inserts/s *(workload:
`masstree_conc_str`)*. Expanse's aggregate insert rate does not plateau at its
single-writer rate: it **falls** as writers are added, 5.66 → 3.95 → 3.21 →
2.60 M/s on integers from one to eight writers, and 2.77 at sixteen. Which share of the fall is lock hand-off and
which is cache-line traffic is **unmeasured** — this arm carries no hardware
counters (§8.9) — and no mechanism beyond the serialization itself is
claimed. §6.1 rows 1 and 3 are **`CONFIRMED`**; the single-writer integer cell
is Expanse's, 1.134 [1.097, 1.169], a **`REFUTED`** row in Expanse's favour,
which was the direction the ROWEX arm registered and the opposite of what this
one did.

![Writer throughput vs writer count](results/chart_concurrent_writers.svg)

**Readers under any writer load go to Masstree, on both arms.** One writer
takes Expanse's eight integer readers from 142.8 to 18.7 M lookups/s while
Masstree's keep 43.2 of their 58.4 — 0.450 [0.431, 0.501]; on strings from
31.4 to 6.4 against Masstree's 26.9 — 0.228 [0.212, 0.241] *(workloads:
`masstree_conc_map_64bit`, `masstree_conc_str`)*. **`CONFIRMED`** in direction
(§6.1 row 3). The mechanism is **unmeasured**, and the health cells rule out
the obvious one: the restart share stays at 3.8–6.8% at every writer count (§7),
which cannot account for a seven-fold drop, while `sample_spins ÷ read_ops`
of 0.95–1.24 says a reader waits on the writer's open tree-level bracket about
once per lookup. Whether the remainder is that wait or coherence traffic
on the shared version line is a counter question this arm did not take (§8.9).
The writer pays too: with eight readers probing, the Expanse single writer
falls from 5.66 to 1.86 M inserts/s where Masstree's falls from 5.09 to 3.82
*(workload: `masstree_conc_map_64bit`)*.

![Reader throughput alongside writers](results/chart_concurrent_readers.svg)

**Ordered scan on string keys is a loss in every cell**, 0.548 [0.547, 0.550]
at best (`prefixed`, k=10) and 0.035 [0.035, 0.036] at worst (`counter`,
k=1000) *(workload: `masstree_str_map`)* — **`CONFIRMED`** at the high
confidence registered. As in the HOT suite this measures the shipped
`ExpanseStrMap` navigation surface, which re-descends from the root and
allocates a key per visited element, against one descent and a leaf walk; a
cursor iterator for `ExpanseStrMap` is [#722](https://github.com/orieg/expanse/issues/722).

**String insertion is Masstree's on every representable shape**, 0.420
[0.414, 0.425] on `short`, 0.441 [0.439, 0.443] on `counter`, 0.593
[0.589, 0.598] on `skewed`, 0.884 [0.877, 0.888] on `prefixed`. Only
`prefixed` was registered (**`CONFIRMED`**); `counter` was registered the other
way and is an **`UNPREDICTED LOSS`**; `short` and `skewed` were not predicted.
The §10.2 sensitivity rows say what is being measured: on a shuffled
permutation of the same keys `short` insertion is 0.969 [0.966, 0.973] and
`prefixed` 1.208 [1.204, 1.211] — sorted insertion is a B+-tree's best case
(every leaf fills, no split lands mid-leaf) and the shared generator hands
both arms the population sorted.

**Integer insertion on `random`, `sparse` and `clustered` keys is Masstree's
too, in the sorted order the suite builds in** — 0.767 [0.760, 0.773], 0.668
[0.661, 0.677], 0.972 [0.967, 0.976] — three **`UNPREDICTED LOSS`** cells
against a medium-confidence registration; `sequential` is Expanse's at 1.538
[1.519, 1.560] *(workload: `masstree_map_64bit`)*. Masstree inserts at a flat
20.7–20.8 ns whatever the distribution. On the shuffled permutation the same
`random` cell is 1.883 [1.870, 1.899] in Expanse's favour, and with Masstree's
concurrent table 1.150 [1.144, 1.154] (§10.3): the registered win exists, in
the insertion order and the configuration the pre-registration did not name.

**`counter` string lookup at N = 10⁶ is Masstree's**, 0.930 [0.927, 0.933]
at 100% hit and 0.973 [0.969, 0.978] at 50/50 — registered as a
high-confidence Expanse win at exactly this population, so two
**`UNPREDICTED LOSS`** cells with nothing to hide behind.

**Reader-only string throughput goes to Masstree**, 0.865 [0.853, 0.886] with
eight readers and no writer *(workload: `masstree_conc_str`)* — an
**`UNPREDICTED LOSS`** against the medium-confidence registration, consistent
with the single-threaded `short` lookup being only 1.25× rather than the
larger integer margins.

**Memory at the ends of the density sweep and on `prefixed` strings goes to
Masstree by a few bytes per key.** Masstree holds 22.76 B/key structurally at
every λ **in the sorted order the suite builds in** — every leaf fills to 15
keys, the B+-tree's bulk-load figure; the same keys shuffled fill 70.7% and
cost 33.10 (§6) — and 23.0–24.1 B/key on the allocator instrument outside the
quantum-dominated cells. Its per-key cost does not depend on key density. `ExpanseMap` is below that from λ = 8
(18.99) through λ = 30 (19.39), with its best at λ = 23 (16.26), and above it
at λ = 38, 46 and 61 (23.91, 24.71, 23.82) — the `LEAF_CAP` cascade
`hot_comparison` §9.4 documents. §6.2 row 1 (λ ∈ [8, 23]) is **`CONFIRMED`**;
row 2 (outside the band) is an **`UNPREDICTED LOSS`** above the cascade and
parity-by-magnitude at λ = 4 (24.05 vs 22.80). On `prefixed` strings
Masstree's 69.06 B/key against `ExpanseStrMap`'s 72.53 is an **`UNPREDICTED
LOSS`** against a medium-confidence row; §10.2 records that the registration
leaned on a shuffled-order figure (84.2 B/key structural, which the
sensitivity table reproduces at 88.52 allocator). Memory cells carry no
interval, so these labels are by magnitude, and the taxonomy is the
wall-clock one: a registered Expanse win that landed the other way is an
`UNPREDICTED LOSS`, never a `REFUTED`, which the derived tables reserve for a
registered Masstree win that Expanse took.

## 2. Where Expanse wins

**Integer point lookup, by 3.2× to 13.6× at N = 10⁶, on every distribution
and at every population.** At N = 10⁶ Masstree answers a lookup in 117–118 ns
whatever the distribution — a B+-tree descent of the same height regardless
of key structure — while Expanse answers `sparse` in 9.8 ns (13.648
[12.602, 14.908]), `sequential` in 12.4 (10.441 [9.775, 11.231]), `clustered`
in 22.1 (5.388 [5.327, 5.453]) and `random` in 37.6 (3.153 [3.111, 3.184])
*(workload: `masstree_map_64bit`)*. `sequential`, `sparse` and `random` were
registered (**`CONFIRMED`**); `clustered` was not. The 50/50 pillar follows,
3.178 [3.144, 3.212] to 8.919 [8.683, 9.137]. Why Masstree's descent costs
what it does here is unmeasured — no counter was taken — and the cross-suite
comparison the reader will want (HOT held random 1M lookup near parity) is one
this suite does not draw (§8 item 6).

![Latency at N = 1M](results/chart_latency_1m.svg)

**Ordered scan on integer keys goes to Expanse on every structured
distribution and on `random` at 1M, and to Masstree on `random` below 1M.**
Through `ExpanseMap::range()` Expanse visits an element in 1.4–11.5 ns against
Masstree's 3.7–17.5 at N = 10⁶: 1.704 [1.615, 1.810] at k = 10 on `random`,
2.618 [2.562, 2.639] at k = 1000 on `sequential`. On `random` at N = 10⁴ the
direction reverses — 0.953 [0.946, 0.969] at k = 10 and 0.599 [0.593, 0.603]
at k = 1000 — and at 10⁵ it does so from k = 100 (0.614 [0.603, 0.620]) while
k = 10 is Expanse's at 1.102 [1.086, 1.135]. §6.1 row 4 registered Masstree
for k = 10 and k = 100 on the strength of the ART and HOT results: it is
**`REFUTED`** on 21 of the 24 registered cells and **`CONFIRMED`** on `random`
at 10⁴ (k = 10, 100) and 10⁵ (k = 100); the k = 1000 cells were `not
pre-registered`. Masstree's scan is driven through its visitor interface with
a key reassembled per element; why that is cheaper than Expanse's iterator on
a small random population and dearer everywhere else is unmeasured. The k = 10
cells are where the §10.6 start count told most: with a hundred times more
distinct starts per round the per-element cost rose on both arms — on `random`
at 10⁶ Masstree 14.5 → 17.4 ns and Expanse 9.4 → 11.1 against the first run
*(measured: reference host, harness commit `82966aae`, artifacts at `a8da40e3`
in history)* — which is consistent with a colder descent per start and is not
measured further.

**Reader-only integer throughput**, 2.420 [2.341, 2.478] with eight readers —
**`CONFIRMED`**.

**Memory on structured integer keys and in the density band.** `sequential`
and `clustered` at N = 10⁶: 8.91 and 8.97 B/key against Masstree's flat 23.08
(**`CONFIRMED`**, the low-information cell §6.2 said it would be); `sparse`
16.41 against 23.08 (**`CONFIRMED`**); `random` at λ = 15 and 23: 16.72 and
16.26 against 23.48 and 23.66 (**`CONFIRMED`**).

**String point lookup on `short`, `skewed` and, narrowly, `prefixed`.**
`short` 1.248 [1.244, 1.252] (**`CONFIRMED`**), `skewed` 1.408 [1.402, 1.413]
(`not pre-registered`), and `prefixed` 1.062 [1.059, 1.065] — the issue's
stated expectation that Expanse loses on long shared-prefix keys is
**`REFUTED`**, narrowly, at the low confidence it was registered: both
structures descend the same twelve 8-byte slices (`masstree_envelope.layers_for_shared_prefix(96)`),
and the interval sits just above parity. The 50/50 `prefixed` cell is 1.153
[1.141, 1.157].

**`counter` and `short`-key memory** *(workload: `masstree_str_map`)*. `counter`: 20.53 against 25.18 B/key
(**`CONFIRMED`**). And the one string memory cell that was registered as a
loss landed as one: `short` at 33.91 against 69.17 (**`CONFIRMED`** for
Masstree), `skewed` 46.63 against 47.74 — a 1.1 B/key margin that the
pre-registration counted as a Masstree win and this suite reports as
parity-by-magnitude.

## 3. What the census says, and what it does not

Two instruments per cell, never mixed (§3.3). The allocator column is what
the process holds; on Masstree it is quantized to the 2 MiB pool slab, so at
λ = 1 and λ = 2 and on every string cell below N ≈ 150k the figure is mostly
slab and is flagged `QUANTUM_DOMINATED`. Where it is not flagged, the measured
slack above Masstree's own node census is 0.2–3.6 B/key. Masstree's structural
figure is 22.76 B/key on every integer cell in the sorted build order: 66,667
leaves at 100% fill plus 4,448 internodes for 10⁶ keys, exactly what `masstree_envelope.structural_bytes`
gives for those counts — and 33.10 B/key at 70.7% fill on the shuffled
permutation (§10.2). The RCU settle step (§10.4) reclaimed 12.2 B/key of
superseded suffix bags on `prefixed` (81.20 → 69.00) and nothing on integer
keys, which allocate no bags.

![Memory across expanse occupancy](results/chart_memory_curve.svg)

#### Memory, integer map: two instruments per cell, bytes per key

`allocator` is what the process holds from the C allocator after a build-only population, one instrument for both arms; on Masstree it is quantized to the 2 MiB pool slab and a cell whose measured slack exceeds 25% of its structural bytes is flagged `QUANTUM_DOMINATED` (§3.3). `structural` is Masstree's own `json_stats` node census; `mem_used` is Expanse's own accounting. The engine columns are never mixed with the allocator columns in one ratio.

| Distribution | λ | N | Masstree allocator, settled (unsettled) | Expanse allocator | Masstree structural | Expanse `mem_used` | Masstree slack | slabs | leaf fill | flag |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `random` | 1 | 65,536 | 32.19 (32.19) | 23.88 | 22.78 | 22.66 | 9.41 | 1 | 1.000 | `QUANTUM_DOMINATED` |
| `random` | 2 | 131,072 | 32.09 (32.09) | 24.81 | 22.77 | 24.09 | 9.33 | 2 | 1.000 | `QUANTUM_DOMINATED` |
| `random` | 4 | 262,144 | 24.05 (24.05) | 22.80 | 22.76 | 22.28 | 1.29 | 3 | 1.000 | `ok` |
| `random` | 8 | 524,288 | 24.02 (24.02) | 18.99 | 22.76 | 19.05 | 1.27 | 6 | 1.000 | `ok` |
| `random` | 15 | 983,040 | 23.48 (23.48) | 16.72 | 22.76 | 16.75 | 0.72 | 11 | 1.000 | `ok` |
| `random` | 23 | 1,507,328 | 23.66 (23.66) | 16.26 | 22.76 | 16.20 | 0.90 | 17 | 1.000 | `ok` |
| `random` | 30 | 1,966,080 | 23.47 (23.47) | 19.39 | 22.76 | 19.05 | 0.72 | 22 | 1.000 | `ok` |
| `random` | 38 | 2,490,368 | 23.58 (23.58) | 23.91 | 22.76 | 23.24 | 0.83 | 28 | 1.000 | `ok` |
| `random` | 46 | 3,014,656 | 22.96 (22.96) | 24.71 | 22.76 | 24.00 | 0.20 | 33 | 1.000 | `ok` |
| `random` | 61 | 3,997,696 | 23.09 (23.09) | 23.83 | 22.76 | 23.17 | 0.33 | 44 | 1.000 | `ok` |
| `clustered` | — | 1,000,000 | 23.08 (23.08) | 8.97 | 22.76 | 8.61 | 0.32 | 11 | 1.000 | `ok` |
| `sequential` | — | 1,000,000 | 23.08 (23.08) | 8.91 | 22.76 | 8.56 | 0.32 | 11 | 1.000 | `ok` |
| `sparse` | — | 1,000,000 | 23.08 (23.08) | 16.41 | 22.76 | 16.31 | 0.32 | 11 | 1.000 | `ok` |

#### Memory, string map: two instruments per cell, bytes per key

Both sides copy key bytes into their own nodes, so the index column is the ownership column on both (§4). Columns as the integer table.

| Shape | N | mean len | Masstree allocator, settled (unsettled) | Expanse allocator | Masstree structural | Expanse `mem_used` | Masstree slack | layers | flag |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `beyond` | 1,000,000 | 272.0 | withheld (1,000,000 keys > 255 B) | 71.84 | withheld (1,000,000 keys > 255 B) | 54.78 | withheld (1,000,000 keys > 255 B) | — | `NOT_REPRESENTABLE_MASSTREE` |
| `counter` | 1,000,000 | 12.0 | 25.18 (25.18) | 20.53 | 22.82 | 19.56 | 2.36 | 100 | `ok` |
| `prefixed` | 1,000,000 | 120.0 | 69.06 (80.07) | 72.54 | 65.43 | 62.77 | 3.63 | 12 | `ok` |
| `short` | 1,000,000 | 12.0 | 33.91 (34.02) | 69.17 | 32.02 | 50.77 | 1.89 | 0 | `ok` |
| `skewed` | 998,150 | 14.3 | 46.63 (49.26) | 47.74 | 43.24 | 41.19 | 3.38 | 0 | `ok` |

#### String map, allocator column across the population sweep (Masstree / Expanse B/key)

| N | `beyond` | `counter` | `prefixed` | `short` | `skewed` |
|---:|---:|---:|---:|---:|---:|
| 1,000 | — / 109.8 | 2109.6† / 91.1 | 4241.4† / 110.1 | 6304.4† / 106.4 | 6315.7† / 84.8 |
| 2,000 | — / 91.5 | 1054.8† / 56.0 | 2138.0† / 91.2 | 3152.1† / 89.7 | 3164.7† / 67.2 |
| 5,000 | — / 84.5 | 421.9† / 34.1 | 876.8† / 85.3 | 1261.3† / 80.8 | 1273.8† / 59.1 |
| 10,000 | — / 78.4 | 211.0† / 27.5 | 456.2† / 79.6 | 630.9† / 75.6 | 643.2† / 54.0 |
| 20,000 | — / 73.3 | 105.5† / 24.0 | 246.0† / 74.2 | 315.6† / 70.5 | 327.6† / 48.9 |
| 50,000 | — / 67.3 | 42.2† / 21.8 | 119.9† / 68.0 | 126.4† / 64.6 | 138.9† / 42.8 |
| 100,000 | — / 66.6 | 42.1† / 21.1 | 98.8† / 67.5 | 84.3† / 64.0 | 97.1† / 42.5 |
| 125,000 | — / 70.7 | 33.7† / 21.0 | 86.2† / 71.7 | 67.5† / 68.0 | 80.3† / 46.7 |
| 150,000 | — / 73.8 | 28.0 / 20.9 | 91.7† / 74.8 | 70.3† / 71.2 | 69.1† / 49.8 |
| 200,000 | — / 74.5 | 42.0† / 20.8 | 77.8 / 75.3 | 52.8† / 71.8 | 65.5† / 50.4 |
| 500,000 | — / 74.0 | 29.4† / 20.6 | 73.5 / 74.9 | 38.1 / 71.3 | 50.8 / 49.8 |
| 1,000,000 | — / 71.8 | 25.2 / 20.5 | 69.1 / 72.5 | 33.9 / 69.2 | 46.6 / 47.7 |

† `QUANTUM_DOMINATED`: the allocator figure is mostly the 2 MiB slab, not the index (§3.3).

![String memory across population](results/chart_string_memory_sweep.svg)

## 4. Latency tables, integer keys

#### Point lookup, 100% hit, integer keys (N = 1,000,000)

| Distribution | λ | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---|
| `clustered` | — | 116.98 | 22.10 | 5.388 [5.327, 5.453] | Expanse — `not pre-registered` |
| `random` | 15.3 | 118.03 | 37.58 | 3.153 [3.111, 3.184] | Expanse — `CONFIRMED` |
| `sequential` | — | 117.70 | 12.44 | 10.441 [9.775, 11.231] | Expanse — `CONFIRMED` |
| `sparse` | — | 118.03 | 9.83 | 13.648 [12.602, 14.908] | Expanse — `CONFIRMED` |

#### Point lookup, 50% hit / 50% rejection-sampled miss, integer keys (N = 1,000,000)

| Distribution | λ | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---|
| `clustered` | — | 98.77 | 15.82 | 6.303 [6.254, 6.355] | Expanse — `not pre-registered` |
| `random` | 15.3 | 120.80 | 38.45 | 3.178 [3.144, 3.212] | Expanse — `not pre-registered` |
| `sequential` | — | 64.87 | 9.12 | 7.315 [7.069, 7.521] | Expanse — `CONFIRMED` |
| `sparse` | — | 65.18 | 7.49 | 8.919 [8.683, 9.137] | Expanse — `CONFIRMED` |

#### Insertion into a cold structure, integer keys (N = 1,000,000)

| Distribution | λ | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---|
| `clustered` | — | 20.66 | 21.22 | 0.972 [0.967, 0.976] | Masstree — **`UNPREDICTED LOSS`** |
| `random` | 15.3 | 20.77 | 27.10 | 0.767 [0.760, 0.773] | Masstree — **`UNPREDICTED LOSS`** |
| `sequential` | — | 20.72 | 13.40 | 1.538 [1.519, 1.560] | Expanse — `CONFIRMED` |
| `sparse` | — | 20.81 | 30.92 | 0.668 [0.661, 0.677] | Masstree — **`UNPREDICTED LOSS`** |

#### Ordered range scan, integer keys (N = 1,000,000; Masstree ÷ Expanse per visited element)

| Distribution | k=10 | k=100 | k=1000 |
|---|---:|---:|---:|
| `sequential` | 2.388 [2.285, 2.507] · **`REFUTED`** | 2.324 [2.235, 2.401] · **`REFUTED`** | 2.618 [2.562, 2.639] · `not pre-registered` |
| `clustered` | 2.471 [2.363, 2.588] · **`REFUTED`** | 2.265 [2.169, 2.353] · **`REFUTED`** | 2.531 [2.491, 2.555] · `not pre-registered` |
| `sparse` | 1.626 [1.525, 1.744] · **`REFUTED`** | 1.256 [1.225, 1.285] · **`REFUTED`** | 1.115 [1.108, 1.124] · `not pre-registered` |
| `random` | 1.704 [1.615, 1.810] · **`REFUTED`** | 1.555 [1.472, 1.651] · **`REFUTED`** | 1.619 [1.579, 1.643] · `not pre-registered` |

## 5. Latency tables, string keys

![String latency at N = 1M](results/chart_string_latency.svg)

#### Point lookup, 100% hit, string keys (N = 1,000,000)

| Shape | N held | mean len | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---:|---|
| `beyond` | 1,000,000 | 272.0 | withheld (1,000,000 keys > 255 B) | 364.77 | — | no Masstree cell (§3.4) |
| `counter` | 1,000,000 | 12.0 | 141.75 | 152.80 | 0.930 [0.927, 0.933] | Masstree — **`UNPREDICTED LOSS`** |
| `prefixed` | 1,000,000 | 120.0 | 308.10 | 290.16 | 1.062 [1.059, 1.065] | Expanse — **`REFUTED`** |
| `short` | 1,000,000 | 12.0 | 170.36 | 136.81 | 1.248 [1.244, 1.252] | Expanse — `CONFIRMED` |
| `skewed` | 998,150 | 14.3 | 202.52 | 144.08 | 1.408 [1.402, 1.413] | Expanse — `not pre-registered` |

#### Point lookup, 50% hit / 50% rejection-sampled miss, string keys (N = 1,000,000)

| Shape | N held | mean len | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---:|---|
| `beyond` | 1,000,000 | 272.0 | withheld (1,000,000 keys > 255 B) | 321.07 | — | no Masstree cell (§3.4) |
| `counter` | 1,000,000 | 12.0 | 79.02 | 81.20 | 0.973 [0.969, 0.978] | Masstree — `not pre-registered` |
| `prefixed` | 1,000,000 | 120.0 | 278.19 | 241.35 | 1.153 [1.141, 1.157] | Expanse — **`REFUTED`** |
| `short` | 1,000,000 | 12.0 | 161.25 | 112.99 | 1.426 [1.422, 1.430] | Expanse — `not pre-registered` |
| `skewed` | 998,150 | 14.3 | 182.24 | 123.14 | 1.487 [1.481, 1.493] | Expanse — `not pre-registered` |

#### Insertion into a cold structure, string keys (N = 1,000,000)

| Shape | N held | mean len | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---:|---|
| `beyond` | 1,000,000 | 272.0 | withheld (1,000,000 keys > 255 B) | 282.73 | — | no Masstree cell (§3.4) |
| `counter` | 1,000,000 | 12.0 | 24.59 | 55.90 | 0.441 [0.439, 0.443] | Masstree — **`UNPREDICTED LOSS`** |
| `prefixed` | 1,000,000 | 120.0 | 166.16 | 187.05 | 0.884 [0.877, 0.888] | Masstree — `CONFIRMED` |
| `short` | 1,000,000 | 12.0 | 52.22 | 122.55 | 0.420 [0.414, 0.425] | Masstree — `not pre-registered` |
| `skewed` | 998,150 | 14.3 | 66.29 | 111.69 | 0.593 [0.589, 0.598] | Masstree — `not pre-registered` |

#### Ordered range scan, string keys (N = 1,000,000; Masstree ÷ Expanse per visited element)

| Shape | k=10 | k=100 | k=1000 |
|---|---:|---:|---:|
| `short` | 0.250 [0.249, 0.251] | 0.115 [0.114, 0.116] | 0.097 [0.096, 0.097] |
| `counter` | 0.180 [0.179, 0.181] | 0.053 [0.052, 0.053] | 0.035 [0.035, 0.036] |
| `prefixed` | 0.548 [0.547, 0.550] | 0.123 [0.121, 0.125] | 0.069 [0.068, 0.069] |
| `skewed` | 0.253 [0.252, 0.254] | 0.105 [0.105, 0.107] | 0.083 [0.082, 0.083] |
| `beyond` | Expanse 306 ns; Masstree withheld | Expanse 276 ns; Masstree withheld | Expanse 273 ns; Masstree withheld |

`beyond` (272-byte keys) fails the §3.4 predicate for its whole population:
Masstree's declared contract is `MASSTREE_MAXKEYLEN = 255`, and the validation
gate records that the shim refuses every such key at the call. The Expanse
figures on that shape are published alone.

## 6. Sensitivity: insertion order and table configuration

#### Sensitivity (§10.2 insertion order, §10.3 table configuration) — both arms, same population

Sorted / single is the order the shared generator produces and the table configuration §10.3 assigns, which every cell above was built in. Shuffled is a Fisher–Yates permutation of the same keys: Masstree's leaf fill, footprint and insertion cost depend on the order, and so do Expanse's insertion cost and allocator footprint; Expanse's own node census (`mem_used`) is the one order-invariant figure. Concurrent is Masstree's fenced, spin-locked node version, the configuration the MC cells use, driven single-threaded here to show the protocol's own cost. Ratios are Masstree ÷ Expanse; no verdict is given against §6.

| Arm | Shape | Order | Table | N | Masstree allocator, settled (unsettled) | Masstree structural | leaf fill | Expanse allocator | Expanse `mem_used` | lookup_hit ratio [BCa 95%] | insert ratio [BCa 95%] |
|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| map | `random` | sorted | single | 1,000,000 | 23.08 (23.08) | 22.76 | 1.000 | 16.67 | 16.70 | 3.242 [3.208, 3.277] | 0.760 [0.752, 0.769] |
| map | `random` | sorted | concurrent | 1,000,000 | 23.08 (23.08) | 22.76 | 1.000 | 16.67 | 16.70 | 3.333 [3.306, 3.366] | 1.150 [1.144, 1.154] |
| map | `random` | shuffled | single | 1,000,000 | 33.57 (33.57) | 33.10 | 0.707 | 23.63 | 16.70 | 3.296 [3.261, 3.333] | 1.883 [1.870, 1.899] |
| str | `prefixed` | sorted | single | 1,000,000 | 69.00 (80.96) | 65.43 | 1.000 | 72.49 | 62.77 | 1.067 [1.060, 1.070] | 0.880 [0.876, 0.884] |
| str | `prefixed` | shuffled | single | 1,000,000 | 88.51 (92.92) | 84.20 | 0.706 | 74.79 | 62.77 | 1.066 [1.065, 1.069] | 1.208 [1.204, 1.211] |
| str | `short` | sorted | single | 1,000,000 | 33.91 (34.02) | 32.02 | 1.000 | 69.17 | 50.77 | 1.243 [1.239, 1.246] | 0.420 [0.415, 0.425] |
| str | `short` | sorted | concurrent | 1,000,000 | 33.91 (34.02) | 32.02 | 1.000 | 69.17 | 50.77 | 1.409 [1.405, 1.414] | 0.525 [0.515, 0.534] |
| str | `short` | shuffled | single | 1,000,000 | 50.62 (51.05) | 47.99 | 0.707 | 71.99 | 50.77 | 1.422 [1.420, 1.426] | 0.969 [0.966, 0.973] |

The shuffled rows are the regime the Step 0 gate measured and the §6
predictions leaned on; the sorted rows are the suite's cells in the shared
generator's order, with the table configuration §10.3 assigns. The
difference between them is the finding, and it cuts both ways: **Masstree's
insertion cost (20.8 → 122.7 ns on `random`), leaf fill (1.000 → 0.707) and
footprint (23.08 → 33.57 B/key) depend on insertion order — and so do
Expanse's insertion cost (27.4 → 65.2 ns) and its allocator footprint (16.67
→ 23.63 B/key), while Expanse's own node census (`mem_used`, 16.70 in both
orders) is the one figure that is order-invariant.** The 7 B/key the allocator
holds beyond `mem_used` on the shuffled build is capacity the engine
instrument does not see; its cause is unmeasured. Lookup ratios barely move
with order (3.242 sorted, 3.296 shuffled on `random`), but every latency
verdict above is a sorted-order verdict all the same. The concurrent-table rows show the protocol's own single-threaded
cost: on `random` integer keys the insertion ratio moves from 0.760
[0.752, 0.769] with the single-threaded table to 1.150 [1.144, 1.154] with the
concurrent one, and the lookup ratio from 3.242 [3.208, 3.277] to 3.333
[3.306, 3.366] — which is why the single-threaded pairings use the
single-threaded configuration (§10.3).

## 7. The concurrent arm

#### MC1 — `u64` keys, Masstree vs `SyncExpanseMap`

**C1 — writer throughput as writer count scales** (W writers insert 2²⁰ fresh keys into a 2²⁰ prefill; fixed work; Expanse ÷ Masstree)

| W | Masstree M/s | Expanse M/s | ratio [BCa 95%] | verdict |
|--:|---:|---:|---|---|
| 1 | 5.09 | 5.66 | 1.134 [1.097, 1.169] | Expanse — **`REFUTED`** (in Expanse's favour) |
| 2 | 9.80 | 3.95 | 0.407 [0.397, 0.418] | Masstree — `CONFIRMED` |
| 4 | 18.37 | 3.21 | 0.174 [0.170, 0.177] | Masstree — `CONFIRMED` |
| 8 | 31.68 | 2.60 | 0.082 [0.079, 0.084] | Masstree — `CONFIRMED` |
| 16 | 34.40 | 2.77 | 0.075 [0.070, 0.079] | Masstree — `not pre-registered` |

**C2 — reader throughput alongside writers** (8 readers probe 50/50 while W writers insert; W = 0 is the reader-only reference; the reader window is the writers' fixed work, so the two arms' windows differ in length by the writer ratio and the population grows at different rates inside them)

| W | Masstree readers M/s | Expanse readers M/s | ratio [BCa 95%] | verdict | Masstree writers M/s | Expanse writers M/s | writer ratio |
|--:|---:|---:|---|---|---:|---:|---|
| 0 | 58.42 | 142.84 | 2.420 [2.341, 2.478] | Expanse — `CONFIRMED` | — | — | — |
| 1 | 43.16 | 18.68 | 0.450 [0.431, 0.501] | Masstree — `CONFIRMED` | 3.82 | 1.86 | 0.488 [0.461, 0.510] |
| 2 | 40.35 | 31.00 | 0.748 [0.715, 0.772] | Masstree — `CONFIRMED` | 7.30 | 1.51 | 0.211 [0.205, 0.219] |
| 4 | 37.56 | 17.45 | 0.472 [0.455, 0.494] | Masstree — `CONFIRMED` | 13.75 | 1.87 | 0.136 [0.131, 0.140] |
| 8 | 30.84 | 23.97 | 0.775 [0.745, 0.805] | Masstree — `CONFIRMED` | 24.64 | 1.52 | 0.068 [0.063, 0.074] |

#### MC2 — `short` string keys, Masstree vs `SyncExpanseStrMap`

**C1 — writer throughput as writer count scales** (W writers insert 2²⁰ fresh keys into a 2²⁰ prefill; fixed work; Expanse ÷ Masstree)

| W | Masstree M/s | Expanse M/s | ratio [BCa 95%] | verdict |
|--:|---:|---:|---|---|
| 1 | 4.20 | 3.54 | 0.883 [0.792, 0.992] | Masstree — `CONFIRMED` |
| 2 | 8.00 | 2.51 | 0.335 [0.308, 0.364] | Masstree — `CONFIRMED` |
| 4 | 14.96 | 2.26 | 0.175 [0.158, 0.197] | Masstree — `CONFIRMED` |
| 8 | 22.07 | 2.00 | 0.094 [0.081, 0.108] | Masstree — `CONFIRMED` |
| 16 | 30.55 | 0.52 | 0.019 [0.017, 0.022] | Masstree — `not pre-registered` |

**C2 — reader throughput alongside writers** (8 readers probe 50/50 while W writers insert; W = 0 is the reader-only reference; the reader window is the writers' fixed work, so the two arms' windows differ in length by the writer ratio and the population grows at different rates inside them)

| W | Masstree readers M/s | Expanse readers M/s | ratio [BCa 95%] | verdict | Masstree writers M/s | Expanse writers M/s | writer ratio |
|--:|---:|---:|---|---|---:|---:|---|
| 0 | 36.89 | 31.37 | 0.865 [0.853, 0.886] | Masstree — **`UNPREDICTED LOSS`** | — | — | — |
| 1 | 26.87 | 6.40 | 0.228 [0.212, 0.241] | Masstree — `CONFIRMED` | 2.86 | 1.58 | 0.603 [0.547, 0.682] |
| 2 | 24.91 | 6.54 | 0.279 [0.252, 0.315] | Masstree — `CONFIRMED` | 4.63 | 1.49 | 0.310 [0.281, 0.342] |
| 4 | 22.75 | 7.67 | 0.354 [0.333, 0.378] | Masstree — `CONFIRMED` | 7.49 | 1.27 | 0.177 [0.157, 0.205] |
| 8 | 18.84 | 5.84 | 0.340 [0.317, 0.367] | Masstree — `CONFIRMED` | 13.42 | 1.42 | 0.113 [0.104, 0.128] |

#### H — protocol health, Expanse side only (occ-stats build; event ratios, never a timing)

| Arm | W | R | restart share, median [min, max] | fallback share, median | `sample_spins` ÷ `read_ops` (medians) | §6.3 |
|---|--:|--:|---|---|---:|---|
| map | 1 | 8 | 3.75% [3.67%, 3.95%] | 0.0000% | 0.95 | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 2 | 8 | 6.84% [5.61%, 9.21%] | 0.0000% | 1.24 | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 4 | 8 | 4.90% [4.54%, 6.28%] | 0.0000% | 1.04 | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 8 | 8 | 6.02% [5.95%, 6.07%] | 0.0000% | 1.20 | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| str | 1 | 8 | `NOT_INSTRUMENTED` (§10.5) | `NOT_INSTRUMENTED`; `read_fallbacks` = 0 absolute | — | not evaluable |
| str | 2 | 8 | `NOT_INSTRUMENTED` (§10.5) | `NOT_INSTRUMENTED`; `read_fallbacks` = 0 absolute | — | not evaluable |
| str | 4 | 8 | `NOT_INSTRUMENTED` (§10.5) | `NOT_INSTRUMENTED`; `read_fallbacks` = 0 absolute | — | not evaluable |
| str | 8 | 8 | `NOT_INSTRUMENTED` (§10.5) | `NOT_INSTRUMENTED`; `read_fallbacks` = 0 absolute | — | not evaluable |

The MC1 health rows land where `hot_comparison` §7.3's did on the same
construction: restart share 3.8–6.8% **without rising monotonically with
writer count** (3.75% at W = 1, then 6.84%, 4.90%, 6.02%) — the first half of
§6.3 registered a rise and is `REFUTED` — and zero reads took
the writer mutex at any writer count. That zero is not a finding about the
protocol: a fallback needs 64 consecutive failed walks, and at these bracket
lengths the probability of one is negligible by construction, so the second
half of §6.3 is `PASS_categorical_by_design` and a health falsifier that can
fire — reader nanoseconds per probe under a writer against alone, which moved
seven-fold here — is what a future arm should register. The health build
itself perturbs what it counts: every restart and spin is a `fetch_add` on
one shared counter line across nine threads (#721 scopes per-thread counters). MC2's
rows are `NOT_INSTRUMENTED`: `StrReader::get` counts fallbacks only (§10.5) —
the zero fallbacks are real, the restart share does not exist. Instrumenting
the string reader is [#721](https://github.com/orieg/expanse/issues/721).

#### M — build-only single-writer census, Masstree vs `SyncExpanseMap` (B/key)

| λ | N | Masstree allocator | SyncExpanseMap allocator | Masstree structural | Expanse `mem_used` | flag |
|---:|---:|---:|---:|---:|---:|---|
| 1 | 65,536 | 32.19 | 24.26 | 22.78 | 22.66 | `QUANTUM_DOMINATED` |
| 2 | 131,072 | 32.09 | 24.71 | 22.77 | 24.09 | `QUANTUM_DOMINATED` |
| 4 | 262,144 | 24.05 | 22.95 | 22.76 | 22.28 | `ok` |
| 8 | 524,288 | 24.02 | 19.34 | 22.76 | 19.05 | `ok` |
| 15 | 983,040 | 23.48 | 16.80 | 22.76 | 16.75 | `ok` |
| 23 | 1,507,328 | 23.66 | 16.36 | 22.76 | 16.20 | `ok` |
| 30 | 1,966,080 | 23.47 | 20.34 | 22.76 | 19.05 | `ok` |
| 38 | 2,490,368 | 23.58 | 25.90 | 22.76 | 23.24 | `ok` |
| 46 | 3,014,656 | 22.96 | 26.78 | 22.76 | 24.00 | `ok` |
| 61 | 3,997,696 | 23.09 | 25.60 | 22.76 | 23.17 | `ok` |

**The Expanse column beside #692's.** MC1's cells are the construction of
`hot_comparison` §11.4 — same generator, seeds, prefill, fresh-key stream and
thread placement — so the `SyncExpanseMap` column here is a second measurement
of #692's, on another day, in another process, and at a different engine
commit (`2ce92b7f` against `5232af74`): single writer 5.66 M/s here against
5.22 there, eight writers 2.60 against 2.59, sixteen 2.77 against 2.64, eight
readers alone 142.8 against 126.6 *(workloads differ:
`masstree_conc_map_64bit` vs `hot_rowex_map_64bit`; identical construction,
not a §8.4 paired claim)*. No tolerance was registered for this comparison, so
it carries **no replication verdict**: the direction and the shape of the fall
agree, the levels differ by 0.4–13%, and whether that spread is the instrument
or the engine commits between the two runs is not measured here.

**Between-run spread of this suite's own cells.** This arm has been run twice
on the reference host, at harness commit `82966aae` *(artifacts at `a8da40e3`
in history)* and at `2ce92b7f` (above); between them only the census shim's
free path changed in the concurrent binary (§10.6). The C2 string W = 1 reader
cell moved from 0.114 [0.095, 0.129] to 0.228 [0.212, 0.241] and the integer
W = 4 reader cell from 0.697 [0.683, 0.710] to 0.472 [0.455, 0.494]; neither
pair of intervals overlaps. For the C2 cells the between-run spread therefore
exceeds the within-run interval, and the levels above are one run's, not a
settled figure; the direction held in every concurrent cell across both runs,
and no single-threaded cell's interval moved past parity except the three
that had straddled it.

## 8. Scorecard against the pre-registration

#### Scorecard (wall-clock cells with a Masstree column)

| | Count |
|---|---:|
| Expanse wins (CI excludes parity) | 81 |
| Masstree wins (CI excludes parity) | 89 |
| `BOUNDARY_RESULT` | 2 |
| Masstree column withheld (§3.4, `beyond`) | 18 |

| Label | Cells |
|---|---:|
| Masstree — `CONFIRMED` | 65 |
| Expanse — `not pre-registered` | 32 |
| Expanse — **`REFUTED`** | 27 |
| Expanse — `CONFIRMED` | 21 |
| Masstree — `not pre-registered` | 13 |
| Masstree — **`UNPREDICTED LOSS`** | 11 |
| `BOUNDARY_RESULT` | 2 |
| Expanse — **`REFUTED`** (in Expanse's favour) | 1 |

| Registered (§6) | Outcome |
|---|---|
| Masstree wins C1 at W ≥ 2, both arms (high) | **CONFIRMED** on every cell |
| Masstree wins or `BOUNDARY_RESULT` at W = 1 (medium-low) | **CONFIRMED** on strings (0.883); **REFUTED in Expanse's favour** on integers (1.134) |
| Masstree wins readers under writers (medium-high) | **CONFIRMED** on every cell |
| Masstree wins integer scan at k = 10, 100 (medium-high) | **REFUTED** on 21 of 24 cells; **CONFIRMED** on `random` at 10⁴ (k = 10, 100) and 10⁵ (k = 100) |
| Masstree wins string scan, every k (high) | **CONFIRMED** on every cell |
| Masstree wins `prefixed` lookup and insert (low) | insert **CONFIRMED**; lookup **REFUTED**, narrowly (1.062) |
| Masstree wins `short` / `skewed` index memory (medium) | **CONFIRMED**; `skewed` by 1.1 B/key |
| Expanse wins `random` memory, λ ∈ [8, 23] (high) | **CONFIRMED** |
| Expanse wins `random` memory outside the band (medium) | **UNPREDICTED LOSS** at λ ≥ 38 (by magnitude, no interval); parity-by-magnitude at λ = 4; wins at λ = 30 |
| Expanse wins `sequential` / `clustered` / `sparse` memory (high / medium) | **CONFIRMED** |
| Expanse wins integer lookup on `sequential`, `sparse`, `random` (medium-high / medium) | **CONFIRMED**, 3.2×–13.6× at 10⁶ |
| Expanse wins integer insert (medium) | **CONFIRMED** on `sequential`; **UNPREDICTED LOSS** on `random`, `sparse`, `clustered` in sorted order |
| Expanse wins `counter` lookup and insert at 10⁶ (high) | **UNPREDICTED LOSS** on all three cells (100%-hit lookup, 50/50 lookup, insert) |
| Expanse wins `short` 100%-hit lookup (low-medium) | **CONFIRMED** |
| Expanse wins `counter` / `prefixed` index memory (medium) | `counter` **CONFIRMED**; `prefixed` **UNPREDICTED LOSS** (by magnitude) |
| Expanse wins reader-only C2 (medium) | **CONFIRMED** on integers; **UNPREDICTED LOSS** on strings |
| H: restart share rises with W; fallback share < 1% at W ≤ 8 (§6.3) | restart share **did not rise monotonically** (3.75% at W = 1, then 6.84%, 4.90%, 6.02%) — that half **REFUTED** on MC1; fallback 0% — `PASS_categorical_by_design`, since a fallback needs 64 consecutive failed walks and cannot occur at these bracket lengths; not evaluable on MC2 (§10.5) |

Eleven `UNPREDICTED LOSS` cells (registered Expanse wins that Masstree took)
and 28 `REFUTED` cells (registered Masstree wins that Expanse took — every
`REFUTED` in the derived tables is in Expanse's favour by construction)
against 86 `CONFIRMED`. **Insertion order is the one cause that was
measured**, and it is measured for one cell: the `random` integer insert
that is an `UNPREDICTED LOSS` sorted (0.767) and an Expanse win shuffled
(1.883). The registration was informed by a shuffled-order Step 0 build and
the suite builds sorted, a B+-tree's best case (§10.2); whether the same
mechanism explains the `sparse` and `clustered` inserts is plausible and
unmeasured, and it does not explain the string lookup or reader-only
surprises at all, since lookups do not depend on the order keys arrived in.

## 9. Claims this suite may and may not carry

Per `METHODOLOGY.md` §8: one Masstree commit built as documented on glibc
`malloc` with superpages on, x86-64 with AVX2/BMI2, integer keys over the
full 64-bit domain and string keys of at most 255 bytes, insert and
point-lookup concurrency on up to 16 threads of 8 physical P-cores, no
deletion under concurrency, no cross-suite ratio, no peer review. The
insertion-order and table-configuration rows are sensitivity disclosures, not
cells with verdicts.

## 10. Reproduction

```bash
git submodule update --init --depth 1 third_party/hot third_party/masstree
docs/benchmarks/masstree_comparison/run.sh --concurrent        # everything, concurrent sweep last
docs/benchmarks/masstree_comparison/run.sh --quick             # smoke, results/quick/
python3 docs/benchmarks/masstree_comparison/scripts/tables.py  # README tables from results/
python3 scripts/check_readme_tables.py --write                 # splice them into this README
python3 docs/benchmarks/masstree_comparison/scripts/generate_charts.py
```

**Tables reach this README only through `scripts/check_readme_tables.py`.** It
re-runs the generator and fails when a row here is not the row `results/` now
produces, so a re-measurement cannot leave a stale cell behind; `--write`
performs the splice, and it rewrites table rows only, never the prose around
them — a splice that reached prose is what reverted a corrected paragraph
during this suite's own re-measurement (#736).

The runner takes the host-wide benchmark lock and the P-core pin, runs the
validation gate (`masstree_validate`, 39 deterministic checks) first and
fatally, then one process per cell. Requires an x86-64 host with AVX2 and BMI2
(both arms are bound to one ISA target, §3.5), a C++17 toolchain, and the
rustup toolchain on `PATH` — the crate is edition 2024. Masstree is compiled
without autoconf from `crates/expanse-hot-bench/cpp/masstree_config/config.h`.
