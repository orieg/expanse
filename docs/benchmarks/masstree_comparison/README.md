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
> *(measured: reference host — Intel Core i9-12900F, 8P+8E/24 threads, 30 MiB L3, Ubuntu 22.04, kernel 6.8; Masstree [`kohler/masstree-beta`](https://github.com/kohler/masstree-beta) `1119842`, MIT with a publicity clause; single-threaded phases re-measured at commit `41dc7bfd` for [#723](https://github.com/orieg/expanse/issues/723) with `docs/benchmarks/masstree_comparison/run.sh`; **the concurrent phases (§5, MC1/MC2/H/M) were not re-run and remain at harness commit `2ce92b7f`** — see the note below §5; benchmark shell pinned to CPUs 0-15 and every concurrent row records `Cpus_allowed_list 0-15`; both arms built for one ISA target — `-C target-cpu=haswell` and `-march=haswell -O3 -std=c++17 -DNDEBUG`, assertions off, superpages on, glibc 2.35 `malloc`; load average 0.32 / 0.32 / 0.37 / 0.47 / 0.65 / 0.99 / 1.00 across the re-measured single-threaded phases with the host's busy CPU at 1.0-1.2 core-equivalents between every pair of snapshots — the benchmark and nothing else, and 1.02 at the concurrent sweep's start, 5.68 after it with 4.13 cores busy across it — its own threads, which is why it runs last; frequency driver `intel_pstate` in `powersave`, transparent huge pages `madvise`, P-cores `0-15` with SMT and E-cores `16-23` outside the pin; 15 rounds per wall-clock cell, the arm timed first alternating per round, per-arm medians reported beside a mean-of-rounds ratio with its BCa 95% bootstrap interval over 2,000 resamples, every round's samples in `rounds_raw`; `results/baseline_*.json`; gate transcript `results/validate.log`)*
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

**Ordered scan on string keys is a loss in 33 of 36 cells**, 0.089
[0.088, 0.091] at worst (`counter`, k = 1000) and 2.149 [2.140, 2.175] at best
(`prefixed`, k = 10, where Expanse now wins) *(workload: `masstree_str_map`)* —
**`CONFIRMED`** on the 33, **`REFUTED`** on the three `prefixed` k = 10 cells.

> **Re-measured after [#722](https://github.com/orieg/expanse/issues/722).**
> This pillar used to measure the shipped `ExpanseStrMap` navigation surface —
> `next_at_or_after` / `next_after`, a fresh root descent per element that
> allocated a key the arm then discarded — against Masstree's `scan` visitor,
> which XORs values and never materializes a key. That is an asymmetry (§8.3),
> not a property of the trie, and it is why the loss was read as a finding
> about the surface. `ExpanseStrMap` now has a cursor: one descent, then a
> step along the recorded path, no allocation per element.
>
> **Every one of the 45 scan cells improved, by 1.40× to 10.88× (median
> 2.28×)**, measured as a paired pair of runs differing only in the surface —
> `origin/main`'s positional walk at `82f400b0` against the cursor at
> `7fe02c0b`, same builder, same start distribution, same quiet host. The 15
> pillars the cursor does not touch drifted at most 3.3% between those runs,
> most under 1%, which is the floor the comparison resolves; every scan cell
> cleared it. The superseded cells (0.548 at best, 0.035 at worst) are
> registered in `.github/superseded-figures.json`.
>
> **It narrows the gap; it does not close it.** Masstree still wins 33 of the
> 36 comparable cells, and `counter` at k = 1000 is still 0.089. The surface
> was *a* cause of the loss, not the whole of it.

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
`short` 1.313 [1.306, 1.319] (**`CONFIRMED`**), `skewed` 1.459 [1.455, 1.462]
(`not pre-registered`), and `prefixed` 1.091 [1.085, 1.095] — the issue's
stated expectation that Expanse loses on long shared-prefix keys is
**`REFUTED`**, narrowly, at the low confidence it was registered: both
structures descend the same twelve 8-byte slices (`masstree_envelope.layers_for_shared_prefix(96)`),
and the interval sits just above parity.

> **How much of this is #723 is not established, and the direction is all that
> is.** Every leaf-bearing shape reads faster after the change and `counter`
> does not, which is the shape of the expected effect. But the magnitude cannot
> be attributed on this design: `counter` **is** the no-op control — its
> allocation count is identical across the change — and its ratio still moved
> 0.930 [0.927, 0.933] → 0.946 [0.943, 0.951] between the two runs, intervals
> **not overlapping**. Between-run spread on these single-threaded string cells
> therefore exceeds the within-run BCa interval, exactly as rule 18 documents
> for concurrent cells, so comparing one run's interval against another's is
> not a §8.4 paired claim and is not made here. Attributing the latency would
> need both binaries built at both commits and their arms interleaved in one
> sweep, which this suite does not currently do. **The memory figures above are
> not affected by any of this**: they are deterministic byte and allocation
> counts and they reproduced to the digit across both runs.

**`counter` and `short`-key memory** *(workload: `masstree_str_map`)*. `counter`:
20.53 against 25.18 B/key (**`CONFIRMED`**). The one string memory cell that was
registered as a loss is still a loss: `short` at 33.91 against **47.84**
(**`CONFIRMED`** for Masstree). `skewed` is now **41.33 against Masstree's
46.63 — Expanse ahead**, where the pre-registration counted a Masstree win.

> **Superseded by [#723](https://github.com/orieg/expanse/issues/723).** These
> string memory cells were **69.17** (`short`) and **47.74** (`skewed`) B/key
> when this suite was first published, against a leaf that spent two
> allocations on every key not resolved inside a terminal chunk — a
> `StrSuffix` shell plus a separate byte buffer. The leaf now holds its bytes
> and value in one allocation, and the whole string sweep was re-measured on
> the reference host at the commit that changed it. The superseded figures are
> named here rather than overwritten (§8.7); they are registered in
> `.github/superseded-figures.json` so they cannot return silently. The
> allocation census moved 1,953,568 → **1,065,010** for 10⁶ `short` keys, and
> `mem_used` 50.77 → **42.77** B/key. `counter` is unchanged in both instruments
> (20.53 B/key, 160,627 allocations) because its keys resolve inside terminal
> chunks and allocate no suffix leaf at all — the control for the change.

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
| `random` | 2 | 131,072 | 32.09 (32.09) | 24.80 | 22.77 | 24.09 | 9.33 | 2 | 1.000 | `QUANTUM_DOMINATED` |
| `random` | 4 | 262,144 | 24.05 (24.05) | 22.81 | 22.76 | 22.28 | 1.29 | 3 | 1.000 | `ok` |
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
| `beyond` | 1,000,000 | 272.0 | withheld (1,000,000 keys > 255 B) | 47.83 | withheld (1,000,000 keys > 255 B) | 46.78 | withheld (1,000,000 keys > 255 B) | — | `NOT_REPRESENTABLE_MASSTREE` |
| `counter` | 1,000,000 | 12.0 | 25.18 (25.18) | 20.53 | 22.82 | 19.56 | 2.36 | 100 | `ok` |
| `prefixed` | 1,000,000 | 120.0 | 69.00 (81.51) | 63.46 | 65.43 | 54.77 | 3.58 | 12 | `ok` |
| `short` | 1,000,000 | 12.0 | 33.91 (34.02) | 47.84 | 32.02 | 42.77 | 1.89 | 0 | `ok` |
| `skewed` | 998,150 | 14.3 | 46.63 (49.26) | 41.33 | 43.24 | 37.71 | 3.38 | 0 | `ok` |

#### String map, allocator column across the population sweep (Masstree / Expanse B/key)

| N | `beyond` | `counter` | `prefixed` | `short` | `skewed` |
|---:|---:|---:|---:|---:|---:|
| 1,000 | — / 86.0 | 2109.6† / 91.4 | 4241.4† / 101.7 | 6304.4† / 85.5 | 6315.7† / 78.7 |
| 2,000 | — / 67.6 | 1054.8† / 56.2 | 2138.0† / 81.9 | 3152.1† / 68.2 | 3164.7† / 61.4 |
| 5,000 | — / 60.5 | 421.9† / 34.2 | 876.8† / 75.8 | 1261.3† / 59.5 | 1273.8† / 52.9 |
| 10,000 | — / 54.4 | 211.0† / 27.5 | 456.2† / 70.2 | 630.9† / 54.3 | 643.2† / 47.7 |
| 20,000 | — / 49.3 | 105.5† / 24.0 | 246.0† / 65.0 | 315.6† / 49.2 | 327.6† / 42.5 |
| 50,000 | — / 43.2 | 42.2† / 21.8 | 119.9† / 58.5 | 126.4† / 43.3 | 138.9† / 36.4 |
| 100,000 | — / 42.5 | 42.1† / 21.1 | 98.8† / 58.0 | 84.3† / 42.5 | 97.1† / 36.1 |
| 125,000 | — / 46.5 | 33.7† / 21.0 | 86.2† / 62.2 | 67.5† / 46.5 | 80.3† / 40.2 |
| 150,000 | — / 49.7 | 28.0 / 20.9 | 91.7† / 65.4 | 70.3† / 49.9 | 69.1† / 43.4 |
| 200,000 | — / 50.5 | 42.0† / 20.8 | 77.8 / 66.0 | 52.8† / 50.5 | 65.5† / 44.0 |
| 500,000 | — / 50.0 | 29.4† / 20.6 | 73.5 / 65.3 | 38.1 / 50.0 | 50.8 / 43.3 |
| 1,000,000 | — / 47.8 | 25.2 / 20.5 | 69.0 / 63.5 | 33.9 / 47.8 | 46.6 / 41.3 |

† `QUANTUM_DOMINATED`: the allocator figure is mostly the 2 MiB slab, not the index (§3.3).

![String memory across population](results/chart_string_memory_sweep.svg)

## 4. Latency tables, integer keys

#### Point lookup, 100% hit, integer keys (N = 1,000,000)

| Distribution | λ | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---|
| `clustered` | — | 117.69 | 22.14 | 5.394 [5.330, 5.461] | Expanse — `not pre-registered` |
| `random` | 15.3 | 118.34 | 37.86 | 3.151 [3.116, 3.187] | Expanse — `CONFIRMED` |
| `sequential` | — | 118.10 | 12.57 | 10.455 [9.761, 11.289] | Expanse — `CONFIRMED` |
| `sparse` | — | 117.75 | 9.83 | 13.710 [12.661, 15.002] | Expanse — `CONFIRMED` |

#### Point lookup, 50% hit / 50% rejection-sampled miss, integer keys (N = 1,000,000)

> ⚠️ **Harness methodology disclosure (§8.10, [#760](https://github.com/orieg/expanse/issues/760)).**
> The figures below were measured with a probe builder that drew the hit half of
> the stream from `population[..hits_wanted]`. The population is sorted, so every
> hit landed in the low half of the keyspace — root byte `0x00..0x7F` for uniform
> 64-bit keys — while the misses spanned all of it. The defect is **structurally
> symmetric across both arms**: both saw the identical probe stream, so the
> **ratios and their intervals stand** under §8.10's ratio-versus-absolute
> framework, and the verdicts below are unchanged. The **absolute ns/op are a
> measurement of a keyspace half** and are pending re-measurement under [#760](https://github.com/orieg/expanse/issues/760). The builder is
> fixed (hits are now strided across the whole population); the cells are queued
> for a re-run on the reference host, after which these figures carry a fresh
> provenance tag rather than this note.

| Distribution | λ | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---|
| `clustered` | — | 101.24 | 17.84 | 5.776 [5.697, 5.863] | Expanse — `not pre-registered` |
| `random` | 15.3 | 122.16 | 39.12 | 3.139 [3.114, 3.167] | Expanse — `not pre-registered` |
| `sequential` | — | 68.18 | 11.88 | 6.491 [6.053, 7.037] | Expanse — `CONFIRMED` |
| `sparse` | — | 68.29 | 9.76 | 7.891 [7.380, 8.524] | Expanse — `CONFIRMED` |

#### Insertion into a cold structure, integer keys (N = 1,000,000)

| Distribution | λ | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---|
| `clustered` | — | 20.83 | 21.24 | 0.978 [0.972, 0.984] | Masstree — **`UNPREDICTED LOSS`** |
| `random` | 15.3 | 20.65 | 27.08 | 0.762 [0.755, 0.768] | Masstree — **`UNPREDICTED LOSS`** |
| `sequential` | — | 20.68 | 13.41 | 1.543 [1.526, 1.560] | Expanse — `CONFIRMED` |
| `sparse` | — | 20.61 | 31.10 | 0.663 [0.654, 0.670] | Masstree — **`UNPREDICTED LOSS`** |

#### Ordered range scan, integer keys (N = 1,000,000; Masstree ÷ Expanse per visited element)

| Distribution | k=10 | k=100 | k=1000 |
|---|---:|---:|---:|
| `sequential` | 2.394 [2.292, 2.511] · **`REFUTED`** | 2.327 [2.236, 2.409] · **`REFUTED`** | 2.590 [2.526, 2.615] · `not pre-registered` |
| `clustered` | 2.463 [2.352, 2.587] · **`REFUTED`** | 2.275 [2.186, 2.365] · **`REFUTED`** | 2.535 [2.485, 2.563] · `not pre-registered` |
| `sparse` | 1.635 [1.536, 1.750] · **`REFUTED`** | 1.257 [1.226, 1.288] · **`REFUTED`** | 1.124 [1.113, 1.144] · `not pre-registered` |
| `random` | 1.675 [1.579, 1.785] · **`REFUTED`** | 1.545 [1.462, 1.634] · **`REFUTED`** | 1.622 [1.578, 1.648] · `not pre-registered` |

## 5. Latency tables, string keys

![String latency at N = 1M](results/chart_string_latency.svg)

#### Point lookup, 100% hit, string keys (N = 1,000,000)

| Shape | N held | mean len | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---:|---|
| `beyond` | 1,000,000 | 272.0 | withheld (1,000,000 keys > 255 B) | 347.25 | — | no Masstree cell (§3.4) |
| `counter` | 1,000,000 | 12.0 | 147.66 | 152.18 | 0.978 [0.971, 1.000] | `BOUNDARY_RESULT` |
| `prefixed` | 1,000,000 | 120.0 | 306.80 | 278.67 | 1.100 [1.092, 1.103] | Expanse — **`REFUTED`** |
| `short` | 1,000,000 | 12.0 | 172.69 | 130.85 | 1.322 [1.318, 1.326] | Expanse — `CONFIRMED` |
| `skewed` | 998,150 | 14.3 | 202.55 | 139.10 | 1.460 [1.455, 1.464] | Expanse — `not pre-registered` |

#### Point lookup, 50% hit / 50% rejection-sampled miss, string keys (N = 1,000,000)

> ⚠️ **Harness methodology disclosure (§8.10, [#760](https://github.com/orieg/expanse/issues/760)).**
> Same hit-sampling defect as the integer 50/50 pillar above, in the string
> builder. Symmetric across arms, so the **ratios and verdicts stand**; the
> **absolute ns/op are pending re-measurement** under [#760](https://github.com/orieg/expanse/issues/760).

| Shape | N held | mean len | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---:|---|
| `beyond` | 1,000,000 | 272.0 | withheld (1,000,000 keys > 255 B) | 316.09 | — | no Masstree cell (§3.4) |
| `counter` | 1,000,000 | 12.0 | 83.79 | 97.34 | 0.863 [0.860, 0.867] | Masstree — `not pre-registered` |
| `prefixed` | 1,000,000 | 120.0 | 289.62 | 243.05 | 1.193 [1.190, 1.195] | Expanse — **`REFUTED`** |
| `short` | 1,000,000 | 12.0 | 173.13 | 114.63 | 1.512 [1.509, 1.516] | Expanse — `not pre-registered` |
| `skewed` | 998,150 | 14.3 | 197.00 | 126.72 | 1.552 [1.526, 1.561] | Expanse — `not pre-registered` |

#### Insertion into a cold structure, string keys (N = 1,000,000)

| Shape | N held | mean len | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---:|---|
| `beyond` | 1,000,000 | 272.0 | withheld (1,000,000 keys > 255 B) | 270.64 | — | no Masstree cell (§3.4) |
| `counter` | 1,000,000 | 12.0 | 24.73 | 55.98 | 0.443 [0.441, 0.446] | Masstree — **`UNPREDICTED LOSS`** |
| `prefixed` | 1,000,000 | 120.0 | 165.41 | 175.54 | 0.940 [0.934, 0.945] | Masstree — `CONFIRMED` |
| `short` | 1,000,000 | 12.0 | 52.41 | 107.94 | 0.480 [0.472, 0.491] | Masstree — `not pre-registered` |
| `skewed` | 998,150 | 14.3 | 66.10 | 105.72 | 0.628 [0.623, 0.632] | Masstree — `not pre-registered` |

#### Ordered range scan, string keys (N = 1,000,000; Masstree ÷ Expanse per visited element)

| Shape | k=10 | k=100 | k=1000 |
|---|---:|---:|---:|
| `short` | 0.412 [0.410, 0.415] | 0.251 [0.249, 0.253] | 0.222 [0.221, 0.223] |
| `counter` | 0.346 [0.344, 0.348] | 0.129 [0.128, 0.130] | 0.089 [0.088, 0.091] |
| `prefixed` | 1.474 [1.467, 1.481] | 0.607 [0.603, 0.611] | 0.385 [0.383, 0.388] |
| `skewed` | 0.362 [0.360, 0.365] | 0.192 [0.190, 0.193] | 0.159 [0.158, 0.160] |
| `beyond` | Expanse 114 ns; Masstree withheld | Expanse 46 ns; Masstree withheld | Expanse 38 ns; Masstree withheld |

`beyond` (272-byte keys) fails the §3.4 predicate for its whole population:
Masstree's declared contract is `MASSTREE_MAXKEYLEN = 255`, and the validation
gate records that the shim refuses every such key at the call. The Expanse
figures on that shape are published alone.

## 6. Sensitivity: insertion order and table configuration

#### Sensitivity (§10.2 insertion order, §10.3 table configuration) — both arms, same population

Sorted / single is the order the shared generator produces and the table configuration §10.3 assigns, which every cell above was built in. Shuffled is a Fisher–Yates permutation of the same keys: Masstree's leaf fill, footprint and insertion cost depend on the order, and so do Expanse's insertion cost and allocator footprint; Expanse's own node census (`mem_used`) is the one order-invariant figure. Concurrent is Masstree's fenced, spin-locked node version, the configuration the MC cells use, driven single-threaded here to show the protocol's own cost. Ratios are Masstree ÷ Expanse; no verdict is given against §6.

| Arm | Shape | Order | Table | N | Masstree allocator, settled (unsettled) | Masstree structural | leaf fill | Expanse allocator | Expanse `mem_used` | lookup_hit ratio [BCa 95%] | insert ratio [BCa 95%] |
|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| map | `random` | sorted | single | 1,000,000 | 23.08 (23.08) | 22.76 | 1.000 | 16.67 | 16.70 | 3.149 [3.116, 3.183] | 0.763 [0.751, 0.775] |
| map | `random` | sorted | concurrent | 1,000,000 | 23.08 (23.08) | 22.76 | 1.000 | 16.67 | 16.70 | 3.311 [3.282, 3.345] | 1.148 [1.141, 1.154] |
| map | `random` | shuffled | single | 1,000,000 | 33.57 (33.57) | 33.10 | 0.707 | 23.62 | 16.70 | 3.341 [3.304, 3.380] | 1.886 [1.874, 1.900] |
| str | `prefixed` | sorted | single | 1,000,000 | 69.10 (77.60) | 65.43 | 1.000 | 63.45 | 54.77 | 1.099 [1.091, 1.103] | 0.940 [0.930, 0.945] |
| str | `prefixed` | shuffled | single | 1,000,000 | 88.52 (93.87) | 84.20 | 0.706 | 66.32 | 54.77 | 1.108 [1.105, 1.111] | 1.268 [1.264, 1.276] |
| str | `short` | sorted | single | 1,000,000 | 33.91 (34.02) | 32.02 | 1.000 | 47.84 | 42.77 | 1.325 [1.322, 1.329] | 0.476 [0.469, 0.481] |
| str | `short` | sorted | concurrent | 1,000,000 | 33.91 (34.02) | 32.02 | 1.000 | 47.84 | 42.77 | 1.462 [1.459, 1.466] | 0.598 [0.586, 0.608] |
| str | `short` | shuffled | single | 1,000,000 | 50.62 (51.02) | 47.99 | 0.707 | 50.61 | 42.77 | 1.498 [1.492, 1.503] | 1.091 [1.088, 1.095] |

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

#### What the counters say about the order effect (#737)

The 7 B/key the allocator holds beyond `mem_used` on the shuffled build, and the
insertion cost that moves with it, now have counters. `perf stat` over the
`insert · random · 10⁶` cell in both orders — the same binary, the same
population, one token different *(measured: reference host, `f173f0e6`,
`results/counters_masstree_insert_random_1m_sorted.json` and
`..._shuffled.json`; PMU `cpu_core`, workload pinned to `0-15`,
`perf_event_paranoid` = 1, 7 repeats, BCa 95% intervals in the artifacts)*:

| event | `masstree_insert_random_1m_sorted` | `masstree_insert_random_1m_shuffled` | ratio |
|---|---:|---:|---:|
| `page-faults` | 0.004621 | 0.006296 | **1.36×** |
| `dTLB-load-misses` | 0.007217 | 0.5137 | **71.18×** |
| `LLC-load-misses` | 0.01151 | 0.1722 | **14.96×** |
| `cycles` | 270 | 1,007 | **3.72×** |
| `instructions` | 973 | 1,162 | **1.19×** |

**The shuffled build costs 3.72× the cycles for 1.19× the instructions.** It is
not doing much more work; it is waiting. Translation misses rise **71×** and
last-level load misses **15×**, while page faults move only 1.36× — so the cost
is address-translation and cache locality, not the page-fault bill that §10.2
and [#725](https://github.com/orieg/expanse/issues/725) name as the first
candidate. That is the question #725 asks first, answered.

**What these counters do not say, and the sentences that therefore stand.**
`perf stat` counts the whole process, and this cell runs **both arms** and the
population build inside it. So the figures above are a property of the *cell*,
not of either engine's insert path: they cannot say whether Masstree's append
path or Expanse's is the one whose translation misses rose, and nothing here is
attributed to either. The per-arm mechanism remains **unmeasured**, and every
sentence above that says so still says so. Separating the arms needs one process
per arm, which needs a per-arm selector in the bins — recorded on #725 and #737,
not done here, because adding one would change the binaries every wall-clock
cell in this suite was measured on.

## 7. The concurrent arm

> **Pending re-measurement, tracked by [#730](https://github.com/orieg/expanse/issues/730).**
> Every cell in this section was measured at harness commit `2ce92b7f` and was
> **not** re-run for [#723](https://github.com/orieg/expanse/issues/723). MC1, C1/C2 on `u64` keys, and the `M` census are
> unaffected by that change — it touches `ExpanseStrMap` only. **MC2 is not**:
> its readers walk the string suffix leaf whose representation changed, so its
> levels are stale. They are left in place rather than deleted because the
> *direction* they establish — Expanse string writers losing from one writer —
> rests on the writer mutex, which #723 does not touch, and no re-run is
> needed to keep saying that. The **levels** are withheld pending
> re-measurement (§8.10): quote the direction, not the number, until they are
> re-taken. Re-taking them means two runs, not one, because a concurrent
> level does not replicate within its own interval (rule 18) — that work
> belongs with [#730](https://github.com/orieg/expanse/issues/730), which is
> already open on this arm.

#### MC1 — `u64` keys, Masstree vs `SyncExpanseMap`

**C1 — writer throughput as writer count scales** (W writers insert 2²⁰ fresh keys into a 2²⁰ prefill; fixed work; Expanse ÷ Masstree)

| W | Masstree M/s | Expanse M/s | ratio [BCa 95%] | verdict |
|--:|---:|---:|---|---|
| 1 | 5.12 | 5.68 | 1.141 [1.103, 1.177] | Expanse — **`REFUTED`** (in Expanse's favour) |
| 2 | 9.67 | 4.17 | 0.438 [0.422, 0.452] | Masstree — `CONFIRMED` |
| 4 | 18.19 | 3.24 | 0.180 [0.175, 0.184] | Masstree — `CONFIRMED` |
| 8 | 32.29 | 3.01 | 0.096 [0.093, 0.099] | Masstree — `CONFIRMED` |
| 16 | 35.13 | 2.29 | 0.061 [0.056, 0.065] | Masstree — `not pre-registered` |

**C2 — reader throughput alongside writers** (8 readers probe 50/50 while W writers insert; W = 0 is the reader-only reference; the reader window is the writers' fixed work, so the two arms' windows differ in length by the writer ratio and the population grows at different rates inside them)

| W | Masstree readers M/s | Expanse readers M/s | ratio [BCa 95%] | verdict | Masstree writers M/s | Expanse writers M/s | writer ratio |
|--:|---:|---:|---|---|---:|---:|---|
| 0 | 59.54 | 140.03 | 2.366 [2.347, 2.396] | Expanse — `CONFIRMED` | — | — | — |
| 1 | 42.53 | 16.70 | 0.473 [0.412, 0.556] | Masstree — `CONFIRMED` | 3.81 | 1.95 | 0.485 [0.449, 0.511] |
| 2 | 40.77 | 26.52 | 0.651 [0.635, 0.667] | Masstree — `CONFIRMED` | 7.27 | 1.61 | 0.222 [0.217, 0.229] |
| 4 | 37.26 | 24.77 | 0.662 [0.649, 0.672] | Masstree — `CONFIRMED` | 13.80 | 1.58 | 0.114 [0.112, 0.116] |
| 8 | 32.21 | 24.71 | 0.772 [0.738, 0.805] | Masstree — `CONFIRMED` | 19.30 | 1.54 | 0.075 [0.070, 0.081] |

#### MC2 — `short` string keys, Masstree vs `SyncExpanseStrMap`

**C1 — writer throughput as writer count scales** (W writers insert 2²⁰ fresh keys into a 2²⁰ prefill; fixed work; Expanse ÷ Masstree)

| W | Masstree M/s | Expanse M/s | ratio [BCa 95%] | verdict |
|--:|---:|---:|---|---|
| 1 | 4.18 | 3.57 | 0.897 [0.807, 1.003] | `BOUNDARY_RESULT` — `CONFIRMED` |
| 2 | 7.91 | 2.43 | 0.328 [0.304, 0.362] | Masstree — `CONFIRMED` |
| 4 | 14.82 | 2.18 | 0.163 [0.151, 0.180] | Masstree — `CONFIRMED` |
| 8 | 23.75 | 1.29 | 0.066 [0.052, 0.084] | Masstree — `CONFIRMED` |
| 16 | 30.60 | 0.50 | 0.019 [0.017, 0.021] | Masstree — `not pre-registered` |

**C2 — reader throughput alongside writers** (8 readers probe 50/50 while W writers insert; W = 0 is the reader-only reference; the reader window is the writers' fixed work, so the two arms' windows differ in length by the writer ratio and the population grows at different rates inside them)

| W | Masstree readers M/s | Expanse readers M/s | ratio [BCa 95%] | verdict | Masstree writers M/s | Expanse writers M/s | writer ratio |
|--:|---:|---:|---|---|---:|---:|---|
| 0 | 36.47 | 30.96 | 0.851 [0.845, 0.866] | Masstree — **`UNPREDICTED LOSS`** | — | — | — |
| 1 | 26.81 | 4.05 | 0.161 [0.131, 0.222] | Masstree — `CONFIRMED` | 2.89 | 1.78 | 0.651 [0.602, 0.746] |
| 2 | 25.00 | 7.63 | 0.314 [0.293, 0.345] | Masstree — `CONFIRMED` | 3.89 | 1.20 | 0.296 [0.254, 0.334] |
| 4 | 22.23 | 5.67 | 0.282 [0.263, 0.307] | Masstree — `CONFIRMED` | 7.25 | 1.41 | 0.188 [0.169, 0.207] |
| 8 | 18.23 | 6.50 | 0.365 [0.347, 0.384] | Masstree — `CONFIRMED` | 12.34 | 1.30 | 0.102 [0.093, 0.109] |

#### H — protocol health, Expanse side only (occ-stats build; event ratios, never a timing)

| Arm | W | R | restart share, median [min, max] | fallback share, median | `sample_spins` ÷ `read_ops` (medians) | §6.3 |
|---|--:|--:|---|---|---:|---|
| map | 1 | 8 | 4.22% [4.13%, 4.95%] | 0.0000% | 0.96 | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 2 | 8 | 5.65% [4.12%, 6.39%] | 0.0000% | 1.12 | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 4 | 8 | 6.09% [5.83%, 6.73%] | 0.0000% | 1.19 | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 8 | 8 | 5.93% [5.27%, 6.82%] | 0.0000% | 1.18 | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| str | 1 | 8 | 35.93% [35.65%, 38.28%] | 0.0000% | 2.49 | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| str | 2 | 8 | 26.36% [25.04%, 31.44%] | 0.0000% | 2.07 | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| str | 4 | 8 | 29.23% [26.52%, 30.52%] | 0.0000% | 2.61 | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| str | 8 | 8 | 31.14% [29.91%, 33.90%] | 0.0000% | 2.59 | rise with W: **`REFUTED`**; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |

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
one shared counter line across nine threads (#721 scopes per-thread counters).

MC2's rows were `NOT_INSTRUMENTED` until #744: `StrReader::get` counted
fallbacks only, so a 0% restart share would have been a number about the
counters rather than the protocol (§10.5). The string, bytes and blob readers
now count `ReadOps` and `ReadAttempts` where the map path does, restarts
included, and the rows above are the first measurement of them. **The string
reader restarts five to seven times as often as the map reader under the same
eight-reader load** — 26.4%–35.9% against 4.2%–6.1% — and its `sample_spins`
per read op is 2.1–2.6 against the map's 0.9–1.3. Neither arm reaches the
writer-lock fallback at all, which needs 64 consecutive failed walks. Both
shares reproduced within a point or two on a discarded first attempt of this
run (below), so the gap is not one sample.

What the restart share costs is **not** measured here. These are event ratios
from the counting build, the timing build is a separate binary (§5.1), and no
cell in this suite attributes a nanosecond to a restart. Whether it is the
mechanism behind the string reader's concurrency cost is a hypothesis, and
[#730](https://github.com/orieg/expanse/issues/730) is where it would be
tested.

#### M — build-only single-writer census, Masstree vs `SyncExpanseMap` (B/key)

| λ | N | Masstree allocator | SyncExpanseMap allocator | Masstree structural | Expanse `mem_used` | flag |
|---:|---:|---:|---:|---:|---:|---|
| 1 | 65,536 | 32.19 | 24.26 | 22.78 | 22.66 | `QUANTUM_DOMINATED` |
| 2 | 131,072 | 32.09 | 24.72 | 22.77 | 24.09 | `QUANTUM_DOMINATED` |
| 4 | 262,144 | 24.05 | 22.95 | 22.76 | 22.28 | `ok` |
| 8 | 524,288 | 24.02 | 19.34 | 22.76 | 19.05 | `ok` |
| 15 | 983,040 | 23.48 | 16.80 | 22.76 | 16.75 | `ok` |
| 23 | 1,507,328 | 23.66 | 16.36 | 22.76 | 16.20 | `ok` |
| 30 | 1,966,080 | 23.47 | 20.34 | 22.76 | 19.05 | `ok` |
| 38 | 2,490,368 | 23.58 | 25.90 | 22.76 | 23.24 | `ok` |
| 46 | 3,014,656 | 22.96 | 26.77 | 22.76 | 24.00 | `ok` |
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

**Between-run spread of this suite's own cells.** This arm has now been run
three times on the reference host: at harness commit `82966aae` *(artifacts at
`a8da40e3` in history)*, at `2ce92b7f` *(artifacts at `68ce68a9` in history)*,
and at `13cb3eb5` (the cells above), the last of them to measure the string
reader's restart share once #744 instrumented it. Between the first two only
the census shim's free path changed in the concurrent binary (§10.6); between
the second and the third only the reader counter bumps, which are compiled out
of the timing binary entirely. A fourth attempt at `13cb3eb5` was **discarded
unread** for starting at a load average of 0.81 left over from its own build,
the condition that discarded an earlier run of this suite; the run published
here started at 0.01.

The spread is the finding, and it is large. **Seventeen of the twenty-eight
C1 and C2 ratio cells put one run's point estimate outside the other run's BCa
interval** — among them the integer W = 4 R = 8 reader cell, 0.472 [0.455,
0.494] then 0.662 [0.649, 0.672], and the string W = 1 R = 8 reader cell,
0.228 [0.212, 0.241] then 0.161 [0.131, 0.222]. **No cell changed direction in
any of the three runs**, and no verdict moved. The levels above are therefore
one run's and not a settled figure: per
[`docs/BENCHMARKING.md`](../../BENCHMARKING.md) rule 18 the claim ceiling on a
concurrent cell is the union of the runs' intervals, and a cell whose runs do
not overlap is quoted as a direction and a range, never as a level. What three
runs settle is direction; what they do not settle is magnitude.

## 8. Scorecard against the pre-registration

#### Scorecard (wall-clock cells with a Masstree column)

| | Count |
|---|---:|
| Expanse wins (CI excludes parity) | 85 |
| Masstree wins (CI excludes parity) | 81 |
| `BOUNDARY_RESULT` | 6 |
| Masstree column withheld (§3.4, `beyond`) | 18 |

| Label | Cells |
|---|---:|
| Masstree — `CONFIRMED` | 58 |
| Expanse — `not pre-registered` | 32 |
| Expanse — **`REFUTED`** | 31 |
| Expanse — `CONFIRMED` | 21 |
| Masstree — `not pre-registered` | 13 |
| Masstree — **`UNPREDICTED LOSS`** | 10 |
| `BOUNDARY_RESULT` | 5 |
| Expanse — **`REFUTED`** (in Expanse's favour) | 1 |
| `BOUNDARY_RESULT` — `CONFIRMED` | 1 |

| Registered (§6) | Outcome |
|---|---|
| Masstree wins C1 at W ≥ 2, both arms (high) | **CONFIRMED** on every cell |
| Masstree wins or `BOUNDARY_RESULT` at W = 1 (medium-low) | **CONFIRMED** on strings (0.883); **REFUTED in Expanse's favour** on integers (1.134) |
| Masstree wins readers under writers (medium-high) | **CONFIRMED** on every cell |
| Masstree wins integer scan at k = 10, 100 (medium-high) | **REFUTED** on 21 of 24 cells; **CONFIRMED** on `random` at 10⁴ (k = 10, 100) and 10⁵ (k = 100) |
| Masstree wins string scan, every k (high) | **CONFIRMED** on 33 of 36; **REFUTED** on `prefixed` at k = 10, all three populations, after [#722](https://github.com/orieg/expanse/issues/722) gave `ExpanseStrMap` a cursor. Every scan cell moved 1.4×–10.9× in Expanse's favour; the prediction survives on the rest |
| Masstree wins `prefixed` lookup and insert (low) | insert **CONFIRMED** at 10⁶, `BOUNDARY_RESULT` at 10⁴; lookup **REFUTED** (1.091 [1.085, 1.095]) |
| Masstree wins `short` / `skewed` index memory (medium) | `short` **CONFIRMED** (33.91 against 47.84); `skewed` **REFUTED** after [#723](https://github.com/orieg/expanse/issues/723) — 46.63 against 41.33, Expanse ahead. At the pre-registration's leaf it was 46.63 against 47.74, a Masstree win by 1.1 B/key; that figure is superseded, not overwritten |
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
