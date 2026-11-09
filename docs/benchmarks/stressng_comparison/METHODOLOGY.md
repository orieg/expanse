# stress-ng × Judy: methodology and pre-registration

This is the pre-registration for item 4 of [#1666](https://github.com/orieg/expanse/issues/1666): the re-measure of stress-ng's Judy stressors after items 1–3 (AGENTS.md §8.8 commit 2). It is committed before the first timed process of this measurement and is frozen from then on. Outcomes go in [`README.md`](README.md) and are never reconciled into this file. A bound, method, arm or sample size changed after timed data exists relabels the result `INTERMEDIATE` and needs fresh runs (AGENTS.md §8.19).

## 1. The question

stress-ng links libJudy for two stressors. When the stock library is swapped for `libexpanse` at runtime (the same binary, a different `libJudy.so.1`), how do stress-ng's own per-phase rates and its resident memory compare on two hosts of different microarchitecture?

An earlier evaluation answered this at Expanse `82eb56af`, on the reference host only, with nothing committed. Its findings are the basis of #1666 items 1–3. This measurement repeats it on the current main and adds a second host. Its artifacts carry AGENTS.md §8.17 provenance, and its workloads carry §8.12 shapes.

## 2. Inputs, frozen

| Item | Value |
|---|---|
| stress-ng | `github.com/ColinIanKing/stress-ng` at `d5f8133a0cc5d2fa91695c6ac5c28f5cc4a3bdf5` (V0.22.02-39), built in `ubuntu:22.04` with gcc 11.4.0 and glibc 2.35, using stress-ng's default flags against the distro `Judy.h` (`scripts/build.sh`). Its `DT_NEEDED` is `libJudy.so.1`. |
| Stock libJudy | Ubuntu jammy `libjudydebian1_1.0.5-5_amd64.deb` and `libjudy-dev_1.0.5-5_amd64.deb`, extracted with `dpkg-deb -x` (no install); `libJudy.so.1.0.3`. |
| Expanse | `libexpanse.so` from main `6ec29a656`, built with `EXPANSE_LINK_SONAME=1 cargo build --release -p expanse-capi` (rustc 1.98.1), giving `DT_SONAME libexpanse.so.1`. It is laid out as the `libjudy-compat` package installs it: `libexpanse.so.1.0.0`, `libexpanse.so.1`, `libexpanse.so`, and `libJudy.so.1 → libexpanse.so.1`. sha256 `da5814a70f316a4114449a51d53ec7798d20d2376a0ec652218b124a9627804a`. |
| Change since `82eb56af` on the paths stress-ng calls | None. `crates/expanse/src` is unchanged. In `crates/expanse-capi` only `Judy1Count`/`JudyLCount` (their zero-count `JError`, #1673) and the SONAME link argument (#1667) changed, and stress-ng calls neither function. |

The binary, both library files and their hashes are recorded in each run's provenance.

## 3. Hosts

| Label | Host | Pin (one thread per physical core) | Governor |
|---|---|---|---|
| `ref` | Reference host: Intel Core i9-12900F (8 P-cores + 8 E-cores, 24 threads), Ubuntu 22.04, Linux 6.8 | `0,2,4,6,8,10,12,14` (P-cores, one SMT sibling each) | as found and recorded per CPU (`powersave` at the earlier evaluation) |
| `zen5` | Second host: AMD Ryzen 9 9955HX (16 cores, 32 threads; SMT siblings `N` and `N+16`), Ubuntu 24.04 | `0-7` | `performance` for the runs, recorded per CPU |

Pins are applied through `scripts/bench_pin.sh` with `EXPANSE_BENCH_PIN`. Every stress-ng worker is an independent busy process, so one CPU per worker is the right pin (AGENTS.md §8.20.5 step 0). The second host runs only with the owner's approval for the governor change and for stopping that host's CI runner during the timed rounds. Without that approval the `zen5` half is not run, and the README says so.

## 4. Arms

- **S (stock):** the binary above with `LD_LIBRARY_PATH=<stock libdir>`.
- **E (Expanse):** **the same binary** with `LD_LIBRARY_PATH=<expanse libdir>`, so `libJudy.so.1` resolves to `libexpanse.so.1.0.0`. The library is the only variable. The inlined Judy macros are the same object code in both arms.

Which library actually loaded is shown per run, by reading `/proc/<worker>/maps` of one live worker per arm in a probe before the timed rounds (`run.sh`).

## 5. Workloads (AGENTS.md §8.12)

Common to every workload:
- `insertion_order`: `generator` (stress-ng's own order).
- `value_dereference`: find writes nothing; sparsematrix get reads `*pvalue`.
- `measured_region`: stress-ng's own per-phase timers (`stress_time_now()` around each loop) for the per-operation rates, and the whole process for wall clock.
- `arm_symmetry`: same binary, same seed, same operations; only the shared library differs.
- `statistics`: two-sample BCa 95% interval on the ratio of means E/S, 2,000 resamples, seed 42 (`scripts/bca_bootstrap.py::bca_bootstrap_ratio_ci_with_method`), per run.
- `verdict`: §8.

| workload_id | stressor arguments | population | probes and reuse | hit rate | miss generation |
|---|---|---|---|---|---|
| `sng_judy_1k` | `--judy W --judy-size 1024 --judy-ops 5000·W` | 1,024 keys per worker | each bogo op: n `JLI` in index order, n `JLG` in the same order, n `JLD` in reverse order; the array is rebuilt from empty every bogo op | 100% (find and delete) | none |
| `sng_judy_256k` | `--judy-size 262144 --judy-ops 40·W` | 262,144 | as above | 100% | none |
| `sng_judy_4m` | `--judy-size 4194304 --judy-ops 3·W` | 4,194,304 (the stressor's maximum) | as above | 100% | none |
| `sng_sm_judy_default` | `--sparsematrix W --sparsematrix-method judy --sparsematrix-ops 1000·W` | ≤ 10,000 random (x, y) in 500 × 500, key `x << 32 \| y` | per bogo op: n get-then-put, n verified gets, n random gets, n get-then-zero; `JLMU` and `JLFA` at the end | put phase mostly misses; verify phase 100%; random-get phase mostly misses | stress-ng's PRNG, `--seed 12345` |
| `sng_sm_judy_big` | `… --sparsematrix-items 1000000 --sparsematrix-size 100000 --sparsematrix-ops 4·W` | ≤ 1,000,000 in 10⁵ × 10⁵ | as above | as above | as above |
| `sng_sm_hashjudy_default` | `--sparsematrix-method hashjudy --sparsematrix-ops 100·W` | 500 JudyL arrays (one per x), key y | as above, but stress-ng's `hashjudy_destroy` calls `JLMU` and never `JLFA`, so the arrays leak every bogo op in both arms | as above | as above |
| `sng_sm_hashjudy_big` | `… 1000000 items, size 100000, --sparsematrix-ops 1·W` | 100,000 JudyL arrays | as above | as above | as above |

- **Worker counts:** `sng_judy_*` at W ∈ {1, 2, 4, 8}; `sng_sm_judy_*` at W ∈ {1, 8}; `sng_sm_hashjudy_*` at W ∈ {1, 4}. The hashjudy operations and W are bounded because of the leak, which reached about 86 GB at W = 8 in the earlier evaluation's smoke test.
- **Weak scaling:** `--ops` scales with W, so each worker does the same work.
- **What W means:** each stress-ng worker is a separate process with its own array. This measures single-threaded per-process Judy cost under 1–8 co-running copies, not Expanse's concurrent mode.
- **Fixed `--ops` rather than `--timeout`:** on a timeout, stress-ng's find loop stops early but still credits all n finds. Every process gets `-t 300` as a safety bound only.

## 6. Metrics

**Primary**, per cell: stress-ng's YAML metric, the harmonic mean over instances as stress-ng reports it.
- `sng_judy_*`: `judy-insert-operations-per-sec`, `judy-find-operations-per-sec`, `judy-delete-operations-per-sec`.
- `sng_sm_judy_*`: `judy-puts-per-sec`, `judy-gets-per-sec`.
- `sng_sm_hashjudy_*`: `hashjudy-puts-per-sec`, `hashjudy-gets-per-sec`.

**Secondary:**
- `bogo-ops-per-second-real-time`;
- stress-ng's `max-rss`;
- the process wall clock, measured outside stress-ng (`time.monotonic` around `wait4`), as a cross-check of stress-ng's timer. A cell where the wall-clock ratio and the inverse bogo-ops/s ratio differ by more than 5% is reported as such.

## 7. Sample, order, independence and host hygiene (AGENTS.md §8.17)

- **Runs:** two independent runs per host, each a separate `run.sh` invocation holding the host bench lock for its whole duration. A run is 10 rounds. Each round runs every (cell, arm) once, one stress-ng process each. The cell order is shuffled per round (seed = run number · 1000 + round), and the arm order alternates by round. So n = 10 per arm per cell per run. Every process is a fresh start, and no warm-up is excluded.
- **Refusals before a run starts:**
  - a running container, or a sibling or foreign workload (`zmap`, `mysqld`, `mariadbd`, `kvbench`, the host's CI runner);
  - less than 20 GB free on the result volume;
  - load1 above 1.5 after a bounded 15-minute wait for decay;
  - a bench workflow in flight (checked by the operator with `gh run list`). Dispatch avoids the reference host's hourly maintenance rsync.
- **Per-process load window:** `bench_provenance.begin_cell` / `end_cell` around every process. The process's foreign load is the host's busy CPU over the process wall time minus the runner's children's CPU, in core-equivalents.
- **Discards and voids:**
  - a process with foreign load above 0.5 core-equivalents is **discarded**: kept in the raw rows, flagged, excluded from statistics and disclosed;
  - a cell-arm with fewer than 8 valid samples in a run is `INCONCLUSIVE` for that run;
  - a run in which load1 exceeds half the host's logical CPUs at any snapshot is void.

## 8. Decision rule

Per host, per cell and per primary metric, on R = E/S (higher means Expanse is faster):

1. **FASTER:** the BCa lower bound is above 1.00 in both runs.
2. **SLOWER:** the BCa upper bound is below 1.00 in both runs.
3. **EQUIVALENT:** both runs' intervals lie inside [0.95, 1.05].
4. Otherwise **INCONCLUSIVE**, which includes runs that disagree in direction.

**Headline H:** "Expanse is a performance-neutral-or-better runtime drop-in for stress-ng's Judy users." Per host, H holds if and only if no primary cell-metric is SLOWER with an upper bound below 0.95 in both runs. Every SLOWER cell is reported, whatever its size.

## 9. Predictions (written before any timed data of this measurement)

The engine is unchanged on every path stress-ng calls (§2), so the reference-host predictions are a replication of the `82eb56af` evaluation. They are therefore informed by its results and are not blind.

- **R1 (reference host):** every primary cell-metric's verdict equals its `82eb56af` verdict:
  - `sng_judy_*` insert FASTER at 1k and 256k, SLOWER at 4m;
  - find FASTER at 1k and 256k; at 4m INCONCLUSIVE at W ≤ 2 and FASTER at W ≥ 4;
  - delete SLOWER at every size;
  - `sng_sm_judy_*` gets SLOWER; `sng_sm_judy_big` puts FASTER; `sng_sm_judy_default` puts SLOWER by less than 5%;
  - `sng_sm_hashjudy_*` puts SLOWER; `sng_sm_hashjudy_default` gets FASTER; `sng_sm_hashjudy_big` gets SLOWER.

  A cell whose verdict differs is reported as a failed replication, cell by cell. The earlier evaluation ran hashjudy at `--sparsematrix-ops` 100·W and 1·W, the same as §5.
- **R2 (reference host):** H is NOT MET, through the same 26 cell-metrics.
- **Z1 (second host):** every primary cell-metric whose reference-host interval lay entirely outside [0.95, 1.05] in both `82eb56af` runs keeps its direction (FASTER or SLOWER).
- **Z2 (second host):** H is NOT MET. Magnitudes and the remaining cells are not predicted.
- **Not predicted:** W-dependence on the second host, and every RSS ratio on the second host.

## 10. Instruments and artifacts

- `run.sh <run> <host-label>`:
  - takes the host bench lock (`scripts/bench_lock.py`) and the pin (`scripts/bench_pin.sh`);
  - refuses on the §7 conditions;
  - proves the loaded library per arm;
  - runs `scripts/driver.py`, which writes one raw row per process plus the run's provenance.
- `scripts/analyze.py <host-label> <run1> <run2>`:
  - writes `results/baseline_<host-label>_run{1,2}.json`, each carrying `provenance` (host facts, estimators, load snapshots with busy-CPU deltas) and per-cell `rounds_raw`, with every interval's `ci_method`;
  - prints the §8 verdict table that the README carries.
- The independent, non-LLM review that #1666 item 4 requires before any statement to stress-ng or to distribution packagers is outside this suite. Its absence is stated in the README until it happens.
