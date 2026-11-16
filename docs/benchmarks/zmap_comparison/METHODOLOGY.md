# Pre-registration: zmap with stock libJudy vs libexpanse

Frozen before any timed data. Pilot data (rate selection, thread
identification) is excluded from every verdict. Evaluation for the owner,
not a published claim.

## Inputs
- zmap `d8fc99c9709962f8b016938a9e3fd0e5d67b55a1` (default branch, 2026-11-06).
- Expanse `82eb56af1a8a9512d9ab1b3b335a6d9c12756e23` (`origin/main`, v0.18.0),
  `cargo build --release -p expanse-capi` (thin LTO, cgu=1, panic=abort).
- Stock: Debian bookworm `libjudy-dev` (version recorded from dpkg in the image).
- Image `zmap-judy-eval:d8fc99c9-82eb56af`, base `rust:1.98-bookworm`, gcc from
  bookworm, zmap built with its defaults (`cmake` with no build type: `-O2`
  plus zmap's hardening flags, asserts on). Harness built `-O2 -std=gnu11`.

## What Judy does in zmap (from the audit)
JudyHS only (`JHSG`, `JHSI`, `JHSD`, `JHSFA`), only in `lib/cachehash.c`, only
reached from `src/recv.c:handle_packet` when `dedup_method == window` (the
default when more than one port is scanned). `--dryrun` never calls
`handle_packet` (`src/recv.c` recv loop sleeps), so a dryrun benchmark cannot
see Judy at all; the end-to-end level therefore uses a namespace-isolated
loopback scan.

## Isolation
`docker run --network none --privileged`: the container's root namespace has
only `lo` (asserted; the setup refuses otherwise). Two namespaces joined by one
veth pair: `scan` (192.0.2.1/24, no default route, asserted) and `resp`
(192.0.2.2/24, AnyIP local route for 198.18.0.0/15 so its kernel answers every
probe: SYN-ACK on 80/8080 via a listener, RST on 443/8443). No packet can
leave the container.

## Arms
- **NONE** — stock-built zmap, `--dedup-method none`. Upstream mode that makes
  no Judy call. It does less work (no dedup; `repeat` is always 0), so it is a
  floor, not a competitor: it bounds what any dedup structure could save.
- **STOCK** — stock-built zmap, `--dedup-method window` (window 1,000,000, the
  zmap default), Debian libJudy.
- **EXPANSE** — zmap compiled against Expanse's `include/Judy.h` and linked to
  `libexpanse` (`-lJudy` resolving to `libJudy.so -> libexpanse.so`), same
  flags as STOCK.
- Correctness only: **SWAP** — the STOCK binary run with
  `LD_LIBRARY_PATH` holding `libJudy.so.1 -> libexpanse.so` (the relink-free
  `libjudy-compat` path).

Microbenchmark: STOCK vs EXPANSE only. No no-Judy arm at this level: zmap has
no upstream cachehash without Judy, and a hand-written hash stand-in would
measure that stand-in, not zmap. The upstream floor is NONE end to end.

## Workloads
End to end (`198.18.0.0/15` x ports 80,443,8080,8443 = 524,288 targets,
1 sender thread, cooldown 2 s, fixed seed, output CSV
`saddr,sport,classification,success,repeat` with all responses kept):
- `zmap_e2e_e1_probes1`: 1 probe per target, 524,288 responses, every dedup
  lookup misses, no eviction.
- `zmap_e2e_e2_probes2`: 2 probes per target sent back to back,
  1,048,576 responses, 50% of lookups hit, no eviction.
Rate: chosen in a pilot as the highest of {50k, 100k, 200k, 400k} pps at
which all three arms show `pcap_drop == 0` and the expected response count
in E2, then fixed.

Microbenchmark (zmap's `lib/cachehash.c` compiled unchanged; workload shape
table in `cachehash_bench.c`): `zmap_dedup_m1_miss` (1,048,576 targets,
window 1,000,000; the last 48,576 puts evict), `zmap_dedup_m2_dup50` (same
targets, each looked up twice: 50% hit), `zmap_dedup_m3_evict` (4,194,304
targets, window 1,000,000: steady-state eviction through `JHSD`).

## Metrics
- E2E primary: CPU of zmap's receive thread (schedstat ns; the first thread
  zmap creates, `src/zmap.c:233`; identification confirmed in the pilot as the
  thread whose CPU scales with responses), divided by responses.
- E2E secondary: process user+sys CPU, peak RSS (GNU time `%M`), wall time.
- Micro primary: ns per response over the op sequence. Secondary: ru_maxrss
  growth across the timed region.

## Design
- E2E: 10 rounds per run, 2 independent runs (separate containers, fresh
  namespaces). Per round, every (workload, arm) cell once, arm order rotated
  per round. One zmap process per measurement.
- Micro: 15 rounds per run, 2 runs, arm order alternated per round, workloads
  rotated, one process per measurement, `taskset` to one CPU.
- Host: bare-metal reference host (i9-12900F), container
  `--cpuset-cpus=0,2,4,6,8,10,12,14` (one sibling per P-core), host bench lock
  held, no ssh to the host while a run is in flight.
- Load: `/proc/loadavg` before, host busy-CPU (core-equivalents) across every
  process. A measurement is void if host busy minus the process's own
  (user+sys)/wall exceeds 2.5 core-equivalents (the responder's softirq work
  is part of the scan and lands in host busy), or if `pcap_drop > 0`, or if
  the response count differs from the expected count. Voids are counted and
  disclosed; a round with a void cell is dropped whole.

## Statistics and decision rules
BCa 95% (`scripts/bca_bootstrap.py`, 2,000 resamples) on each arm mean, and the
two-sample BCa ratio interval EXPANSE/STOCK, per run.

- **H0 correctness (gate before timing).** EXPANSE and SWAP produce the same
  sorted output (sha256) and the same zmap counters (`success_unique`,
  `success_total`, `failure_total`, `validation_passed`) as STOCK on E1, E2
  and an eviction configuration (`--dedup-window-size 65536`, probes 2), and
  identical dryrun packet streams. The harness checks exact hit/miss counts.
  Any mismatch stops the benchmark and is the headline.
- **H1 Judy on the hot path?** Dedup share = (STOCK − NONE) / STOCK on
  receive-thread CPU. "Off the hot path" iff the BCa upper bound of the share
  is below 10% in both runs; "on the hot path" iff the lower bound exceeds
  10% in both runs; otherwise INTERMEDIATE. This charges all of cachehash
  (malloc/free of the key, list moves) to dedup, so it upper-bounds Judy.
- **H2 / H3 performance, per metric and workload.** EXPANSE_FASTER iff the
  ratio interval's upper bound < 1.0 in both runs; EXPANSE_SLOWER iff the
  lower bound > 1.0 in both runs; otherwise NO_DIFFERENCE_DETECTED with the
  interval reported. Same rule for peak RSS and the micro memory growth.
- No directional prediction for EXPANSE vs STOCK. Expected: the dedup share
  is small, because each response also pays pcap capture, validation, a
  fieldset allocation and CSV output.
- Threshold, n and method are fixed here. Any change after data relabels the
  affected result INTERMEDIATE and needs fresh runs.

## Amendment A1 (before any timed data; written 2026-11-09T07:32Z)
Found during the correctness gate and pilot, both excluded from verdicts:
1. The loopback rig loses a few responses per run with `pcap_drop == 0`
   (pilot at 200k pps: 2-12 of 1,048,576; correctness runs at 100k pps: 1-6),
   in every arm including NONE and STOCK, so the drop is in the rig, not the
   library. The void rule "response count differs from expected" becomes
   "fewer than expected − 0.01% (52 for E1, 104 for E2) or pcap_drop > 0".
   Per-response metrics divide by the run's own `validation_passed`.
2. Correctness of a live run is judged by an exact oracle
   (`dedup_oracle.py`) that replays the run's own arrival-ordered output
   through a reference LRU with cachehash semantics and compares the
   `repeat` column row by row, instead of cross-run output hashes. A churn
   differential (`diff_churn.c`: 200,000-key pool, window 65,536, 4,000,000
   ops) covers re-insertion after eviction.
3. Dryrun streams are compared on the ordered (daddr, dport) sequence:
   sequence numbers and source ports derive from a per-run random validation
   key, so full streams differ stock-vs-stock too.
4. Rate fixed at 200,000 pps (400k dropped in all arms; 200k had none).
5. Receive thread confirmed as the first created thread.
