## End to end

### e2e_run1: 10 admissible rounds of 10; void cells {}
foreign load core-eq: median 0.00, max 0.02

#### e1 (responses 524,288)
| metric | NONE | STOCK | EXPANSE | EXPANSE/STOCK |
|---|---|---|---|---|
| recv_ns_per_resp | 2626.1 [2621.3, 2629.0] (bca) | 2774.7 [2771.6, 2777.6] (bca) | 2792.9 [2788.7, 2797.6] (bca) | 1.0066 [1.0048, 1.0086] (bca) |
| proc_cpu_s | 4.028 [4.020, 4.041] (bca) | 4.157 [4.136, 4.187] (bca) | 4.137 [4.123, 4.154] (bca) | 0.9952 [0.9873, 1.0012] (bca) |
| maxrss_mb | 6.188 [6.094, 6.244] (bca) | 72.694 [72.600, 72.731] (bca) | 72.394 [72.263, 72.469] (bca) | 0.9959 [0.9943, 0.9972] (bca) |
| wall_s | 4.680 [4.672, 4.689] (bca) | 4.732 [4.711, 4.758] (bca) | 4.702 [4.689, 4.716] (bca) | 0.9937 [0.9876, 0.9985] (bca) |

dedup share of recv-thread CPU (STOCK): 5.4 [5.2, 5.5] (bca) %
dedup share of recv-thread CPU (EXPANSE): 6.0 [5.8, 6.2] (bca) %

#### e2 (responses 1,048,576)
| metric | NONE | STOCK | EXPANSE | EXPANSE/STOCK |
|---|---|---|---|---|
| recv_ns_per_resp | 2613.7 [2611.0, 2619.0] (bca) | 2710.9 [2701.5, 2723.2] (bca) | 2714.1 [2710.4, 2721.0] (bca) | 1.0012 [0.9966, 1.0053] (bca) |
| proc_cpu_s | 8.049 [8.021, 8.086] (bca) | 8.164 [8.144, 8.186] (bca) | 8.142 [8.130, 8.155] (bca) | 0.9973 [0.9941, 1.0001] (bca) |
| maxrss_mb | 6.188 [6.112, 6.225] (bca) | 72.694 [72.600, 72.731] (bca) | 72.487 [72.338, 72.581] (bca) | 0.9972 [0.9951, 0.9987] (bca) |
| wall_s | 7.338 [7.312, 7.373] (bca) | 7.351 [7.336, 7.366] (bca) | 7.326 [7.315, 7.338] (bca) | 0.9966 [0.9940, 0.9993] (bca) |

dedup share of recv-thread CPU (STOCK): 3.6 [3.2, 4.0] (bca) %
dedup share of recv-thread CPU (EXPANSE): 3.7 [3.5, 3.9] (bca) %

### e2e_run2: 10 admissible rounds of 10; void cells {}
foreign load core-eq: median 0.01, max 0.02

#### e1 (responses 524,288)
| metric | NONE | STOCK | EXPANSE | EXPANSE/STOCK |
|---|---|---|---|---|
| recv_ns_per_resp | 2628.1 [2621.6, 2637.1] (bca) | 2768.6 [2765.0, 2770.4] (bca) | 2794.4 [2790.6, 2800.6] (bca) | 1.0093 [1.0078, 1.0118] (bca) |
| proc_cpu_s | 4.056 [4.031, 4.091] (bca) | 4.130 [4.117, 4.153] (bca) | 4.138 [4.123, 4.155] (bca) | 1.0019 [0.9954, 1.0070] (bca) |
| maxrss_mb | 6.150 [6.075, 6.206] (bca) | 72.694 [72.600, 72.750] (bca) | 72.431 [72.338, 72.525] (bca) | 0.9964 [0.9948, 0.9982] (bca) |
| wall_s | 4.705 [4.681, 4.735] (bca) | 4.705 [4.693, 4.726] (bca) | 4.703 [4.689, 4.720] (bca) | 0.9996 [0.9945, 1.0043] (bca) |

dedup share of recv-thread CPU (STOCK): 5.1 [4.7, 5.3] (bca) %
dedup share of recv-thread CPU (EXPANSE): 6.0 [5.6, 6.2] (bca) %

#### e2 (responses 1,048,576)
| metric | NONE | STOCK | EXPANSE | EXPANSE/STOCK |
|---|---|---|---|---|
| recv_ns_per_resp | 2607.9 [2604.4, 2611.8] (bca) | 2688.9 [2686.4, 2691.7] (bca) | 2714.9 [2712.4, 2718.7] (bca) | 1.0097 [1.0082, 1.0113] (bca) |
| proc_cpu_s | 8.023 [7.998, 8.046] (bca) | 8.148 [8.123, 8.167] (bca) | 8.171 [8.152, 8.188] (bca) | 1.0028 [0.9995, 1.0065] (bca) |
| maxrss_mb | 6.206 [6.131, 6.244] (bca) | 72.675 [72.600, 72.713] (bca) | 72.394 [72.319, 72.431] (bca) | 0.9961 [0.9951, 0.9972] (bca) |
| wall_s | 7.318 [7.295, 7.341] (bca) | 7.356 [7.329, 7.377] (bca) | 7.349 [7.331, 7.366] (bca) | 0.9990 [0.9951, 1.0034] (bca) |

dedup share of recv-thread CPU (STOCK): 3.0 [2.8, 3.2] (bca) %
dedup share of recv-thread CPU (EXPANSE): 3.9 [3.8, 4.1] (bca) %

### E2E verdicts
H1 e1: OFF_HOT_PATH
H1 e2: OFF_HOT_PATH
H2 e1 maxrss_mb: EXPANSE_FASTER/SMALLER
H2 e1 proc_cpu_s: NO_DIFFERENCE_DETECTED
H2 e1 recv_ns_per_resp: EXPANSE_SLOWER/LARGER
H2 e1 wall_s: NO_DIFFERENCE_DETECTED
H2 e2 maxrss_mb: EXPANSE_FASTER/SMALLER
H2 e2 proc_cpu_s: NO_DIFFERENCE_DETECTED
H2 e2 recv_ns_per_resp: NO_DIFFERENCE_DETECTED
H2 e2 wall_s: NO_DIFFERENCE_DETECTED

## Microbenchmark

### micro_run1: 15 admissible rounds; void rounds []
| workload | STOCK ns/resp | EXPANSE ns/resp | ratio | STOCK RSS growth MB | EXPANSE RSS growth MB | ratio |
|---|---|---|---|---|---|---|
| m1 | 123.7 [123.5, 123.9] (bca) | 118.7 [118.6, 118.9] (bca) | 0.960 [0.958, 0.962] (bca) | 47.1 [47.1, 47.1] (bca) | 56.5 [56.5, 56.6] (bca) | 1.200 [1.198, 1.201] (bca) |
| m2 | 69.2 [69.1, 69.3] (bca) | 68.0 [67.9, 68.1] (bca) | 0.982 [0.981, 0.985] (bca) | 47.1 [47.0, 47.1] (bca) | 56.6 [56.5, 56.6] (bca) | 1.202 [1.200, 1.204] (bca) |
| m3 | 221.6 [220.0, 229.2] (bca) | 181.7 [181.6, 181.9] (bca) | 0.820 [0.798, 0.826] (bca) | 47.4 [47.3, 47.4] (bca) | 57.3 [57.2, 57.3] (bca) | 1.209 [1.208, 1.210] (bca) |

### micro_run2: 15 admissible rounds; void rounds []
| workload | STOCK ns/resp | EXPANSE ns/resp | ratio | STOCK RSS growth MB | EXPANSE RSS growth MB | ratio |
|---|---|---|---|---|---|---|
| m1 | 123.6 [123.3, 124.5] (bca) | 118.6 [118.4, 119.0] (bca) | 0.960 [0.953, 0.963] (bca) | 47.1 [47.1, 47.2] (bca) | 56.5 [56.5, 56.5] (bca) | 1.199 [1.198, 1.201] (bca) |
| m2 | 69.1 [69.0, 69.2] (bca) | 68.0 [68.0, 68.1] (bca) | 0.984 [0.982, 0.986] (bca) | 47.1 [47.0, 47.1] (bca) | 56.6 [56.5, 56.6] (bca) | 1.201 [1.200, 1.203] (bca) |
| m3 | 220.4 [220.1, 220.8] (bca) | 181.7 [181.5, 182.0] (bca) | 0.825 [0.823, 0.826] (bca) | 47.4 [47.3, 47.4] (bca) | 57.3 [57.2, 57.3] (bca) | 1.208 [1.207, 1.210] (bca) |

### Micro verdicts
H3 m1 ns: EXPANSE_FASTER/SMALLER
H3 m1 rss: EXPANSE_SLOWER/LARGER
H3 m2 ns: EXPANSE_FASTER/SMALLER
H3 m2 rss: EXPANSE_SLOWER/LARGER
H3 m3 ns: EXPANSE_FASTER/SMALLER
H3 m3 rss: EXPANSE_SLOWER/LARGER
