# RocksDB MemTable suite: pre-registration and measurement discipline

*(measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, Linux 6.8, run [33398474866](https://github.com/orieg/expanse/actions/runs/33398474866), commit `6cb64b45`; 100,000 keys, 16-byte key, 64-byte value payload; 5 rounds with BCa 95% bootstrap intervals; memory via deterministic seeded byte accounting; artifact [`results/baseline_rocksdb.json`](results/baseline_rocksdb.json).)*

Point-in-time gate record for the RocksDB pluggable MemTable comparative suite (#372, #382). Frozen: the pre-registration and baseline corrections below quote the issues and discussions before the reference-host runs, with empirical outcomes appended alongside them, never reconciled in place (AGENTS.md §8.7). Results and their reading live in [`README.md`](README.md); the C++ implementation and build flow in [`integrations/rocksdb/`](../../../integrations/rocksdb/README.md).

## 1. What the suite asks

1. What is the true RAM key density of `ExpanseMemTableRep` against a production-grade variable-height skiplist baseline (RocksDB `InlineSkipList` equivalent), after retracting the fat-node strawman (#372)?
2. How does sequential iteration (`prefixscan` Iterator and `ScanBatch`) perform when traversing contiguous 64-byte leaf blocks via sibling chaining vs traversing randomized skiplist towers?
3. What is the throughput edge for point lookups (`readrandom`), range seeks (`seekrandom`), and random inserts (`fillrandom`)?
4. Where does an unordered append-vector ceiling (`VectorRep`) sit in terms of throughput and density relative to ordered indices?

## 2. Pre-registered hypotheses and their outcomes

| # | pre-registered (source, before the run) | outcome | verdict |
|---|---|---|---|
| H1 | Memory density / fair baseline: earlier ~11× (146.7 B/entry) strawman retracted (#372); fair variable-height node costs 8 B key ptr + height×8 B tower ($E[\text{height}]=4/3 \to 18.7$ B/entry). Expanse predicted 13.2 B/entry (1.42× higher key density vs ordered skiplist; VectorRep 10.5 B/entry is denser than both) | 13.2 B/entry (Expanse) vs 18.7 B/entry (fair SkipList) vs 10.5 B/entry (VectorRep); 1.42× higher key density over the ordered baseline | confirmed |
| H2 | Sequential scan: intrusive sibling leaf block chaining avoids skiplist pointer chasing and delivers higher iteration throughput (post-#372 re-measurement target, #382 item 5) | `prefixscan` 154.18 Mops/s [151.17, 156.15] vs 46.29 Mops/s [44.28, 47.86] -> 3.331× [3.198, 3.486]; batch scan 116.82 Mops/s -> 2.524× [2.421, 2.644] | confirmed |
| H3 | Point lookup and range seek: $O(k)$ digital trie descent and cache-line binary search within leaf blocks beats pointer-chasing skiplist descent (#382 item 5) | `readrandom` 3.79 Mops/s [3.76, 3.81] vs 2.60 Mops/s [2.58, 2.61] -> 1.457× [1.444, 1.470]; `seekrandom` 3.67 Mops/s [3.62, 3.70] vs 2.43 Mops/s [2.37, 2.45] -> 1.512× [1.492, 1.546] | confirmed |
| H4 | Random ingestion: synchronized leaf insertion with automatic block split maintains competitive insertion throughput against skiplist (#382 item 5) | `fillrandom` 4.42 Mops/s [4.36, 4.53] vs 3.15 Mops/s [3.12, 3.16] -> 1.406× [1.385, 1.442] | confirmed |
| H5 | Unordered ceiling: `VectorRep` append vector will win insert and scan by design due to contiguous unindexed layout, but cannot serve ordered seeks | `VectorRep` achieves 202.65 Mops/s insert, 614.20 Mops/s scan, and 10.5 B/entry density; seekrandom scan is 3.94 Mops/s vs Expanse 3.67 Mops/s | confirmed |

## 3. Measurement discipline

- **Instrument & Runner**: `benches/bench_memtable.cc` built `-O3` against release `libexpanse.so` on the dedicated reference host (Intel i9-12900F, 24 threads, 30 MiB L3, Linux 6.8.0; commit `6cb64b45`, run [33398474866](https://github.com/orieg/expanse/actions/runs/33398474866)). 100,000 keys, 16-byte key, 64-byte value payload.
- **Statistical Processing**: 5 rounds harvested by `scripts/rocksdb_bench_harvest.py` into [`results/baseline_rocksdb.json`](results/baseline_rocksdb.json). Throughput metrics evaluated as mean with BCa 95% bootstrap intervals (2,000 resamples, seed 42); speedup ratios computed via two-sample BCa ratio intervals (`bca_bootstrap_ratio_ci`).
- **Deterministic Byte Accounting**: Memory footprint per key is evaluated via deterministic allocator instrumentation (M1 8-core, Apple clang 21, `-O3`, reproduced across runs).
- **Symmetric Baselines**: `ReferenceSkipListRep` models a realistic `InlineSkipList` variable-height tower allocation (`Node* next[1]` over-allocated by `height`), with identical `BenchBytewiseComparator` key comparator and memory allocator. `VectorRep` models an unindexed append vector.
- **Retractions and Corrections Handled**: #372 retracted the 146.7 B/entry fat-node skiplist strawman and the resulting 11.1× headline. #382 item 5 re-measured wall-clock throughput on the quiet reference host with BCa intervals.
- **Provenance**: Artifact [`results/baseline_rocksdb.json`](results/baseline_rocksdb.json) contains all raw round samples, bootstrap intervals, and provenance metadata.

## 4. Not covered

- End-to-end LSM SSTable flush / compaction write amplification (`db_bench` integration; flush reduction is an inferred target).
- Multi-threaded concurrent write contention (`InsertConcurrently`), and the read path's serialisation against it. Every cell above is single-threaded: `FindLeafBlockForSeek` takes the same `mutex_` that `Insert` holds for its whole body, so nothing measured here can show a read-scaling change in either direction. Tracked in [#802](https://github.com/orieg/expanse/issues/802), whose first work unit is the concurrent arm rather than a code change. That arm is pre-registered in §5.
  - What that mutex permits is derivable ahead of the arm, and is derived: `scripts/rocksdb_locate_bound.py` computes the aggregate read ceiling, its independence from the reader count, the shared-lock twin's prediction, and the inverse that turns a measured scaling ratio back into the share of a read the lock covers. It reads `insert_ns` and `read_ns` from [`results/baseline_rocksdb.json`](results/baseline_rocksdb.json) rather than carrying copies, so a re-measurement re-derives the prediction; its arithmetic is pinned by `--self-test` in the `lint` job. The numbers it returns are `(projected)`. §5.3 fixes H1's and H2's thresholds from them, and §5.9 derives H1's gateability from them.

## 5. Pre-registration — concurrent read scaling under one writer (#802)

Locked before any concurrent measurement exists, and before the harness that will produce it is written (AGENTS.md §8.8 commit 2). Nothing in §1–§4 is restated or revised here; those cells are single-threaded and stay as they are. No data appears in this section: outcomes are appended beside each hypothesis when the arm has run, never reconciled in place (§8.7).

### 5.1 The question, and why it needs its own arm

`FindLeafBlockForSeek` (`integrations/rocksdb/src/expanse_memtable.cc:136`) opens with `std::lock_guard<std::mutex> lock(mutex_)`, and is the locate path for `Contains`, `Get` and `IteratorImpl::Seek`. It is the same `mutex_` that `Insert` holds for its whole body. The per-leaf seqlock therefore protects the in-block scan *after* a reader has already serialised, and every cell in §2 is single-threaded, so nothing measured in this suite so far can show a read-scaling change in either direction.

`scripts/rocksdb_locate_bound.py` derives what that mutex permits from the §2 insert and read cells. Its outputs are `(projected)`, and the free input — `locked_fraction`, the share of a read spent inside the lock — is exactly what no committed artifact pins and what this arm is built to measure.

### 5.2 Cells

| dimension | value | why |
|---|---|---|
| primary operation | `Get` (point lookup) | the RocksDB read path that matters, and the one with a published single-threaded cell in §2 (`readrandom`) |
| readers `R` | 1, 2, 4, 7 | one writer thread accompanies every cell, so `R = 7` is 8 threads on the reference host's 8 physical P-cores. `R = 8` would put 9 runnable threads on 8 cores and price SMT into the widest cell |
| pin | `EXPANSE_BENCH_PIN=0,2,4,6,8,10,12,14` | the single-sibling knob `scripts/bench_pin.sh` already documents; one hardware thread per physical P-core, so no cell shares a core with itself |
| writer | paced at 250,000 inserts/s | a lock duty of ~5.7% against the §2 insert cell — low enough not to starve readers, and the bound still separates a fully-locked read path from a barely-locked one across the sweep. Achieved rate is recorded per cell; duty is computed from it, never assumed |
| control | writer idle (`R` sweep, no inserts) | isolates reader-vs-reader serialisation from reader-vs-writer blocking. Without it a flat curve cannot be attributed to either, and the first two rows of §5.4 cannot be told apart |
| saturation cell | writer free-running | reported, **never gated** — see §5.3 on why gating it would make H1 pass by construction |
| population | the §2 fixture (100,000 keys, 16-byte key, 64-byte value) | same fixture as §2, so the two are read on one workload. The §2 `readrandom` cell is **not** an `R = 1` datum for this arm: it runs with no writer and no reader threading, so the control's `R = 1` cell is its analogue and even that is a separate measurement. Every `S(R)` denominator comes from this arm's own `R = 1` cell in the same round |
| probe keys | drawn only from the pre-populated set | the writer adds keys during the run; a reader probing those would have a hit rate that drifts with the writer's progress rather than a declared one |
| rounds | interleaved `(cell × R)` within each round | AGENTS.md §8.20.2. A block of all `R = 1` followed by all `R = 7` confounds thermal drift with the effect |
| statistics | paired BCa 95% bootstrap on `S(R) = T(R)/T(1)` across interleaved rounds | §8.4; paired because the ratio's arms come from the same round |

### 5.3 Hypotheses and gates

Thresholds, method and sample size are fixed here and are not revisited after seeing results; a change to any of them relabels the outcome `INTERMEDIATE` and requires fresh rounds (§8.19).

**H1 — the mutex binds the read path.** Aggregate read throughput does not rise with reader count under the paced writer.

Why the writer is paced, and not free-running. The §2 insert cell puts a saturating writer at ~4.4 M inserts/s, which is the rate at which one writer holds `mutex_` essentially all of the time. Under such a writer readers are starved whatever the locate phase costs, `S(7)` collapses to ~1, and H1 "passes" without having measured anything about the read path — the twin is structurally denied any regime in which it could fail. That is a `PASS_categorical_by_design`, not a result. Pacing the writer well below saturation is what leaves H1 able to come out either way, and is load-bearing rather than a convenience.

- Instrument: `S(7)`, paired BCa 95% CI.
- `PASS` (the mutex binds): CI **upper** bound < 2.0.
- `REFUTED`: CI **lower** bound > 2.0.
- Otherwise `BOUNDARY_RESULT`.
- Where 2.0 comes from: at the pre-registered writer duty, `rocksdb_locate_bound.py` puts the scaling ceiling at 1.89 for a read half-covered by the lock and at 9.43 for one a tenth covered. A threshold of 2.0 therefore divides "the lock covers about half a read or more" from "it does not", and sits far above the §8.4 detection floor for the interval widths §2 reports.

**H2 — readers serialise against each other, not only against the writer.** The idle-writer control is also sublinear.

- Instrument: `S_idle(7)`, same estimator.
- `PASS`: CI **upper** bound < 3.5.
- `REFUTED`: CI **lower** bound > 3.5.
- Otherwise `BOUNDARY_RESULT`.
- Where 3.5 comes from: it is not "half of linear". With the writer idle, `rocksdb_locate_bound.py` makes the shared-lock twin's advantage over the mutex at `R = 7` exactly `7 / S_idle(7)`, so `S_idle(7) = 3.5` is the point where a shared lock would double reader throughput and no more. The threshold is therefore the decision it feeds: below it, splitting the lock is worth at least 2x at the widest cell; above it, it is worth less than that and the case has to be made on something else.
- H2 is what makes §5.4 falsifiable rather than a preference.

**H3 — the bound and the fit agree.** The USL contention parameter `fit_usl.py` fits to the measured curve matches the one `rocksdb_locate_bound.py` predicts from the locked fraction implied by `S(7)`.

- `PASS`: the fitted `alpha` interval overlaps the predicted `alpha`.
- A miss is not a finding about the code; it says one of the two models does not describe this system, and attribution from either is then unsafe. A fitted `beta` above zero is cost the bound contains no term for, and is reported on the unexplained line (§8.20.4) rather than folded into `alpha`.

### 5.4 What each outcome decides

| H1 | H2 | reading | next |
|---|---|---|---|
| `PASS` | `PASS` | readers serialise against each other; the locate phase is the constraint | a shared lock over the locate phase is the cheapest candidate, and its predicted gain is already a function of the measured locked fraction |
| `PASS` | `REFUTED` | readers scale fine alone and block only on the writer | a shared lock buys nothing; the writer's exclusive hold is the constraint |
| `REFUTED` | — | the mutex is not the binding constraint at these thread counts | record the measurement and close #802 on it; correct `integrations/rocksdb/README.md`, which currently implies otherwise |
| `BOUNDARY_RESULT` | any | the sweep cannot separate the hypotheses | report as such; widen rounds before widening claims |

No design is pre-approved by this section. It fixes what would have to be true for each one to be worth building.

### 5.5 Expected losses

Stated before the run so that meeting one is a recorded outcome and not a retrofit.

- **A small locked fraction refutes H1 honestly.** If a read's comparator and in-block scan dominate its locate phase, `S` rises and H1 is refuted for a real reason. That is a result, not a harness failure.
- **Pacing is new machinery, with no precedent in this repo.** Every existing concurrent harness here runs its threads free (`crates/expanse/examples/writer_scaling.rs`, the FFI concurrent suites), so the paced writer is the one component of this arm that no prior run has exercised. A paced writer built on sleeps can miss its offered rate, and a pacing loop that spins instead of sleeping would contend for the very lock under measurement. Mitigations, pre-registered: the achieved rate is recorded per cell and duty computed from it rather than assumed; a cell whose achieved rate departs materially from 250,000 inserts/s is reported and not gated; the pacing mechanism must block, never spin, and the harness records which.
- **A growing structure.** The writer adds keys while readers probe, so read cost drifts within a cell. The writer's total inserts are bounded and start/end population is recorded; a cell whose population moved enough to shift the §2 read cost is reported and not gated.
- **The competitor arms cannot run here at all.** `ReferenceSkipListRep` uses raw `Node* next[]` with a non-atomic `count_`, and `ReferenceVectorRep::EnsureSorted` sorts through a `const_cast` inside a `const` method with no lock. Neither is thread-safe, so a concurrent cell against either would be a data race, not a baseline. This arm is an Expanse self-scaling curve, as the concurrency suite's own sweeps are, and no cross-implementation ratio is claimed from it.

### 5.6 Not covered here

- Multi-writer scaling. `InsertConcurrently` is `Insert` verbatim (`:319`), so writers are fully serialised by construction; measuring that is a separate question from the read path.
- Iterator and scan concurrency beyond the `Seek` locate phase; the scan bracket was closed in #769.
- Any change to the locate phase. This section pre-registers the measurement only; a design lands against its result, not beside it.

### 5.7 Outcomes

*(measured: reference host — Intel i9-12900F, 8P+8E / 24 threads, 30 MiB L3, Linux 6.8; commit `0ed8f5e5`; pin `0-15` — the auto pin, **not** the pre-registered `0,2,4,6,8,10,12,14` (deviation note below); 5 rounds per cell, 60 cells per run; two independent runs, [34720727337](https://github.com/orieg/expanse/actions/runs/34720727337) and [34721708502](https://github.com/orieg/expanse/actions/runs/34721708502); artifacts [`results/baseline_concurrent_reads.json`](results/baseline_concurrent_reads.json) and [`results/baseline_concurrent_reads_run2.json`](results/baseline_concurrent_reads_run2.json).)*

Appended beside §5.3, which is not rewritten (§8.7). Both runs are committed because a cross-run delta is not claimable from one (`docs/BENCHMARKING.md` rule 18).

> **Pre-registration deviation: the pin.** §5.2 pre-registers `EXPANSE_BENCH_PIN=0,2,4,6,8,10,12,14`, one hardware thread per physical P-core. Neither run used it. Both artifacts record `provenance.core_pin` as `0-15`, with `host.scaling_governor_pin_source` as `EXPANSE_BENCH_PIN_APPLIED`, and `0-15` is the list `scripts/bench_pin.sh` selects by itself on this host: the kernel's `cpu_core` list (`host.cpu_core_cpus` in the same artifacts). That list is 8 physical P-cores with **both** SMT siblings of each (`host.cpu0_thread_siblings` is `0-1`), so it is two hardware threads per physical core, not one (AGENTS.md §8.20.5 step 1). §5.2 is not rewritten (§8.7); this note records what ran instead.
>
> **What the deviation exposes.** `integrations/rocksdb/benches/bench_memtable_concurrent.cc` sets no per-thread affinity, so placement inside `0-15` is the scheduler's. A paced or free cell at `R = 7` runs 8 threads, 7 readers and the writer (the idle control spawns no writer thread, so its `R = 7` runs 7), on 16 hardware threads, and nothing stops two of them sharing a physical core. §5.2's `readers` row stops at `R = 7` so that the widest cell would not price SMT. Under `0-15` that exclusion does not hold, at `R = 7` or at any `R ≥ 2`.
>
> **Why the pin is not a detail for this arm.** AGENTS.md §8.20.5 step 0: a pin is not neutral for an arm whose threads serialise on one lock, and this arm serialises every reader on `mutex_` in `FindLeafBlockForSeek` (§5.1). The concurrency suite's `str` arm, which holds its writer mutex for a whole insert, moved by placement alone between exactly these two pins at one commit ([`docs/benchmarks/concurrency/README.md`](../concurrency/README.md) §10), and there the one-sibling pin was the slower of the two. That arm's direction and size do not transfer to this one, so the `0-15` cells are not a proxy for the pre-registered cells in either direction. Nor do the margins settle it by arithmetic. Taking the larger upper bound of the two runs, H1's 2.0 is crossed by paced `S(7)` [0.762, 0.771] (run 2) under an upward move of 2.594× on that bound, and H2's 3.5 by idle `S(7)` [0.627, 0.633] (run 1) under 5.529×. Those are the sizes a move would need, not a prediction that one happens; no pin comparison of this arm has been measured.
>
> Step 0 landed in #903, after #875 locked this section, and it says the one-sibling pin is the wrong choice for a coarse-mutex arm whose thread count equals the core count, because the blocked threads then compete for the CPUs of the one thread making progress. That does not license adopting the pin that ran in place of the one pre-registered. §5.3's gates are evaluated on the pin they were locked against (§8.19); changing it is a new pre-registration with fresh rounds, not a re-reading of these.
>
> **Status: the §5.3 verdicts were deferred pending re-run** under the pre-registered pin, tracked in [#802](https://github.com/orieg/expanse/issues/802). **That re-run is §5.8, and the deferral is discharged there.** No figure is retracted. Every figure in this section was measured at `0ed8f5e5` under pin `0-15`, its provenance is stated correctly above, and it stays as committed, so `.github/superseded-figures.json` is unchanged. What is withheld is reading those figures as the pre-registered outcome: the verdict labels below describe the `0-15` runs only, and §5.4's routing is not acted on from them. The re-run is a `workflow_dispatch` of `bench_baremetal.yml` with `benchmark_suite=rocksdb_concurrent` and `cpu_pin=0,2,4,6,8,10,12,14`, as two independent runs (`docs/BENCHMARKING.md` rule 18). The pre-registered verdict is read from that re-run alone, and the `0-15` artifacts stay committed beside it.

| hypothesis | gate | run 1 | run 2 | verdict |
|---|---|---|---|---|
| **H1** the mutex binds the read path | paced `S(7)` CI upper < 2.0 | 0.748 [0.733, 0.756] | 0.768 [0.762, 0.771] | ~~**PASS** in both, under pin `0-15`~~ — **superseded by §5.8: not gated.** The writer reached 200,116–225,044 and 213,714–216,479 inserts/s in this cell against 250,000 offered, and §5.5 does not gate such a cell |
| **H2** readers serialise against each other | idle `S(7)` CI upper < 3.5 | 0.631 [0.627, 0.633] | 0.620 [0.611, 0.629] | **PASS** in both, under pin `0-15`; the pre-registered verdict is in §5.8 |
| **H3** the bound and the fit agree | fitted `alpha` interval overlaps predicted | see below | see below | ~~**partly — `alpha` matches at its ceiling, `beta` does not exist in the bound** (under pin `0-15`)~~ — **superseded by §5.8: not gated.** Its instrument is the paced `S(7)` cell and that cell's duty |

Aggregate read throughput, Mops/s, mean of 5 rounds (run 1):

| writer | R=1 | R=2 | R=4 | R=7 |
|---|---|---|---|---|
| idle (control) | 4.085 | 3.065 | 2.783 | 2.576 |
| paced (250k/s offered, 5.45% measured duty averaged over `R`: 5.65% at `R ≤ 4`, 4.84% at `R = 7`; §5.8) | 1.340 | 0.965 | 0.959 | 1.002 |
| free (never gated) | 0.209 | 0.360 | 0.686 | 0.981 |

~~**H1 and H2 both PASS under pin `0-15`, which is §5.4's first row:** readers serialise against each other, and the locate phase is the constraint. A shared lock over it is the cheapest candidate. That routing is not acted on until the pre-registered pin has been run (deviation note above).~~ **Superseded by §5.8.** H1 is not gated in any run, so no §5.4 row applies. H2's reading stands on its own: with no writer, readers serialise against each other. That the locate phase is *the* constraint, and that a shared lock over it is the cheapest candidate, rested on H1 and is not established.

~~They pass by more than the gates asked.~~ H2 passes by more than its gate asked; the paced cell is measured but not gated (§5.8). `S(7) < 1` in both the paced and the idle cell means aggregate read throughput *falls* as readers are added — not merely failing to scale. The idle control is what makes that attributable: with no writer at all, seven readers deliver 0.63× what one delivers, so the serialisation is reader-against-reader and not reader-against-writer.

**The bound of `scripts/rocksdb_locate_bound.py` held as an upper bound and its floor was refuted.** It predicts `S(W) = clamp(K, 1, W)`, so its minimum is 1.0: adding readers can never hurt. Measured 0.748 in the paced cell, and 0.631 [0.627, 0.633] in the idle cell, which has no writer and so does not depend on the pacing question §5.8 raises. The bound's own docstring claims only a ceiling on an idealised handoff — zero lock transfer cost, no convoying, a reader's unlocked remainder perfectly overlapped — and the measurement says the last two of those are false. The ceiling claim survives; the implicit floor does not, and the bound carries no term for the cost that produces it.

**Superseded by §5.8: H3 is not gated.** The paragraph below is kept as written. Its predicted `alpha` reads the 5.45% duty averaged over all four paced cells, and its measured side reads the paced `S(7)` cell, whose writer missed its pace.

**H3, stated without folding the residue into `alpha` (§8.20.4).** At the measured 5.45% duty the bound predicts `alpha = 1.0` (a fully-locked read path). To reproduce the measurement, USL needs either `alpha = 1.394` with `beta = 0`, which `scripts/fit_usl.py` treats as inadmissible because `alpha > 1`, or `alpha = 1` with **`beta = 0.0563`**. So the contention parameter matches the prediction at its ceiling, and there is a substantial coherency term the derivation contains nothing for. That belongs on the unexplained line. Naming it: the candidates are cache-line traffic on the mutex word, lock convoying, and scheduler wake-up cost, none of which has been measured here — no counter was collected, so this is a list of hypotheses and not an attribution.

**The free cell behaved exactly as §5.2 and §5.3 predicted it would, which is why it is not gated.** Its `S(7)` is 4.705, the only cell in the sweep that looks like scaling. It is not: a single reader against a free-running writer gets 0.209 Mops/s against the idle control's 4.085, a starvation of roughly twenty-fold, and adding readers only recovers collectively what one reader could not take. Gating that cell would have reported excellent read scaling from a lock-fairness artifact.

**Rule 18 in practice.** Seven of nine scaling cells overlap between the two runs; `paced S(4)` and `paced S(7)` do not (0.716 vs 0.736, 0.748 vs 0.768). Neither non-overlap changes a verdict, and both are small, but they are the concrete demonstration that a within-run BCa interval does not bound between-run spread — which is why both runs are committed rather than the better one.

**Host.** Maximum foreign busy CPU across all 120 cells was 0.010 core-equivalents, so neither run competed with anything. The per-cell mean is slightly negative (−0.08), an artifact of subtracting the runner's own children's CPU time from the host's busy delta at this resolution; it is reported rather than clamped.

**What is not established.** No counter was collected, so the mechanism behind `beta` is unmeasured. The arm measures `Get`; `Contains` and `IteratorImpl::Seek` share the same locate path but were not swept. And no design has been built or measured — ~~H1 and H2 say a shared lock is the cheapest candidate, not that it works.~~ with H1 not gated (§5.8), not even the candidate is established. Every cell was also taken with both SMT siblings of each P-core available to the scheduler, so neither the verdicts nor the `beta` term is established for the pre-registered one-sibling placement; §5.8 measures that placement.

### 5.8 Outcomes under the pre-registered pin

*(measured: reference host — Intel i9-12900F, 8P+8E / 24 threads, 30 MiB L3, Linux 6.8; commit `0ed8f5e5`; pin `0,2,4,6,8,10,12,14` as pre-registered in §5.2, recorded as `provenance.core_pin` with source `EXPANSE_BENCH_PIN_APPLIED`; 5 rounds per cell, 60 cells per run; two independent runs, [34773016420](https://github.com/orieg/expanse/actions/runs/34773016420) and [34773192119](https://github.com/orieg/expanse/actions/runs/34773192119); artifacts [`results/baseline_concurrent_reads_pin_one_sibling.json`](results/baseline_concurrent_reads_pin_one_sibling.json) and [`results/baseline_concurrent_reads_pin_one_sibling_run2.json`](results/baseline_concurrent_reads_pin_one_sibling_run2.json).)*

This is the re-run the §5.7 deviation note deferred to. It was taken at `0ed8f5e5`, the commit §5.7 measured, so the pin is the only thing that differs between the two pairs of runs. §5.2 and §5.3 are not rewritten (§8.7), and the pre-registered verdicts are read here, from these two runs alone.

| hypothesis | gate | run 1 | run 2 | verdict |
|---|---|---|---|---|
| **H1** the mutex binds the read path | paced `S(7)` CI upper < 2.0 | 0.634 [0.628, 0.641] | 0.625 [0.621, 0.633] | **not gated**: the writer reached 138,388–144,013 and 138,559–142,664 inserts/s in this cell against 250,000 offered (below) |
| **H2** readers serialise against each other | idle `S(7)` CI upper < 3.5 | 0.624 [0.614, 0.631] | 0.628 [0.618, 0.634] | **PASS** in both |
| **H3** the bound and the fit agree | fitted `alpha` interval overlaps predicted | — | — | **not gated**: its instrument is the paced `S(7)` cell and that cell's duty |

**The paced `R = 7` cell cannot be gated under §5.5, in any run of either pin.** §5.5 pre-registers that a cell whose achieved rate "departs materially" from 250,000 inserts/s is reported and not gated, but it never gives "materially" a value. The driver does not supply one either. It refuses a paced cell only when the writer ran out of keys (`writer_exhausted`), and no cell in the four runs did. The achieved rates were:

| pin | run | paced `R ≤ 4`, lowest cell (inserts/s) | paced `R = 7`, range (inserts/s) | paced `R = 7`, mean duty |
|---|---|---|---|---|
| `0-15` | 1 | 249,920 | 200,116–225,044 | 4.84% |
| `0-15` | 2 | 249,797 | 213,714–216,479 | 4.87% |
| `0,2,4,6,8,10,12,14` | 1 | 249,697 | 138,388–144,013 | 3.19% |
| `0,2,4,6,8,10,12,14` | 2 | 249,957 | 138,559–142,664 | 3.18% |

Every `R ≤ 4` cell held its offered rate to within 303 inserts/s. Every `R = 7` cell fell short: by 10.0–20.0% under `0-15`, and by 42.4–44.6% under the pre-registered pin. Deciding now which of those shortfalls is "material" would fix the threshold after seeing the data, which §8.19 forbids. So H1 is not gated under either pin, and §5.7's H1 `PASS` is marked superseded in place. H3 falls with it, because the locked fraction it reads is implied by the same cell and by the duty computed from it.

The duty §5.7 published, 5.45%, is the mean over all four paced cells, and it hid the `R = 7` shortfall. That is a pre-registration defect, recorded here rather than repaired in §5.5. Gating H1 needs a tolerance fixed before fresh rounds are taken (§8.19), tracked in [#802](https://github.com/orieg/expanse/issues/802).

**No §5.4 row applies.** Every row keys on H1, and a not-gated H1 is not the `BOUNDARY_RESULT` the last row covers, so no design is routed from this arm. H2's `PASS` stands on its own. With the writer idle, seven readers deliver less aggregate throughput than one under the pre-registered pin as well (`S(7)` 0.624 [0.614, 0.631] and 0.628 [0.618, 0.634]), so readers serialise against each other.

Aggregate read throughput, Mops/s, mean of 5 rounds (run 1):

| writer | R=1 | R=2 | R=4 | R=7 |
|---|---|---|---|---|
| idle (control) | 4.081 | 3.081 | 2.782 | 2.547 |
| paced (250k/s offered; achieved rates above) | 1.339 | 0.982 | 0.993 | 0.849 |
| free (never gated) | 0.211 | 0.379 | 0.702 | 0.794 |

**Pin sensitivity, at one commit.** The two pairs of runs differ only in the pin. A move counts only where both runs moved the same way with separated intervals (`docs/BENCHMARKING.md` rule 18).

- **Idle: no pin effect is detectable.** Every idle `S(2)`, `S(4)` and `S(7)` interval under one pin overlaps every matching interval under the other. For `S(7)` that is [0.627, 0.633] and [0.611, 0.629] under `0-15`, against [0.614, 0.631] and [0.618, 0.634] under the pre-registered pin.
- **Paced `S(7)` is lower under the pre-registered pin, but not attributably to it.** It fell in both runs with separated intervals: [0.733, 0.756] and [0.762, 0.771], against [0.628, 0.641] and [0.621, 0.633]. The writer's achieved rate moved with it, though, so pin and duty changed together. Paced `S(2)` and `S(4)` do not separate in both runs.
- **Free `S(7)` is pin-sensitive by rule 18's test.** It was lower under the pre-registered pin in both runs, with separated intervals: [4.539, 5.154] and [4.650, 5.011], against [3.629, 3.906] and [3.707, 3.773]. The cell is never gated (§5.2), and its mechanism is unmeasured.

**The writer's `R = 7` shortfall is larger under the pre-registered pin, and the cause is unmeasured.** That cell puts 8 runnable threads, 7 readers and the writer, on 8 hardware threads under the pre-registered pin, against 16 under `0-15`. AGENTS.md §8.20.5 step 0 describes blocked threads competing for the CPUs of the thread making progress in a coarse-mutex arm. That is a hypothesis for the sleeping writer missing its schedule; no counter was collected, so nothing here tests it.

**Host.** Maximum foreign busy CPU across the 120 cells was 0.02 core-equivalents (0.02 in run 1, 0.01 in run 2). The per-cell means were −0.083 and −0.081, the same subtraction artifact §5.7 reports. `load1` peaked at 2.24 and 2.27 on 24 logical CPUs, a figure that includes the sweep's own threads.

**What is not established.** H1 is not decided in either direction: the reader curve under a writer is measured, but not at the pre-registered duty. No counter was collected, so neither the writer's shortfall nor the readers' regression has a measured mechanism. `Contains` and `IteratorImpl::Seek` share the locate path but were not swept, and no design has been built or measured.

### 5.9 Amendment — gating H1 when the paced writer runs short

This amendment was written after §5.7 and §5.8 were measured and read, and it is locked before the rounds it is evaluated on (§8.19). It is therefore **not a blind pre-registration**. Its authors already knew that every paced `R = 7` cell had missed its offered rate, and what `S(7)` those cells returned. It changes a method and fixes a new sample; it changes no threshold.

§5.2–§5.5 are not rewritten (§8.7). The §5.7 and §5.8 verdicts stand as recorded, and nothing in them is re-read under this amendment.

**What it changes for H1.** §5.5 excludes from gating a paced cell whose achieved rate "departs materially" from the offered 250,000 inserts/s, but never defines "materially" (§5.8). For H1, that exclusion is replaced by a rule derived from the bound. No tolerance is chosen.

- H1's threshold, `S(7)` = 2.0, is the decision "the lock covers at least `(1 − d) / 2` of a read", where `d` is the writer's duty (`gate_boundary_locked_fraction` in `scripts/rocksdb_locate_bound.py`).
- A paced writer sleeps to an absolute schedule, so its achieved duty lies in `[0, d_offered]`. Across that interval the boundary moves by at most `d_offered / 2` = 0.0283: from 0.4717 at the offered duty to 0.5 for a writer that stalls outright (`max_gate_boundary_shift`, pinned by `--self-test`). A shortfall of any size leaves H1 deciding "about half a read".
- A paced `R = 7` cell whose achieved rate is **at or below** the offered rate is therefore gated for H1, whatever the size of the shortfall.
- A cell whose achieved rate is **above** the offered rate, or whose writer ran out of keys (`writer_exhausted`), is outside the derivation. It is reported, not gated.
- H1's `PASS`, `REFUTED` and `BOUNDARY_RESULT` thresholds are unchanged (§5.3).

**What it changes for H3.** H3's pass criterion is a fitted `alpha` *interval*. On every curve this arm has produced — paced and idle, under both pins, six curves — `fit_usl_with_bootstrap` in `scripts/fit_usl.py` pins `alpha` at its bound of 1 and labels the `alpha` interval `zero_width` and unusable. That is observed on those six curves, not derived for every curve: the script's own docstring gives a retrograde USL curve that both of its estimators recover.

H3 is therefore evaluated on the fresh rounds **only if** `fit_usl_with_bootstrap` returns a usable `alpha` interval for a run's paced curve. Its predicted side is then `predicted_alpha_from_scaling` at that run's paced `S(7)` point estimate. The bound's prediction carries no duty, so the writer's shortfall does not enter it. Where the interval is unusable, H3 is reported **not evaluable** for that run.

**The fresh rounds, fixed here.**

- **Commit and pin:** commit `0ed8f5e5`, the commit §5.7 and §5.8 measured, with pin `0,2,4,6,8,10,12,14` (§5.2).
- **Cell parameters:** cells, rounds per cell (5), window (2.0 s), offered rate (250,000 inserts/s) and estimator all as §5.2.
- **Runs:** two independent runs, each a `bench_baremetal.yml` dispatch with `benchmark_suite=rocksdb_concurrent` and `cpu_pin=0,2,4,6,8,10,12,14`. Run 1 completes before run 2 is dispatched.
- **Verdicts:** H1 and H2 are read per §5.3 in each run. A verdict is claimed only where both runs return it; otherwise it is `BOUNDARY_RESULT`.
- **H3:** as above.
- **Pooling:** no round from §5.7 or §5.8 is pooled into these.

**Expected outcome, stated before the rounds.** Both earlier pairs of runs put paced `S(7)` far below 2.0: [0.628, 0.641] and [0.621, 0.633] under this pin, and [0.733, 0.756] and [0.762, 0.771] under `0-15`. H1 is expected to `PASS`. That expectation comes from having seen the data, which is why the rounds that decide it are fresh. The gate is still applied as written, and a paced `S(7)` whose lower bound clears 2.0 in both runs would refute H1.

### 5.10 Outcomes under the §5.9 amendment

*(measured: reference host — Intel i9-12900F, 8P+8E / 24 threads, 30 MiB L3, Linux 6.8; commit `0ed8f5e5`; pin `0,2,4,6,8,10,12,14`, recorded as `provenance.core_pin` with source `EXPANSE_BENCH_PIN_APPLIED`; 5 rounds per cell, 60 cells per run; two independent runs, dispatched in sequence after §5.9 was pushed, [34775640656](https://github.com/orieg/expanse/actions/runs/34775640656) and [34775850899](https://github.com/orieg/expanse/actions/runs/34775850899); artifacts [`results/baseline_concurrent_reads_amended_h1.json`](results/baseline_concurrent_reads_amended_h1.json) and [`results/baseline_concurrent_reads_amended_h1_run2.json`](results/baseline_concurrent_reads_amended_h1_run2.json).)*

These outcomes are read against §5.9 as it was locked. No round from §5.7 or §5.8 is pooled into them, and §5.7 and §5.8 are not re-read.

| hypothesis | gate | run 1 | run 2 | verdict |
|---|---|---|---|---|
| **H1** the mutex binds the read path | paced `S(7)` CI upper < 2.0, the cell gated per §5.9 | 0.624 [0.618, 0.630] | 0.622 [0.618, 0.628] | **PASS** in both |
| **H2** readers serialise against each other | idle `S(7)` CI upper < 3.5 | 0.621 [0.611, 0.631] | 0.609 [0.599, 0.614] | **PASS** in both |
| **H3** the bound and the fit agree | usable fitted `alpha` interval overlaps predicted `alpha` | not evaluable | not evaluable | **not evaluable** |

**H1's gate cell is gated under §5.9.** In the paced `R = 7` cell the writer reached 139,698–145,363 inserts/s in run 1 and 139,403–143,861 in run 2. That is below the offered 250,000 in every cell, and no writer ran out of keys. Every `R ≤ 4` paced cell held within 221 inserts/s of the offered rate. The mean `R = 7` duty was 3.20% and 3.19%. That puts H1's decision boundary at a locked fraction of 0.4840 and 0.4841 (`gate_boundary_locked_fraction`), inside the [0.4717, 0.5] band §5.9 derived.

**H1 and H2 both PASS, which is §5.4's first row.** Readers serialise against each other, and the locate phase is the constraint. A shared lock over the locate phase is the cheapest candidate. §5.4 pre-approves no design, and none has been built or measured.

**H3 is not evaluable.** On both paced curves, `fit_usl_with_bootstrap` pinned `alpha` at its bound of 1 and labelled the `alpha` interval `zero_width`, as it did on every earlier curve. §5.9 makes that "not evaluable", so there is no H3 verdict. The bound's predicted `alpha` from paced `S(7)` is 1.0 in both runs (`predicted_alpha_from_scaling`), but agreement at a clamp is not the test H3 pre-registered.

Aggregate read throughput, Mops/s, mean of 5 rounds (run 1):

| writer | R=1 | R=2 | R=4 | R=7 |
|---|---|---|---|---|
| idle (control) | 4.092 | 3.113 | 2.808 | 2.541 |
| paced (250k/s offered; achieved rates above) | 1.344 | 0.985 | 1.006 | 0.838 |
| free (never gated) | 0.203 | 0.380 | 0.704 | 0.782 |

**The expectation §5.9 stated was met, and that is weaker evidence than a blind result.** §5.9 predicted `PASS` from §5.7 and §5.8, and the upper bounds landed at 0.630 and 0.628 against 2.0. The fresh rounds guard against a rule fitted to one sample. They do not guard against a rule written by someone who already knew the effect's size, and §5.9 says so.

**Against §5.8 (same commit and pin, not pooled).** Paced `S(7)` reproduces: every fresh interval overlaps every §5.8 interval. Idle `S(7)` in fresh run 2, [0.599, 0.614], separates from §5.8 run 2's [0.618, 0.634] but overlaps §5.8 run 1's [0.614, 0.631]. One pairing of four does not make a cross-run delta, so none is claimed (`docs/BENCHMARKING.md` rule 18).

**Host.** Maximum foreign busy CPU was 0.01 core-equivalents in run 1 and 0.04 in run 2. The per-cell means were −0.082 and −0.083, the subtraction artifact §5.7 reports. `load1` peaked at 1.62 and 1.98 on 24 logical CPUs.

**What is not established.** No counter was collected, so neither the readers' regression nor the writer's `R = 7` shortfall has a measured mechanism. `Contains` and `IteratorImpl::Seek` share the locate path but were not swept. H3 being not evaluable leaves the bound-versus-fit agreement untested. The shared-lock design that §5.4 names has not been built or measured.

**Estimator behind the H3 reading** *(appended after publication; the verdict above is unchanged)*. `scripts/fit_usl.py` refines its OLS fit with non-linear least squares only when scipy imports, and records which ran as `estimator`. The reading above is what `min_ssr(ols, nlls)` returns: `alpha` pins at 1 with a zero-width interval on both paced curves, and reproduces with scipy 1.18.1 and numpy 2.5.3. Neither §5.9 nor the reading above names the estimator, and the result depends on it. Under the OLS fallback, on a host without scipy, the same two curves return usable BCa intervals of [0.9578, 1.0000] and [0.9669, 1.0000], both containing the predicted `alpha` of 1.0, so §5.9's rule would read H3 as `PASS`. Both intervals end at `alpha`'s bound of 1, where the prediction's clamp also sits. §5.9's premise that `alpha` pins at 1 on every earlier curve holds under `min_ssr(ols, nlls)` as well, and not under OLS, where the §5.8 run-2 paced curve returns a usable interval. [`scripts/concurrent_verdicts.py`](scripts/concurrent_verdicts.py) applies §5.9's rules to the two runs, reads H3 only with `min_ssr(ols, nlls)`, and refuses it otherwise.

### 5.11 Hardware counters on the concurrent arm

*(measured: reference host — Intel i9-12900F, 8P+8E / 24 threads, 30 MiB L3, Linux 6.8; commits `e5645d83` (34783487238) and `09ed3357` (34783862078, 34784008215, 34784203230), which differ only in how `scripts/bench_counters.py` composes its launch pin (#915) — both launch a `0-15` run under `taskset -c 0-15`; `scripts/bench_counters.py` per-thread mode over the harness's counters mode, 5 rounds per cell, and a `perf c2c` round on each `R = 7` cell; two runs per pin: `0-15` [34783487238](https://github.com/orieg/expanse/actions/runs/34783487238), [34784203230](https://github.com/orieg/expanse/actions/runs/34784203230), and `0,2,4,6,8,10,12,14` [34783862078](https://github.com/orieg/expanse/actions/runs/34783862078), [34784008215](https://github.com/orieg/expanse/actions/runs/34784008215), each round's pin verified from the harness's own `cpus_allowed`; artifacts under [`results/counters/`](results/counters/); every table below is generated by [`scripts/counters_report.py`](scripts/counters_report.py).)*

Nothing in this section was pre-registered, and none of it is a verdict. It is AGENTS.md §8.20.5 steps 3–5: the shape of the per-thread counters across `R`, which cache lines are contended, and which symbols the sampler attributes to them. It was taken before any design is built. Counters and `perf c2c` are observational, so no mechanism is decided here (§8.20.3).

**Runs not used.**

- [34783092421](https://github.com/orieg/expanse/actions/runs/34783092421) is discarded rather than relabelled. It was dispatched with the one-sibling pin, and every harness row reported `cpus_allowed` `0-15` while its artifacts recorded the one-sibling `core_pin`. `bench_counters.py` launched workloads under the counting PMU's CPU list, which #915 fixed.
- [34783226388](https://github.com/orieg/expanse/actions/runs/34783226388) never started: its dispatch named a commit that does not exist.

**The counters harness does not reproduce §5.10's paced `R = 7` regime.**

- Under `0,2,4,6,8,10,12,14`, its paced `R = 7` writer reached 201,940–205,503 inserts/s and its readers 1.057–1.125 Mops/s across both runs. The four wall-clock runs at that pin recorded 138,388–145,363 inserts/s and 0.822–0.865 Mops/s.
- Under `0-15`, the counters writer (200,561–212,955) and the wall-clock writer (200,116–225,044) are in the same range.
- The cause is unmeasured. The attach did not slow the readers: counters-harness reads at `R = 7` are at or above the wall-clock arm's at both pins.
- The two harness modes differ in three ways, and none of them has been isolated. Counters mode rebuilds the fixture per round inside one process, blocks threads on the start gate, and launches under `taskset`. The wall-clock mode runs one cell per process, behind a spinning gate.
- **Consequence:** the paced `R = 7` counters below describe the counters harness's regime, not §5.10's gated cell. Idle `R = 7` reads are closer: 2.616–2.647 Mops/s under `0,2,4,6,8,10,12,14` in this harness, against 2.442–2.609 in the wall-clock arm.

**Per-read shape across `R`, idle writer, all four runs** (the range over the four runs' means):

| `R` | cross-core snoop hits per read (`xsnp_hitm`) | context switches per read | instructions per read |
|---|---|---|---|
| 1 | 0.000315–0.000454 | 5.9e-07–8.9e-07 | 2,281 |
| 2 | 1.5–1.91 | 0.044–0.058 | 2,939–2,957 |
| 7 | 8.51–8.72 | 0.41 | 5,903–5,941 |

Snoop hits per read grow roughly with the number of other readers. §8.20.5 step 3 reads that shape as pairwise coherency traffic (β) rather than a fixed per-operation step. That reading is a hypothesis about what the count is, not a measurement of its cause. No counter here says what the added instructions per read are.

**Pin effects under rule 18.** 13 of 72 (cell, event) pairs have both `0,2,4,6,8,10,12,14` runs' intervals separated from both `0-15` runs', in one direction. Each is the change from the mean of the two `0-15` runs to the mean of the two `0,2,4,6,8,10,12,14` runs:

- **Idle `R = 7`:** `reader/cycles` +2.3%; `reader/ref-cycles` +2.3%; `reader/mem_load_l3_hit_retired.xsnp_hitm` +2.2%; `reader/task-clock` +1.2%; `reader/instructions` +0.5%.
- **Paced `R = 2`:** `writer/mem_load_l3_hit_retired.xsnp_hitm` +7.0%.
- **Paced `R = 7`, in the counters harness's regime:** `writer/mem_load_l3_hit_retired.xsnp_hitm` +19.9%; `writer/l2_rqsts.rfo_miss` +14.5%; `reader/mem_load_l3_hit_retired.xsnp_hitm` +13.1%; `writer/context-switches` +13.0%; `writer/instructions` +6.3%; `reader/context-switches` +6.2%; `reader/instructions` +4.1%.

**Between-run spread.** 42 same-pin pairs separate. Idle `R = 2` falls into two states under both pins (`xsnp_hitm` per read 1.504 and 1.896 under `0-15`; 1.908 and 1.501 under `0,2,4,6,8,10,12,14`), so no pin effect is read at `R = 2`.

**`perf c2c`, eight reports (idle and paced `R = 7`, four runs).** The report numbers its shared cache lines from 0, and the same three lead every report:

- **Line 0**, kernel data: 13.6–25.5% of load HITMs. Its symbols do not resolve on this host.
- **Line 1**, a user-space address: 8.1–14.1%. The sampler attributes its HITMs to `ExpanseMemTableRep::Get`, `ExpanseMemTableRep::FindLeafBlockForSeek`, `pthread_mutex_lock` and `pthread_mutex_unlock`.
- **Line 2**, the same address as line 1: 4.0–10.9%, touched only by unresolved kernel code.

- **Where that address is:** the harness constructs the `ExpanseMemTableRep`, and with it its `mutex_`, on `RunCell`'s stack (`integrations/rocksdb/benches/bench_memtable_concurrent.cc:292`).
- **Attribution differs by pin, in both runs of each.** At idle `R = 7`:
  - under `0-15`, `ExpanseMemTableRep::Get` takes 59–61% of line 1's HITMs;
  - under `0,2,4,6,8,10,12,14`, `pthread_mutex_lock` takes 63–67%.
- **Total load HITMs:** 3,148–3,163 per idle report and 1,557–1,629 per paced report.

**What is not established.**

- **No mechanism.** Nothing here has been confirmed by intervention (§8.20.5 step 6), for either the readers' regression or the writer's shortfall.
- **Field mapping.** No layout offsets were recorded, so which field of the memtable object each HITM lands on is not mapped.
- **The kernel line is unidentified.**
- **No futex counter.** The futex tracepoint was unavailable on this host (`syscalls:sys_enter_futex`: event syntax error), so there is no direct count of mutex waits.
- **The counters harness's paced regime is unexplained.**

#### Generated tables

<!-- generated by docs/benchmarks/rocksdb_memtable/scripts/counters_report.py from results/counters/ (runs 34783487238, 34784203230, 34783862078, 34784008215; commit 09ed3357, e5645d83) -->

Per-thread counters per operation: mean [BCa 95%] over 5 rounds. `reader/*` divides the reader threads' rows by the round's reads and `writer/*` the writer's rows by its inserts.

| event | cell | `0-15` 34783487238 | `0-15` 34784203230 | `one_sibling` 34783862078 | `one_sibling` 34784008215 |
|---|---|---|---|---|---|
| `reader/instructions` | idle_r1 | 2,281 [2,281, 2,281] | 2,281 [2,281, 2,281] | 2,281 [2,281, 2,281] | 2,281 [2,281, 2,281] |
| `reader/instructions` | idle_r2 | 2,942 [2,939, 2,944] | 2,956 [2,955, 2,957] | 2,957 [2,956, 2,957] | 2,939 [2,937, 2,941] |
| `reader/instructions` | idle_r7 | 5,908 [5,897, 5,922] | 5,903 [5,888, 5,914] | 5,941 [5,936, 5,948] | 5,933 [5,929, 5,940] |
| `reader/instructions` | paced_r1 | 3,968 [3,964, 3,972] | 3,975 [3,967, 3,986] | 3,970 [3,965, 3,973] | 3,970 [3,966, 3,972] |
| `reader/instructions` | paced_r2 | 5,525 [5,513, 5,536] | 5,500 [5,473, 5,526] | 5,523 [5,499, 5,540] | 5,527 [5,500, 5,566] |
| `reader/instructions` | paced_r7 | 9,375 [9,324, 9,511] | 9,361 [9,250, 9,478] | 9,793 [9,767, 9,815] | 9,720 [9,696, 9,736] |
| `reader/cycles` | idle_r1 | 1,247 [1,246, 1,251] | 1,250 [1,248, 1,251] | 1,247 [1,246, 1,248] | 1,247 [1,245, 1,248] |
| `reader/cycles` | idle_r2 | 2,065 [2,061, 2,069] | 2,264 [2,262, 2,266] | 2,270 [2,266, 2,273] | 2,064 [2,060, 2,072] |
| `reader/cycles` | idle_r7 | 6,068 [6,052, 6,081] | 6,052 [6,049, 6,056] | 6,244 [6,237, 6,251] | 6,155 [6,136, 6,174] |
| `reader/cycles` | paced_r1 | 2,855 [2,834, 2,894] | 2,863 [2,837, 2,925] | 2,859 [2,836, 2,921] | 2,875 [2,827, 2,923] |
| `reader/cycles` | paced_r2 | 4,429 [4,395, 4,470] | 4,433 [4,392, 4,507] | 4,435 [4,376, 4,493] | 4,523 [4,462, 4,591] |
| `reader/cycles` | paced_r7 | 8,973 [8,951, 8,991] | 8,804 [8,723, 8,866] | 9,394 [9,377, 9,406] | 8,853 [8,830, 8,874] |
| `reader/mem_load_l3_hit_retired.xsnp_hitm` | idle_r1 | 0.000336 [0.000314, 0.000358] | 0.000454 [0.000339, 0.000616] | 0.000359 [0.00033, 0.000405] | 0.000315 [0.000299, 0.000328] |
| `reader/mem_load_l3_hit_retired.xsnp_hitm` | idle_r2 | 1.5 [1.49, 1.52] | 1.9 [1.88, 1.91] | 1.91 [1.9, 1.91] | 1.5 [1.48, 1.54] |
| `reader/mem_load_l3_hit_retired.xsnp_hitm` | idle_r7 | 8.51 [8.46, 8.56] | 8.53 [8.5, 8.55] | 8.72 [8.7, 8.76] | 8.7 [8.66, 8.74] |
| `reader/mem_load_l3_hit_retired.xsnp_hitm` | paced_r1 | 0.835 [0.805, 0.881] | 0.82 [0.8, 0.855] | 0.837 [0.812, 0.902] | 0.851 [0.796, 0.915] |
| `reader/mem_load_l3_hit_retired.xsnp_hitm` | paced_r2 | 3.47 [3.38, 3.54] | 3.38 [3.31, 3.42] | 3.5 [3.41, 3.63] | 3.56 [3.51, 3.62] |
| `reader/mem_load_l3_hit_retired.xsnp_hitm` | paced_r7 | 11.8 [11.7, 12.1] | 11.7 [11.5, 12] | 13.8 [13.8, 13.9] | 12.8 [12.7, 12.9] |
| `reader/l2_rqsts.rfo_miss` | idle_r1 | 0.013 [0.0125, 0.0134] | 0.0133 [0.0131, 0.0134] | 0.0128 [0.0123, 0.0134] | 0.013 [0.0125, 0.0132] |
| `reader/l2_rqsts.rfo_miss` | idle_r2 | 1.11 [1.1, 1.13] | 1.97 [1.95, 1.98] | 1.92 [1.91, 1.93] | 1.06 [1.04, 1.09] |
| `reader/l2_rqsts.rfo_miss` | idle_r7 | 8.14 [8.1, 8.19] | 8.13 [8.07, 8.2] | 8.2 [8.19, 8.21] | 8.17 [8.17, 8.18] |
| `reader/l2_rqsts.rfo_miss` | paced_r1 | 0.447 [0.438, 0.46] | 0.485 [0.479, 0.492] | 0.448 [0.438, 0.465] | 0.453 [0.437, 0.469] |
| `reader/l2_rqsts.rfo_miss` | paced_r2 | 2.99 [2.96, 3.02] | 2.91 [2.81, 2.96] | 2.99 [2.94, 3.04] | 3.58 [3.55, 3.61] |
| `reader/l2_rqsts.rfo_miss` | paced_r7 | 12.4 [12.3, 12.8] | 11.3 [11.1, 11.6] | 13.8 [13.7, 13.8] | 12.2 [12.2, 12.3] |
| `reader/context-switches` | idle_r1 | 8.12e-07 [6.16e-07, 1.16e-06] | 5.92e-07 [5.17e-07, 6.66e-07] | 6.15e-07 [4.18e-07, 8.36e-07] | 8.86e-07 [5.16e-07, 1.28e-06] |
| `reader/context-switches` | idle_r2 | 0.058 [0.0576, 0.0584] | 0.0444 [0.0442, 0.0447] | 0.0436 [0.0434, 0.0438] | 0.0556 [0.0552, 0.056] |
| `reader/context-switches` | idle_r7 | 0.412 [0.41, 0.414] | 0.411 [0.409, 0.412] | 0.413 [0.412, 0.413] | 0.412 [0.41, 0.413] |
| `reader/context-switches` | paced_r1 | 0.0396 [0.039, 0.0408] | 0.0396 [0.0389, 0.0409] | 0.0397 [0.039, 0.0416] | 0.0403 [0.0389, 0.0417] |
| `reader/context-switches` | paced_r2 | 0.247 [0.245, 0.25] | 0.245 [0.241, 0.249] | 0.249 [0.245, 0.254] | 0.246 [0.241, 0.252] |
| `reader/context-switches` | paced_r7 | 0.754 [0.747, 0.771] | 0.753 [0.738, 0.769] | 0.803 [0.802, 0.804] | 0.798 [0.797, 0.799] |
| `writer/instructions` | paced_r1 | 5,595 [5,580, 5,624] | 5,566 [5,549, 5,597] | 5,580 [5,562, 5,613] | 5,601 [5,572, 5,632] |
| `writer/instructions` | paced_r2 | 6,363 [6,347, 6,392] | 6,341 [6,295, 6,368] | 6,387 [6,357, 6,420] | 6,322 [6,295, 6,357] |
| `writer/instructions` | paced_r7 | 7,988 [7,869, 8,174] | 8,119 [7,931, 8,267] | 8,517 [8,417, 8,595] | 8,607 [8,484, 8,654] |
| `writer/mem_load_l3_hit_retired.xsnp_hitm` | paced_r1 | 3.48 [3.43, 3.57] | 3.61 [3.5, 3.72] | 3.58 [3.46, 3.73] | 3.65 [3.51, 3.88] |
| `writer/mem_load_l3_hit_retired.xsnp_hitm` | paced_r2 | 5.1 [5.01, 5.24] | 5.07 [4.9, 5.22] | 5.43 [5.32, 5.55] | 5.45 [5.31, 5.6] |
| `writer/mem_load_l3_hit_retired.xsnp_hitm` | paced_r7 | 9.48 [9.11, 10.1] | 9.46 [9.09, 9.79] | 11.7 [11.4, 11.9] | 11 [10.9, 11.2] |
| `writer/context-switches` | paced_r1 | 0.176 [0.175, 0.178] | 0.171 [0.17, 0.173] | 0.174 [0.173, 0.176] | 0.176 [0.173, 0.178] |
| `writer/context-switches` | paced_r2 | 0.256 [0.253, 0.258] | 0.254 [0.247, 0.256] | 0.257 [0.255, 0.259] | 0.246 [0.244, 0.249] |
| `writer/context-switches` | paced_r7 | 0.489 [0.467, 0.514] | 0.516 [0.485, 0.539] | 0.556 [0.541, 0.569] | 0.579 [0.557, 0.587] |

**Pin effects that replicate** (13 of 72 (cell, event) pairs): both `one_sibling` runs' intervals are separated from both `0-15` runs', in one direction.

| cell | event | `0-15` runs | `one_sibling` runs |
|---|---|---|---|
| idle_r7 | `reader/cycles` | 6,068, 6,052 | 6,244, 6,155 |
| idle_r7 | `reader/instructions` | 5,908, 5,903 | 5,941, 5,933 |
| idle_r7 | `reader/mem_load_l3_hit_retired.xsnp_hitm` | 8.51, 8.53 | 8.72, 8.7 |
| idle_r7 | `reader/ref-cycles` | 3,139, 3,130 | 3,229, 3,183 |
| idle_r7 | `reader/task-clock` | 0.00196, 0.00195 | 0.00199, 0.00197 |
| paced_r2 | `writer/mem_load_l3_hit_retired.xsnp_hitm` | 5.1, 5.07 | 5.43, 5.45 |
| paced_r7 | `reader/context-switches` | 0.754, 0.753 | 0.803, 0.798 |
| paced_r7 | `reader/instructions` | 9,375, 9,361 | 9,793, 9,720 |
| paced_r7 | `reader/mem_load_l3_hit_retired.xsnp_hitm` | 11.8, 11.7 | 13.8, 12.8 |
| paced_r7 | `writer/context-switches` | 0.489, 0.516 | 0.556, 0.579 |
| paced_r7 | `writer/instructions` | 7,988, 8,119 | 8,517, 8,607 |
| paced_r7 | `writer/l2_rqsts.rfo_miss` | 12.7, 12.8 | 15, 14.3 |
| paced_r7 | `writer/mem_load_l3_hit_retired.xsnp_hitm` | 9.48, 9.46 | 11.7, 11 |

**Between-run spread** (42 same-pin pairs whose intervals separate):

| pin | cell | event | run A | run B |
|---|---|---|---|---|
| `0-15` | idle_r2 | `reader/l2_rqsts.rfo_miss` | 1.11 | 1.97 |
| `one_sibling` | idle_r2 | `reader/l2_rqsts.rfo_miss` | 1.92 | 1.06 |
| `one_sibling` | idle_r2 | `reader/context-switches` | 0.0436 | 0.0556 |
| `0-15` | idle_r2 | `reader/mem_load_l3_hit_retired.xsnp_hitm` | 1.5 | 1.9 |
| `0-15` | idle_r2 | `reader/context-switches` | 0.058 | 0.0444 |
| `one_sibling` | idle_r2 | `reader/mem_load_l3_hit_retired.xsnp_hitm` | 1.91 | 1.5 |
| `one_sibling` | paced_r2 | `reader/l2_rqsts.rfo_miss` | 2.99 | 3.58 |
| `one_sibling` | idle_r1 | `reader/mem_load_l3_hit_retired.xsnp_hitm` | 0.000359 | 0.000315 |
| `one_sibling` | paced_r7 | `reader/l2_rqsts.rfo_miss` | 13.8 | 12.2 |
| `0-15` | idle_r2 | `reader/ref-cycles` | 1,017 | 1,117 |
| `0-15` | idle_r2 | `reader/cycles` | 2,065 | 2,264 |
| `one_sibling` | idle_r2 | `reader/cycles` | 2,270 | 2,064 |
| `one_sibling` | idle_r2 | `reader/ref-cycles` | 1,117 | 1,019 |
| `0-15` | paced_r1 | `reader/l2_rqsts.rfo_miss` | 0.447 | 0.485 |
| `0-15` | paced_r7 | `reader/l2_rqsts.rfo_miss` | 12.4 | 11.3 |
| `one_sibling` | paced_r7 | `reader/mem_load_l3_hit_retired.xsnp_hitm` | 13.8 | 12.8 |
| `one_sibling` | paced_r7 | `writer/mem_load_l3_hit_retired.xsnp_hitm` | 11.7 | 11 |
| `one_sibling` | paced_r7 | `reader/ref-cycles` | 4,869 | 4,586 |
| `one_sibling` | paced_r7 | `reader/cycles` | 9,394 | 8,853 |
| `one_sibling` | paced_r7 | `writer/l2_rqsts.rfo_miss` | 15 | 14.3 |
| `one_sibling` | idle_r2 | `reader/task-clock` | 0.000528 | 0.000505 |
| `0-15` | idle_r2 | `reader/task-clock` | 0.000508 | 0.000529 |
| `one_sibling` | paced_r2 | `writer/context-switches` | 0.257 | 0.246 |
| `one_sibling` | paced_r1 | `writer/l2_rqsts.rfo_miss` | 3.58 | 3.43 |
| `one_sibling` | paced_r2 | `writer/l2_rqsts.rfo_miss` | 5.75 | 5.99 |
| `one_sibling` | paced_r7 | `reader/task-clock` | 0.00329 | 0.00317 |
| `one_sibling` | paced_r7 | `writer/ref-cycles` | 4,295 | 4,150 |
| `one_sibling` | paced_r7 | `writer/cycles` | 8,285 | 8,007 |
| `0-15` | paced_r1 | `writer/context-switches` | 0.176 | 0.171 |
| `0-15` | paced_r1 | `writer/l2_rqsts.rfo_miss` | 3.62 | 3.55 |
| `0-15` | paced_r7 | `reader/cycles` | 8,973 | 8,804 |
| `0-15` | paced_r7 | `reader/ref-cycles` | 4,641 | 4,572 |
| `one_sibling` | idle_r7 | `reader/cycles` | 6,244 | 6,155 |
| `one_sibling` | idle_r7 | `reader/ref-cycles` | 3,229 | 3,183 |
| `one_sibling` | paced_r2 | `writer/instructions` | 6,387 | 6,322 |
| `one_sibling` | idle_r7 | `reader/task-clock` | 0.00199 | 0.00197 |
| `one_sibling` | paced_r7 | `reader/instructions` | 9,793 | 9,720 |
| `one_sibling` | idle_r2 | `reader/instructions` | 2,957 | 2,939 |
| `one_sibling` | paced_r7 | `reader/context-switches` | 0.803 | 0.798 |
| `0-15` | idle_r2 | `reader/instructions` | 2,942 | 2,956 |
| `0-15` | idle_r7 | `reader/task-clock` | 0.00196 | 0.00195 |
| `one_sibling` | idle_r7 | `reader/l2_rqsts.rfo_miss` | 8.2 | 8.17 |

**Achieved rates**, min–max over rounds: aggregate reads (Mops/s) and paced writer inserts/s, in the counters harness and in the wall-clock artifacts.

| instrument | pin | idle R=7 reads | paced R=7 reads | paced R=7 writer |
|---|---|---|---|---|
| counters 34783487238 | `0-15` | 2.631–2.650 | 1.016–1.092 | 206,446–212,955 |
| counters 34784203230 | `0-15` | 2.639–2.658 | 1.015–1.095 | 200,561–208,002 |
| counters 34783862078 | `0,2,4,6,8,10,12,14` | 2.616–2.628 | 1.057–1.101 | 203,253–205,503 |
| counters 34784008215 | `0,2,4,6,8,10,12,14` | 2.634–2.647 | 1.076–1.125 | 201,940–204,872 |
| wall-clock `baseline_concurrent_reads.json` | `0-15` | 2.559–2.589 | 0.969–1.022 | 200,116–225,044 |
| wall-clock `baseline_concurrent_reads_amended_h1.json` | `0,2,4,6,8,10,12,14` | 2.484–2.609 | 0.825–0.850 | 139,698–145,363 |
| wall-clock `baseline_concurrent_reads_amended_h1_run2.json` | `0,2,4,6,8,10,12,14` | 2.442–2.499 | 0.822–0.845 | 139,403–143,861 |
| wall-clock `baseline_concurrent_reads_pin_one_sibling.json` | `0,2,4,6,8,10,12,14` | 2.474–2.600 | 0.837–0.865 | 138,388–144,013 |
| wall-clock `baseline_concurrent_reads_pin_one_sibling_run2.json` | `0,2,4,6,8,10,12,14` | 2.508–2.606 | 0.828–0.848 | 138,559–142,664 |
| wall-clock `baseline_concurrent_reads_run2.json` | `0-15` | 2.478–2.599 | 1.009–1.033 | 213,714–216,479 |

**`perf c2c`**, top three shared cache lines by load HITM. Symbol shares are of that line's local HITMs, as the sampler attributes them.

| run | cell | load HITM | line | address | share of HITM | symbols |
|---|---|---|---|---|---|---|
| `0-15` 34783487238 | idle_r7 | 3,163 | 0 | kernel `0xffff8909c25aa940` | 24.09% | `kernel (unresolved)` 100% |
| `0-15` 34783487238 | idle_r7 | 3,163 | 1 | user `0x7ffe0b342880` | 14.07% | `ExpanseMemTableRep::Get` 59%; `ExpanseMemTableRep::FindLeafBlockForSeek` 14%; `RunCell thread body` 13%; `pthread_mutex_lock` 7% |
| `0-15` 34783487238 | idle_r7 | 3,163 | 2 | user `0x7ffe0b342880` | 8.09% | `kernel (unresolved)` 100% |
| `0-15` 34783487238 | paced_r7 | 1,569 | 0 | kernel `0xffff8909c25d6440` | 14.79% | `kernel (unresolved)` 100% |
| `0-15` 34783487238 | paced_r7 | 1,569 | 1 | user `0x7ffd81430700` | 11.98% | `ExpanseMemTableRep::Get` 45%; `ExpanseMemTableRep::FindLeafBlockForSeek` 20%; `RunCell thread body` 16%; `pthread_mutex_lock` 10% |
| `0-15` 34783487238 | paced_r7 | 1,569 | 2 | user `0x7ffd81430700` | 4.40% | `kernel (unresolved)` 100% |
| `0-15` 34784203230 | idle_r7 | 3,152 | 0 | kernel `0xffff8909c25f7a00` | 25.54% | `kernel (unresolved)` 100% |
| `0-15` 34784203230 | idle_r7 | 3,152 | 1 | user `0x7ffd753890c0` | 13.61% | `ExpanseMemTableRep::Get` 61%; `ExpanseMemTableRep::FindLeafBlockForSeek` 11%; `RunCell thread body` 11%; `pthread_mutex_lock` 9% |
| `0-15` 34784203230 | idle_r7 | 3,152 | 2 | user `0x7ffd753890c0` | 8.85% | `kernel (unresolved)` 100% |
| `0-15` 34784203230 | paced_r7 | 1,557 | 0 | kernel `0xffff8909c25f42c0` | 13.94% | `kernel (unresolved)` 100% |
| `0-15` 34784203230 | paced_r7 | 1,557 | 1 | user `0x7ffc0fa633c0` | 10.40% | `ExpanseMemTableRep::Get` 49%; `ExpanseMemTableRep::FindLeafBlockForSeek` 25%; `pthread_mutex_lock` 19%; `pthread_mutex_unlock` 3% |
| `0-15` 34784203230 | paced_r7 | 1,557 | 2 | user `0x7ffc0fa633c0` | 4.43% | `kernel (unresolved)` 100% |
| `one_sibling` 34783862078 | idle_r7 | 3,157 | 0 | kernel `0xffff8909c25ac9c0` | 23.54% | `kernel (unresolved)` 100% |
| `one_sibling` 34783862078 | idle_r7 | 3,157 | 1 | user `0x7ffcd1599dc0` | 11.21% | `pthread_mutex_lock` 67%; `pthread_mutex_unlock` 27%; `libc.so.6 (unresolved)` 4%; `ExpanseMemTableRep::FindLeafBlockForSeek` 2% |
| `one_sibling` 34783862078 | idle_r7 | 3,157 | 2 | user `0x7ffcd1599dc0` | 10.90% | `kernel (unresolved)` 100% |
| `one_sibling` 34783862078 | paced_r7 | 1,629 | 0 | kernel `0xffff8909c25dac00` | 13.57% | `kernel (unresolved)` 100% |
| `one_sibling` 34783862078 | paced_r7 | 1,629 | 1 | user `0x7ffe528882c0` | 12.03% | `ExpanseMemTableRep::Get` 58%; `pthread_mutex_lock` 21%; `ExpanseMemTableRep::FindLeafBlockForSeek` 14%; `ExpanseMemTableRep::FindLeafBlockForInsert` 3% |
| `one_sibling` 34783862078 | paced_r7 | 1,629 | 2 | user `0x7ffe528882c0` | 4.05% | `kernel (unresolved)` 100% |
| `one_sibling` 34784008215 | idle_r7 | 3,148 | 0 | kernel `0xffff8909c25a8500` | 25.54% | `kernel (unresolved)` 100% |
| `one_sibling` 34784008215 | idle_r7 | 3,148 | 1 | user `0x7ffc6a089e80` | 11.15% | `pthread_mutex_lock` 63%; `pthread_mutex_unlock` 29%; `libc.so.6 (unresolved)` 5%; `ExpanseMemTableRep::FindLeafBlockForSeek` 3% |
| `one_sibling` 34784008215 | idle_r7 | 3,148 | 2 | user `0x7ffc6a089e80` | 10.55% | `kernel (unresolved)` 100% |
| `one_sibling` 34784008215 | paced_r7 | 1,622 | 0 | kernel `0xffff8909c25bc040` | 14.06% | `kernel (unresolved)` 100% |
| `one_sibling` 34784008215 | paced_r7 | 1,622 | 1 | user `0x7fffe173da00` | 8.08% | `pthread_mutex_lock` 55%; `pthread_mutex_unlock` 36%; `libc.so.6 (unresolved)` 7%; `ExpanseMemTableRep::FindLeafBlockForSeek` 2% |
| `one_sibling` 34784008215 | paced_r7 | 1,622 | 2 | user `0x7fffe173da00` | 5.92% | `kernel (unresolved)` 100% |

### 5.12 Pre-registration — the idle curve's shape, checked on reader counts no fit has seen

This section was written after §5.10, §5.11 and the two locate-profile runs below had been read, and before any cell at `R` = 3, 5 or 6 existed. It is **not blind** about `R` = 2, 4 and 7. What it fixes before the new cells are measured is the shapes, the fit, the acceptance rule, and what follows from each outcome (§8.19).

**Why a shape has to be fixed first.** The narrowed-mutex arm would hold the lock around `expanse_map_prev_at_or_before` only, instead of around all of `FindLeafBlockForSeek`. It needs a prediction. The inputs are two locate-profile runs on idle `R = 1` under the one-sibling pin *(measured: reference host — Intel i9-12900F, 8P+8E / 24 threads, 30 MiB L3, Linux 6.8; commits `6bf3f85c` and `a2191c3a`; runs [34790145594](https://github.com/orieg/expanse/actions/runs/34790145594) and [34790867119](https://github.com/orieg/expanse/actions/runs/34790867119); 5 rounds each, about 8,010 LBR samples per round; artifacts under [`results/locate_profile/pin_one_sibling/`](results/locate_profile/pin_one_sibling/))*:

| share of a `Get` | run 34790145594 | run 34790867119 |
|---|---|---|
| inside the locked region (`locked_fraction`) | 0.5094 [0.5045, 0.5162] | 0.5052 [0.5014, 0.5114] |
| inside `expanse_map_prev_at_or_before` (`trie_fraction`) | 0.1158 [0.1149, 0.1167] | 0.1142 [0.1104, 0.1171] |

Every interval overlaps between the two runs, so no cross-run difference is claimed (`docs/BENCHMARKING.md` rule 18).

USL with `alpha` fixed at the locked fraction does not describe the idle curve. On each of the four committed one-sibling idle curves (§5.8 and §5.10), the `beta` it needs is 0.56–0.57 at `R = 2`, 0.28 at `R = 4` and 0.17–0.18 at `R = 7` (`unexplained_term`). A USL has one `beta`, so a bracket built on it is not a prediction. Three more shapes were then fitted to the same curves, and each missed at least one point's interval: `1/S` linear in `R`, a closed queue with a constant handoff cost, and the same queue with that cost growing per waiter. With `S` measured at three reader counts, a two-parameter shape leaves one point to check it on, and trying shapes until one fits is the forking-paths error §8.19 names. So the shapes are fixed here and checked where none of them was fitted.

**The candidates.** Each is a function in [`scripts/rocksdb_locate_bound.py`](../../../scripts/rocksdb_locate_bound.py) (`MODEL_CANDIDATES`), pinned by its `--self-test`:

| candidate | `S(W)` | free parameters (search box) | takes `locked_fraction` |
|---|---|---|---|
| `usl_alpha_profiled` | `W / (1 + f (W − 1) + β W (W − 1))` | `β` ∈ [0, 5] | yes |
| `queue_handoff` | `queue_scaling(W, f, h)` | `h` ∈ [0, 20] | yes |
| `queue_handoff_growing` | `queue_scaling(W, f, h, g)` | `h` ∈ [0, 20], `g` ∈ [0, 5] | yes |
| `reciprocal_linear` | `1 / (a + b W)` | `a` ∈ [−5, 5], `b` ∈ [−1, 1] | no |
| `step_power` | `s₂ (W / 2)^(−γ)` | `s₂` ∈ [0.1, 2], `γ` ∈ [−1, 2] | no |
| `usl_free` | `W / (1 + α (W − 1) + β W (W − 1))` | `α` ∈ [0, 1], `β` ∈ [0, 5] | no |

- **`queue_scaling`** is Reiser and Lavenberg's mean-value analysis for `W` readers in a closed network. Readers spend `1 − f` of an uncontended read at a delay station, then queue at one lock. The lock's service time is `f` with one reader present, and `f + h + g (n − 2)` once `n ≥ 2` are. `h` and `g` are fitted, not counted. A futex wake or a migration is a hypothesis for what they stand for, unmeasured here.
- **Where the shapes came from.** `queue_handoff`, `queue_handoff_growing` and `reciprocal_linear` are the three shapes already tried, now under the fit below. `step_power` was suggested by the measured curves themselves (`S(4) / S(2)` and `S(7) / S(4)` both lie in 0.886–0.919 across the four curves' point estimates) and has no mechanism. `usl_free` is the standard reference.
- **Mechanistic shapes.** Only the three that take `locked_fraction` name a lock parameter that a narrower lock could change.

**The fit.** Each candidate is fitted per run, on that run's `S(2)`, `S(4)` and `S(7)` only.

- **Objective:** minimise `Σ ((S_model(W) − S(W)) / h(W))²`, where `h(W)` is half the width of `S(W)`'s BCa interval (`fit_candidate`).
- **Minimiser:** a deterministic grid followed by a pattern search (`_minimise`). It does not use scipy, so the result does not depend on what the host has installed (§5.10).
- **Search limits.** A parameter may end on a physical limit: a cost of zero, or `α = 1`. A fit whose parameter ends on the edge of its search box has not converged, and is refused.

**The acceptance rule** (`check_candidate`, `model_verdicts`).

- **Per run:** a candidate passes when its fitted curve lies inside the measured BCa 95% interval at every `W` in {2, 3, 4, 5, 6, 7}. Three of those points were fitted and three were not.
- **Runs:** a candidate is `ACCEPTED` only if it passes in both runs.
- **The locked fraction:** the three mechanistic candidates must also pass at `f` = 0.5014, 0.5073 and 0.5162, refitted at each. Those are the lowest interval end, the mean point and the highest interval end across the two profile runs (`profile_fraction_span`).
- **Strictness:** the fit's own uncertainty is not carried into the prediction. That makes the rule stricter than an interval-overlap test, and a correct shape fitted to noisy training points can fail it.

**What follows** (`select_model`, `model_consequence`).

- **More than one accepted:** mechanistic before descriptive, then fewer parameters, then the lower held-out chi-square summed over both runs.
- **A mechanistic candidate accepted:** the narrowed-mutex arm's prediction is derived from it, in that arm's own pre-registration, written before its rounds. How `h` and `g` are taken to change under a narrower lock is stated there, not here.
- **Only a descriptive candidate accepted:** it names no lock parameter. The narrowed-mutex arm is then pre-registered on the directional gate: the paired ratio narrowed/mutex `S(R)`, CI lower bound above 1 at `R = 7`, `R = 1` as the control, both pins.
- **None accepted:** the same directional gate, with every candidate reported with its misses.
- **What acceptance means:** "consistent with six reader counts in two runs". It is not a measured mechanism (§8.20.3).

**The cells, fixed here.**

- **Suite:** `rocksdb_concurrent_heldout`. Idle writer only, `R` = 1 through 7 interleaved within each round, 5 rounds per cell, 2.0 s window, and §5.2's estimator (paired BCa on `S(R) = T(R) / T(1)`). The same binary serves `rocksdb_concurrent`.
- **Pin:** `0,2,4,6,8,10,12,14`, with source `EXPANSE_BENCH_PIN_APPLIED`. `held_out_problems` refuses any other pin, window, round count or mode.
- **Commit:** the `main` commit this section lands on, recorded in each artifact. Each run is fitted and checked only against itself, so that commit's difference from `0ed8f5e5` enters no verdict. A difference from §5.10's curves is not a claim.
- **Runs:** two `bench_baremetal.yml` dispatches with `benchmark_suite=rocksdb_concurrent_heldout` and `cpu_pin=0,2,4,6,8,10,12,14`, the second dispatched after the first completes. Artifacts: `results/baseline_concurrent_reads_heldout.json` and `results/baseline_concurrent_reads_heldout_run2.json`.
- **Pooling:** no round from §5.7–§5.10 is pooled into these.

**How well the cells can discriminate, derived before they exist** (`python3 scripts/rocksdb_locate_bound.py`, on the four committed one-sibling curves at `f` = 0.5073).

| candidate | curves where the training fit lies inside all three intervals |
|---|---|
| `usl_alpha_profiled` | 0 of 4 |
| `queue_handoff` | 0 of 4 |
| `queue_handoff_growing` | 3 of 4 |
| `reciprocal_linear` | 1 of 4 |
| `step_power` | 3 of 4 |
| `usl_free` | 0 of 4 |

- **The two shapes that fit most curves predict close together.** On the curves where both `queue_handoff_growing` and `step_power` fit, their held-out predictions are at most 0.98 to 2.65 training half-widths apart (`held_out_separation`). The gap is largest at `R = 3`: 0.694–0.706 against 0.710–0.716.
- **Consequence:** both can pass in the same run. If both are accepted, the preference order carries `queue_handoff_growing` forward, and the held-out cells will not have excluded `step_power`. The narrowed-arm pre-registration must say so.

**Expected outcome, stated before the cells.**

- **Expected rejected:** `usl_alpha_profiled`, `queue_handoff` and `usl_free`. None fits the training points of any committed curve.
- **No outcome predicted:** `queue_handoff_growing`, `step_power` and `reciprocal_linear`. The first two each missed the training points of one committed curve in four, and `reciprocal_linear` missed three in four.

### 5.13 Outcomes of the §5.12 model check

*(measured: reference host — Intel i9-12900F, 8P+8E / 24 threads, 30 MiB L3, Linux 6.8; commit `b290aa0b`; pin `0,2,4,6,8,10,12,14`, source `EXPANSE_BENCH_PIN_APPLIED`; suite `rocksdb_concurrent_heldout`, idle writer, `R` = 1–7 interleaved, 5 rounds per cell, 2.0 s window; two runs dispatched in sequence after §5.12 merged, [34793233295](https://github.com/orieg/expanse/actions/runs/34793233295) and [34793373222](https://github.com/orieg/expanse/actions/runs/34793373222); artifacts [`results/baseline_concurrent_reads_heldout.json`](results/baseline_concurrent_reads_heldout.json) and [`results/baseline_concurrent_reads_heldout_run2.json`](results/baseline_concurrent_reads_heldout_run2.json); verdicts from `python3 scripts/rocksdb_locate_bound.py`.)*

These outcomes are read against §5.12 as it was merged. No shape was added, refitted or re-weighted after the cells were read, and no round from §5.7–§5.10 is pooled in.

**Both runs are admissible.** `held_out_problems` reports nothing for either. Foreign busy CPU peaked at 0.01 core-equivalents in both, and `load1` peaked at 2.69 and 2.04 on 24 logical CPUs.

| `S(R)` | run 34793233295 | run 34793373222 |
|---|---|---|
| `S(2)` | 0.7623 [0.7549, 0.7677] | 0.7594 [0.7511, 0.7680] |
| `S(3)` | 0.7050 [0.7032, 0.7065] | 0.7081 [0.6999, 0.7164] |
| `S(4)` | 0.6834 [0.6812, 0.6866] | 0.6855 [0.6730, 0.6956] |
| `S(5)` | 0.6764 [0.6658, 0.6877] | 0.6820 [0.6715, 0.6899] |
| `S(6)` | 0.6428 [0.6384, 0.6499] | 0.6499 [0.6419, 0.6609] |
| `S(7)` | 0.6332 [0.6277, 0.6377] | 0.6273 [0.6091, 0.6341] |

Every interval overlaps between the two runs, so no cross-run difference is claimed (`docs/BENCHMARKING.md` rule 18).

**Every candidate is `REJECTED`.** The misses below are the reader counts where the fitted curve lies outside the measured interval, in either run and at any of the three locked fractions.

| candidate | verdict | misses, run 1 | misses, run 2 | held-out chi-square, both runs |
|---|---|---|---|---|
| `usl_alpha_profiled` | `REJECTED` | 2, 3, 4, 5, 6, 7 | 2, 3, 4, 5, 6, 7 | 7309.24 |
| `queue_handoff` | `REJECTED` | 2, 3, 4, 6, 7 | 2, 3, 6, 7 | 252.19 |
| `queue_handoff_growing` | `REJECTED` | 2, 5, 6 (5, 6 at `f` = 0.5162) | 3, 5 | 9.35 |
| `reciprocal_linear` | `REJECTED` | 2, 3, 4, 7 | 3, 4, 5 | 49.76 |
| `step_power` | `REJECTED` | 3, 5 | 5 | 42.79 |
| `usl_free` | `REJECTED` | 2, 3, 4, 5, 6, 7 | 2, 3, 5, 6, 7 | 2438.55 |

- **The three shapes §5.12 expected to fail did.** `usl_alpha_profiled`, `queue_handoff` and `usl_free` miss training points in both runs.
- **The two shapes with no predicted outcome both miss `R = 5` in both runs.** `queue_handoff_growing` predicts 0.6653 and 0.6608 (at the mean locked fraction), and `step_power` 0.6626 and 0.6613, against measured intervals of [0.6658, 0.6877] and [0.6715, 0.6899]. Each also misses one further held-out point in at least one run.
- **No fit ended on a search limit.** Every rejection is an interval miss.

**Consequence (`model_consequence`).** No candidate passed. The narrowed-mutex arm is therefore pre-registered on the directional gate §5.12 names: the paired ratio narrowed/mutex `S(R)`, CI lower bound above 1 at `R = 7`, `R = 1` as the control, both pins. None of the six shapes supplies a numeric prediction for it.

**What the cells show and do not explain.** In both runs `S(5)` sits above the two closest shapes' predictions, and its interval overlaps `S(4)`'s: [0.6812, 0.6866] and [0.6658, 0.6877] in run 1, [0.6730, 0.6956] and [0.6715, 0.6899] in run 2. No candidate has a flat step followed by a further fall: `queue_handoff` flattens from `R = 4` on and misses `R = 6` and `R = 7` in both runs, and the other five fall at every step. The cause is unmeasured. No seventh shape is fitted to it here, because a shape chosen after seeing these cells would be checked on the cells that suggested it (§8.19).

**What is not established.** A shape outside the six may describe the curve. That would need its own pre-registration and fresh cells. `h` and `g` were fitted, never measured, so the rejection says nothing about what a lock handoff costs on this host. The narrowed-mutex arm has not been built or measured.

### 5.14 Pre-registration — the narrowed-mutex arm

This section was written after §5.13 rejected every candidate shape. Under §5.12 that means the arm is gated on a direction, with no numeric prediction. Its authors have seen every full-scope curve in §5.7–§5.13, so the section is **not blind** to the control arm's behaviour. What it fixes before any narrowed cell exists: the change, the gate, the cells, and what each outcome decides (§8.19).

**The change.** `ExpanseMemTableRep` gains `SeekLockScope`, set at construction (`integrations/rocksdb/include/expanse_memtable.h`).

- **`kFullLocate`** (the default, and every earlier cell) holds `mutex_` for all of `FindLeafBlockForSeek`.
- **`kTrieCall`** holds it only for the head/tail load and `expanse_map_prev_at_or_before`. `expanse_map_t` is not safe for a read concurrent with `Insert`'s `expanse_map_insert`, so that call stays under the lock.
- **The leaf walk** that follows (`SettleSeekCandidate`) runs outside the lock, as the walks in `Get`, `Contains` and `IteratorImpl::Seek` already do. Why that is sound is stated beside the function as four invariants of the writer path.
- **The writer** is unchanged: `Insert` holds `mutex_` for its whole body.

**What checks the change.**

- **Test coverage.** The CI TSan lane runs `TestMultiThreadedConcurrentOperations` and `TestHighConcurrencyOptimisticReaders` under both scopes.
- **Two §2.3 mutations, run locally before this section.**
  - Disabling the backward walk fails `TestHighConcurrencyOptimisticReaders`' `assert(found)`.
  - Moving the trie call out of the lock under `kTrieCall` fails the TSan build in three of three runs. TSan reports the walk's `prev_leaf` load racing `SplitLeafBlock`'s construction of a new block.
- **Limits of that coverage.**
  - `libexpanse.a` is not instrumented, so TSan cannot report a race inside the trie itself. It reports this mutation only because the block pointer no longer arrives under `mutex_`.
  - No linearizability checker covers either scope.

**The full-scope path changed as well** (§8.7).

- **What changed:** `FindLeafBlockForSeek` now branches on the scope. Its walk moved into `SettleSeekCandidate`, whose loads are `acquire` rather than `relaxed`; under the mutex that changes no ordering.
- **Consequences:** §5.7–§5.13 describe their own commits. The runs below measure `full` again, interleaved with `trie`, and nothing here is compared against an earlier commit's numbers.

**Hypothesis N.** Narrowing the seek's critical section to the trie call raises read scaling under an idle writer. The profile motivates the direction, not a size: the locked region is 0.5094 [0.5045, 0.5162] and 0.5052 [0.5014, 0.5114] of a `Get`, and the trie call 0.1158 [0.1149, 0.1167] and 0.1142 [0.1104, 0.1171] (§5.12). No magnitude is predicted, because §5.13 left no model to predict one.

**The gate** (`directional_verdict` in `scripts/rocksdb_locate_bound.py`, over the artifact's `lock_scope_ratio`, AGENTS.md §8.20.2).

- **The decision statistic:** the paired ratio `S_trie(7) / S_full(7)` under an idle writer. Each round's quotient uses that round's four cells, `(T_trie(7) / T_trie(1)) / (T_full(7) / T_full(1))`, with a BCa 95% interval over the rounds.
- **Verdicts:** `PASS` when both runs' lower bounds are above 1, and `REFUTED` when both runs' upper bounds are below 1. Anything else is `BOUNDARY_RESULT`, including a bound exactly at 1.
- **Pins:** read separately for `0,2,4,6,8,10,12,14` and for `0-15` (§8.20.5 step 0). One verdict per pin, never pooled.
- **Reported, never gated:**
  - the `R = 1` control `T_trie(1) / T_full(1)`, idle and paced;
  - the idle ratios at `R = 2` and `R = 4`;
  - every paced ratio, with each scope's achieved writer rate per `R` (`paced_rate_check_by_lock_scope`).
- **Why paced is not gated:** the two scopes' paced writers are not held to the same achieved rate. A narrower read lock can change how often the writer wins the lock, so a paced ratio mixes the reader effect with a different writer load.
- **A control whose interval excludes 1** in both runs is reported as a single-reader effect of the scope. The gated ratio divides it out and still decides.

**How small an effect the gate can see, derived before the cells** (`render_gate_detectability`, projected from the committed full-scope curves under the one-sibling pin).

| source curve | `S(7)` relative half-width | projected paired half-width | lower bound clears 1 above |
|---|---|---|---|
| `baseline_concurrent_reads_amended_h1.json`, idle | 1.59% | 2.25% | 1.0230 |
| `baseline_concurrent_reads_amended_h1_run2.json`, idle | 1.28% | 1.81% | 1.0184 |
| `baseline_concurrent_reads_heldout.json`, idle | 0.79% | 1.11% | 1.0113 |
| `baseline_concurrent_reads_heldout_run2.json`, idle | 2.00% | 2.82% | 1.0290 |
| `baseline_concurrent_reads_amended_h1.json`, paced | 0.96% | 1.35% | 1.0137 |
| `baseline_concurrent_reads_amended_h1_run2.json`, paced | 0.79% | 1.12% | 1.0113 |

- **How the projection is built:** `paired_ratio_relative_halfwidth` treats the two arms as independent. Interleaving is meant to correlate them positively, so the measured paired interval is expected to be no wider than this.
- **What it means:** an idle `S(7)` gain below the projected threshold, 1.1–2.9% across these curves, may return `BOUNDARY_RESULT` however real it is.

**The cells, fixed here.**

- **Suite:** `rocksdb_concurrent_narrowed`, which runs `concurrent_read_scaling.py --lock-scopes full,trie --modes idle,paced --readers 1,2,4,7`.
  - Lock scope × writer mode × `R` interleave within each round.
  - 5 rounds per cell, 2.0 s window, paced writer offered 250,000 inserts/s.
  - Estimator as §5.2, plus the paired `lock_scope_ratio`.
- **Commit:** the `main` commit this section lands on, recorded in each artifact.
- **Runs:** four dispatches of `bench_baremetal.yml`, each after the previous completes, in this order: two with `cpu_pin=0,2,4,6,8,10,12,14`, then two with `cpu_pin=0-15`.
- **Artifacts:** `results/baseline_concurrent_reads_narrowed_pin_one_sibling.json` and `_run2`, then `results/baseline_concurrent_reads_narrowed_pin_0-15.json` and `_run2`.
- **Admissibility:** `narrowed_problems` refuses a run whose pin, pin source, window, offered rate, reader counts, modes, scopes or round count differ from these.
- **Pooling:** no round from §5.7–§5.13 is pooled in.

**What each outcome decides.**

- **`PASS` under both pins:** a separate change may propose `kTrieCall` as the default, with its own review of the soundness argument. This section changes no default.
- **`PASS` under one pin only:** the effect is pin-sensitive and is reported as such; the default is unchanged.
- **`BOUNDARY_RESULT` or `REFUTED`:** the default is unchanged, and the option's future is decided in the issue, not here.
- **In every case:** ordered navigation on the OCC surface (#900) is the next arm.

**Expected outcome, stated before the cells.** Idle `S(7)` is expected to `PASS` under both pins: the lock would cover the trie call instead of the whole locate phase. That expectation comes from the profile, not from a model, and it names no size.

**Not covered.**

- **Other callers:** `Contains` and `IteratorImpl::Seek` take the same path but are not swept; the cells time `Get`.
- **The free writer:** not swept.
- **Shared or reader-writer lock:** not an arm here.
- **Correctness of `kTrieCall`** beyond the argument, the TSan lane and the two mutations above is not established.

### 5.15 Outcomes of the narrowed-mutex arm

*(measured: reference host — Intel i9-12900F, 8P+8E / 24 threads, 30 MiB L3, Linux 6.8; commit `ed2a02b9`; suite `rocksdb_concurrent_narrowed`, lock scopes `full` and `trie` interleaved with idle and paced writers at `R` = 1, 2, 4, 7, 5 rounds per cell, 2.0 s window, paced writer offered 250,000 inserts/s, 80 cells per run; four runs dispatched in sequence after §5.14 merged — `0,2,4,6,8,10,12,14`: [34795076442](https://github.com/orieg/expanse/actions/runs/34795076442), [34795328360](https://github.com/orieg/expanse/actions/runs/34795328360); `0-15`: [34795592230](https://github.com/orieg/expanse/actions/runs/34795592230), [34795837264](https://github.com/orieg/expanse/actions/runs/34795837264); pin source `EXPANSE_BENCH_PIN_APPLIED`; artifacts [`results/baseline_concurrent_reads_narrowed_pin_one_sibling.json`](results/baseline_concurrent_reads_narrowed_pin_one_sibling.json), [`…_pin_one_sibling_run2.json`](results/baseline_concurrent_reads_narrowed_pin_one_sibling_run2.json), [`…_pin_0-15.json`](results/baseline_concurrent_reads_narrowed_pin_0-15.json), [`…_pin_0-15_run2.json`](results/baseline_concurrent_reads_narrowed_pin_0-15_run2.json); verdicts from `python3 scripts/rocksdb_locate_bound.py`.)*

These outcomes are read against §5.14 as merged. No threshold, cell or rule was changed after the runs, and no round from §5.7–§5.13 is pooled in.

**All four runs are admissible.** `narrowed_problems` reports nothing for any of them. Foreign busy CPU peaked at 0.04, 0.16, 0.03 and 0.06 core-equivalents, and `load1` at 2.91, 2.98, 2.97 and 2.93 on 24 logical CPUs.

**Hypothesis N `PASS`es under both pins.**

| pin | idle `S_trie(7) / S_full(7)`, run 1 | run 2 | verdict |
|---|---|---|---|
| `0,2,4,6,8,10,12,14` | 1.8692 [1.7927, 1.9379] | 1.7589 [1.6694, 1.8487] | **`PASS`** |
| `0-15` | 1.7731 [1.6436, 1.8943] | 1.7892 [1.6154, 1.9183] | **`PASS`** |

- **Pin sensitivity:** every one-sibling interval overlaps every `0-15` interval, so no pin effect on the gated ratio is claimed (`docs/BENCHMARKING.md` rule 18).
- **Against §5.14's expectation:** the expected outcome was `PASS` under both pins, and it was met. The authors had seen every full-scope curve, so this is weaker evidence than a blind result.
- **Mechanism:** the size of the ratio has no measured mechanism. §5.14 predicted a direction, not a magnitude, and why the trie-call lock gives roughly 1.8× rather than some other factor is unmeasured.

**Reported, not gated.**

| pin | writer | `T(1)` | `S(2)` | `S(4)` | `S(7)` |
|---|---|---|---|---|---|
| `0,2,4,6,8,10,12,14` | idle | 1.0109 [1.0061, 1.0170] · 1.0132 [1.0032, 1.0269] | 2.0639 [1.8049, 2.2164] · 1.9442 [1.7314, 2.1361] | 2.1031 [1.7832, 2.2328] · 1.9455 [1.7015, 2.2099] | gated above |
| `0,2,4,6,8,10,12,14` | paced | 1.0733 [1.0708, 1.0769] · 1.0757 [1.0651, 1.1007] | 2.1790 [2.0153, 2.3432] · 2.2769 [2.0810, 2.4491] | 2.8634 [2.5983, 3.1276] · 2.6002 [2.4040, 2.9739] | 3.1636 [2.8073, 3.5449] · 3.1981 [2.8898, 3.5171] |
| `0-15` | idle | 1.0082 [1.0049, 1.0108] · 1.0103 [1.0074, 1.0133] | 1.8610 [1.6641, 2.0578] · 1.7119 [1.5820, 1.9236] | 2.1945 [1.8758, 2.3110] · 1.7887 [1.7497, 1.8621] | gated above |
| `0-15` | paced | 1.0626 [1.0569, 1.0685] · 1.0739 [1.0676, 1.0781] | 2.3417 [2.1519, 2.4153] · 2.1457 [2.0313, 2.2853] | 2.8682 [2.5579, 3.1997] · 2.6294 [2.4595, 2.9180] | 2.7869 [2.3708, 2.9374] · 2.5531 [2.2331, 2.8731] |

Each cell is run 1 · run 2 of the trie/full paired ratio.

- **The `R = 1` control excludes 1 in every idle and paced run.** One reader goes 0.8–1.3% faster idle, and 6–8% faster paced, under `trie`. §5.14 reports this as a single-reader effect of the scope. The gated ratio divides it out. Its cause is unmeasured.
- **Idle `S(4)` under `0-15` separates between its two runs**: [1.8758, 2.3110] and [1.7497, 1.8621]. It is the only reported ratio that does. The gated `S(7)` overlaps.
- **The paced writers carried different loads.** At `R = 7` the `full` writer reached 135,750–143,566 inserts/s under the one-sibling pin and 201,203–216,810 under `0-15`. The `trie` writer held 249,974–249,985 in every run. Paced ratios therefore mix the reader effect with a heavier writer load on the `trie` side, which is why §5.14 does not gate them. No paced cell was flagged above the offered rate or exhausted.

**What the scope does to the curve** (read Mops/s, mean of 5 rounds, run 1 of each pin; `S(7)` with its interval for both runs).

| pin | scope | writer | `R=1` | `R=2` | `R=4` | `R=7` | `S(7)` run 1 · run 2 |
|---|---|---|---|---|---|---|---|
| `0,2,4,6,8,10,12,14` | `full` | idle | 4.056 | 3.025 | 2.811 | 2.472 | 0.609 [0.597, 0.627] · 0.636 [0.635, 0.638] |
| `0,2,4,6,8,10,12,14` | `trie` | idle | 4.100 | 6.296 | 5.974 | 4.671 | 1.139 [1.083, 1.191] · 1.119 [1.062, 1.176] |
| `0-15` | `full` | idle | 4.072 | 3.083 | 2.761 | 2.546 | 0.625 [0.615, 0.634] · 0.615 [0.595, 0.631] |
| `0-15` | `trie` | idle | 4.106 | 5.776 | 6.106 | 4.557 | 1.110 [1.013, 1.198] · 1.098 [1.021, 1.177] |

- **Adding readers now adds throughput.** Under `full` the idle curve is retrograde in all four runs. Under `trie`, idle `S(7)` sits above 1 with its lower bound above 1 in all four runs.
- **The `trie` curve still falls after `R = 2` or `R = 4`,** so it is not linear scaling. What bounds it is unmeasured: the trie call is still serialised, and the writer mutex still covers `Insert`.
- **Not comparable to earlier sections:** these `full` curves are at `ed2a02b9`, not §5.10's `0ed8f5e5`, and are not compared to them (§5.14).

**Consequence (§5.14).** `PASS` under both pins means a separate change may propose `kTrieCall` as the default, with its own review of the soundness argument in `SettleSeekCandidate`. That change is a promotion under AGENTS.md §2.7: it meets a pre-registered single-threaded bound before any wall-clock run, and keeps `kFullLocate` selectable so the default can itself be compared. This section changes no default. Ordered navigation on the OCC surface (#900) is the next arm.

**What is not established.**

- **Correctness of `kTrieCall`:** nothing beyond the argument, the TSan lane and the two §2.3 mutations in §5.14. No linearizability checker has run on it.
- **`Contains`, `IteratorImpl::Seek` and the free writer:** not measured.
- **Mechanism:** none is attributed. No counters were collected on either scope, so neither the size of the gain, the single-reader control's effect, nor the paced writer's rate difference has a measured cause.
- **What the scope branch costs the default path:** #922 added a runtime branch on the scope to `FindLeafBlockForSeek`, which `kFullLocate` takes too (AGENTS.md §2.1.5). `T(1)` compares the two scopes at `ed2a02b9`; it does not compare `kFullLocate` before and after #922, so that cost on a single-threaded read is unmeasured.

### 5.16 Pre-registration — the optimistic-seek arm (appended 2026-09-15, locked before any optimistic-seek code or cell)

This is step 10 of [#900](https://github.com/orieg/expanse/issues/900), the RocksDB consumer of the ordered reads pre-registered in [`docs/benchmarks/concurrency/METHODOLOGY.md`](../concurrency/METHODOLOGY.md) §12, and the arm §5.14 names next for [#802](https://github.com/orieg/expanse/issues/802). It was written after §5.15 and §12.9 had been read, so it is **not blind** to the `full` and `trie` curves or to the ordered-read outcomes (disclosure below). What it fixes before any code or cell: the prerequisites, the change, the soundness gates, the single-threaded bound, the gate and the ratio reported beside it, the rounds, the cells, what voids a run, and what each outcome decides (AGENTS.md §8.19). §5.1–§5.15 are not edited.

The ordered reads it consumes are the reader-handle calls `expanse_sync_map_reader_{first,last,next_at_or_after,next_after,prev_at_or_before,prev_before}`, provided by [#956](https://github.com/orieg/expanse/pull/956), which is open. Citations of that C ABI below are to the branch of #956. Nothing in this section builds against it; the implementation cannot land before #956 does.

**What `mutex_` covers in a seek today, and what replaces it.** Line numbers are `integrations/rocksdb/src/expanse_memtable.cc` unless another file is named.

| locked work in `FindLeafBlockForSeek` | `kFullLocate` | `kTrieCall` | why it is under the lock | `kOptimistic` |
|---|---|---|---|---|
| `head_` / `tail_` loads and the `h == t` return (`:146`–`:150`, `:168`–`:172`) | locked | locked | consistency with the trie: under the lock, `h == t` means one block exists and the trie is not consulted | acquire loads, no lock. `head_` is stored once, in the constructor (`:77`); `tail_` only by `SplitLeafBlock` (`:302`), a release store after the new block's entries, count and own links are stored (`:280`–`:296`). A reader that loads `tail_` before that store returns the head, and `Get`, `Contains` and `IteratorImpl::Seek` already step forward over `next_leaf` from wherever the locate ends (`:559`, `:438`, `:890`) |
| `expanse_map_prev_at_or_before` on `trie_index_` (`:154`, `:176`) | locked | locked | the one thing `kTrieCall` still needs the lock for: `expanse_map_t` is not safe for a read concurrent with `expanse_map_insert`, which `Insert` (`:369`) and `SplitLeafBlock` (`:310`) call under `mutex_` (`:318`) | `trie_index_` is an `expanse_sync_map_t`, read with `expanse_sync_map_reader_prev_at_or_before` (#956, `include/expanse.h:586`) through the calling thread's reader handle. Its contract: "the entry returned was present, and every key the search passed over was absent, at one instant" (#956, `include/expanse.h:567`–`:576`) |
| the leaf walk, `SettleSeekCandidate` (`:210`–`:253`) | locked | not locked | — | not locked, unchanged |

- **Why the walk's argument carries over.** The four invariants beside `SettleSeekCandidate` (`:191`–`:201`) need the candidate to be a block in the chain, not a candidate taken from a writer-quiescent trie. Every value the trie can hold at any instant names a linked block: `SplitLeafBlock` inserts the new block's prefix (`:310`) after the release store that links it (`:305`), and `Insert` maps a prefix only to the block it just wrote (`:369`). Invariant 1 keeps that block valid for the rep's lifetime. This is an argument, not a proof; the gates below are what check it.
- **The writer.** `Insert` still holds `mutex_` for its whole body. Under `kOptimistic`, its two trie inserts become `expanse_sync_map_insert`. The ordered read in `FindLeafBlockForInsert` (`:107`) goes through the writing thread's reader handle, under `mutex_`, so no other trie writer runs beside it.
- **Lock order.** The sync map's fallback read quiesces writers and takes the sync map's writer lock (#956, `include/expanse.h:519`–`:527`). No reader holds `mutex_`, and the writer path takes `mutex_` before the sync map's writer lock on every path, so the scope introduces one lock order and no inversion. That is derived from the code above, not tested.
- **Not lock-free.** The optimistic read takes no lock on its common path and falls back to the writer-excluding path after its retry budget (#956). AGENTS.md §2.2 applies as written.
- **Other `mutex_` holders.** `ApproximateMemoryUsage` calls `expanse_map_mem_used` under `mutex_` (`:449`). `SyncExpanseMap` exposes no `mem_used` today (`crates/expanse/src/sync.rs`, `impl SyncExpanseMap`), so prerequisite P2 below adds one, and under `kOptimistic` `ApproximateMemoryUsage` calls `expanse_sync_map_mem_used` in its place. `SuggestCompactRange` (`:585`–`:596`) does not touch the trie and is unchanged. Nothing in this section is gated on memory.

**Prerequisites.** Each lands in its own PR, before the implementation.

- **P0.** #956 merged: the ordered reads on the sync map reader handles.
- **P1.** The C ABI states and tests that an `expanse_sync_map_reader_t` may be freed from a thread other than the one that created it, once the creating thread no longer uses it. The rep's destructor relies on exactly that (R2 below).
- **P2.** `SyncExpanseMap::mem_used`, read under the lock as `with_locked` reads, and an `expanse_sync_map_mem_used` C symbol. **The implementation PR does not start before P2 merges.**

**Construction and the reader handles.**

- `SeekLockScope` (`integrations/rocksdb/include/expanse_memtable.h:538`) gains `kOptimistic`. The default stays `kFullLocate` (`:546`), and `ExpanseMemTableRepFactory` (`:610`) is unchanged.
- The trie's type is chosen at construction. `kFullLocate` and `kTrieCall` keep `expanse_map_t`, so their insert path does not take on the sync map's write protocol.
- A reader handle "belongs to the thread that created it and must be freed before its parent container" (#956, `include/expanse.h:532`–`:533`), and one handle's epoch pins are not reentrant (`crates/expanse/src/sync.rs:7261`). Fixed requirements:
  - **R1.** One handle per (rep, thread), never used by two threads.
  - **R2.** The rep keeps a registry of the handles it created, one per thread, and its destructor frees every one of them before `expanse_sync_map_free`, on whichever thread destroys the rep. The rep is destroyed only after every thread has stopped reading it, which RocksDB's memtable lifetime already requires.
  - **R3.** The handle lookup, and any thread-local access it needs, is reached through an out-of-line call on the `kOptimistic` branch only. `kFullLocate`'s and `kTrieCall`'s paths gain no thread-local access (AGENTS.md §2.1.5).
- **R2 relies on a cross-thread free,** so the destructor frees handles created by other threads. #956's header says only that a handle "belongs to the thread that created it and must be freed before its parent container" (`include/expanse.h:532`–`:533`). P1 is what makes that free part of the contract. The Rust side already permits it:
  - **The handle is `Send`.** `expanse_sync_map_reader_t` is `SyncMapReader(MapReader<'static>)` (`crates/expanse-capi/src/modern_sync.rs:22`), and `MapReader` is `{ map: &SyncExpanseMap, reader: Reader }` (`crates/expanse/src/sync.rs:7230`). `Reader` is `{ collector: Arc<Collector>, slot: Arc<Slot> }` (`crates/expanse/src/occ.rs:1867`–`:1870`), with `Slot = AtomicUsize` (`:1097`), and `SyncExpanseMap` is `Sync` through `Shared`'s `unsafe impl Sync` (`sync.rs:1413`). None of the three declares `Send` by hand, and none opts out. A compile check that `SyncExpanseMap`, `occ::Reader`, `MapReader<'static>`, `OwnedMapReader` and `DetachedMapReader` are all `Send` builds at `726b01fc`. That check is a local probe, not a committed test; P1 commits one.
  - **The drop touches no per-thread state.** Neither `SyncMapReader` nor `MapReader` implements `Drop`, and `expanse_sync_map_reader_free` is a `Box` drop (`modern_sync.rs:256`). `Reader`'s drop (`occ.rs:1910`–`:1920`) takes the collector's reader-registry mutex and removes its slot. It reads no thread-local; `writer_slot()` is on the writer's retire path (`occ.rs:1623`), not here.
  - **No pin outlives a call.** `Reader::pin` (`occ.rs:1889`) returns a `Pin<'_>` that borrows the reader's slot (`:1925`), and every C call drops it before returning. A free that starts after the creating thread's last call has returned cannot race a pin.
  - **`Sync` is not the rule to lean on.** `Reader` is also `Sync` by its fields (the same probe builds `is_sync::<Reader>()`), although its doc comment calls it "not `Sync` — one per reading thread" (`occ.rs:1864`–`:1865`). One handle per thread is a usage rule about non-reentrant pins that the type does not enforce, which is why R1 is stated here and tested in G-O4.
  - **What stays unsound, and stays forbidden:** freeing a handle while another thread is still inside a call on it, or after `expanse_sync_map_free`.
- The concurrent harness's `--lock` gains `opt`, and the driver's `LOCK_SCOPES` (`docs/benchmarks/rocksdb_memtable/scripts/concurrent_read_scaling.py:65`) gains `opt`.

**Soundness gates, before any measurement.** No cell below is read until every gate passes on the head being measured, mirroring `docs/benchmarks/concurrency/METHODOLOGY.md` §12.2 and §5.14's TSan note.

- **G-O1 — differential under quiescence.** `integrations/rocksdb/tests/test_differential_memtable.cc` runs its fuzz once per scope. On every probe, `Get` (the matched entries), `Contains`, `IteratorImpl::Seek` and `SeekForPrev` (the entry landed on) agree across the three scopes and with `ReferenceMemTable`. The key sets must include keys that share their first 8 bytes across a block boundary, so the trie's answer lands after the target and the backward walk runs, and a rep with a single block, so the `h == t` return runs.
- **G-O2 — concurrent neighbour check.** A new test, run under all three scopes.
  - Setup: one writer inserts a fixed, shuffled key set and publishes a committed count after each `Insert` returns. Readers sample the count (`c0`), `Seek` a probe `k`, and sample again.
  - Checks: a landed entry is at or above `k` and is a key from the set; no key among the first `c0` committed lies at or above `k` and below the landed entry; a probe equal to one of those keys is found by `Get` and by `Contains`.
  - Why that is a linearizability condition: the memtable is insert-only, so a key committed before a read began is present for all of it. It is a condition on this history, not a general checker.
- **G-O3 — a deterministic reproducer** (AGENTS.md §2.1.5: race timing is never the primary check). A park point compiled only under a test macro sits between the trie read and `SettleSeekCandidate` under `kOptimistic`. The test parks a reader after the trie returns block `B`, has the writer split `B` so the target moves into `B`'s new successor, resumes the reader, and requires `Get`, `Contains` and `Seek` to find the target.
- **G-O4 — handle lifecycle.** Deterministic tests:
  - two threads reading one rep hold different handles (R1);
  - a counter of the handles a rep registered and has not freed reads 0 before the sync map is freed (R2);
  - the destructor, run on a thread that never read the rep, frees handles created by reader threads that are still alive but have stopped reading it (R2, P1);
  - a thread that read from one rep, then from a second rep created after the first was destroyed, holds a new handle for the second.
- **§2.3 fail-then-pass mutations**, each recorded in the implementation PR with the failing test's name:
  - delete `SettleSeekCandidate`'s backward walk: G-O1 fails;
  - under the G-O3 park point, replace `Get`'s `next_leaf` step (`:559`) with a return: G-O3 fails;
  - give every thread one shared handle: G-O4 fails;
  - skip freeing handles in the destructor: G-O4 fails.
- **G-O5 — lanes.** The standard, ASan/UBSan and TSan lanes of `test-rocksdb-memtable` (`.github/workflows/ci.yml:2226`) run G-O1–G-O4 under all three scopes. The engine side is held to §12.2's G12.1–G12.5 on the measured head, with #956 merged.

**What TSan can and cannot see here.** The lane links `libexpanse.a` from `cargo build --release -p expanse-capi` with no sanitizer (`.github/workflows/ci.yml:2246`–`:2247`, `:2474`–`:2478`).

- **A race inside the sync map** is invisible to this lane.
- **The ordering the sync map provides** is invisible too: the happens-before from a writer's `expanse_sync_map_insert` to the block pointer a reader's call returns.
  - §5.14 records that TSan caught its mutation only because the block pointer stopped arriving under `mutex_`. Under `kOptimistic` it never arrives under `mutex_`.
  - So, before any TSan run of this scope: the lane is expected to report the same pair on a correct build, the walk's `prev_leaf` load against `SplitLeafBlock`'s construction of a new block. That is an expectation read from §5.14's report, not an observation, and on its own the lane cannot tell a correct build from that mutation.
- **Compensation.**
  - **The annotation.** In TSan builds only, the rep annotates the ordering the sync map provides on the trie's address (`sanitizer/tsan_interface.h`): `__tsan_release` after each sync-map insert under `mutex_`, and `__tsan_acquire` after each reader call returns. The implementation PR shows the annotation is wired by a fail-then-pass: without the reader-side call, the lane reports.
  - **What the annotation costs.** It asserts the engine's contract, so with it the lane checks the integration's own atomics and not the trie. The contract is checked where the engine is instrumented: §12.2's G12.1–G12.4, the nightly TSan job built with `-Zsanitizer=thread` and `-Zbuild-std` (`.github/workflows/nightly.yml:174`), and #956's C ABI tests.
  - **What runs uninstrumented.** G-O2 and G-O3 run in the standard lane as well, at uninstrumented timing.

**The single-threaded bound, before any wall-clock run** (AGENTS.md §2.7 item 4, #923).

- **Cells.** The four `ExpanseMemTable` phases of `integrations/rocksdb/benches/bench_memtable.cc`, whose rep is built with the default scope (`:481`): `fillrandom` (`Insert`, `:488`), `readrandom` (`Get`, `:539`), `seekrandom` (`IteratorImpl::Seek`, `:601`), `prefixscan` (`IteratorImpl::Next`, `:646`, and `ScanBatch`, `:660`). `memory` is a byte census and is not in the bound.
- **Instrument.** Callgrind `Ir`, inclusive, of those entry points (`callgrind_annotate --inclusive=yes`), from one `bench_memtable --arm <phase> --round 0` process per phase.
  - Base and head are built and run in the same CI job on x86_64 Linux, with the Makefile's flags and the same release `libexpanse`.
  - Base is the implementation PR's merge base, which contains #956.
  - The instrument lands before the change, with a base-against-base control. The control must show no difference on any cell. If it does, the instrument cannot resolve the bound, and the bound is reported unmet rather than widened.
- **Bound.** No cell's inclusive `Ir` rises by more than 0.1%, the AGENTS.md §6 review threshold. Any change beyond ±0.1%, in either direction, is attributed per function (AGENTS.md §6) in the PR body before any wall-clock run.
- **Disassembly.** In the head build, `objdump -d -C` of `FindLeafBlockForSeek`, `SettleSeekCandidate`, `Insert`, `FindLeafBlockForInsert`, `Get`, `Contains` and `IteratorImpl::Seek` shows no `%fs:` access (R3).
- **Reported, not bounded.** The same four cells with a `kOptimistic` rep, through a scope flag on `bench_memtable`. No sign is predicted. `kTrieCall` has no single-threaded cell.
- **Order.** Bound met, then the soundness gates, then the dispatches. A failed bound dispatches nothing: the implementation is changed and re-measured at a new head, and the bound does not move (AGENTS.md §8.19).

**The gate** (`directional_verdict` in `scripts/rocksdb_locate_bound.py`, AGENTS.md §8.20.2). The gated statistic, and the reported O2 beside it, is a paired per-round ratio under an idle writer: `(T_a(7) / T_a(1)) / (T_b(7) / T_b(1))`, from that round's four cells, with a BCa 95% interval over the rounds. The gate is read per pin over that pin's two runs:

- `PASS` when both runs' lower bounds are above 1;
- `REFUTED` when both runs' upper bounds are below 1;
- `BOUNDARY_RESULT` otherwise, including a bound exactly at 1.

Pins `0,2,4,6,8,10,12,14` and `0-15` are read separately and never pooled (AGENTS.md §8.20.5 step 0).

- **O1 (gated): `S_opt(7) / S_full(7)`.** The locate phase no longer takes `mutex_`, against the scope every earlier cell measured.
- **O2 (reported, never gated): `S_opt(7) / S_trie(7)`**, same estimator, read with the same `directional_verdict` wording but deciding nothing.
  - **What §5.15 left.** Under `kTrieCall` the idle curve still falls after `R = 2` or `R = 4`. Idle `S_trie(7)` was 1.139 [1.083, 1.191] and 1.119 [1.062, 1.176] under `0,2,4,6,8,10,12,14`, and 1.110 [1.013, 1.198] and 1.098 [1.021, 1.177] under `0-15` (workload: `rocksdb_memtable_concurrent_read_scaling`; `results/baseline_concurrent_reads_narrowed_*.json`).
  - **What differs between the two scopes.** `kTrieCall` still serialises the trie call, which §5.12's profile put at 0.1158 [0.1149, 0.1167] and 0.1142 [0.1104, 0.1171] of an idle `R = 1` `Get`. `kOptimistic` replaces that lock with a validated walk and an epoch pin, whose cost on this path is unmeasured.
  - **Why no direction is predicted.** §5.13 rejected all six curve shapes, so no model supplies one.
  - **Why it is reported, not gated.** Neither #802 nor #900 depends on it, and it does not set the rounds. A later default proposal that names one of `kTrieCall` and `kOptimistic` over the other needs its own pre-registered comparison; these intervals inform that proposal and do not decide it.
- **Expected outcome, stated before the cells.**
  - **O1:** `PASS` under both pins. `kOptimistic` takes no `mutex_` in its locate phase, and §5.15 measured `S_trie(7) / S_full(7)` above 1 in all four runs with the lock still around the trie call. The expectation comes from having seen §5.15, so a `PASS` is weaker evidence than a blind one.
  - **O2:** no outcome predicted.
- **Reported, never gated:**
  - the `R = 1` controls `T_opt(1) / T_full(1)` and `T_opt(1) / T_trie(1)`, idle and paced;
  - idle `R = 2` and `R = 4` for both ratios;
  - `S_trie(R) / S_full(R)`, re-measured at the new head and not compared with §5.15 (§8.7);
  - every paced ratio, with each scope's achieved writer rate per `R`. Paced cells are not gated, for §5.14's reason: the scopes' paced writers are not held to one achieved rate.
- **A control whose interval excludes 1** in both runs is reported as a single-reader effect of the scope. The gated ratio divides it out and still decides.

**How small an effect the gate can see, and the rounds that follow** (`python3 scripts/rocksdb_locate_bound.py`, `optimistic_gate_sizing`).

- **Inputs.** Every round of the idle `R = 7` cell in the four §5.15 artifacts, read with no filtering: `scaling_by_lock_scope.{full,trie}.idle.S(7).rounds_raw` and `lock_scope_ratio.idle.S(7).rounds_raw`.
- **The assumption.** No `kOptimistic` round exists, so its per-round spread is taken to be `kTrieCall`'s, the other scope that does not hold the lock over the whole locate phase.
- **Planning spread** (per-round coefficient of variation):
  - `opt/full`: the larger, over the four runs, of the measured paired `trie/full` spread and the independent projection `hypot(cv_trie, cv_full)`;
  - `opt/trie`: the largest over the four runs of `hypot(cv_trie, cv_trie)`.
- **Half-width:** `t(0.975, n − 1) × cv / √n`. It is wider than a BCa interval over few rounds, so it errs toward more rounds.
- **Target:** a true O1 ratio of 1.05 must clear 1 at the lower bound. That is a choice, fixed before the sizing was computed, not a derivation. O2 does not set the rounds; its detectable ratio at those rounds is reported.

| source run | CV `full` | CV `trie` | CV paired `trie/full` | `hypot(trie, full)` | `hypot(trie, trie)` |
|---|---|---|---|---|---|
| `baseline_concurrent_reads_narrowed_pin_one_sibling.json` | 0.0305 | 0.0634 | 0.0517 | 0.0704 | 0.0897 |
| `…_pin_one_sibling_run2.json` | 0.0026 | 0.0676 | 0.0672 | 0.0676 | 0.0956 |
| `…_pin_0-15.json` | 0.0192 | 0.1094 | 0.0925 | 0.1111 | 0.1547 |
| `…_pin_0-15_run2.json` | 0.0394 | 0.0914 | 0.1122 | 0.0996 | 0.1293 |

| ratio | role | planning CV | at 5 rounds, the lower bound clears 1 above | rounds to reach 1.05 | at 24 rounds, clears 1 above |
|---|---|---|---|---|---|
| `opt/full` | O1, gated | 0.1122 | 1.1618 | 24 | 1.0497 |
| `opt/trie` | O2, reported | 0.1547 | 1.2377 | 43 | 1.0699 |

- **Rounds per cell: 24**, set by O1 alone. At §5.14's 5 rounds, a true O1 ratio below the projected 1.1618 could read `BOUNDARY_RESULT` however real it is.
- **What remains undetectable.** At 24 rounds, a true O1 ratio below 1.0497 may read `BOUNDARY_RESULT`, and an O2 interval cannot separate a true ratio below 1.0699 from 1. These are projections, not predictions of the outcome.

**The cells, fixed here.**

- **Suite:** `rocksdb_concurrent_optimistic`, which runs `concurrent_read_scaling.py --lock-scopes full,trie,opt --modes idle,paced --readers 1,2,4,7 --rounds 24`.
  - Lock scope × writer mode × `R` interleave within each round: 24 cells per round, 576 per run.
  - 2.0 s window, paced writer offered 250,000 inserts/s, estimator as §5.2.
  - The artifact adds paired ratios for `opt/full`, `opt/trie` and `trie/full`. The instrument PR fixes their key names.
  - An `optimistic_problems` check in `scripts/rocksdb_locate_bound.py` refuses a run whose pin, pin source, window, offered rate, reader counts, modes, scopes, rounds or pairs per ratio differ from these. It lands with the instrument, before any run.
- **Wiring (AGENTS.md §2.7 item 3):**
  - the `.github/bench-suites.json` entry;
  - the `bench_baremetal.yml` dispatch `case`, mirroring `rocksdb_concurrent_narrowed`, including its one-cell smoke run with `--lock opt`;
  - the flag spelling;
  - `rocksdb-concurrent-optimistic.txt` in the upload list;
  - `python3 scripts/check_bench_suites.py --write`;
  - `--lock opt` in the `test-rocksdb-memtable` smoke step;
  - the README's related-links entry.
- **Commit:** the `main` commit the implementation lands on, recorded in each artifact. It contains #956.
- **Runs:** four dispatches of `bench_baremetal.yml` with `benchmark_suite=rocksdb_concurrent_optimistic`, each after the previous completes: two with `cpu_pin=0,2,4,6,8,10,12,14`, then two with `cpu_pin=0-15`.
- **Artifacts:** `results/baseline_concurrent_reads_optimistic_pin_one_sibling.json` and `_run2`, then `results/baseline_concurrent_reads_optimistic_pin_0-15.json` and `_run2`.
- **Pooling:** no round from §5.7–§5.15 is pooled in.

**What voids a run.** In addition to AGENTS.md §6:

- it was dispatched before the single-threaded bound was met, or read before G-O1–G-O5 passed, on the measured head;
- `optimistic_problems` refuses it;
- in any cell the foreign busy CPU exceeds 1.0 core-equivalent, or `load1` exceeds 12 on the 24 logical CPUs at any snapshot (AGENTS.md §8.17). The run is discarded whole, and the discard is disclosed;
- it was dispatched before the previous run of the same sequence completed.

Threshold, method and round count are fixed here. Changing any of them after a run relabels that run `INTERMEDIATE`, with fresh runs (AGENTS.md §8.19). A paced cell that is flagged or whose writer ran out of keys is reported and voids nothing, because no paced cell is gated.

**What each outcome decides.**

- **O1 `PASS` under both pins, two runs each:** #802 closes. Its first "Done when" branch is then measured: a locate phase that does not take `mutex_`, and a concurrent arm that shows the difference with BCa intervals on the reference host. The default does not change here.
  - **Making any scope the default is a separate change** under AGENTS.md §2.7. It has its own single-threaded bound, keeps `kFullLocate` selectable, and migrates every consumer (`ExpanseMemTableRepFactory`, the harnesses and the tests).
- **O1 `PASS` under one pin only:** the effect is pin-sensitive, reported as such, and the default is unchanged.
- **O1 `BOUNDARY_RESULT` or `REFUTED`:** the default is unchanged, and #802's second branch, keeping the mutex with its measurement recorded, is decided in the issue on these runs.
- **O2:** reported with its intervals and decides nothing. It closes no issue and routes no default.
- **#900:** step 10 is done when this section's outcomes are appended, in any direction. Its other open steps, 8 and 9, are #956. Once #956 has merged and these outcomes are appended, no step of #900's plan remains. Neither an O1 nor an O2 `REFUTED` reopens §12.9.

**Prior observations at lock time (disclosure).**

- **§5.15, idle, paired `S_trie(7) / S_full(7)`** (workload: `rocksdb_memtable_concurrent_read_scaling`; `results/baseline_concurrent_reads_narrowed_*.json`):
  - under `0,2,4,6,8,10,12,14`: 1.8692 [1.7927, 1.9379] and 1.7589 [1.6694, 1.8487];
  - under `0-15`: 1.7731 [1.6436, 1.8943] and 1.7892 [1.6154, 1.9183];
  - the idle `T(1)` control: 1.0109 [1.0061, 1.0170] and 1.0132 [1.0032, 1.0269] under `0,2,4,6,8,10,12,14`, and 1.0082 [1.0049, 1.0108] and 1.0103 [1.0074, 1.0133] under `0-15`.
  - Every one of the section's per-round values also feeds the sizing above.
- **§5.12's locate profile** at idle `R = 1` under `0,2,4,6,8,10,12,14`:
  - `locked_fraction` 0.5094 [0.5045, 0.5162] and 0.5052 [0.5014, 0.5114];
  - `trie_fraction` 0.1158 [0.1149, 0.1167] and 0.1142 [0.1104, 0.1171].
- **§5.11's `perf c2c`** under `0,2,4,6,8,10,12,14` at idle `R = 7` attributes 63–67% of the load HITMs on the harness's shared user-space line to `pthread_mutex_lock`. That counter is observational and decides nothing here (§8.20.3).
- **§12.9's P12.5:** at W = 1, R = 4, the optimistic `prev_before` read 91.81 [85.52, 96.74] and 89.47 [84.24, 93.75] times the throughput of `with_locked`, and the W = 0, R = 1 control read 2.15 [2.14, 2.16] and 2.13 [2.12, 2.14] (workloads differ: `concurrency_ordered_readers_map_64bit` vs `rocksdb_memtable_concurrent_read_scaling`).
  - **Why that ratio does not transfer.** Its denominator takes the fallback mutex, quiesces writers and takes the writer mutex on every read (`Shared::with_locked`). A `kFullLocate` or `kTrieCall` seek holds one `std::mutex` around one `expanse_map_t` call and never quiesces a writer. The denominators are different operations, so no magnitude carries over, and O1 and O2 are directional.
- **§12.8:** #928 cost the 64-bit `SyncExpanseMap` readers throughput in `masstree_concurrent` while instructions per read stayed flat, with the difference in `ld_blocks.store_forward`. The ordered reads go through the same `Shared::optimistic_read`. That was measured on another workload and no prediction is drawn from it.

**Explicitly not predicted, and not covered.**

- **Magnitudes:** of O1 and O2, and the sign of O2.
- **Reported cells:** the `R = 1` controls, the paced cells, `S(2)` and `S(4)`.
- **Other operations:** the cells time `Get`; G-O1 and G-O2 cover `Contains`, `IteratorImpl::Seek` and `SeekForPrev` for correctness only.
- **Writers:** the free writer, and multiple writers. `InsertConcurrently` is still `Insert` under `mutex_` (`:377`–`:379`).
- **`kOptimistic`'s single-threaded cost:** reported, not bounded.
- **Memory:** no memory figure is measured or gated for any scope. P2 tests `expanse_sync_map_mem_used` itself.
- **Mechanism:** of any ratio. No counters are collected on this arm.
- **32-bit targets:** the integration uses the 64-bit `expanse_map_t` and `expanse_sync_map_t` surface only.
- **Another lock design:** a shared or reader-writer lock is not an arm here.
