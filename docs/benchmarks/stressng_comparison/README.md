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

**Superseded for the `sng_judy_*` rows** by [The #1679 criterion on main](#the-1679-criterion-on-main) (reference host). These rows measure libexpanse at `6ec29a656`, before #1708, #1719 and #1720. They stay as the pre-registered record; the `sparsematrix` rows are not re-measured.

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

## Diagnostic: layout-only builds on the 1k find cell

**A diagnostic, not a pre-registered result** (#1684). METHODOLOGY.md does not cover it, and nothing below changes a verdict above.

**Question.** The counters put N's extra cycles in the front end, and the hot find function sits at a different offset within a 64-byte line in O and N. Is the O/N gap a property of where the code sits, or of what the code is? A build that changes only the layout answers that; the counters alone cannot.

**Design, fixed before the builds** (`scripts/build_layout_lib.sh`, `scripts/diag_layout.py`, `run.sh <run> ref layout`):
- Two new library files, built on the reference host by `build.sh`'s libexpanse build with one addition, `RUSTFLAGS="-C llvm-args=-align-all-functions=6"`, which starts every function on a 64-byte boundary:
  - N64: the source of N (`6ec29a656`);
  - O64: the source of O (`82eb56af`).
- **Toolchain control.** Before the aligned builds, the N source is rebuilt with an empty `RUSTFLAGS` and its sha256 must equal N's file (`da5814a7…`). If it does not, the toolchain has moved, nothing is run, and the result is reported as that. The O source is rebuilt the same way and its sha256 compared with O's file (`cce4ba9d…`); that comparison is reported and gates nothing, since O is not an arm. `82eb56af` predates the opt-in `DT_SONAME` (#1667), so O64 is built without one, as O was.
- **Layout check.** `objdump -t` must place `get::locate_slot_popcnt` and `JudyLGet` at offset 0 within a 64-byte line in both N64 and O64. If it does not, nothing is run.
- Four arms, the same stress-ng binary and judy stressor, differing only in `LD_LIBRARY_PATH`: N, N64, O64 and S (stock). `sng_judy_1k` at W = 1, 2, 4, 8, one process per arm per W per round.
- 12 rounds per run, rotating the arms through their four cyclic orders, so every arm runs in every position equally often. Two independent runs under the host bench lock and the core pin of the comparison. Load windows, the discard rule and the void rule are the library A/B's.
- `scripts/diag_layout_analyze.py` writes `results/diagnostic_layout_ref_run{1,2}.json`: two-sample BCa 95% intervals on N64/N, O64/N64, N64/S and N/S for find, insert, delete, bogo-ops/s and wall clock.

**Reading rule, per W, from the find-rate intervals over both runs.** A pair is SAME when both intervals lie inside [0.98, 1.02], FASTER when both lower bounds exceed 1, SLOWER when both upper bounds are below 1, and MIXED otherwise.
- PLACEMENT: O64/N64 SAME, and N64/N FASTER or SLOWER. Equal layout removes the O/N gap, and the layout alone moves N.
- NOT PLACEMENT: O64/N64 FASTER. O's code is faster at equal layout, so the gap is in the code.
- UNRESOLVED: anything else.

**#1684's fix criterion is separate from the reading.** N64/S lower bound at or above 1.0 at every W in both runs, here and on the second host, would make 64-byte function alignment a candidate fix. The second-host run is not part of this design and is booked separately.

**Expected if placement carries the gap** (projected): O64/N64 ≈ 1.00. N64/N could be on either side of 1, since offset 0 within a line is neither O's offset (48) nor N's (16).

### Layout result

(measured: reference host Intel Core i9-12900F, pin `0,2,4,6,8,10,12,14`, governor `powersave`, rustc 1.98.1, harness `5c0c3861e`; artifacts `results/diagnostic_layout_ref_run{1,2}.json`) (workload: sng_judy_1k)

**Builds.** Under the bench lock, 20:51:51–20:52:35 UTC.
- The no-flag rebuild of the N source reproduced N's file byte for byte (sha256 `da5814a7…`), and the no-flag rebuild of the O source reproduced O's (`cce4ba9d…`). The toolchain had not moved since either file was built, so the flag is the only difference between N and N64, and between O and O64.
- With the flag, N64 (`d99c1f0c…`) places `get::locate_slot_popcnt` at `0x230900` and `JudyLGet` at `0x44300`, and O64 (`b7745ddf…`) places them at `0x230880` and `0x44280`. All four sit at offset 0 within a 64-byte line.
- The flag grows the file by 23,104 bytes (0.82%), 2,828,040 → 2,851,144.
- A first attempt stopped on the build script's SONAME check before anything was timed. `82eb56af` predates the opt-in `DT_SONAME` (#1667), and `5c0c3861e` builds O64 without one, as O was built.

**Hygiene.** Run 1 21:20:02–21:21:38 UTC and run 2 21:21:38–21:23:43, each under the bench lock, after the 21:17 host rsync had exited. 192 processes per run and none discarded; the worst foreign load over any process was 0.21 core-equivalents, and max load1 was 2.23 and 3.28 against a void limit of 12.

**Reading: PLACEMENT at every W in both runs.** At equal layout the two sources find alike, and the layout alone moves N's file by the whole O/N gap.

| W | O64/N64 run 1 | O64/N64 run 2 | N64/N run 1 | N64/N run 2 | N64/S run 1 | N64/S run 2 |
|---|---|---|---|---|---|---|
| 1 | 0.999 [0.993, 1.006] | 1.001 [0.996, 1.008] | 1.128 [1.120, 1.135] | 1.124 [1.117, 1.131] | 1.086 [1.080, 1.090] | 1.084 [1.078, 1.089] |
| 2 | 0.998 [0.991, 1.004] | 0.999 [0.995, 1.003] | 1.129 [1.121, 1.138] | 1.130 [1.122, 1.137] | 1.086 [1.080, 1.092] | 1.090 [1.085, 1.094] |
| 4 | 1.000 [0.996, 1.005] | 0.997 [0.993, 1.003] | 1.138 [1.132, 1.146] | 1.137 [1.129, 1.145] | 1.096 [1.089, 1.101] | 1.095 [1.088, 1.101] |
| 8 | 0.998 [0.995, 1.000] | 0.999 [0.997, 1.001] | 1.138 [1.133, 1.141] | 1.138 [1.134, 1.146] | 1.089 [1.086, 1.091] | 1.088 [1.086, 1.089] |

- **Per-arm find rates.** Means at W = 1 over both runs, in M ops/s: N 97.03, N64 109.29, O64 109.30, S 100.71. N64 recovers O's 108.94 from the library A/B.
- **The other phases.**
  - Delete is unchanged by the layout: N64/N 0.995–1.008, O64/N64 0.994–1.001.
  - Insert costs a little: N64/N 0.976–0.986, SLOWER at W = 2 and W = 4 and SAME at W = 1 and W = 8 by the reading's bands. O64/N64 on insert is 1.000–1.005, so the insert cost is the layout's, not either source's.
  - N64/S on insert is 1.090–1.099.
- **Whole process.** Bogo-ops N64/N is 1.005–1.016. Wall clock N64/N is 0.984–0.995, the aligned processes finishing sooner.
- **#1684's fix criterion on this host.** N64/S on find has its lower bound above 1.0 at every W in both runs (1.078 lowest). The criterion also needs the second host, which this design does not cover.
- **What this establishes.** The O/N find gap on this host is a property of code placement and not of the code. Aligning every function to 64 bytes fixes the placement independently of link order and, on this host, takes the 1k find from 0.96× stock to 1.08–1.10×. Why offset 0 is fast and offset 16 slow is not established here, beyond the counter result above.

### Layout result (second host)

(measured: second host AMD Ryzen 9 9955HX (Zen 5), CPUs `0-7` pinned, governor `performance` on every CPU for the timed rounds, the same stress-ng binary and N, N64, O64 and stock files as the reference host (sha256 `490c0595…`, `da5814a7…`, `d99c1f0c…`, `b7745ddf…`, `76f121a8…`), harness `5c0c3861e`; artifacts `results/diagnostic_layout_zen5_run{1,2}.json`) (workload: sng_judy_1k)

**Hygiene.** Run 1 started 02:41:34 UTC and run 2 ended 02:44:49, under the bench lock.
- The governor was set to `performance`, and the host's CI runner container was stopped for the timed rounds only, both restored and read back equal afterwards (METHODOLOGY.md §3).
- 192 processes per run and none discarded. The foreign load before each run was 0.010 and 0.012 core-equivalents, and max load1 was 1.86 and 2.29 against a void limit of 16.

**#1684's fix criterion: MET on both hosts.** N64/S on find has its lower bound above 1.0 at every W in both runs here as on the reference host. The lowest lower bound is 1.143 here and 1.078 there.

| W | O64/N64 run 1 | O64/N64 run 2 | N64/N run 1 | N64/N run 2 | N64/S run 1 | N64/S run 2 | reading |
|---|---|---|---|---|---|---|---|
| 1 | 0.996 [0.984, 1.007] | 1.016 [1.003, 1.045] | 1.047 [1.035, 1.059] | 1.024 [0.991, 1.038] | 1.200 [1.185, 1.207] | 1.183 [1.143, 1.198] | UNRESOLVED |
| 2 | 1.001 [1.000, 1.002] | 1.003 [0.997, 1.016] | 1.047 [1.043, 1.055] | 1.039 [1.025, 1.046] | 1.201 [1.198, 1.204] | 1.197 [1.180, 1.204] | PLACEMENT |
| 4 | 1.003 [1.000, 1.008] | 1.008 [1.003, 1.023] | 1.045 [1.040, 1.049] | 1.042 [1.027, 1.048] | 1.202 [1.195, 1.204] | 1.197 [1.182, 1.203] | UNRESOLVED |
| 8 | 1.002 [1.001, 1.002] | 1.005 [1.001, 1.021] | 1.047 [1.044, 1.050] | 1.049 [1.039, 1.064] | 1.205 [1.202, 1.213] | 1.197 [1.181, 1.203] | NOT PLACEMENT |

- **Per-arm find rates.** Means at W = 1 over both runs, in M ops/s: N 142.08, N64 147.15, O64 147.98, S 123.53. On this host N already finds 1.15× faster than stock, and alignment adds 4–5%.
- **The placement reading is mixed here.**
  - At equal layout, O64/N64 is 0.996–1.016. Its intervals fall inside [0.98, 1.02] at W = 2 only, reach past 1.02 at W = 1 and 4, and have lower bounds above 1 in both runs at W = 8.
  - O's code is at most 1.6% faster at equal layout, against N64/N's 1.024–1.049 from the layout alone.
  - On this host the O/N find gap was itself small: the comparison above found N 1.14–1.15× stock, and the library A/B was not run here. So the reading has less to separate than on the reference host, where it is PLACEMENT at every W.
- **No cost on the other phases here.** N64/N on insert is 0.995–1.003 and on delete 1.001–1.006, every interval inside [0.98, 1.02]. The reference host's 1.4–2.4% insert cost does not appear on this host.
- **What this establishes for #1684.** Aligning every function to 64 bytes takes the 1k find from 0.96× to 1.08–1.10× stock on the reference host and from 1.15× to 1.18–1.21× here. Insert pays 1.4–2.4% on the reference host and nothing here, and delete is unchanged on both. That makes 64-byte function alignment a candidate fix measured on both hosts. Shipping it is a build-configuration change, made separately.
## Diagnostic: phase counters on the shapes Expanse loses

**A diagnostic, not a pre-registered result** (#1680). METHODOLOGY.md does not cover it, and nothing below changes a verdict above.

**Question.** The comparison above finds Expanse slower than stock on the sparsematrix judy gets and on the 4 Mi `judy` inserts. Where do Expanse's extra cycles go in those phases: memory stalls, the front end, or something else? stress-ng's own rates time each phase, but a whole-process counter mixes the phases, so this diagnostic counts one phase only.

**Design, fixed before the run** (`scripts/phase_probe.c`, `scripts/diag_phase_counters.py`, `run.sh <run> ref phase`):
- `phase_probe` replays a stressor's key shape through the Judy API and enables `perf stat` (`-D -1 --control fifo:…`) only around the measured phase. It is built against this repository's clean-room `include/Judy.h`. Its draws come from splitmix64, so the keys follow each stressor's distribution and not its exact sequence, and every arm draws the same keys.
  - `sm_default`: 10,000 draws of `(x, y)` on `[0, 500)²`, key `x << 32 | y`, each put when absent. The measured phase is both of the stressor's get phases (the same draws again, then as many fresh draws), repeated 200 times on the same array: 4,000,000 gets.
  - `sm_big`: the same over 1,000,000 draws on `[0, 100000)²`, once: 2,000,000 gets.
  - `judy_4m`: the inserts of `gen_index(i)` for `i` below 4,194,304, the measured phase: 4,194,304 inserts.
- Three arms, differing only in `LD_LIBRARY_PATH`: N (the library of the pre-registered runs), N64 (its source with 64-byte function alignment, from the layout diagnostic) and S (stock).
- User-mode `cpu_core` events, fixed `cycles` and `instructions` in every process, and one group per process:
  - `mem`: `mem_load_retired.l1_miss`, `.l2_miss`, `.l3_miss`, `br_misp_retired.all_branches`;
  - `stall`: `cycle_activity.stalls_total`, `.stalls_l1d_miss`, `.stalls_l2_miss`;
  - `l3`: `cycle_activity.stalls_l3_miss` alone. The first launch's preflight found it counting 350 alone and 0 beside the other three stall events, the #1177 failure, and stopped before anything was timed;
  - `fe`: the fixed `slots` counter, `idq_uops_not_delivered.core`, `dtlb_load_misses.walk_completed`, `br_inst_retired.all_branches`.
- The preflight, the discard rule and the void rule are the counter diagnostic's. The preflight runs on `sm_big`, whose misses reach every cache level. A second launch's preflight on `sm_default` read `mem_load_retired.l3_miss` as 9 alone and 0 in its group, too few to tell a co-scheduling zero from run-to-run variation, and stopped before anything was timed. 12 rounds per run, cycling the arms through their six orders and shuffling (shape, group) within a round. Two independent runs under the host bench lock and the comparison's pin.
- `scripts/diag_phase_counters_analyze.py` writes `results/diagnostic_phase_counters_ref_run{1,2}.json`: per shape, two-sample BCa 95% intervals on N/S, N64/S and N64/N for every counter, with per-operation means.

**Reading rule, N against S, per shape, over both runs:**
- If the `cycles` lower bound exceeds 1 in both runs (N takes more cycles than S), take the arms' means, and with ΔC = cycles_N − cycles_S:
  - memory share m = Δ`cycle_activity.stalls_l1d_miss` / ΔC;
  - front-end share f = Δ`idq_uops_not_delivered.core` / (w · ΔC), with w = slots_N / cycles_N.
- MEMORY: m ≥ 0.5 in both runs and f < 0.5 in either. FRONT-END: the reverse. BOTH: both ≥ 0.5 in both runs. MIXED: neither.
- NO GAP: the `cycles` upper bound is below 1 in both runs. UNRESOLVED: the cycle intervals disagree on a side.
- The other counters are reported with their intervals and name which level of the hierarchy moved. They carry no verdict of their own.
- A share names where the cycles went, not why the engine spends them. N64 against N says whether the layout matters on these phases, as it does on the 1k find.

### Phase result

(measured: reference host Intel Core i9-12900F, pin `0,2,4,6,8,10,12,14`, governor `powersave`, perf 6.8.12, harness `bbeab35a8`, analysis `4cb8bb6cb`; artifacts `results/diagnostic_phase_counters_ref_run{1,2}.json`) (workloads: phase_sm_default, phase_sm_big, phase_judy_4m)

**Hygiene.** Run 1 01:33:33–01:35:03 UTC and run 2 01:35:03–01:36:34, each under the bench lock.
- 432 processes per run. One process of run 1 was discarded by the library A/B's rule, and none of run 2.
- Max load1 was 0.80 and 0.96 against a void limit of 12.
- Two earlier launches stopped in the preflight before anything was timed (`8b0cabeae` and `bbeab35a8` fixed them, as the design section records).
- The first analysis stopped on a pair with zero counts in an arm (an L3 miss on the cache-resident `sm_default` array). `4cb8bb6cb` reports such a pair by its means with no interval; the raw rows are unchanged.

**Reading.** The sparsematrix gets read FRONT-END and the 4 Mi inserts read MIXED. Per-operation means are below, with N/S over both runs.

| Shape | Counter | N per op | S per op | N/S run 1 | N/S run 2 |
|---|---|---|---|---|---|
| `sm_default` | `cycles` | 181.9 | 120.4 | 1.5109 [1.5082, 1.5138] | 1.5105 [1.5079, 1.5130] |
| `sm_default` | `instructions` | 281.9 | 275.9 | 1.0217 (bc) | 1.0217 (bc) |
| `sm_default` | `br_misp_retired.all_branches` | 3.117 | 0.4891 | 6.3738 [6.3570, 6.3886] | 6.3953 [6.3793, 6.4118] |
| `sm_default` | `idq_uops_not_delivered.core` | 469.8 | 54.36 | 8.6422 [8.5723, 8.7202] | 8.6266 [8.5483, 8.6929] |
| `sm_default` | `cycle_activity.stalls_l1d_miss` | 2.474 | 0.9108 | 2.7161 [2.5561, 2.9190] | 2.6542 [2.5016, 2.8774] |
| `sm_big` | `cycles` | 250.5 | 179.0 | 1.3995 [1.3976, 1.4014] | 1.3962 [1.3934, 1.3981] |
| `sm_big` | `instructions` | 279.2 | 266.8 | 1.0468 (bc) | 1.0468 (bc) |
| `sm_big` | `br_misp_retired.all_branches` | 2.312 | 0.5229 | 4.4212 [4.4150, 4.4267] | 4.4255 [4.4197, 4.4309] |
| `sm_big` | `idq_uops_not_delivered.core` | 472.9 | 52.4 | 9.0250 [8.9863, 9.0490] | 8.9680 [8.8468, 9.0158] |
| `sm_big` | `cycle_activity.stalls_l1d_miss` | 28.17 | 31.85 | 0.8842 [0.8750, 0.8939] | 0.8802 [0.8706, 0.8930] |
| `judy_4m` | `cycles` | 189.5 | 167.4 | 1.1315 [1.1227, 1.1404] | 1.1328 [1.1225, 1.1410] |
| `judy_4m` | `instructions` | 661.7 | 783.7 | 0.8443 (bc) | 0.8443 (bc) |
| `judy_4m` | `br_misp_retired.all_branches` | 0.1748 | 0.1514 | 1.1539 [1.1501, 1.1591] | 1.1530 [1.1470, 1.1584] |
| `judy_4m` | `idq_uops_not_delivered.core` | 68.77 | 37.70 | 1.8240 [1.8063, 1.8400] | 1.8163 [1.8037, 1.8307] |
| `judy_4m` | `cycle_activity.stalls_l1d_miss` | 8.240 | 4.568 | 1.8040 [1.6284, 1.9680] | 1.9085 [1.7034, 2.0713] |

- **Sparsematrix gets: FRONT-END on both sizes.**
  - The front-end share of N's extra cycles is 1.125 and 1.124 at `sm_default` and 0.980 and 0.986 at `sm_big`. The memory share is 0.024–0.025 and −0.054 to −0.052.
  - N retires about stock's instructions (1.02× and 1.05×) and mispredicts 6.4× and 4.4× as many branches: 2.6 and 1.8 extra mispredictions per get.
  - The issue slots left empty after a misprediction, while the front end refetches, count in `idq_uops_not_delivered.core`. That is why the front-end share can exceed 1, and why it names the stage the cycles go to rather than the cause.
  - That mispredictions drive these gets is consistent with the counters but not established by them. No instrument here splits the empty slots by what emptied them.
  - Memory does not explain the gap: at `sm_big` N's L1D-miss stall cycles are 0.88× stock's.
- **4 Mi inserts: MIXED.** N retires 0.84× stock's instructions and takes 1.13× its cycles. The memory share is 0.167 and 0.188 and the front-end share 0.233 and 0.230. N's L1D-miss stall cycles are 1.80–1.91× stock's, its empty slots 1.82×, and its mispredictions 1.15×. No single stage carries half the difference.
- **Alignment does not move these phases.**
  - N64/N cycles are 0.9894–0.9982, every interval inside [0.98, 1.02].
  - N64/S cycles are 1.495 and 1.496 (`sm_default`), 1.389 and 1.392 (`sm_big`), and 1.127 and 1.131 (`judy_4m`).
  - 64-byte function alignment, which fixed the 1k find, is not a fix for #1680.
- **Not measured.** Why N mispredicts more here, and at which branches. A `perf record` of `br_misp_retired.all_branches` on the get phase would name them.
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

**D/S is superseded on main** by [The #1679 criterion on main](#the-1679-criterion-on-main), which re-measures the same cells with #1719 and #1720 in the library. D/B stands as the measure of #1708's change.

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

### Delete result (second host)

(measured: second host AMD Ryzen 9 9955HX (Zen 5), CPUs `0-7` pinned, governor `performance` on every CPU for the timed rounds, the same stress-ng binary and B, D and stock files as the reference host (sha256 `490c0595…`, `da5814a7…`, `f6bb333e…`, `76f121a8…`), harness `9d9628307`; artifacts `results/diagnostic_delete_zen5_run{1,2}.json`) (workloads: sng_judy_1k, sng_judy_256k, sng_judy_4m)

**Hygiene.** Run 1 02:44:54–02:51:33 UTC and run 2 02:51:38–02:58:48, under the bench lock held once for this host's layout and delete runs.
- The governor was set to `performance`, and the host's CI runner container was stopped for those rounds only, both restored and read back equal afterwards (METHODOLOGY.md §3).
- 432 processes per run and none discarded. The foreign load before each run was 0.008 and 0.014 core-equivalents, and max load1 was 4.03 and 4.46 against a void limit of 16.
- The first attempt, at 00:16, stopped before anything was timed, because an idle CI job container was running and `run.sh` refuses to run beside one.

**Reading: #1679's criterion is NOT MET on this host either.**

| Cell | D/B delete (all W, both runs) | D/S delete (all W, both runs) |
|---|---|---|
| 1 Ki | 1.150–1.156, FASTER | 0.923–0.928, below stock |
| 256 Ki | 1.473–1.504, FASTER | 1.011–1.093, at or above stock |
| 4 Mi | 1.397–1.409, FASTER | 0.981–1.021: below stock at W = 1 and 2, at or above at W = 4 and 8 |

- **The fix's effect** matches the reference host's: 1.15× at 1 Ki, 1.47–1.50× at 256 Ki and 1.40–1.41× at 4 Mi.
- **Against stock, compared with the reference host.** D/S is higher here at 4 Mi (0.98–1.02 against 0.87–0.90), lower at 256 Ki (1.01–1.09 against 1.07–1.11), and about the same at 1 Ki (0.92 against 0.91). Why the 4 Mi gap is smaller on this host is not measured.
- **Controls.** Insert holds: D/B point estimates 0.996–1.007. Find moves 1.05–1.07× at 1 Ki, the placement effect seen on the reference host, and is within 2% of 1 at 256 Ki and 4 Mi.

## Diagnostic: delete-phase counters

**A diagnostic, not a pre-registered result** (#1679). METHODOLOGY.md does not cover it, and nothing below changes a verdict above.

**Question.** With #1708's fix, Expanse's stress-ng delete rate is still below stock at 1 Ki keys (0.91× on the reference host) and at 4 Mi (0.87–0.90×). Callgrind puts about 16% of the 4 Mi delete's instructions in the recursive `map_remove`'s entry and exit. Before changing the engine: where do Expanse's extra delete cycles go?

**Design, fixed before the run** (`scripts/phase_probe.c` shapes `del_1k` and `del_4m`; `run.sh <run> ref phase_delete`, which runs `scripts/diag_phase_counters.py` with `SNG_PHASE_SET=delete`):
- `del_1k`: JLI of `gen_index(i)` for `i` below 1,024, then JLD in reverse order, stress-ng's delete phase at its minimum size. Each of 10,000 repetitions rebuilds the array with counting disabled and deletes it with counting enabled, so the array is as warm as in stress-ng's run: 10,240,000 deletes. The first launch used 1,000 repetitions. A process then lasted 0.085 s, under the load window's 0.1 s minimum, so every `del_1k` process read no foreign load and was discarded. That launch is not used, for either shape.
- `del_4m`: the same at 4,194,304 keys, once.
- Two arms, differing only in `LD_LIBRARY_PATH`: D (#1708's library, `45f83ac5e`, sha256 `f6bb333e…`, the code on main) and S (stock).
- The event groups, the preflight, the rounds (12 per run, alternating the two arms' order), the discard and void rules and the analysis are the phase-counter diagnostic's above. The preflight runs on `del_4m`.

**Reading rule, D against S, per shape, over both runs.** The phase-counter rule above applies with D in N's place: MEMORY, FRONT-END, BOTH, MIXED, NO GAP or UNRESOLVED. One addition decides whether #1679's next engine increment, moving the rarely taken paths out of the recursive `map_remove`, goes ahead.
- **CALL-STRUCTURE:** D's cycles are above S's, D's `instructions` lower bound exceeds 1 in both runs, and in both runs the memory share and the front-end share are each below 0.5. Expanse does more work and is neither waiting on memory nor starved by its front end, which is where per-level call overhead would show.
- **Any other reading:** report before any engine change.

### Delete-phase result

(measured: reference host Intel Core i9-12900F, pin `0,2,4,6,8,10,12,14`, governor `powersave`, perf 6.8.12, harness `80518b1bf`; artifacts `results/diagnostic_delete_phase_ref_run{1,2}.json`) (workloads: phase_del_1k, phase_del_4m)

**Hygiene.** Run 1 04:24:10–04:26:14 UTC and run 2 04:26:14–04:28:17, each under the bench lock. 192 processes per run and none discarded; the worst foreign load over any process was 0.17 core-equivalents, and max load1 was 0.99 and 1.00 against a void limit of 12.

**Reading: not CALL-STRUCTURE at either size.** `del_1k` reads FRONT-END and `del_4m` MIXED. Under the design, #1679's next engine increment does not go ahead on this evidence.

| Shape | Counter | D per op | S per op | D/S run 1 | D/S run 2 |
|---|---|---|---|---|---|
| `del_1k` | `cycles` | 157.5 | 143.8 | 1.0959 [1.0926, 1.1074] | 1.0939 [1.0929, 1.0953] |
| `del_1k` | `instructions` | 661.4 | 771.4 | 0.8574 (bc) | 0.8574 (bc) |
| `del_1k` | `idq_uops_not_delivered.core` | 144.1 | 60.70 | 2.3748 [2.3470, 2.3987] | 2.3957 [2.3830, 2.4095] |
| `del_1k` | `cycle_activity.stalls_total` | 12.46 | 3.914 | 3.1841 [3.1065, 3.4128] | 3.1901 [3.1096, 3.2496] |
| `del_1k` | `br_misp_retired.all_branches` | 0.05849 | 0.08112 | 0.7211 [0.7130, 0.7287] | 0.7314 [0.7231, 0.7390] |
| `del_4m` | `cycles` | 212.0 | 191.8 | 1.1055 [1.0987, 1.1131] | 1.0984 [1.0907, 1.1055] |
| `del_4m` | `instructions` | 745.8 | 812.9 | 0.9174 (bc) | 0.9174 (bc) |
| `del_4m` | `idq_uops_not_delivered.core` | 64.27 | 22.31 | 2.8803 [2.8622, 2.9044] | 2.8369 [2.8199, 2.8517] |
| `del_4m` | `cycle_activity.stalls_l1d_miss` | 13.95 | 11.02 | 1.2662 [1.1944, 1.3379] | 1.2242 [1.1352, 1.3077] |
| `del_4m` | `mem_load_retired.l1_miss` | 5.086 | 2.448 | 2.0774 [1.8964, 2.2339] | 2.0421 [1.8600, 2.2046] |
| `del_4m` | `br_misp_retired.all_branches` | 0.1472 | 0.1456 | 1.0104 [1.0078, 1.0134] | 1.0098 [1.0069, 1.0127] |

- **Expanse retires fewer instructions than stock on both deletes:** 0.857× at 1 Ki and 0.917× at 4 Mi. Its extra cycles are not extra work, so they are not the per-level call overhead the outlining increment targets. That overhead would show as more instructions retired.
- **1 Ki: FRONT-END.**
  - The front-end share of D's extra cycles is 1.011 and 1.034, and the memory share 0.009.
  - D's front end leaves 2.4× stock's issue slots empty, and its total stall cycles are 3.2× stock's.
  - D mispredicts fewer branches than stock (0.72×), so the empty slots are not misprediction refetch.
  - Which front-end resource is short is not measured here: the uop-source counters of the 1k find diagnostic were not in these groups.
- **4 Mi: MIXED.** The memory share is 0.145 and 0.131, the front-end share 0.346 and 0.368. D's L1 misses are 2.0× stock's, its L1D-miss stall cycles 1.22–1.27×, and its empty front-end slots 2.8×. Mispredictions are equal.
- **Not measured.** The front-end resource behind the empty slots. The layout diagnostic found the delete rate insensitive to 64-byte function alignment (N64/N 0.995–1.008), so whole-function placement does not explain it; code size or uop-cache capacity are not excluded.

## Diagnostic: front-end resource on the delete phase

**A diagnostic, not a pre-registered result** (#1679). METHODOLOGY.md does not cover it, and nothing below changes a verdict above.

**Question.** The delete-phase counters put Expanse's extra 1 Ki delete cycles in its front end, and a third of its extra 4 Mi cycles there, but not which front-end resource. Does the gap sit in uop-cache or instruction-cache capacity? That reading lets #1679's next increment proceed: moving the rarely taken paths out of the recursive `map_remove`, framed as a code-footprint change.

**Design, fixed before the run** (`run.sh <run> ref phase_delete_fe`, which runs `scripts/diag_phase_counters.py` with `SNG_PHASE_SET=delete_fe`):
- The delete-phase design above, unchanged in shapes (`del_1k`, `del_4m`), arms (D, #1708's library, against S, stock), rounds, preflight, discard and void rules.
- The event groups are those of the 1k find diagnostic: `uops`, `fe`, `misc` and `br`, beside the fixed `cycles` and `instructions`.
- `scripts/diag_phase_counters_analyze.py` writes `results/diagnostic_delete_fe_ref_run{1,2}.json`.

**Reading rule, D against S, per shape, over both runs.** It applies where D's `cycles` lower bound exceeds 1 in both runs; otherwise NO GAP or UNRESOLVED as above. Let d be D's mean cycles per process minus S's. For each fetch-latency counter, its share of d is D's mean minus S's, divided by d:
- UOP-CACHE: `dsb2mite_switches.penalty_cycles`;
- ICACHE: `icache_data.stalls`;
- ITLB: `icache_tag.stalls`;
- LCP: `decode.lcp`.

A resource is **named** when its share is at least 0.25 in both runs. Two uop-source readings can also name one. The share of an arm's delivered uops from a source is that source's mean over the sum of the four `uops` counters.
- UOP-CACHE is also named when D's share of uops from the legacy decoders (`idq.mite_uops`) exceeds S's by at least 0.10 in both runs.
- LOOP-STREAM is named when D's share from the loop stream detector (`lsd.uops`) is below S's by at least 0.10 in both runs.

What the reading decides:
- If neither of those names a resource, the reading is UNNAMED. The empty slots then sit in a front-end cost these counters do not separate.
- **Option 2 proceeds** only when UOP-CACHE or ICACHE is named on a shape. Each increment is then measured alone against the delete arms, and any Callgrind arm above +0.1% is disclosed.
- **Any other reading:** report before any engine change. This includes ITLB, LCP, LOOP-STREAM alone, or UNNAMED.

The front-end share f of the phase-counter rule is printed beside the named resources, and every counter's D/S interval is reported. The 0.25 and 0.10 bounds are fixed here, before the run.

### Front-end resource result

(measured: reference host Intel Core i9-12900F, pin `0,2,4,6,8,10,12,14`, governor `powersave`, perf 6.8.12, harness `10b6847c6`; artifacts `results/diagnostic_delete_fe_ref_run{1,2}.json`) (workloads: phase_del_1k, phase_del_4m)

**Hygiene.** Run 1 ran 17:39:00–17:41:06 UTC and run 2 17:41:06–17:43:12, each under the bench lock. Run 1 had 192 processes with 2 discarded, and run 2 had 192 with none discarded. The worst foreign load over any process was 0.37 core-equivalents (run 1) and 0.04 (run 2). Max load1 was 0.93 and 1.05 against a void limit of 12. The preflight found no event that counts alone and reads zero in its group, and no row ran below 100% of the time.

**Reading: UNNAMED at both sizes.** No measured front-end resource carries the gap, so under the design #1679's outlining increment does not go ahead on this evidence.

| Shape | Counter | D per op | S per op | D/S run 1 | D/S run 2 |
|---|---|---|---|---|---|
| `del_1k` | `cycles` | 157.8 | 143.6 | 1.0990 [1.0951, 1.1052] | 1.0939 [1.0931, 1.0948] |
| `del_1k` | `idq_uops_not_delivered.core` | 144.9 | 60.45 | 2.3967 [2.3745, 2.4239] | 2.4024 [2.3855, 2.4174] |
| `del_1k` | `uops_issued.any` | 648.3 | 735.3 | 0.8817 [0.8813, 0.8823] | 0.8814 [0.8810, 0.8817] |
| `del_1k` | `idq.dsb_uops` | 597.9 | 701.6 | 0.8522 [0.8510, 0.8554] | 0.8509 [0.8504, 0.8515] |
| `del_1k` | `dsb2mite_switches.penalty_cycles` | 2.897 | 2.345 | 1.2354 [1.2163, 1.2675] | 1.2394 [1.2200, 1.2570] |
| `del_1k` | `icache_data.stalls` | 0.5347 | 0.5408 | 0.9888 [0.9684, 1.0221] | 0.9754 [0.9667, 0.9836] |
| `del_1k` | `idq.ms_uops` | 2.047 | 0.1735 | 11.7949 [11.6085, 12.0117] | 11.9368 [11.8015, 12.1178] |
| `del_1k` | `br_inst_retired.all_branches` | 122.6 | 110.1 | 1.1139 (bc) | 1.1139 (bc) |
| `del_4m` | `cycles` | 211.8 | 192.3 | 1.1014 [1.0949, 1.1070] | 1.1048 [1.0981, 1.1122] |
| `del_4m` | `idq_uops_not_delivered.core` | 64.27 | 22.41 | 2.8677 [2.8504, 2.8812] | 2.8706 [2.8501, 2.8879] |
| `del_4m` | `dsb2mite_switches.penalty_cycles` | 0.774 | 0.3865 | 2.0024 [1.7170, 2.3936] | 1.9024 [1.6636, 2.2307] |
| `del_4m` | `icache_data.stalls` | 0.1444 | 0.06643 | 2.1741 [2.1133, 2.2177] | 2.1954 [2.1525, 2.2412] |
| `del_4m` | `icache_tag.stalls` | 0.3743 | 0.1018 | 3.6766 [3.4659, 3.9310] | 3.9530 [3.7121, 4.1546] |
| `del_4m` | `idq.mite_uops` | 4.501 | 1.152 | 3.9079 [3.7839, 4.0488] | 3.8399 [3.6919, 3.9635] |
| `del_4m` | `br_inst_retired.all_branches` | 125.7 | 89.81 | 1.3992 (bc) | 1.3992 (bc) |

Every other counter's interval is in the artifacts. `decode.lcp` has no ratio, because one arm counts zero in some processes.

- **The fetch-latency counters carry almost none of D's extra cycles.** The four shares are:
  - UOP-CACHE (DSB-to-MITE switch penalty): 0.039 and 0.042 at 1 Ki, 0.020 and 0.017 at 4 Mi;
  - ICACHE: 0.000 and −0.001 at 1 Ki, 0.004 at 4 Mi;
  - ITLB: 0.001–0.003 at 1 Ki, 0.014 at 4 Mi;
  - LCP: 0.001 or less.

  The 4 Mi instruction-cache, ITLB and switch counters are 2–4× stock's, but per operation they are fractions of a cycle against 19.5 extra cycles.
- **The uop sources are nearly the same.** At 1 Ki, D's uops come 94.4% from the uop cache (DSB), 3.7–3.8% from the legacy decoders and 1.5–1.6% from the loop stream detector. Stock's come 95.7%, 3.5% and 0.8%. At 4 Mi the figures are 97.5%, 0.6% and 1.8% against stock's 99.2%, 0.1% and 0.6%. Neither the MITE shift nor the LSD shift reaches the 0.10 bound, so the uop cache is not missing D's delete code.
- **What remains.** The front end still leaves D's issue slots empty: f = 0.991 and 1.040 at 1 Ki, 0.358 and 0.349 at 4 Mi, as in the delete-phase counters. Two differences are measured:
  - D issues 0.88× and 0.91× stock's uops, but retires 1.11× (1 Ki) and 1.40× (4 Mi) stock's branches.
  - D takes 12× (1 Ki) and 56× (4 Mi) stock's microcode-sequencer uops, which is 2.0 and 0.8 uops per operation.

  A per-cycle limit on taken branches in uop-cache delivery fits the first difference. The cost of switching to the microcode sequencer fits the second. Both are **hypotheses, unmeasured here**: no counter in these groups reads taken branches per cycle or microcode-sequencer switches, so neither is a finding.
- **Next instrument, not run.** Separating them needs `br_inst_retired.near_taken` and `idq.ms_switches` (or the MS-switch cycles counter), read as shares of D's extra cycles. That would be another design, fixed before its run.

## Diagnostic: taken branches and microcode on the delete phase

**A diagnostic, not a pre-registered result** (#1679). METHODOLOGY.md does not cover it, and nothing below changes a verdict above.

**Question.** The front-end resource diagnostic above left Expanse's empty delete slots UNNAMED. Two measured differences remained:
- D retires 1.11× (1 Ki) and 1.40× (4 Mi) stock's branches while issuing fewer uops.
- D takes 12× and 56× stock's microcode-sequencer uops.

Does either carry the gap? Which instructions issue the microcode uops?

**Design, fixed before the run:**
- **Counters** (`run.sh <run> ref phase_delete_ms`, `SNG_PHASE_SET=delete_ms`). This is the delete-phase design above, unchanged in shapes (`del_1k`, `del_4m`), arms (D, #1708's library, against S, stock), rounds, preflight, discard and void rules. Three event groups sit beside the fixed `cycles` and `instructions`:
  - `ms`: `idq.ms_uops`, `idq.ms_switches`, `idq.ms_cycles_any`, `br_inst_retired.near_taken`;
  - `dsb`: `idq.dsb_cycles_any`, `idq.dsb_cycles_ok`, `idq.dsb_uops`, `uops_issued.any`;
  - `fe`: the fixed `slots` counter, `idq_uops_not_delivered.core`, `br_inst_retired.all_branches`, `idq.mite_uops`.
- `scripts/diag_phase_counters_analyze.py` writes `results/diagnostic_delete_ms_ref_run{1,2}.json`.
- **Attribution** (`run.sh 1 ref ms_attribution`, `scripts/diag_ms_attribution.py`). For each shape and arm, one probe process runs under `perf record`, with sampling enabled only around the delete phase. The event is `frontend_retired.ms_flows` with precise attribution, which counts retired instructions delivered by the microcode sequencer. If the host refuses it, the event is `idq.ms_uops` without precision, whose samples can land a few instructions after the issuing one; the artifact records which event was used. Sampling period: 503.
  - `perf annotate` gives each instruction's share of the samples. `scripts/diag_ms_attribution_analyze.py` writes `results/diagnostic_ms_attribution_ref.json`, with every instruction holding at least 1% of the samples and every function holding at least 0.5%.
  - That is one process per shape and arm: a census, not an interval.

**Reading rule for the counters: D against S, per shape, over both runs.** It applies where D's `cycles` lower bound exceeds 1 in both runs; otherwise NO GAP or UNRESOLVED as above. Let d be D's mean cycles per process minus S's, and Δ a counter's D mean minus its S mean.
- **Microcode-switch share:** 3 · Δ`idq.ms_switches` / d. The 3 cycles per switch is the switch cost constant of Intel's top-down microarchitecture analysis model, not measured on this host. Δ`idq.ms_cycles_any` / d, the cycles in which the sequencer was delivering, is reported beside it as an upper envelope and carries no verdict.
- **DSB-below-width share:** (Δ`idq.dsb_cycles_any` − Δ`idq.dsb_cycles_ok`) / d. These are the cycles in which the uop cache delivered uops but fewer than its full width.
- **Taken-branch density:** `br_inst_retired.near_taken` over `idq.dsb_uops`, D's over S's.

The verdicts:
- **MICROCODE:** the microcode-switch share is at least 0.25 in both runs. The gap points at microcode-switch cost.
- **TAKEN-BRANCH:** the DSB-below-width share is at least 0.25 and the taken-branch density ratio at least 1.10, both in both runs. The gap points at taken-branch delivery bandwidth: more taken branches per delivered uop, while the uop cache runs below its width. The DSB-below-width cycles alone do not name taken branches, since other limits can hold delivery below width too, so the density condition is part of the reading.
- **BOTH:** both named.
- **UNNAMED:** neither named.

**Reading rule for the attribution**, D per shape: the share of D's samples on a `rep`-prefixed string instruction (`movs`, `stos`, `cmps`, `scas`, `lods`). The hypothesis it tests, held as a hypothesis and not a target, is a short copy compiled to `rep movsb` in leaf removal.
- CONFIRMED at 0.5 or more;
- REFUTED at 0.1 or less;
- PARTIAL otherwise.

The functions and instructions holding the samples are named either way.

**Any reading:** report before any engine change.

### Taken-branch and microcode result

(measured: reference host Intel Core i9-12900F, pin `0,2,4,6,8,10,12,14`, governor `powersave`, perf 6.8.12; counters at harness `85379ff1c`, artifacts `results/diagnostic_delete_ms_ref_run{1,2}.json`; attribution at harness `3588d4b6e`, artifact `results/diagnostic_ms_attribution_ref.json`, raw text `results/ms_attribution_raw/`) (workloads: phase_del_1k, phase_del_4m)

**Hygiene.**
- **Counter runs.** Run 1 ran 19:51:09–19:52:46 UTC and run 2 19:52:46–19:54:23, each under the bench lock. Each had 144 processes and none was discarded. The worst foreign load was 0.05 and 0.04 core-equivalents, and max load1 was 0.82 and 0.97 against a void limit of 12. The preflight found no event that counts alone and reads zero in its group.
- **Attribution.** It ran three times under the lock, and only the third is used.
  - 19:54:23 stopped on stock's 4 Mi cell, which gave no sample. `859433de8` records a zero-sample cell instead.
  - 19:55:53 stopped on the same cell, which this time gave 4 samples outside any symbol `perf annotate` disassembles. `3588d4b6e` keeps a cell under 100 samples with its count.
  - 19:56:54–19:57:54 completed.
- **Symbols.** The run's parser did not read function names from perf 6.8's annotate layout, so the instruction lists were parsed again from the run's verbatim text with the parser fixed (`ms_annotate.py`). The re-parse reproduces each cell's sample total exactly, and the artifact records it per cell.
- **Sample totals.** Instruction shares are over the samples `perf annotate` placed: 39,989 of the 40,871 samples `perf script` counts for D at 1 Ki, and 6,617 of 6,620 at 4 Mi.

**Reading: TAKEN-BRANCH at both sizes. The microcode hypothesis is not supported, and its `rep movsb` form is refuted.**

| Shape | Counter | D per op | S per op | D/S run 1 | D/S run 2 |
|---|---|---|---|---|---|
| `del_1k` | `cycles` | 157.2 | 143.6 | 1.0944 [1.0926, 1.0961] | 1.0938 [1.0926, 1.0952] |
| `del_1k` | `br_inst_retired.near_taken` | 60.36 | 61.15 | 0.9872 (bc) | 0.9872 (degenerate) |
| `del_1k` | `idq.dsb_uops` | 597.8 | 701.7 | 0.8519 [0.8510, 0.8546] | 0.8510 [0.8505, 0.8515] |
| `del_1k` | `idq.dsb_cycles_any` | 107.3 | 116.0 | 0.9254 [0.9236, 0.9282] | 0.9252 [0.9245, 0.9260] |
| `del_1k` | `idq.dsb_cycles_ok` | 58.84 | 72.34 | 0.8134 [0.8120, 0.8158] | 0.8119 [0.8106, 0.8131] |
| `del_1k` | `idq.ms_switches` | 0.2332 | 0.02572 | 9.0648 [8.9122, 9.1881] | 8.9891 [8.8858, 9.1173] |
| `del_4m` | `cycles` | 213.4 | 192.0 | 1.1114 [1.1041, 1.1174] | 1.1080 [1.1006, 1.1143] |
| `del_4m` | `br_inst_retired.near_taken` | 60.16 | 53.30 | 1.1288 (degenerate) | 1.1288 (degenerate) |
| `del_4m` | `idq.dsb_uops` | 718.2 | 815.5 | 0.8806 [0.8795, 0.8814] | 0.8815 [0.8809, 0.8822] |
| `del_4m` | `idq.dsb_cycles_any` | 122.1 | 126.6 | 0.9643 [0.9629, 0.9655] | 0.9650 [0.9640, 0.9661] |
| `del_4m` | `idq.dsb_cycles_ok` | 73.07 | 91.37 | 0.7997 [0.7973, 0.8018] | 0.8006 [0.7990, 0.8022] |
| `del_4m` | `idq.ms_switches` | 0.1026 | 0.002313 | 44.3442 [43.0378, 45.4386] | 43.7060 [40.7469, 45.9682] |

Every other counter's interval is in the artifacts. The taken-branch counts are exact per process, so their intervals collapse (`bc`, `degenerate`).

| Shape | Run | Microcode-switch share | MS-active cycles share | DSB-below-width share | Taken-branch density D/S |
|---|---|---|---|---|---|
| `del_1k` | 1 | 0.046 | 0.038 | 0.358 | 1.159 |
| `del_1k` | 2 | 0.046 | 0.038 | 0.368 | 1.160 |
| `del_4m` | 1 | 0.014 | 0.011 | 0.644 | 1.282 |
| `del_4m` | 2 | 0.015 | 0.011 | 0.665 | 1.281 |

- **Microcode switches carry under 5% of D's extra cycles.** Under the model's 3 cycles per switch they carry 0.046 at 1 Ki and 0.014–0.015 at 4 Mi. The cycles in which the sequencer delivered anything carry less, 0.038 and 0.011. D's 9× and 44× switch counts are 0.23 and 0.10 switches per delete.
- **The uop cache runs below its width in more of D's cycles.** D's uop cache delivers in fewer cycles than stock's: 0.93× at 1 Ki, 0.96× at 4 Mi. But it delivers at full width in only 0.81× and 0.80× as many. The cycles in which it delivers below full width carry 0.36–0.37 of D's extra cycles at 1 Ki and 0.64–0.67 at 4 Mi.
- **D delivers fewer uops per taken branch.** At 1 Ki, D takes as many taken branches as stock (0.987×) while delivering 0.85× its uops from the uop cache, so each taken branch ends a shorter run of uops: 1.16× stock's density. At 4 Mi it takes 1.13× stock's taken branches with 0.88× its uops, 1.28× the density. That is the TAKEN-BRANCH reading. It names the front-end limit, not the code that causes it, which this run does not locate.
- **What issues D's microcode uops.** Of D's `frontend_retired.ms_flows` samples, 94.6% (1 Ki) and 83.1% (4 Mi) fall in `map_remove::<false, false>`, the plain-tree remove. Nearly all are `lock`-prefixed read-modify-writes on three words:
  - `lock add`/`lock sub` of a register at 0x200;
  - `lock inc`/`lock dec` at 0x208;
  - `lock inc` at 0x218.

  That is the pattern of the plain-tree allocator accounting in `crates/expanse/src/alloc.rs` at D's commit: `bytes_in_use` (`fetch_add`/`fetch_sub` of the size), `live_allocs` (±1) and `total_allocs` (+1), each an atomic read-modify-write. The match is by operation pattern; the offsets were not checked against the struct layout. No sampled instruction is a `rep` string instruction (share 0.000 at both sizes), so the `rep movsb` hypothesis is REFUTED. Stock issues almost no microcode on these phases: 38 samples at 1 Ki, mostly in the probe's `read`/`write` system calls, and none at 4 Mi.
- **Not priced here.** A locked read-modify-write costs more than its microcode switch. Its own latency and ordering are back-end costs, which these front-end counters do not measure. Whether the plain-tree accounting atomics cost delete cycles beyond the 4.6% is **unmeasured**. Who reads these counters, and what gating them would remove, is the audit AGENTS.md §8.22.1 asks for first (#930).

## Diagnostic: unlocked accounting on the delete phase

**A diagnostic ablation, not a pre-registered result and not a shippable change** (#1679). METHODOLOGY.md does not cover it, and nothing below changes a verdict above.

**Question.** The attribution above puts D's microcode uops on the plain tree's locked accounting updates. What do those `lock` prefixes cost the delete phase in all, back end included?

**Build.** Both libraries come from this section's commit, which adds the diagnostic feature `ablation-unlocked-plain-acct` (`crates/expanse/Cargo.toml`). They are built on the reference host under the bench lock by `scripts/build_layout_lib.sh`, with no rustflags:
- **A**, with `expanse-trie/ablation-unlocked-plain-acct`. On a plain tree's allocation path (`alloc_raw_body!`) and free path (`free_raw`) in `crates/expanse/src/alloc.rs`, `bytes_in_use`, `live_allocs` and `total_allocs` update by a relaxed load and a store instead of a locked read-modify-write. A deferred tree's sharded accounting is unchanged.
- **D**, with no feature. This D is not the `45f83ac5e` file of the earlier delete sections. It is this commit's engine, which includes #1705, #1710 and #1714.

**Controls, before the timed run, on a host that is not the reference host (none of it timed):**
- **Default unchanged.** D's source, built with the same toolchain, reproduces a build of its parent (`e63ed38dd`) byte for byte. If it does not, report before the run.
- **Disassembly.** `objdump -d` of A and of D:
  - in A, `map_remove::<false, false>` and every function that inlines the plain allocation or free path carry no `lock`-prefixed instruction on the accounting words;
  - every `lock` instruction that remains in A is listed with its function.

  Dumps go in `results/acct_ablation_disasm/`.
- **Callgrind.** Instructions per delete for A and for D: `phase_probe` under `valgrind --tool=callgrind --toggle-collect=JudyLDel`, on both shapes. `del_1k` runs at fewer repetitions. A's count may rise, since a locked update becomes a load, an add and a store. For a diagnostic that rise is not a regression verdict.

**Cells and instrument** (`run.sh <run> ref phase_delete_acct`, which runs `scripts/diag_phase_counters.py` with `SNG_PHASE_SET=delete_acct`):
- **Shapes.** `del_1k` (10,000 × 1,024 deletes) and `del_4m` (4,194,304 deletes), as in the delete-phase sections above.
- **Arms.** A, D and S (stock), differing only in `LD_LIBRARY_PATH`. The harness stops if A and D are the same file.
- **Counters.** One user-mode `cpu_core` group: `cycles`, `instructions`, `slots`, `idq_uops_not_delivered.core`, `cycle_activity.stalls_total` and `br_inst_retired.near_taken`.
- **Wall clock.** The phase's wall clock is `phase_ns`, the probe's own `CLOCK_MONOTONIC` time summed over the counted delete passes.
- **Rounds.** 12 rounds per run, cycling through the six orders of the three arms, and two runs. The preflight runs on `del_4m`, arm D.
- **Rules carried over.** The load windows and the discard and void rules are the phase-counter diagnostic's.
- **Intervals.** `scripts/diag_phase_counters_analyze.py` writes `results/diagnostic_delete_acct_ref_run{1,2}.json`, with two-sample BCa 95% intervals on A/D, A/S and D/S for every counter and for `phase_ns`.

**Reading rule, per shape, on `cycles`, over both runs** (`acct_reading` in the analyzer). Per run, the share is (D − A) / (D − S), on the arms' mean cycles: the part of D's cycle excess over stock that A removes.
- **MATTER:** A/D's upper bound is below 1, and the share is at least 0.25, in both runs. The locked counters carry at least 0.25 of D's excess over stock. The 0.25 bar is the share the earlier front-end rules used to name a resource.
- **SMALL:** A/D's upper bound is below 1 in both runs, and the share is below 0.25 in a run.
- **NO EFFECT:** A/D's interval contains 1 in both runs.
- **SLOWER:** A/D's lower bound is above 1 in both runs.
- **MIXED:** any other combination.
- **Wall clock.** The phase wall clock's A/D is read for its side of 1 in the same way. A run where it lies on another side than the cycle ratio is reported as WALL DISAGREES, never reinterpreted.
- **Against stock.** BELOW STOCK if A/S's upper bound is below 1 in both runs, ABOVE STOCK if its lower bound is above 1 in both, otherwise NOT SEPARATED FROM STOCK.

**Expected** (projected, not derived): A/D below 1 at both shapes. No size is predicted. The microcode-switch share above (0.046 of D's excess at 1 Ki, 0.014 at 4 Mi) bounds only the front-end part of the locked instructions' cost. Their back-end cost is what this run measures.

**Amendment, made after the controls ran and before any timed data** (AGENTS.md §8.19). The pre-registration above is unchanged.
- **The byte-identical control, as written, is NOT MET, and could not have passed.** D (`cc924b150`, no feature) and P (its parent `e63ed38dd`), built with one toolchain, differ in 49 bytes. They are the build-id note and 26 words of `.data.rel.ro`, each a panic `Location` line number in `alloc.rs`, shifted by 18 or 29 lines with the column unchanged. Any edit that moves lines in that file changes those words.
- **Replacement control: MET.** `.text`, `.rodata` and `.eh_frame` are identical between D and P, and the `objdump -d` diff is empty, so the default build's code is unchanged.
- The cells, the instrument and the reading rule stand as committed. The A-against-D comparison does not depend on this control.

**What the result does not decide.** Whether a shippable form exists is a separate decision. The counters are public: `NodeAlloc::live_allocs` and `total_allocs` in Rust, and `total_node_allocs` in the Python binding. AGENTS.md §2.6 and §8.22 apply. A MATTER reading says the `lock` prefixes cost delete cycles, not that this ablation's form is the fix.

### Unlocked-accounting result

(measured: reference host Intel Core i9-12900F, pin `0,2,4,6,8,10,12,14`, perf 6.8.12, rustc 1.98.1, libraries and harness `cc924b150`; artifacts `results/diagnostic_delete_acct_ref_run{1,2}.json`, `results/acct_ablation_disasm/`, `results/acct_ablation_callgrind/`) (workloads: phase_del_1k, phase_del_4m)

**Hygiene.**
- **Builds.** A (sha256 `bc6eb1d0…`) and D (`cc4c0139…`) were built 22:02:35–22:03:33 UTC under the bench lock. S is stock, `76f121a8…`.
- **Runs.** Run 1 22:03:33–22:04:28 UTC and run 2 22:04:28–22:05:22, each under the bench lock. 72 processes per run and none discarded. The worst foreign load over any process was 0.03 and 0.05 core-equivalents. Max load1 was 0.86 and 1.00 against a void limit of 12. The preflight read every event in its group.
- **Amendment timing.** The amendment above was committed at 22:03:34 UTC, one second after run 1's first process started. No timed data had been read when it was written.

**Controls** (amended above):
- **Default build:** the replacement control is MET.
- **Disassembly.** `map_remove::<false, false>` carries 92 `lock` instructions in D and 0 in A. `JudyLDel` goes from 21 to 0 and `JudyLIns` from 25 to 0. The `lock` instructions left in A are in the OCC and deferred instantiations (`<true, …>`) and the shared wrappers (`lock_summary.json`).
- **Which files.** The dumps and the Callgrind counts come from a build of the same commit, with the same rustc, on another host (A `469612fc…`, D `72acaf57…`). They are not the measured files.
- **Callgrind, `JudyLDel` inclusive** (valgrind 3.22.0):

  | Shape | Deletes | D | A | A/D |
  |---|---|---|---|---|
  | `del_1k` | 102,400 | 67,542,921 | 67,568,321 | 1.0004 |
  | `del_4m` | 4,194,304 | 3,076,256,264 | 3,082,552,070 | 1.0020 |

  The reference host's `instructions` ratio is the same, 1.0004 and 1.0020. A retires slightly more instructions than D and takes fewer cycles.

**Reading: MATTER at both shapes.** A takes fewer cycles than D in both runs. At 1 Ki A removes more than D's whole excess over stock, and at 4 Mi it removes 0.63 of it. The phase wall clock agrees on its side of 1 in every run.

| Shape | Metric | A/D run 1 | A/D run 2 | A/S run 1 | A/S run 2 |
|---|---|---|---|---|---|
| `del_1k` | `cycles` | 0.8558 [0.8373, 0.8883] | 0.8525 [0.8505, 0.8546] | 0.9411 [0.9283, 0.9904] | 0.9299 [0.9279, 0.9319] |
| `del_1k` | `phase_ns` | 0.8541 [0.8346, 0.8883] | 0.8504 [0.8465, 0.8534] | 0.9440 [0.9301, 0.9970] | 0.9282 [0.9243, 0.9322] |
| `del_4m` | `cycles` | 0.9389 [0.9253, 0.9498] | 0.9376 [0.9235, 0.9497] | 1.0401 [1.0247, 1.0537] | 1.0403 [1.0255, 1.0530] |
| `del_4m` | `phase_ns` | 0.9408 [0.9276, 0.9516] | 0.9391 [0.9254, 0.9513] | 1.0592 [1.0438, 1.0729] | 1.0594 [1.0442, 1.0723] |

| Shape | Run | D − S (cycles) | Share of it A removes |
|---|---|---|---|
| `del_1k` | 1 | 1.467e8 | 1.591 |
| `del_1k` | 2 | 1.336e8 | 1.772 |
| `del_4m` | 1 | 8.677e7 | 0.628 |
| `del_4m` | 2 | 8.826e7 | 0.632 |

- **Per delete.** At 1 Ki, D takes 158.0 cycles and A 135.2; stock takes 143.7. At 4 Mi, D takes 212.7, A 199.7 and stock 192.0.
- **Against stock.** At 1 Ki A is BELOW STOCK, 0.93–0.94× its cycles. At 4 Mi it stays ABOVE STOCK, at 1.04×.
- **Which stage moved.** Total stall cycles fall to 0.42–0.50× D's at 1 Ki and 0.84–0.85× at 4 Mi. The undelivered-uop slots move only at 4 Mi (0.66×). At 1 Ki they are 0.96–1.01×, and run 1's interval contains 1.
- **Scope.** This measures the `lock` prefixes' total cost on these two shapes, through one implementation that drops them. It does not show that this form is safe to ship. The counters are public (`NodeAlloc::live_allocs` and `total_allocs`, Python's `total_node_allocs`), and AGENTS.md §2.6 and §8.22 apply. Any shippable version is the owner's call.

## Step 0: the unlocked accounting, measured without code

**A zero-code measurement, pre-registered before it runs** (#1679). It decides the shape of the next promotion attempt.
- **Why not the split.** The attempt in #1718 keyed the lock on block size, and its Callgrind gate failed: 20 of 294 arms above +0.1%.
- **What it compares.** The form measured here is the existing diagnostic feature `ablation-unlocked-plain-acct`: every plain-path counter update unlocked, with no size test and no added code. It is compared with `main`.
- **Why it can now be the default.** Making `NodeAlloc` `Send` and not `Sync` is a separate soundness fix: no safe caller can then share one allocator across threads, so the locked updates protect nothing.

**Builds and instrument.** Callgrind on a host that is not the reference host, in the `cg968-noble` image (Ubuntu 24.04, glibc 2.39) with rustc 1.99.1, the CI toolchain. That combination reproduced CI run 43126937195 arm for arm.
- **Two builds of one commit** (`main` at this section's parent): M, default features; U, with `ablation-unlocked-plain-acct`.
- **Arms:** every arm of CI's instruction jobs:
  - `expanse-trie`'s `instructions`, `search_instructions`, `rmw_instructions` and `smoke_instructions`;
  - `expanse-capi`'s `smoke_instructions` and `vs_stock`, with stock libjudy from the image's `libjudy-dev`.
- **AArch64.** If a 64-bit Arm build is available, its disassembly records whether the locked updates compile to `__aarch64_ldadd8_relax` calls (outline atomics) or to LSE instructions, and what the unlocked form compiles to. That is a record, not a gate.

**Reading, over every arm, U against M:**
- **(i) Fully unlocked.** Every arm is within +0.1%. The next promotion unlocks every plain-path update.
- **(ii) Keyed on `OCC`.** Some arms exceed +0.1%, and for each of them the per-function delta (`callgrind_annotate --inclusive=no`, U against M) falls only in `<true, …>` instantiations or in the `Sync*` wrappers' code. The next promotion keys the unlock on the `OCC` const parameter: `OCC = false` unlocked, `OCC = true` locked token for token, so no runtime test is added.
- **(iii) Stop.** Any arm exceeds +0.1% with part of its delta in a `<false, …>` function or in a plain-tree caller. That function is attributed (per-line annotation, and `objdump` where its source did not change), and the next promotion waits for the owner.
- **Fixed in advance.** The bound is §6's review threshold and is not moved. A delta in a function of neither kind (`libc`, the bench harness) counts against (i) and is attributed under (iii).

### Step 0 result

(measured: Callgrind, rustc 1.99.1, valgrind 3.22.0, Ubuntu 24.04, on a host that is not the reference host; M and U built from `fc10d6e47` (code of `573b64d50`); artifacts `results/step0_unlocked/`)

**Reading: (iii).** Unlocking every plain-path update, with no added code, puts plain-tree arms above +0.1%. Per the pre-registration, the next promotion waits for the owner.

| Harness | Arms | Above +0.1% | Below −0.1% |
|---|---|---|---|
| `instructions` | 294 | 37 | 7 |
| `search_instructions` | 50 | 0 | 0 |
| `rmw_instructions` | 12 | 0 | 0 |
| `smoke_instructions` (`expanse-trie`) | 20 | 2 | 3 |
| `smoke_instructions` (`expanse-capi`) | 14 | 4 | 1 |
| `vs_stock` | 51 | 10 | 4 |

- **The losses are plain-tree arms, mostly the set.** The worst are `set_oscillate/random` +3.03%, `set_insert/clustered` +2.32%, `set_insert/sparse` +1.30%, and `set_insert` on the other key shapes +1.04% to +1.06%. The map loses less: `map_remove/random` +0.32%, `map_remove_revgen` +0.39%. Through the C ABI, `judy1_set_expanse/clustered` (`vs_stock`) is +2.13% and `judy1_set/clustered` (smoke) +2.19%.
- **No deferred arm moved.** All 89 `sync_*` arms are within −0.08% and +0.01%.
- **The string map gains.** The largest improvements are `strmap_churn/routes` −1.06% and `strmap_refill_small` −0.46%.
- **The counter sites cost nothing extra.** Each unlocked update compiles to one un-prefixed memory instruction (`sub %rax,0x200(%rcx)`, `decq 0x208(%rcx)`), the same count as `lock sub` and `lock decq` (`disasm/map_remove_false_false.{M,U}.s`).
- **Where the deltas sit:**
  - `set_oscillate/random`: exactly +2 instructions per operation, all on the bench loop's iterator lines. That loop never allocates. Its wrapper got statically smaller (1,404 → 1,377 instructions, and 291 → 274 `%rsp`-relative), and its registers were reassigned (the accumulator moved from `%r15` to `%r14`).
  - `set_insert/clustered`: +240,417 Ir on `mutate.rs` lines inlined into `insert_with_path_flat`, whose source did not change; its `%rsp`-relative instructions went from 555 to 574.
  - `map_remove/random`: +67,583 on `alloc.rs` lines and +44,378 on `ptr` lines, against −49,185 on `common.rs` lines.
- **What it means.** The `lock` prefixes cost nothing in instruction count. What moves the counts is the compiler's code generation in the callers that inline the allocation path, as in the split before it. This is measured as instruction deltas on lines and functions whose source did not change. The register-allocation cause is read from the disassembly, not measured.
- **AArch64:** not done in this run.

## Promotion, judged on timing

**The unlocked plain-tree accounting promoted to the default, judged on wall clock** (#1679, AGENTS.md §2.7 and §6). Pre-registered here before any timed run of the promoted build, as §8.8's second commit.
- **What is promoted.** The diagnostic `ablation-unlocked-plain-acct` form: every plain-path counter update in `alloc_raw_body!` and `free_raw` becomes a relaxed load and store. The inverse `ablation-locked-plain-acct` restores the locked updates, and the old name becomes a `compile_error!`.
- **Why it is now sound.** Since #1719, `NodeAlloc` is `Send` and not `Sync`, so no safe caller can reach one allocator from two threads.
- **Why wall clock decides.** The change removes stalls, not work: §6 makes wall clock primary for such a change. Its Callgrind costs are already known from Step 0. The owner accepted them as a written trade-off, so this section judges the promotion on timing.

### Callgrind: costs known before this registration

The arms below rose above +0.1% in Step 0 (`results/step0_unlocked/`, measured on code identical to this promotion's, against `573b64d50`; #1719 changed no arm). They are **known before this registration, accepted by the owner, and not predictions**.
- **Bound per accepted arm.** This PR's own CI run (`instruction-counts`, and the smoke and `vs_stock` jobs) must not exceed its Step 0 value by more than the 0.1% resolution.
- **Every other arm** stays within +0.1%, all `sync_*` arms included.
- **An arm over its bound** fails the registration: stop, report, and move no bound.

| Harness | Arm | Step 0 | Bound |
|---|---|---|---|
| instructions | `set_oscillate/random` | +3.025% | +3.125% |
| instructions | `set_insert/clustered` | +2.321% | +2.421% |
| instructions | `set_insert/sparse` | +1.300% | +1.400% |
| instructions | `set_insert/linear_leaf` | +1.060% | +1.160% |
| instructions | `set_insert/dense_leaf` | +1.053% | +1.153% |
| instructions | `set_insert/random` | +1.038% | +1.138% |
| instructions | `set_clear_refill/random` | +0.968% | +1.068% |
| instructions | `set_subtree_split/e1024` | +0.895% | +0.995% |
| instructions | `set_subtree_condense/h1` | +0.873% | +0.973% |
| instructions | `set_subtree_condense_control/h1` | +0.865% | +0.965% |
| instructions | `set_subtree_condense/wide` | +0.805% | +0.905% |
| instructions | `set_subtree_condense_control/wide` | +0.769% | +0.869% |
| instructions | `set_subtree_boundary_oscillate/band9` | +0.760% | +0.860% |
| instructions | `set_refill/random` | +0.757% | +0.857% |
| instructions | `set_remove/random` | +0.745% | +0.845% |
| instructions | `set_subtree_boundary_oscillate/band2` | +0.735% | +0.835% |
| instructions | `set_remove_partial/random60` | +0.711% | +0.811% |
| instructions | `set_compact_drained/random60` | +0.680% | +0.780% |
| instructions | `set_rebuild_drained/random60` | +0.663% | +0.763% |
| instructions | `map_compact_drained/random60` | +0.659% | +0.759% |
| instructions | `set_subtree_split_control/e1024` | +0.508% | +0.608% |
| instructions | `strmap_refill/routes` | +0.477% | +0.577% |
| instructions | `set_insert/sequential` | +0.452% | +0.552% |
| instructions | `map_remove_revgen/gen_256k` | +0.392% | +0.492% |
| instructions | `map_remove_revgen/gen_4m` | +0.391% | +0.491% |
| instructions | `map_remove/random` | +0.323% | +0.423% |
| instructions | `blobmap_insert_inline/random` | +0.295% | +0.395% |
| instructions | `map_rebuild_drained/random60` | +0.290% | +0.390% |
| instructions | `blobmap_remove/random` | +0.287% | +0.387% |
| instructions | `blobmap_insert/random` | +0.272% | +0.372% |
| instructions | `map_clone/random` | +0.242% | +0.342% |
| instructions | `map_remove_revgen/gen_1k` | +0.228% | +0.328% |
| instructions | `strmap_clear_refill/routes` | +0.210% | +0.310% |
| instructions | `map_regrow_after_shrink/random60` | +0.188% | +0.288% |
| instructions | `map_remove_partial/random60` | +0.179% | +0.279% |
| instructions | `blobmap_churn/random` | +0.124% | +0.224% |
| instructions | `map_small_tree/hashjudy` | +0.119% | +0.219% |
| smoke (trie) | `map_insert/random` | +0.207% | +0.307% |
| smoke (trie) | `map_ins_slot/random` | +0.206% | +0.306% |
| smoke (capi) | `judy1_set/clustered` | +2.187% | +2.287% |
| smoke (capi) | `judy1_set/random` | +0.674% | +0.774% |
| smoke (capi) | `judy1_set/sequential` | +0.408% | +0.508% |
| smoke (capi) | `judyl_insert/random` | +0.332% | +0.432% |
| vs_stock | `judy1_set_expanse/clustered` | +2.126% | +2.226% |
| vs_stock | `judy1_set_expanse/random` | +0.887% | +0.987% |
| vs_stock | `judyl_insert_expanse_dl/clustered` | +0.442% | +0.542% |
| vs_stock | `judyl_churn_expanse_dl/random` | +0.412% | +0.512% |
| vs_stock | `judysl_insert_expanse_dl/random` | +0.323% | +0.423% |
| vs_stock | `judyl_insert_expanse/random` | +0.296% | +0.396% |
| vs_stock | `judyl_churn_expanse/random` | +0.189% | +0.289% |
| vs_stock | `judyl_insert_expanse_dl/random` | +0.182% | +0.282% |
| vs_stock | `judysl_insert_expanse_dl/clustered` | +0.137% | +0.237% |
| vs_stock | `judyl_insert_expanse/clustered` | +0.101% | +0.201% |

### Wall clock

**Cells.** `scripts/phase_probe.c` drives each through the C API, so stock libjudy runs the same code.
- **Targets:** the two delete shapes, `del_1k` and `del_4m`.
- **No-loss cells:** the shapes of the worst instruction losers, added in `585c76b2c` before this registration:
  - `osc_1`: `set_oscillate`'s insert-then-remove of one key at a time, which never allocates;
  - `j1s_clustered` and `j1s_sparse`: set inserts on the Callgrind bench's own `clustered` and `sparse` keys. `j1s_clustered` is also `judy1_set` on clustered keys through the C API;
  - `ins_1k`: one insert shape at 1 Ki keys.

| field | `phase_osc_1` | `phase_j1s_clustered` | `phase_j1s_sparse` | `phase_ins_1k` |
|---|---|---|---|---|
| `workload_id` | `phase_osc_1` | `phase_j1s_clustered` | `phase_j1s_sparse` | `phase_ins_1k` |
| `group` | promote2 | promote2 | promote2 | promote2 |
| `population` | 50,000 keys, at most one present at a time | 50,000 | 50,000 | 1,024 |
| `insertion_order` | generator | generator | generator | generator |
| `probes_and_reuse` | 30 passes of 50,000 J1S+J1U pairs | 400 builds | 150 builds | 6,000 builds |
| `hit_rate` | 100% (each J1U finds its key) | n/a (inserts) | n/a (inserts) | n/a (inserts) |
| `miss_gen_method` | n/a | n/a | n/a | n/a |
| `value_dereference` | none (`Judy1`) | none (`Judy1`) | none (`Judy1`) | one store per JLI slot |
| `measured_region` | the pairs only | the inserts only; J1C and J1FA outside | as `j1s_clustered` | the inserts only; JLFA outside |
| `arm_symmetry` | P, M and S run the same binary, differing only in `LD_LIBRARY_PATH` | same | same | same |
| `statistics` | two-sample BCa 95%, 12 processes per arm per run, 2 runs | same | same | same |
| `verdict` | no-loss bound | no-loss bound | no-loss bound | no-loss bound |

**Arms and instrument** (`run.sh <run> ref phase_promote2`; the `promote2` set of `scripts/diag_phase_counters.py`):
- **Arms.** P (this PR's head) and M (its merge base on `main`) are built on the reference host under the bench lock by `scripts/build_layout_lib.sh`. S is stock.
- **Measurements.** The probe's own phase clock (`phase_ns`), plus one user-mode `cpu_core` group: `cycles`, `instructions`, `mem_inst_retired.lock_loads`, `cycle_activity.stalls_total` and `idq_uops_not_delivered.core`, three general-purpose events.
- **Rounds.** 12 rounds per run, cycling through the six orders of the three arms, one process per (round, shape, arm), and two runs. The P-core pin applies.
- **Load and rules.** Load windows carry the busy-CPU delta (§8.17). The discard and void rules are the phase-counter diagnostic's.

**Reading, P against M, in both runs** (`promote2_reading` in `scripts/diag_phase_counters_analyze.py`). It applies to the phase wall clock and to `cycles` alike:
- **Target.** `del_1k` and `del_4m` are FASTER: the upper bound is below 1.
- **No-loss bound.** On `osc_1`, `j1s_clustered`, `j1s_sparse` and `ins_1k`, the upper bound is at most **1.020**. That is a 2% tolerance, named here and never moved. It is the upper edge of this suite's SAME band, [0.98, 1.02], and code layout alone moves this instrument by about 1%: the phase-counter diagnostic's alignment-only builds read N64/N cycles of 0.9894–0.9982.
- **Verdict.** PASS only if every target and every bound holds in both runs. Otherwise FAIL: stop, report, and move no bound.
- **Counters, as expectations (projected).** `mem_inst_retired.lock_loads` per operation is 0 on P, whose plain paths run no locked instruction, and positive on M. They are reported, not gated.

**The `allow-regression:` line** is added to the pull request only after a PASS. It cites this section and the PR's own CI run, and names every losing arm.

### Promotion result

#### Wall clock: PASS

(measured: reference host Intel Core i9-12900F, pin `0,2,4,6,8,10,12,14`, perf 6.8.12, rustc 1.98.1; P built from `48a2fd692` (sha256 `3734ffc9…`), M from `c83d32bed` (`96204cd9…`), S stock (`76f121a8…`); harness `48a2fd692`; artifacts `results/diagnostic_promote2_ref_run{1,2}.json`) (workloads: phase_del_1k, phase_del_4m, phase_osc_1, phase_j1s_clustered, phase_j1s_sparse, phase_ins_1k)

**Hygiene.**
- **Builds and runs.** Builds ran 05:39:29–05:40:27 UTC under the bench lock. Run 1 ran 05:40:27–05:41:57 and run 2 05:41:57–05:43:28, each under the lock.
- **Processes.** 216 processes per run. The worst foreign load over any process was 0.06 and 0.07 core-equivalents, and max load1 was 0.97 and 1.06 against a void limit of 12.
- **Discards.** Each run discarded 12 processes, all of them stock's `osc_1` cell, and all because none had a load reading: stock runs that shape in less than the load window's 0.1 s minimum. S is context only, and no P or M process was discarded.

**Every target and every bound holds in both runs**, on the phase clock and on `cycles`. P/M, phase clock, BCa 95%:

| Shape | Role | Run 1 | Run 2 |
|---|---|---|---|
| `del_1k` | target | 0.8644 [0.8594, 0.8695] | 0.8502 [0.8230, 0.8666] |
| `del_4m` | target | 0.9616 [0.9496, 0.9737] | 0.9614 [0.9498, 0.9736] |
| `osc_1` | no-loss | 0.8736 [0.8642, 0.8775] | 0.8774 [0.8736, 0.8825] |
| `j1s_clustered` | no-loss | 0.9627 [0.9584, 0.9673] | 0.9559 [0.9510, 0.9639] |
| `j1s_sparse` | no-loss | 0.9450 [0.9330, 0.9603] | 0.9327 [0.9162, 0.9407] |
| `ins_1k` | no-loss | 0.7680 [0.7623, 0.7730] | 0.7679 [0.7634, 0.7717] |

- **Every shape is faster.** That includes the four whose Callgrind counts the change raises: their upper bounds are below 1, not just below the 1.020 tolerance. `cycles` agrees with the phase clock on every shape (`results/diagnostic_promote2_ref_analysis.txt`).
- **The `lock_loads` expectation was not met; it was reported, not gated.** P still executes locked loads on these paths. Per operation, run 1 medians:

  | Shape | P | M | S |
  |---|---|---|---|
  | `osc_1` | 10.0 | 15.0 | 0 |
  | `ins_1k` | 0.156 | 2.47 | 0.020 |
  | `del_1k` | 0.039 | 2.0 | 0.0003 |
  | `del_4m` | 0.0 | 0.79 | 0.0005 |

  Stock executes almost none, so P's remaining locked loads are Expanse code outside the accounting counters. Their source is not attributed.
- **Correction to two earlier records.** On `osc_1`, M − P is 5 locked loads per insert/remove pair: the three counter updates of one allocation plus the two of one free. So through the C API each pair allocates and frees a block. The Step 0 result and #1718's attribution state that the Rust bench's `set_oscillate` loop "never allocates". That was not verified and is unverified, not established. Their per-line Callgrind attributions are measurements and stand.

#### Callgrind: every arm within its bound

(measured: CI `Perf / Callgrind Deterministic Instructions` and `Perf / Callgrind Fast Smoke`, [run 43162806617](https://github.com/orieg/expanse/actions/runs/43162806617), head `48a2fd692` against `c83d32bed`, rustc 1.99.1; `vs_stock` as amended below; artifacts `results/promote2_callgrind/`)

- **The accepted arms.** All 53 are within their bounds, at their Step 0 values to within 0.002 points. Two examples: `set_oscillate/random` +3.025% (bound +3.125%), and `judy1_set/clustered` (smoke) +2.187% (bound +2.287%).
- **No other arm** is above +0.1% across the 288 distinct `instructions` arms, the 50 `search_instructions` arms, the 12 `rmw_instructions` arms and the 34 smoke arms.
- **The `sync_*` arms** are all within −0.080% and +0.011%.

**Amendment, made after the CI run and before the verdict** (AGENTS.md §8.19).
- **What could not be checked as written.** The registration names the PR's own `vs_stock` job as the instrument for the 10 `vs_stock` bounds. That job compares Expanse with stock and never with `main`, so it carries no number to check them against.
- **What checked them instead.** They were measured with Step 0's own instrument: the same container and rustc 1.99.1, which reproduced CI's verdict arm for arm. Base `c83d32bed` and head `48a2fd692` were each run once.
- **Result.** All 10 equal their Step 0 values to the third decimal and are within their bounds. `judy1_set_expanse/clustered`, for example, is +2.126% against a bound of +2.226%.
- **Unchanged.** No bound moved.

#### Verdict: PASS

Every pre-registered target and bound holds in both runs, on the phase clock and on `cycles`, and every Callgrind arm is within its bound. Merging is the owner's decision.

## The #1679 criterion on main

**A re-measure against stock, outside METHODOLOGY.md's pre-registration** (#1679). #1708, #1719 and #1720 have landed since the last vs-stock reading. This section re-runs the delete-fix diagnostic's cells against stock on main.

**Design, fixed before the builds** (`scripts/diag_delete.py` with `SNG_DELETE_SET=main`, `run.sh <run> <host> delete_main`):
- **Arms.** The same stress-ng binary and judy stressor, differing only in `LD_LIBRARY_PATH`:
  - N: libexpanse at `5f4892932`, #1720's merge commit, built by `scripts/build_layout_lib.sh` with no flag. No library source (`crates/expanse/src`, `crates/expanse-capi`) changes between it and this design's commit.
  - D: #1708's head library (`45f83ac5e`, sha256 `f6bb333e…`), the file the delete-fix diagnostic published.
  - S: stock.
- **Toolchain control.** Before N is built, D is rebuilt from its export with the same toolchain. It must reproduce `f6bb333e…` byte for byte, or the build stops.
- **Second host.** The second host runs the reference host's files, copied and hash-checked.
- **Cells and rounds.** Every `sng_judy_*` cell (1 Ki, 256 Ki and 4 Mi keys at W = 1, 2, 4, 8) and one process per arm per cell per round.
  - 12 rounds per run, cycling through the six orders of the three arms.
  - Two independent runs per host, under the host bench lock and that host's pin (METHODOLOGY.md §3).
  - The second host's governor change and stopped CI runner are the delete-fix diagnostic's.
  - Load windows, the discard rule and the void rule are the delete-fix diagnostic's.
- **Analysis.** `scripts/diag_delete_analyze.py` writes `results/diagnostic_delete_main_<host>_run{1,2}.json`: two-sample BCa 95% intervals on N/S, N/D and D/S for delete, insert, find, bogo-ops/s and wall clock.

**Reading rule:**
- **#1679's criterion, as the issue states it:** on the delete rate, N/S lower bound at or above 1.0 in both runs, at every cell and W, on both hosts.
- **Changed side:** every insert, find and delete cell whose N/S verdict (METHODOLOGY.md §8's rule) differs from the E/S verdict in the host's published table above, and every delete cell whose side of stock differs from the published D/S.
- **N/D** shows what changed since the delete-fix diagnostic. It is the combined effect of #1719 and #1720 and of any code placement between the two files, which this design cannot separate.

**Expected** (projected): N/D on delete at or above 1 at every size, since #1720's timed promotion measured P/M 0.85–0.96 on the delete phase's time per operation. Whether the 1 Ki and 4 Mi delete cells reach stock is not predicted.

### Main result (reference host)

(measured: reference host Intel Core i9-12900F, pin `0,2,4,6,8,10,12,14`, governor `powersave`, rustc 1.98.1, harness `5b7d578aa`; N sha256 `3734ffc9…`, D `f6bb333e…`, S `76f121a8…`, binary `490c0595…`; artifacts `results/diagnostic_delete_main_ref_run{1,2}.json`) (workloads: sng_judy_1k, sng_judy_256k, sng_judy_4m)

**Builds.** Under the bench lock, 10:19:47–10:20:45 UTC.
- **Control.** The rebuilt D reproduced `f6bb333e…` byte for byte, so the toolchain matches the delete-fix diagnostic's.
- **N's file.** It is the file #1720's timed promotion measured as P (`3734ffc9…`).

**Hygiene.**
- **Runs.** Run 1 ran 10:20:45–10:26:42 UTC and run 2 10:26:42–10:33:36, each under the bench lock, with 432 processes per run.
- **Discards.** Run 1 discarded 7 processes, all in round 3, with a worst foreign load of 1.02 core-equivalents. Run 2 discarded none.
- **Load.** Max load1 was 4.00 and 4.35, against a void limit of 12.

**Reading: #1679's criterion is NOT MET on this host.**
- **1 Ki and 256 Ki.** The delete rate is at or above stock at every W, in both runs.
- **4 Mi.** It stays below stock at every W.

| Cell | W | N/S delete run 1 | N/S delete run 2 |
|---|---|---|---|
| 1k | 1 | 1.035 [1.031, 1.038] | 1.037 [1.036, 1.039] |
| 1k | 2 | 1.041 [1.034, 1.046] | 1.040 [1.036, 1.044] |
| 1k | 4 | 1.033 [1.026, 1.038] | 1.033 [1.027, 1.039] |
| 1k | 8 | 1.035 [1.034, 1.037] | 1.036 [1.034, 1.037] |
| 256k | 1 | 1.150 [1.128, 1.172] | 1.172 [1.148, 1.189] |
| 256k | 2 | 1.164 [1.146, 1.182] | 1.162 [1.147, 1.177] |
| 256k | 4 | 1.177 [1.171, 1.183] | 1.181 [1.174, 1.187] |
| 256k | 8 | 1.188 [1.186, 1.190] | 1.188 [1.187, 1.190] |
| 4m | 1 | 0.939 [0.932, 0.948] | 0.939 [0.938, 0.941] |
| 4m | 2 | 0.943 [0.940, 0.947] | 0.944 [0.941, 0.947] |
| 4m | 4 | 0.931 [0.925, 0.937] | 0.927 [0.922, 0.933] |
| 4m | 8 | 0.962 [0.960, 0.966] | 0.961 [0.960, 0.962] |

**Changed side against the published tables.**
- **Against this host's E/S table** (`6ec29a656`, above), 14 of the 36 `judy` cell-metrics change verdict. Every other verdict holds.
  - Delete moves from SLOWER to FASTER at every 1 Ki and 256 Ki cell (8 cells). At 4 Mi it stays SLOWER, at 0.93–0.96 against 0.64–0.65.
  - Find at 1 Ki moves from SLOWER to EQUIVALENT at W = 1 and 2, and to FASTER at W = 4 and 8.
  - Find at 4 Mi moves from INCONCLUSIVE (W = 1) and EQUIVALENT (W = 2) to FASTER.
  - Insert keeps its side everywhere. It stays SLOWER at 4 Mi, at 0.95–0.99 against 0.88–0.89, and is 1.44–1.45× stock at 1 Ki.
- **Against the delete-fix diagnostic's D/S:** the four 1 Ki delete cells move from below stock to above it. 256 Ki stays above stock, and 4 Mi below.

**What changed since the delete-fix diagnostic (N/D, both runs).** This is the combined effect of #1719, #1720 and code placement, which this design does not separate.
- **Delete:** 1.13–1.14× at 1 Ki, 1.06–1.07× at 256 Ki and 1.05–1.06× at 4 Mi.
- **Insert:** 1.30–1.32× at 1 Ki, 1.09–1.10× at 256 Ki and 1.08–1.10× at 4 Mi.
- **Find:** within 1% at 1 Ki. At 256 Ki it is 0.96–0.98, slower than D but still 1.13–1.17× stock. At 4 Mi it is 0.997–1.10. Why find at 256 Ki moved is not measured.

### Main result (second host)

**Zen 5 pending: host busy with ExpanseDB CI.** Two pre-checks, at 10:34 and 10:53 UTC, found that host's CI job containers running. By the design's terms nothing runs beside them, so nothing was staged or changed there. The criterion therefore stands as measured on the reference host only, and #1679's two-host condition is not yet tested on main.

## The sparsematrix cells on main

**A re-measure against stock, outside METHODOLOGY.md's pre-registration** (#1680). The `sng_sm_judy_*` rows above measure `6ec29a656`. #1705 changed the linear-leaf search these gets reach after it, and #1708, #1719 and #1720 landed too. This re-runs those cells against stock on main, with "The #1679 criterion on main"'s design.

**Design, fixed before the run** (`scripts/diag_delete.py` with `SNG_DELETE_SET=sm_main`, `run.sh <run> <host> sm_main`):
- **Arms.** The same stress-ng binary and sparsematrix stressor, differing only in `LD_LIBRARY_PATH`:
  - N: libexpanse at `5f4892932` (sha256 `3734ffc9…`), the file "The #1679 criterion on main" measured. No library source (`crates/expanse/src`, `crates/expanse-capi`) changes between it and this design's commit.
  - E: the library of the published comparison, `6ec29a656` (`da5814a7…`), the reference host's `expcompat/lib`, hash-checked before the run.
  - S: stock (`76f121a8…`).
- **Cells and rounds.** Every `sng_sm_judy_*` cell of METHODOLOGY.md §5 (`default` and `big`, W = 1 and 8), one process per arm per cell per round, 12 rounds per run cycling the six orders of the three arms. Two independent runs under the host bench lock and that host's pin.
- **Second host.** The reference host's files, copied and hash-checked, on the delete-fix diagnostic's governor and runner terms.
- **Analysis.** `scripts/diag_delete_analyze.py` writes `results/diagnostic_sm_main_<host>_run{1,2}.json`: two-sample BCa 95% intervals on N/S, N/E and E/S for gets, puts, bogo-ops/s and wall clock. Load windows, the discard rule and the void rule are the delete-fix diagnostic's.

**Reading rule:**
- **#1680's gets criterion, as the issue states it ("parity or better with stock"):** at every `sng_sm_judy_*` cell, the N/S gets verdict (METHODOLOGY.md §8) is EQUIVALENT or FASTER, on both hosts.
- **Changed side:** every gets and puts cell whose N/S verdict differs from the published E/S verdict above.
- **N/E** is what changed since the published table. It combines #1705, #1708, #1719, #1720 and any code placement between the two files, which this design cannot separate.

**Expected** (projected): N/E on gets above 1, since the attribution diagnostic sampled fewer mispredicts per get on N than the phase counters counted on E. Whether the gets reach stock is not predicted.

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

The layout-only diagnostic builds its two libraries first, from `git archive` exports of the two commits, then runs with the mode `layout`:

```bash
docs/benchmarks/stressng_comparison/scripts/build_layout_lib.sh <export of 6ec29a656> /var/tmp/stressng-eval/expcompat-6ec29a656-fa64/lib "-C llvm-args=-align-all-functions=6"
```

```bash
docs/benchmarks/stressng_comparison/scripts/build_layout_lib.sh <export of 82eb56af> /var/tmp/stressng-eval/expcompat-82eb56af-fa64/lib "-C llvm-args=-align-all-functions=6"
```

```bash
docs/benchmarks/stressng_comparison/run.sh 1 ref layout
```

```bash
docs/benchmarks/stressng_comparison/run.sh 2 ref layout
```

```bash
python3 docs/benchmarks/stressng_comparison/scripts/diag_layout_analyze.py ref /var/tmp/stressng-eval/results/diag_layout_ref_run1 /var/tmp/stressng-eval/results/diag_layout_ref_run2
```

The phase-counter diagnostic needs `perf`, `cc` and the N64 library at `$SNG_D/expcompat-6ec29a656-fa64/lib` (the layout diagnostic's `scripts/build_layout_lib.sh`), and runs with the mode `phase`:

```bash
docs/benchmarks/stressng_comparison/run.sh 1 ref phase
```

```bash
docs/benchmarks/stressng_comparison/run.sh 2 ref phase
```

```bash
python3 docs/benchmarks/stressng_comparison/scripts/diag_phase_counters_analyze.py ref /var/tmp/stressng-eval/results/diag_phase_counters_ref_run1 /var/tmp/stressng-eval/results/diag_phase_counters_ref_run2
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

The delete-phase counters need #1708's library at `$SNG_D/expcompat-45f83ac5e/lib` (the delete-fix diagnostic's `scripts/build_layout_lib.sh` build), `perf` and `cc`:

```bash
docs/benchmarks/stressng_comparison/run.sh 1 ref phase_delete
```

```bash
docs/benchmarks/stressng_comparison/run.sh 2 ref phase_delete
```

```bash
python3 docs/benchmarks/stressng_comparison/scripts/diag_phase_counters_analyze.py ref /var/tmp/stressng-eval/results/diag_delete_phase_ref_run1 /var/tmp/stressng-eval/results/diag_delete_phase_ref_run2
```

The accounting ablation builds A and D from a `git archive` export of its pre-registration commit, then runs with the mode `phase_delete_acct`:

```bash
docs/benchmarks/stressng_comparison/scripts/build_layout_lib.sh <export> /var/tmp/stressng-eval/expcompat-1679acct-A/lib "" ablation-unlocked-plain-acct
```

```bash
docs/benchmarks/stressng_comparison/scripts/build_layout_lib.sh <export> /var/tmp/stressng-eval/expcompat-1679acct-D/lib
```

```bash
docs/benchmarks/stressng_comparison/run.sh 1 ref phase_delete_acct
```

```bash
docs/benchmarks/stressng_comparison/run.sh 2 ref phase_delete_acct
```

```bash
python3 docs/benchmarks/stressng_comparison/scripts/diag_phase_counters_analyze.py ref /var/tmp/stressng-eval/results/diag_delete_acct_ref_run1 /var/tmp/stressng-eval/results/diag_delete_acct_ref_run2
```

The timed accounting promotion builds P from an export of its head and M from an export of its merge base, then runs with the mode `phase_promote2`:

```bash
docs/benchmarks/stressng_comparison/scripts/build_layout_lib.sh <export of head> /var/tmp/stressng-eval/expcompat-1679p2-P/lib
```

```bash
docs/benchmarks/stressng_comparison/scripts/build_layout_lib.sh <export of merge base> /var/tmp/stressng-eval/expcompat-1679p2-M/lib
```

```bash
docs/benchmarks/stressng_comparison/run.sh 1 ref phase_promote2
```

```bash
docs/benchmarks/stressng_comparison/run.sh 2 ref phase_promote2
```

```bash
python3 docs/benchmarks/stressng_comparison/scripts/diag_phase_counters_analyze.py ref /var/tmp/stressng-eval/results/diag_promote2_ref_run1 /var/tmp/stressng-eval/results/diag_promote2_ref_run2
```

The #1679 criterion on main rebuilds D as a toolchain control and builds N, each from a `git archive` export, then runs with the mode `delete_main` on each host:

```bash
docs/benchmarks/stressng_comparison/scripts/build_layout_lib.sh <export of 45f83ac5e> /var/tmp/stressng-eval/expcompat-45f83ac5e-ctl/lib
```

```bash
docs/benchmarks/stressng_comparison/scripts/build_layout_lib.sh <export of 5f4892932> /var/tmp/stressng-eval/expcompat-5f4892932/lib
```

```bash
docs/benchmarks/stressng_comparison/run.sh 1 ref delete_main
```

```bash
docs/benchmarks/stressng_comparison/run.sh 2 ref delete_main
```

```bash
python3 docs/benchmarks/stressng_comparison/scripts/diag_delete_analyze.py ref /var/tmp/stressng-eval/results/diag_delete_main_ref_run1 /var/tmp/stressng-eval/results/diag_delete_main_ref_run2
```
