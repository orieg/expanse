# stress-ng × Judy: Expanse as a runtime libJudy replacement

stress-ng's `judy` and `sparsematrix` stressors run one binary against stock libJudy and against `libexpanse`. The library is swapped through `libJudy.so.1` and nothing else changes. This suite is item 4 of [#1666](https://github.com/orieg/expanse/issues/1666): the re-measure after the packaging fix (#1667), the Callgrind arms (#1668) and the documented small-array floor (#1669).

- **Pre-registration:** [`METHODOLOGY.md`](METHODOLOGY.md). It was committed before the first timed process of this measurement and is never edited to match outcomes.
- **Status:** both hosts have been measured (below), the second with the governor change and the paused CI runner that METHODOLOGY.md §3 makes conditional on approval.
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
  - An A/B of the two library files on the 1k find cell tests part of it, as a diagnostic outside this pre-registration (below).

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

## Results: second host

(measured: second host AMD Ryzen 9 9955HX (Zen 5, 16 cores, 32 threads), CPUs `0-7` pinned, governor `performance` on every CPU (`amd-pstate-epp`, EPP `performance`), Linux 6.8, harness `668bf4a58`, the same stress-ng binary, stock libJudy and libexpanse file as the reference host (sha256 `490c0595…`, `76f121a8…`, `da5814a7…`); artifacts `results/baseline_zen5_run1.json`, `results/baseline_zen5_run2.json`; workloads: each row's `workload_id`)

This is the second host's result, kept apart from the reference host's. No figure below is pooled with the tables above. The harness's timed path is the one the reference host ran: `3b17913df` moved the library proof into a function and changed nothing it runs.

### Host hygiene

- **Dispatch:** no bench workflow was in flight and the host's CI runner had no job. Both runs held the host bench lock once for the pair, from 16:04:19 to 16:17:50 UTC.
- **Host changes for the timed rounds only (METHODOLOGY.md §3):** the governor went from `powersave` to `performance` on all 32 CPUs, and the host's other CI runner container was stopped. A trap restored both on exit, and both were read back equal to the state recorded before the change: the governor and EPP of every CPU, and the container running with restart policy `unless-stopped`.
- **Load:** 800 processes ran, with 0 non-zero exits, 0 stress-ng failures and none discarded.
  - Foreign load median 0.01 core-equivalents in both runs; max 0.30 in run 1 and 0.17 in run 2.
  - Max load1 was 4.77 and 5.59, against a void threshold of 16.
  - The host's periodic deployment check and statistics collector were not paused. No process crossed the discard limit.
- **Library proof:** arm S mapped `libJudy.so.1.0.3`; arm E mapped `libexpanse.so.1.0.0`.

### Primary metrics (E/S, higher means Expanse is faster)

| workload | W | metric | run 1 E/S [BCa 95%] | run 2 E/S [BCa 95%] | verdict |
|---|---|---|---|---|---|
| `sng_judy_1k` | 1 | `judy-delete-operations-per-sec` | 0.799 [0.796, 0.802] | 0.801 [0.800, 0.803] | SLOWER |
| `sng_judy_1k` | 1 | `judy-find-operations-per-sec` | 1.140 [1.123, 1.151] | 1.150 [1.145, 1.153] | FASTER |
| `sng_judy_1k` | 1 | `judy-insert-operations-per-sec` | 1.193 [1.186, 1.213] | 1.185 [1.180, 1.188] | FASTER |
| `sng_judy_1k` | 2 | `judy-delete-operations-per-sec` | 0.803 [0.801, 0.805] | 0.801 [0.798, 0.804] | SLOWER |
| `sng_judy_1k` | 2 | `judy-find-operations-per-sec` | 1.149 [1.141, 1.156] | 1.145 [1.134, 1.150] | FASTER |
| `sng_judy_1k` | 2 | `judy-insert-operations-per-sec` | 1.188 [1.184, 1.192] | 1.182 [1.177, 1.188] | FASTER |
| `sng_judy_1k` | 4 | `judy-delete-operations-per-sec` | 0.800 [0.798, 0.801] | 0.799 [0.796, 0.801] | SLOWER |
| `sng_judy_1k` | 4 | `judy-find-operations-per-sec` | 1.143 [1.139, 1.150] | 1.142 [1.121, 1.150] | FASTER |
| `sng_judy_1k` | 4 | `judy-insert-operations-per-sec` | 1.192 [1.178, 1.218] | 1.176 [1.169, 1.180] | FASTER |
| `sng_judy_1k` | 8 | `judy-delete-operations-per-sec` | 0.800 [0.798, 0.802] | 0.801 [0.796, 0.804] | SLOWER |
| `sng_judy_1k` | 8 | `judy-find-operations-per-sec` | 1.149 [1.140, 1.153] | 1.142 [1.133, 1.147] | FASTER |
| `sng_judy_1k` | 8 | `judy-insert-operations-per-sec` | 1.193 [1.186, 1.206] | 1.184 [1.179, 1.190] | FASTER |
| `sng_judy_256k` | 1 | `judy-delete-operations-per-sec` | 0.680 [0.676, 0.683] | 0.680 [0.678, 0.683] | SLOWER |
| `sng_judy_256k` | 1 | `judy-find-operations-per-sec` | 1.325 [1.320, 1.332] | 1.326 [1.321, 1.332] | FASTER |
| `sng_judy_256k` | 1 | `judy-insert-operations-per-sec` | 1.156 [1.146, 1.160] | 1.157 [1.155, 1.161] | FASTER |
| `sng_judy_256k` | 2 | `judy-delete-operations-per-sec` | 0.680 [0.677, 0.683] | 0.680 [0.677, 0.683] | SLOWER |
| `sng_judy_256k` | 2 | `judy-find-operations-per-sec` | 1.324 [1.319, 1.329] | 1.320 [1.301, 1.326] | FASTER |
| `sng_judy_256k` | 2 | `judy-insert-operations-per-sec` | 1.149 [1.139, 1.153] | 1.154 [1.152, 1.157] | FASTER |
| `sng_judy_256k` | 4 | `judy-delete-operations-per-sec` | 0.680 [0.678, 0.682] | 0.683 [0.681, 0.686] | SLOWER |
| `sng_judy_256k` | 4 | `judy-find-operations-per-sec` | 1.323 [1.289, 1.337] | 1.338 [1.331, 1.344] | FASTER |
| `sng_judy_256k` | 4 | `judy-insert-operations-per-sec` | 1.157 [1.147, 1.162] | 1.167 [1.163, 1.169] | FASTER |
| `sng_judy_256k` | 8 | `judy-delete-operations-per-sec` | 0.748 [0.745, 0.753] | 0.746 [0.743, 0.748] | SLOWER |
| `sng_judy_256k` | 8 | `judy-find-operations-per-sec` | 1.537 [1.514, 1.556] | 1.522 [1.499, 1.544] | FASTER |
| `sng_judy_256k` | 8 | `judy-insert-operations-per-sec` | 1.171 [1.158, 1.177] | 1.171 [1.168, 1.175] | FASTER |
| `sng_judy_4m` | 1 | `judy-delete-operations-per-sec` | 0.703 [0.699, 0.707] | 0.700 [0.697, 0.703] | SLOWER |
| `sng_judy_4m` | 1 | `judy-find-operations-per-sec` | 1.697 [1.679, 1.717] | 1.672 [1.650, 1.694] | FASTER |
| `sng_judy_4m` | 1 | `judy-insert-operations-per-sec` | 0.868 [0.861, 0.872] | 0.867 [0.861, 0.872] | SLOWER |
| `sng_judy_4m` | 2 | `judy-delete-operations-per-sec` | 0.707 [0.704, 0.709] | 0.707 [0.704, 0.712] | SLOWER |
| `sng_judy_4m` | 2 | `judy-find-operations-per-sec` | 1.641 [1.626, 1.652] | 1.638 [1.628, 1.649] | FASTER |
| `sng_judy_4m` | 2 | `judy-insert-operations-per-sec` | 0.869 [0.866, 0.872] | 0.869 [0.866, 0.871] | SLOWER |
| `sng_judy_4m` | 4 | `judy-delete-operations-per-sec` | 0.720 [0.718, 0.723] | 0.720 [0.718, 0.723] | SLOWER |
| `sng_judy_4m` | 4 | `judy-find-operations-per-sec` | 1.596 [1.580, 1.606] | 1.604 [1.592, 1.615] | FASTER |
| `sng_judy_4m` | 4 | `judy-insert-operations-per-sec` | 0.875 [0.873, 0.878] | 0.873 [0.869, 0.876] | SLOWER |
| `sng_judy_4m` | 8 | `judy-delete-operations-per-sec` | 0.727 [0.726, 0.728] | 0.732 [0.727, 0.740] | SLOWER |
| `sng_judy_4m` | 8 | `judy-find-operations-per-sec` | 1.596 [1.585, 1.607] | 1.597 [1.582, 1.614] | FASTER |
| `sng_judy_4m` | 8 | `judy-insert-operations-per-sec` | 0.875 [0.871, 0.878] | 0.874 [0.869, 0.879] | SLOWER |
| `sng_sm_hashjudy_big` | 1 | `hashjudy-gets-per-sec` | 0.539 [0.531, 0.546] | 0.537 [0.531, 0.543] | SLOWER |
| `sng_sm_hashjudy_big` | 1 | `hashjudy-puts-per-sec` | 0.107 [0.105, 0.109] | 0.105 [0.104, 0.107] | SLOWER |
| `sng_sm_hashjudy_big` | 4 | `hashjudy-gets-per-sec` | 0.616 [0.613, 0.620] | 0.620 [0.617, 0.623] | SLOWER |
| `sng_sm_hashjudy_big` | 4 | `hashjudy-puts-per-sec` | 0.146 [0.144, 0.148] | 0.148 [0.146, 0.150] | SLOWER |
| `sng_sm_hashjudy_default` | 1 | `hashjudy-gets-per-sec` | 1.333 [1.323, 1.342] | 1.340 [1.331, 1.354] | FASTER |
| `sng_sm_hashjudy_default` | 1 | `hashjudy-puts-per-sec` | 0.179 [0.178, 0.181] | 0.179 [0.178, 0.180] | SLOWER |
| `sng_sm_hashjudy_default` | 4 | `hashjudy-gets-per-sec` | 1.336 [1.331, 1.340] | 1.336 [1.331, 1.341] | FASTER |
| `sng_sm_hashjudy_default` | 4 | `hashjudy-puts-per-sec` | 0.163 [0.162, 0.164] | 0.163 [0.162, 0.164] | SLOWER |
| `sng_sm_judy_big` | 1 | `judy-gets-per-sec` | 0.689 [0.685, 0.694] | 0.688 [0.684, 0.692] | SLOWER |
| `sng_sm_judy_big` | 1 | `judy-puts-per-sec` | 1.081 [1.073, 1.089] | 1.080 [1.070, 1.089] | FASTER |
| `sng_sm_judy_big` | 8 | `judy-gets-per-sec` | 0.917 [0.915, 0.920] | 0.920 [0.915, 0.928] | SLOWER |
| `sng_sm_judy_big` | 8 | `judy-puts-per-sec` | 1.131 [1.125, 1.136] | 1.125 [1.110, 1.133] | FASTER |
| `sng_sm_judy_default` | 1 | `judy-gets-per-sec` | 0.632 [0.629, 0.635] | 0.638 [0.636, 0.640] | SLOWER |
| `sng_sm_judy_default` | 1 | `judy-puts-per-sec` | 1.053 [1.045, 1.057] | 1.055 [1.050, 1.058] | FASTER |
| `sng_sm_judy_default` | 8 | `judy-gets-per-sec` | 0.635 [0.634, 0.637] | 0.635 [0.632, 0.637] | SLOWER |
| `sng_sm_judy_default` | 8 | `judy-puts-per-sec` | 1.053 [1.046, 1.057] | 1.056 [1.054, 1.058] | FASTER |

### Against the predictions (METHODOLOGY.md §9)

- **Z1 holds: 47 of 47.**
  - The set is every primary cell-metric whose `82eb56af` reference-host E/S interval lay entirely outside [0.95, 1.05] in both runs. That is all 52 except `sng_judy_4m` find at W = 1, 2, 4 and `sng_sm_judy_default` puts at W = 1, 8.
  - Each of the 47 keeps its `82eb56af` direction here.
  - The `82eb56af` intervals were recomputed from that evaluation's raw rows with this suite's estimator (two-sample BCa, 2,000 resamples, seed 42). Its run-1 intervals equal the ones the earlier evaluation reported. Those rows are not committed (METHODOLOGY.md §1), so the set is stated here in full.
- **Z2 holds.** H is NOT MET, through the same 26 cell-metrics as on the reference host:
  - every delete;
  - every 4m insert;
  - every `sparsematrix judy` get;
  - every `hashjudy` put, and the `hashjudy` big gets.
- **Outside the Z1 set (not predicted):** the 4m finds at W = 1, 2, 4 are FASTER here (1.596–1.697×). `sng_sm_judy_default` puts are FASTER here (1.053–1.056×), where the reference host has them SLOWER (0.956–0.958×).
- **The 1k finds go the other way from the reference host.** The same file gives E/S 1.140–1.150× here (FASTER) and 0.955–0.966× on the reference host (SLOWER).
  - Per-arm means at W = 1 over both runs here, in M ops/s: S 123.81, E 141.71.
  - Why one file's 1k find is slower than stock on one host and faster on the other was not measured. No PMU counter was taken on either host.

### stress-ng bogo-ops/s (secondary)

| workload | W | metric | run 1 E/S [BCa 95%] | run 2 E/S [BCa 95%] | verdict |
|---|---|---|---|---|---|
| `sng_judy_1k` | 1 | `bogo-ops-per-second-real-time` | 0.991 [0.987, 0.999] | 0.990 [0.988, 0.992] | |
| `sng_judy_1k` | 2 | `bogo-ops-per-second-real-time` | 0.992 [0.989, 0.995] | 0.989 [0.986, 0.992] | |
| `sng_judy_1k` | 4 | `bogo-ops-per-second-real-time` | 0.992 [0.986, 1.003] | 0.985 [0.981, 0.987] | |
| `sng_judy_1k` | 8 | `bogo-ops-per-second-real-time` | 0.992 [0.989, 0.997] | 0.988 [0.984, 0.992] | |
| `sng_judy_256k` | 1 | `bogo-ops-per-second-real-time` | 0.899 [0.895, 0.901] | 0.899 [0.897, 0.902] | |
| `sng_judy_256k` | 2 | `bogo-ops-per-second-real-time` | 0.896 [0.894, 0.899] | 0.898 [0.894, 0.901] | |
| `sng_judy_256k` | 4 | `bogo-ops-per-second-real-time` | 0.899 [0.893, 0.902] | 0.905 [0.902, 0.907] | |
| `sng_judy_256k` | 8 | `bogo-ops-per-second-real-time` | 0.969 [0.964, 0.974] | 0.966 [0.962, 0.970] | |
| `sng_judy_4m` | 1 | `bogo-ops-per-second-real-time` | 0.876 [0.872, 0.880] | 0.872 [0.868, 0.877] | |
| `sng_judy_4m` | 2 | `bogo-ops-per-second-real-time` | 0.877 [0.874, 0.879] | 0.877 [0.875, 0.880] | |
| `sng_judy_4m` | 4 | `bogo-ops-per-second-real-time` | 0.883 [0.881, 0.886] | 0.884 [0.881, 0.886] | |
| `sng_judy_4m` | 8 | `bogo-ops-per-second-real-time` | 0.888 [0.886, 0.890] | 0.891 [0.886, 0.897] | |
| `sng_sm_hashjudy_big` | 1 | `bogo-ops-per-second-real-time` | 0.190 [0.187, 0.192] | 0.188 [0.187, 0.189] | |
| `sng_sm_hashjudy_big` | 4 | `bogo-ops-per-second-real-time` | 0.285 [0.283, 0.287] | 0.286 [0.284, 0.288] | |
| `sng_sm_hashjudy_default` | 1 | `bogo-ops-per-second-real-time` | 0.332 [0.329, 0.334] | 0.331 [0.329, 0.333] | |
| `sng_sm_hashjudy_default` | 4 | `bogo-ops-per-second-real-time` | 0.305 [0.304, 0.306] | 0.305 [0.303, 0.306] | |
| `sng_sm_judy_big` | 1 | `bogo-ops-per-second-real-time` | 0.815 [0.807, 0.820] | 0.816 [0.811, 0.821] | |
| `sng_sm_judy_big` | 8 | `bogo-ops-per-second-real-time` | 0.992 [0.989, 0.994] | 0.990 [0.985, 0.994] | |
| `sng_sm_judy_default` | 1 | `bogo-ops-per-second-real-time` | 0.782 [0.778, 0.786] | 0.786 [0.784, 0.788] | |
| `sng_sm_judy_default` | 8 | `bogo-ops-per-second-real-time` | 0.785 [0.783, 0.787] | 0.786 [0.783, 0.787] | |

### Resident memory at W = 1 (stress-ng `max-rss`, E/S)

| workload | W | metric | run 1 E/S [BCa 95%] | run 2 E/S [BCa 95%] | verdict |
|---|---|---|---|---|---|
| `sng_judy_1k` | 1 | `max-rss` | 1.383 [1.336, 1.431] | 1.373 [1.341, 1.423] | |
| `sng_judy_256k` | 1 | `max-rss` | 1.000 [0.979, 1.024] | 0.984 [0.968, 1.001] | |
| `sng_judy_4m` | 1 | `max-rss` | 1.002 [1.000, 1.004] | 1.002 [1.000, 1.004] | |
| `sng_sm_hashjudy_big` | 1 | `max-rss` | 76.864 [76.631, 77.020] | 76.642 [76.410, 76.884] | |
| `sng_sm_hashjudy_default` | 1 | `max-rss` | 49.792 [49.498, 50.147] | 49.494 [49.373, 49.672] | |
| `sng_sm_judy_big` | 1 | `max-rss` | 1.188 [1.181, 1.196] | 1.188 [1.182, 1.195] | |
| `sng_sm_judy_default` | 1 | `max-rss` | 1.400 [1.369, 1.452] | 1.377 [1.347, 1.424] | |

### Timer cross-check

The outside wall-clock ratio agrees with the inverse of stress-ng's bogo-ops/s ratio to within 5% in every cell of both runs except `sng_sm_hashjudy_default` at W = 4. There, wall clock gives 3.470 against bogo-ops 3.279 in run 1, and 3.484 against 3.282 in run 2, which is 5.8% and 6.2% apart. The cause was not measured.

## Diagnostic: the two library files on the 1k find cell

**A diagnostic, not a pre-registered result.** METHODOLOGY.md does not cover it, and nothing below changes a verdict above.

**Question.** Is the fall of arm E's `sng_judy_1k` find rate a property of the two library files, or of the host between the two evaluations? The A/B can separate those two. It cannot separate code placement from any other difference between the files, and it takes no PMU counter.

**Design, fixed before the run** (`scripts/diag_lib_ab.py`, `run.sh <run> ref lib_ab`):
- Three arms, the same stress-ng binary and judy stressor as above, differing only in `LD_LIBRARY_PATH`:
  - O: libexpanse built at `82eb56af`, the file of the earlier evaluation (sha256 `cce4ba9d…`);
  - N: the file of the pre-registered runs (sha256 `da5814a7…`);
  - S: stock libJudy, as above.
- `sng_judy_1k` at W = 1, 2, 4, 8, one process per arm per W per round.
- 12 rounds per run, cycling through the six orders of the three arms, so each arm runs first, second and third equally often. Two independent runs under the host bench lock and the same core pin.
- A load snapshot before every process and a window after it, so every pair of adjacent arms has a snapshot between them. A process with foreign load above 0.5 core-equivalents is discarded, and a run is void under METHODOLOGY.md §7.
- `scripts/diag_lib_ab_analyze.py` writes `results/diagnostic_lib_ab_ref_run{1,2}.json`: two-sample BCa 95% intervals on O/N, N/S and O/S for find, insert, delete, bogo-ops/s and wall clock.

**Reading rule for O/N on the find rate, per W, over both runs:**
- O FASTER (lower bound > 1 in both runs): the fall is in the library files.
- SAME (both intervals inside [0.98, 1.02]): the files run the find alike on this host now, so the fall came from outside them.
- Anything else is reported as measured, without a reading.

**Expected if the files carry the whole fall:** O/N ≈ 1.13 at W = 1 (109.14 M / 96.97 M ops/s, the per-arm means of the two evaluations), O/S ≈ 1.08 and N/S ≈ 0.96. O/N on insert and delete is the control, expected inside [0.98, 1.02] from the same per-arm means. (projected)

### Diagnostic result

(measured: reference host Intel Core i9-12900F, pin `0,2,4,6,8,10,12,14`, governor `powersave`, harness `3b17913df`; artifacts `results/diagnostic_lib_ab_ref_run{1,2}.json`) (workload: sng_judy_1k)

**Hygiene.** Run 1 02:24:32–02:25:44 UTC and run 2 02:25:44–02:27:40, each under the bench lock, after the 02:17 host rsync had exited. 144 processes per run and none discarded; the worst foreign load over any process was 0.06 core-equivalents. Max load1 was 3.19 and 2.72 against a void limit of 12. The library proof mapped each arm's own file: O `expcompat-82eb56af/lib/libexpanse.so.1.0.0`, N `expcompat/lib/libexpanse.so.1.0.0`, S stock `libJudy.so.1.0.3`.

**O/N on the find rate: O FASTER at every W in both runs.** The fall is in the library files.

| W | O/N run 1 [BCa 95%] | O/N run 2 [BCa 95%] | O/S run 1 | O/S run 2 | N/S run 1 | N/S run 2 |
|---|---|---|---|---|---|---|
| 1 | 1.114 [1.107, 1.122] | 1.130 [1.121, 1.139] | 1.080 [1.077, 1.085] | 1.082 [1.076, 1.087] | 0.970 [0.964, 0.975] | 0.958 [0.951, 0.963] |
| 2 | 1.131 [1.122, 1.139] | 1.130 [1.122, 1.136] | 1.090 [1.083, 1.097] | 1.083 [1.078, 1.089] | 0.964 [0.958, 0.970] | 0.959 [0.953, 0.965] |
| 4 | 1.123 [1.116, 1.129] | 1.130 [1.122, 1.137] | 1.083 [1.077, 1.089] | 1.086 [1.081, 1.090] | 0.964 [0.960, 0.970] | 0.961 [0.955, 0.967] |
| 8 | 1.138 [1.135, 1.141] | 1.138 [1.136, 1.141] | 1.087 [1.084, 1.088] | 1.086 [1.084, 1.087] | 0.955 [0.953, 0.957] | 0.954 [0.951, 0.956] |

- **Both evaluations' find rates are recovered in one session.** Per-arm means at W = 1 over both runs, in M ops/s: O 108.94, N 97.10, S 100.74. The two evaluations' arm E means were 109.14 and 96.97, and their arm S means 100.64 and 100.60. O/S matches the `82eb56af` evaluation's 1.081–1.087×, and N/S the pre-registered runs' 0.955–0.966×.
- **The controls met the expectation in their point estimates, not in every interval.**
  - O/N on insert is 0.991–1.014 across W and runs. Every interval lies inside [0.98, 1.02] except W = 8 run 2, [0.998, 1.079].
  - O/N on delete is 0.999–1.014. Three intervals reach past 1.02: W = 1 run 2 [0.969, 1.008] on the low side, W = 4 run 2 [1.007, 1.028] and W = 8 run 1 [1.005, 1.038] on the high side.
  - Outside wall clock O/N is 0.972–0.987, the O arm's processes finishing sooner. Wall clock covers the whole process, of which the find phase is one part; how large a part was not measured.
- **What this does and does not establish.**
  - Established: on this host today, the 82eb56af file runs the 1k find faster than the 6ec29a656 file, by about the amount the two evaluations differed. Host drift between the evaluations does not account for the shift.
  - Not established: that code placement is the cause. The files differ in everything between the two commits, not only in where `JudyLGet` starts. No PMU counter was taken, and no build controls the layout.

## Diagnostic: front-end counters on the 1k find cell

**A diagnostic, not a pre-registered result** (#1684). METHODOLOGY.md does not cover it, and nothing below changes a verdict above.

**Question.** The library A/B above put the 1k find shift in the library files. `JudyLGet` retires the same instructions in both files and sits 0x60 bytes later in N, so the leading hypothesis is code placement: the same instructions delivered less well by the front end. Do the counters show N's extra cycles as front-end cycles, and which front-end resource?

**Design, fixed before the run** (`scripts/diag_find_counters.py`, `run.sh <run> ref counters`):
- The three arms O, N and S of the library A/B, `sng_judy_1k` at W = 1 only. Its work is fixed (`--judy-ops 5000`), so whole-process counts compare directly across arms.
- Every process runs under `perf stat` with user-mode events on the `cpu_core` PMU, where the pin places the run. Fixed counters `cycles` and `instructions` in every process, plus one event group per process:
  - `uops`: `idq.dsb_uops`, `idq.mite_uops`, `idq.ms_uops`, `lsd.uops` (which unit delivered the uops);
  - `fe`: `idq_uops_not_delivered.core`, `frontend_retired.dsb_miss`, `dsb2mite_switches.penalty_cycles`, `icache_data.stalls`;
  - `misc`: `icache_tag.stalls`, `itlb_misses.walk_completed`, `br_misp_retired.all_branches`, `baclears.any`;
  - `br`: the fixed `slots` counter, `br_inst_retired.all_branches`, `uops_issued.any`, `decode.lcp`.
- No group holds more than four general-purpose events, because a larger set can read a real event as zero on this PMU (#1177). A preflight counts each general-purpose event alone and in its group on arm N, and stops if one that counts alone reads zero in its group, or if any row in any process ran below 100% of the time.
- 12 rounds per run. Each round runs the four groups in a shuffled order and, within a group, the three arms in one of their six orders, cycling so each arm runs first, second and third equally often. Two independent runs under the host bench lock and the core pin of the comparison. Load windows, the discard rule and the void rule are the library A/B's.
- After the rounds, one `perf record` of user cycles per arm gives each symbol's share, to say whether the cycle difference sits in `JudyLGet`. That is one sample per arm per run, and is reported as such.
- `scripts/diag_find_counters_analyze.py` writes `results/diagnostic_find_counters_ref_run{1,2}.json`: two-sample BCa 95% intervals on O/N, N/S and O/S for every counter and for stress-ng's find rate. `cycles`, `instructions` and the find rate pool all processes of an arm, and every other counter uses its own group's processes.

**Reading rule, O/N over both runs:**
- R0, same work: `instructions` inside [0.995, 1.005] in both runs.
- R1, cost reproduced: the `cycles` upper bound below 1 in both runs.
- If R0 and R1 hold, f = (FE_N − FE_O) / (w · (cycles_N − cycles_O)), from the arms' means. FE is `idq_uops_not_delivered.core`, the issue slots the front end left empty, and w = slots_N / cycles_N is the measured issue width. f is the share of N's extra slots that the front end left empty.
  - FRONT-END: f ≥ 0.5 in both runs.
  - NOT FRONT-END: f ≤ 0.2 in both runs, and the other counters are read for the cause.
  - PARTIAL: anything else.
- The remaining counters are reported with their intervals and name which front-end resource moved. They carry no verdict of their own.
- A FRONT-END reading supports the placement hypothesis and does not establish it. Establishing it needs a build that moves only the layout, which is #1684's fix step.

### Counter result

(measured: reference host Intel Core i9-12900F, pin `0,2,4,6,8,10,12,14`, governor `powersave`, perf 6.8.12, harness `aa526046c`; artifacts `results/diagnostic_find_counters_ref_run{1,2}.json`) (workload: sng_judy_1k)

**Hygiene.** Run 1 19:24:03–19:25:30 UTC and run 2 19:25:30–19:26:49, each under the bench lock, after the 19:17 host rsync had exited. A first launch at 19:20 stopped in the preflight on a perf event-syntax error before anything was timed; `aa526046c` fixed the syntax and nothing else. 144 processes per run and none discarded; the worst foreign load over any process was 0.04 core-equivalents, and max load1 was 0.84 and 0.96 against a void limit of 12. The preflight found no event that counts alone and reads zero in its group, and no row in any process ran below 100% of the time. The library proof mapped each arm's own file, as in the library A/B.

**Reading: FRONT-END.** R0 and R1 hold, and f = 0.672 in run 1 and 0.788 in run 2.

| O/N | run 1 [BCa 95%] | run 2 [BCa 95%] |
|---|---|---|
| `instructions` | 1.0000 (bc) | 1.0000 (bc) |
| `cycles` | 0.9849 [0.9816, 0.9863] | 0.9846 [0.9830, 0.9866] |
| find rate | 1.1220 [1.1182, 1.1261] | 1.1235 [1.1195, 1.1276] |
| `idq_uops_not_delivered.core` | 0.9357 [0.9290, 0.9424] | 0.9236 [0.9159, 0.9307] |
| `dsb2mite_switches.penalty_cycles` | 0.9214 [0.9082, 0.9339] | 0.9203 [0.9135, 0.9274] |
| `idq.mite_uops` | 0.9249 [0.9181, 0.9322] | 0.9280 [0.9203, 0.9352] |
| `idq.ms_uops` | 0.9123 [0.9074, 0.9176] | 0.9160 [0.9101, 0.9215] |
| `lsd.uops` | 1.0793 [1.0498, 1.1095] | 1.0896 [1.0578, 1.1362] |
| `decode.lcp` | 0.9615 [0.9480, 0.9762] | 0.9618 [0.9441, 0.9758] |
| `idq.dsb_uops` | 0.9991 [0.9985, 0.9998] | 0.9987 [0.9978, 0.9996] |
| `frontend_retired.dsb_miss` | 1.0424 [1.0291, 1.0581] | 1.0349 [1.0239, 1.0462] |
| `icache_data.stalls` | 1.0833 [1.0667, 1.1148] | 1.0734 [1.0580, 1.0918] |
| `br_misp_retired.all_branches` | 1.0008 [0.9862, 1.0151] | 1.0088 [0.9870, 1.0290] |

- **The arithmetic of f.** Per process, N takes 28.98 M (run 1) and 29.55 M (run 2) more cycles than O. At the measured width of 5.993 and 6.003 slots per cycle, that is 173.7 M and 177.4 M slots. N's front end left 116.7 M and 139.8 M more slots empty than O's.
- **Which front-end resource moved.** In both runs N's uops came less often from the loop stream detector and more often from the legacy decoders (MITE) and the microcode sequencer, and N paid more DSB-to-MITE switch penalty cycles. The DSB delivered the same uops in both files. Instruction-cache data stalls and DSB-miss retirements moved the other way, slightly higher in O. Branch mispredictions are the same in both files, so the cycles are not bad speculation.
- **Where.** In one `perf record` per arm per run, the find path's hot function `get::locate_slot_popcnt` holds 7.79% and 7.91% of O's user cycles and 9.52% and 10.77% of N's. It is the same 1,614 bytes in both files, at `0x22b5f0` in O and `0x22b650` in N. The 0x60-byte shift keeps its 32-byte alignment (16 bytes past a 32-byte boundary in both files) and changes its 64-byte alignment (48 bytes past a 64-byte boundary in O, 16 in N). `JudyLGet` moved by the same 0x60. One sample per arm is not an interval, and the symbol shares are reported as such.
- **What this establishes.** The same instructions cost N more cycles because the front end delivers them less well, with the decoded-uop path changing in a way consistent with the hot code's position within 64-byte lines. That supports the placement hypothesis. It does not establish it: no build has yet moved only the layout, which is #1684's fix step.
- **Against stock (N/S, not the question here).** N retires 0.808× stock's instructions and takes 1.063× its cycles. Its front end leaves 3.26× as many issue slots empty as stock's (`idq_uops_not_delivered.core`, intervals [3.2385, 3.2818] and [3.2471, 3.3005]), while stock mispredicts about four times as many branches (N/S 0.2449 and 0.2428). The whole stress-ng process is measured here, insert and delete included, so this is the library's front-end cost over the cell and not the find path's alone.

## Diagnostic: the #1679 delete fix on the `judy` cells

**A diagnostic, not a pre-registered result** (#1679). METHODOLOGY.md does not cover it, and nothing below changes a verdict above.

**Question.** #1708 keeps a removal-only lower bound on a `BranchU`'s live slots, which drops the Callgrind count of the stress-ng delete phase by 12.8% (1 Ki keys) to 29.7% (256 Ki). Does the wall-clock delete rate follow, and does it reach stock?

**Design, fixed before the builds** (`scripts/build_layout_lib.sh`, `scripts/diag_delete.py`, `run.sh <run> <host> delete`):
- Two library files, built on the reference host by `build.sh`'s libexpanse build with no flag:
  - B: #1708's merge base (`99ef70855`);
  - D: #1708's head (`45f83ac5e`). The design first named `dd12b6e0e`. That head was replaced before any build or run because it added a null guard that #1708 then reverted, and `45f83ac5e` is the code that merges.

  The second host runs the same files, copied and hash-checked.
- Three arms, the same stress-ng binary and judy stressor, differing only in `LD_LIBRARY_PATH`: B, D and S (stock). Every `sng_judy_*` cell (1 Ki, 256 Ki and 4 Mi keys at W = 1, 2, 4, 8), one process per arm per cell per round.
- 12 rounds per run, cycling through the six orders of the three arms. Two independent runs per host under the host bench lock and that host's pin. Load windows, the discard rule and the void rule are the library A/B's.
- The layout diagnostic found the delete rate insensitive to function alignment (N64/N 0.995–1.008). So B against D measures the fix together with whatever else changed between the two commits, which is only #1708's diff.
- `scripts/diag_delete_analyze.py` writes `results/diagnostic_delete_<host>_run{1,2}.json`: two-sample BCa 95% intervals on D/B, D/S and B/S for delete, insert, find, bogo-ops/s and wall clock.

**Reading rule, on the delete rate, per cell and W, over both runs:**
- **#1679's criterion:** D/S lower bound at or above 1.0 in both runs, at every cell and W, on both hosts.
- **D/B** is FASTER, SAME, SLOWER or MIXED, by the layout diagnostic's bands.
- **Insert and find** under D/B are reported as controls, since #1708 does not touch those paths.

**Expected** (projected): D/B on delete above 1 at every size. The size of the move is not predicted from the Callgrind change, since #1668 found the delete gap's wall-clock share unmeasured.

### Delete result (reference host)

(measured: reference host Intel Core i9-12900F, pin `0,2,4,6,8,10,12,14`, governor `powersave`, rustc 1.98.1, harness `9d9628307`; artifacts `results/diagnostic_delete_ref_run{1,2}.json`) (workloads: sng_judy_1k, sng_judy_256k, sng_judy_4m)

**Builds.** Under the bench lock, 01:37:03–01:38:00 UTC.
- B (`99ef70855`) reproduced N's file byte for byte (sha256 `da5814a7…`), so no library code changed between `6ec29a656` and #1708's merge base.
- D (`45f83ac5e`) is `f6bb333e…`.

**Hygiene.** Run 1 01:38:00–01:44:26 UTC and run 2 01:44:26–01:51:52, each under the bench lock. 432 processes per run. Run 1 discarded 8, its worst foreign load 1.09 core-equivalents; run 2 discarded none. Max load1 was 5.03 and 4.27 against a void limit of 12.

**Reading: #1679's criterion is NOT MET on this host.** D/B on the delete rate is FASTER at every cell and W, and D/S is at or above stock only at 256 Ki keys.

| Cell | W | D/B run 1 | D/B run 2 | D/S run 1 | D/S run 2 |
|---|---|---|---|---|---|
| 1k | 1 | 1.120 [1.118, 1.121] | 1.120 [1.116, 1.123] | 0.912 [0.910, 0.913] | 0.911 [0.908, 0.914] |
| 1k | 2 | 1.120 [1.113, 1.127] | 1.111 [1.076, 1.123] | 0.911 [0.906, 0.917] | 0.906 [0.877, 0.916] |
| 1k | 4 | 1.118 [1.112, 1.123] | 1.116 [1.110, 1.124] | 0.952 [0.915, 1.081] | 0.911 [0.906, 0.917] |
| 1k | 8 | 1.121 [1.120, 1.123] | 1.118 [1.101, 1.125] | 0.912 [0.911, 0.914] | 0.906 [0.891, 0.911] |
| 256k | 1 | 1.449 [1.427, 1.469] | 1.416 [1.397, 1.434] | 1.097 [1.080, 1.111] | 1.081 [1.064, 1.098] |
| 256k | 2 | 1.424 [1.402, 1.446] | 1.416 [1.398, 1.435] | 1.076 [1.060, 1.090] | 1.071 [1.057, 1.084] |
| 256k | 4 | 1.438 [1.429, 1.447] | 1.442 [1.434, 1.451] | 1.097 [1.090, 1.103] | 1.100 [1.094, 1.106] |
| 256k | 8 | 1.435 [1.431, 1.440] | 1.433 [1.431, 1.436] | 1.107 [1.105, 1.109] | 1.106 [1.105, 1.108] |
| 4m | 1 | 1.395 [1.387, 1.401] | 1.393 [1.386, 1.397] | 0.874 [0.869, 0.876] | 0.876 [0.871, 0.878] |
| 4m | 2 | 1.392 [1.386, 1.401] | 1.399 [1.391, 1.406] | 0.882 [0.878, 0.886] | 0.887 [0.881, 0.896] |
| 4m | 4 | 1.391 [1.382, 1.400] | 1.406 [1.397, 1.413] | 0.883 [0.878, 0.889] | 0.892 [0.886, 0.897] |
| 4m | 8 | 1.403 [1.394, 1.406] | 1.403 [1.396, 1.406] | 0.898 [0.893, 0.900] | 0.900 [0.897, 0.903] |

- **The fix's effect.** The delete rate rises 1.11–1.12× at 1 Ki keys, 1.42–1.45× at 256 Ki and 1.39–1.41× at 4 Mi. At 256 Ki it takes Expanse from below stock to 1.07–1.11× stock. At 1 Ki (0.91×) and 4 Mi (0.87–0.90×) a gap to stock remains, which the live-slot scan #1679 removed does not account for.
- **Insert, the control #1708 does not touch, holds.** D/B on insert is 0.984–1.006. Every interval lies inside [0.98, 1.02] except four whose lower bounds reach 0.972–0.979: three at 256 Ki and one at 4 Mi (W = 1, run 1).
- **Find moved, and #1708 does not touch find either.**
  - D/B on find is 1.033–1.067 at 1 Ki and 256 Ki, FASTER in both runs. At 4 Mi it is 0.991–1.066 and mostly MIXED.
  - The hot find function `get::locate_slot_popcnt` sits 16 bytes into a 64-byte line in B (`0x22b650`) and at offset 0 in D (`0x22b880`), the offset the layout diagnostic measured fast. That is the placement effect, not the change under test. Its size here is not measured.

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

The second host runs the same commands with the label `zen5`, which pins CPUs `0-7`. Its governor change and the stopped CI runner (METHODOLOGY.md §3) are host operations outside the script:

```bash
docs/benchmarks/stressng_comparison/run.sh 1 zen5
```

```bash
docs/benchmarks/stressng_comparison/run.sh 2 zen5
```

```bash
python3 docs/benchmarks/stressng_comparison/scripts/analyze.py zen5 /var/tmp/stressng-eval/results/zen5_run1 /var/tmp/stressng-eval/results/zen5_run2
```

The diagnostic additionally needs the `82eb56af` library in `$SNG_D/expcompat-82eb56af/lib`, laid out as `build.sh` lays out `expcompat/lib`:

```bash
docs/benchmarks/stressng_comparison/run.sh 1 ref lib_ab
```

```bash
docs/benchmarks/stressng_comparison/run.sh 2 ref lib_ab
```

```bash
python3 docs/benchmarks/stressng_comparison/scripts/diag_lib_ab_analyze.py ref /var/tmp/stressng-eval/results/diag_lib_ab_ref_run1 /var/tmp/stressng-eval/results/diag_lib_ab_ref_run2
```

`build.sh` is untimed. Each `run.sh` holds the host bench lock for its whole run and refuses to start on the conditions in METHODOLOGY.md §7. `analyze.py` writes `results/baseline_<host>_run{1,2}.json` and prints the verdict table.

The counter diagnostic needs the same three libraries and `perf`, and runs with the mode `counters`:

```bash
docs/benchmarks/stressng_comparison/run.sh 1 ref counters
```

```bash
docs/benchmarks/stressng_comparison/run.sh 2 ref counters
```

```bash
python3 docs/benchmarks/stressng_comparison/scripts/diag_find_counters_analyze.py ref /var/tmp/stressng-eval/results/diag_find_counters_ref_run1 /var/tmp/stressng-eval/results/diag_find_counters_ref_run2
```

The delete-fix diagnostic builds its two libraries first, from `git archive` exports of #1708's merge base and head, then runs with the mode `delete` on each host:

```bash
docs/benchmarks/stressng_comparison/scripts/build_layout_lib.sh <export of 99ef70855> /var/tmp/stressng-eval/expcompat-99ef70855/lib
```

```bash
docs/benchmarks/stressng_comparison/scripts/build_layout_lib.sh <export of 45f83ac5e> /var/tmp/stressng-eval/expcompat-45f83ac5e/lib
```

```bash
docs/benchmarks/stressng_comparison/run.sh 1 ref delete
```

```bash
docs/benchmarks/stressng_comparison/run.sh 2 ref delete
```

```bash
python3 docs/benchmarks/stressng_comparison/scripts/diag_delete_analyze.py ref /var/tmp/stressng-eval/results/diag_delete_ref_run1 /var/tmp/stressng-eval/results/diag_delete_ref_run2
```
