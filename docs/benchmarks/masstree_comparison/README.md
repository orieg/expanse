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
> *(measured: reference host — Intel Core i9-12900F, 8P+8E/24 threads, 30 MiB L3, Ubuntu 22.04, kernel 6.8; Masstree [`kohler/masstree-beta`](https://github.com/kohler/masstree-beta) `1119842`, MIT with a publicity clause; single-threaded phases (§1–§6 and the single-threaded cells of the §8 scorecard) re-measured at commit `b868fb2e` with `docs/benchmarks/masstree_comparison/run.sh`, **twice** — the tables are run 1, and run 2 at the same commit on the same host (`results/baseline_*_run2.json`) is the between-run check `docs/BENCHMARKING.md` rule 18 asks for (§8, "Two runs"); **the concurrent phases (§7: MC1, MC2, H and M) were measured at commit `b868fb2e`, two runs, load average 1.00 and 1.01 at their starts and at most 5.15 and 5.91 during them — the sweep's own threads, which is why it runs last — with foreign busy CPU at most 0.02 core-equivalents in any cell**; benchmark shell pinned to CPUs 0-15 and every concurrent row records `Cpus_allowed_list 0-15`; both arms built for one ISA target — `-C target-cpu=haswell` and `-march=haswell -O3 -std=c++17 -DNDEBUG`, assertions off, superpages on, glibc 2.35 `malloc`; load average 1.00 at every one of the seven snapshots of both runs, start to end, with busy CPU 1.00–1.22 core-equivalents between consecutive snapshots in run 1 and 1.00–1.15 in run 2, of which the benchmark's own process accounts for 0.96–1.19 and 0.96–1.18, and a largest foreign busy-CPU delta of 0.04 core-equivalents in each run; frequency driver `intel_pstate` in `powersave`, transparent huge pages `madvise`, P-cores `0-15` with SMT and E-cores `16-23` outside the pin; 15 rounds per wall-clock cell, the arm timed first alternating per round, per-arm medians reported beside a mean-of-rounds ratio with its BCa 95% bootstrap interval over 2,000 resamples, every round's samples in `rounds_raw`; `results/baseline_*.json`; gate transcript `results/validate.log`)*
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
writer on string keys.** Stage B's optimistic lock coupling lets
`SyncExpanseMap` admit concurrent writers, and it does not cover
`SyncExpanseStrMap` (`docs/ARCHITECTURE.md` §4.2); Masstree's per-node locks
admit writers on both. With eight writers inserting 2²⁰ fresh keys into a 2²⁰
prefill, Masstree sustains 32.76 M inserts/s against Expanse's 11.68 in run 1, ratio
0.356 [0.349, 0.366] and 0.351 [0.346, 0.358] in run 2; at sixteen,
0.289 [0.266, 0.305] and 0.279 [0.256, 0.300] — sixteen threads on eight
physical P-cores with SMT *(workload: `masstree_conc_map_64bit`)*. Expanse's
aggregate integer insert rate rises with writer count, 5.27 → 6.67 → 8.97 → 11.68 M/s
from one to eight writers and 10.66 at sixteen (run 1), while Masstree's runs
5.18 → 9.87 → 18.48 → 32.76 → 34.57. The single-writer integer cell claims no winner in either run,
1.004 [0.977, 1.028] and 0.986 [0.961, 1.012] — **`CONFIRMED`** against a
registration of "Masstree wins or `BOUNDARY_RESULT`". On `short` string keys
the loss starts at one writer — 0.879 [0.801, 0.952] and
0.902 [0.819, 0.981] — and reaches 0.017 [0.016, 0.018] at sixteen in
both runs, where the Expanse string writers run at 0.47–0.48 M inserts/s
*(workload: `masstree_conc_str`)*. What limits either Expanse arm's scaling is
**unmeasured** — this arm carries no hardware counters (§8.9) — and no
mechanism is claimed. §6.1 rows 1 and 2 are **`CONFIRMED`**.

![Writer throughput vs writer count](results/chart_concurrent_writers.svg)

**String readers under any writer load go to Masstree.** One writer takes
Expanse's eight `short`-key readers from 36.92–36.95 to 1.08–1.14 M lookups/s across
the two runs while Masstree's go from 34.23–34.27 to 25.26–25.54 —
0.042 [0.040, 0.044] and 0.045 [0.043, 0.048] — and the ratio stays at
0.189–0.269 with two to eight writers *(workload: `masstree_conc_str`)*.
**`CONFIRMED`** (§6.1 row 3). The mechanism is **unmeasured**: the health rows
in §7 count what the string reader does under load, and no cell in this suite
attributes the throughput loss to any of them. The writer pays too: with eight
readers probing, Expanse's single string writer runs at 1.93–2.02 M
inserts/s against Masstree's 2.88–2.89, and the integer single writer at
2.98–3.03 against 3.86–4.00 — writer ratios 0.801 [0.746, 0.887] and
0.760 [0.727, 0.826] *(workload: `masstree_conc_map_64bit`)*.

![Reader throughput alongside writers](results/chart_concurrent_readers.svg)

**Ordered scan on string keys is a loss in 33 of 36 cells**, 0.090
[0.089, 0.091] at worst (`counter`, k = 1000, N = 10⁴) and 2.271 [2.258, 2.302]
at best (`prefixed`, k = 10, N = 10⁴, where Expanse now wins) *(workload:
`masstree_str_map`)*, with the same winner in every cell in both runs —
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
> 36 comparable cells, and `counter` at k = 1000 and N = 10⁶ is 0.093
> [0.093, 0.094] at `b868fb2e`. The surface was *a* cause of the loss, not the
> whole of it.

**String insertion at N = 10⁶ is Masstree's on every representable shape**,
0.479 [0.472, 0.485] on `short`, 0.433 [0.427, 0.438] on `counter`, 0.634
[0.631, 0.638] on `skewed`, 0.964 [0.959, 0.968] on `prefixed` *(workload:
`masstree_str_map`)*. Only `prefixed` was registered (**`CONFIRMED`**);
`counter` was registered the other way and is an **`UNPREDICTED LOSS`**;
`short` and `skewed` were not predicted. Below 10⁶ `prefixed` is not
Masstree's. At 10⁵ it is Expanse's in both runs, 1.088 [1.074, 1.107] and
1.076 [1.062, 1.095], a **`REFUTED`** cell and a correction of the earlier
`7fe02c0b` measurement, which had it at `BOUNDARY_RESULT`, 1.005
[0.995, 1.017]; at 10⁴ it is `BOUNDARY_RESULT` in both runs, 1.002
[0.971, 1.068]. The §10.2 sensitivity rows say what is being measured: on a
shuffled permutation of the same keys `short` insertion is 1.089
[1.087, 1.093] and `prefixed` 1.297 [1.292, 1.300], both Expanse's. This
README earlier quoted 0.969 and 1.208 for those two cells, figures the
`7fe02c0b` artifact already contradicted (1.091 [1.088, 1.095] and 1.268
[1.264, 1.276]). Sorted insertion is a B+-tree's best case (every leaf fills,
no split lands mid-leaf) and the shared generator hands both arms the
population sorted.

**Integer insertion on `random`, `sparse` and `clustered` keys is Masstree's
too, in the sorted order the suite builds in** — 0.754 [0.746, 0.760] on
`random` and 0.684 [0.676, 0.691] on `sparse` at N = 10⁶, two
**`UNPREDICTED LOSS`** cells in both runs against a medium-confidence
registration, and 0.989 [0.984, 0.995] on `clustered`, which run 2 puts at
parity (0.996 [0.990, 1.003]), so that third cell is a direction only (rule
18); `sequential` is Expanse's at 1.656 [1.639, 1.672] *(workload:
`masstree_map_64bit`)*. Masstree inserts at a flat 20.4–20.6 ns at that
population whatever the distribution. On the shuffled permutation the same
`random` cell is 1.891 [1.874, 1.921] in Expanse's favour, and with Masstree's
concurrent table 1.137 [1.131, 1.141] (§10.3): the registered win exists, in
the insertion order and the configuration the pre-registration did not name.
At N = 10⁵ `clustered` insertion is `BOUNDARY_RESULT` in both runs, 0.993
[0.970, 1.026], a correction of the earlier `7fe02c0b` measurement's
`UNPREDICTED LOSS`, 0.931 [0.909, 0.969]. At N = 10⁴ `sequential` insertion is
`BOUNDARY_RESULT` in run 1, 1.171 [0.984, 1.556], and Expanse's in run 2,
1.183 [1.001, 1.561]: a direction, not a confirmed win.

**`counter` string lookup at N = 10⁶ is Masstree's**, 0.952 [0.948, 0.956]
at 100% hit and 0.866 [0.863, 0.870] at 50/50, and run 2 agrees (0.964
[0.961, 0.967] and 0.862 [0.858, 0.866]) *(workload: `masstree_str_map`)*. It
was registered as a high-confidence Expanse win at exactly this population:
the 100%-hit cell is an **`UNPREDICTED LOSS`**, and the 50/50 cell, which the
derived table labels `not pre-registered`, lands the same way. **The 100%-hit
verdict is a correction:** the earlier `7fe02c0b` measurement had that cell at
`BOUNDARY_RESULT`, 0.978 [0.971, 1.000]. The change is on the Masstree side —
its median fell from 147.66 to 144.22 ns (146.06 in run 2) while
`ExpanseStrMap`'s is 151.79 (151.97) against 152.18 — and its cause is
unmeasured *(measured: reference host, `results/baseline_string_latency.json`
at `b868fb2e` against the `7fe02c0b` artifact in history)*.

**Reader-only string throughput goes to Masstree**, 0.865 [0.853, 0.886] with
eight readers and no writer *(workload: `masstree_conc_str`)* — an
**`UNPREDICTED LOSS`** against the medium-confidence registration, measured on
the concurrent arm's own engine commit (see the provenance note above).

**Memory at the ends of the density sweep goes to Masstree by a few bytes per
key.** Masstree holds 22.76 B/key structurally at
every λ **in the sorted order the suite builds in** — every leaf fills to 15
keys, the B+-tree's bulk-load figure; the same keys shuffled fill 70.7% and
cost 33.10 (§6) — and 23.0–24.1 B/key on the allocator instrument outside the
quantum-dominated cells. Its per-key cost does not depend on key density. `ExpanseMap` is below that from λ = 8
(19.02) through λ = 30 (19.98), with its best at λ = 23 (17.57), and above it
at λ = 38, 46 and 61 (23.99, 24.72, 23.82) — the `LEAF_CAP` cascade
`hot_comparison` §9.4 documents. §6.2 row 1 (λ ∈ [8, 23]) is **`CONFIRMED`**;
row 2 (outside the band) is an **`UNPREDICTED LOSS`** above the cascade and
parity-by-magnitude at λ = 4 (24.05 vs 22.80). **`prefixed` string memory is
not a loss:** `ExpanseStrMap` holds 63.97 against Masstree's 68.93 B/key
(69.05 in run 2), so the medium-confidence row is **`CONFIRMED`** by magnitude
— a correction of this README's earlier reading, 72.53 against 69.06 and an
`UNPREDICTED LOSS`, which the `7fe02c0b` artifact had already reversed (63.46
against 69.00) *(workload: `masstree_str_map`)*. §10.2 records that the
registration leaned on a shuffled-order figure (84.2 B/key structural, which
the sensitivity table reproduces at 88.51 allocator). Memory cells carry no
interval, so these labels are by magnitude, and the taxonomy is the
wall-clock one: a registered Expanse win that landed the other way is an
`UNPREDICTED LOSS`, never a `REFUTED`, which the derived tables reserve for a
registered Masstree win that Expanse took.

## 2. Where Expanse wins

**Integer point lookup, by 2.9× to 13.5× at N = 10⁶, on every distribution
and at every population.** At N = 10⁶ Masstree answers a lookup in 116–117 ns
whatever the distribution — a B+-tree descent of the same height regardless
of key structure — while Expanse answers `sparse` in 9.9 ns (13.537
[12.522, 14.764]), `sequential` in 12.8 (10.277 [9.579, 11.111]), `clustered`
in 22.0 (5.406 [5.342, 5.474]) and `random` in 40.0 (2.949 [2.921, 2.978])
*(workload: `masstree_map_64bit`)*. `sequential`, `sparse` and `random` were
registered (**`CONFIRMED`**); `clustered` was not. The 50/50 pillar follows,
2.961 [2.927, 2.995] to 7.846 [7.329, 8.523]. The two runs agree on every
winner but not on every level: Masstree's medians were 8–13% higher in run 2
on every lookup cell at this population except `sparse` at 100% hit, which
puts run 2's `random` 100%-hit ratio at 3.214 [3.189, 3.241], outside run 1's
interval, so these levels are run 1's and the direction is what both settle.
`ExpanseMap`'s `random` median is 40.04 and 39.92 ns in the two runs against
37.86 at the earlier `7fe02c0b` measurement, cause unmeasured.
Why Masstree's descent costs
what it does here is unmeasured — no counter was taken — and the cross-suite
comparison the reader will want (HOT held random 1M lookup near parity) is one
this suite does not draw (§8 item 6).

![Latency at N = 1M](results/chart_latency_1m.svg)

**Ordered scan on integer keys goes to Expanse on every structured
distribution and on `random` at 1M, and to Masstree on `random` below 1M.**
Through `ExpanseMap::range()` Expanse visits an element in 1.5–11.8 ns against
Masstree's 3.8–17.1 at N = 10⁶: 1.577 [1.491, 1.675] at k = 10 on `random`,
2.502 [2.453, 2.526] at k = 1000 on `sequential` *(workload:
`masstree_map_64bit`)*. On `random` below 10⁶ the direction reverses from
k = 100 in both runs — 0.665 [0.663, 0.668] and 0.611 [0.609, 0.613] at 10⁴,
0.567 [0.565, 0.571] and 0.490 [0.488, 0.494] at 10⁵, for k = 100 and
k = 1000 — and the k = 10 cells are not settled: at 10⁴ run 1 has Expanse
ahead at 1.006 [1.003, 1.010] and run 2 has Masstree at 0.935 [0.932, 0.939],
and at 10⁵ run 1 has Masstree at 0.957 [0.948, 0.962] and run 2 parity, 1.005
[0.991, 1.016]. Both are quoted as a direction only (rule 18). The 10⁵ cell is
also a correction: the earlier `7fe02c0b` measurement had Expanse ahead there
at 1.074 [1.058, 1.080], which neither run reproduces. §6.1 row 4 registered
Masstree for k = 10 and k = 100 on the strength of the ART and HOT results: it
is **`REFUTED`** on 20 of the 24 registered cells in both runs (21 in run 1)
and **`CONFIRMED`** on `random` at k = 100 at 10⁴ and 10⁵ in both runs; the
two `random` k = 10 cells below 10⁶ are the unsettled pair above. The k = 1000
cells were `not pre-registered`. Masstree's scan is driven through its visitor interface with
a key reassembled per element; why that is cheaper than Expanse's iterator on
a small random population and dearer everywhere else is unmeasured. The k = 10
cells are where the §10.6 start count told most: with a hundred times more
distinct starts per round the per-element cost rose on both arms — on `random`
at 10⁶ Masstree 14.5 → 17.4 ns and Expanse 9.4 → 11.1 against the first run
*(measured: reference host, harness commit `82966aae`, artifacts at `a8da40e3`
in history)* — which is consistent with a colder descent per start and is not
measured further.

**Integer readers, alone and alongside writers.** Eight readers alone:
1.829 [1.812, 1.856] and 1.853 [1.833, 1.886] across the two runs —
**`CONFIRMED`**. With one to eight writers inserting, every cell goes to
Expanse in both runs, 1.924–1.987, each interval clear of parity — the
registered Masstree win (§6.1 row 3) is **`REFUTED`** in Expanse's favour on
all four integer cells. What sets these reader levels is **unmeasured**
*(measured: reference host — Intel i9-12900F, commit `b868fb2e`, two runs;
workload: `masstree_conc_map_64bit`)*.

**Reader-only string throughput**, 1.079 [1.077, 1.081] and
1.077 [1.066, 1.081] — **`CONFIRMED`** *(workload: `masstree_conc_str`)*.

**Memory on structured integer keys and in the density band.** `sequential`
and `clustered` at N = 10⁶: 8.91 and 8.96 B/key against Masstree's flat 23.08
(**`CONFIRMED`**, the low-information cell §6.2 said it would be); `sparse`
16.41 against 23.08 (**`CONFIRMED`**); `random` at λ = 15 and 23: 17.84 and
17.57 against 23.48 and 23.66 (**`CONFIRMED`**) *(workload:
`masstree_map_64bit`)*.

**String point lookup on `short`, `skewed` and, narrowly, `prefixed`.**
At N = 10⁶ and 100% hit, `short` 1.326 [1.322, 1.329] (**`CONFIRMED`**),
`skewed` 1.467 [1.461, 1.472] (`not pre-registered`), and `prefixed` 1.119
[1.116, 1.122] — the issue's stated expectation that Expanse loses on long
shared-prefix keys is **`REFUTED`**, narrowly, at the low confidence it was
registered: both structures descend the same twelve 8-byte slices
(`masstree_envelope.layers_for_shared_prefix(96)`), and the interval sits about
a tenth above parity in both runs (run 2: 1.098 [1.097, 1.100]) *(workload:
`masstree_str_map`)*.

> **How much of this is #723 is not established, and the direction is all that
> is.** Every leaf-bearing shape read faster after the change and `counter` did
> not, which is the shape of the expected effect. But the magnitude cannot be
> attributed on this design: `counter` **is** the no-op control — its
> allocation count did not change with #723 — and between-run spread on these
> single-threaded string cells exceeds the within-run BCa interval. The two
> `b868fb2e` runs, the same binaries on the same host, put `counter` at 100%
> hit at 0.952 [0.948, 0.956] and 0.964 [0.961, 0.967], intervals **not
> overlapping**, exactly as rule 18 documents for concurrent cells, so
> comparing one run's interval against another's is not a §8.4 paired claim and
> is not made here. Attributing the latency would need both binaries built at
> both commits and their arms interleaved in one sweep, which this suite does
> not currently do. **The Expanse memory figures above are not affected by any
> of this**: they are deterministic byte and allocation counts and they
> reproduced to the digit across both runs.

**`counter` and `short`-key memory** *(workload: `masstree_str_map`)*. `counter`:
20.54 against 25.18 B/key (**`CONFIRMED`**). The one string memory cell that was
registered as a loss is still a loss: `short` at 33.91 against **48.20**
(**`CONFIRMED`** for Masstree). `skewed` is now **41.76 against Masstree's
46.63 — Expanse ahead**, where the pre-registration counted a Masstree win.

> **Superseded by [#723](https://github.com/orieg/expanse/issues/723).** These
> string memory cells were **69.17** (`short`) and **47.74** (`skewed`) B/key
> when this suite was first published, against a leaf that spent two
> allocations on every key not resolved inside a terminal chunk — a
> `StrSuffix` shell plus a separate byte buffer. The leaf now holds its bytes
> and value in one allocation, and the whole string sweep was re-measured on
> the reference host at the commit that changed it. The superseded figures are
> named here rather than overwritten (§8.7); they are registered in
> `.github/superseded-figures.json` so they cannot return silently. Across that
> change the allocation census moved 1,953,568 → 1,065,010 for 10⁶ `short`
> keys and `mem_used` 50.77 → 42.77 B/key, while `counter` was unchanged in both
> instruments (20.53 B/key, 160,627 allocations) because its keys resolve inside
> terminal chunks and allocate no suffix leaf at all — the control for the
> change. The census has moved again since, identically in both `b868fb2e`
> runs: **1,049,567** allocations and **43.25** B/key `mem_used` for `short`,
> and 141,423 allocations for `counter`; which engine commit moved it is not
> isolated here.

## 3. What the census says, and what it does not

Two instruments per cell, never mixed (§3.3). The allocator column is what
the process holds; on Masstree it is quantized to the 2 MiB pool slab, so at
λ = 1 and λ = 2 and on every string cell below N ≈ 150k the figure is mostly
slab and is flagged `QUANTUM_DOMINATED`. Where it is not flagged, the measured
slack above Masstree's own node census is 0.2–3.5 B/key on the integer cells
and the N = 10⁶ string cells (3.6 on `prefixed` in run 2), and 5.2–12.3 B/key
on the unflagged string cells of the population sweep below 10⁶. Masstree's structural
figure is 22.76 B/key on every integer cell in the sorted build order: 66,667
leaves at 100% fill plus 4,448 internodes for 10⁶ keys, exactly what `masstree_envelope.structural_bytes`
gives for those counts — and 33.10 B/key at 70.7% fill on the shuffled
permutation (§10.2). The RCU settle step (§10.4) reclaimed 11.3 B/key of
suffix bags left behind on `prefixed` in both runs (80.27 → 68.93 and 80.33 →
69.05), and nothing on integer keys, which allocate no bags; that settled
`prefixed` figure is the one §3 memory cell whose two runs differ, by 0.12
B/key. `ExpanseMap`'s own census moved between the earlier `7fe02c0b`
artifacts and these — `random` at λ = 15 from 16.75 to 17.60 B/key `mem_used`
(16.72 → 17.84 allocator), at λ = 23 from 16.20 to 17.18 — identically in both
runs, so the tables below are current; which engine commit moved it is not
isolated here *(workload: `masstree_map_64bit`)*.

![Memory across expanse occupancy](results/chart_memory_curve.svg)

#### Memory, integer map: two instruments per cell, bytes per key

`allocator` is what the process holds from the C allocator after a build-only population, one instrument for both arms; on Masstree it is quantized to the 2 MiB pool slab and a cell whose measured slack exceeds 25% of its structural bytes is flagged `QUANTUM_DOMINATED` (§3.3). `structural` is Masstree's own `json_stats` node census; `mem_used` is Expanse's own accounting. The engine columns are never mixed with the allocator columns in one ratio.

| Distribution | λ | N | Masstree allocator, settled (unsettled) | Expanse allocator | Masstree structural | Expanse `mem_used` | Masstree slack | slabs | leaf fill | flag |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `random` | 1 | 65,536 | 32.19 (32.19) | 24.86 | 22.78 | 23.65 | 9.41 | 1 | 1.000 | `QUANTUM_DOMINATED` |
| `random` | 2 | 131,072 | 32.09 (32.09) | 24.79 | 22.77 | 24.09 | 9.33 | 2 | 1.000 | `QUANTUM_DOMINATED` |
| `random` | 4 | 262,144 | 24.05 (24.05) | 22.80 | 22.76 | 22.28 | 1.29 | 3 | 1.000 | `ok` |
| `random` | 8 | 524,288 | 24.02 (24.02) | 19.02 | 22.76 | 19.07 | 1.27 | 6 | 1.000 | `ok` |
| `random` | 15 | 983,040 | 23.48 (23.48) | 17.84 | 22.76 | 17.60 | 0.72 | 11 | 1.000 | `ok` |
| `random` | 23 | 1,507,328 | 23.66 (23.66) | 17.57 | 22.76 | 17.18 | 0.90 | 17 | 1.000 | `ok` |
| `random` | 30 | 1,966,080 | 23.47 (23.47) | 19.98 | 22.76 | 19.49 | 0.72 | 22 | 1.000 | `ok` |
| `random` | 38 | 2,490,368 | 23.58 (23.58) | 23.99 | 22.76 | 23.30 | 0.83 | 28 | 1.000 | `ok` |
| `random` | 46 | 3,014,656 | 22.96 (22.96) | 24.72 | 22.76 | 24.01 | 0.20 | 33 | 1.000 | `ok` |
| `random` | 61 | 3,997,696 | 23.09 (23.09) | 23.82 | 22.76 | 23.17 | 0.33 | 44 | 1.000 | `ok` |
| `clustered` | — | 1,000,000 | 23.08 (23.08) | 8.96 | 22.76 | 8.61 | 0.32 | 11 | 1.000 | `ok` |
| `sequential` | — | 1,000,000 | 23.08 (23.08) | 8.91 | 22.76 | 8.56 | 0.32 | 11 | 1.000 | `ok` |
| `sparse` | — | 1,000,000 | 23.08 (23.08) | 16.41 | 22.76 | 16.31 | 0.32 | 11 | 1.000 | `ok` |

#### Memory, string map: two instruments per cell, bytes per key

Both sides copy key bytes into their own nodes, so the index column is the ownership column on both (§4). Columns as the integer table.

| Shape | N | mean len | Masstree allocator, settled (unsettled) | Expanse allocator | Masstree structural | Expanse `mem_used` | Masstree slack | layers | flag |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `beyond` | 1,000,000 | 272.0 | withheld (1,000,000 keys > 255 B) | 48.19 | withheld (1,000,000 keys > 255 B) | 47.25 | withheld (1,000,000 keys > 255 B) | — | `NOT_REPRESENTABLE_MASSTREE` |
| `counter` | 1,000,000 | 12.0 | 25.18 (25.18) | 20.54 | 22.82 | 19.56 | 2.36 | 100 | `ok` |
| `prefixed` | 1,000,000 | 120.0 | 68.93 (80.27) | 63.97 | 65.43 | 55.25 | 3.50 | 12 | `ok` |
| `short` | 1,000,000 | 12.0 | 33.91 (34.02) | 48.20 | 32.02 | 43.25 | 1.89 | 0 | `ok` |
| `skewed` | 998,150 | 14.3 | 46.63 (49.26) | 41.76 | 43.24 | 38.18 | 3.38 | 0 | `ok` |

#### String map, allocator column across the population sweep (Masstree / Expanse B/key)

| N | `beyond` | `counter` | `prefixed` | `short` | `skewed` |
|---:|---:|---:|---:|---:|---:|
| 1,000 | — / 87.0 | 2109.6† / 90.8 | 4241.4† / 102.3 | 6304.4† / 86.1 | 6315.7† / 80.2 |
| 2,000 | — / 67.9 | 1054.8† / 55.8 | 2138.0† / 82.4 | 3152.1† / 68.5 | 3164.7† / 61.6 |
| 5,000 | — / 61.5 | 421.9† / 34.1 | 876.8† / 76.7 | 1261.3† / 60.4 | 1273.8† / 53.9 |
| 10,000 | — / 54.5 | 211.0† / 27.5 | 456.2† / 70.6 | 630.9† / 54.4 | 643.2† / 48.0 |
| 20,000 | — / 49.6 | 105.5† / 24.0 | 246.0† / 65.4 | 315.6† / 49.5 | 327.6† / 42.9 |
| 50,000 | — / 44.0 | 42.2† / 21.8 | 119.9† / 59.4 | 126.4† / 44.1 | 138.9† / 37.2 |
| 100,000 | — / 43.6 | 42.1† / 21.1 | 98.8† / 59.2 | 84.3† / 43.6 | 97.1† / 37.1 |
| 125,000 | — / 47.0 | 33.7† / 21.0 | 86.2† / 62.9 | 67.5† / 47.0 | 80.3† / 40.7 |
| 150,000 | — / 50.0 | 28.0 / 20.9 | 91.7† / 65.9 | 70.3† / 50.2 | 69.1† / 43.7 |
| 200,000 | — / 51.2 | 42.0† / 20.8 | 77.8 / 66.8 | 52.8† / 51.2 | 65.5† / 44.6 |
| 500,000 | — / 50.0 | 29.4† / 20.6 | 73.5 / 65.7 | 38.1 / 50.0 | 50.8 / 43.5 |
| 1,000,000 | — / 48.2 | 25.2 / 20.5 | 68.9 / 64.0 | 33.9 / 48.2 | 46.6 / 41.8 |

† `QUANTUM_DOMINATED`: the allocator figure is mostly the 2 MiB slab, not the index (§3.3).

![String memory across population](results/chart_string_memory_sweep.svg)

## 4. Latency tables, integer keys

#### Point lookup, 100% hit, integer keys (N = 1,000,000)

| Distribution | λ | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---|
| `clustered` | — | 116.46 | 21.99 | 5.406 [5.342, 5.474] | Expanse — `not pre-registered` |
| `random` | 15.3 | 116.40 | 40.04 | 2.949 [2.921, 2.978] | Expanse — `CONFIRMED` |
| `sequential` | — | 116.54 | 12.75 | 10.277 [9.579, 11.111] | Expanse — `CONFIRMED` |
| `sparse` | — | 117.43 | 9.86 | 13.537 [12.522, 14.764] | Expanse — `CONFIRMED` |

#### Point lookup, 50% hit / 50% rejection-sampled miss, integer keys (N = 1,000,000)

> **Measured with the corrected probe builder.** The hit-sampling defect
> [#760](https://github.com/orieg/expanse/issues/760) describes — the hit half of
> the stream drawn from `population[..hits_wanted]`, which the sort above confines
> to one end of the keyspace while the misses span all of it — was fixed in the
> shared builder before this artifact was measured. The cells below come from
> `b868fb2e`, which postdates that fix, so they need no re-measurement and carry
> no withheld figure.

| Distribution | λ | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---|
| `clustered` | — | 99.10 | 17.93 | 5.660 [5.579, 5.746] | Expanse — `not pre-registered` |
| `random` | 15.3 | 120.29 | 41.09 | 2.961 [2.927, 2.995] | Expanse — `not pre-registered` |
| `sequential` | — | 67.78 | 12.26 | 6.334 [5.855, 6.935] | Expanse — `CONFIRMED` |
| `sparse` | — | 67.96 | 9.78 | 7.846 [7.329, 8.523] | Expanse — `CONFIRMED` |

#### Insertion into a cold structure, integer keys (N = 1,000,000)

| Distribution | λ | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---|
| `clustered` | — | 20.40 | 20.63 | 0.989 [0.984, 0.995] | Masstree — **`UNPREDICTED LOSS`** |
| `random` | 15.3 | 20.54 | 27.18 | 0.754 [0.746, 0.760] | Masstree — **`UNPREDICTED LOSS`** |
| `sequential` | — | 20.60 | 12.42 | 1.656 [1.639, 1.672] | Expanse — `CONFIRMED` |
| `sparse` | — | 20.50 | 29.95 | 0.684 [0.676, 0.691] | Masstree — **`UNPREDICTED LOSS`** |

#### Ordered range scan, integer keys (N = 1,000,000; Masstree ÷ Expanse per visited element)

| Distribution | k=10 | k=100 | k=1000 |
|---|---:|---:|---:|
| `sequential` | 2.322 [2.217, 2.436] · **`REFUTED`** | 2.289 [2.199, 2.358] · **`REFUTED`** | 2.502 [2.453, 2.526] · `not pre-registered` |
| `clustered` | 2.412 [2.311, 2.522] · **`REFUTED`** | 2.258 [2.171, 2.324] · **`REFUTED`** | 2.454 [2.400, 2.476] · `not pre-registered` |
| `sparse` | 1.639 [1.540, 1.756] · **`REFUTED`** | 1.265 [1.232, 1.296] · **`REFUTED`** | 1.138 [1.124, 1.150] · `not pre-registered` |
| `random` | 1.577 [1.491, 1.675] · **`REFUTED`** | 1.458 [1.382, 1.542] · **`REFUTED`** | 1.496 [1.459, 1.520] · `not pre-registered` |

## 5. Latency tables, string keys

![String latency at N = 1M](results/chart_string_latency.svg)

#### Point lookup, 100% hit, string keys (N = 1,000,000)

| Shape | N held | mean len | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---:|---|
| `beyond` | 1,000,000 | 272.0 | withheld (1,000,000 keys > 255 B) | 334.17 | — | no Masstree cell (§3.4) |
| `counter` | 1,000,000 | 12.0 | 144.22 | 151.79 | 0.952 [0.948, 0.956] | Masstree — **`UNPREDICTED LOSS`** |
| `prefixed` | 1,000,000 | 120.0 | 306.36 | 274.19 | 1.119 [1.116, 1.122] | Expanse — **`REFUTED`** |
| `short` | 1,000,000 | 12.0 | 173.04 | 130.98 | 1.326 [1.322, 1.329] | Expanse — `CONFIRMED` |
| `skewed` | 998,150 | 14.3 | 203.81 | 139.21 | 1.467 [1.461, 1.472] | Expanse — `not pre-registered` |

#### Point lookup, 50% hit / 50% rejection-sampled miss, string keys (N = 1,000,000)

> **Measured with the corrected probe builder**, as the integer pillar above:
> the string builder's hit sampling was fixed
> ([#760](https://github.com/orieg/expanse/issues/760)) before `b868fb2e`.

| Shape | N held | mean len | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---:|---|
| `beyond` | 1,000,000 | 272.0 | withheld (1,000,000 keys > 255 B) | 302.44 | — | no Masstree cell (§3.4) |
| `counter` | 1,000,000 | 12.0 | 83.62 | 96.93 | 0.866 [0.863, 0.870] | Masstree — `not pre-registered` |
| `prefixed` | 1,000,000 | 120.0 | 290.95 | 239.45 | 1.218 [1.214, 1.221] | Expanse — **`REFUTED`** |
| `short` | 1,000,000 | 12.0 | 173.54 | 114.90 | 1.513 [1.508, 1.517] | Expanse — `not pre-registered` |
| `skewed` | 998,150 | 14.3 | 196.07 | 127.43 | 1.547 [1.542, 1.552] | Expanse — `not pre-registered` |

#### Insertion into a cold structure, string keys (N = 1,000,000)

| Shape | N held | mean len | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |
|---|---:|---:|---:|---:|---:|---|
| `beyond` | 1,000,000 | 272.0 | withheld (1,000,000 keys > 255 B) | 250.79 | — | no Masstree cell (§3.4) |
| `counter` | 1,000,000 | 12.0 | 24.63 | 56.87 | 0.433 [0.427, 0.438] | Masstree — **`UNPREDICTED LOSS`** |
| `prefixed` | 1,000,000 | 120.0 | 164.06 | 170.00 | 0.964 [0.959, 0.968] | Masstree — `CONFIRMED` |
| `short` | 1,000,000 | 12.0 | 52.11 | 106.88 | 0.479 [0.472, 0.485] | Masstree — `not pre-registered` |
| `skewed` | 998,150 | 14.3 | 65.97 | 104.14 | 0.634 [0.631, 0.638] | Masstree — `not pre-registered` |

#### Ordered range scan, string keys (N = 1,000,000; Masstree ÷ Expanse per visited element)

| Shape | k=10 | k=100 | k=1000 |
|---|---:|---:|---:|
| `short` | 0.424 [0.422, 0.427] | 0.256 [0.254, 0.259] | 0.227 [0.226, 0.227] |
| `counter` | 0.361 [0.359, 0.362] | 0.135 [0.134, 0.136] | 0.093 [0.093, 0.094] |
| `prefixed` | 1.528 [1.523, 1.534] | 0.635 [0.631, 0.638] | 0.407 [0.405, 0.408] |
| `skewed` | 0.382 [0.379, 0.384] | 0.206 [0.204, 0.207] | 0.172 [0.171, 0.173] |
| `beyond` | Expanse 108 ns; Masstree withheld | Expanse 44 ns; Masstree withheld | Expanse 37 ns; Masstree withheld |

`beyond` (272-byte keys) fails the §3.4 predicate for its whole population:
Masstree's declared contract is `MASSTREE_MAXKEYLEN = 255`, and the validation
gate records that the shim refuses every such key at the call. The Expanse
figures on that shape are published alone.

## 6. Sensitivity: insertion order and table configuration

#### Sensitivity (§10.2 insertion order, §10.3 table configuration) — both arms, same population

Sorted / single is the order the shared generator produces and the table configuration §10.3 assigns, which every cell above was built in. Shuffled is a Fisher–Yates permutation of the same keys: Masstree's leaf fill, footprint and insertion cost depend on the order, and so do Expanse's insertion cost and allocator footprint; Expanse's own node census (`mem_used`) is the one order-invariant figure. Concurrent is Masstree's fenced, spin-locked node version, the configuration the MC cells use, driven single-threaded here to show the protocol's own cost. Ratios are Masstree ÷ Expanse; no verdict is given against §6.

| Arm | Shape | Order | Table | N | Masstree allocator, settled (unsettled) | Masstree structural | leaf fill | Expanse allocator | Expanse `mem_used` | lookup_hit ratio [BCa 95%] | insert ratio [BCa 95%] |
|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| map | `random` | sorted | single | 1,000,000 | 23.08 (23.08) | 22.76 | 1.000 | 17.84 | 17.58 | 3.003 [2.977, 3.033] | 0.750 [0.743, 0.756] |
| map | `random` | sorted | concurrent | 1,000,000 | 23.08 (23.08) | 22.76 | 1.000 | 17.84 | 17.58 | 3.195 [3.167, 3.225] | 1.137 [1.131, 1.141] |
| map | `random` | shuffled | single | 1,000,000 | 33.57 (33.57) | 33.10 | 0.707 | 24.76 | 17.58 | 3.100 [3.067, 3.136] | 1.891 [1.874, 1.921] |
| str | `prefixed` | sorted | single | 1,000,000 | 69.08 (78.42) | 65.43 | 1.000 | 63.96 | 55.25 | 1.114 [1.106, 1.118] | 0.970 [0.966, 0.973] |
| str | `prefixed` | shuffled | single | 1,000,000 | 88.51 (92.63) | 84.20 | 0.706 | 66.74 | 55.25 | 1.124 [1.120, 1.126] | 1.297 [1.292, 1.300] |
| str | `short` | sorted | single | 1,000,000 | 33.91 (34.02) | 32.02 | 1.000 | 48.20 | 43.25 | 1.346 [1.342, 1.349] | 0.478 [0.472, 0.484] |
| str | `short` | sorted | concurrent | 1,000,000 | 33.91 (34.02) | 32.02 | 1.000 | 48.20 | 43.25 | 1.480 [1.477, 1.484] | 0.597 [0.585, 0.607] |
| str | `short` | shuffled | single | 1,000,000 | 50.62 (50.92) | 47.99 | 0.707 | 50.95 | 43.25 | 1.493 [1.485, 1.497] | 1.089 [1.087, 1.093] |

The shuffled rows are the regime the Step 0 gate measured and the §6
predictions leaned on; the sorted rows are the suite's cells in the shared
generator's order, with the table configuration §10.3 assigns. The
difference between them is the finding, and it cuts both ways: **Masstree's
insertion cost (20.4 → 124.9 ns on `random`), leaf fill (1.000 → 0.707) and
footprint (23.08 → 33.57 B/key) depend on insertion order — and so do
Expanse's insertion cost (27.2 → 66.5 ns) and its allocator footprint (17.84
→ 24.76 B/key), while Expanse's own node census (`mem_used`, 17.58 in both
orders) is the one figure that is order-invariant.** The 7 B/key the allocator
holds beyond `mem_used` on the shuffled build is capacity the engine
instrument does not see; its cause is unmeasured. Lookup ratios move far less
with order than insertion does (3.003 sorted, 3.100 shuffled on `random`), but
every latency verdict above is a sorted-order verdict all the same. The
concurrent-table rows show the protocol's own single-threaded cost: on
`random` integer keys the insertion ratio moves from 0.750 [0.743, 0.756] with
the single-threaded table to 1.137 [1.131, 1.141] with the concurrent one, and
the lookup ratio from 3.003 [2.977, 3.033] to 3.195 [3.167, 3.225] — which is
why the single-threaded pairings use the single-threaded configuration
(§10.3) *(workload: `masstree_map_64bit`)*.

#### Why a string lookup costs about four times a `u64` lookup (#724)

A 12-byte `counter` string key costs `ExpanseStrMap` 151.79 ns at N = 10⁶
where a uniform-random `u64` key costs `ExpanseMap` 40.04 ns — 3.79× at
`b868fb2e` *(workloads differ: `masstree_str_map` vs `masstree_map_64bit`)*. That gap had no measured
mechanism: the counter cells above run **both engines in one process**, and
`perf stat` counts the process, so they cannot attribute anything to one side.

The question is Expanse-internal, so it does not need the competitor.
`crates/expanse/examples/perf_point_lookup.rs` gained `strmap_get` (the
suites' `short` shape) and `strmap_get_counter` beside its existing `map_get`,
and `perf stat` wraps one engine at a time *(measured: reference host, commit
`43c68caa`, `results/counters_strmap_vs_map_lookup_1m.json`; 7 paired runs,
phase-differenced, BCa 95% intervals, every event at 100% `running` — nothing
multiplexed)*:

| per probe, N = 10⁶, 100% hit | `map_get` (u64) | `strmap_get` (`short`) |
|---|---:|---:|
| `cycles` | 186.32 [177.46, 190.85] | 670.49 [658.56, 689.20] |
| `instructions` | 179.98 [179.85, 180.14] | 298.54 [297.96, 299.15] |
| `L1-dcache-load-misses` | 4.235 [4.195, 4.267] | 12.299 [12.254, 12.333] |
| **`LLC-load-misses`** | 0.031 [-0.028, 0.054] | 2.558 [2.539, 2.589] |
| `dTLB-load-misses` | 0.704 [0.692, 0.719] | 3.878 [3.857, 3.923] |
| `branch-misses` | 2.181 [2.175, 2.184] | 2.021 [2.015, 2.025] |

**The gap is memory-latency-bound, not instruction-bound.** `cycles` rise
3.6× while `instructions` rise 1.66×. What rises with the cycles is
`LLC-load-misses` — from a figure indistinguishable from zero to 2.558 per
probe — and `dTLB-load-misses`, 0.704 → 3.878. `branch-misses` do not move at
all. Anything that removes instructions from this path is optimising the term
that is not the cost.

**No ratio is quoted for `LLC-load-misses`, and that is the honest reading.**
The `u64` arm's interval spans zero: at this population its tree is
effectively cache-resident and the phase difference cannot separate it from
nothing. Two runs put the point estimate at 0.055 and 0.031, so a ratio would
read 46× or 82× on run-to-run noise in a near-zero denominator. The absolute
figures are what reproduce — every other cell in this table matched within
2.1% across those two runs — and the absolutes are the finding: a string
lookup misses last-level cache about two and a half times per probe where a
`u64` lookup essentially never does.

**`strmap_get_counter` separates the leaf from the rest.** Its keys resolve
inside a terminal chunk and allocate no suffix leaf, and it costs 796.48
cycles, 509.91 instructions and **1.156** `LLC-load-misses` per probe. So of
the 2.558 misses a `short` lookup takes, roughly 1.4 are the suffix leaf — the
dependent load past the node — and about 1.16 are already there without one.
**The leaf is under half of it**, which is not what the issue assumed: most of
the miss cost is the string tree's own shape. (`counter` retires more
instructions than `short` and is the slower arm in cycles despite fewer
misses. That is its own question and not this one.)

**The huge-page pair is measured, and it refutes the page-size hypothesis.**
[#724](https://github.com/orieg/expanse/issues/724) registered a sensitivity
pair on the strength of that 0.704 → 3.878 `dTLB-load-misses` figure. Running
it needs no engine change: `GLIBC_TUNABLES=glibc.malloc.hugetlb=1` makes
glibc `madvise(MADV_HUGEPAGE)` its arenas, so the shipped code is what gets
measured. The host runs THP in `madvise` mode, and **the treatment was verified
before it was interpreted** — the probe's own `AnonHugePages` went 0 kB →
573 MB under the tunable, so a null result could not have been an inert knob
*(measured: reference host, commit `b1868813`,
`results/counters_strmap_hugepage_off_1m.json` and
`…_on_1m.json`; 7 paired runs each, same events, same quiet host)*:

| per probe, N = 10⁶ | 4 KiB pages | huge pages | change |
|---|---:|---:|---:|
| `strmap_get` `cycles` | 670.08 [662.04, 682.83] | 594.79 [571.97, 624.22] | −11.2% |
| `map_get` `cycles` | 194.97 [187.67, 207.68] | 180.93 [175.88, 189.62] | −7.2% |
| `strmap_get` `dTLB-load-misses` | 3.878 [3.843, 3.912] | **0.002** [0.000, 0.005] | −99.9% |
| `map_get` `dTLB-load-misses` | 0.699 [0.676, 0.715] | **0.001** [-0.000, 0.001] | −99.9% |
| `strmap_get` `LLC-load-misses` | 2.576 [2.553, 2.631] | 2.899 [2.833, 2.969] | +12.5% |
| `strmap_get` `L1-dcache-load-misses` | 12.343 [12.269, 12.424] | 8.556 [8.267, 8.981] | −30.7% |
| **string ÷ `u64`, in cycles** | **3.44×** | **3.29×** | **−4.3%** |

Translation misses go to **zero** on both arms and the gap does not close:
3.44× → 3.29×. The misses were real and removing them is worth about 11% of
the string arm, but **they were not the cost** — 96% of the gap survives their
complete elimination. An `madvise(MADV_HUGEPAGE)` change to the engine's
arenas would buy a real, small win and would not address what #724 is about.
`instructions` are flat to four digits on every arm, which is the control
confirming the two runs executed identical work and only the page mapping
differed.

**What it leaves.** With translation eliminated as a variable, the surviving
memory signal is unambiguous: `LLC-load-misses` 2.899 against `map_get`'s
0.107, and `L1-dcache-load-misses` 8.556 against 3.918, both under huge pages.
Read with `strmap_get_counter` — no suffix leaf, still 1.156 `LLC-load-misses`
— the remaining target is the **string tree's descent and node layout**,
neither the leaf nor the allocator. That `LLC-load-misses` rose 12.5% under
huge pages while L1 misses fell 30.7% is recorded as observed; no counter run
covers why, and this suite does not guess (§8.9).

**The per-function ranking, which §6 requires before any change to this path.**
Callgrind with cache simulation over the same probe at the same population,
phase-differenced so only the lookup loop is attributed. Two things make it
readable as an attribution rather than as numbers: the last-level cache is
forced to 32 MiB / 16-way — the nearest geometry Callgrind accepts to the
reference host's 30 MiB L3, since it requires a power-of-two set count — and
the simulation is checked against the machine before it is believed.
`Ir` comes out at 179.66 and 296.07 per probe against 179.98 and 298.54
measured, **within 0.8%**, and the string-to-`u64` miss ratio at 77× against
83× measured *(measured: reference host counters at `b1868813`; Callgrind at
`ee6b4290`, `results/callgrind_strmap_lookup/`)*.

| `DLmr` per probe, lookup loop only | `map_get` | `strmap_get` | share of the string arm |
|---|---:|---:|---:|
| `ExpanseStrMap::get` — the chunk cascade | — | 0.961 | 25.1% |
| `__memcpy_avx_unaligned_erms` — `chunk_at` | — | 0.830 | 21.7% |
| `leaf::search` | — | 0.633 | 16.5% |
| `get_map_popcnt` | 0.029 | 0.516 | 13.5% |
| the probe's own key vector | — | 0.331 | 8.6% |
| **total** | **0.050** | **3.835** | |

**There is no hot spot, and that is the finding.** The misses spread across
four functions at 13–25% each. No `StrSuffix` dereference dominates — which
agrees with [#723](https://github.com/orieg/expanse/issues/723) having already
collapsed that leaf into one allocation, with `strmap_get_counter` taking 1.156
misses while allocating no leaf at all, and with the huge-page pair above. Three
instruments now say the same thing: this is not one idiom, it is the shape of
the descent.

**A fifth of the cost is reading the key, not walking the tree.** `chunk_at`
copies each 8-byte slice into a `[u8; 8]` with `copy_from_slice`, twice for a
12-byte key, and those reads touch the caller's key bytes — a million separate
allocations in this probe. That is a property of the byte-slice API rather than
of the trie, and it is the one line item here that a caller's own key storage
can move.

**`get_map_popcnt` is the shared word-map node search**, and it is the only
function that appears in both arms: 0.029 misses on `u64` against 0.516 on
strings, 18× for being entered at every cascade level over a larger and
sparser node set.

**What Callgrind cannot say.** Its cache is an LRU model with no prefetcher and
no adjacent-line fetch, so the absolute counts above are not the machine's —
the reference host measures 2.558 `LLC-load-misses` where this simulates 3.835.
The ranking is what is claimed, not the levels, and the `Ir` agreement is the
evidence that the ranking is worth reading.

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

> **Measured at `b868fb2e`, two runs.** Every cell in this section — MC1 and
> MC2, C1, C2, the H health cells and the M census — was taken twice under the
> standing conditions (host lock, P-core pin, a load snapshot per cell with the
> runner's own CPU subtracted, foreign load ≤ 0.02 core-equivalents at every
> cell). The throughput and census tables below are run 1; run 2 is
> [`results/baseline_concurrent_run2.json`](results/baseline_concurrent_run2.json),
> the H table carries both, and "Between-run spread" at the end of this section
> reads the two against each other. The figures this section published before
> were measured on the single-writer engine and are superseded; the `a1982ff2`
> pair among them is kept at `results/step0/`, the data
> `docs/benchmarks/concurrency/README.md` §3–§5 and §8 read.

#### MC1 — `u64` keys, Masstree vs `SyncExpanseMap`

**C1 — writer throughput as writer count scales** (W writers insert 2²⁰ fresh keys into a 2²⁰ prefill; fixed work; Expanse ÷ Masstree)

| W | Masstree M/s | Expanse M/s | ratio [BCa 95%] | verdict |
|--:|---:|---:|---|---|
| 1 | 5.18 | 5.27 | 1.004 [0.977, 1.028] | `BOUNDARY_RESULT` — `CONFIRMED` |
| 2 | 9.87 | 6.67 | 0.679 [0.667, 0.691] | Masstree — `CONFIRMED` |
| 4 | 18.48 | 8.97 | 0.479 [0.470, 0.488] | Masstree — `CONFIRMED` |
| 8 | 32.76 | 11.68 | 0.356 [0.349, 0.366] | Masstree — `CONFIRMED` |
| 16 | 34.57 | 10.66 | 0.289 [0.266, 0.305] | Masstree — `not pre-registered` |

**C2 — reader throughput alongside writers** (8 readers probe 50/50 while W writers insert; W = 0 is the reader-only reference; the reader window is the writers' fixed work, so the two arms' windows differ in length by the writer ratio and the population grows at different rates inside them)

| W | Masstree readers M/s | Expanse readers M/s | ratio [BCa 95%] | verdict | Masstree writers M/s | Expanse writers M/s | writer ratio |
|--:|---:|---:|---|---|---:|---:|---|
| 0 | 59.30 | 107.12 | 1.829 [1.812, 1.856] | Expanse — `CONFIRMED` | — | — | — |
| 1 | 41.38 | 81.50 | 1.967 [1.933, 2.002] | Expanse — **`REFUTED`** | 3.86 | 2.98 | 0.801 [0.746, 0.887] |
| 2 | 38.97 | 76.95 | 1.951 [1.913, 1.979] | Expanse — **`REFUTED`** | 7.49 | 4.51 | 0.605 [0.578, 0.632] |
| 4 | 36.02 | 70.19 | 1.937 [1.895, 1.974] | Expanse — **`REFUTED`** | 14.06 | 6.66 | 0.476 [0.465, 0.487] |
| 8 | 31.44 | 60.35 | 1.957 [1.893, 2.023] | Expanse — **`REFUTED`** | 24.61 | 9.39 | 0.419 [0.387, 0.469] |

#### MC2 — `short` string keys, Masstree vs `SyncExpanseStrMap`

**C1 — writer throughput as writer count scales** (W writers insert 2²⁰ fresh keys into a 2²⁰ prefill; fixed work; Expanse ÷ Masstree)

| W | Masstree M/s | Expanse M/s | ratio [BCa 95%] | verdict |
|--:|---:|---:|---|---|
| 1 | 4.23 | 3.90 | 0.879 [0.801, 0.952] | Masstree — `CONFIRMED` |
| 2 | 7.98 | 2.63 | 0.340 [0.319, 0.362] | Masstree — `CONFIRMED` |
| 4 | 14.88 | 2.32 | 0.161 [0.151, 0.174] | Masstree — `CONFIRMED` |
| 8 | 24.23 | 2.04 | 0.086 [0.073, 0.097] | Masstree — `CONFIRMED` |
| 16 | 28.32 | 0.48 | 0.017 [0.016, 0.018] | Masstree — `not pre-registered` |

**C2 — reader throughput alongside writers** (8 readers probe 50/50 while W writers insert; W = 0 is the reader-only reference; the reader window is the writers' fixed work, so the two arms' windows differ in length by the writer ratio and the population grows at different rates inside them)

| W | Masstree readers M/s | Expanse readers M/s | ratio [BCa 95%] | verdict | Masstree writers M/s | Expanse writers M/s | writer ratio |
|--:|---:|---:|---|---|---:|---:|---|
| 0 | 34.27 | 36.92 | 1.079 [1.077, 1.081] | Expanse — `CONFIRMED` | — | — | — |
| 1 | 25.54 | 1.08 | 0.042 [0.040, 0.044] | Masstree — `CONFIRMED` | 2.89 | 1.93 | 0.712 [0.654, 0.778] |
| 2 | 23.46 | 4.40 | 0.189 [0.172, 0.207] | Masstree — `CONFIRMED` | 5.52 | 1.61 | 0.331 [0.304, 0.364] |
| 4 | 21.10 | 4.81 | 0.233 [0.227, 0.241] | Masstree — `CONFIRMED` | 8.34 | 1.51 | 0.180 [0.170, 0.195] |
| 8 | 17.84 | 4.61 | 0.269 [0.259, 0.283] | Masstree — `CONFIRMED` | 13.72 | 1.51 | 0.108 [0.103, 0.115] |

#### H — protocol health, Expanse side only (occ-stats build; event ratios, never a timing)

| Arm | W | R | run | restart share, median [min, max] | fallback share, median | `sample_spins` ÷ `read_ops` (medians) | `locked_reads` ÷ `read_ops` | unconditional lock share | handoffs ÷ write | branch replacements ÷ write | deep-cascade share | root-rewrite share | spin time ÷ reader wall | §6.3 |
|---|--:|--:|--:|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| map | 1 | 8 | 1 | 0.13% [0.13%, 0.17%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.057 | 2.84% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 1 | 8 | 2 | 0.13% [0.13%, 0.17%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.057 | 2.84% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 1 in a round) |
| map | 2 | 8 | 1 | 0.19% [0.18%, 0.19%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.058 | 2.84% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks) |
| map | 2 | 8 | 2 | 0.20% [0.19%, 0.20%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.058 | 2.84% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 2 in a round) |
| map | 4 | 8 | 1 | 0.27% [0.26%, 0.28%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.061 | 2.84% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 5 in a round) |
| map | 4 | 8 | 2 | 0.27% [0.26%, 0.28%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.061 | 2.84% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 6 in a round) |
| map | 8 | 8 | 1 | 0.41% [0.39%, 0.42%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.067 | 2.84% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 9 in a round) |
| map | 8 | 8 | 2 | 0.41% [0.40%, 0.43%] | 0.0000% | 0.00 | 0.00% | 0.00% | 0.000 | 0.067 | 2.84% | 0.00% | 0.00% | rise with W: `CONFIRMED`; fallback 0.0000% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 6 in a round) |
| str | 1 | 8 | 1 | 92.66% [89.34%, 93.69%] | 0.4496% | 104.78 | 0.45% | 0.00% | 0.000 | 0.000 | 0.00% | 0.00% | 68.77% | rise with W: **`REFUTED`**; fallback 0.4496% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 8,350 in a round) |
| str | 1 | 8 | 2 | 92.80% [92.03%, 93.36%] | 0.2447% | 102.77 | 0.24% | 0.00% | 0.000 | 0.000 | 0.00% | 0.00% | 70.06% | rise with W: **`REFUTED`**; fallback 0.2447% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 2,101 in a round) |
| str | 2 | 8 | 1 | 65.41% [61.58%, 73.52%] | 0.0069% | 19.13 | 0.01% | 0.00% | 0.151 | 0.000 | 0.00% | 0.00% | 62.63% | rise with W: **`REFUTED`**; fallback 0.0069% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 1,215 in a round) |
| str | 2 | 8 | 2 | 65.83% [61.44%, 73.67%] | 0.0072% | 18.26 | 0.01% | 0.00% | 0.111 | 0.000 | 0.00% | 0.00% | 60.94% | rise with W: **`REFUTED`**; fallback 0.0072% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 1,045 in a round) |
| str | 4 | 8 | 1 | 62.48% [61.78%, 64.45%] | 0.0002% | 17.42 | 0.00% | 0.00% | 0.156 | 0.000 | 0.00% | 0.00% | 59.78% | rise with W: **`REFUTED`**; fallback 0.0002% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 16 in a round) |
| str | 4 | 8 | 2 | 62.93% [62.28%, 66.72%] | 0.0007% | 16.77 | 0.00% | 0.00% | 0.114 | 0.000 | 0.00% | 0.00% | 59.58% | rise with W: **`REFUTED`**; fallback 0.0007% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 565 in a round) |
| str | 8 | 8 | 1 | 63.56% [62.93%, 64.18%] | 0.0036% | 18.45 | 0.00% | 0.00% | 0.143 | 0.000 | 0.00% | 0.00% | 60.03% | rise with W: **`REFUTED`**; fallback 0.0036% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 194 in a round) |
| str | 8 | 8 | 2 | 64.33% [62.00%, 67.43%] | 0.0025% | 18.69 | 0.00% | 0.00% | 0.116 | 0.000 | 0.00% | 0.00% | 60.57% | rise with W: **`REFUTED`**; fallback 0.0025% median, below 1% — `CONFIRMED` (fallbacks recorded, at most 333 in a round) |

The MC1 health rows: the restart share sits at 0.13%–0.41% of walks and
rises with writer count in both runs — **`CONFIRMED`** in each, so on this
pair the first half of §6.3 is settled rather than direction-only.
`sample_spins` and spin time ÷ reader wall read zero at the median of every MC1
cell. Readers did take the writer mutex: at most 9 fallbacks in a round
(W = 8, run 1), against a median fallback share that rounds to 0.0000%, so
the second half is a measured **`CONFIRMED`** wherever a round recorded a
fallback and **`PASS_categorical_by_design`** in the 2 cells where none did (W = 1
and W = 2 in run 1) — a fallback needs 64 consecutive failed walks, and a
falsifier that cannot fire is not a measurement.

The MC2 rows: the string reader restarts on 92.66%–92.80% of walks with one
writer and 62.48%–65.83% with two to eight, so its share falls from one
writer to two and §6.3's rise is **`REFUTED`** in both runs. Its median
fallback share is 0.4496% (run 1) and 0.2447% (run 2) at W = 1 and at most
0.0072% at W ≥ 2 — below the 1% starvation line in both runs, a measured
**`CONFIRMED`**, with one run-1 round at W = 1 reaching 1.16%.
`sample_spins` per read op is 102.77–104.78 at W = 1 and 16.77–19.13 at W ≥ 2,
and spin time ÷ reader wall 59.58%–70.06%.

What these counters cost is **not** measured here. They are event ratios from
the counting build, a separate binary from the timing build, and no cell in
this suite attributes a nanosecond of the string reader's loss to a restart, a
spin or a fallback.

#### M — build-only single-writer census, Masstree vs `SyncExpanseMap` (B/key)

| λ | N | Masstree allocator | SyncExpanseMap allocator | Masstree structural | Expanse `mem_used` | flag |
|---:|---:|---:|---:|---:|---:|---|
| 1 | 65,536 | 32.19 | 25.50 | 22.78 | 23.65 | `QUANTUM_DOMINATED` |
| 2 | 131,072 | 32.09 | 24.85 | 22.77 | 24.09 | `QUANTUM_DOMINATED` |
| 4 | 262,144 | 24.05 | 23.02 | 22.76 | 22.28 | `ok` |
| 8 | 524,288 | 24.02 | 19.42 | 22.76 | 19.07 | `ok` |
| 15 | 983,040 | 23.48 | 17.94 | 22.76 | 17.60 | `ok` |
| 23 | 1,507,328 | 23.66 | 17.66 | 22.76 | 17.18 | `ok` |
| 30 | 1,966,080 | 23.47 | 20.92 | 22.76 | 19.49 | `ok` |
| 38 | 2,490,368 | 23.58 | 25.95 | 22.76 | 23.30 | `ok` |
| 46 | 3,014,656 | 22.96 | 26.78 | 22.76 | 24.01 | `ok` |
| 61 | 3,997,696 | 23.09 | 25.59 | 22.76 | 23.17 | `ok` |

**The Expanse column beside #692's.** MC1's cells are the construction of
`hot_comparison` §11.4 — same prefill, fresh-key count and thread placement —
so the `SyncExpanseMap` column here is a second measurement of that suite's map
arm, at the same commit `b868fb2e` and in another process: single writer
5.05–5.27 M/s here against 3.86–3.87 there, eight writers 11.50–11.68 against 10.78–10.82, sixteen
10.66–10.71 against 10.14–10.28, eight readers alone 106.78–107.12 against 104.25–104.38 (two runs each)
*(workloads differ: `masstree_conc_map_64bit` vs `hot_rowex_map_64bit`; not a
§8.4 paired claim)*. No tolerance was registered for this comparison, so it
carries **no replication verdict**: the single-writer levels differ by about a
third and the others by a few per cent, and what separates the two suites'
single-writer cells is **unmeasured**.

**Between-run spread.** None of the 28 C1 and C2 ratio cells separates between
the two runs at `b868fb2e` — each run's interval overlaps the other's — and no
verdict differs. Per [`docs/BENCHMARKING.md`](../../BENCHMARKING.md) rule 18
the claim ceiling on a concurrent cell is still the union of its two runs'
intervals, so the levels in the tables are run 1's and §1 and §2 quote both.
The census's Masstree and `mem_used` columns are identical between the runs;
the `SyncExpanseMap` allocator column differs by at most 0.0074 B/key, a
difference whose cause is unmeasured.

## 8. Scorecard against the pre-registration

#### Scorecard (wall-clock cells with a Masstree column)

| | Count |
|---|---:|
| Expanse wins (CI excludes parity) | 89 |
| Masstree wins (CI excludes parity) | 78 |
| `BOUNDARY_RESULT` | 5 |
| Masstree column withheld (§3.4, `beyond`) | 18 |

| Label | Cells |
|---|---:|
| Masstree — `CONFIRMED` | 56 |
| Expanse — **`REFUTED`** | 35 |
| Expanse — `not pre-registered` | 32 |
| Expanse — `CONFIRMED` | 22 |
| Masstree — `not pre-registered` | 13 |
| Masstree — **`UNPREDICTED LOSS`** | 9 |
| `BOUNDARY_RESULT` | 4 |
| `BOUNDARY_RESULT` — `CONFIRMED` | 1 |

The scorecard counts run 1 of the single-threaded cells at `b868fb2e` together
with the concurrent cells at `a1982ff2`.

**Two runs.** Every single-threaded cell was measured twice at `b868fb2e` on
the same host; the tables are run 1, and run 2 is not committed. Of the 144
single-threaded ratio cells with a Masstree column, 140 name the same winner,
or the same `BOUNDARY_RESULT`, in both runs, and so do all 16 sensitivity
ratios; 27 of those 160 intervals do not overlap between the runs, which is
the between-run spread rule 18 records, and no claim above compares one run's
interval against the other's. The four cells the runs disagree on are all
integer cells and are quoted as a direction only: `sequential` insertion at
N = 10⁴ (1.171 [0.984, 1.556], then 1.183 [1.001, 1.561]), `clustered`
insertion at N = 10⁶ (0.989 [0.984, 0.995], then 0.996 [0.990, 1.003]), and
the `random` k = 10 scan at N = 10⁴ (1.006 [1.003, 1.010], then 0.935
[0.932, 0.939]) and at N = 10⁵ (0.957 [0.948, 0.962], then 1.005
[0.991, 1.016]) *(workload: `masstree_map_64bit`)*. With run 2's cells the
scorecard reads 86 Expanse wins, 80 Masstree wins and 6 `BOUNDARY_RESULT`,
which is exactly those four cells moving. Expanse's columns in the §3 censuses
reproduce to the digit and its sensitivity memory columns within 0.01 B/key;
Masstree's §3 columns differ only on `prefixed` at N = 10⁶.

#### Against the pre-registration

This table is written by hand — a verdict against a registered prediction is a
judgement, not a count — which is why it lives under its own heading rather than
inside the generated scorecard above. `check_readme_tables.py` matches a
section's tables against the generator's positionally, and a third table the
generator does not emit made that match ambiguous.

| Registered (§6) | Outcome |
|---|---|
| Masstree wins C1 at W ≥ 2, both arms (high) | **CONFIRMED** on every cell |
| Masstree wins or `BOUNDARY_RESULT` at W = 1 (medium-low) | **CONFIRMED** on both: strings a Masstree win (0.879–0.902), integers `BOUNDARY_RESULT` (0.986–1.004) in both runs |
| Masstree wins readers under writers (medium-high) | **CONFIRMED** on strings (0.042–0.269); **REFUTED in Expanse's favour** on integers (1.924–1.987), every cell in both runs |
| Masstree wins integer scan at k = 10, 100 (medium-high) | **REFUTED** on 20 of 24 cells in both runs (21 in run 1); **CONFIRMED** on `random` at k = 100 at 10⁴ and 10⁵ in both runs; the `random` k = 10 cells at 10⁴ and 10⁵ are direction-only (run 1 `REFUTED` and `CONFIRMED`, run 2 `CONFIRMED` and `BOUNDARY_RESULT`), and the earlier `7fe02c0b` measurement had the 10⁵ cell `REFUTED` |
| Masstree wins string scan, every k (high) | **CONFIRMED** on 33 of 36; **REFUTED** on `prefixed` at k = 10, all three populations, after [#722](https://github.com/orieg/expanse/issues/722) gave `ExpanseStrMap` a cursor. Every scan cell moved 1.4×–10.9× in Expanse's favour; the prediction survives on the rest |
| Masstree wins `prefixed` lookup and insert (low) | insert **CONFIRMED** at 10⁶, **REFUTED** at 10⁵ (1.088 [1.074, 1.107], both runs; `BOUNDARY_RESULT` in the earlier `7fe02c0b` measurement), `BOUNDARY_RESULT` at 10⁴; lookup **REFUTED** (1.119 [1.116, 1.122] at 10⁶) |
| Masstree wins `short` / `skewed` index memory (medium) | `short` **CONFIRMED** (33.91 against 48.20); `skewed` **REFUTED** after [#723](https://github.com/orieg/expanse/issues/723) — 46.63 against 41.76, Expanse ahead. At the pre-registration's leaf it was 46.63 against 47.74, a Masstree win by 1.1 B/key; that figure is superseded, not overwritten |
| Expanse wins `random` memory, λ ∈ [8, 23] (high) | **CONFIRMED** |
| Expanse wins `random` memory outside the band (medium) | **UNPREDICTED LOSS** at λ ≥ 38 (by magnitude, no interval); parity-by-magnitude at λ = 4; wins at λ = 30 |
| Expanse wins `sequential` / `clustered` / `sparse` memory (high / medium) | **CONFIRMED** |
| Expanse wins integer lookup on `sequential`, `sparse`, `random` (medium-high / medium) | **CONFIRMED**, 2.9×–13.5× at 10⁶ |
| Expanse wins integer insert (medium) | **CONFIRMED** on `sequential` at 10⁵ and 10⁶, direction-only at 10⁴; **UNPREDICTED LOSS** on `random` and `sparse` in sorted order; `clustered` direction-only at 10⁶ (run 1 `UNPREDICTED LOSS`, run 2 `BOUNDARY_RESULT`) and `BOUNDARY_RESULT` at 10⁵ in both runs, where the earlier `7fe02c0b` measurement had an `UNPREDICTED LOSS` |
| Expanse wins `counter` lookup and insert at 10⁶ (high) | **UNPREDICTED LOSS** on all three cells (100%-hit lookup, 50/50 lookup, insert) in both runs; the 100%-hit lookup was `BOUNDARY_RESULT` in the earlier `7fe02c0b` measurement |
| Expanse wins `short` 100%-hit lookup (low-medium) | **CONFIRMED** |
| Expanse wins `counter` / `prefixed` index memory (medium) | **CONFIRMED** on both (by magnitude; `prefixed` 63.97 against 68.93). This README earlier recorded `prefixed` as an **UNPREDICTED LOSS** |
| Expanse wins reader-only C2 (medium) | **CONFIRMED** on both: integers 1.829–1.853, strings 1.077–1.079 |
| H: restart share rises with W; fallback share < 1% at W ≤ 8 (§6.3) | restart share **CONFIRMED** on integers in both runs, **REFUTED** on strings in both (it falls from W = 1 to W = 2); fallback share below 1% on both arms in both runs — a measured **CONFIRMED** where fallbacks were recorded, **`PASS_categorical_by_design`** in the two integer cells where none were (§7) |

The generated wall-clock scorecard above counts 9 `UNPREDICTED LOSS` cells (registered Expanse wins that Masstree took)
and 35 `REFUTED` cells (registered Masstree wins that Expanse took — every `REFUTED` in the derived tables is in Expanse's favour by construction)
against 79 `CONFIRMED` (56 Masstree, 22 Expanse, 1 `BOUNDARY_RESULT`); the memory cells, labelled by magnitude, are outside that count and
add three `UNPREDICTED LOSS` cells at λ ≥ 38. **Insertion order is the one cause that was
measured**, and it is measured for one cell: the `random` integer insert
that is an `UNPREDICTED LOSS` sorted (0.754) and an Expanse win shuffled
(1.891). The registration was informed by a shuffled-order Step 0 build and
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
