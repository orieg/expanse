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

## 8. Pre-registration for #568 PR 3 — the single-writer bracket re-scope (appended 2026-09-08)

**Status: locked before any engine code on this branch. Appended, never
rewritten in place (AGENTS.md §8.7); §1–§7 above are Step 0's record and stand
as written.** Baseline: the `a1982ff2` pair, two runs per cell, in this
directory and the two FFI suites.

### 8.1 What the change is, and is not

The writer keeps its mutex. What moves is *which* version words go odd for
*which* stores: the tree-level `SeqVersion` brackets only root-state writes
(a `Root` variant change, a root-leaf mutation, a `top` edge rewrite), the
root population becomes an atomic word bumped under no bracket, and every
per-node bracket wraps the frame's own writes to that node — never the
recursion into a branch child. Readers are unchanged. It is not multi-writer,
not lock coupling with per-node acquisition, and it does not touch C1: the
writer-count cells are predicted unchanged and are published as controls.

### 8.2 Hypotheses, each with its refuter

- **H3.1 — the reader collapse under one writer is mostly the tree-level
  bracket.** Step 0 measured readers at C2 W=1 R=8 spending 53–60% of their
  time in `SeqVersion::sample` waiting on that bracket (P0.1). Removing it
  from ordinary writes should recover at least that share. Gate cells and
  their baseline (reader ns per probe = 8 ÷ aggregate M ops/s, the runner's
  own median; union of the two runs):

  | cell | readers alone (ns/probe) | under one writer, run A / run B (ns/probe) | union |
  |---|--:|--:|--:|
  | `hot_conc_set_w1_r8` (`hot_rowex_set_63bit`) | 54 | 282 / 521 | [282, 521] |
  | `hot_conc_map_w1_r8` (`hot_rowex_map_64bit`) | 63 | 499 / 435 | [435, 499] |
  | `masstree_conc_map_w1_r8` (`masstree_conc_map_64bit`) | 57 | 407 / 273 | [273, 407] |

  **PASS iff, on every cell, the union-upper of two post-change runs' ns per
  probe is below 0.5 × the baseline union-lower**: 141, 218 and 137 ns. That is
  a 2× improvement, the smallest effect consistent with a 50–60% spin share
  removed. Both runs improved but unions overlapping the threshold →
  `BOUNDARY_RESULT`, gate unmet; either run in the wrong direction →
  `REFUTED`. Instrument: two-commit mode in the suite runners (base and head
  binaries interleaved per round, the only before/after admissible under
  BENCHMARKING rule 18) — that runner work is part of this PR and lands
  before the numbers.
- **H3.2 — the writer's cost with readers present falls with the version
  line's traffic.** `perf c2c` put 47.7% / 51.7% of HITM samples on the line
  holding the tree version and the mutex (Step 0). A writer that stops storing
  to that line on ordinary writes should see its RFO misses per insert fall
  from 12.2–12.3 toward the readers-absent 1.9–2.1. Registered: writer-thread
  `l2_rqsts.rfo_miss ÷ write_ops` at C2 W=1 R=8 **≤ 6.0** (half the Step 0
  level); **REFUTED if ≥ 10.0**. The writer's insert rate at C2 W=1 R=8 is
  published beside it but is *not* a gate: its baseline union is wide
  (1.53–2.23 M/s across the three cells' runs) and c2c's share sat at the 50%
  line, so a level is predicted only as "above the baseline union-upper" with
  confidence `low`.
- **H3.3 — restarts rise, bounded.** Two brief brackets per level per op
  (slot write, then the population bump on the way out) replace one nested
  bracket; readers mid-node when either opens restart instead of waiting.
  Registered ceiling: restart share at C2 W=1 R=8 **≤ 30%** (Step 0 band
  9–15%); above it the design is reconsidered before shipping.
- **H3.4 — nothing the change is not targeting moves.** Callgrind: 0
  regressions on every non-`sync_*` arm (brackets compile out at
  `OCC=false`; measured, not assumed). The six `sync_*` arms are the path the
  change *targets* and may rise (a second bracket per level); their deltas are
  disclosed in the PR body and are not regressions under §6 — a
  `sync_map_get` rise above 0.1% *is*, since readers are unchanged. C1 W ∈
  {1, 2, 4, 8, 16} on both FFI suites: predicted inside the baseline union;
  a move is unpredicted and reported as such.

### 8.3 Soundness instruments that precede any measurement

- The address-checked bracket assert (`assert_bracketed_by(&node.version)` at
  every interior write, `assert_bracketed_by(parent_version)` at every leaf,
  immediate and subarray write) replaces `bracket_depth`; deleting any one
  bracket must panic at that site (fail-then-pass, AGENTS.md §5).
- `loom_obsolete_mark_covers_replaced_node` (#806) stays green; a single-threaded
  test drives the `OCC=true` monomorph under Miri (Stacked and Tree Borrows).
- `linearizability.rs`, `sync32_stress.rs`, the TSan shard, ASan.

### 8.4 What voids a cell

§6 above, plus: a post-change run taken without the two-commit interleaving,
or a cell whose base-commit half does not reproduce the `a1982ff2` union
(direction and overlap) — then the host, not the change, is being measured.

### 8.5 Explicitly not predicted

The string, bytes and blob wrappers (they keep the full tree bracket); the
50/50 `core_concurrency` sweep (a different workload, and single-writer
bound by construction); any writer-count scaling.

## 9. Reproducing the §8 gate (appended 2026-09-08, instrument only)

Nothing here changes §8; it names the commands that produce the artifacts §8
is read against. The reference host's session directory is an rsync'd tree,
so the base build comes from a second synced tree at the base commit, not
from a checkout.

```bash
# base tree (the branch's parent commit) built once; its two harness binaries
# are the `--ab-base-bin` of each suite
EXPANSE_BENCH_COMMIT=<head-sha> EXPANSE_BENCH_BASE_TREE=<path> EXPANSE_BENCH_BASE_COMMIT=<base-sha> \
  nohup docs/benchmarks/concurrency/scripts/fine_grained_brackets_campaign.sh > fine_grained_brackets.log 2>&1 &
```

The base binaries are the head tree's harness sources built against the base
tree's engine — one harness, two engines — so the interleaving flags exist in
both and only the engine differs. The campaign takes the host lock and the
P-core pin once, then in this order: the H3.2 counter cells at the head commit (`scripts/bench_counters.py`
into `results/fine_grained_brackets/` of each suite, so the Step 0 counter artifacts they are
read against stay in place), then two two-commit runs of each FFI suite's
concurrent arm (`results/baseline_concurrent_ab.json` and `_ab_run2.json`:
C1 W ∈ {1, 2, 4, 8, 16}, C2 W ∈ {0, 1} at R = 8, and the head-only health
cell at W = 1 R = 8). `scripts/fine_grained_brackets_gate.py` reads the six artifacts against
§8.2 and §8.4 and `scripts/tables.py` renders its verdicts into
[`README.md`](README.md) §8; nothing in the table is typed.

## 10. Pre-registration for #568 PR 5 — multi-writer optimistic lock coupling (appended 2026-09-09, locked before any engine code)

Written after #809 merged (`10cd755d`) and before PR 5 exists. The protocol
is `docs/ARCHITECTURE.md` §4.2; the bound functions are `scripts/olc_bounds.py`,
which reads every measured input from a committed artifact at run time.
Nothing below is rewritten in place once PR 5's cells run (§8.7); a
threshold that turns out wrong is relabelled `INTERMEDIATE` and re-run
(§8.19).

### 10.1 What the merged engine measures, which PR 5 is gated against

The two-commit sweeps of #809 (`results/baseline_concurrent_ab{,_run2}.json`
of both FFI suites) are the baseline: the *head halves* at `10cd755d`,
base and head interleaved round by round, two runs. The writer-only C1
cells, M inserts/s, the union of the two runs' medians:

| suite | arm | W = 1 | W = 2 | W = 4 | W = 8 | W = 16 |
|---|---|--:|--:|--:|--:|--:|
| hot_comparison | set | [7.19, 7.24] | [4.11, 4.16] | [3.17, 3.21] | [2.55, 2.57] | [2.35, 2.40] |
| hot_comparison | map | [5.09, 5.09] | [3.32, 3.36] | [2.72, 2.73] | [2.34, 2.35] | [1.69, 2.12] |
| masstree_comparison | map | [5.43, 5.44] | [3.44, 3.47] | [2.75, 2.76] | [2.33, 2.40] | [2.23, 2.23] |
| masstree_comparison | str | [3.87, 3.88] | [2.38, 2.39] | [2.23, 2.25] | [1.93, 2.04] | [0.48, 0.48] |

Two facts about that baseline PR 5 has to carry. First, the merged engine's
writers *fall* with W on every arm — one mutex, and the brief brackets'
version stores are the next writer's misses (a hypothesis; the C1 W ∈ {4, 8}
writer-thread `l2_rqsts.rfo_miss` cell is registered below to test it).
Second, on three of four arms the merged engine's C1 cells are below the
pre-#809 base halves (up to −25% at HOT set W = 8, README §8); PR 5's gate
is stated against the merged engine, and the pre-#809 levels are named
beside it as the second bar.

### 10.2 Predictions, each with its refuter

- **P5.1 — writers scale on disjoint expanses.** On every FFI C1 arm and every
  W ∈ {2, 4, 8, 16}, the aggregate insert rate's union-lower over two
  two-commit runs (head halves, PR 5 against `10cd755d`) is above the W = 1
  union-upper of the same run pair. **REFUTED** if any arm at any W ≥ 2 has
  a union-upper below its own W = 1 union-lower (writers still fall).
  `BOUNDARY_RESULT` where the unions overlap. The string arm is published
  but not a gate cell: its writer holds the tree word for the whole
  operation (#730).
- **P5.2 — the pre-#809 levels are recovered.** At W ∈ {4, 8}, the PR 5 head
  half's union-lower is above the *base* halves of #809's sweeps (HOT set
  [4.05, 4.09] and [3.41, 3.63]; HOT map [3.18, 3.25] and [2.74, 2.75];
  Masstree map [3.22, 3.34] and [2.62, 2.75]). **REFUTED** on an arm whose
  union-upper stays below that; then Stage B recovered nothing the brief
  brackets cost, and that is published as the finding.
- **P5.3 — restarts stay bounded.** `Stat::LockRestarts ÷ write_ops` at each
  W is below `olc_bounds.restart_ceiling(W, t_hold, t_op, safety_factor = 2.0)`
  with `t_hold` the *measured* median lock hold from the health build's
  `Stat::LockHoldCycles ÷ cycles_hz` on the same head and `t_op` from the
  head's own W = 1 cell. The ceiling is instantiated from those two
  measurements before the W ≥ 2 cells are read, and the instantiated
  numbers are written into README §9 beside the verdict; with the
  hypothesis hold of 15 ns and the merged W = 1 rate the shape is 0.18 /
  0.58 / 1.61 / 5.10 restarts per operation at W = 2 / 4 / 8 / 16
  (`python3 scripts/olc_bounds.py`). **REFUTED** at any W above the
  instantiated ceiling. The safety factor 2.0 is a choice, fixed here.
- **P5.4 — the contended-line bound holds.** The FFI arms' W = 16 rate is at
  or below `olc_bounds.contended_rmw_ceiling(k = 1, t_line, t_hold)` with
  the measured `t_line` (33.37 ns, the artifact's median cell) and the
  measured `t_hold`; the `core_concurrency` shape (`rng % 2M`, k = 5) is
  predicted *not* to clear its own W = 1 and is published as a losing cell,
  not gated. **REFUTED** if an FFI arm exceeds the bound (then `k = 1` is
  wrong for that shape and the bound is re-derived, not the gate).
- **Controls, predicted unchanged:** every single-threaded Callgrind arm at
  0.00% (the `sync_*` reader arms included); the C2 W = 1 R = 8 reader cells
  inside their #809 head-half unions (97–104 ns per probe, README §8); the
  reader fallback rate `locked_reads ÷ read_ops` at W = 16 below 1% — that
  cell is measured *before* `MAX_RETRIES` is fixed for PR 5, and the
  chosen value is written here as an amendment, dated, never as a rewrite.

### 10.3 Instruments

The same two-commit runners as §8 (`--ab-base-bin` / `--ab-base-commit`
against `10cd755d`), two runs per suite, one host lock, concurrent sweeps
last (§8.17), executed via `docs/benchmarks/concurrency/scripts/multi_writer_olc_campaign.sh`
and evaluated by `scripts/multi_writer_olc_gate.py` (with `results/multi_writer_olc/`).
New counters PR 5 adds and this section relies on:
`Stat::LockRestarts`, `Stat::LockSpins`, `Stat::LockHoldCycles` (sharded per
thread like the #804 counters). New `bench_counters.py` cells: writer-thread
`l2_rqsts.rfo_miss` at C1 W ∈ {4, 8} on the map arm (the P5.1 mechanism
test, run at `10cd755d` first as the baseline), and the W = 16 fallback
cell. `benches/concurrency.rs` is not an instrument here until it has a
barrier start, an elapsed-from-barrier divisor, at least 15 windows with
BCa intervals and the `bench_pin.apply` call.

### 10.4 What voids a cell

§6 applies. In addition: a P5.3 verdict read before `t_hold` is measured is
void; a P5.1 or P5.2 verdict whose base half sits outside the `10cd755d`
union of §10.1 is `VOID` the way §8.4 voids a cell.

### 10.5 Explicitly not predicted

The `sync_*` writer arms' instruction counts (they carry the lock protocol
and will move); the string arm's writers at any W; the SMT-paired W = 16
cells' relation to W = 8 beyond "not below"; wall clock on any host but the
reference host.
