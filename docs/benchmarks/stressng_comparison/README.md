# stress-ng × Judy: Expanse as a runtime libJudy replacement

stress-ng's `judy` and `sparsematrix` stressors run one binary against stock libJudy and against `libexpanse`. The library is swapped through `libJudy.so.1` and nothing else changes. This suite is item 4 of [#1666](https://github.com/orieg/expanse/issues/1666): the re-measure after the packaging fix (#1667), the Callgrind arms (#1668) and the documented small-array floor (#1669).

- **Pre-registration:** [`METHODOLOGY.md`](METHODOLOGY.md). It was committed before the first timed process of this measurement and is never edited to match outcomes.
- **Status:** the reference host has been measured (below). The second host waits on approval for its governor change and for pausing its CI runner (METHODOLOGY.md §3).
- **Review:** no independent, non-LLM review has been done. #1666 item 4 requires one before any statement to stress-ng or to distribution packagers.

## Results: reference host

(measured: reference host Intel Core i9-12900F, P-cores pinned one SMT sibling each `0,2,4,6,8,10,12,14`, governor `powersave` on every pinned CPU, Linux 6.8, harness `1b71c3546`, libexpanse from main `6ec29a656`; artifacts `results/baseline_ref_run1.json`, `results/baseline_ref_run2.json`; workloads: each row's `workload_id`)

### Host hygiene

- **Dispatch:** both runs held the host bench lock. No bench workflow was in flight at dispatch.
- **Early dispatch:** the launcher's time check failed, so both runs started at 23:49 UTC rather than the booked 00:19. They ran from 23:49:16 to 00:02:51 UTC, between two runs of the host's hourly maintenance job, which the booked slot existed to avoid.
- **Load:** 800 processes ran, with 0 non-zero exits and 0 stress-ng failures.
  - Run 1: foreign load median 0.00, max 0.07 core-equivalents.
  - Run 2: 1 process discarded at 1.08 core-equivalents (`sng_sm_hashjudy_default`, W = 1, arm S, round 5), leaving 9 valid samples in that cell-arm.
  - Max load1 was 4.90 and 4.38, against a void threshold of 12.
- **Library proof:** each run's provenance carries the `/proc/<worker>/maps` proof. Arm S mapped `libJudy.so.1.0.3`; arm E mapped `libexpanse.so.1.0.0`.

### Primary metrics (E/S, higher means Expanse is faster)

| workload | W | metric | run 1 E/S [BCa 95%] | run 2 E/S [BCa 95%] | verdict |
|---|---|---|---|---|---|
| `sng_judy_1k` | 1 | `judy-delete-operations-per-sec` | 0.814 [0.813, 0.816] | 0.816 [0.814, 0.817] | SLOWER |
| `sng_judy_1k` | 1 | `judy-find-operations-per-sec` | 0.962 [0.956, 0.967] | 0.966 [0.962, 0.973] | SLOWER |
| `sng_judy_1k` | 1 | `judy-insert-operations-per-sec` | 1.105 [1.095, 1.110] | 1.108 [1.104, 1.112] | FASTER |
| `sng_judy_1k` | 2 | `judy-delete-operations-per-sec` | 0.816 [0.812, 0.820] | 0.818 [0.813, 0.823] | SLOWER |
| `sng_judy_1k` | 2 | `judy-find-operations-per-sec` | 0.958 [0.953, 0.963] | 0.964 [0.958, 0.969] | SLOWER |
| `sng_judy_1k` | 2 | `judy-insert-operations-per-sec` | 1.111 [1.105, 1.116] | 1.114 [1.107, 1.121] | FASTER |
| `sng_judy_1k` | 4 | `judy-delete-operations-per-sec` | 0.813 [0.809, 0.817] | 0.816 [0.811, 0.821] | SLOWER |
| `sng_judy_1k` | 4 | `judy-find-operations-per-sec` | 0.957 [0.951, 0.962] | 0.958 [0.953, 0.964] | SLOWER |
| `sng_judy_1k` | 4 | `judy-insert-operations-per-sec` | 1.100 [1.094, 1.106] | 1.107 [1.100, 1.114] | FASTER |
| `sng_judy_1k` | 8 | `judy-delete-operations-per-sec` | 0.815 [0.812, 0.817] | 0.813 [0.811, 0.814] | SLOWER |
| `sng_judy_1k` | 8 | `judy-find-operations-per-sec` | 0.955 [0.948, 0.958] | 0.956 [0.953, 0.958] | SLOWER |
| `sng_judy_1k` | 8 | `judy-insert-operations-per-sec` | 1.116 [1.110, 1.119] | 1.117 [1.115, 1.118] | FASTER |
| `sng_judy_256k` | 1 | `judy-delete-operations-per-sec` | 0.752 [0.737, 0.772] | 0.746 [0.728, 0.762] | SLOWER |
| `sng_judy_256k` | 1 | `judy-find-operations-per-sec` | 1.142 [1.121, 1.168] | 1.111 [1.097, 1.126] | FASTER |
| `sng_judy_256k` | 1 | `judy-insert-operations-per-sec` | 1.138 [1.117, 1.167] | 1.139 [1.115, 1.161] | FASTER |
| `sng_judy_256k` | 2 | `judy-delete-operations-per-sec` | 0.760 [0.748, 0.775] | 0.763 [0.751, 0.773] | SLOWER |
| `sng_judy_256k` | 2 | `judy-find-operations-per-sec` | 1.136 [1.123, 1.152] | 1.132 [1.116, 1.151] | FASTER |
| `sng_judy_256k` | 2 | `judy-insert-operations-per-sec` | 1.153 [1.139, 1.170] | 1.160 [1.142, 1.176] | FASTER |
| `sng_judy_256k` | 4 | `judy-delete-operations-per-sec` | 0.766 [0.759, 0.772] | 0.768 [0.763, 0.772] | SLOWER |
| `sng_judy_256k` | 4 | `judy-find-operations-per-sec` | 1.117 [1.108, 1.125] | 1.122 [1.113, 1.130] | FASTER |
| `sng_judy_256k` | 4 | `judy-insert-operations-per-sec` | 1.175 [1.160, 1.185] | 1.180 [1.172, 1.189] | FASTER |
| `sng_judy_256k` | 8 | `judy-delete-operations-per-sec` | 0.779 [0.776, 0.782] | 0.780 [0.778, 0.783] | SLOWER |
| `sng_judy_256k` | 8 | `judy-find-operations-per-sec` | 1.110 [1.107, 1.113] | 1.105 [1.101, 1.108] | FASTER |
| `sng_judy_256k` | 8 | `judy-insert-operations-per-sec` | 1.177 [1.171, 1.179] | 1.173 [1.170, 1.177] | FASTER |
| `sng_judy_4m` | 1 | `judy-delete-operations-per-sec` | 0.644 [0.639, 0.649] | 0.641 [0.638, 0.646] | SLOWER |
| `sng_judy_4m` | 1 | `judy-find-operations-per-sec` | 0.997 [0.965, 1.050] | 1.049 [0.999, 1.098] | INCONCLUSIVE |
| `sng_judy_4m` | 1 | `judy-insert-operations-per-sec` | 0.882 [0.877, 0.890] | 0.884 [0.879, 0.890] | SLOWER |
| `sng_judy_4m` | 2 | `judy-delete-operations-per-sec` | 0.646 [0.643, 0.650] | 0.649 [0.645, 0.654] | SLOWER |
| `sng_judy_4m` | 2 | `judy-find-operations-per-sec` | 0.999 [0.964, 1.029] | 1.020 [0.989, 1.047] | EQUIVALENT |
| `sng_judy_4m` | 2 | `judy-insert-operations-per-sec` | 0.880 [0.877, 0.883] | 0.882 [0.873, 0.890] | SLOWER |
| `sng_judy_4m` | 4 | `judy-delete-operations-per-sec` | 0.640 [0.636, 0.645] | 0.640 [0.635, 0.644] | SLOWER |
| `sng_judy_4m` | 4 | `judy-find-operations-per-sec` | 1.034 [1.008, 1.054] | 1.061 [1.049, 1.071] | FASTER |
| `sng_judy_4m` | 4 | `judy-insert-operations-per-sec` | 0.880 [0.876, 0.885] | 0.882 [0.878, 0.887] | SLOWER |
| `sng_judy_4m` | 8 | `judy-delete-operations-per-sec` | 0.644 [0.643, 0.646] | 0.644 [0.643, 0.645] | SLOWER |
| `sng_judy_4m` | 8 | `judy-find-operations-per-sec` | 1.074 [1.072, 1.075] | 1.075 [1.074, 1.077] | FASTER |
| `sng_judy_4m` | 8 | `judy-insert-operations-per-sec` | 0.891 [0.888, 0.893] | 0.893 [0.892, 0.895] | SLOWER |
| `sng_sm_hashjudy_big` | 1 | `hashjudy-gets-per-sec` | 0.419 [0.409, 0.426] | 0.435 [0.423, 0.462] | SLOWER |
| `sng_sm_hashjudy_big` | 1 | `hashjudy-puts-per-sec` | 0.105 [0.105, 0.106] | 0.107 [0.105, 0.109] | SLOWER |
| `sng_sm_hashjudy_big` | 4 | `hashjudy-gets-per-sec` | 0.485 [0.484, 0.487] | 0.486 [0.484, 0.488] | SLOWER |
| `sng_sm_hashjudy_big` | 4 | `hashjudy-puts-per-sec` | 0.126 [0.125, 0.126] | 0.127 [0.126, 0.127] | SLOWER |
| `sng_sm_hashjudy_default` | 1 | `hashjudy-gets-per-sec` | 1.327 [1.323, 1.331] | 1.328 [1.325, 1.332] | FASTER |
| `sng_sm_hashjudy_default` | 1 | `hashjudy-puts-per-sec` | 0.153 [0.153, 0.154] | 0.154 [0.153, 0.155] | SLOWER |
| `sng_sm_hashjudy_default` | 4 | `hashjudy-gets-per-sec` | 1.308 [1.301, 1.314] | 1.316 [1.310, 1.322] | FASTER |
| `sng_sm_hashjudy_default` | 4 | `hashjudy-puts-per-sec` | 0.146 [0.145, 0.147] | 0.146 [0.145, 0.147] | SLOWER |
| `sng_sm_judy_big` | 1 | `judy-gets-per-sec` | 0.673 [0.669, 0.676] | 0.674 [0.672, 0.676] | SLOWER |
| `sng_sm_judy_big` | 1 | `judy-puts-per-sec` | 1.089 [1.087, 1.091] | 1.090 [1.087, 1.092] | FASTER |
| `sng_sm_judy_big` | 8 | `judy-gets-per-sec` | 0.833 [0.832, 0.834] | 0.832 [0.831, 0.833] | SLOWER |
| `sng_sm_judy_big` | 8 | `judy-puts-per-sec` | 1.164 [1.159, 1.167] | 1.163 [1.158, 1.165] | FASTER |
| `sng_sm_judy_default` | 1 | `judy-gets-per-sec` | 0.630 [0.627, 0.633] | 0.626 [0.624, 0.629] | SLOWER |
| `sng_sm_judy_default` | 1 | `judy-puts-per-sec` | 0.957 [0.956, 0.958] | 0.958 [0.956, 0.959] | SLOWER |
| `sng_sm_judy_default` | 8 | `judy-gets-per-sec` | 0.628 [0.627, 0.630] | 0.629 [0.628, 0.631] | SLOWER |
| `sng_sm_judy_default` | 8 | `judy-puts-per-sec` | 0.957 [0.956, 0.958] | 0.956 [0.954, 0.957] | SLOWER |

### Against the predictions (METHODOLOGY.md §9)

- **R2 holds.** H is NOT MET, through the same 26 cell-metrics as at `82eb56af`:
  - every delete;
  - every 4m insert;
  - every `sparsematrix judy` get;
  - every `hashjudy` put, and the `hashjudy` big gets.
- **R1 fails in 5 of 52 primary cell-metrics.**
  - The four `sng_judy_1k` finds were FASTER at `82eb56af` (1.081–1.087×) and are SLOWER here (0.955–0.966× in both runs).
  - `sng_judy_4m` find at W = 2 moves from INCONCLUSIVE to EQUIVALENT.
  - The other 47 verdicts replicate.
- **The stock arm reproduced; the Expanse arm's finds did not.** Arm S's rates match the `82eb56af` evaluation to within 0.1%: `sng_judy_1k` W = 1 find was 100.64 M ops/s then and 100.60 M now. Arm E's finds fell: 109.14 M → 96.97 M at 1k, 96.35 M → 88.63 M at 256k, W = 1. Its inserts (34.56 M → 34.76 M) and deletes (29.34 M → 29.14 M) did not move. These are per-arm means over both runs, from the raw rows of both evaluations.
- **Cause: a hypothesis, unmeasured.** The engine source is unchanged on this path (METHODOLOGY.md §2).
  - `JudyLGet` is byte-for-byte the same 107 instructions in both libraries, with operands normalised (`objdump -d`).
  - It starts 0x60 bytes later in the current library, at offset 48 of a 64-byte line instead of 16. #1673 added code ahead of it in `compat.rs`.
  - Code placement is the leading hypothesis. No PMU counter was taken, so it is not established.
  - An A/B of the two library files on the 1k find cell would test it. That is a diagnostic outside this pre-registration.

### stress-ng bogo-ops/s (secondary)

| workload | W | metric | run 1 E/S [BCa 95%] | run 2 E/S [BCa 95%] | verdict |
|---|---|---|---|---|---|
| `sng_judy_1k` | 1 | `bogo-ops-per-second-real-time` | 0.949 [0.945, 0.951] | 0.951 [0.949, 0.953] | |
| `sng_judy_1k` | 2 | `bogo-ops-per-second-real-time` | 0.951 [0.947, 0.956] | 0.954 [0.948, 0.960] | |
| `sng_judy_1k` | 4 | `bogo-ops-per-second-real-time` | 0.946 [0.941, 0.951] | 0.950 [0.944, 0.955] | |
| `sng_judy_1k` | 8 | `bogo-ops-per-second-real-time` | 0.952 [0.947, 0.954] | 0.952 [0.951, 0.953] | |
| `sng_judy_256k` | 1 | `bogo-ops-per-second-real-time` | 0.927 [0.910, 0.950] | 0.920 [0.901, 0.939] | |
| `sng_judy_256k` | 2 | `bogo-ops-per-second-real-time` | 0.935 [0.923, 0.952] | 0.939 [0.925, 0.952] | |
| `sng_judy_256k` | 4 | `bogo-ops-per-second-real-time` | 0.944 [0.935, 0.951] | 0.947 [0.941, 0.953] | |
| `sng_judy_256k` | 8 | `bogo-ops-per-second-real-time` | 0.951 [0.949, 0.953] | 0.950 [0.948, 0.952] | |
| `sng_judy_4m` | 1 | `bogo-ops-per-second-real-time` | 0.768 [0.762, 0.775] | 0.771 [0.764, 0.778] | |
| `sng_judy_4m` | 2 | `bogo-ops-per-second-real-time` | 0.768 [0.763, 0.772] | 0.773 [0.767, 0.780] | |
| `sng_judy_4m` | 4 | `bogo-ops-per-second-real-time` | 0.767 [0.762, 0.772] | 0.770 [0.766, 0.775] | |
| `sng_judy_4m` | 8 | `bogo-ops-per-second-real-time` | 0.777 [0.775, 0.778] | 0.777 [0.777, 0.778] | |
| `sng_sm_hashjudy_big` | 1 | `bogo-ops-per-second-real-time` | 0.175 [0.172, 0.177] | 0.180 [0.176, 0.186] | |
| `sng_sm_hashjudy_big` | 4 | `bogo-ops-per-second-real-time` | 0.238 [0.237, 0.239] | 0.239 [0.238, 0.240] | |
| `sng_sm_hashjudy_default` | 1 | `bogo-ops-per-second-real-time` | 0.282 [0.281, 0.283] | 0.283 [0.282, 0.283] | |
| `sng_sm_hashjudy_default` | 4 | `bogo-ops-per-second-real-time` | 0.267 [0.266, 0.269] | 0.268 [0.267, 0.270] | |
| `sng_sm_judy_big` | 1 | `bogo-ops-per-second-real-time` | 0.816 [0.813, 0.818] | 0.817 [0.815, 0.818] | |
| `sng_sm_judy_big` | 8 | `bogo-ops-per-second-real-time` | 0.957 [0.955, 0.958] | 0.956 [0.954, 0.957] | |
| `sng_sm_judy_default` | 1 | `bogo-ops-per-second-real-time` | 0.756 [0.754, 0.758] | 0.754 [0.752, 0.756] | |
| `sng_sm_judy_default` | 8 | `bogo-ops-per-second-real-time` | 0.755 [0.754, 0.757] | 0.755 [0.754, 0.757] | |

### Resident memory at W = 1 (stress-ng `max-rss`, E/S)

| workload | W | metric | run 1 E/S [BCa 95%] | run 2 E/S [BCa 95%] | verdict |
|---|---|---|---|---|---|
| `sng_judy_1k` | 1 | `max-rss` | 1.428 [1.371, 1.454] | 1.388 [1.339, 1.421] | |
| `sng_judy_256k` | 1 | `max-rss` | 1.046 [1.039, 1.055] | 1.045 [1.036, 1.051] | |
| `sng_judy_4m` | 1 | `max-rss` | 1.007 [1.006, 1.008] | 1.007 [1.006, 1.008] | |
| `sng_sm_hashjudy_big` | 1 | `max-rss` | 77.185 [77.013, 77.359] | 77.341 [77.112, 77.594] | |
| `sng_sm_hashjudy_default` | 1 | `max-rss` | 49.425 [49.294, 49.559] | 49.303 [49.149, 49.510] | |
| `sng_sm_judy_big` | 1 | `max-rss` | 1.218 [1.211, 1.226] | 1.221 [1.214, 1.229] | |
| `sng_sm_judy_default` | 1 | `max-rss` | 1.248 [1.225, 1.268] | 1.247 [1.205, 1.282] | |

`hashjudy`'s 49× and 77× come from the small-array floor that #1669 documents, multiplied by stress-ng's own leak (`hashjudy_destroy` calls no `JLFA`). At 1k keys, RSS is dominated by the stress-ng process itself.

### Timer cross-check

The outside wall-clock ratio agrees with the inverse of stress-ng's bogo-ops/s ratio to within 5% in every cell of both runs.

## Reproduce

On a host with Docker and the workspace toolchain:

```bash
docs/benchmarks/stressng_comparison/scripts/build.sh
```

```bash
docs/benchmarks/stressng_comparison/run.sh 1 ref
```

```bash
docs/benchmarks/stressng_comparison/run.sh 2 ref
```

```bash
python3 docs/benchmarks/stressng_comparison/scripts/analyze.py ref /var/tmp/stressng-eval/results/ref_run1 /var/tmp/stressng-eval/results/ref_run2
```

`build.sh` is untimed. Each `run.sh` holds the host bench lock for its whole run and refuses to start on the conditions in METHODOLOGY.md §7. `analyze.py` writes `results/baseline_<host>_run{1,2}.json` and prints the verdict table.
