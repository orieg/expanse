## End to end

### e2e_run1: 10 admissible rounds of 10; void cells {}
foreign load core-eq: median 0.01, max 0.06

#### e1 (responses 524,288)
| metric | NONE | STOCK | EXPANSE | EXPANSE/STOCK |
|---|---|---|---|---|
| recv_ns_per_resp | 2628.3 [2624.6, 2634.5] (bca) | 2780.1 [2775.6, 2785.1] (bca) | 2905.1 [2901.2, 2909.7] (bca) | 1.0450 [1.0428, 1.0472] (bca) |
| proc_cpu_s | 4.034 [4.024, 4.048] (bca) | 4.133 [4.122, 4.153] (bca) | 4.216 [4.193, 4.244] (bca) | 1.0201 [1.0138, 1.0283] (bca) |
| maxrss_mb | 6.131 [6.037, 6.169] (bca) | 72.694 [72.600, 72.731] (bca) | 122.287 [122.250, 122.325] (bca) | 1.6822 [1.6809, 1.6838] (bca) |
| wall_s | 4.686 [4.676, 4.699] (bca) | 4.703 [4.691, 4.727] (bca) | 4.724 [4.702, 4.755] (bca) | 1.0045 [0.9985, 1.0115] (bca) |

dedup share of recv-thread CPU (STOCK): 5.5 [5.2, 5.7] (bca) %
dedup share of recv-thread CPU (EXPANSE): 9.5 [9.3, 9.7] (bca) %

#### e2 (responses 1,048,576)
| metric | NONE | STOCK | EXPANSE | EXPANSE/STOCK |
|---|---|---|---|---|
| recv_ns_per_resp | 2617.9 [2613.4, 2623.6] (bca) | 2697.9 [2694.9, 2701.1] (bca) | 2775.7 [2772.4, 2779.9] (bca) | 1.0288 [1.0270, 1.0306] (bca) |
| proc_cpu_s | 8.023 [8.006, 8.041] (bca) | 8.172 [8.152, 8.195] (bca) | 8.219 [8.198, 8.239] (bca) | 1.0058 [1.0021, 1.0096] (bca) |
| maxrss_mb | 6.206 [6.131, 6.244] (bca) | 72.675 [72.600, 72.713] (bca) | 122.287 [122.175, 122.381] (bca) | 1.6827 [1.6807, 1.6846] (bca) |
| wall_s | 7.310 [7.296, 7.325] (bca) | 7.373 [7.352, 7.402] (bca) | 7.336 [7.317, 7.354] (bca) | 0.9950 [0.9905, 0.9989] (bca) |

dedup share of recv-thread CPU (STOCK): 3.0 [2.7, 3.2] (bca) %
dedup share of recv-thread CPU (EXPANSE): 5.7 [5.5, 5.9] (bca) %

### e2e_run2: 10 admissible rounds of 10; void cells {}
foreign load core-eq: median 0.01, max 0.08

#### e1 (responses 524,288)
| metric | NONE | STOCK | EXPANSE | EXPANSE/STOCK |
|---|---|---|---|---|
| recv_ns_per_resp | 2627.5 [2623.3, 2631.8] (bca) | 2772.0 [2770.7, 2773.5] (bca) | 2905.7 [2901.7, 2911.1] (bca) | 1.0482 [1.0466, 1.0502] (bca) |
| proc_cpu_s | 4.043 [4.026, 4.066] (bca) | 4.132 [4.117, 4.155] (bca) | 4.224 [4.206, 4.242] (bca) | 1.0223 [1.0156, 1.0282] (bca) |
| maxrss_mb | 6.169 [6.094, 6.188] (bca) | 72.731 [72.656, 72.769] (bca) | 122.325 [122.213, 122.400] (bca) | 1.6819 [1.6798, 1.6837] (bca) |
| wall_s | 4.693 [4.677, 4.713] (bca) | 4.708 [4.694, 4.729] (bca) | 4.731 [4.712, 4.751] (bca) | 1.0049 [0.9992, 1.0104] (bca) |

dedup share of recv-thread CPU (STOCK): 5.2 [5.1, 5.4] (bca) %
dedup share of recv-thread CPU (EXPANSE): 9.6 [9.4, 9.8] (bca) %

#### e2 (responses 1,048,576)
| metric | NONE | STOCK | EXPANSE | EXPANSE/STOCK |
|---|---|---|---|---|
| recv_ns_per_resp | 2608.0 [2604.8, 2611.5] (bca) | 2692.7 [2690.7, 2694.9] (bca) | 2778.4 [2774.7, 2782.2] (bca) | 1.0318 [1.0302, 1.0335] (bca) |
| proc_cpu_s | 8.023 [8.008, 8.046] (bca) | 8.136 [8.117, 8.157] (bca) | 8.240 [8.219, 8.263] (bca) | 1.0128 [1.0092, 1.0164] (bca) |
| maxrss_mb | 6.131 [6.056, 6.150] (bca) | 72.713 [72.638, 72.731] (bca) | 122.306 [122.213, 122.362] (bca) | 1.6821 [1.6806, 1.6835] (bca) |
| wall_s | 7.317 [7.300, 7.339] (bca) | 7.343 [7.325, 7.363] (bca) | 7.356 [7.333, 7.374] (bca) | 1.0018 [0.9978, 1.0055] (bca) |

dedup share of recv-thread CPU (STOCK): 3.1 [3.0, 3.3] (bca) %
dedup share of recv-thread CPU (EXPANSE): 6.1 [6.0, 6.3] (bca) %

### E2E verdicts
H1 e1: OFF_HOT_PATH
H1 e2: OFF_HOT_PATH
H2 e1 maxrss_mb: EXPANSE_SLOWER/LARGER
H2 e1 proc_cpu_s: EXPANSE_SLOWER/LARGER
H2 e1 recv_ns_per_resp: EXPANSE_SLOWER/LARGER
H2 e1 wall_s: NO_DIFFERENCE_DETECTED
H2 e2 maxrss_mb: EXPANSE_SLOWER/LARGER
H2 e2 proc_cpu_s: EXPANSE_SLOWER/LARGER
H2 e2 recv_ns_per_resp: EXPANSE_SLOWER/LARGER
H2 e2 wall_s: NO_DIFFERENCE_DETECTED

## Microbenchmark

### micro_run1: 15 admissible rounds; void rounds []
| workload | STOCK ns/resp | EXPANSE ns/resp | ratio | STOCK RSS growth MB | EXPANSE RSS growth MB | ratio |
|---|---|---|---|---|---|---|
| m1 | 123.6 [123.3, 124.1] (bca) | 194.0 [193.7, 194.7] (bca) | 1.570 [1.564, 1.576] (bca) | 47.1 [47.1, 47.1] (bca) | 148.2 [148.1, 148.3] (bca) | 3.147 [3.144, 3.149] (bca) |
| m2 | 69.1 [69.0, 69.3] (bca) | 115.2 [115.0, 115.3] (bca) | 1.666 [1.663, 1.669] (bca) | 47.1 [47.0, 47.1] (bca) | 148.2 [148.1, 148.3] (bca) | 3.149 [3.145, 3.153] (bca) |
| m3 | 220.1 [219.9, 220.3] (bca) | 296.1 [295.9, 296.4] (bca) | 1.346 [1.344, 1.347] (bca) | 47.4 [47.3, 47.4] (bca) | 148.9 [148.8, 149.0] (bca) | 3.142 [3.139, 3.145] (bca) |

### micro_run2: 15 admissible rounds; void rounds []
| workload | STOCK ns/resp | EXPANSE ns/resp | ratio | STOCK RSS growth MB | EXPANSE RSS growth MB | ratio |
|---|---|---|---|---|---|---|
| m1 | 123.2 [123.1, 123.5] (bca) | 194.6 [194.0, 195.9] (bca) | 1.579 [1.574, 1.589] (bca) | 47.1 [47.1, 47.2] (bca) | 148.2 [148.1, 148.3] (bca) | 3.144 [3.140, 3.147] (bca) |
| m2 | 69.1 [69.1, 69.3] (bca) | 115.3 [115.1, 115.5] (bca) | 1.667 [1.664, 1.671] (bca) | 47.1 [47.0, 47.1] (bca) | 148.1 [148.0, 148.2] (bca) | 3.145 [3.141, 3.150] (bca) |
| m3 | 220.1 [220.0, 220.3] (bca) | 296.2 [295.8, 297.7] (bca) | 1.346 [1.343, 1.351] (bca) | 47.4 [47.4, 47.4] (degenerate) | 148.9 [148.8, 149.0] (bca) | 3.139 [3.137, 3.141] (bca) |

### Micro verdicts
H3 m1 ns: EXPANSE_SLOWER/LARGER
H3 m1 rss: EXPANSE_SLOWER/LARGER
H3 m2 ns: EXPANSE_SLOWER/LARGER
H3 m2 rss: EXPANSE_SLOWER/LARGER
H3 m3 ns: EXPANSE_SLOWER/LARGER
H3 m3 rss: EXPANSE_SLOWER/LARGER
