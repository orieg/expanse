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
  - What that mutex permits is derivable ahead of the arm, and is derived: `scripts/rocksdb_locate_bound.py` computes the aggregate read ceiling, its independence from the reader count, the shared-lock twin's prediction, and the inverse that turns a measured scaling ratio back into the share of a read the lock covers. It reads `insert_ns` and `read_ns` from [`results/baseline_rocksdb.json`](results/baseline_rocksdb.json) rather than carrying copies, so a re-measurement re-derives the prediction; its arithmetic is pinned by `--self-test` in the `lint` job. The numbers it returns are `(projected)` and no hypothesis is pre-registered from them yet.

## 5. Pre-registration — concurrent read scaling under one writer (#802)

Locked before any concurrent measurement exists, and before the harness that will produce it is written (AGENTS.md §8.8 commit 2). Nothing in §1–§4 is restated or revised here; those cells are single-threaded and stay as they are. No data appears in this section: outcomes are appended beside each hypothesis when the arm has run, never reconciled in place (§8.7).

### 5.1 The question, and why it needs its own arm

`FindLeafBlockForSeek` (`integrations/rocksdb/src/expanse_memtable.cc:137`) opens with `std::lock_guard<std::mutex> lock(mutex_)`, and is the locate path for `Contains`, `Get` and `IteratorImpl::Seek`. It is the same `mutex_` that `Insert` holds for its whole body. The per-leaf seqlock therefore protects the in-block scan *after* a reader has already serialised, and every cell in §2 is single-threaded, so nothing measured in this suite so far can show a read-scaling change in either direction.

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

- Multi-writer scaling. `InsertConcurrently` is `Insert` verbatim (`:337`), so writers are fully serialised by construction; measuring that is a separate question from the read path.
- Iterator and scan concurrency beyond the `Seek` locate phase; the scan bracket was closed in #769.
- Any change to the locate phase. This section pre-registers the measurement only; a design lands against its result, not beside it.
