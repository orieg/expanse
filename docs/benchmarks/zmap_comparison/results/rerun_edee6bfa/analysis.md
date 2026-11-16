## End to end

### e2e_run1: 10 admissible rounds of 10; void cells {}
foreign load core-eq: median 0.00, max 0.02

#### e1 (responses 524,288)
| metric | NONE | STOCK | EXPANSE | EXPANSE/STOCK |
|---|---|---|---|---|
| recv_ns_per_resp | 2627.9 [2624.4, 2632.0] (bca) | 2775.3 [2771.9, 2779.3] (bca) | 2782.7 [2775.3, 2801.6] (bca) | 1.0026 [0.9996, 1.0093] (bca) |
| proc_cpu_s | 4.028 [4.017, 4.040] (bca) | 4.125 [4.092, 4.138] (bca) | 4.144 [4.128, 4.159] (bca) | 1.0046 [0.9993, 1.0117] (bca) |
| maxrss_mb | 6.188 [6.112, 6.225] (bca) | 72.750 [72.675, 72.787] (bca) | 72.300 [72.188, 72.394] (bca) | 0.9938 [0.9920, 0.9954] (bca) |
| wall_s | 4.680 [4.670, 4.693] (bca) | 4.699 [4.671, 4.713] (bca) | 4.714 [4.699, 4.726] (bca) | 1.0032 [0.9987, 1.0088] (bca) |

dedup share of recv-thread CPU (STOCK): 5.3 [5.1, 5.5] (bca) %
dedup share of recv-thread CPU (EXPANSE): 5.6 [5.3, 6.2] (bca) %

#### e2 (responses 1,048,576)
| metric | NONE | STOCK | EXPANSE | EXPANSE/STOCK |
|---|---|---|---|---|
| recv_ns_per_resp | 2611.4 [2607.2, 2617.7] (bca) | 2695.0 [2693.4, 2696.4] (bca) | 2707.1 [2703.1, 2712.4] (bca) | 1.0045 [1.0028, 1.0065] (bca) |
| proc_cpu_s | 8.054 [8.043, 8.068] (bca) | 8.140 [8.125, 8.152] (bca) | 8.147 [8.113, 8.168] (bca) | 1.0009 [0.9968, 1.0041] (bca) |
| maxrss_mb | 6.225 [6.188, 6.263] (bca) | 72.656 [72.581, 72.694] (bca) | 72.150 [72.056, 72.263] (bca) | 0.9930 [0.9915, 0.9951] (bca) |
| wall_s | 7.345 [7.337, 7.357] (bca) | 7.346 [7.330, 7.363] (bca) | 7.334 [7.301, 7.356] (bca) | 0.9984 [0.9940, 1.0023] (bca) |

dedup share of recv-thread CPU (STOCK): 3.1 [2.9, 3.3] (bca) %
dedup share of recv-thread CPU (EXPANSE): 3.5 [3.3, 3.8] (bca) %

### e2e_run2: 10 admissible rounds of 10; void cells {}
foreign load core-eq: median 0.00, max 0.02

#### e1 (responses 524,288)
| metric | NONE | STOCK | EXPANSE | EXPANSE/STOCK |
|---|---|---|---|---|
| recv_ns_per_resp | 2632.4 [2625.1, 2650.1] (bca) | 2774.1 [2770.7, 2777.3] (bca) | 2779.0 [2774.0, 2784.5] (bca) | 1.0018 [0.9996, 1.0041] (bca) |
| proc_cpu_s | 4.041 [4.027, 4.058] (bca) | 4.121 [4.112, 4.135] (bca) | 4.130 [4.103, 4.151] (bca) | 1.0022 [0.9952, 1.0085] (bca) |
| maxrss_mb | 6.150 [6.056, 6.169] (bca) | 72.694 [72.600, 72.731] (bca) | 72.188 [72.037, 72.263] (bca) | 0.9930 [0.9912, 0.9946] (bca) |
| wall_s | 4.693 [4.680, 4.706] (bca) | 4.698 [4.688, 4.710] (bca) | 4.704 [4.677, 4.724] (bca) | 1.0013 [0.9951, 1.0066] (bca) |

dedup share of recv-thread CPU (STOCK): 5.1 [4.5, 5.4] (bca) %
dedup share of recv-thread CPU (EXPANSE): 5.3 [4.7, 5.6] (bca) %

#### e2 (responses 1,048,576)
| metric | NONE | STOCK | EXPANSE | EXPANSE/STOCK |
|---|---|---|---|---|
| recv_ns_per_resp | 2611.1 [2607.3, 2616.8] (bca) | 2694.2 [2692.4, 2695.5] (bca) | 2711.2 [2701.2, 2739.1] (bca) | 1.0063 [1.0026, 1.0166] (bca) |
| proc_cpu_s | 8.037 [8.012, 8.064] (bca) | 8.142 [8.121, 8.162] (bca) | 8.153 [8.128, 8.198] (bca) | 1.0014 [0.9974, 1.0076] (bca) |
| maxrss_mb | 6.188 [6.112, 6.225] (bca) | 72.638 [72.581, 72.675] (bca) | 72.431 [72.300, 72.487] (bca) | 0.9972 [0.9956, 0.9985] (bca) |
| wall_s | 7.329 [7.307, 7.352] (bca) | 7.341 [7.320, 7.360] (bca) | 7.344 [7.324, 7.367] (bca) | 1.0004 [0.9966, 1.0044] (bca) |

dedup share of recv-thread CPU (STOCK): 3.1 [2.9, 3.2] (bca) %
dedup share of recv-thread CPU (EXPANSE): 3.7 [3.3, 4.8] (bca) %

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
| m1 | 123.8 [123.6, 124.3] (bca) | 108.0 [107.8, 108.1] (bca) | 0.872 [0.869, 0.874] (bca) | 47.1 [47.0, 47.1] (bca) | 56.4 [56.3, 56.4] (bca) | 1.198 [1.196, 1.199] (bca) |
| m2 | 69.2 [69.1, 69.3] (bca) | 61.1 [61.0, 61.3] (bca) | 0.884 [0.882, 0.886] (bca) | 47.1 [47.0, 47.1] (bca) | 56.4 [56.3, 56.4] (bca) | 1.198 [1.196, 1.199] (bca) |
| m3 | 220.3 [220.0, 220.6] (bca) | 167.3 [167.0, 167.6] (bca) | 0.759 [0.758, 0.761] (bca) | 47.4 [47.4, 47.4] (bca) | 57.1 [57.1, 57.2] (bca) | 1.205 [1.204, 1.206] (bca) |

### micro_run2: 15 admissible rounds; void rounds []
| workload | STOCK ns/resp | EXPANSE ns/resp | ratio | STOCK RSS growth MB | EXPANSE RSS growth MB | ratio |
|---|---|---|---|---|---|---|
| m1 | 123.6 [123.3, 124.3] (bca) | 107.7 [107.5, 107.9] (bca) | 0.871 [0.867, 0.874] (bca) | 47.1 [47.1, 47.2] (bca) | 56.4 [56.3, 56.4] (bca) | 1.196 [1.194, 1.198] (bca) |
| m2 | 69.0 [68.9, 69.1] (bca) | 61.0 [60.9, 61.2] (bca) | 0.884 [0.882, 0.886] (bca) | 47.1 [47.0, 47.1] (bca) | 56.4 [56.3, 56.4] (bca) | 1.197 [1.196, 1.199] (bca) |
| m3 | 221.6 [220.1, 229.1] (bca) | 167.1 [166.9, 167.3] (bca) | 0.754 [0.729, 0.759] (bca) | 47.4 [47.3, 47.4] (bca) | 57.2 [57.1, 57.2] (bca) | 1.206 [1.205, 1.207] (bca) |

### Micro verdicts
H3 m1 ns: EXPANSE_FASTER/SMALLER
H3 m1 rss: EXPANSE_SLOWER/LARGER
H3 m2 ns: EXPANSE_FASTER/SMALLER
H3 m2 rss: EXPANSE_SLOWER/LARGER
H3 m3 ns: EXPANSE_FASTER/SMALLER
H3 m3 rss: EXPANSE_SLOWER/LARGER
