# Concurrency instruments — results

The home of the core concurrency instruments' published numbers for
[#568](https://github.com/orieg/expanse/issues/568): the host's cross-core
line-transfer matrix, the `pause` calibration, and the attribution verdicts
that the two FFI suites' health and counter cells feed. The pre-registration
every number here is read against is [`METHODOLOGY.md`](METHODOLOGY.md);
read it first. The `Sync*` scaling sweep (`benches/concurrency.rs`) stays
where it was — the `/benchmark concurrency` builtin runner in
`.github/workflows/bench_baremetal.yml`, reported in
`docs/BENCHMARKING.md` ("Concurrent read scaling").

**Status: pending measurement.** No cell in this directory has been run.
Every table below is a placeholder whose rows are filled by the scripts
named beside it, never by hand (AGENTS.md §8.2); until then each cell reads
`pending`, tracked by [#568](https://github.com/orieg/expanse/issues/568).

## 1. Reproduce

```bash
docs/benchmarks/concurrency/run.sh            # line-transfer matrix + pause, reference host
docs/benchmarks/concurrency/run.sh --quick    # three cores, scratch output under results/quick/
```

The health cells and per-thread counter cells are produced by the two FFI
suites and `scripts/bench_counters.py`:

```bash
docs/benchmarks/hot_comparison/run.sh --only-concurrent
docs/benchmarks/masstree_comparison/run.sh --only-concurrent
python3 scripts/bench_counters.py --cell masstree_conc_map_w1_r8 --repeats 7
```

Their artifacts stay in their own suites (`results/baseline_concurrent*.json`,
`results/counters_<cell>.json`); this page cites them.

## 2. Line-transfer matrix (`results/line_transfer.json`)

One-way cache-line transfer between every pair of physical performance
cores, spinning and parked, plus the `pause` iteration cost. Rendered by
`scripts/line_transfer_matrix.py` from the artifact.

| kind | mode | cells | min ns | median ns | max ns |
|---|---|--:|--:|--:|--:|
| pause | pause | pending ([#568](https://github.com/orieg/expanse/issues/568)) | pending | pending | pending |
| cross-core | spin | pending | pending | pending | pending |
| cross-core | park | pending | pending | pending | pending |
| smt-sibling | spin | pending | pending | pending | pending |
| smt-sibling | park | pending | pending | pending | pending |

## 3. Attribution — D1, readers under one writer (METHODOLOGY §4, §5)

One row per candidate share of the per-probe delta at C2 W=1 R=8, per cell.
Filled from the FFI suites' health and counter artifacts once both runs
exist; the verdict labels are the shared vocabulary.

| cell | run | spin time (P0.1) | restarts | fallback | writer RFO / insert (P0.2) | writer HITM / insert | reader HITM / probe (vs alone) | reader cycles / probe (vs alone) | unattributed | verdict |
|---|--:|---|---|---|---|---|---|---|---|---|
| `hot_conc_set_w1_r8` | — | pending ([#568](https://github.com/orieg/expanse/issues/568)) | pending | 0 by construction | pending | pending | pending | pending | pending | pending |
| `hot_conc_map_w1_r8` | — | pending | pending | 0 by construction | pending | pending | pending | pending | pending | pending |
| `masstree_conc_map_w1_r8` | — | pending | pending | 0 by construction | pending | pending | pending | pending | pending | pending |

## 4. Attribution — D2, writers under load (METHODOLOGY §4, §5)

| cell | context switches / insert (P0.3) | writer off-CPU share | cycles / insert | RFO / insert | HITM / insert | futex / insert | verdict |
|---|---|---|---|---|---|---|---|
| `masstree_conc_str_w8_r0` | `NOT_INSTRUMENTED` | `NOT_INSTRUMENTED` | `NOT_INSTRUMENTED` | `NOT_INSTRUMENTED` | `NOT_INSTRUMENTED` | `NOT_INSTRUMENTED` | see METHODOLOGY §4 P0.3 |
| `masstree_conc_map_w8_r0` | `NOT_INSTRUMENTED` | `NOT_INSTRUMENTED` | `NOT_INSTRUMENTED` | `NOT_INSTRUMENTED` | `NOT_INSTRUMENTED` | `NOT_INSTRUMENTED` | see METHODOLOGY §4 P0.3 |

## 5. The counter's own spread (P0.4)

Two runs of the H cells at one commit with per-thread counter shards;
reported as a band per cell. Pending ([#568](https://github.com/orieg/expanse/issues/568)).

| suite | arm | W | spins ÷ read_ops run 1 | run 2 | ratio | restart run 1 | run 2 | ratio | verdict |
|---|---|--:|--:|--:|--:|--:|--:|--:|---|
| — | — | — | pending ([#568](https://github.com/orieg/expanse/issues/568)) | pending | pending | pending | pending | pending | pending |

## 6. Between-run spread and what voids a cell

Both runs, load snapshots per cell with the foreign-CPU share, governor per
pinned core, effective clock per counter cell — all in the artifacts, none
retyped here. A cell voided under METHODOLOGY §6 is listed in this section
with its reason, never silently dropped (AGENTS.md §8.1).
