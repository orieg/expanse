# RocksDB MemTable suite: results and how to read them

*(measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, Linux 6.8, run [33398474866](https://github.com/orieg/expanse/actions/runs/33398474866), commit `6cb64b45`; `benches/bench_memtable.cc` built `-O3` against release `libexpanse.so`; 100,000 keys, 16-byte key, 64-byte value payload; **5 rounds, mean with BCa 95% bootstrap intervals** (2,000 resamples, seed 42) harvested by `scripts/rocksdb_bench_harvest.py` into [`results/baseline_rocksdb.json`](results/baseline_rocksdb.json); every bracketed pair is that arm's interval, and each ratio is a two-sample BCa interval whose **lower bound** clears 1.0. SkipList arm = the fair variable-height baseline. Memory row: deterministic seeded byte accounting, re-measured with the fair variable-height baseline at the #372 fix commit (Apple M1, 8 cores, Apple clang 21, `-O3`; reproduced twice; Expanse/VectorRep cells reproduce the reference-host values byte-for-byte). `SkipListRep`/`VectorRep` are the in-file reference implementations, not stock RocksDB.)*

![RocksDB MemTable Benchmark: ExpanseMemTable vs SkipList vs VectorRep](results/bench_rocksdb.svg)

> **Baseline retraction ([#372](https://github.com/orieg/expanse/issues/372)), now discharged.** Every "vs SkipList" cell below was once measured against a strawman skiplist whose nodes statically embedded the full 16-pointer tower (~146.7 B/entry). The density edge was corrected first — **1.42×**, not 11.11×, by deterministic byte accounting. The **throughput rows are now re-measured against the fair variable-height baseline too** (run [33398474866](https://github.com/orieg/expanse/actions/runs/33398474866)), five rounds with BCa 95% intervals, and every ratio moved down: sequential scan from a published ~10× to **3.331×**, and the retracted-run point-lookup and seek figures to **1.457×** and **1.512×**. The fat-node layout was degrading the skiplist's cache locality exactly as suspected, and the corrected ratios are the smaller, honest ones.

> **The single-threaded cells below are the `6cb64b45` measurement and two of them are superseded.** Re-measured on the reference host at `2b4c15a8` (run [34715487558](https://github.com/orieg/expanse/actions/runs/34715487558)) and at `314deb39` (run [34718103662](https://github.com/orieg/expanse/actions/runs/34718103662)), five rounds each with BCa intervals. Both scan ratios moved **down** with separated intervals in both runs: sequential scan `3.331×` → **2.942× / 3.066×**, batch scan `2.524×` → **2.249× / 2.266×**. Point lookup and range seek did not move detectably (intervals overlap the published ones in both runs). `fillrandom` separated upward in the second run only, and [#873](https://github.com/orieg/expanse/pull/873) changed the engine's mutation paths between the two runs, so that cell is **not attributable** from these measurements and is not restated here.
>
> The scan movement is expected in direction: [#769](https://github.com/orieg/expanse/pull/769) and [#877](https://github.com/orieg/expanse/pull/877) rewrote the iterator and `ScanBatch`, and until #877 the batch-scan loop did not terminate at all — so the published `2.524×` was timing a loop that never completed a scan, not a faster scan. The table is left at its measured-and-committed values rather than backfilled, because [`results/baseline_rocksdb.json`](results/baseline_rocksdb.json) still holds the `6cb64b45` run and the refresh needs the provenance-bearing driver tracked in [#868](https://github.com/orieg/expanse/issues/868). Quoting the cells below as current would be a §8.7 violation; quoting the runs above is not.

| Benchmark Metric | ExpanseMemTable | Reference SkipListRep | VectorRep | Expanse vs SkipList |
|---|---:|---:|---:|---|
| **Memory Footprint** (100K keys) | **1.26 MB** (13.2 B/entry) | 1.8 MB (18.7 B/entry, fair baseline) | **1.0 MB (10.5 B/entry)** | **1.42× Higher Key Density** (VectorRep is denser than both) |
| **Fill Random** (`fillrandom` insert) | **4.42 Mops/s** [4.36, 4.53] | 3.15 Mops/s [3.12, 3.16] | 202.65 Mops/s | **1.406×** [1.385, 1.442] |
| **Point Lookup** (`readrandom`) | **3.79 Mops/s** [3.76, 3.81] | 2.60 Mops/s [2.58, 2.61] | 1.83 Mops/s | **1.457×** [1.444, 1.470] |
| **Range Seek** (`seekrandom`) | **3.67 Mops/s** [3.62, 3.70] | 2.43 Mops/s [2.37, 2.45] | 3.94 Mops/s | **1.512×** [1.492, 1.546] |
| **Sequential Scan** (`prefixscan` Iterator) | **154.18 Mops/s** [151.17, 156.15] | 46.29 Mops/s [44.28, 47.86] | 614.20 Mops/s | **3.331×** [3.198, 3.486] |
| **Batch Scan** (`ScanBatch` 1024-chunk) | **116.82 Mops/s** [114.88, 118.43] | 46.29 Mops/s [44.28, 47.86] | 614.20 Mops/s | **2.524×** [2.421, 2.644] |

> `VectorRep` (an unordered append-only vector) wins on insert and unordered scan by construction — and on raw memory density (10.5 B/entry) — but cannot serve ordered range seeks; it is included as a throughput/density ceiling, not an ordered-index competitor.

### How to read the results

1. **Panel 1: Sequential Scan (`prefixscan`) — 3.331× Speedup [3.198, 3.486]**
   Traversing contiguous 64-byte leaf blocks via intrusive sibling leaf chaining achieves **154.18 Mops/s** vs **46.29 Mops/s** for `ReferenceSkipListRep`, eliminating pointer chasing along skip-list tower links. `VectorRep` scans at **614.20 Mops/s** as an unindexed flat array ceiling.
2. **Panel 2: RAM Footprint per Entry — 1.42× Higher Key Density (13.2 vs 18.7 B/entry)**
   Expanse organizes entry pointers into dense 64-byte aligned blocks indexed by the digital trie, consuming 13.2 bytes of metadata per key vs 18.7 bytes for the fair variable-height skiplist. `VectorRep` uses 10.5 B/entry (single contiguous pointer array).
3. **Panel 3: Point Lookup Latency (`readrandom`) — 1.457× Faster (264 ns vs 385 ns)**
   Bounded $O(k)$ trie descent followed by binary search within the target 64-byte leaf block reduces random read latency to **264 ns** (3.79 Mops/s) vs **385 ns** (2.60 Mops/s) for SkipList.

### Concurrent read scaling (#802)

*(measured: reference host — Intel i9-12900F, 8P+8E / 24 threads, 30 MiB L3, Linux 6.8; commit `0ed8f5e5`; pin `0-15`, one hardware thread per physical P-core; `benches/bench_memtable_concurrent.cc`; 5 rounds per cell, 60 cells per run; **two independent runs**, [34720727337](https://github.com/orieg/expanse/actions/runs/34720727337) and [34721708502](https://github.com/orieg/expanse/actions/runs/34721708502); artifacts [`results/baseline_concurrent_reads.json`](results/baseline_concurrent_reads.json) and [`results/baseline_concurrent_reads_run2.json`](results/baseline_concurrent_reads_run2.json). Pre-registered in [`METHODOLOGY.md`](METHODOLOGY.md) §5 before the harness existed; outcomes in §5.7.)*

`FindLeafBlockForSeek` takes the same `mutex_` that `Insert` holds for its whole body, so every read serialises on the writer's lock before it reaches the per-leaf seqlock. This arm measures what that costs.

Aggregate read throughput, Mops/s, mean of 5 rounds (run 1):

| writer | R=1 | R=2 | R=4 | R=7 | `S(7)` paired BCa 95% |
|---|---:|---:|---:|---:|---|
| **idle** (control) | 4.085 | 3.065 | 2.783 | 2.576 | **0.631** [0.627, 0.633] |
| **paced** (250k/s offered, 5.45% measured duty) | 1.340 | 0.965 | 0.959 | 1.002 | **0.748** [0.733, 0.756] |
| free (reported, never gated) | 0.209 | 0.360 | 0.686 | 0.981 | 4.705 [4.539, 5.154] |

**Reads do not scale; they regress.** `S(7) < 1` means seven readers deliver *less* aggregate throughput than one. The idle control is what makes that attributable: with no writer at all, seven readers deliver 0.63× what one delivers, so the serialisation is reader-against-reader, not reader-against-writer. Both pre-registered gates pass in both runs (H1: paced `S(7)` upper bound 0.756 and 0.771, floor 2.0; H2: idle `S(7)` upper bound 0.633 and 0.629, floor 3.5).

**The free-writer cell is the one that looks like scaling and is not.** Its `S(7)` of 4.705 comes from starvation, not parallelism: a single reader against a free-running writer gets 0.209 Mops/s against the control's 4.085, and adding readers only recovers collectively what one reader could not take from the lock. It was pre-registered as never-gated for exactly this reason.

**What is not claimed.** No hardware counter was collected, so the mechanism behind the regression is unmeasured — `scripts/rocksdb_locate_bound.py` predicted a flat curve and its floor of `S = 1` is refuted, which means there is real cost (cache-line traffic on the mutex word, convoying, wake-up) that the derivation has no term for. A USL fit needs `beta ≈ 0.056` alongside `alpha = 1`, and that `beta` is on the unexplained line (§8.20.4). No alternative design has been built or measured.

### Related links

- Pre-registration and methodology: [`METHODOLOGY.md`](METHODOLOGY.md)
- Raw BCa interval artifacts: [`results/baseline_rocksdb.json`](results/baseline_rocksdb.json) (single-threaded, `6cb64b45`) · [`results/baseline_concurrent_reads.json`](results/baseline_concurrent_reads.json) and [`results/baseline_concurrent_reads_run2.json`](results/baseline_concurrent_reads_run2.json) (concurrent, `0ed8f5e5`)
- Suite reproduction runner: [`run.sh`](run.sh)
- Measurement drivers: [`scripts/single_threaded_bench.py`](scripts/single_threaded_bench.py) (single-threaded rounds) · [`scripts/concurrent_read_scaling.py`](scripts/concurrent_read_scaling.py) (concurrent cells). Each owns the rounds, the cell boundaries, the per-cell load attribution and the intervals; the C++ binaries time one phase or one cell per invocation and emit no interval.
  - The single-threaded cells above are **pending re-measurement** through that driver ([#868](https://github.com/orieg/expanse/issues/868)): they were taken at `6cb64b45` by a shell loop whose artifact carries no load snapshot and no raw rows, and the engine has changed since. They are not retracted — they are what was measured, under the provenance stated above — and the ratio estimator changes with the re-run, from a two-sample interval over the two arms' rounds to a paired interval over the per-round quotient, so the replacement cells will not be a cell-for-cell swap.
- C++ MemTable implementation and build instructions: [`integrations/rocksdb/`](../../../integrations/rocksdb/README.md)
- Canonical benchmarking and database guides: [`docs/BENCHMARKING.md`](../../BENCHMARKING.md) · [`docs/DATABASE.md`](../../DATABASE.md)
