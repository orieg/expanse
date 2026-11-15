# zmap × JudyHS: libexpanse as a drop-in for zmap's response dedup

zmap's window dedup (`lib/cachehash.c`, zmap `d8fc99c9`) indexes an LRU of recent responses with one JudyHS array on 8-byte keys, 1,000,000 entries by default. This suite runs the unmodified zmap against stock libJudy and against `libexpanse`, end to end in an isolated network namespace and in a harness that compiles zmap's `cachehash.c` unchanged. It is the evaluation record behind the zmap row of the top-level README's [Verified in real projects](../../../README.md#verified-in-real-projects) table ([#1681](https://github.com/orieg/expanse/issues/1681)).

The records were measured by earlier evaluation sessions and committed here afterwards. Nothing in this directory was re-measured to commit it. Every result below was recomputed from the committed raw rows by `scripts/analyze.py` and matches the committed `analysis.md` of its record line for line.

The suite has no `/benchmark` token: its harness runs zmap in a privileged container with two network namespaces, which the bare-metal workflow does not provide.

## Records

| Record | libexpanse | Host | What it holds |
|---|---|---|---|
| [`results/era_0_18_82eb56af/`](results/era_0_18_82eb56af/) | `82eb56af` (v0.18.0) | Intel Core i9-12900F reference host, container on `0,2,4,6,8,10,12,14` | the correctness gate (12 live runs, dedup oracle, churn differential, dryrun), the first timed runs, the derived JudyHS footprint, and the library provenance |
| [`results/rerun_573a6f39/`](results/rerun_573a6f39/) | `573a6f39` | same host and pin | timed re-run: end to end and cachehash harness, two runs each |
| [`results/rerun_edee6bfa/`](results/rerun_edee6bfa/) | `edee6bfa` | same host and pin | timed re-run on main, the figures the top-level README cites |

Each timed record holds `e2e_run{1,2}/raw.jsonl` and `micro_run{1,2}/raw.jsonl` (one row per process, with its `load_before` and host busy-CPU in core-equivalents), the `analysis.md` that `scripts/analyze.py` prints for it, and, for the re-runs, the driver's `driver.log` with the quiet-gate and load lines between runs. The re-runs used the 0.18-era pre-registration and harness unchanged, with the container image rebuilt at the named commit.

Peak RSS of the same edee6bfa dense runs, and the sparse-responder memory studies, are in [`results/zmap_judyhs_rss_edee6bfa.json`](../../../results/zmap_judyhs_rss_edee6bfa.json) (#1736) and are not repeated here.

**§8.17 disclosure.** These rows carry a load average before each process and the host busy-CPU over it, the busy-CPU delta §8.17 asks for, per measurement. They do not carry the `provenance.host` and `estimators` blocks of `scripts/bench_provenance.py`, which postdates the 0.18-era harness. The file names sit outside `check_bench_provenance.py`'s governed globs, so the gate does not check them. The host facts are in this README and in `METHODOLOGY.md`'s Design section. The estimator is `scripts/bca_bootstrap.py` at its defaults (2,000 resamples), as pre-registered. Every timed record reports 0 void rounds under the pre-registered rule.

## Pre-registration

[`METHODOLOGY.md`](METHODOLOGY.md) is the pre-registration, byte for byte as it was locked, amendment A1 included. A1 was written before any timed data. Its sha256 is `e534083627f85e42dfea620b5a4602c058b06ea8455a3c52f15c332f67fdbe75`, as recorded in [`results/era_0_18_82eb56af/PREREG.sha256`](results/era_0_18_82eb56af/PREREG.sha256). It is not reconciled with any outcome below.

## Correctness (0.18.0, `82eb56af`)

*(measured: Intel Core i9-12900F reference host, libexpanse `82eb56af`; [`results/era_0_18_82eb56af/correctness/`](results/era_0_18_82eb56af/correctness/))*

- **Live dedup oracle.** Four configurations ran under each of three arms: E1 (one probe), E2 (two probes), and eviction with a 65,536-entry window at two probes and at one probe. The arms are the stock build, the build linked to libexpanse, and the stock binary on libexpanse through `libJudy.so.1`. That makes 12 runs and 9,429,885 output rows. `dedup_oracle.py` replayed each run's arrival-ordered output through a reference LRU and found **0 mismatches** of the `repeat` flag in every run (`oracle_and_churn.txt`; per-run zmap counters in `live.jsonl`).
- **Churn differential.** `diff_churn.c` drove zmap's `cachehash.c` over a 200,000-key pool with a 65,536-entry window, 4,000,000 operations and 1,297,196 hits. Stock, the libexpanse build and the swapped binary gave the same sequence hash, `b38819d521ed1b43`.
- **Dryrun.** The ordered (daddr, dport) target sequence was identical for stock, stock again, libexpanse and the swapped binary. The four files have one sha256, listed in `dryrun_targets.sha256`; `dryrun.targets` is the one copy committed.

These are 0.18-era records. Since then, the differential oracle's JudyHS evict-and-reinsert case (#1739) has checked the same call pattern in CI on every change.

## Results on main (`edee6bfa`)

*(measured: Intel Core i9-12900F reference host, P-cores pinned one SMT sibling each, libexpanse `edee6bfa`; [`results/rerun_edee6bfa/`](results/rerun_edee6bfa/), two runs; ratios are libexpanse ÷ stock with two-sample BCa 95% intervals)*

**cachehash harness, time per response** (below 1 means libexpanse is faster):

| Workload | Run 1 | Run 2 | Verdict |
|---|---:|---:|---|
| `zmap_dedup_m1_miss` | 0.872 [0.869, 0.874] | 0.871 [0.867, 0.874] | faster |
| `zmap_dedup_m2_dup50` | 0.884 [0.882, 0.886] | 0.884 [0.882, 0.886] | faster |
| `zmap_dedup_m3_evict` | 0.759 [0.758, 0.761] | 0.754 [0.729, 0.759] | faster |

**cachehash harness, RSS growth** (below 1 means libexpanse is smaller):

| Workload | Run 1 | Run 2 | Verdict |
|---|---:|---:|---|
| `zmap_dedup_m1_miss` | 1.198 [1.196, 1.199] | 1.196 [1.194, 1.198] | larger |
| `zmap_dedup_m2_dup50` | 1.198 [1.196, 1.199] | 1.197 [1.196, 1.199] | larger |
| `zmap_dedup_m3_evict` | 1.205 [1.204, 1.206] | 1.206 [1.205, 1.207] | larger |

**End to end, receive-thread time per response** (below 1 means libexpanse is faster):

| Workload | Run 1 | Run 2 | Verdict |
|---|---:|---:|---|
| `zmap_e2e_e1_probes1` | 1.0026 [0.9996, 1.0093] | 1.0018 [0.9996, 1.0041] | no difference detected |
| `zmap_e2e_e2_probes2` | 1.0045 [1.0028, 1.0065] | 1.0063 [1.0026, 1.0166] | slower |

End to end, process CPU and wall time show no difference detected on either workload, and dedup is 3.1–5.6% of receive-thread CPU in either arm (H1: off the hot path). The full tables, with the no-Judy floor arm, are in [`results/rerun_edee6bfa/analysis.md`](results/rerun_edee6bfa/analysis.md).

## Earlier records

The two earlier timed records are kept as measured. They show the same workloads at the commits before each engine change on this path.

| Record | cachehash time per response (m1 / m2 / m3, both runs) | e2e receive time per response (e1 / e2) |
|---|---|---|
| `82eb56af` (0.18.0) | 1.570–1.579 / 1.666–1.667 / 1.346: slower | 1.0450–1.0482 / 1.0288–1.0318: slower |
| `573a6f39` | 0.960 / 0.982–0.984 / 0.820–0.825: faster | 1.0066–1.0093: slower / 1.0012–1.0097: no difference detected |

Every interval in this table excludes 1 except the `573a6f39` e2 run 1 cell, 1.0012 [0.9966, 1.0053]. At 0.18.0 the harness's RSS growth was 3.139–3.149× stock's, and the derived JudyHS footprint was about 123–124 B per entry against 17–25 B for stock ([`results/era_0_18_82eb56af/derived_memory.txt`](results/era_0_18_82eb56af/derived_memory.txt), derived from RSS growth, not an allocator census). Intervals for every cell are in each record's `analysis.md`.

## Reproducing

`scripts/Dockerfile` builds zmap `d8fc99c9` twice, against the distro libJudy and against libexpanse at a chosen commit, together with `scripts/cachehash_bench.c`. `scripts/netns_setup.sh` sets up the isolated pair of namespaces, `scripts/correctness.sh` runs the gate, and `scripts/driver.sh` runs e2e and micro twice each through `scripts/e2e.sh`, `scripts/micro.sh` and `scripts/measure.py`. Then:

```bash
python3 docs/benchmarks/zmap_comparison/scripts/analyze.py \
  docs/benchmarks/zmap_comparison/results/rerun_edee6bfa .
```

prints that record's `analysis.md`.

No independent, non-LLM review of this evaluation has been done.
