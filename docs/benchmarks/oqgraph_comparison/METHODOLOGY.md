# OQGraph × Expanse — pre-registration (locked before any profile or verdict data)

Written 2026-11-09 (host UTC ~07:50), before the Callgrind profile and before any timed
run on the reference host. Data seen before locking, disclosed: (a) correctness traces
and SQL result hashes; (b) one sizing pilot on the build host (Ryzen 9 9955HX, Docker,
`--cpus=8`, 1 round, arms A and C, K=4), used only to choose N and K. The pilot printed
per-cell medians for A and C. Most of C's block scrolled off before it was read. The
pilot is not used for any verdict.

## Code under test
- MariaDB/server `8e2919178136de95fb0b45dc979ff4426aa9221f` (main, 2026-11-09), RelWithDebInfo,
  flags in `cmake-common-flags.txt`; one server build shared by every arm; only `ha_oqgraph.so` differs.
- Expanse `82eb56af1a8a9512d9ab1b3b335a6d9c12756e23` (origin/main), `cargo build --release -p expanse-capi`,
  rustc 1.98.1, baseline x86-64 (no `target-cpu`, no glibc-hwcaps variant).
- Stock: Ubuntu 24.04 `libjudy-dev`/`libjudydebian1` 1.0.5-5.1build1 (`libJudy.so.1.0.3`).

## Arms
| id | plugin build | library mapped at run time (asserted from /proc/pid/maps every server start) |
|---|---|---|
| A  | stock `Judy.h`, `-lJudy` | `/usr/lib/x86_64-linux-gnu/libJudy.so.1.0.3` |
| B  | Expanse `Judy.h` from /usr/local/include, unchanged OQGraph source, no CMake flags | `libexpanse.so` (via `libJudy.so` link) |
| C  | native patch `oqgraph-expanse-native.patch` (expanse.h) | `libexpanse.so` |
| B1 | A's plugin binary, `LD_LIBRARY_PATH` → `libJudy.so.1 -> libexpanse.so` (the libjudy-compat deployment) | `libexpanse.so` |

## Hypotheses
- **H1 (profile, deterministic).** In arm A the bitset — self Ir of every function in
  `libJudy.so*` plus `open_query::judy_bitset::*` — is **< 5 %** of the Ir of the timed
  queries, in every profiled cell. Instrument: Callgrind, instrumentation toggled on for
  the measured queries only (warm-up excluded). If H1 holds, the Amdahl bound
  (`bounds/amdahl.py`, tested) caps the possible gain of any replacement at that share.
- **H2 (profile, deterministic).** Total Ir of the same queries in arm C vs arm A: reported
  as a ratio per cell, no gate (descriptive; explains H3).
- **H3 (wall clock, primary).** For each arm X ∈ {B, C, B1} and each cell, the paired
  ratio T_X / T_A lies within the equivalence margin **±3 %**.

## Cells (workload IDs)
Shapes (`gen_graph.py`, seed 1, ~4 directed edges/vertex): `er` (uniform random), `pl`
(Chung–Lu γ=2.2), `grid` (√N×√N lattice, both directions), `er_sid` (`er` with ids scattered
over 63 bits). N ∈ {10 000, 100 000}. Backing tables InnoDB, PK(origid,destid) + KEY(destid),
`ANALYZE TABLE` after load. Workloads (SQL `SELECT COUNT(*) FROM g WHERE …`):

| workload_id | latch / predicate | N |
|---|---|---|
| `oq_bfs_from` | breadth_first, origid | 10k, 100k |
| `oq_dijkstra_from` | dijkstras, origid | 10k, 100k (not er_sid) |
| `oq_leaves_from` | leaves, origid | 10k, 100k |
| `oq_bfs_path` | breadth_first, origid+destid (dest reachable, uniform) | 10k, 100k |
| `oq_dijkstra_path` | dijkstras, origid+destid | 10k, 100k (not er_sid) |
| `oq_bfs_to` | breadth_first, destid (reverse graph) | 10k only |

Dijkstra on `er_sid` is excluded in every arm: it aborts `mariadbd` in the stock arm too
(boost index-in-heap `vector_property_map` resized to the vertex id → `std::length_error`;
probed, all arms identical). 40 cells. K = 16 query endpoints per cell at 10k, 8 at 100k
(fixed lists from the generator).

## Workload shape (per AGENTS.md §8.12 fields)
- population: the graph sizes above; insertion_order: generator (sorted edge list, LOAD DATA).
- probes_and_reuse: K fixed endpoints per cell, same list in every arm and round.
- hit_rate: n/a (graph traversal); path queries use reachable destinations.
- value_dereference: COUNT(*) over the full OQGraph result set (all rows produced by the engine).
- measured_region: client wall clock around `execute`+`fetchone` per query, after one untimed
  warm-up pass of every query in the same server process; server start, data load and
  shutdown outside.
- arm_symmetry: same server binary, datadir, queries and order; only `ha_oqgraph.so`
  / mapped library differ.
- statistics: below. verdict: below.

## Protocol
- Host: reference host (i9-12900F, 8P+8E, Linux 6.8), Docker `oqeval:24.04`,
  `--cpuset-cpus=0-15` (the `cpu_core` list `scripts/bench_pin.sh` pins to);
  mariadbd `taskset -c 2,4,6,8`; client `taskset -c 10` — one SMT sibling per P-core, client and
  server on separate cores.
- Load snapshot (`/proc/loadavg`, `/proc/stat` busy jiffies) before each server start,
  after warm-up and after the timed pass; host lock `/run/expanse-bench/expanse-bench.flock`
  held for the whole run; no ssh to the host while it runs.
- Rounds: 10 per run, one mariadbd process per (round, arm), arm order shuffled per round
  (seeded). Two independent runs (separate invocations, different order seeds 11 and 22).
- Contamination rule: a round is void if the busy-CPU delta over its timed window
  exceeds 1.5 core-equivalents above the run's own median, or load1 > 12 (cores/2) at its
  start. Void rounds are discarded whole (all arms of that round) and disclosed.

## Statistics and decision rule
- Per round r, cell c, arm X: T = sum of the K query times. Paired log-ratio
  ℓ = ln(T_X / T_A) within the round.
- Estimate: exp(mean ℓ) with a BCa 95 % interval (`scripts/bca_bootstrap.py`
  `bca_bootstrap_ci_with_method`, 10 000 resamples, seed 42), per run.
- Cell verdict per run: `EQUIVALENT` if the interval ⊂ [0.97, 1.03]; `SLOWER` if lower
  > 1.00; `FASTER` if upper < 1.00; otherwise `INCONCLUSIVE`. (`SLOWER`/`FASTER` can
  co-occur with `EQUIVALENT`; both are reported.)
- A direction (`SLOWER`/`FASTER`) is claimed only when both runs give it for the cell
  (BENCHMARKING.md rule 18). Arm verdict: `PERFORMANCE-NEUTRAL` if every cell is
  `EQUIVALENT` in both runs; `REGRESSES` if any cell is `SLOWER` with lower bound > 1.03 in
  both runs; `IMPROVES` symmetric; otherwise `MIXED/INCONCLUSIVE`, with every cell listed.
- RSS: VmHWM and VmRSS after the timed pass, per arm, median over rounds; descriptive.
- Every losing cell is published. Thresholds, n and K are not changed after data; any
  change relabels the result `INTERMEDIATE` and needs fresh runs.
