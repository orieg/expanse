# Concurrency instruments — pre-registration for #568 Step 0 (attribution)

**Status: locked before any measurement in this suite. This is commit 2 of
the three-commit cadence (AGENTS.md §8.8); the instruments it names are
commit 1 (`crates/expanse/src/occ_stats.rs`, `crates/expanse/examples/line_transfer.rs`,
`scripts/line_transfer_matrix.py`, the `sync_*` arms in
`crates/expanse/benches/instructions.rs`, the per-thread mode of
`scripts/bench_counters.py`, and the widened health rows of the two FFI
suites). No number below is a result; every figure quoted is an existing
committed artifact, cited.**

Tracking issue: [#568](https://github.com/orieg/expanse/issues/568) (open).
This section is never rewritten in place (AGENTS.md §8.7); outcomes go in
`README.md` with their verdict labels, and any amendment is appended as a
dated subsection at the end.

## 1. The question

`SyncExpanseMap` / `SyncExpanseSet` show two declines under write load, and
the issue proposes one remedy (per-node write locks) for both. Before any
engine change, Step 0 attributes each decline to a mechanism with a counter
that could have said otherwise:

- **D1 — readers collapse under one writer.** Eight readers with one
  writer present make 0.10–0.14× the lookups they make alone
  *(measured: reference host, harness commit `64f8a3af`,
  `docs/benchmarks/hot_comparison/results/baseline_concurrent.json` and
  `docs/benchmarks/masstree_comparison/results/baseline_concurrent.json`,
  cells C2 W=1 R=8; workloads `hot_rowex_set_63bit`, `hot_rowex_map_64bit`,
  `masstree_conc_map_64bit`)*.
- **D2 — writers fall as writers are added.** Aggregate inserts fall from
  one writer to sixteen rather than plateau *(measured: same artifacts,
  cells C1 W∈{1,2,4,8,16}, workload `masstree_conc_map_64bit`)*, and one
  writer with eight readers present inserts at about a third of its
  readers-absent rate *(measured: same artifact, C2 W=1 R=8 vs C1 W=1)*.

The two are hypothesised to be different mechanisms (§4), which is why the
engine work that follows is staged (#568 plan, PR 3 then PR 5). If they are
not — if the counters put both on the same line — the staging changes
before any of it is built.

## 2. Prior observations at lock time (mandatory disclosure)

Everything already known that bears on the predictions, so a reader can
judge how much of §4 was informed:

- Health cells at C2 W=1 R=8 read `sample_spins ÷ read_ops` of 0.58–1.02
  on the integer arms and 2.06–2.26 on the string arm, restart share
  1.3–4.8% (integers) and 31–35% (strings), fallback share 0 in every
  round *(measured: `64f8a3af`, both suites' `baseline_concurrent.json` and
  `baseline_concurrent_run2.json`)*. The HOT README §7.3 table was
  hand-typed from an earlier harness commit and printed 1.64–2.05 for the
  same cells; both suites' health tables are now generated from the
  artifacts (AGENTS.md §8.2), and each README's prose reads from them.
- The counter itself moved between two identical runs — HOT set C2 W=1:
  0.89 → 0.58 spins per lookup, restart share 4.2% → 1.3% — while the
  throughput cells overlapped. The counters were one process-global cache
  line bumped by nine threads; they are per-thread shards from commit
  `59915e80` on. **The spread of the re-sharded counter is itself a Step 0
  output** (two runs, §5).
- A spin iteration is one `pause`; a count of ~1 per lookup cannot by
  itself account for a per-probe cost that rises from ~58 ns to ~490 ns
  (readers alone vs one writer, `masstree_conc_map_64bit` C2). Whether the
  residue is time spent in the spin loop, line transfers on the version
  word, the reader-slot lines the writer scans, or displacement is
  **unmeasured** at lock time.
- `Stat::Handoffs` (#789) is zero by construction at C2 W=1 (no second
  thread ever takes the mutex; `read_fallbacks` is 0). It informs D2 at
  W ≥ 2 only.
- The recorded governor in both artifacts is `powersave` (intel_pstate
  HWP). The issue's third comment reports up to 2× movement of write-mixed
  points between governors on another host; PR 0 records the governor per
  pinned core and the effective clock (`cycles ÷ ref-cycles`) beside every
  counter cell so this is a fact in the artifact, not a caveat in prose.
- The concurrent cells place W + R threads on the 16 logical CPUs of 8
  physical performance cores; at W + R > 8 some threads share a core. This
  is now declared in the harness shape tables; it is not controlled.

## 3. Instruments and cells

Every instrument is committed before this file is; none has produced a
number for this suite.

| Instrument | What it measures | Cells |
|---|---|---|
| FFI health rows (`--health`, `occ-stats` build) | `read_ops`, `read_attempts`, `read_fallbacks`, `locked_reads`, `sample_spins`, **`sample_spin_cycles`** (+ `cycles_hz`), `handoffs`, `branch_replacements`, `deep_cascades`, `root_rewrites`, `retired`, `freed_raw`, `write_ops`, reader/writer elapsed | `hot_concurrent` set/map and `masstree_concurrent` map/str, H W∈{1,2,4,8} R=8 |
| Per-thread hardware counters (`scripts/bench_counters.py`, `--arm expanse` counters build) | per **role** (writer threads / reader threads): `cycles`, `ref-cycles`, `instructions`, `task-clock`, `context-switches`, `LLC-load-misses`, `l2_rqsts.rfo_miss`, `mem_load_l3_hit_retired.xsnp_hitm`; `syscalls:sys_enter_futex` only if the preflight opens it | `masstree_conc_map_{w1_r8, w1_r0, w0_r8, w8_r0, w16_r0}`, `masstree_conc_str_{w8_r0, w16_r0}`, `hot_conc_{set,map}_{w1_r8, w1_r0, w0_r8}` — the `w0_r8` cells are the reader-side controls for the C2 coherence columns |
| `perf c2c` | which lines the writer and readers contend on, named by `sync::layout_report()` offsets | `masstree_conc_map_w1_r8`, `hot_conc_map_w1_r8` |
| `scripts/line_transfer_matrix.py` | one-way cache-line transfer cost between every pair of physical P-cores, spinning (`spin`) and parked (`park`); `pause` iterations per second | the host |
| `benches/instructions.rs` `sync_*` arms | exact instruction counts of the `OCC=true` engine on one thread (writer mutex uncontended, all brackets executed, advance every 32) | `sync_map_insert`, `sync_set_insert`, `sync_map_get`, `sync_set_contains`, `sync_map_churn`, `sync_map_remove` |
| #789 ablations | `advance-every-4096`, `advance-never`, `lock-padded` builds | C1 W=1 and C2 W=1 R=8, `masstree_conc_map_64bit` |

Cells are Expanse-arm only except where the competitor is named; the
competitor's counters are not an attribution of Expanse's decline and are
not collected here.

## 4. Predictions — locked, each with its refuter

All predictions are on the three integer cells `hot_conc_set_w1_r8`,
`hot_conc_map_w1_r8`, `masstree_conc_map_w1_r8` unless a cell is named.
"REFUTED" thresholds are chosen so that a refutation changes what is built
next (AGENTS.md §8.19), not so that they cannot fire.

- **P0.1 (D1, spin time).** `spin_time_share` — `sample_spin_cycles ÷
  cycles_hz` over the readers' summed elapsed time — is **≥ 0.50** at C2
  W=1 R=8. **REFUTED if < 0.25.** Consequence of refutation: the reader
  gain expected from narrowing the tree-level bracket (#568 PR 3) is
  re-scoped to whatever share `perf c2c` attributes to the version line
  before PR 3 is built.
- **P0.2 (D2, the writer's loss to readers).** Writer-thread
  `l2_rqsts.rfo_miss ÷ write_ops` at C2 W=1 R=8 exceeds the same figure at
  C1 W=1 R=0 by **≥ 3.0**. **REFUTED if the difference is < 1.0.** Beside
  it, as a control: writer-thread `xsnp_hitm ÷ write_ops` ≥ 1.0 at C2 and
  ≤ 0.1 at C1 W=1.
- **P0.3 (D2, the string cliff).** Writer-thread `context-switches ÷
  write_ops` on `masstree_conc_str` at C1 W=16 is **≥ 0.5** and at C1 W=8 is
  **≤ 0.1**; on `masstree_conc_map_64bit` at W=16 it is **≤ 0.1**.
  **REFUTED if the string W=16 figure is < 0.2.** The Masstree-suite map
  C1 W=16 cell moved past its own interval between the two committed runs,
  so its level is not predicted (BENCHMARKING rule 18); direction only.
- **P0.4 (the counter's own spread).** With per-thread shards, the H-cell
  ratios `sample_spins ÷ read_ops` and restart share on two runs of the
  same commit overlap in every integer cell. **REFUTED if any integer cell's
  two values differ by more than 1.25×.** Consequence: the counter is
  reported as a band, never as a level, in every later stage.

**Controls, not predictions** (they are categorical by design at these
writer counts, per `docs/benchmarks/hot_comparison/METHODOLOGY.md` §11.8):
restart share, `fallback_share = 0`, `locked_reads ÷ read_ops` (published
for the first time; expected 0 at these cells because no reader takes the
mutex and no `with_locked` path runs), `deep_cascade_share`,
`root_rewrite_share` (published as the rarity the #568 PR 4 lock-set rule is
sized against — no level is predicted).

## 5. Verdict form

Per cell, an attribution table with one row per candidate share of the
per-probe (D1) or per-insert (D2) delta: spin time (P0.1), restarts (from
`read_attempts − read_ops` × the readers-alone per-probe cost), fallback
(0 by construction, stated), writer RFO / HITM lines (P0.2, with `perf
c2c` naming the lines), wake and context-switch (P0.3), and
**unattributed** — the remainder, published as a number. Each prediction's
row carries its verdict label from the shared vocabulary (`CONFIRMED`,
`REFUTED`, `BOUNDARY_RESULT`, `NOT_INSTRUMENTED`).

Two runs of every cell on the reference host under the standing
conditions (host lock, P-core pin, load snapshot per cell); the claim
ceiling is the union of the two intervals, and a cell whose two runs do
not overlap is reported direction-only (BENCHMARKING rule 18). The
line-transfer matrix is one run of seven repeats per pair; it is a host
property and is re-taken whenever the host's kernel, microcode or governor
changes.

## 6. What voids a cell

- A load snapshot with `foreign_busy_cpus` above 1.0 core-equivalents, a
  load average above 12 (cores ÷ 2) or a load shift above 2 between the
  two runs of a pair (AGENTS.md §8.17): discarded and disclosed.
- Any timing taken from a binary built with `occ-stats` (the health build
  is counters-only; the harness refuses, and so does this file).
- A hardware event the preflight could not open: its column is `None`
  and the prediction that needs it is `NOT_INSTRUMENTED`, never 0.
- A per-thread counter row whose thread name is neither `writer-*` nor
  `reader-*`: the cell is void (the role split is the instrument).
- Governor not `performance` or `powersave` as recorded per core, or the
  two runs taken under different governors.

## 7. Explicitly not predicted

- The size of D1 after PR 3 (that is PR 3's own pre-registration, against
  this suite's numbers).
- Anything about the string wrapper's reader path beyond P0.3 (#730).
- Competitor-side counters.
- Any level for the Masstree-suite map C1 W=16 cell (rule 18, above).
