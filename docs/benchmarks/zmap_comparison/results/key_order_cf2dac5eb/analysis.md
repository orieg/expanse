## End to end

### e2e_run1: 10 admissible rounds of 10; void cells {}
foreign load core-eq: median 0.00, max 0.02

#### e1 (responses 524,288)
| metric | NONE | STOCK | EXPANSE | EXPANSE/STOCK |
|---|---|---|---|---|
| recv_ns_per_resp | 2629.1 [2618.0, 2660.6] (bca) | 2768.8 [2765.6, 2774.3] (bca) | 2774.9 [2769.8, 2786.3] (bca) | 1.0022 [0.9999, 1.0064] (bca) |
| proc_cpu_s | 4.033 [4.019, 4.053] (bca) | 4.117 [4.111, 4.132] (bca) | 4.135 [4.119, 4.160] (bca) | 1.0044 [1.0000, 1.0112] (bca) |
| maxrss_mb | 6.169 [6.112, 6.169] (bca) | 72.694 [72.600, 72.750] (bca) | 71.869 [71.775, 71.925] (bca) | 0.9887 [0.9871, 0.9899] (bca) |
| wall_s | 4.683 [4.670, 4.698] (bca) | 4.693 [4.688, 4.704] (bca) | 4.706 [4.692, 4.727] (bca) | 1.0028 [0.9994, 1.0081] (bca) |

dedup share of recv-thread CPU (STOCK): 5.0 [3.9, 5.5] (bca) %
dedup share of recv-thread CPU (EXPANSE): 5.3 [4.1, 5.7] (bca) %

#### e2 (responses 1,048,576)
| metric | NONE | STOCK | EXPANSE | EXPANSE/STOCK |
|---|---|---|---|---|
| recv_ns_per_resp | 2606.4 [2602.7, 2610.1] (bca) | 2691.9 [2687.3, 2699.4] (bca) | 2715.1 [2711.3, 2718.2] (bca) | 1.0086 [1.0058, 1.0107] (bca) |
| proc_cpu_s | 8.024 [8.002, 8.059] (bca) | 8.135 [8.110, 8.170] (bca) | 8.174 [8.151, 8.202] (bca) | 1.0048 [0.9999, 1.0094] (bca) |
| maxrss_mb | 6.188 [6.112, 6.225] (bca) | 72.675 [72.600, 72.713] (bca) | 71.869 [71.812, 71.963] (bca) | 0.9889 [0.9879, 0.9907] (bca) |
| wall_s | 7.323 [7.303, 7.353] (bca) | 7.341 [7.320, 7.374] (bca) | 7.354 [7.331, 7.378] (bca) | 1.0018 [0.9966, 1.0063] (bca) |

dedup share of recv-thread CPU (STOCK): 3.2 [3.0, 3.5] (bca) %
dedup share of recv-thread CPU (EXPANSE): 4.0 [3.8, 4.2] (bca) %

### e2e_run2: 10 admissible rounds of 10; void cells {}
foreign load core-eq: median 0.00, max 0.02

#### e1 (responses 524,288)
| metric | NONE | STOCK | EXPANSE | EXPANSE/STOCK |
|---|---|---|---|---|
| recv_ns_per_resp | 2622.4 [2618.3, 2625.5] (bca) | 2772.0 [2766.5, 2775.3] (bca) | 2774.7 [2770.3, 2782.3] (bca) | 1.0010 [0.9988, 1.0040] (bca) |
| proc_cpu_s | 4.023 [4.016, 4.040] (bca) | 4.144 [4.122, 4.177] (bca) | 4.136 [4.123, 4.154] (bca) | 0.9981 [0.9900, 1.0048] (bca) |
| maxrss_mb | 6.225 [6.188, 6.263] (bca) | 72.713 [72.619, 72.731] (bca) | 71.794 [71.719, 71.831] (bca) | 0.9874 [0.9863, 0.9884] (bca) |
| wall_s | 4.676 [4.667, 4.694] (bca) | 4.721 [4.700, 4.755] (bca) | 4.707 [4.693, 4.728] (bca) | 0.9970 [0.9899, 1.0030] (bca) |

dedup share of recv-thread CPU (STOCK): 5.4 [5.2, 5.6] (bca) %
dedup share of recv-thread CPU (EXPANSE): 5.5 [5.3, 5.8] (bca) %

#### e2 (responses 1,048,576)
| metric | NONE | STOCK | EXPANSE | EXPANSE/STOCK |
|---|---|---|---|---|
| recv_ns_per_resp | 2612.9 [2609.0, 2616.5] (bca) | 2700.0 [2692.7, 2718.9] (bca) | 2722.8 [2717.2, 2730.9] (bca) | 1.0084 [1.0015, 1.0121] (bca) |
| proc_cpu_s | 8.056 [8.034, 8.073] (bca) | 8.149 [8.129, 8.174] (bca) | 8.156 [8.126, 8.177] (bca) | 1.0009 [0.9962, 1.0044] (bca) |
| maxrss_mb | 6.206 [6.188, 6.244] (bca) | 72.750 [72.750, 72.750] (degenerate) | 71.869 [71.681, 71.925] (bca) | 0.9879 [0.9853, 0.9887] (bca) |
| wall_s | 7.347 [7.324, 7.362] (bca) | 7.346 [7.329, 7.377] (bca) | 7.331 [7.300, 7.355] (bca) | 0.9980 [0.9925, 1.0023] (bca) |

dedup share of recv-thread CPU (STOCK): 3.2 [2.9, 3.9] (bca) %
dedup share of recv-thread CPU (EXPANSE): 4.0 [3.8, 4.4] (bca) %

### E2E verdicts
H1 e1: OFF_HOT_PATH
H1 e2: OFF_HOT_PATH
H2 e1 maxrss_mb: EXPANSE_FASTER/SMALLER
H2 e1 proc_cpu_s: NO_DIFFERENCE_DETECTED
H2 e1 recv_ns_per_resp: NO_DIFFERENCE_DETECTED
H2 e1 wall_s: NO_DIFFERENCE_DETECTED
H2 e2 maxrss_mb: EXPANSE_FASTER/SMALLER
H2 e2 proc_cpu_s: NO_DIFFERENCE_DETECTED
H2 e2 recv_ns_per_resp: EXPANSE_SLOWER/LARGER
H2 e2 wall_s: NO_DIFFERENCE_DETECTED

## Microbenchmark

### micro_run1: 15 admissible rounds; void rounds []
| workload | STOCK ns/resp | EXPANSE ns/resp | ratio | STOCK RSS growth MB | EXPANSE RSS growth MB | ratio |
|---|---|---|---|---|---|---|
| m1 | 123.8 [123.5, 124.3] (bca) | 108.6 [108.5, 108.8] (bca) | 0.877 [0.874, 0.880] (bca) | 47.0 [47.0, 47.0] (bca) | 50.9 [50.8, 51.0] (bca) | 1.083 [1.081, 1.085] (bca) |
| m2 | 69.2 [69.2, 69.3] (bca) | 64.7 [64.6, 64.9] (bca) | 0.934 [0.932, 0.937] (bca) | 47.0 [47.0, 47.1] (bca) | 50.9 [50.9, 51.0] (bca) | 1.082 [1.081, 1.083] (bca) |
| m3 | 220.2 [220.0, 220.4] (bca) | 161.3 [161.2, 161.4] (bca) | 0.733 [0.732, 0.733] (bca) | 47.4 [47.4, 47.4] (degenerate) | 52.2 [52.2, 52.3] (bca) | 1.101 [1.100, 1.103] (bca) |

### micro_run2: 15 admissible rounds; void rounds []
| workload | STOCK ns/resp | EXPANSE ns/resp | ratio | STOCK RSS growth MB | EXPANSE RSS growth MB | ratio |
|---|---|---|---|---|---|---|
| m1 | 126.0 [123.8, 136.6] (bca) | 108.6 [108.4, 108.8] (bca) | 0.862 [0.797, 0.877] (bca) | 47.1 [47.0, 47.1] (bca) | 50.9 [50.9, 51.0] (bca) | 1.082 [1.081, 1.083] (bca) |
| m2 | 69.3 [69.2, 69.4] (bca) | 64.5 [64.5, 64.7] (bca) | 0.932 [0.929, 0.933] (bca) | 47.1 [47.0, 47.1] (bca) | 51.0 [50.9, 51.0] (bca) | 1.083 [1.081, 1.084] (bca) |
| m3 | 220.3 [220.1, 220.5] (bca) | 161.5 [161.3, 161.7] (bca) | 0.733 [0.732, 0.734] (bca) | 47.4 [47.3, 47.4] (bca) | 52.2 [52.1, 52.3] (bca) | 1.102 [1.100, 1.103] (bca) |

### Micro verdicts
H3 m1 ns: EXPANSE_FASTER/SMALLER
H3 m1 rss: EXPANSE_SLOWER/LARGER
H3 m2 ns: EXPANSE_FASTER/SMALLER
H3 m2 rss: EXPANSE_SLOWER/LARGER
H3 m3 ns: EXPANSE_FASTER/SMALLER
H3 m3 rss: EXPANSE_SLOWER/LARGER

## Key-order gates (EXPANSE = candidate, MAIN = comparison base)

### micro_run1: 15 admissible rounds; void rounds []
| workload | MAIN ns/resp | EXPANSE/MAIN ns | EXPANSE/STOCK ns | MAIN RSS growth MB | EXPANSE/MAIN RSS | EXPANSE/STOCK RSS |
|---|---|---|---|---|---|---|
| m1 | 106.6 [106.4, 106.8] (bca) | 1.019 [1.017, 1.021] (bca) | 0.877 [0.874, 0.880] (bca) | 56.4 [56.3, 56.4] (bca) | 0.903 [0.901, 0.904] (bca) | 1.083 [1.081, 1.085] (bca) |
| m2 | 60.6 [60.3, 61.8] (bca) | 1.067 [1.048, 1.073] (bca) | 0.934 [0.932, 0.937] (bca) | 56.4 [56.3, 56.4] (bca) | 0.903 [0.902, 0.904] (bca) | 1.082 [1.081, 1.083] (bca) |
| m3 | 164.0 [163.9, 164.2] (bca) | 0.983 [0.982, 0.985] (bca) | 0.733 [0.732, 0.733] (bca) | 57.1 [57.1, 57.2] (bca) | 0.914 [0.913, 0.915] (bca) | 1.101 [1.100, 1.103] (bca) |

### micro_run2: 15 admissible rounds; void rounds []
| workload | MAIN ns/resp | EXPANSE/MAIN ns | EXPANSE/STOCK ns | MAIN RSS growth MB | EXPANSE/MAIN RSS | EXPANSE/STOCK RSS |
|---|---|---|---|---|---|---|
| m1 | 106.7 [106.5, 106.8] (bca) | 1.018 [1.015, 1.021] (bca) | 0.862 [0.797, 0.877] (bca) | 56.4 [56.3, 56.4] (bca) | 0.903 [0.902, 0.904] (bca) | 1.082 [1.081, 1.083] (bca) |
| m2 | 60.7 [60.5, 61.2] (bca) | 1.064 [1.055, 1.068] (bca) | 0.932 [0.929, 0.933] (bca) | 56.4 [56.4, 56.5] (bca) | 0.903 [0.902, 0.904] (bca) | 1.083 [1.081, 1.084] (bca) |
| m3 | 164.2 [164.0, 164.4] (bca) | 0.983 [0.982, 0.985] (bca) | 0.733 [0.732, 0.734] (bca) | 57.2 [57.1, 57.2] (bca) | 0.914 [0.912, 0.915] (bca) | 1.102 [1.100, 1.103] (bca) |

### e2e_run1: 10 admissible rounds; void rounds []
| workload | EXPANSE/MAIN recv ns per response | EXPANSE/MAIN peak RSS | EXPANSE/MAIN process CPU | EXPANSE/MAIN wall |
|---|---|---|---|---|
| e1 | 0.9987 [0.9928, 1.0026] (bca) | 0.9943 [0.9922, 0.9961] (bca) | 0.9978 [0.9906, 1.0053] (bca) | 0.9983 [0.9924, 1.0040] (bca) |
| e2 | 1.0019 [1.0000, 1.0039] (bca) | 0.9938 [0.9922, 0.9956] (bca) | 0.9995 [0.9937, 1.0042] (bca) | 0.9985 [0.9918, 1.0034] (bca) |

### e2e_run2: 10 admissible rounds; void rounds []
| workload | EXPANSE/MAIN recv ns per response | EXPANSE/MAIN peak RSS | EXPANSE/MAIN process CPU | EXPANSE/MAIN wall |
|---|---|---|---|---|
| e1 | 1.0001 [0.9978, 1.0032] (bca) | 0.9935 [0.9917, 0.9951] (bca) | 0.9993 [0.9904, 1.0051] (bca) | 0.9985 [0.9911, 1.0038] (bca) |
| e2 | 1.0048 [1.0023, 1.0080] (bca) | 0.9943 [0.9920, 0.9961] (bca) | 0.9988 [0.9940, 1.0029] (bca) | 0.9971 [0.9916, 1.0018] (bca) |

### Key-order gate outcomes (this host)
K1 micro RSS growth EXPANSE/MAIN: MET (upper bound <= 0.95 required in each of 6 cells)
K2 micro ns per response EXPANSE/MAIN: NOT MET (upper bound <= 1.02 required in each of 6 cells)
K3 micro ns per response EXPANSE/STOCK: MET (upper bound < 1.0 required in each of 6 cells)
K5 e2e receive-thread ns per response EXPANSE/MAIN: MET (upper bound <= 1.01 required in each of 4 cells)
K5 e2e peak RSS EXPANSE/MAIN: MET (upper bound <= 1.005 required in each of 4 cells)
