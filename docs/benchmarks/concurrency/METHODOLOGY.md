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
| `benches/instructions.rs` `sync_*` arms | exact instruction counts of the `OCC=true` engine on one thread (writer mutex uncontended, all brackets executed, advance every 32) | `sync_map_insert`, `sync_set_insert`, `sync_map_get`, `sync_set_contains`, `sync_map_churn`, `sync_map_remove`, `sync_set_churn`, `sync_set_remove` |
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
  or below `olc_bounds.contended_rmw_ceiling(k, t_line, t_hold)` with
  the measured `t_line` (33.37 ns, the artifact's median cell), the
  measured `t_hold`, and $k$ determined by the build configuration ($k = 2$
  for the default build `ffi_disjoint_default`; $k = 1$ under `lock-padded`
  `ffi_disjoint_padded`); the `core_concurrency` shape (`rng % 2M`, k = 5) is
  predicted *not* to clear its own W = 1 and is published as a losing cell,
  not gated. **REFUTED** if an FFI arm exceeds the bound.
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
and evaluated by `docs/benchmarks/concurrency/scripts/multi_writer_olc_gate.py` (with `results/multi_writer_olc/`).
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

### 10.6 Dated amendment (2026-09-10) — base ref reconciliation, canonical sequence comparison, and contention health measurement

1. **Base ref update to `b49835ad`**: Section 10.2 pre-registered comparison against `10cd755d`, while campaign automation had temporarily defaulted to `1edfa952`. Across two runs against `1edfa952`, 7 of 20 base halves fell outside §10.1's registered union — notably Masstree map W=1 (5.53 / 5.52 M/s versus registered [5.43, 5.44] M/s), which voided the cell under §10.4.
2. **Canonical Phase 1.5A evaluation**: The authoritative question Phase 1.5A (#568) evaluates is whether the full zero-sharing and lazy rollup sequence (#818, #819, #821, #822) recovered write scaling compared to the mature pre-1.5A OLC baseline. The baseline is therefore fixed as `b49835ad` (the immediate predecessor of #818), and the head build is `f9efa260` (landed in #822) or current `main`.
3. **Contention health cells ($W \ge 2$)**: The two-commit sweep script (`run_all.py`) is updated to record health rows for $`W \in \{1, 2, 4, 8, 16\}, R = 0`$ in addition to the $W = 1, R = 8$ mixed-reader row. Previously committed health cells were restricted to $W = 1$, where `Stat::LockRestarts` and `Stat::LockSpins` were 0 by construction. Measuring health across all $W$ provides the empirical restart counts required for P5.3 evaluation against the instantiated restart ceilings, and directly measures the fallback share and spin time under concurrent write pressure.
4. **Build configuration and $k$ selection for P5.4**: The default engine build (`Cargo.toml` `default = ["std"]`) runs without `feature = "lock-padded"`. Per `scripts/olc_bounds.py`, residual false sharing across the 64 atomic writer slots packs 8 per cache line, so the default build shape is `ffi_disjoint_default` with $k = 2$ ($12.39\text{ M ops/s}$ for set, $8.95\text{ M ops/s}$ for HOT map, $9.06\text{ M ops/s}$ for Masstree map). The gate script selects $k = 2$ for the default build, and selects $k = 1$ (`ffi_disjoint_padded`) only when the `lock-padded` feature is enabled. P5.4 tests whether the observed W = 16 throughput stays at or below this ceiling.

## 11. Hypothesis D — shared allocator and reclamation state (appended 2026-09-11, locked before any ablation run)

The question (#568): at W ≥ 2, is per-insert cost inflated by state that every
writer of a tree shares in its allocator and its epoch collector? Each arm is a
build feature that removes one sharing mechanism and changes nothing else; the
default build compiles all three out.

| Arm | Feature | Shared state it removes | Replaced by | Suite token |
|---|---|---|---|---|
| a | `ablation-sharded-alloc` | `NodeAlloc`'s `bytes_in_use`, `live_allocs` and `total_allocs`, updated by every allocation and free | one cache-line-aligned shard per stripe; the accessors sum the shards | `writer_scaling_ablation_alloc` |
| b | `ablation-striped-epoch` (landed in production standard, Refs #568) | the collector's epoch bins (one mutex per bin, taken by every retire) and its `retained_bytes` counter | `bins[e % BINS][stripe]` and one `retained_bytes` per stripe ($S=16$ in production standard with thread-exit slot recycling) | `writer_scaling_ablation_epoch` (retired; measured via `writer_scaling`) |
| c | `ablation-unstriped-freelist` | reverts collector freelists to a single shared array across all writer stripes (Refs #568; per-stripe freelists landed as production default) | one set of class freelists shared by all stripes; reverts per-stripe freelists to measure contention | `writer_scaling_ablation_unstriped_freelist` |

**Stripe.** A thread's stripe is assigned via dense slot reservation and thread-exit recycling (`occ::writer_slot`, backed by `ALLOC_SLOTS_MASK`), so $N_{\text{live}}$ active threads strictly occupy slots $0..N_{\text{live}}-1$ with zero modulo collision across the 16 stripes. The sweep's W is at most 8.

**Instrument and decision rule.** `writer_scaling.py --compare-ablation-<arm>`
runs the default and the ablated build in interleaved (build × W) rounds and
reports, per W ≥ 2, the paired ratio $C_{\text{variant}}(W) / C_{\text{default}}(W)$
with its BCa 95% interval (AGENTS.md §8.20.2). W = 1 is the control. Per cell:
`SINGLE_RUN_PASS` if the interval's lower bound exceeds 1.0, `REJECTED` if its
upper bound is below 1.0, `INCONCLUSIVE` otherwise. A claim needs the same
verdict on the same cells in two independent runs (`docs/BENCHMARKING.md`
rule 18). Threshold, method and round count are fixed here; changing any of
them after a run relabels that run `INTERMEDIATE` (AGENTS.md §8.19).

**What a verdict can say (AGENTS.md §8.20.3).** A `REJECTED` arm rules out its
own mechanism, at the cells tested, and nothing else. Shared state that no arm
removes stays on the unexplained line: the system allocator behind a
`pop_freelist` miss, the collector's reader registry (locked by every advance),
its `epoch` and `op_count` words, and the tree-level writer state that
Hypothesis B's `lock-padded` comparison addresses. The arms are not additive:
removing one sharing point can move contention to another, so one arm's
`SINGLE_RUN_PASS` does not show that its mechanism dominates, and the sum of
the arms' effects is not a decomposition.

**Confounds, stated before any run.**
- Arm b changes the advance, which runs on the write path: every writer's
  successful optimistic operation bumps the collector's `op_count` and every
  `ADVANCE_EVERY` (32)th attempts one (`Collector::tick_advance`), as does
  every 32nd covered write. The default build takes one bin lock per advance;
  arm b reads one flag per stripe (`MAX_WRITER_SLOTS` loads, no lock) and
  locks only the stripes that hold garbage. The extra loads are work the
  default build does not do, and bias the arm against a pass.
- Arm c changes where blocks are reused, not only which lock is taken: in the
  default build a writer can reuse a block another writer retired, and under
  arm c only its own. Where writers retire and allocate at different rates,
  more allocations fall through to the system allocator. No counter records
  `pop_freelist` hits, so this shift is unmeasured.
- Arm a adds a thread-local read to every allocation and free; arms b and c add
  one to every retire and, for c, every `pop_freelist`.

**How often each arm's state is touched** is recorded by the counters pass of
the same run (`total_allocs_per_insert`, and the `retired` counter), so a
verdict can be read against the rate at which its mechanism is exercised.

**Explicitly not predicted.** No direction or magnitude is predicted for any
arm; this is an interventional diagnostic, and no run exists yet (#568).

## 12. Pre-registration for #900 — optimistic ordered reads on the concurrent map (appended 2026-09-14, locked before any ordered-read engine code)

The question (#900): can `SyncExpanseMap` answer ordered queries under optimistic lock coupling, instead of through `with_locked`, which excludes every writer? And do its readers gain from it?

Decisions fixed before this section, on #900:

- **Scope:** the six-operation family on the map, at 64-bit and 32-bit.
- **Shape:** single calls, no cursor.
- **Retries:** restart the whole walk when a validation fails. After `MAX_RETRIES` (64), a 64-bit read falls back through `read_locked`; a 32-bit reader returns `Busy`.
- **Code layout:** the validated walks are separate from `nav.rs`. The retry protocol is extracted once (`optimistic_read`) before the walks are added.

### 12.1 What is derived, and what the change must do

- **The read set.** `nav::prev` and `nav::next` make at most one sibling descent. A search that backtracks at branch level ℓ therefore validates ℓ + 5 branch versions: at most 13, against `get`'s 7 (`olc_bounds.ordered_read_set_branches`, #926).
- **The validation rule.** `get` validates hand-over-hand. A search that drops a child's snapshot before its sibling descent can return a key that was never the answer, and `loom_ordered_read_hand_over_hand_is_not_enough` finds that interleaving. The rule that passes `loom_ordered_read_retained_read_set` (#925) is a *retained read set*:
  - every branch version the search read is validated again after its last load;
  - every empty subtree is confirmed against its node.
- **A projection, not a prediction.** It assumes per-node independence and is dated to artifact commits `a1982ff2` and `c71fa4ba`. On that basis an ordered read fails 10.06–49.71% of attempts, takes 1.11–1.99 attempts per operation, and falls back on at most 3.7e-20 of operations (`python3 scripts/olc_bounds.py` at `4bb5a4cf`). Writes concentrated near the probe break the independence hypothesis; P12.4 measures that case.

### 12.2 Soundness gates, before any measurement

No cell in §12.4 is read until every gate below passes on the head being measured.

- **G12.1 — deterministic reproducer.** A thread-armed `cfg(test)` park point sits at the validated walk's backtrack step, following the `test_hooks::Gate` pattern in `sync.rs`. It replays the Loom interleaving on the real walk, in an insert variant and a remove variant. A variant that drops the child's snapshot must fail by name, and the shipped walk must pass (AGENTS.md §2.3).
- **G12.2 — history.** The whole-map checker in `tests/linearizability.rs` passes with the optimistic operations in place of `with_locked`, on the tree-rooted history and on a hot-spot history.
- **G12.3 — differential.** On quiescent trees, across the `keys` distributions, the validated walks agree with `nav::next` and `nav::prev`, including keys 0 and `u64::MAX` and `next_after(u64::MAX)`.
- **G12.4 — lanes, and a known defect.** The Loom, TSan and ASan lanes pass. The intermittent length mismatch in `sync::obsolete_tests::set_lazy_branch_pop0_fold_and_sharded_tree_pop_invariant` must also be explained: CI run 34801088246 counted 8999 against 9000 after every writer joined. Ordered reads run beside that insert path, so a concurrent cell measured before the mismatch is explained is void.
- **G12.5 — 32-bit.** `sync32_stress` checks that every key a `try_*` ordered read returns was the correct neighbour in some committed state.

G12.1 and the Loom models are re-run after any #568 change to the obsolete-marking or cover rules.

### 12.3 Predictions, each with its refuter

- **P12.1 — the single-threaded paths do not move** (AGENTS.md §2.1.5).
  - The claim: in each PR that adds ordered-read code, against that PR's base, `map_nav/*`, `map_prev/*`, `map_get/*`, `map_insert/*`, `map32_nav/*` and `map32_prev/*` change by at most 0.1%, the §6 review threshold. The disassembly of `nav::next`, `nav::prev` and `mutate::insert_with_path_flat` also gains no thread-local access.
  - **REFUTED** on any arm above 0.1%.
- **P12.2 — extracting the retry protocol moves no concurrent reader.**
  - The claim: in the PR that moves them onto `optimistic_read`, `sync_map_get/random` and `sync_set_contains/random` change by at most 0.1%.
  - **REFUTED** above that.
- **P12.3 — an optimistic ordered read costs fewer instructions than the locked one.**
  - The claim: on the head that adds it, the new `sync_map_prev/random` arm (a reader handle's `prev_before` from each present key) counts fewer instructions per operation than `sync_map_prev_locked/random` on the same head.
  - For scale, #925 measured 437.6 per operation for `map_prev/random` and 1,142.6 for `sync_map_prev_locked/random` *(measured: CI `instruction-counts` on #925, head `409e1d30`)*. No magnitude is predicted.
  - **REFUTED** at or above the locked arm.
- **P12.4 — fallbacks stay rare when writes concentrate near the probe.**
  - The claim: in the counters build, `read_fallbacks ÷ read_ops` for ordered reads is below 0.1% in every §12.4 cell, uniform and hot-spot.
  - The ceiling is a choice, fixed here, orders of magnitude above the independence projection. Every fallback quiesces writers (`read_locked`), so the cell tests one hypothesis only: that correlated validation failures turn ordered reads into repeated writer stalls.
  - Attempts per operation are reported beside the projection and are not gated.
  - **REFUTED** at any cell above 0.1%.
- **P12.5 — readers gain over `with_locked`.**
  - The statistic: in the throughput build, the reader throughput of `prev_before` on the optimistic path, divided by the throughput of the same probe stream through `with_locked`. The ratio is paired within each round.
  - The gate cell is W = 1, R = 4, uniform. The W = 0, R = 1 cell is the control, reported and not gated.
  - Verdict per run: `SINGLE_RUN_PASS` when the BCa 95% lower bound is above 1.0, `REJECTED` when the upper bound is below 1.0, `INCONCLUSIVE` otherwise. A claim needs the same verdict in two independent runs (`docs/BENCHMARKING.md` rule 18).

### 12.4 Instruments and cells

- **Reader mode.** `writer_scaling.rs` and `writer_scaling.py` gain `--readers R`, `--read-op get|prev_locked|prev` and `--probe uniform|hotspot`.
  - With W ≥ 1, readers start at the writers' barrier and probe until the writers join. With W = 0, each reader makes 2^20 probes.
  - Rows add `readers`, `read_op`, `probe`, `reader_ops` and `reader_elapsed_s`. In the counters role they also add `read_ops`, `read_attempts`, `read_fallbacks` and `locked_reads`.
- **Probes.**
  - `uniform`: each reader draws present prefill keys, shuffled per reader, and asks `prev_before`.
  - `hotspot`: probe keys and the writers' fresh keys come from one 2^16-wide expanse, so writes land in the subtrees the searches backtrack through.
- **Cells.** (W, R) ∈ {(0, 1), (0, 4), (1, 4), (4, 4)}, crossed with `read_op` ∈ {`prev_locked`, `prev`} and `probe` ∈ {`uniform`, `hotspot`}.
  - 8 rounds per cell, the driver's default, with (`read_op` × W × R) interleaved within each round.
  - Pin `0,2,4,6,8,10,12,14`, recorded in the artifact, so each thread gets its own physical P-core (AGENTS.md §8.20.5 step 0; every thread here does work). W + R ≤ 8.
- **Suite.** `writer_scaling_ordered_readers`, wired at every point AGENTS.md §2.7 item 3 lists: `.github/bench-suites.json`, and the dispatch `case`, flag spelling and upload paths in `bench_baremetal.yml`. Artifact `results/ordered_readers_writer_scaling.json`, two runs.
- **Callgrind.** `sync_map_prev/random` beside `sync_map_prev_locked/random` in `benches/instructions.rs`, registered in `scripts/perf_report.py`.

### 12.5 What voids a cell

§6 applies. In addition, a cell is void if:

- it was read before every §12.2 gate passed on the measured head;
- it is a throughput cell from an `occ-stats` build, or a counters cell from a default build (the two roles never share a binary);
- its harness rows record a pin other than `0,2,4,6,8,10,12,14`.

Threshold, method and round count are fixed here. Changing any of them after a run relabels that run `INTERMEDIATE` (AGENTS.md §8.19).

### 12.6 Explicitly not predicted

- **The other four operations,** `first`, `last`, `next_at_or_after`, `next_after` and `prev_at_or_before`, separately. They derive from the two walks, and §12.2's gates and the Callgrind arms cover them; no wall-clock cell does.
- **Magnitudes** for P12.3 and P12.5.
- **Reader cells at W ≥ 2** beyond reporting.
- **The 32-bit surface's wall clock,** which is measured on-device, not here.
- **Ordered reads on `SyncExpanseSet`, and rank or select.**
- **The RocksDB consumer arm.** It is pre-registered in `docs/benchmarks/rocksdb_memtable/METHODOLOGY.md` before its runs, once the C ABI exists.

### 12.7 Outcomes (appended 2026-09-14; §12.1–§12.6 are not edited)

**P12.2 — REFUTED on `sync_set_contains/random`; holds on `sync_map_get/random`** (#928, which moved both readers onto `Shared::optimistic_read`).

| arm | bound | result | verdict |
|---|---|---|---|
| `sync_map_get/random` | at most 0.1% change | 0.00% | holds |
| `sync_set_contains/random` | at most 0.1% change | −0.95% | **REFUTED** |

*(measured: CI `instruction-counts`, #928 head `4364337d` against `4305add8`, [run 34891445998](https://github.com/orieg/expanse/actions/runs/34891445998))*

The bound has no direction, so fewer instructions refute it just as more would. The set arm is not reclassified as a pass (AGENTS.md §8.19).

**Attribution** (AGENTS.md §6). *(measured: iai-callgrind 0.16.1 on an x86_64 Linux host, rustc 1.98.0, the same two commits; per-function `callgrind_annotate --inclusive=no` and `objdump -d -C` of both bench binaries)*

- **Reproduced.** The set arm's total fell from 14,866,219 to 14,725,106 instructions (−141,113). The map arm did not move.
- **One function.** The whole change sits in `SetReader::contains` and the code inlined into it:

  | inlined source | Δ instructions |
  |---|---|
  | `sync.rs` | −191,113 |
  | `core/src/macros` | −100,000 |
  | `bits.rs` | −50,000 |
  | `atomic.rs` | +50,000 |
  | `occ.rs` | +50,000 |
  | `set.rs` | +100,000 |

  Data reads fell by 208,929 and data writes by 100,021 in the same profiles.
- **The source that changed** is the call site, not the walk. `walk_validated` and the node code are identical in both commits. Base matched the walk's `Result` and then converted with `r.is_some()`. Head converts inside the walk closure, `walked.map(|r| r.is_some())`, and the helper returns the `Ok` value unchanged. `map_get_with` returns the walk's result with no conversion, and its count did not move.
- **The disassembly changed with it.**
  - **Size.** `SetReader::contains` is 805 instructions in base and 789 in head. `map_get_with` is 978 and 976, differing only in padding and branch offsets. Both builds call the same seven targets out of line, `leaf::search` among them, so no inlining decision changed.
  - **Base exits.** Every walk exit stores its answer in `%rax` and branches to one shared block, reached by seven branches. That block converts the answer to a bool (`test %rax,%rax; setne`), writes `INACTIVE` to the reader's slot (the pin's drop, `occ.rs:1932@58565660`) and jumps to the epilogue. The bitmap exit reaches it through `bt`/`setb`.
  - **Head exits.** Each exit writes the bool directly, with its own pin release and jump, plus one `and $0x1` at the epilogue. The shared block and the `bt` are gone.
  - **The stack slot.** Base keeps a 32-bit value in `0x4(%rsp)`, with 21 references. Head holds it in a register, with 0 references. That fits the fall in data reads and writes.
- **Not established.**
  - How the 141,113 splits between the exit layout and the stack slot. Per-instruction execution counts (`--dump-instr=yes`) were not collected.
  - Why the code generator chose the different layout.

### 12.8 Addendum to §12.7 (appended 2026-09-14; §12.1–§12.7 are not edited)

**P12.2's map verdict is an instruction-count verdict.** §12.3 states P12.2 over Callgrind instruction counts, and §12.7's "holds" on `sync_map_get/random` is a statement on that instrument. P12.2 made no wall-clock prediction, and neither verdict in §12.7 changes.

On the reference host, the same change (#928) cost the 64-bit `SyncExpanseMap` reader throughput in `masstree_concurrent` with readers only, while instructions per read stayed flat. Per-thread reader counters placed the difference in `ld_blocks.store_forward` per read, and `perf record` on that event placed it in `walk_validated::<true>`. #949 inlines `walk_validated`, and its body carries the measurement: two interleaved runs with per-read counters. The figures are not repeated here because that measurement is diagnostic and has no committed artifact (AGENTS.md §8.7). What remains against the pre-#928 build after #949 has an unmeasured cause.

**For the §12.4 runs.** Ordered reads go through the same `Shared::optimistic_read`. The P12.4 and P12.5 runs name the commit they measure relative to #949, and record `ld_blocks.store_forward` and cycles per read beside the §12.4 counters as diagnostics. They are not verdict inputs, and no bound or cell in §12.3–§12.5 changes.

### 12.9 Outcomes of P12.4 and P12.5 (appended 2026-09-15; §12.1–§12.8 are not edited)

*(measured: reference host — Intel Core i9-12900F, 8P+8E / 24 threads; pin `0,2,4,6,8,10,12,14`; head `5228fc3a`; 8 rounds per cell in two independent runs, [34929950085](https://github.com/orieg/expanse/actions/runs/34929950085) and [34930077887](https://github.com/orieg/expanse/actions/runs/34930077887); `results/ordered_readers_writer_scaling.json` and `results/ordered_readers_writer_scaling_run2.json`; workload: `concurrency_ordered_readers_map_64bit`)*

**§12.2 gates before any cell was read.** G12.1–G12.3 landed with #940 and G12.5 with #945. G12.4 holds on `5228fc3a`: Loom and ASan passed on that head's CI run [34929871313](https://github.com/orieg/expanse/actions/runs/34929871313), and TSan passed on nightly run [34930036976](https://github.com/orieg/expanse/actions/runs/34930036976) at the same head. The artifacts record the §12.4 pin, and neither run voids a cell (§12.5).

**P12.4 — HOLDS in both runs.** Every `prev` cell's `read_fallbacks ÷ read_ops` is below 0.1%. The largest is `hotspot`, W = 4, R = 4 in run 2: 36 of 1,994,749 reads fell back.

| cell | fallbacks / read ops, run 1 | run 2 | attempts per op, run 1 / run 2 | P12.4 |
|---|---|---|---|---|
| `uniform`, W = 0, R = 1 | 0 / 8,388,608 | 0 / 8,388,608 | 1.000 / 1.000 | holds |
| `uniform`, W = 0, R = 4 | 0 / 33,554,432 | 0 / 33,554,432 | 1.000 / 1.000 | holds |
| `uniform`, W = 1, R = 4 | 7 / 97,540,008 | 2 / 96,961,628 | 1.002 / 1.002 | holds |
| `uniform`, W = 4, R = 4 | 10 / 46,721,061 | 8 / 45,313,985 | 1.004 / 1.004 | holds |
| `hotspot`, W = 0, R = 1 | 0 / 8,388,608 | 0 / 8,388,608 | 1.000 / 1.000 | holds |
| `hotspot`, W = 0, R = 4 | 0 / 33,554,432 | 0 / 33,554,432 | 1.000 / 1.000 | holds |
| `hotspot`, W = 1, R = 4 | 29 / 2,758,371 | 29 / 2,879,230 | 1.721 / 1.725 | holds |
| `hotspot`, W = 4, R = 4 | 10 / 1,997,823 | 36 / 1,994,749 | 2.053 / 2.027 | holds |

Attempts per operation are reported and not gated (§12.3). The two `hotspot` cells with writers exceed 1.00–1.99, the range `scripts/olc_bounds.py` `ordered_projection` gives under per-node independence from the get-path health cells of `58565660` and `a1982ff2` at 4 writers. Those cells concentrate writes in the subtrees each search backtracks through, which is the case that hypothesis does not cover. Neither the attempts nor the fallbacks decide P12.4 beyond the ceiling above.

**P12.5 — `SINGLE_RUN_PASS` in both runs, so the claim holds** (`docs/BENCHMARKING.md` rule 18). At W = 1, R = 4, `uniform`, the optimistic `prev_before` reads 91.81 [85.52, 96.74] and 89.47 [84.24, 93.75] times the throughput of the same probe stream through `with_locked`, per-round paired, BCa 95% (workload: `concurrency_ordered_readers_map_64bit`). The W = 0, R = 1 control reads 2.15 [2.14, 2.16] and 2.13 [2.12, 2.14]. Every `with_locked` call takes the fallback mutex, quiesces writers and takes the writer mutex (`Shared::with_locked`), so the gate cell's ratio includes the time the locked readers and the writer wait on each other. How the ratio splits between waiting and work per read is unmeasured.

| cell | `prev` ÷ `prev_locked`, run 1 | run 2 | role |
|---|---|---|---|
| `uniform`, W = 0, R = 1 | 2.15 [2.14, 2.16] | 2.13 [2.12, 2.14] | control |
| `uniform`, W = 0, R = 4 | 16.18 [15.86, 16.41] | 16.42 [16.27, 16.55] | reported |
| `uniform`, W = 1, R = 4 | 91.81 [85.52, 96.74] | 89.47 [84.24, 93.75] | gate |
| `uniform`, W = 4, R = 4 | 353.12 [331.88, 377.93] | 365.58 [352.44, 385.31] | reported |
| `hotspot`, W = 0, R = 1 | 2.50 [2.50, 2.51] | 2.52 [2.49, 2.63] | reported |
| `hotspot`, W = 0, R = 4 | 17.69 [17.24, 18.17] | 17.98 [17.64, 18.48] | reported |
| `hotspot`, W = 1, R = 4 | 8.71 [8.20, 9.30] | 7.76 [7.47, 8.43] | reported |
| `hotspot`, W = 4, R = 4 | 11.38 [10.07, 12.47] | 12.38 [11.14, 15.19] | reported |

**The hotspot geometry,** fixed before the runs in #951, prefills offset 1 of every 256-key block of one 2^16 expanse and probes `prev_before` on those keys, so every probe begins with a sibling descent. Writers insert the other 65,280 keys; the 255 offset-0 keys of blocks 1–255 turn a probe into a same-block answer as they land. The share of probes still backtracking during a window is unmeasured.

**What this leaves for #900:** the C ABI and bindings (step 8), the Python and Node reads (step 9), and the RocksDB consumer arm (step 10). No magnitude was predicted for P12.5 (§12.6), and none is claimed beyond the intervals above.


## 13. Pre-registration for #568 Steps 1–2 — the single-writer baseline and the 50/50 16-thread gate (appended 2026-09-14, locked before any baseline or gate run)

#568's Gate section asks for a single-writer baseline on the stated workload shape (Step 1) and a pre-registered 50/50, 16-thread mixed-throughput gate whose BCa 95% lower bound clears that baseline by a stated margin (Step 2). Neither had text until this section. The bound functions are in `scripts/olc_bounds.py` (`ratio_cv`, `rounds_for_halfwidth`, `ratio_needed_to_clear`, `mixed_window_spread`), which read every input from the committed artifacts named below. Nothing here is rewritten in place once a run exists (AGENTS.md §8.7); a threshold, method or round count changed after a run relabels that run `INTERMEDIATE` (§8.19).

### 13.1 What is compared

- **Baseline build: `1edfa952`.** It is the last commit before `07f0de43`, which landed multi-writer optimistic lock coupling for `SyncExpanseSet` and `SyncExpanseMap`; `07f0de43` is its only descendant on that path (`git rev-list --count 1edfa952..07f0de43` is 1). Its measured half is Step 1's single-writer baseline.
- **Head build:** `main` at run time, named in the artifact. It must contain #949, because the mixed cells include readers and #949 removes a reader stall the head would otherwise carry.
- **One harness for both.** Both builds run the head's `crates/expanse/benches/concurrency.rs`; only `crates/expanse` differs. That file compiles unchanged against `1edfa952` (a `cargo bench -p expanse-trie --bench concurrency --no-run` build on the reference host).
- **Arms and mix.** `map` (`SyncExpanseMap`) and `set` (`SyncExpanseSet`) at `EXPANSE_BENCH_WORKLOADS=50` (workload: `core_concurrency`): a 1M-draw prefill over a 2M keyspace; each operation reads with probability one half and otherwise inserts the drawn key when it is even and removes it when it is odd.

### 13.2 Prior observations at lock time (mandatory disclosure)

- **What was read.** #935's two head-only runs, `results/baseline_concurrent_mixed.json` and `results/baseline_concurrent_mixed_run2.json`, were read before this section was written. Their 50/50 cells at 16 threads averaged 62.52 and 62.23 M ops/s on `map` and 82.94 and 84.81 M ops/s on `set` *(measured: reference host — Intel Core i9-12900F, pin `0-15`, `76432c5c`; workload: `core_concurrency`)*. No baseline window of any kind has been measured, so no head/baseline ratio has been seen. Those artifacts size the rounds below and are not inputs to any verdict.
- **An order effect in those runs.** In both runs and on both arms, the three lowest of the eighteen 16-thread windows are rounds 3, 9 and 15, and each of them opens its round directly after a 4-thread window (map 26–40 against 53–76 M ops/s for the other fifteen; set 56–64 against 67–106) (workload: `core_concurrency`; `results/baseline_concurrent_mixed.json`, `results/baseline_concurrent_mixed_run2.json`). The cause is unmeasured. The bench runs every window of a process over one prefilled structure, and head and baseline complete different numbers of operations per window, so an effect carried from one window to the next would not cancel in their ratio. That is why §13.3 gives every window its own process and prefill.

### 13.3 Instrument and cells

- **Two-build mode of `scripts/mixed_concurrency.py`**, landed and self-tested before any run. It builds the bench from the head tree and from `1edfa952` with the head's bench file, applies the core pin, and runs one process per window: `EXPANSE_BENCH_ENGINES` one arm, `EXPANSE_BENCH_THREADS` one thread count, `EXPANSE_BENCH_ROUNDS=1`, so every window starts from a fresh prefill.
- **Cells.** Threads ∈ (1, 16) on `map` and `set`. 16 threads is the gate cell; 1 thread is the control.
- **Rounds: 48 per run.** `rounds_for_halfwidth` gives 47 for a relative half-width of 0.10, taking the largest per-window CV of the 16-thread cells above (0.247) for both builds and no correlation between them (`python3 scripts/olc_bounds.py`); 48 is the next even count, so each build order occurs equally often.
- **Order within a round.** Every (arm, threads, build) window once; the build order alternates between rounds (head first, then baseline first), and so does the thread order. A load snapshot is taken around every round (§8.17).
- **Pin `0-15`,** as in the #935 runs, recorded in the artifact: sixteen threads on the eight P-cores' sixteen logical CPUs (AGENTS.md §8.20.5 step 0).
- **Statistic.** Per arm and thread count, the per-round ratio head / baseline of total operations per second (read plus write operations over the window's own elapsed time), and the BCa 95% interval of its mean (`scripts/bca_bootstrap.py`, at least 1,000 resamples). Each build's own mean and BCa interval are published beside it; the baseline's is Step 1's figure.
- **Two independent runs,** each in its own session from fresh builds (`docs/BENCHMARKING.md` rule 18).

### 13.4 The gate and its verdicts

- **Per arm, per run, at 16 threads:** `PASS` when the lower bound is at least 1.5; `REFUTED` when the upper bound is below 1.5; `INCONCLUSIVE` otherwise.
- **#568's gate is met** when `map` and `set` both read `PASS` in both runs. A `REFUTED` on either arm in either run fails it. An `INCONCLUSIVE` leaves it unmet; a further run added to decide it is a change of sample size and relabels the result `INTERMEDIATE` (§8.19).
- **The margin 1.5 is a choice fixed here, not a derivation.** At the planned relative half-width of 0.10 the true ratio must be at least 1.667 for the lower bound to reach 1.5 (`ratio_needed_to_clear`), so a real gain between 1.5 and about 1.67 can read `INCONCLUSIVE`. That is the instrument's resolution, stated before the runs.
- **The 1-thread control** is reported with its interval and not gated: it is what the multi-writer path costs a single thread under this mix.
- **P5.1–P5.4** stand as recorded in `docs/benchmarks/concurrency/README.md` §9; this section adds no P5 cell.

### 13.5 What voids a cell

§6 applies. In addition:

- a round in which either build's process exits non-zero, or a window reports a thread count, arm or read percentage other than the one requested, is void;
- a run is void if its head does not contain #949, if its baseline tree differs from `1edfa952` in anything but `crates/expanse/benches/concurrency.rs`, or if its artifact records a pin other than `0-15`.

A run with any void round is discarded whole and re-run from fresh builds, and the discard is disclosed beside the result (§8.17).

### 13.6 Explicitly not predicted

- The 100% and 95% read mixes, and 2, 4 and 8 threads.
- The string, bytes and blob wrappers (#929), and the `writer_scaling` throughput target (#930).
- Any comparison with a third-party structure.
- A magnitude beyond the gate's bound.

### 13.7 Outcomes (appended 2026-09-15; §13.1–§13.6 are not edited)

*(measured: reference host — Intel Core i9-12900F, 8P+8E / 24 threads; pin `0-15`; head `5228fc3a` against `1edfa952` with the head's `crates/expanse/benches/concurrency.rs`; 48 rounds of one-process windows per run in two independent runs, [34930107714](https://github.com/orieg/expanse/actions/runs/34930107714) and [34930136807](https://github.com/orieg/expanse/actions/runs/34930136807); `results/baseline_concurrent_step2_gate.json` and `results/baseline_concurrent_step2_gate_run2.json`; workload: `core_concurrency`)*

**No cell is void (§13.5).** Both runs record a head that contains #949 and a baseline tree that differs from `1edfa952` in `crates/expanse/benches/concurrency.rs` alone. Both record pin `0-15`, and every window reports the arm, thread count and read percentage it was asked for. The largest foreign busy-CPU figure over any round is 0.01 in run 1 and 0.00 in run 2.

**#568's gate is met: `PASS` on both arms in both runs (§13.4).** Per-round paired head ÷ baseline total operations per second at 16 threads, BCa 95% (workload: `core_concurrency`):

| arm, 16 threads | run 1 | run 2 | verdicts |
|---|---|---|---|
| `SyncExpanseMap` | 4.08 [4.01, 4.25] | 4.12 [4.04, 4.30] | `PASS`, `PASS` |
| `SyncExpanseSet` | 8.85 [8.31, 9.20] | 8.55 [7.98, 8.95] | `PASS`, `PASS` |

Every lower bound clears the 1.5 margin, and the 1.667 that §13.4 names as the planned resolution. The margin was a choice fixed before the runs.

**Step 1, the single-writer baseline, and the head beside it** (total M ops/s, mean [BCa 95%], workload: `core_concurrency`):

| arm | threads | `1edfa952`, run 1 | `1edfa952`, run 2 | head, run 1 | head, run 2 |
|---|---|---|---|---|---|
| `SyncExpanseMap` | 1 | 26.96 [26.93, 26.99] | 27.00 [26.95, 27.03] | 24.93 [24.88, 24.97] | 24.92 [24.86, 24.97] |
| `SyncExpanseMap` | 16 | 5.44 [5.41, 5.46] | 5.42 [5.38, 5.45] | 22.18 [21.79, 23.01] | 22.33 [21.87, 23.37] |
| `SyncExpanseSet` | 1 | 44.37 [44.34, 44.40] | 44.34 [44.27, 44.38] | 41.47 [41.44, 41.50] | 41.37 [40.89, 41.48] |
| `SyncExpanseSet` | 16 | 6.08 [6.03, 6.11] | 6.10 [6.06, 6.14] | 53.71 [50.49, 55.83] | 52.15 [48.63, 54.52] |

**The 1-thread control, reported and not gated.** Head ÷ baseline reads 0.92 [0.92, 0.93] and 0.92 [0.92, 0.93] on `map`, and 0.93 [0.93, 0.94] and 0.93 [0.92, 0.94] on `set` (workload: `core_concurrency`). Under this mix a single thread runs slower on the multi-writer engine than on `1edfa952`. That cost is what the 16-thread gain is set against. Its cause is unmeasured.

**Not established.**
- Why `set` gains more than `map` at 16 threads.
- The 2-, 4- and 8-thread cells, and the 100% and 95% read mixes (§13.6).
- Whether the order effect disclosed in §13.2 would have moved these ratios. The instrument ran every window in its own process to keep any such effect out of them, so it does not measure that effect.

## 14. Pre-registration for #930 — `SyncExpanseMap` and `SyncExpanseSet` writer throughput ≥ 20 M ops/s at W = 8 (appended 2026-09-15, locked before any run toward the target)

#930's first step asks for the target to be registered before any run. This section fixes the threshold, statistic, pins, round count and void rules. It predicts no outcome. The sizing functions are in `scripts/olc_bounds.py`: `w8_round_spread`, `planning_cv`, `rounds_for_williams`, `mean_needed_to_clear`, `mean_needed_with_probability` and `writer_target_verdict`. Their inputs are frozen in `W8_SPREAD_AT_LOCK`, and `check_frozen_spread` reproduces them from four committed sizing artifacts (§14.3), so a later edit to those files cannot move the registered sizing (AGENTS.md §8.7). Nothing here is rewritten once a run exists. A threshold, statistic, pin or round count changed after a run relabels that run `INTERMEDIATE` (§8.19).

### 14.1 The target

- **Arms and workloads.** `map` (`SyncExpanseMap`, workload `concurrency_writer_map_64bit`) and `set` (`SyncExpanseSet`, workload `concurrency_writer_set_63bit`) in `crates/expanse/examples/writer_scaling.rs`: a 2^20-key prefill, then 2^20 fresh keys inserted by W writers. Inserts only.
- **The gate cell is W = 8.** Its statistic is the one the driver already writes. `expanse_writer_mops_mean` is the mean of the run's per-round `writer_mops`, and `writer_ci_lower` and `writer_ci_upper` are its BCa 95% interval (`scripts/bca_bootstrap.py`, 2,000 resamples).
- **The threshold, 20 M ops/s, is #930's choice, not a derivation.** The issue's USL table is stated there as reasoning, not a prediction, and nothing in this section depends on it.
- **Both pins are part of the gate:** `0-15` and `0,2,4,6,8,10,12,14`. A cell is never pooled across pins.
  - `0-15` is the pin of #930's table and of every writer-sweep artifact committed before `bad1bd3d` except #912's pair. #965's comparison re-runs at `bad1bd3d` ran at `0,2,4,6,8,10,12,14`, and the sizing sweeps (§14.3) ran both pins. At W = 8 it lets two writers share one physical core's SMT siblings, and the effect of that on the W = 8 cells is unmeasured (`README.md` §10).
  - `0,2,4,6,8,10,12,14` places one writer per physical P-core (AGENTS.md §8.20.5 step 0), and #912's gate was measured under it.
  - §11.5's one-per-core re-run moved no C(W) interval, but it measured C(W), not absolute W = 8 throughput, on an engine that predates #912. At `bad1bd3d` the sizing sweeps' W = 8 intervals overlap across the two pins (§14.2). Which pin reads higher on an evaluated head is not predicted (§14.6).
  - **Why both, not each reported separately.** The target is a statement about writer throughput at W = 8, and it names no placement. If one pin were the gate and the other only reported, #930 could close on a `PASS` under one placement beside a `REFUTED` under the other. Requiring both keeps the claim to what both placements measure. The cost is twice the runs.

### 14.2 Prior observations at lock time (mandatory disclosure)

- **#930's own table.** Two runs at pin `0-15`, CI runs [34766142088](https://github.com/orieg/expanse/actions/runs/34766142088) (`270f87f1`) and [34766103296](https://github.com/orieg/expanse/actions/runs/34766103296) (`cb308f71`), both before #912. At W = 8: `map` 10.40 [10.16, 10.62] and 10.31 [10.11, 10.48] M ops/s (workload: `concurrency_writer_map_64bit`); `set` 12.58 [12.45, 12.74] and 12.55 [12.38, 12.65] M ops/s (workload: `concurrency_writer_set_63bit`). No committed artifact holds these runs. They are quoted from the issue and are not inputs to any function above.
- **The writer-sweep artifacts committed before `bad1bd3d`, as context.** The fifteen writer-sweep files in `results/` and the two `ordered_readers_*` files, which hold reader cells only, hold 48 W = 8 `map` and `set` cells, default and variant builds alike: 40 at pin `0-15` (engines `e0b287f2` to `1f465728`) and 8 at `0,2,4,6,8,10,12,14` (`5cf94b17`). Every one ran all the W cells of a round in one harness process (§15.3), and none is a sizing input. The highest W = 8 means at each pin:
  - **Pin `0-15`.** The `ablation-striped-epoch` builds at `1f465728`, the build §11.5 describes becoming the default: `map` 10.55 [10.29, 10.77] and 10.40 [10.12, 10.65] M ops/s (workload: `concurrency_writer_map_64bit`); `set` 12.38 [12.30, 12.48] and 12.33 [12.17, 12.51] M ops/s (workload: `concurrency_writer_set_63bit`). *(measured: reference host, `1f465728`; `results/ablation_epoch_writer_scaling_post4e.json`, `results/ablation_epoch_writer_scaling_post4e_run2.json`)*
  - **Pin `0,2,4,6,8,10,12,14`.** The default (per-stripe freelist) builds at `5cf94b17`: `map` 11.48 [11.24, 11.76] and 11.38 [11.21, 11.66] M ops/s (workload: `concurrency_writer_map_64bit`); `set` 13.47 [13.25, 13.61] and 13.39 [13.19, 13.57] M ops/s (workload: `concurrency_writer_set_63bit`). *(measured: reference host, `5cf94b17`; `results/ablation_unstriped_freelist_writer_scaling.json`, `results/ablation_unstriped_freelist_writer_scaling_run2.json`)*
- **Main at lock, measured by the sizing sweeps.** The four sizing runs (§14.3) measured main at `bad1bd3d`: the default build, one harness process per timed cell, 8 rounds. At W = 8:
  - **Pin `0-15`, run 1 and run 2.** `map` 11.51 [11.48, 11.56] and 11.41 [11.21, 11.60] M ops/s (workload: `concurrency_writer_map_64bit`); `set` 13.38 [13.24, 13.52] and 13.25 [12.98, 13.49] M ops/s (workload: `concurrency_writer_set_63bit`). *(measured: reference host, `bad1bd3d`; `results/sizing958_writer_scaling_bad1bd3d_pin0-15.json`, `results/sizing958_writer_scaling_bad1bd3d_pin0-15_run2.json`)*
  - **Pin `0,2,4,6,8,10,12,14`, run 1 and run 2.** `map` 11.39 [11.09, 11.50] and 11.51 [11.43, 11.60] M ops/s (workload: `concurrency_writer_map_64bit`); `set` 13.26 [13.12, 13.44] and 13.32 [13.23, 13.50] M ops/s (workload: `concurrency_writer_set_63bit`). *(measured: reference host, `bad1bd3d`; `results/sizing958_writer_scaling_bad1bd3d_percore.json`, `results/sizing958_writer_scaling_bad1bd3d_percore_run2.json`)*
  - **Main sits well below 20 M ops/s.** The target is 1.74–1.76× these `map` means and 1.49–1.51× these `set` means (target ÷ mean, `python3 scripts/olc_bounds.py`), and 1.92–1.94× and 1.59× #930's table. No prediction is made from these levels.
  - `5cf94b17`, where the context figures above were measured, is the head #912 was measured at. It is not an ancestor of #912's merge commit `228ef94d`, and `crates/expanse/src/occ.rs` differs between the two. Between `228ef94d` and `bad1bd3d`, #928, #938, #940, #945, #949, #956, #957 and #962 changed `crates/expanse/src`. The sizing sweeps measure the result at `bad1bd3d`, not the effect of any one change.
- **Variant builds at `bad1bd3d`, one process per cell (#965, `README.md` §11.8), as context.** Six two-build comparison runs, pin `0,2,4,6,8,10,12,14`, 8 rounds, two runs per variant. Their default-build W = 8 cells read 11.38–11.72 M ops/s on `map` and 13.18–13.62 on `set`. The variant W = 8 cells, run 1 and run 2:
  - **`ablation-sharded-alloc`.** `map` 13.97 [13.67, 14.40] and 14.37 [14.10, 14.54] M ops/s (workload: `concurrency_writer_map_64bit`); `set` 15.09 [14.91, 15.37] and 14.97 [14.60, 15.19] M ops/s (workload: `concurrency_writer_set_63bit`). *(measured: reference host, `bad1bd3d`; `results/ablation_alloc_writer_scaling_bad1bd3d.json`, `results/ablation_alloc_writer_scaling_bad1bd3d_run2.json`)*
  - **`lock-padded`.** `map` 12.14 [11.90, 12.56] and 12.16 [11.91, 12.59] M ops/s (workload: `concurrency_writer_map_64bit`); `set` 17.32 [17.19, 17.55] and 17.29 [17.07, 17.54] M ops/s (workload: `concurrency_writer_set_63bit`). *(measured: reference host, `bad1bd3d`; `results/padded_writer_scaling_bad1bd3d.json`, `results/padded_writer_scaling_bad1bd3d_run2.json`)*
  - **`ablation-sharded-alloc,lock-padded`.** `map` 24.27 [23.78, 24.54] and 23.60 [22.90, 24.08] M ops/s (workload: `concurrency_writer_map_64bit`); `set` 32.33 [31.23, 33.15] and 33.08 [32.67, 33.52] M ops/s (workload: `concurrency_writer_set_63bit`). *(measured: reference host, `bad1bd3d`; `results/combined_alloc_padded_writer_scaling_bad1bd3d.json`, `results/combined_alloc_padded_writer_scaling_bad1bd3d_run2.json`)*
  - **What this does and does not say.** The combined build's W = 8 lower bounds are above 20 M ops/s on both arms, in both runs, at one-per-core. It is a feature build, not the suite's default build, and these are comparison runs at one pin, not evaluations under §14.3–§14.5. No verdict is read from them. Promoting either feature is an engine change on #930's path and keeps its own gates. Whether a head that carries both features by default reaches the target at both pins is not predicted (§14.6).
  - None of these cells is a sizing input. A comparison run interleaves two builds, 2 × len(W) cells per round, which is a different schedule from the single-build sweep §14.3 evaluates.
- **What this section is, then:** a target fixed for engine changes that have not been made. It does not replace their gates. Each change on #930's path is still gated by its own paired C(W) ratio (AGENTS.md §8.20.2) and its Callgrind bounds, as #930's "Work, in order" states. A change is never credited with progress because a W = 8 level moved.
- **The sizing inputs are main at lock, on one host, from 8 rounds each.** The largest per-round CV at `0-15`, 0.02939, is `set` in the second run. A CV taken from 8 rounds is itself uncertain, and it says nothing about the spread on a future head. A head with wider per-round spread resolves less finely at the registered count, and the count does not change for it (§14.4).
- **The competitor suites, as context only.** #952 re-published the HOT and Masstree concurrent arms at harness commit `6f8d6ba5` (run 1 of two, pin `0-15`, 15 rounds). They are a different binary (`crates/expanse-hot-bench`) and a different harness, report a ratio against a competitor, and are not an input here. At W = 8, Masstree inserts at 32.73 M ops/s against `SyncExpanseMap`'s 11.56, ratio 0.350 [0.344, 0.355] (workload: `masstree_conc_map_64bit`; `docs/benchmarks/masstree_comparison/results/baseline_concurrent.json`). HOT-ROWEX inserts at 24.97 against `SyncExpanseSet`'s 13.50, ratio 0.546 [0.531, 0.560] (workload: `hot_rowex_set_63bit`), and at 15.04 against `SyncExpanseMap`'s 10.94, ratio 0.715 [0.693, 0.733] (workload: `hot_rowex_map_64bit`; `docs/benchmarks/hot_comparison/results/baseline_concurrent.json`). *(Note appended after lock, not an edit of the observation: both suites' live names were since re-measured at `929574b5` and the figures quoted here are superseded there; the `6f8d6ba5` pair this bullet reads is kept at `results/at_6f8d6ba5/` in each suite.)*

### 14.3 Instrument and cells

- **The `writer_scaling` suite, run by the per-cell driver (§15).** Every timed writer cell runs in a harness process of its own, and the artifact records `provenance.cell_isolation` as `"process"`. Throughput comes from the uninstrumented build, with the W ∈ {1, 2, 4, 8} cells of round r following row r of the driver's Williams design over the four writer counts. The `map`, `set` and `str` arms run as the suite runs them, and the `occ-stats` counters pass runs as usual. Only the W = 8 `map` and `set` cells are verdict inputs.
- **One dispatch per pin per run.** The `cpu_pin` input is `0-15` or `0,2,4,6,8,10,12,14`, and the artifact records the applied pin in `provenance.core_pin`.
- **The sizing inputs.** Four runs of the default build through `writer_scaling.py` without `--compare`: 8 rounds, one harness process per timed cell, engine, harness and driver at `bad1bd3d`, on the reference host. Two runs per pin, interleaved by pin (`0-15`, one-per-core, `0-15`, one-per-core, by their snapshot times). *(measured: reference host, `bad1bd3d`; `results/sizing958_writer_scaling_bad1bd3d_pin0-15.json` and `_pin0-15_run2.json` at pin `0-15`, `results/sizing958_writer_scaling_bad1bd3d_percore.json` and `_percore_run2.json` at pin `0,2,4,6,8,10,12,14`)*
  - Every W = 8 `map` and `set` cell records 8 rounds and `writer_ci_method` `bca`, and every file records `cell_isolation` `"process"`.
  - Against §14.5, none is void. `load.foreign_busy_cpus` is at most 0.01 on every `throughput` result, each read over its arm's whole writer pass (`wall_s` 17.3–17.6 s for `map` and `set`). Run-start load averages are 0.00 and 1.14 at `0-15`, and 1.07 and 1.11 at one-per-core. No snapshot exceeds 2.34.
  - `W8_SPREAD_AT_LOCK` freezes the eight W = 8 cells' means and per-round CVs. `check_frozen_spread` reproduces all eight from the files and lists any file whose `cell_isolation` is not `"process"`.
- **Why not the first sizing.** The first draft of this section sized 40 rounds from the 48 W = 8 cells of the writer-sweep artifacts committed before `bad1bd3d`, whose largest per-round CV was 0.0783 at `0-15` (a default build at `10ce2f9d`). Every one of those ran all the W cells of a round in one harness process. §15 found a cell's throughput moving with the cell run before it in the same process, so that spread includes position-dependent carryover and may not describe cells measured one process each (§15.3). The target is evaluated with the per-cell driver, so the count is sized from spread measured that way. The re-sizing was made before any run toward the target.
- **Rounds: 8 per run, at both pins, derived.** The relative half-width 0.025 is a choice. The planning CV at each pin is the largest per-round CV among that pin's four frozen cells. `rounds_for_williams` rounds `rounds_for_halfwidth` up to a multiple of the four writer counts, so the single-build Williams design is complete.
  - At `0-15` the planning CV is 0.02939: (1.96 × 0.02939 / 0.025)² = 5.31, so 6 rounds, so 8.
  - At `0,2,4,6,8,10,12,14` it is 0.02318: (1.96 × 0.02318 / 0.025)² = 3.30, so 4 rounds, already a multiple of 4.
  - One count serves both pins, the larger: 8. It is derived, not a floor, and it equals the driver's default.
- **The dispatch.** The pull request that adds this section also adds a `rounds` input to `.github/workflows/bench_baremetal.yml`. It passes `--rounds` to the `writer_scaling` case only when set, and changes nothing else in the driver or harness. An empty value runs the driver's default of 8, and any other suite refuses a non-empty value. Each run under this section is one `workflow_dispatch` with `ref` naming the head being evaluated and:
  - pin `0-15`: `benchmark_suite=writer_scaling`, `rounds=8`, `cpu_pin=0-15`;
  - pin `0,2,4,6,8,10,12,14`: `benchmark_suite=writer_scaling`, `rounds=8`, `cpu_pin=0,2,4,6,8,10,12,14`.

  8 is also the driver's default, so a dispatch that leaves `rounds` empty runs the same count. The input stays: passing `rounds=8` puts the count in the dispatch record, and a later re-sizing needs no workflow change. §14.5 reads the count and the cell isolation from the artifact, not from the dispatch.
- **Resolution, stated before any run.** At the registered relative half-width of 0.025, a true mean of 20.513 M ops/s reaches the bound in about half of runs (`mean_needed_to_clear`) and 21.039 in 97.5% of runs (`mean_needed_with_probability`).
  - At 8 rounds the `0-15` planning CV gives a relative half-width of 0.0204, where the same two figures are 20.416 and 20.840 M ops/s.
  - For context only, the first draft's 40 rounds with these CVs would give 0.0091 at `0-15` (20.184 and 20.369) and 0.0072 at one-per-core (20.145 and 20.290).
  - Every figure here counts within-run spread only (`python3 scripts/olc_bounds.py`). A real level between 20 and about 21 M ops/s can therefore read `INCONCLUSIVE`.
- **Load.** The driver takes a snapshot around every arm (`begin_cell`, `end_cell`), recorded in `provenance.loads` and in each cell's `load` (AGENTS.md §8.17).
- **Two independent runs per pin** (`docs/BENCHMARKING.md` rule 18), each a fresh dispatch, all four at one head.
- **When a run is taken.** An evaluation is run only after an engine change that claims progress toward the target has met its own §8.20.2 gate, or when the maintainer asks for one. Every evaluation is appended as an outcome subsection whatever its verdicts, so the number of evaluations is visible.

### 14.4 The gate and its verdicts

- **Per arm, per pin, per run, at W = 8** (`writer_target_verdict`):
  - `PASS` when `writer_ci_lower` is at least 20.0 M ops/s;
  - `REFUTED` when `writer_ci_upper` is below 20.0;
  - `INCONCLUSIVE` otherwise.
- **The target is met at a head** when all eight cells read `PASS`: two arms × two pins × two runs. A `REFUTED` on any cell means the target is not met at that head. An `INCONCLUSIVE` leaves it unmet, and a further run added to decide it changes the sample size, which relabels that evaluation `INTERMEDIATE` (§8.19). #930 closes when the target is met.
- **An evaluation that does not meet the target changes nothing in this section.** A later head is evaluated with the same threshold, statistic, pins and round count.
  - At a true mean of exactly 20 M ops/s, each cell's nominal chance of a false `PASS` is 2.5%.
  - The eight cells share a head and a host and are not independent, so no joint rate is claimed.
  - Repeated evaluations raise the chance that some head passes by chance, which is why every evaluation is recorded (§14.3).
- **Reported, not gated:** the W = 1, 2 and 4 cells, C(W) and its paired interval, the `str` arm, and the counters pass.

### 14.5 What voids a cell

§6 applies. In addition, a run is void if:

- its `provenance.core_pin` is not the pin it was dispatched for, or is neither `0-15` nor `0,2,4,6,8,10,12,14`;
- it is a quick run: a W = 8 `map` or `set` cell records `prefill` or `fresh_keys` other than 1,048,576 (`--quick` uses 4,096);
- a W = 8 `map` or `set` cell records `rounds` other than 8, or a `writer_ci_method` other than `bca`;
- its `provenance.cell_isolation` is not `"process"`, so its timed cells shared a harness process (§15);
- any cell's `load.foreign_busy_cpus` is above 1.0 core-equivalents, any snapshot in `provenance.loads` has a load average above 12 (half the host's 24 logical CPUs), or the two runs at one pin start at load averages more than 2 apart (§6, AGENTS.md §8.17). The foreign-CPU rule reads `load.foreign_busy_cpus` on each `throughput` result. That window opens when the arm's writer pass starts (`since` `arm:<arm>:writers`) and closes when the pass ends, so it covers the arm's whole pass and every W cell of the arm carries the same value; it does not isolate one timed cell. The phase snapshots in `provenance.loads` (`foreign_busy_cpus_since_prev`) are not a void criterion: they can read spuriously in either direction, and the sizing runs recorded -0.76 and -0.50 there. Since #966 a window shorter than `bench_provenance.MIN_WINDOW_S` records `None` instead;
- its `provenance.commit` does not contain the engine change the evaluation is taken for (`git merge-base --is-ancestor <change> <commit>`);
- the four runs of one evaluation differ in `crates/` or in `docs/benchmarks/concurrency/scripts/writer_scaling.py` (`git diff --quiet <a> <b> -- crates/ docs/benchmarks/concurrency/scripts/writer_scaling.py`);
- its timings come from an `occ-stats` build. The driver refuses to produce one, so this is listed for completeness.

A void run is discarded whole and replaced by a fresh dispatch at the same head, and the discard is disclosed beside the result (§8.17). Replacing a void run does not change the sample size.

### 14.6 Explicitly not predicted

- That any particular change reaches the target, #930's steps 3 and 4 included, or by how much any change moves the W = 8 level.
- C(W) at any W, and throughput at W ∈ {1, 2, 4}. Each change's own paired C(W) gate stands (AGENTS.md §8.20.2).
- Which pin reads higher at W = 8.
- The `str` arm, and the string, bytes and blob wrappers' writers (#929).
- Remove-heavy or mixed writer scaling. `writer_scaling` measures inserts only.
- Any comparison with HOT-ROWEX or Masstree. #930's step 5 re-runs those suites, and no competitor figure is an input to §14.

## 15. One harness process per timed writer-scaling cell (appended 2026-09-15; §1–§13 are not edited)

Refs #930 and #958. §14 is left to the open pre-registration in #958, so neither section renumbers the other.

### 15.1 What changed

Before this change, `writer_scaling.py` ran the writer-mode throughput pass as one harness process per round (`--writers 1,2,4,8 --round r`). The harness then ran every W cell of that round, in Williams order, inside that one process. The interleaved comparison did the same once per build per round, alternating which build's process went first. Each cell built and dropped its own tree, but process-wide state carried over from one cell to the next.

Every timed writer-mode cell now runs in a harness process of its own (`--writers W --round r --position p`):

- **Single-build sweep.** Within round r the W cells follow row r of the Williams design over the writer counts. That is the order the multi-W invocation used.
- **Comparison** (`--compare`, `--variants`, the ablation shorthands). Within round r, the 2 × len(W) (build, W) cells follow row r of a Williams design over those cells, indexed (default, W1), (variant, W1), (default, W2), …. Over 2 × len(W) rounds, which is 8 at the default four writer counts and the default round count:
  - every (build, W) cell holds every position once;
  - every ordered pair of cells is adjacent once;
  - each build holds each position in half the rounds.
- **Frequency-droop pass** (`--pmu`). It already ran one W per process. It now follows the same Williams order and passes `--position`.

Three things are not changed:

- **Counters pass.** It times nothing and still runs every cell of an arm in one process. Its fallback counts depend on how the writers interleave, and whether they also carry over between cells is not measured.
- **`perf c2c` pass.** It stays one recording over one process that runs the same W in every round. It therefore has no cross-W carryover. Its rounds after the first follow a cell of the same W, and that is not measured either.
- **The multi-W harness invocation.** It stays available for manual use.

The driver checks what it records at three levels, and refuses to write an artifact that disagrees:

- each row against the invocation that produced it (round, position, W);
- each pass's rows against its schedule (build, round, position, W);
- the artifact's `rounds_raw` against the schedule, before writing.

Rows carry `build`. The provenance carries `cell_isolation: "process"` and a `cell_schedule` statement. `scripts/check_bench_provenance.py` requires `cell_isolation == "process"` of every `concurrency` `*writer_scaling*` artifact that is not grandfathered at the commit it was measured at.

### 15.2 Why

**Diagnostic probes, unsourced.** The raw rows are not committed, so these figures are not publishable measurements (§8.7). They come from the reference host, pinned one thread per P-core (`0,2,4,6,8,10,12,14`), on the `map` arm at `4412db44`. Each figure is the median of W = 8 `writer_mops` (workload: `concurrency_writer_map_64bit`).

| harness invocation | `ablation-sharded-alloc,lock-padded` build | default build |
|---|---|---|
| `--writers 8 --round 0` | 24.3 | 11.5 |
| `--writers 4,8 --round 0` | 21.7 | 11.5 |
| `--writers 2,8 --round 0` | 17.6 | 11.4 |
| `--writers 1,8 --round 0` | 14.5 | 11.0 |

The same W = 8 cell read lower the smaller the cell that ran before it in the same process. The default build shows the same ordering, much weaker. Run as `--writers 8,1`, W = 8 read 24.1 on the combined build.

The candidate mechanism is the allocator's per-thread arenas, and it is a hypothesis:

- `MALLOC_ARENA_MAX=1` removed the difference, but both builds dropped to about 5.5.
- `malloc_trim(0)` between cells changed nothing.
- Transparent hugepages were `madvise`, with no anonymous huge pages in use.
- Whole-process `perf stat` of `1,8` against `8,1` counted more page faults and more `cpu_core/dTLB-load-misses`. That covers the whole process, not the cell.

Which piece of process-wide state carries over is unmeasured.

**W = 1 control cells are affected too.** Two committed artifacts show it: `results/ablation_alloc_writer_scaling_726b01fc.json` and `results/ablation_alloc_writer_scaling_726b01fc_run2.json`, both at pin `0,2,4,6,8,10,12,14`. On the `set` arm, the per-round variant ÷ default W = 1 `writer_mops` reads as follows (workload: `concurrency_writer_set_63bit`; per-round samples, not an interval claim):

- **Rounds 3 and 7.** The Williams row there is W8, W1, W4, W2, so W = 1 runs right after a W = 8 cell in the same process. Readings: 0.872 and 0.878 in run 1, 0.880 and 0.876 in run 2.
- **The other rounds.** 0.968–1.001 in run 1.
- **Run 2, round 2.** 0.796. Here W = 1 follows W = 2, not W = 8, so this reading is not explained by following a W = 8 cell. Run 2's remaining rounds read 0.972–0.993.

### 15.3 What this means for artifacts measured before the change (§8.19)

- **Earlier artifacts mix carryover states.** Every `writer_scaling` throughput and comparison artifact committed before this change ran its timed cells in multi-cell processes. Any cell after the first in its process followed other cells. Which cells it followed depends on that round's Williams row, so each cell mixes carryover states across its rounds.
  - These cells are not comparable cell-for-cell with cells measured one process per cell.
  - They are not relabelled in place. `CELL_ISOLATION_GRANDFATHERED` in `scripts/check_bench_provenance.py` pins them at their commits.
- **Published readings that came from multi-cell processes** include §11's Hypothesis D verdicts, the arm (a) and `lock-padded` verdicts in `README.md` §11.7, and the W = 1 control readings beside them. The `set` W = 1 drop under `ablation-sharded-alloc` that #961 records as unexplained, beside making no promotion decision, is one of those W = 1 control readings. A per-round C(W) ratio divides the W ratio by the W = 1 ratio (`compute_paired_scaling_ratios`), so a W = 1 cell that reads low in a round also raises that round's C(W) ratio at every W (derived). Whether any of these readings or verdicts would stand under one process per cell is not established.
- **#958's spread inputs predate this change.** #958 (open) pre-registers W = 8 writer throughput of at least 20 M ops/s. It sizes its rounds from the per-round spread of the committed `writer_scaling` artifacts, all of them from multi-cell processes. That spread includes position-dependent carryover, so it may not describe the spread of cells measured one process each. This section does not amend #958.
- **The §12 ordered-reader artifacts** already ran one process per cell. They carry the field from their next run.

### 15.4 Explicitly not established

- Which process-wide state carries over.
- Whether any committed verdict changes under one process per cell.
- Whether the counters pass, or the `perf c2c` pass's same-W rounds, carry over.
- The same pattern in other concurrency drivers. These run several timed cells, or several timed rounds, inside one process:
  - `mixed_concurrency.py`'s default sweep, where one bench process runs every thread count;
  - the `hot_comparison` and `masstree_comparison` concurrent cells and `ablations.py` without `--processes`, where one process runs every round of a cell.

  They are follow-ups, not changed here.

## 16. Pre-registration for #730 — the readers-only string cell's per-reader cost (appended 2026-09-15, locked before any engine change on the wrapper's read path)

**Status: commit 2 of the three-commit cadence (AGENTS.md §8.8), locked before any
#730 engine change and before any run toward the gate.** It carries no
measurement of its own. Every figure quoted below is read from an artifact
already committed by #974 and named beside it. Commit 1 is
`scripts/reader_scaling_bounds.py`, whose functions this section invokes rather
than restates, and the readers-only instrument in
`docs/benchmarks/concurrency/scripts/writer_scaling.py` and
`crates/expanse/examples/writer_scaling.rs`. Nothing here is rewritten in place
once a run exists (AGENTS.md §8.7); a threshold, statistic, pin or round count
changed after a run relabels that run `INTERMEDIATE` (§8.19). Outcomes are
appended to `README.md` with their verdict labels, and amendments are appended
here as dated subsections.

Tracking issue: [#730](https://github.com/orieg/expanse/issues/730) (open).
§1–§15 are not edited.

### 16.1 What is registered, and what #730 leaves undetermined

**The claim this gate would license, in full:** *on the reference host, at the
registered pins and round count, eight `SyncExpanseStrMap` readers with no
writer present cost less per probe than they cost at `170a4bc3`, by at least
the registered margin, and the single-reader cell does not regress.* Nothing
wider. It is a statement about an outcome on one cell of one arm, not about a
mechanism, not about any other structure, and not about any competitor.

Three things #730 asks for are already instruments rather than open work, and
the gate is written against them:

- The W = 0 sweep at R ∈ {1, 2, 4, 8} exists natively, for `map`, `set` and
  `str`, one harness process per cell (`writer_scaling_readers_only`, §15), and
  its levels at `170a4bc3` are published in `README.md` §15.4.
- Restart and fallback shares for these cells are recorded: the counters pass
  writes `attempts_per_op` and `fallback_rate` per cell, and every readers-only
  cell of the four committed artifacts records `attempts_per_op` 1.0 and
  `fallback_rate` 0.
- The per-reader cost has a committed estimator and a committed interval
  construction (`reader_scaling_bounds.per_arm_interval`), so the gate needs no
  new statistic.

**Two things #730 does not determine, and this section does not invent:**

1. **Which change is made.** The issue names two candidate locations for the
   cost — a per-probe cost in the wrapper's read path, and reader–reader
   sharing that grows with R — and states that nothing was measured between the
   endpoints. It proposes no specific engine change. So what is registered here
   is an **outcome gate**: any candidate change on the wrapper's read path is
   evaluated against it, and the gate neither names nor credits a mechanism.
   A change that meets the gate has met the gate; what moved is a separate
   question with its own instrument and its own pre-registration.
2. **The floor the issue itself states.** #730's Gate section sets the R = 8
   per-reader cost against a figure from the FFI `masstree_conc_str` cell that
   is formally retracted as unsourced (`.github/superseded-figures.json`,
   `masstree_readers_only_string_82966aae`). A retracted figure is not an input to
   anything, so the floor is **re-derived here from the committed
   `concurrency_readers_str` levels at `170a4bc3`** (§16.3). That is a
   different cell from the one #730 was opened on — a different harness, and a
   probe stream in which every probe hits *(workloads differ:
   `concurrency_readers_str` vs `masstree_conc_str`)* — so this gate does not
   restore the issue's original comparison, and no ratio against a competitor
   is formed anywhere in this section.

### 16.2 The gate

Stated verbatim, and evaluated per cell:

> **The #730 readers-only gate.** Let *c* be the per-reader cost of the
> `str` readers-only cell at W = 0, R = 8: per round, the mean over the
> round's eight readers of each reader's own loop time, divided by that
> reader's probe count; over the round series, the mean and its BCa 95%
> interval from `reader_scaling_bounds.per_arm_interval` (2,000 resamples,
> seed 42). A cell **PASSES** iff the **upper** bound of that interval is
> strictly below the floor registered for its pin in §16.3 — 259.198 ns at
> pin `0-15`, 261.062 ns at pin `0,2,4,6,8,10,12,14` — **and** the same
> artifact's R = 1 `str` cell does not regress: the lower bound of its
> interval, constructed identically, is not above the R = 1 reference for
> that pin in §16.3. **The gate is met at a head** when all four cells pass:
> two pins × two independent runs. Any cell whose interval contains its
> floor is `INCONCLUSIVE`; any cell whose interval lies wholly above its
> floor is `REFUTED`.

**Why the upper bound and not the lower.** AGENTS.md §8.4 passes a continuous
claim iff the interval's bound on the *unfavourable* side clears the floor. The
gate statistic is a cost, so smaller is better and the unfavourable side is the
upper bound. On the R = 1 side condition the direction inverts again: a
regression is a *rise*, so the side condition fails only when the R = 1
interval lies confidently above the reference, which is its lower bound being
above it.

**Every input, and the artifact field it is read from.** All paths are relative
to `docs/benchmarks/concurrency/`.

| gate input | where it is read from |
|---|---|
| the cell | the element of `throughput` with `arm` `str`, `writers` 0 and `readers` 8 (R = 1 for the side condition) |
| the per-round series | for each element of that cell's `rounds_raw`, in `round` order: `mean(reader_thread_elapsed_s) × 1e9 ÷ prefill` — the same reduction `scripts/tables.py::_bl_ns_per_probe` applies for `README.md` §15.4 |
| the point estimate | the cell's `reader_ns_per_probe_thread_mean`, which the series above reproduces |
| the interval | `reader_scaling_bounds.per_arm_interval` over that series; its `method` must read `bca` |
| the round count | the cell's `rounds`, and `readers_only.rounds` |
| the pin | `provenance.core_pin`, and the cell's `cpu_pin` |
| the engine commit | `provenance.commit` |
| cell isolation | `provenance.cell_isolation`, which must read `process` (§15) |
| the void list | `readers_only.void`, which must be empty |
| host load | each cell's `load`, and `provenance.loads` (§16.7) |
| reported beside the verdict, not gated | `reader_mops_mean` with `reader_ci_lower` / `reader_ci_upper`, `scaling_s_r` with its interval, `reader_ns_per_probe_to_last_join`, `slowest_over_mean_thread_by_round`, `attempts_per_op`, `fallback_rate` |

**Why the per-thread mean and not the aggregate.** The cell carries two
estimators of the same quantity, and they separate. `reader_ns_per_probe_thread_mean`
divides each reader's own loop time by its probes; `reader_ns_per_probe_to_last_join`
divides the barrier-to-last-join time by one reader's probes, so it reads the
slowest reader. On the four committed `str` R = 8 cells the second is 1.8% and
1.8% above the first at pin `0-15` and 5.3% and 4.8% above it at one thread per
physical core *(workload: `concurrency_readers_str`; `results/baseline_readers_only_writer_scaling_170a4bc3_*.json`)*.
The gate takes the per-thread mean because a straggler is a property of the
round's scheduling rather than of the read path, and because the choice is made
here, before any run, rather than after seeing which estimator flatters a head.
The last-join figure is reported beside every verdict.

**The single-threaded side condition is deterministic, and is not this gate.**
#730 also asks that the single-threaded `short` lookup not regress. That is
already gated exactly, per commit, by the Callgrind arms `strmap_get_short` and
`sync_strmap_get_short` in `crates/expanse/benches/instructions.rs`, registered
in `scripts/perf_report.py` and enforced by the `instruction-counts` job under
AGENTS.md §6. No wall-clock baseline is invented for it here. Those counts are
exact integers on a different instrument and a different workload from the
readers-only cell *(workloads differ: `core_instructions` vs
`concurrency_readers_str`)*, so they are a precondition of a change, never a
term in the interval arithmetic above.

### 16.3 The baseline, and the floors derived from it

The gate is measured against the #974 levels at engine, harness and driver
commit `170a4bc3`: two pins, two independent runs each, 8 rounds per cell, one
harness process per timed cell. These are the four artifacts `README.md` §15
publishes; the columns below are read from them, not retyped from §15's tables.

*(measured: reference host — Intel Core i9-12900F, 8P+8E / 24 threads, kernel
6.8, governor `powersave` on every pinned CPU, transparent huge pages
`madvise`; engine, harness and driver at `170a4bc3`; workload:
`concurrency_readers_str`; reduction: `reader_scaling_bounds.per_arm_interval`
over the series §16.2 names)*

| pin | run | artifact under `results/` | CI run | R = 8 ns per probe per reader [BCa 95%] | R = 1 ns per probe per reader [BCa 95%] |
|---|--:|---|---|---|---|
| `0-15` | 1 | `baseline_readers_only_writer_scaling_170a4bc3_pin0-15.json` | [35021567680](https://github.com/orieg/expanse/actions/runs/35021567680) | 272.840 [272.545, 273.354] | 217.561 [217.225, 217.813] |
| `0-15` | 2 | `baseline_readers_only_writer_scaling_170a4bc3_pin0-15_run2.json` | [35021596408](https://github.com/orieg/expanse/actions/runs/35021596408) | 273.351 [272.886, 274.206] | 218.123 [217.471, 219.890] |
| `0,2,4,6,8,10,12,14` | 1 | `baseline_readers_only_writer_scaling_170a4bc3_percore.json` | [35021624186](https://github.com/orieg/expanse/actions/runs/35021624186) | 275.371 [273.867, 277.264] | 217.211 [216.902, 217.501] |
| `0,2,4,6,8,10,12,14` | 2 | `baseline_readers_only_writer_scaling_170a4bc3_percore_run2.json` | [35021650065](https://github.com/orieg/expanse/actions/runs/35021650065) | 274.802 [273.545, 276.899] | 217.439 [217.255, 217.690] |

**The floors, derived.** Per pin, the floor is the **lower** of that pin's two
committed R = 8 means, reduced by the registered margin of 5%:

- pin `0-15`: min(272.8396, 273.3511) = 272.8396 ns, so the floor is
  0.95 × 272.8396 = **259.198 ns**;
- pin `0,2,4,6,8,10,12,14`: min(275.3707, 274.8021) = 274.8021 ns, so the floor
  is 0.95 × 274.8021 = **261.062 ns**.

Taking the lower of the two runs is the strict choice: a head must beat the
most favourable baseline reading at its pin, not the average of the two. The
pins are never pooled, and the floors are never recomputed against a later
baseline — a re-measured baseline is a new pre-registration, not an edit to
this one (AGENTS.md §8.7).

**The R = 1 references, for the side condition,** are the **higher** of that
pin's two committed R = 1 means, which is the lenient choice on a condition
whose job is to catch a regression rather than to certify an improvement:
**218.1226 ns** at `0-15` and **217.4392 ns** at one thread per physical core.

**The margin of 5% is a choice, not a derivation.** What §16.4 establishes is
only that it is resolvable at the registered round count. Two facts bound it
from below and are stated before any run: the largest two-arm minimum
detectable difference among the four committed `str` R = 8 cells is 1.36%, and
the observed spread between the two runs of a pin is 0.19% at `0-15` and 0.21%
at one thread per physical core.

### 16.4 Math-first audit: is the gate resolvable at 8 rounds?

AGENTS.md §8.8's commit 1 and the repo's rule on executable bounds require the
detectability check to be computed by a committed, unit-tested function rather
than narrated. It is: `reader_scaling_bounds.mde_from_rounds` implements the
two-sample minimum detectable difference at a two-sided 5% test and 80% power
(Cohen 1988, ch. 2), `(z_{1-α/2} + z_{1-β}) · σ · sqrt(2/n)`, with σ the
per-round standard deviation and n the round count. Its hand-checkable
reference value is pinned in `SyntheticTests.test_mde_hand_value`, and
`scripts/gate.sh` and CI's `lint` job run that suite.

Applied to the per-round series of the four committed `str` cells, n = 8:

| pin | run | R | per-round σ (ns) | MDE (ns) | MDE, relative |
|---|--:|--:|--:|--:|--:|
| `0-15` | 1 | 8 | 0.5970 | 0.836 | 0.31% |
| `0-15` | 2 | 8 | 0.9635 | 1.350 | 0.49% |
| `0,2,4,6,8,10,12,14` | 1 | 8 | 2.6648 | 3.733 | 1.36% |
| `0,2,4,6,8,10,12,14` | 2 | 8 | 2.4338 | 3.409 | 1.24% |
| `0-15` | 1 | 1 | 0.4632 | 0.649 | 0.30% |
| `0-15` | 2 | 1 | 1.6065 | 2.250 | 1.03% |
| `0,2,4,6,8,10,12,14` | 1 | 1 | 0.4572 | 0.641 | 0.29% |
| `0,2,4,6,8,10,12,14` | 2 | 1 | 0.3204 | 0.449 | 0.21% |

**The audit's conclusion.** The effect the gate asks for is 5%. The largest
effect the instrument cannot resolve at the registered round count is 1.36%,
on the noisier of the two pins. The gate therefore asks for an effect 3.69× the
worst detectable one, and the experiment is not under-powered against its own
threshold. The check is the one AGENTS.md's math-first rule requires before a
gate is locked: *had* the margin been set at or below about 1.4%, the design
would have been rejected as undetectable at 8 rounds rather than run.

**What the audit does not establish.** That any change reaches the floor; that
σ on an evaluated head resembles σ on `170a4bc3` — a head with wider per-round
spread resolves less finely at the registered count, and the count does not
change for it (§8.19); and anything about statistical power against an effect
between 1.36% and 5%, where a real improvement can still read `INCONCLUSIVE`.

**Why `map` and `set` are reported and not gated.** The same function on their
R = 8 series gives a relative MDE reaching 7.49% (`map`, one thread per
physical core, run 1) and 4.08% (`set`, same pin, run 1)
*(`results/baseline_readers_only_writer_scaling_170a4bc3_percore.json`)*. Cells
that cannot resolve 5% cannot carry a 5% gate, so they are controls with their
levels published and no threshold attached.

**One quantity in #730's plan has no committed bound function, and is not
gated here.** The issue's third scope item asks for `LLC-load-misses` and
`mem_load_l3_hit_retired.xsnp_hitm` per probe at R = 1 and R = 8. The module
has `event_cycle_ceiling` and `unexplained_cycles`, but both convert an event
count into cycles through a *hypothesised* per-event cost that no committed
artifact prices, and the module labels every ceiling built on it a hypothesis.
No counter figure is therefore an input to this gate, and no counter threshold
is registered. Counters taken on an evaluated head are diagnostic and are
reported as such (AGENTS.md §8.9).

### 16.5 Rounds, pins, runs and cells

- **8 rounds per cell.** This is the driver's default (`--rounds`, default 8)
  and the count every one of the four committed baseline artifacts recorded, so
  the evaluation is reduced exactly as the baseline was. §16.4 shows 8 rounds
  resolves the registered margin with room. The count is fixed: adding rounds
  to decide an `INCONCLUSIVE` cell changes the sample size and relabels that
  evaluation `INTERMEDIATE` (§8.19).
- **Both pins, never pooled.** `0-15` is the 8 P-cores with SMT, so at R = 8
  two readers may share one physical core; `0,2,4,6,8,10,12,14` is one reader
  per physical P-core (AGENTS.md §8.20.5 step 0). The baseline was taken at
  both and the two are not interchangeable: the `str` R = 8 level and S(8) read
  lower per-core than at `0-15` in both runs, with disjoint intervals in each
  (`README.md` §15.5). Requiring both keeps the claim to what both placements
  measure, as §14.1 requires of the writer target, at the cost of twice the
  runs.
- **Two independent runs per pin, each a fresh dispatch, all four at one head**
  (`docs/BENCHMARKING.md` rule 18). A within-run BCa interval does not bound
  between-run spread, so no cross-run statement is made from one run.
- **The dispatch** is `bench_baremetal.yml` with
  `benchmark_suite=writer_scaling_readers_only`, `ref` naming the head being
  evaluated, and `cpu_pin` set to the pin. The suite takes the applied pin and
  records it; one dispatch per pin per run, four per evaluation. No workflow or
  driver change is needed to take a run, and none is registered here beyond
  §16.7's snapshot.
- **The gate cells are the `str` R = 8 cells only**, with the `str` R = 1 cells
  as the registered side condition. The `map` and `set` arms and R ∈ {2, 4} run
  as the suite runs them and are reported.
- **The artifact records which pre-registration it was read against.** The
  driver currently writes `readers_only.preregistration` as `null` with a note
  saying no pre-registration exists. The run that evaluates this gate records
  that field as `METHODOLOGY.md §16`; a run whose artifact still reads `null`
  is a baseline, not an evaluation, and carries no verdict.
- **When a run is taken.** Only after a candidate change on the wrapper's read
  path exists, or when the maintainer asks for one. Every evaluation is
  appended to `README.md` whatever its verdicts, so the number of evaluations
  is visible and a head that passes on a later attempt cannot be reported as
  though it were the first.

### 16.6 Rounds are not exchangeable, and the sensitivity the gate carries

This is a fact about the instrument at the moment of locking, disclosed here so
the gate is not silently sensitive to it.

`README.md` §15 records that **8 of the 12 R = 8 readers-only cells hold at
least one round beyond three scaled MADs of the median** — that count is over
the per-round `reader_mops` series, which is the aggregate estimator. On the
per-round series the gate actually uses, the four `str` R = 8 cells flag round
2 at pin `0-15` in both runs and flag nothing at one thread per physical core
*(`results/baseline_readers_only_writer_scaling_170a4bc3_*.json`)*. A BCa
interval over i.i.d. resamples of the rounds does not model a round like that,
as `README.md` §13 found for the committed FFI levels. The cause is unmeasured.

The gate handles it by declaring the sensitivity in advance rather than by
discarding rounds:

- **The primary verdict uses all 8 rounds.** No round is dropped from it, ever.
- **A sensitivity verdict is computed beside it** on the series with every round
  `reader_scaling_bounds.round_outliers` flags on that same series removed, the
  interval reconstructed by `per_arm_interval` on what remains. If fewer than 3
  rounds remain, the sensitivity verdict is `NOT_EVALUABLE` — the function
  refuses fewer than 3 — and that is reported.
- **A cell whose two verdicts disagree is `INTERMEDIATE_outlier_sensitive`,**
  and the gate is not met at that head. The primary verdict is still reported,
  labelled, and never presented alone.
- **The trimmed interval is never the headline** and never replaces the
  registered statistic. It exists so that a `PASS` resting on one favourable
  round is visible as such.

For calibration, the same procedure applied to the committed baseline moves the
`0-15` R = 8 mean by 0.167 ns (run 1) and 0.284 ns (run 2), both far inside the
registered 5% margin, and drops nothing at all per-core
*(workload: `concurrency_readers_str`)*. That is a property of the baseline, not
a prediction about a head.

`slowest_over_mean_thread_by_round` is published per cell beside every verdict,
so the spread across a round's readers is visible next to the number the gate
reads.

### 16.7 Host load, and the one thing the evaluation run must add

AGENTS.md §8.17 makes quietness a property of the artifact. For the readers-only
sweep the picture at lock is the one `README.md` §15 discloses, and it has a
limit worth stating exactly, because the gate depends on it.

**What the committed artifacts do carry.** Each cell's `load` window opens at
the `readers_only:throughput` label and closes when the throughput pass ends. It
is a real busy-CPU delta over a 57.4–57.5 s window, and it reads
`foreign_busy_cpus` 0.01 on every cell of all four artifacts, with
`own_busy_cpus` 1.26–1.27. So §8.17's non-lagging instrument — a busy-CPU delta
rather than a load average alone — is satisfied, and no void rule of §6 applies
to any of the four.

**What they do not carry.** `provenance.loads` holds two snapshots taken back to
back at the start of the sweep, with identical jiffy counters and the same
timestamp. One busy-CPU window therefore covers the whole throughput pass, every
cell of a run carries the same `foreign_busy_cpus`, and **no load average is
recorded after the pass**. The writer artifacts take one window per arm; the
readers-only ones do not.

**Is that acceptable for this gate?** Yes, for the comparison it makes, and the
reasons are specific rather than general:

- The quantity gated is a *within-cell* per-thread mean over 8 rounds, not a
  cross-arm ratio, so a window that does not isolate one cell does not confound
  the statistic.
- The window that exists covers the whole measured pass at a foreign-busy level
  two orders of magnitude below §8.17's ~1.0 core-equivalent bar, on every cell
  of every run.
- The baseline and the evaluation are reduced from the same instrument, so any
  residual coarseness applies symmetrically to both sides of the comparison.

**What the evaluation run must nevertheless do differently.** It must record a
**post-pass snapshot in `provenance.loads`** — an `end_cell` label appended to
the list rather than folded only into each cell's `load`. That costs nothing
inside the measured region and changes no timing; it makes two things checkable
that are not checkable today: whether the load average moved across the pass,
and whether the two runs of a pin ended as close as they started. §6's
"load shift above 2 between the two runs of a pair" can otherwise be applied at
run start only, and that is the whole of what the load evidence would say.

An evaluation run that omits it is **not void** — voiding it would make it
uncomparable with a baseline that has no such snapshot either — but its load
evidence is then start-only, and that limitation is disclosed beside the result
rather than left for a reader to notice (AGENTS.md §8.1). A run that omits it
*and* whose start snapshot is more than 2 apart from its pair's is void under
§6, as before.

Per-arm or per-block windows, which would let a single cell be voided on its own
load, are a further improvement and are not registered as a requirement here.

### 16.8 Expected losses

Pre-registered before any run, so that an unwelcome outcome is a recorded
expectation rather than a later rationalisation (AGENTS.md §8.7).

| cell or condition | expectation at lock | what a loss looks like | consequence |
|---|---|---|---|
| `str` R = 8, pin `0-15` | **not predicted** — no level, direction or magnitude is predicted for any head | interval above the floor | `REFUTED`; the gate is not met at that head |
| `str` R = 8, one thread per physical core | **not predicted**, and this is the cell most likely to read `INCONCLUSIVE` at a true improvement near the floor: its MDE is 1.24–1.36% against 0.31–0.49% at `0-15` (§16.4) | interval straddling the floor while the `0-15` cells pass | `INCONCLUSIVE` on that cell; the gate is not met, and the asymmetry is reported rather than resolved by dropping the pin |
| `str` R = 1 side condition | expected unchanged: a change aimed at reader scaling need not touch the single-reader path, and need not spare it either | R = 1 interval confidently above the reference | the cell fails even if its R = 8 half passes — an R = 8 gain bought by making one reader slower is not what is registered |
| `strmap_get_short`, `sync_strmap_get_short` Callgrind arms | expected flat or lower | a rise above AGENTS.md §6's 0.1% review threshold | a review blocker on the change, decided on the `instruction-counts` job; it is a precondition, not a term in this gate |
| `map` and `set` readers-only arms | expected roughly unchanged; they are controls | either arm moving while `str` moves | reported, no verdict: at 4.08–7.49% MDE these cells cannot resolve a 5% move, so their silence is uninformative and their movement is not attributable here |
| the two runs of a pin | expected to overlap, as all four baseline pairs do (`README.md` §15.5) | the two runs of one pin disagreeing on a cell's verdict | the gate is not met; the cell is reported direction-only (`docs/BENCHMARKING.md` rule 18) |
| the outlier sensitivity (§16.6) | expected to agree with the primary verdict | primary and trimmed verdicts disagreeing | `INTERMEDIATE_outlier_sensitive`; the gate is not met |

### 16.9 Verdicts

Per cell — four of them, two pins × two runs — using this suite's existing
vocabulary (§14.4):

- **`PASS`** — the R = 8 interval's upper bound is strictly below the pin's
  floor, and the R = 1 side condition holds.
- **`REFUTED`** — the R = 8 interval lies wholly above the pin's floor.
- **`INCONCLUSIVE`** — the interval contains the floor. This is AGENTS.md
  §8.4's `INTERMEDIATE_floor_within_ci` under this suite's vocabulary; the two
  names denote the same outcome and no third label is introduced.
- **`INTERMEDIATE_outlier_sensitive`** — primary and trimmed verdicts disagree
  (§16.6).
- **`INTERMEDIATE`** — any threshold, statistic, pin, estimator or round count
  differed from the registration (§8.19).
- **`NOT_EVALUABLE`** — an input the gate names is absent, or the sensitivity
  verdict has fewer than 3 rounds left. Never reported as a pass, never as 0.

**The gate is met at a head only when all four cells read `PASS`.** One
`REFUTED` means not met at that head. An `INCONCLUSIVE` leaves it unmet; a
further run added to decide it changes the sample size and relabels the
evaluation `INTERMEDIATE`. A later head is evaluated with the same threshold,
statistic, pins and round count. At a true cost exactly at the floor each
cell's nominal chance of a false `PASS` is 2.5%; the four cells share a head and
a host and are not independent, so no joint rate is claimed, and repeated
evaluations raise the chance that some head passes by chance — which is why
every evaluation is recorded (§16.5).

### 16.10 What voids a cell

§6 applies in full, and §15's cell-isolation requirement applies: an artifact
whose `provenance.cell_isolation` is not `process` measured its cells in shared
harness processes and is not comparable cell-for-cell with the `170a4bc3`
baseline. §14.5's void items apply where they name something this suite also
records — a wrong or unrecorded pin, a `--quick` population, a round count other
than the registered one, an interval whose method is not `bca`, timings from an
`occ-stats` build, and the four runs of one evaluation differing in `crates/` or
in `docs/benchmarks/concurrency/scripts/writer_scaling.py`.

Only what those sections do not already cover is added here:

- a non-empty `readers_only.void` list (the driver populates it; a `--quick`
  smoke run lands there);
- a `str` R = 8 or R = 1 cell whose `rounds_raw` does not hold each of rounds
  0..7 exactly once, or whose `reader_thread_elapsed_s` does not hold exactly
  `readers` entries — the driver refuses both, so this is listed for
  completeness;
- an artifact whose `readers_only.preregistration` names a section other than
  this one while being reported as an evaluation of it.

A void run is discarded whole, replaced by a fresh dispatch at the same head,
and the discard is disclosed beside the result (AGENTS.md §8.17). Replacing a
void run does not change the sample size.

### 16.11 Explicitly not claimed

- **No mechanism.** #730's two candidate locations for the cost are not
  distinguished by this gate and nothing here attributes the cost to either, or
  to any line, structure or event. A change that meets the gate is credited with
  meeting the gate. Whether the R = 1 level now published bears on the question
  is an analysis this section does not perform.
- **No comparison with Masstree or HOT.** Those readers-only competitor cells
  are pending re-measurement (#730, #1061), no competitor arm is dispatched by this <!-- docs-lint: allow -->
  suite, and no ratio against one is formed *(workloads differ:
  `concurrency_readers_str` vs `masstree_conc_str`)*.
- **Nothing read from the retracted figure.** The floors are derived only from
  the four `170a4bc3` artifacts.
- **No prediction of any level, direction or magnitude** for any head, and none
  that a change will be found at all.
- **No claim about the `map` or `set` readers-only arms,** about R ∈ {2, 4},
  about S(R), or about which pin reads lower on an evaluated head.
- **No claim about the string wrapper's writers,** which are #929's arms and a
  different instrument.
- **No counter threshold,** and no cycles-per-event figure: the module's event
  ceilings rest on a hypothesised per-event cost that no committed artifact
  prices (§16.4).
- **No statement about the cause of the non-exchangeable rounds** (§16.6), which
  is unmeasured.

### 16.12 Dated note, 2026-09-21: what this gate measures, and one pointer

Appended as #730 is closed as superseded. §16.1–§16.11 above are unchanged,
except that the pending citation in §16.11 now reads `(#730, #1061)`, so that it
names an open issue.

- **This gate measures single-reader latency, not reader scaling.** Its
  statistic is the R = 8 per-reader cost's upper bound against an absolute
  floor. In each of the four (pin, run) pairs `README.md` §22 publishes at
  `7cd5140e`, the R = 1 cell is 78.07–79.03 % of the R = 8 cell (derived from
  that table's point estimates, e.g. 213.90 ÷ 270.64 ns at `0-15` run 1), so
  most of what the statistic reads is present with one reader. A change that meets §16 is credited with
  meeting §16. It does not thereby show that the string reader scales with the
  reader count, and §16.11 already declines to attribute a mechanism.
- **The floors are absolute, so unrelated work spends the margin.** Neither the
  floors nor the statistic move (AGENTS.md §8.19). A later evaluation reports
  how much of any improvement was already present at its immediate parent.
- **#730's founding figure is retracted and inverted.** The 0.856 ratio is in
  `.github/superseded-figures.json`, and the same `masstree_conc_str` cell reads
  1.034 [1.015, 1.046] at `929574b5`, Expanse ahead. The readers-only questions
  that remain are carried by #1061.

### 16.13 Dated note, 2026-09-21: the pending citation in §16.11

#1061 is closed. The pending citation in §16.11 names two closed issues, so its
line carries a `docs-lint: allow` marker, the only byte changed there. The cell's
state is recorded in `README.md` §13: two committed level families, one per
measurement procedure, disclosed rather than resolved, with no re-measurement
planned.

## 17. Pre-registration for #929 — the `SyncExpanseStrMap` multi-writer design (appended 2026-09-15, locked before any engine code on the string wrapper's write path)

**Status: commit 2 of the three-commit cadence (AGENTS.md §8.8), locked before
any #929 engine change and before any run toward the gate.** It carries no
measurement of its own. Every figure quoted below is read from an artifact
already committed by #974 and #979 and named beside it, or derived here with
its arithmetic shown. Commit 1 is `scripts/reader_scaling_bounds.py`, whose
`mde_from_rounds` §17.6 invokes rather than restates, and the `str` writer arm
of `crates/expanse/examples/writer_scaling.rs` and
`docs/benchmarks/concurrency/scripts/writer_scaling.py`. Nothing here is
rewritten in place once a run exists (AGENTS.md §8.7); a threshold, statistic,
pin, estimator or round count changed after a run relabels that run
`INTERMEDIATE` (§8.19). Outcomes are appended to `README.md` with their verdict
labels, and amendments are appended here as dated subsections.

Tracking issue: [#929](https://github.com/orieg/expanse/issues/929) (open).
§1–§16 are not edited.

**Scope: the string wrapper only.** `SyncExpanseBytesMap` and
`SyncExpanseBlobMap` are #929's other two wrappers and get their own
registrations, informed by whether this design works. In particular
`SyncExpanseBlobMap::compact` and what an arena-backed value must exclude are
**deferred to the blob registration**, and nothing in this section binds that
choice: no threshold, instrument, verdict or design statement here applies to
either wrapper.

### 17.1 The claim this gate would license, in full

*On the reference host, at the registered pins, round count and workload,
`SyncExpanseStrMap`'s writers scale better with writer count than they do on
the mutex build this gate is measured against, by the paired C(W) ratio at
W ∈ {2, 4, 8}, in two independent runs at each of two pins.* Nothing wider.

It is a statement about an outcome on named cells of one arm. It is **not** a
statement about a mechanism: a head that meets the gate is credited with
meeting the gate, and which structure stopped contending is a separate question
with its own instrument. It is not a statement about `SyncExpanseBytesMap`,
`SyncExpanseBlobMap`, the string wrapper's readers (#730, open), any competitor
tree, any other host, or remove-heavy or mixed workloads — `writer_scaling`
measures inserts only.

### 17.2 The design, registered before it is written

#929 step 3 asks three questions of each wrapper. They are answered here for
`str`, from the code at `a969689a`, with `file:line` anchors rather than from
recollection. Where the code does not settle a question, §17.2.4 says so
instead of filling the gap.

#### 17.2.1 The node forms, the transitions, and what covers each store

**The encoding.** A key is cut into 8-byte big-endian chunks
(`chunk_at`, `crates/expanse/src/strmap.rs:321@eb35e465`). A chunk with fewer than eight
bytes remaining is *terminal* and its entry holds the user value directly; a
non-terminal chunk's entry is a tagged word — tag 0 a `*mut StrNode` child
(`pack_child`/`unpack_child`, `strmap.rs:171@eb35e465`, `:177@eb35e465`), tag 1 a `*mut StrSuffix`
leaf (`pack_suffix`/`unpack_suffix`, `strmap.rs:161@eb35e465`, `:166@eb35e465`); module doc
`strmap.rs:22-27`.

**The two node forms.**

- `StrNode { map: MapCore }` (`strmap.rs:191-193@eb35e465`) — a meta-trie branch is
  exactly one word-map engine core, and nothing else. Its size is pinned equal
  to `MapCore` and at most 64 bytes by `str_node_is_just_the_map_core`
  (`strmap.rs:2153-2160@eb35e465`).
- `StrSuffix { value, len }` with the remaining key bytes inline in the same
  allocation (`strmap.rs:78-81@eb35e465`, bytes at `SUFFIX_BYTES`, `strmap.rs:104@eb35e465`).
  Header and bytes are write-once after publication; only `value` mutates in
  place (`strmap.rs:70-76@eb35e465`).

**Every transition a mutation can take**, enumerated from the entry points:

| # | entry point | transition | the store, and where it lands |
|---|---|---|---|
| T1 | `insert` `strmap.rs:1217@eb35e465` | terminal chunk | `MapCore::insert_pathless` into this node's sub-map (`:1232@eb35e465`) |
| T2 | `insert` | continuation absent | `new_suffix` (`:113@eb35e465`) then `insert_pathless` of the tagged word (`:1240-1241@eb35e465`) |
| T3 | `insert` | suffix present, remainder equal | in-place replace of the value word (`:1259@eb35e465`) — the only in-place payload mutation |
| T4 | `insert` | suffix present, remainder diverges | `split_suffix` (`:1188-1213@eb35e465`): a private child `StrNode` is built and populated (`:1201@eb35e465`, `:1207@eb35e465`), published with one `insert_pathless` over the suffix entry (`:1211@eb35e465`), and the old suffix disposed (`:1212@eb35e465`) |
| T5 | `insert` | child present | descend (`:1268@eb35e465`) — no store |
| T6 | `ins_slot` `strmap.rs:1278@eb35e465` | terminal chunk | `ins_slot_pathless` (`:1297@eb35e465`); otherwise T2/T3/T4/T5 (`:1306@eb35e465`, `:1313-1331@eb35e465`) |
| T7 | `remove` `strmap.rs:1489@eb35e465` → `StrNode::remove` `:720@eb35e465` | terminal chunk | `remove_pathless` (`:737@eb35e465`) |
| T8 | `remove` | suffix match | `remove_pathless` then `dispose_suffix` (`:745-751@eb35e465`) |
| T9 | `remove` unwind | emptied child pruned | `remove_pathless` on the **parent** then `dispose_node` (`:771-777@eb35e465`) — the one transition that propagates upward |
| T10 | `remove` | meta-trie root emptied | `self.root.take()` then `dispose_node` (`strmap.rs:1497-1505@eb35e465`) |
| T11 | `insert`/`ins_slot` | meta-trie root absent | a root `StrNode` is created (`strmap.rs:1225-1227@eb35e465`, `:1286-1288@eb35e465`) |
| T12 | `clear` `strmap.rs:1584@eb35e465` | whole tree | `dispose_tree` (`:283@eb35e465`) |

**What covers each store today.** Every one of T1–T12 runs inside
`Shared::write` (`crates/expanse/src/sync.rs:1654@eb35e465`), reached from
`SyncExpanseStrMap::insert` (`:8019@eb35e465`), `remove` (`:8025@eb35e465`), `clear` (`:8030@eb35e465`) and
`with_locked_mut` (`:8093@eb35e465`). That function takes the writer mutex and holds the
**one tree version word** open across the whole operation, and the wrapper's
sub-tries run the engine's nested mode (`docs/ARCHITECTURE.md` §4.1).

**Which of them could be mutated under a node version lock.** T1, T7 and the
sub-map half of T2, T4, T8 and T9 are ordinary word-map mutations inside a
`MapCore`, and the engine's Phase 4A–4F paths already perform that class under
a parent branch's version word when the sub-map's root is a tree
(`docs/ARCHITECTURE.md` §4.2). T3 is a single covered word write. What none of
them can do today is take a *per-sub-map* cover, for two structural reasons
that are properties of the code and not of this design:

1. **A sub-map's root state carries no version word.** `Root::Empty`,
   `Root::Leaf` and `Root::Tree` (`crates/expanse/src/map.rs:831@eb35e465`, `:837@eb35e465`,
   `:848@eb35e465`) are covered by the tree-level word; only branch headers carry a word
   of their own (`docs/ARCHITECTURE.md` §4.1). A `StrNode` whose sub-map is
   empty or a root leaf therefore has no word anywhere to lock.
2. **The cover word is reached through the allocator, and the string map has
   one allocator for every sub-trie.** The engine finds its word through
   `NodeAlloc::bind_tree_word` / `tree_cover_addr`
   (`crates/expanse/src/alloc.rs:922@eb35e465`, `:936@eb35e465`, `:951@eb35e465`), bound once per
   allocator; `ExpanseStrMap` deliberately shares a single `NodeAlloc` across
   every sub-trie (`strmap.rs:11-14`, `defer_to` `:1129@eb35e465`, `:1137@eb35e465`), which is
   what keeps a `StrNode` at one map root instead of ~700 bytes. One allocator
   is one bound word.

**The design registered, therefore:** a **per-`StrNode` cover word**, covering
that node's sub-map root state and its tagged continuation entries, with the
engine's existing per-node words covering the sub-map interior when that
sub-map is a tree; writers enter through the wrapper's writer-entry path
(`Shared::enter_writer_blocking`, `sync.rs:1579@eb35e465`) and couple hand-over-hand
down the chunk chain, one cover per hop. Reaching a per-node word requires the
engine to take its cover per operation rather than from the allocator binding
above — which of the two available shapes (a word passed down the descent, or
a second binding mechanism) is an implementation choice this section does not
fix, because the gate is an outcome gate and neither shape is measured here.

**Two consequences of that word, named now rather than discovered later
(AGENTS.md §2.3).** A `StrNode` gaining a version word grows past
`size_of::<MapCore>()`, so `str_node_is_just_the_map_core`
(`strmap.rs:2153-2160@eb35e465`) fails and is part of the change, with its ≤ 64-byte
bound re-argued rather than deleted; and `ExpanseStrMap` does not implement
`RootState` today (the only impls are `ExpanseMap`, `sync.rs:1353@eb35e465`, and
`ExpanseSet`, `:1360@eb35e465`), so `write_root_covered` (`sync.rs:1713@eb35e465`) and the whole
`olc_*` route are not reachable for the string wrapper at all until it does.

#### 17.2.2 What a writer must hold for the suffix, and how readers validate

**The writer.** A suffix block is write-once after publication except its value
word (`strmap.rs:70-76@eb35e465`). A writer that publishes one must allocate it
(`new_suffix`, `:113@eb35e465`), store the tagged word into the parent `StrNode`'s
sub-map under that node's cover (T2, T4), and retire the superseded block
through the epoch collector (`dispose_suffix`, `:205-224@eb35e465`) — never free it
inline, since a reader that validated the old tagged word may still be reading
the header and bytes under its pin. T3's in-place value write is the one store
that mutates a published block, and it stays a single word under the cover of
the node holding the entry that points at it.

**The reader, and the hazard this design creates.** `StrReader::get`
(`sync.rs:8117@eb35e465`) calls `ExpanseStrMap::get_validated` (`strmap.rs:1396@eb35e465`), which
walks one sub-map per chunk. Each hop enters `walk_validated::<true>`
(`sync.rs:192@eb35e465`) with `Cover::Tree(ver, snap)` (`sync.rs:158-176@eb35e465`, `:198@eb35e465`) and
moves to `Cover::Node` inside the sub-map; **between hops, and on the suffix
arm before returning, the reader re-validates the tree version**
(`strmap.rs:1440-1447@eb35e465`). A design in which writers no longer bump the tree word
therefore leaves those checks validating a word nobody moves, which is the
precise failure the reader half of this design must prevent: the cross-hop
check and the suffix arm's check move to the cover word of the `StrNode` whose
entry was read, sampled before the entry is loaded and re-validated after it,
as `walk_validated` already does within a sub-map. `SyncExpanseStrMap::len`
(`sync.rs:8059@eb35e465`) keeps the tree word, and `with_locked` / `read_locked`
(`sync.rs:1844@eb35e465`, `:1815@eb35e465`) keep closing the gate and quiescing writers.

This is the first item of the §2.3 five-subsystem audit, and it is the reason
the coverage in §17.5 is registered as a precondition rather than a follow-up.

#### 17.2.3 What stays behind a bounded fallback, and the rate registered

Registered as staying behind the blocking fallback (`write_root_covered`
behind `fallback_mutex`, `sync.rs:1732@eb35e465`, which quiesces writers and serialises):

- **T11**, creation of the meta-trie root, and **T10**, its removal — root-state
  transitions of the wrapper itself.
- **T12**, `clear`, which is a whole-tree disposal.
- **T9**, the upward prune of an emptied child, which mutates a node the
  descent has already left.
- Whatever the engine's own OLC path already falls back on inside a sub-map:
  the `FallbackCause` set is `CapExpansion`, `ImmediateConversion`,
  `BranchSplit`, `RootGrowth`, `Contention`, `UnknownTag` (`sync.rs:1967-1987@eb35e465`),
  and a sub-map in `Root::Empty` or `Root::Leaf` state raises `RootGrowth`.

**The rate registered, and its derivation.** The declared workload is
`concurrency_writer_str`: a 2^20-key prefill, then 2^20 fresh keys inserted by
W writers over contiguous disjoint slices (`writer_scaling.rs:800-867@eb35e465`,
generator `:600-643@eb35e465`). Keys are alphanumeric over a 62-symbol alphabet
(`fill_alnum`, `:543-548@eb35e465`) with length `8 + rng % 9`, so every key is 8 to 16
bytes.

- A key of at least 8 bytes never presents a terminal first chunk (`chunk_at`,
  `strmap.rs:321@eb35e465`), so **T1 cannot occur at the meta-trie root** on this
  workload.
- Two keys share a first chunk only if their first 8 bytes agree. Over
  2^21 = 2,097,152 keys drawn from 62^8 ≈ 2.1834 × 10^14 first chunks, the
  expected number of colliding pairs is
  C(2^21, 2) ÷ 62^8 ≈ 2.1990 × 10^12 ÷ 2.1834 × 10^14 ≈ **0.0101** (derived).
- So essentially every fresh insert is **T2**: one suffix allocation plus one
  `insert_pathless` of a tagged word into the *root* `StrNode`'s sub-map, which
  holds about 2^20 entries and is in tree root state. T4, T5, T9 and T10 occur
  on this workload only at that rate, and T11 exactly once per cell —
  1 ÷ 1,048,576 ≈ 9.5 × 10^-7 of inserts (derived).

The prediction, fixed before any run: **the structural fallback rate
(`fallback_causes_total` less `contention`, over `write_ops`) is below
1 × 10^-3 at every W ∈ {1, 2, 4, 8}, and the total `fallback_rate` is below
1 × 10^-2 at every W.** A head above either is `REFUTED` on that prediction and
the outcome is published whatever the throughput cells say, because a design
whose fallbacks are common is not the design registered here. The comparison
band this sits in: the `map` arm, whose insert into a 2^20-key word map is the
same engine work the dominant T2 path performs, records `fallback_rate` 0.0 in
three of the four committed W = 8 cells and 1 × 10^-6 in the fourth
*(workloads differ: `concurrency_writer_map_64bit` vs `concurrency_writer_str`;
`results/baseline_writer_scaling_170a4bc3_*.json`)*.

#### 17.2.4 What the code does not settle, and is not invented here

- **Whether each of T1–T9 can be performed without a structural fallback under
  a per-`StrNode` cover is not decidable from the code alone**, because the
  cover does not exist yet: §17.2.1's two structural facts mean the answer
  depends on how the engine is given a per-node word, and no committed artifact
  prices either shape. What is registered is the outcome gate and the fallback
  rate; the per-transition answer is reported by the implementation with its own
  counters (the `fallback_causes_total` partition the driver already checks) and
  is not predicted here.
- **The per-insert suffix allocation is not addressed by this design.** T2
  allocates outside the shared `NodeAlloc` on every insert, and the `allocation`
  category is 20.13% of `sync_strmap_insert`'s exclusive Ir
  *(measured: x86_64 Linux in a container, `9b5a6d05`; `README.md` §14.3;
  workload: `core_instructions`)*. Whether W concurrent allocators help or hurt
  at W = 8 is **unmeasured**, and §15.2 records that process-wide allocator
  state moves writer cells by an amount it labels a hypothesis with the carrying
  mechanism unmeasured. No prediction is registered on it.
- **Which structure, if any, stops contending** — see §17.9.

### 17.3 The gate

Stated verbatim, and evaluated per cell:

> **The #929 `str` multi-writer gate.** For a build *b* and writer count W, let
> C_b(W, r) = `writer_mops`(b, W, r) ÷ `writer_mops`(b, 1, r) be the paired
> per-round scaling factor of the `str` arm, taken within round *r* of one
> interleaved comparison run. Let the per-round gate statistic be
> R(W, r) = C_head(W, r) ÷ C_main(W, r), and let its interval be the BCa 95%
> interval over the round series (2,000 resamples). A cell — one (W, pin, run)
> — **PASSES** iff the **lower** bound of that interval is strictly above 1.0.
> The gate cells are W ∈ {2, 4, 8}; W = 1 is the control cell, and it fails if
> the BCa 95% interval of the per-round ratio
> `writer_mops`(head, 1, r) ÷ `writer_mops`(main, 1, r) lies wholly below 1.0.
> **The gate is met at a head** when all twelve gate cells pass — three writer
> counts × two pins (`0-15` and `0,2,4,6,8,10,12,14`) × two independent runs —
> and no control cell fails. A cell whose interval contains 1.0 is
> `INCONCLUSIVE`; a cell whose interval lies wholly below 1.0 is `REFUTED`.
>
> **Single-threaded Callgrind, a precondition and not a term in the interval
> arithmetic above.** On the head's own `instruction-counts` job: each of
> `sync_strmap_insert`, `sync_strmap_remove`, `sync_strmap_churn`,
> `sync_strmap_insert_short`, `sync_strmap_churn_short` and
> `sync_strmap_get_short` at most +5.0% against main, and each of the
> plain-tree arms `strmap_insert`, `strmap_get`, `strmap_churn` and
> `strmap_get_short` at most +0.1%, AGENTS.md §6's review threshold. This
> registration pre-authorises no `allow-regression:` override.

**Why W ∈ {2, 4, 8} and not W = 8 alone.** #929's Gates section asks for the
paired ratio's lower bound above 1.0 "at W ≥ 2". Reading that as *every* W ≥ 2
the suite measures is the strict reading, and it is fixed here, before any run,
so that a head cannot later be reported as meeting the gate on the one writer
count that moved. The cost is that a head which improves W = 8 while leaving
W = 2 unchanged does not meet the gate; §17.8 records that as an expected loss
rather than a surprise.

**Why the ratio of ratios, and not the W = 8 level.** AGENTS.md §8.20.2: an
optimisation is not a concurrency improvement because it made the
single-threaded baseline faster. Dividing by each build's own W = 1 cell,
within the round, is what separates the two; the W = 1 control is published
beside every verdict so a head that only moved the baseline is visible as such.

**Every input, and the artifact field it is read from.** All paths are relative
to `docs/benchmarks/concurrency/`.

| gate input | where it is read from |
|---|---|
| the cells | elements of `throughput` (build `default`, the main build) and `throughput_variant` (the head build) with `arm` `str` and `writers` ∈ {1, 2, 4, 8} |
| the per-round series | each cell's `rounds_raw`, in `round` order, field `writer_mops`, matched round for round between the two builds |
| the interval | BCa 95% over the round series of R(W, r), 2,000 resamples (`scripts/bca_bootstrap.py`, the construction the driver uses for `scaling_factor_c_n_ci_*`) |
| reported beside it, not gated | `expanse_writer_mops_mean` with `writer_ci_lower` / `writer_ci_upper`, `scaling_factor_c_n_mean` with its interval, `lock_fallbacks`, `fallback_rate`, `fallback_causes_total`, `contention_subsets_total`, `lock_restarts_per_insert`, `gate_blocked_entries_per_insert`, `gate_wait_cycles_per_insert`, `retired_per_insert` |
| the fallback prediction (§17.2.3) | `fallback_rate` and `fallback_causes_total`, over `write_ops` — which is the harness's fresh-key count, not `Stat::WriteOps` (`scripts/writer_scaling.py:604-605@eb35e465`, `writer_scaling.rs:2671@eb35e465`) |
| the round count | each cell's `rounds` |
| the pin | `provenance.core_pin`, and each cell's `cpu_pin` |
| the commits | `provenance.commit`, and the comparison run's per-build commit fields |
| cell isolation | `provenance.cell_isolation`, which must read `process` (§15) |
| host load | each cell's `load`, and `provenance.loads` |

### 17.4 The baseline this is measured against

The mutex build the gate compares against is the `str` arm as it stands, whose
committed levels are the #974 baselines at engine, harness and driver commit
`170a4bc3`: two pins, two independent runs each, 8 rounds per cell, one harness
process per timed cell. The rows below are read from the four artifacts
programmatically, not retyped from `README.md` §15.2.

*(measured: reference host — Intel Core i9-12900F, 8P+8E / 24 threads, kernel
6.8, governor `powersave` on every pinned CPU, transparent huge pages
`madvise`; engine, harness and driver at `170a4bc3`; workload:
`concurrency_writer_str`; `results/baseline_writer_scaling_170a4bc3_pin0-15.json`,
`_pin0-15_run2.json`, `_percore.json`, `_percore_run2.json`)*

| pin | run | W = 1 M ops/s [BCa 95%] | W = 2 [BCa 95%] | W = 4 [BCa 95%] | W = 8 [BCa 95%] |
|---|--:|---|---|---|---|
| `0-15` | 1 | 4.0558 [4.0457, 4.0647] | 2.5843 [2.5508, 2.6243] | 2.3470 [2.3096, 2.3838] | 2.1297 [1.9531, 2.2266] |
| `0-15` | 2 | 4.0801 [4.0722, 4.0855] | 2.6981 [2.5816, 2.9040] | 2.3216 [2.2961, 2.3533] | 1.8702 [1.4479, 2.1116] |
| `0,2,4,6,8,10,12,14` | 1 | 4.0821 [4.0736, 4.0875] | 2.6600 [2.5765, 2.8718] | 2.3461 [2.3123, 2.3961] | 0.4657 [0.4613, 0.4723] |
| `0,2,4,6,8,10,12,14` | 2 | 4.0821 [4.0673, 4.0913] | 2.8690 [2.7290, 3.0109] | 2.3408 [2.3040, 2.3814] | 0.4650 [0.4611, 0.4698] |

| pin | run | C(2) [paired BCa 95%] | C(4) [paired BCa 95%] | C(8) [paired BCa 95%] | `fallback_rate` |
|---|--:|---|---|---|--:|
| `0-15` | 1 | 0.6372 [0.6281, 0.6471] | 0.5787 [0.5692, 0.5886] | 0.5251 [0.4815, 0.5489] | 0.0 |
| `0-15` | 2 | 0.6612 [0.6331, 0.7107] | 0.5690 [0.5637, 0.5766] | 0.4585 [0.3552, 0.5185] | 0.0 |
| `0,2,4,6,8,10,12,14` | 1 | 0.6516 [0.6313, 0.7032] | 0.5748 [0.5664, 0.5877] | 0.1141 [0.1129, 0.1157] | 0.0 |
| `0,2,4,6,8,10,12,14` | 2 | 0.7027 [0.6702, 0.7372] | 0.5734 [0.5649, 0.5829] | 0.1139 [0.1129, 0.1152] | 0.0 |

**`fallback_rate` 0.0 here does not mean "no fallbacks".** The string wrapper
has no optimistic path to fall back *from*: every mutation takes
`Shared::write` (`sync.rs:1654@eb35e465`), which bumps `Stat::WriteOps` and never
`Stat::LockFallbacks`. The column becomes informative only once the design
lands, which is why §17.2.3's prediction is registered against it now.

**Both pins, never pooled, and the pin decides the comparison
(AGENTS.md §8.20.5 step 0).** `str` is the arm where the two placements
separate most: `README.md` §15.3 records its W = 8 level and C(8) lower
per-core than at `0-15` in both runs, and the intervals above are disjoint —
C(8) 0.5251 [0.4815, 0.5489] and 0.4585 [0.3552, 0.5185] at `0-15` against
0.1141 [0.1129, 0.1157] and 0.1139 [0.1129, 0.1152] one thread per physical
P-core *(workload: `concurrency_writer_str`)*. A cell is comparable only
against a cell under the same pin; the gate requires both, and every prediction
in §17.8 names the pin it is evaluated on. Neither placement is the "true" one
and which reads higher on an evaluated head is not predicted.

### 17.5 Coverage each new transition must carry, as a precondition

Registered now so a head cannot arrive with the gate met and the coverage
deferred. None of these is a term in §17.3's arithmetic; a head missing any of
them is not evaluated.

- **Loom.** One model per new concurrent transition class, each with the line
  whose deletion turns it red, beside the existing models in
  `crates/expanse/src/occ.rs:2635-3308@eb35e465` and
  `crates/expanse/src/sync.rs:12161@eb35e465`, and run by the `loom` CI job
  (`.github/workflows/ci.yml:1909-1930@eb35e465`). At minimum: two writers publishing
  into one `StrNode`'s sub-map entry are mutually excluded; a superseded suffix
  or child is marked and retired only after the entry pointing at it is
  rewritten (the S3 property, for the tagged word); and a reader's cross-hop
  check validates the word the writer actually bumps — the hazard §17.2.2
  names, whose negative control is a model that is red when the cross-hop check
  is left on the tree word.
- **Tier-1 Miri.** A deterministic single-threaded test of every new transition,
  named so the per-PR filter selects it. That filter is a literal list
  (`.github/workflows/ci.yml:1379@eb35e465`, mirrored in AGENTS.md §5), so either the
  new tests sit under a prefix it already selects — `strmap::tests::deferred`,
  `occ::tests::` — or the filter and `scripts/check_miri_shards.py`'s
  module-to-shard map are updated in the same change. No Miri is run on the
  laptop; the CI jobs are the authority.
- **Linearizability.** `crates/expanse/tests/linearizability.rs` covers
  `SyncExpanseMap` and `SyncExpanseSet` only (`:8@eb35e465`). A `SyncExpanseStrMap`
  history test is added with the same per-key checker shape, at W ≥ 2, plus the
  disjoint-writer census check that `test_multi_writer_parallel_disjoint_and_census`
  (`:468@eb35e465`) performs for the two integer wrappers.
- **The §2.3 five-subsystem audit** accompanies the change, including the two
  consequences §17.2.1 names.

### 17.6 Math-first audit: can the registered rounds resolve the gate?

AGENTS.md §8.8's commit 1 requires the detectability check to be computed by a
committed, unit-tested function rather than narrated. It is:
`reader_scaling_bounds.mde_from_rounds` implements the two-sample minimum
detectable difference at a two-sided 5% test and 80% power (Cohen 1988, ch. 2),
`(z_{1-α/2} + z_{1-β}) · σ · sqrt(2/n)`, with σ the per-round standard
deviation and n the round count; its hand-checkable reference value is pinned
in `SyntheticTests.test_mde_hand_value`, which `scripts/gate.sh` and CI's
`lint` job run. No new script and no new bound function is added for this
registration.

Applied to the per-round C(W) series of the four committed `str` cells — the
series §17.3 gates on, taken on the mutex build, with the equal-spread
assumption the function's docstring states:

| pin | run | W | per-round σ of C(W) | MDE | MDE, relative |
|---|--:|--:|--:|--:|--:|
| `0-15` | 1 | 2 | 0.01497 | 0.02096 | 3.29% |
| `0-15` | 1 | 4 | 0.01437 | 0.02012 | 3.48% |
| `0-15` | 1 | 8 | 0.04737 | 0.06636 | 12.64% |
| `0-15` | 2 | 2 | 0.05445 | 0.07628 | 11.54% |
| `0-15` | 2 | 4 | 0.00985 | 0.01380 | 2.42% |
| `0-15` | 2 | 8 | 0.11869 | 0.16626 | 36.26% |
| `0,2,4,6,8,10,12,14` | 1 | 2 | 0.04903 | 0.06868 | 10.54% |
| `0,2,4,6,8,10,12,14` | 1 | 4 | 0.01591 | 0.02229 | 3.88% |
| `0,2,4,6,8,10,12,14` | 1 | 8 | 0.00210 | 0.00294 | 2.58% |
| `0,2,4,6,8,10,12,14` | 2 | 2 | 0.05245 | 0.07348 | 10.46% |
| `0,2,4,6,8,10,12,14` | 2 | 4 | 0.01362 | 0.01908 | 3.33% |
| `0,2,4,6,8,10,12,14` | 2 | 8 | 0.00187 | 0.00262 | 2.30% |

**The audit's conclusion, and it is not uniform across the pins.** The gate
asks for a ratio confidently above 1.0, so the effect it must resolve is the
improvement in C(W). At one thread per physical P-core the instrument resolves
2.3%–10.5% at 8 rounds, and the W = 8 cell — the one where the mutex build is
furthest from scaling, C(8) ≈ 0.114 — is its most sensitive at 2.30%–2.58%. At
`0-15` the W = 8 cell is the *least* sensitive of the twelve, at 12.64% and
36.26%: an improvement smaller than about a third of C(8) cannot be
distinguished there at this round count. The experiment is therefore not
under-powered against a design that removes a single serialising mutex, where
the change sought is a multiple rather than a few percent — the mutex build's
C(8) is 0.114 per-core against `map`'s 2.24–2.28 on the same runs
*(workloads differ: `concurrency_writer_str` vs `concurrency_writer_map_64bit`)*
— but it **is** under-powered at `0-15`, W = 8 against a modest improvement,
and that asymmetry is declared here rather than discovered when a cell reads
`INCONCLUSIVE`.

**What the audit does not establish.** That any change reaches the gate; that
σ on an evaluated head resembles σ on the mutex build — a head with wider
per-round spread resolves less finely at the registered count, and the count
does not change for it (§8.19); and anything about power against an effect
between the MDE and the gate, where a real improvement can still read
`INCONCLUSIVE`. It also assumes equal spread in the two builds of a comparison
run, which is the function's stated assumption and is not verified in advance.

### 17.7 Rounds, pins, runs, cells, and the instrument prerequisites

- **8 rounds per cell**, the driver's default and the count every committed
  `str` cell records, so the evaluation is reduced exactly as the baseline was.
  Fixed: adding rounds to decide an `INCONCLUSIVE` cell relabels that
  evaluation `INTERMEDIATE` (§8.19).
- **Both pins, never pooled**, two independent runs per pin, each a fresh
  dispatch, all four at one head (`docs/BENCHMARKING.md` rule 18). A within-run
  BCa interval does not bound between-run spread, so no cross-run statement is
  made from one run.
- **One harness process per timed cell** (§15); `provenance.cell_isolation`
  must read `process`.
- **The comparison must be interleaved within rounds.** §17.3's statistic is a
  paired ratio of two builds, which requires both builds' cells inside one run,
  in the Williams order §15.1 fixes for comparison mode. Two separate
  single-build runs at two commits do not produce it and are not an evaluation
  of this gate.

**Instrument prerequisites, registered before any run.** Each is a change to
the instrument, not to the engine, and each must land before a run counts:

1. **The head exposes the new write path behind a build feature**, so the
   evaluation is one interleaved two-build comparison run per pin per run, the
   way the #568 ablation arms were measured. Without it the driver has no
   variant build to interleave.
2. **A suite entry** in `.github/bench-suites.json` and the hand-listed places
   in `.github/workflows/bench_baremetal.yml` (the suite list at `:64-70@eb35e465` and
   the case block at `:990-999@eb35e465`), synced with
   `python3 scripts/check_bench_suites.py --write`.
3. **The gate statistic is computed by committed code** before the run that it
   judges. The driver writes `scaling_factor_c_n_*` per build; the ratio of the
   two builds' C(W) is not a field it emits today, so the reduction §17.3 names
   is committed with the suite entry and reads `rounds_raw` of both builds.
4. **The driver's counters-pass identities must still hold on the head.** It
   refuses a row where the fallback causes do not sum to `lock_fallbacks`
   (`writer_scaling.py:615-618@eb35e465`), where `Stat::Inserts` differs from the
   harness's insert count (`:620-623@eb35e465`), or where `quiesce_calls` differs from
   `lock_fallbacks` (`:626-629@eb35e465`); the harness checks the same identities
   (`writer_scaling.rs:424-441@eb35e465`). A design whose fallback path does not quiesce
   exactly once per fallback breaks the third, and that is a change to the
   instrument's invariant which is disclosed and re-argued, never silently
   relaxed.
5. **The artifact records which pre-registration it was read against**, as the
   ordered-reader artifacts do (`writer_scaling.py:1595@eb35e465`). A run whose artifact
   does not name this section is a baseline, not an evaluation, and carries no
   verdict.

**The dispatch** is `bench_baremetal.yml` with the suite entry from
prerequisite 2, `ref` naming the head being evaluated, `rounds=8` and `cpu_pin`
set to the pin — one dispatch per pin per run, four per evaluation.

**When a run is taken.** Only after a candidate change on the string wrapper's
write path exists and carries §17.5's coverage, or when the maintainer asks for
one. Every evaluation is appended to `README.md` whatever its verdicts, so the
number of evaluations is visible and a head that passes on a later attempt
cannot be reported as though it were the first.

### 17.8 Expected losses

Pre-registered before any run, so an unwelcome outcome is a recorded
expectation rather than a later rationalisation (AGENTS.md §8.7).

| cell or condition | expectation at lock | what a loss looks like | consequence |
|---|---|---|---|
| `str` W = 8, one thread per physical P-core | **not predicted** — no level, direction or magnitude is predicted for any head; this is the most sensitive gate cell (§17.6) | interval containing or below 1.0 | `INCONCLUSIVE` or `REFUTED`; the gate is not met at that head |
| `str` W = 8, pin `0-15` | **not predicted**, and this is the cell most likely to read `INCONCLUSIVE` at a real improvement: its MDE is 12.64% and 36.26% against 2.30%–2.58% per-core (§17.6) | interval straddling 1.0 while the per-core cells pass | `INCONCLUSIVE` on that cell; the gate is not met, and the asymmetry is reported rather than resolved by dropping the pin |
| `str` W = 2 and W = 4 | expected to be the hardest of the three to move: the mutex build already reaches C(2) 0.637–0.703 and C(4) 0.569–0.579, so less of the curve is available there than at W = 8 *(workload: `concurrency_writer_str`)* | W ∈ {2, 4} straddling 1.0 while W = 8 passes | the gate is not met; the outcome is published with all three writer counts, and §17.3's "every W ≥ 2" reading is not revisited after the fact |
| W = 1 control | expected unchanged: the design targets writers under contention, and need not touch the single-writer path — nor need it spare it | W = 1 ratio wholly below 1.0 | the control fails; a W ≥ 2 gain bought by making the single writer slower is not what is registered |
| the structural fallback rate (§17.2.3) | below 1 × 10^-3 at every W; total below 1 × 10^-2 | either exceeded | `REFUTED` on that prediction, published whatever the throughput cells say |
| the `sync_strmap_*` Callgrind arms | expected to move — they carry the protocol — and to stay inside +5.0% | any arm above its bound | a precondition failure on the change, decided on the `instruction-counts` job; never traded against a throughput cell |
| the plain-tree `strmap_*` arms | expected flat: concurrency-only code stays out of the shared inlined paths (AGENTS.md §2.2, #929's constraints) | any arm above +0.1% | a review blocker under AGENTS.md §6 |
| the two runs of a pin | expected to agree, as all four baseline pairs do (`README.md` §15.3) | the two runs of one pin disagreeing on a cell's verdict | the gate is not met; the cell is reported direction-only (`docs/BENCHMARKING.md` rule 18) |
| the `map` and `set` arms in the same sweep | expected unchanged; they are controls, and this design does not touch their path | either moving while `str` moves | reported, and a reason to suspect the run rather than to credit the change |

### 17.9 Verdicts

Per cell, using this suite's existing vocabulary (§14.4, §16.9) with no new
label introduced:

- **`PASS`** — the cell's interval lower bound is strictly above 1.0.
- **`REFUTED`** — the interval lies wholly below 1.0.
- **`INCONCLUSIVE`** — the interval contains 1.0. This is AGENTS.md §8.4's
  `INTERMEDIATE_floor_within_ci` under this suite's vocabulary.
- **`INTERMEDIATE`** — any threshold, statistic, pin, estimator or round count
  differed from this registration (§8.19).
- **`NOT_EVALUABLE`** — an input the gate names is absent, or §17.5's coverage
  is missing. Never reported as a pass, never as 0.

**The gate is met at a head only when all twelve gate cells read `PASS` and no
control cell fails.** One `REFUTED` means not met at that head. An
`INCONCLUSIVE` leaves it unmet, and a further run added to decide it changes
the sample size, which relabels the evaluation `INTERMEDIATE`. A later head is
evaluated with the same threshold, statistic, pins and round count. At a true
ratio of exactly 1.0 each cell's nominal chance of a false `PASS` is 2.5%; the
twelve cells share a head and a host and are not independent, so no joint rate
is claimed, and repeated evaluations raise the chance that some head passes by
chance — which is why every evaluation is recorded (§17.7).

### 17.10 What voids a cell

§6 applies in full, and §14.5's void items apply where they name something this
suite also records: a wrong or unrecorded pin, a `--quick` population (a W cell
recording `prefill` or `fresh_keys` other than 1,048,576), a round count other
than 8, an interval whose method is not `bca`, a `provenance.cell_isolation`
other than `process` (§15), timings from an `occ-stats` build, a
`provenance.commit` that does not contain the change the evaluation is taken
for, and the four runs of one evaluation differing in `crates/` or in
`docs/benchmarks/concurrency/scripts/writer_scaling.py`. §16.10's rule that an
artifact naming a different pre-registration is not an evaluation of this one
applies here too.

A void run is discarded whole, replaced by a fresh dispatch at the same head,
and the discard is disclosed beside the result (AGENTS.md §8.17). Replacing a
void run does not change the sample size.

### 17.11 Explicitly not claimed

- **Nothing follows from the c2c ranking.** `README.md` §16.1 is observational
  (AGENTS.md §8.20.3): it ranks cache lines by HITM load samples in two
  recordings and establishes no cause. Over half its samples are on lines the
  report did not list — 51.66% and 52.03% *(workload:
  `concurrency_writer_str`)* — so what it does locate is a minority of the
  traffic; its top group is kernel addresses; the one Rust symbol it resolves
  is `SyncExpanseStrMap::insert`, and the symbol at the most-contended offset
  of its second group is unresolved. This design is **not** presented as
  following from it. The ranking is why the writer path was the place to look;
  only the interventional step — this gate — decides anything, and a met gate
  credits no line, structure or event.
- **No mechanism claim.** Neither the level of any cell nor its movement is
  attributed here to lock transfer, coherency traffic, allocator behaviour or
  frequency. `README.md` §16.1.5's 81.07% and 81.41% cycles-over-ref-cycles
  readings are reported there as not interpretable as a frequency where writers
  serialise, and they are not an input to anything in this section.
- **No prediction of any level, direction or magnitude** for any head, and none
  that a design meeting the gate will be found at all.
- **No claim about `SyncExpanseBytesMap` or `SyncExpanseBlobMap`.** Their
  designs are separate registrations. `SyncExpanseBlobMap::compact` and what an
  arena-backed value must exclude are deferred to the blob registration
  entirely, and no statement here constrains them — including §17.2's per-node
  cover, which is registered for the string wrapper's `StrNode` and for nothing
  else.
- **No claim about the string wrapper's readers**, which are #730's gate
  (open, §16) and a different instrument. §17.2.2's reader change is a
  correctness precondition of this design, not a performance claim, and no
  reader cell is gated here.
- **No comparison with Masstree or HOT.** No competitor arm is dispatched by
  this suite and no ratio against one is formed *(workloads differ:
  `concurrency_writer_str` vs `masstree_conc_str`)*.
- **No claim that the registered fallback set is minimal.** T9, T10, T11 and
  T12 are registered as staying behind the fallback because this design does
  not attempt them concurrently, not because they cannot be done.
- **No counter threshold.** Counters taken on an evaluated head are diagnostic
  and reported as such (AGENTS.md §8.9).


### 17.12 Implementation record (appended 2026-09-16, before any evaluation run)

**Status: the head §17.3 evaluates exists; no run toward the gate has been
taken.** This subsection records what the implementation is, where it departs
from §17.2's registered description, and which of §17.7's prerequisites it
meets — so an evaluation is read against the design that was built rather than
the one that was imagined. Nothing above it is rewritten; §17.3's threshold,
statistic, pins, estimator and round count are the ones registered.

**Where each transition runs.** §17.2.4 declined to predict which of T1–T9
avoid a structural fallback because the cover did not exist yet. Built, on the
head (`docs/ARCHITECTURE.md` §4.2, *The string wrapper*):

| transition | sub-map in tree state | sub-map in leaf or empty state |
|---|---|---|
| T1 / T7 | the engine's OLC body (`olc_insert_map` / `olc_remove_map`, expanded for the `OlcHost` the `StrNode` implements), under its per-node locks | the plain path under the node's cover taken as a lock at the lookup's snapshot |
| T2 | the engine's body in an insert-if-absent mode: a suffix another writer published first is returned rather than clobbered, and the speculative one freed | the cover lock |
| T3, T4, T8 | the cover lock, with the entry's store through the engine's body underneath it | the cover lock |
| T5 | no store; the cover is re-validated before the hop leaves the node | — |
| T9 | under the emptied node's lock and its parent's, at most two held; the node is marked obsolete through its lock and retired. When the parent cannot be taken or the engine's removal of the entry from a tree-state parent falls back, the node stays linked and empty and the serialised path prunes the key's chain | — |
| T10, T11, T12 | the serialised root-covered path, `RootGrowth`, which quiesces the optimistic writers and holds the tree word | — |

Two departures from §17.2.3's registered fallback set, both narrowing it:

- **A sub-map in `Root::Empty` or `Root::Leaf` state is not a `RootGrowth`
  fallback.** §17.2.3 listed it under "whatever the engine's own OLC path
  already falls back on"; the engine's body still does, but the string writer
  never asks it to: a leaf-state sub-map is mutated under the node's cover
  taken as a lock, which is the purpose §17.2.1 registered the word for. On the
  `routes` Callgrind arms every terminal sub-map holds sixteen entries and is a
  root leaf, so the registered reading would have put every insert and remove
  of `sync_strmap_insert`, `sync_strmap_remove` and `sync_strmap_churn` through
  the quiescing fallback.
- **T9 is attempted optimistically** and falls back only when the parent's
  lock cannot be taken or the engine's removal of the entry falls back. The
  removal itself is never redone: the serialised path runs a prune-only step
  over the key's chain, counted as the one fallback it takes. §17.11 registered
  T9 as staying behind the fallback "because this design does not attempt them
  concurrently, not because they cannot be done"; the `sync_strmap_remove` arm
  empties a terminal sub-map every sixteenth removal, which the registered form
  would have paid a quiescing fallback for.

T10, T11 and T12 stay behind the fallback as registered.

**The two consequences §17.2.1 named** landed in #985 (`4e1fc72b`): `StrNode`
carries the cover word and `ExpanseStrMap` implements `RootState`, answering
`false` so the tree word covers T10–T12 for the whole operation. The node's
size does not move again here: the dirty flag that records a stale sub-map
population sits in the cover word's alignment padding.

**The reader half (§17.2.2)** is as registered: each hop samples the
`StrNode` cover, runs `walk_validated_node` under it, and re-validates it after
the entry it loaded, on the suffix arm and the child arm both; the tree word is
validated once before any answer, for T10–T12. An unlinked node is marked
obsolete before it retires (S3): through its lock on the optimistic prune, and
in `dispose_node` on the serialised path.

**The single-threaded path.** `ExpanseStrMap::insert`, `ins_slot` and `remove`
dispatch once, on the `deferred` state each already loaded, to a plain twin
(`SHARED = false`) that is the previous code, and to a deferred twin that
brackets each node's cover, re-syncs a dirty sub-map's population from a census
fold before reading it, and marks retired nodes obsolete. The plain-tree arms'
precondition (§17.3) is read on the head's own `instruction-counts` job.

**The comparison build.** The head's default build is the design;
`ablation-str-serial-writers` restores the serialised protocol — every
mutation through `Shared::write` under the whole-operation tree bracket, via
the deferred twin, readers unchanged — and is the "main" build of §17.3's
ratio. The driver lists it in `INVERSE_ABLATIONS`, so the ratio it reports is
`C_default(W) / C_variant(W)`, which is `C_head(W, r) / C_main(W, r)`.

**§17.7's prerequisites.**

1. Met: the new path is the default and the feature exposes the protocol it
   replaced, so one interleaved two-build comparison per pin per run is one
   dispatch.
2. Met: `writer_scaling_929_str_gate` in `.github/bench-suites.json`, the
   workflow's case block and its upload list.
3. Met, with a correction to the registration's premise: the driver already
   emitted the per-round ratio of the two builds' C(W) with its BCa 95%
   interval — `compute_paired_scaling_ratios`, written for the #568 ablation
   arms — so no new reduction was needed. `--gate-929-str` reads those
   intervals in §17.9's vocabulary, adds the W = 1 control's interval and
   §17.2.3's fallback verdict per W, refuses a round count or writer set other
   than the registered ones, and voids a run at an unregistered pin or a
   `--quick` population. Its self-test pins each verdict against a synthetic
   comparison.
4. Met by construction: an optimistic fallback quiesces exactly once
   (`remove_root_covered`), and a deferred prune is counted as the one fallback
   it takes.
5. Met: the artifact carries `gate_929_str.preregistration` naming this
   section.

**Explicitly not claimed here.** No number. Whether the head meets the gate is
what the four dispatches decide, and each is appended to `README.md` whatever
it says.

## 18. Pre-registration for #730's readers-only measurement — the FFI string cell's R-curve (appended 2026-09-15, locked before any cell of the sweep runs)

**Status: commit 2 of the three-commit cadence (AGENTS.md §8.8), locked before
any cell of the R sweep runs.** It carries no measurement of its own. Every
figure quoted below is read from an artifact already committed and named beside
it, or from the reduction `README.md` §13 publishes, which this section invokes
rather than restates. Commit 1 is `scripts/reader_scaling_bounds.py`. The data
commit that follows names the SHA at which this section was pushed. Nothing here
is rewritten in place once a run exists (AGENTS.md §8.7); a threshold, statistic,
pin, estimator or round count changed after a run relabels that run
`INTERMEDIATE` (§8.19). §1–§17 are not edited.

Tracking issue: [#730](https://github.com/orieg/expanse/issues/730) (open).

**This section registers a measurement, not a gate.** It registers what the R
sweep on the FFI string cell predicts, what refutes each prediction, what
controls run beside it, what voids a cell, and what the design cannot answer.
It registers nothing that licenses shipping a change. §16 owns #730's fix gate
and is not restated, extended or competed with here (§18.1).

### 18.1 §16 owns the fix gate; this section does not re-register it

§16 locks #730's fix gate as an **outcome** gate: the `str` readers-only cell's
per-reader cost interval upper bound below a floor registered per pin —
259.198 ns at pin `0-15`, 261.062 ns at one thread per physical P-core — with an
R = 1 non-regression side condition, over two pins × two independent runs
*(workload: `concurrency_readers_str`)*. That gate is met or not met entirely
within §16. **No cell of this section's sweep is an input to it, and no verdict
here can meet, fail or amend it.**

Two things follow, and both are deliberate.

**This sweep measures a different cell from the one §16 gates.** §16's gate
reads the native readers-only instrument (`writer_scaling_readers_only`, one
harness process per timed cell, the `str` arm's own prefill and a probe stream in
which every probe hits). This section's sweep reads the FFI cell
`masstree_conc_str`, which is a different harness, a different probe mix and a
different process layout *(workloads differ: `concurrency_readers_str` vs
`masstree_conc_str`)*. The two are not interchangeable and no figure from one is
compared with a figure from the other anywhere in this section.

**Where this section's source plan carried a gate, that gate is not registered.**
The plan this section is adapted from stated its own fix gate, and it differed
from §16 in two respects that are recorded here so the divergence is visible
rather than silently resolved:

- **A Masstree-parity clause.** The plan's gate required, as a second
  condition, that the R = 8 per-reader cost be at or below the Masstree twin's
  on the same run pair — the issue's competitive claim restated as parity on
  symmetric pages. **§16 carries no such clause**, and §16.11 states in terms
  that no comparison with Masstree or HOT is made and no ratio against a
  competitor is formed. This section does not add one. A competitor ratio is
  not registered here, is not a prediction here, and does not become a gate
  condition by appearing in a measurement this section governs.
- **A W = 1, R = 8 non-regression floor.** The plan's gate carried one. §16's
  side condition is the R = 1 cell, and §16 registers no W = 1 floor. This
  section does not add one either; the W = 1, R = 8 cells it runs are controls
  (§18.6), reported with their per-round series and attached to no threshold.

The plan also referenced a suite README section by a number that section no
longer holds. Where it did, this section names the artifact instead.

### 18.2 What is already decided, and what is not

`README.md` §13 publishes the free reduction of the committed rounds
(`scripts/reader_scaling_bounds.py --table`, byte-compared by that module's own
self-test). It is the prior observation set for this registration and is cited,
not restated. What it settles *(workload: `masstree_conc_str`)*:

- **The two committed level families come from two procedures**, and the
  between-run spread within a procedure is far smaller than the gap between
  them: 34.888 and 34.895 M lookups/s for one harness process running every
  round, against 28.651 and 28.713 pooled for one process per round. Which of
  procedure, harness or engine separates the families is **not** decided by the
  committed data.
- **Rounds are not exchangeable in every artifact.** Round 1 is the lowest round
  in three of the four one-process artifacts and lies beyond three scaled MADs
  of the median in each.
- **36% of the R = 1 → R = 8 growth in ns is the core clock**: 15.35 ns of
  43.00 ns, from `frequency_share` on the committed counter pair. Cross-R
  thresholds in this section are therefore stated on **cycles**, never on ns.
- **Instructions fall** from R = 1 to R = 8, 477.17 to 476.54 per probe, with
  the R = 8 interval entirely below R = 1's.
- **The counter budget does not close.** `LLC-load-misses` per probe fall
  (2.579 → 2.383); the `xsnp_hitm` and `l2_rqsts.rfo_miss` growths can cover at
  most 7.80–31.18 and 0.081–0.325 cycles of the 136.15-cycle growth, and those
  ceilings rest on a **hypothesised** per-event cost that no committed artifact
  prices. At the upper end of that hypothesis ≥ 104.64 cycles per probe are
  left with no counter behind them. That residue is reported as unexplained and
  is assigned to no mechanism by subtraction (AGENTS.md §8.20.4).
- **The reader-slot line hypothesis is refuted by arithmetic before any run**:
  the read path stores to shared memory twice per probe, `l2_rqsts.rfo_miss`
  grows by 0.0008 per probe, and the only reader of another thread's slot is
  reachable only from a write path, which nothing calls at W = 0.

Undecided, and what this sweep addresses: which level is current and why the
families differ; whether the R-growth survives on cycles once the clock is held
fixed; and the concurrent magnitude of the page-size asymmetry
`METHODOLOGY.md` §3.3 already discloses.

### 18.3 Instrument audit — what exists, and what a prediction waits on

Every field, helper and selector named by a threshold below was opened before
the threshold was registered. A threshold registered against an instrument that
does not exist is unrunnable, so each row states its status and each prediction
in §18.4 is labelled **runnable** or **contingent** accordingly. Nothing missing
is substituted.

| instrument the plan names | status | evidence |
|---|---|---|
| `reader_scaling_bounds.per_arm_interval` | **exists** | `scripts/reader_scaling_bounds.py:193@686dd6cb` |
| `reader_scaling_bounds.mde_from_rounds` | **exists** | `scripts/reader_scaling_bounds.py:225@686dd6cb` |
| `reader_scaling_bounds.frequency_share` | **exists** | `scripts/reader_scaling_bounds.py:239@686dd6cb` |
| `reader_scaling_bounds.unexplained_cycles` | **exists** | `scripts/reader_scaling_bounds.py:277@686dd6cb` |
| `reader_scaling_bounds.max_over_mean_bias` | **exists** | `scripts/reader_scaling_bounds.py:295@686dd6cb` |
| `reader_scaling_bounds.hugepage_ceiling` | **exists** | `scripts/reader_scaling_bounds.py:306@686dd6cb` |
| `reader_scaling_bounds.round_outliers` | **exists** | `scripts/reader_scaling_bounds.py:206@686dd6cb` |
| `reader_scaling_bounds.threshold_a` | **absent** | no definition anywhere under `scripts/`, `crates/` or `docs/` |
| `hitm_cycle_ceiling` / `rfo_cycle_ceiling` | **absent under those names** | the generic `event_cycle_ceiling(per_probe_r1, per_probe_rk, cost_cycles)` exists at `scripts/reader_scaling_bounds.py:266@686dd6cb` and takes the per-event cost as an argument |
| `instructions`, `cycles`, `ref-cycles`, `task-clock`, `LLC-load-misses`, `l2_rqsts.rfo_miss`, `mem_load_l3_hit_retired.xsnp_hitm`, `context-switches` per thread | **exist** | `scripts/bench_counters.py:147-156`; the eight `reader/*` keys of `results/counters_masstree_conc_str_w0_r{1,8}.json` |
| `cycle_activity.stalls_l3_miss` | **absent from the per-thread set** | not in `THREAD_EVENTS`, `scripts/bench_counters.py:147-156`; absent from both committed cells' `events` |
| `dTLB-load-misses` per thread | **absent from the per-thread set** | present in `BASE_EVENTS` (process mode) at `scripts/bench_counters.py:132`, not in `THREAD_EVENTS`; absent from both committed cells' `events` |
| `mem_load_l3_hit_retired.xsnp_{fwd,none,miss}` | **absent** | only `xsnp_hitm` is registered, `scripts/bench_counters.py:139` |
| `machine_clears.memory_ordering` | **absent** | not in any registered event set |
| per-cell extra events | **mechanism exists, unused by #730** | `Cell.extra_events` and `Cell.events()` at `scripts/bench_counters.py:238-242@686dd6cb`; the precedent is `OPTIMISTIC_EXTRA_EVENTS` at `:169`. No #730 cell requests any extra event today |
| `_masstree` counter cells (`masstree_conc_str_w0_r{1,8}_masstree`) | **absent** | `_conc` hardcodes `arm="expanse"`, `scripts/bench_counters.py:248@686dd6cb`; the registry holds only `masstree_conc_str_w0_r1` and `_r8` at `:287@686dd6cb`, `:290@686dd6cb` |
| harness `--arm <expanse\|masstree>` | **exists** | `crates/expanse-hot-bench/src/bin/masstree_concurrent.rs:540`, documented at `:42` |
| `masstree_conc_str_w0_r{2,4}` counter cells | **absent** | not in the registry |
| W = 0 at R ∈ {2, 4} in the throughput driver | **absent** | `CONCURRENT_MIXED_READERS = 8` is a scalar, `docs/benchmarks/masstree_comparison/scripts/run_all.py:66`; the C2 grid is W ∈ {0, 1, 2, 4, 8} × R = 8, so R = 1, 2 and 4 at W = 0 are not driver cells. R = 1 exists only as a `bench_counters.py` cell |
| `run_all.py --cells` | **absent** | `main()` at `docs/benchmarks/masstree_comparison/scripts/run_all.py:681` selects with `--quick`, `--concurrent`, `--only-concurrent`, `--ab-base-bin`, `--ab-base-commit` and `--self-test` only |
| `scripts/perf_counters.py` with `--arms strmap_get --pops --hit-pcts --runs` | **exists** | flags at `scripts/perf_counters.py:813-817`; `strmap_get` is a documented `EXPANSE_PERF_ARM` value at `crates/expanse/examples/perf_point_lookup.rs:39`, read at `:242`, unknown arms refused at `:341` |
| `GLIBC_TUNABLES=glibc.malloc.hugetlb=1`, verified by `AnonHugePages` | **exists as committed practice** | `docs/benchmarks/masstree_comparison/README.md:597-600`, verified before interpretation |
| `load.foreign_busy_cpus_since_prev` | **exists** | `scripts/check_bench_provenance.py:924@686dd6cb`, `:927@686dd6cb`; per-cell `load.foreign_busy_cpus` required at `:514-541@686dd6cb` |
| `EXPANSE_BENCH_PIN`, one thread per physical P-core | **exists** | `scripts/bench_pin.sh` |

**What this audit costs the campaign.** Two of the five predictions are
contingent on instrument work that has not landed, and the R ∈ {2, 4} arm of the
curve needs both a cell selector and an R-list generalisation in the driver
before it can be dispatched at all. Those are named as preconditions below, not
worked around.

### 18.4 Predictions, each with its refuter

The source plan labelled these P11.0–P11.4; they are renumbered to this section.
Every cross-R threshold is on **cycles per probe**; same-R thresholds are on ns.
Thresholds are fixed here: 1.15×, 50%, 25%, 0.1, 8%, 4%, 0.01 and the control's
1.15. Any later change to one of them is an `INTERMEDIATE` relabel with fresh
rounds (§8.19).

- **P18.0 — which level is current, and why the families differ. Runnable.**
  Observations: **L** (the `a1982ff2` tree under its own harness and procedure,
  re-run), **P** (head tree, the `sweep_concurrent` procedure, the single W = 0
  R = 8 string cell), **B** and **H** (the base and head halves of a two-commit
  AB artifact), **I** (instructions per probe at head against the committed
  476.54 [476.50, 476.65]). Predicted: P ≈ H, so the procedure is not the cause;
  L high and B low, so harness or engine is; I unchanged, in which case the
  engine branch needs cycles rather than counts to survive. **REFUTED as
  "engine"** if B overlaps H with I unchanged and L ≈ P. **REFUTED as
  "harness"** if P and H separate, in which case the procedure is the cause and
  both committed families are right about different instruments. **Any verdict
  read without L, P and I is void.** Because the per-process layout mode is
  bimodal at the sibling cell, L and P run at least 15 rounds each and their
  per-round series is published; a bimodal series is reported as bimodal, never
  as a mean.
- **P18.1 — the cost is mostly per-probe. Contingent on `threshold_a`.**
  The prediction is that the head R = 1 reader cycles-per-probe interval lower
  bound is at or above `reader_scaling_bounds.threshold_a(...)`, instantiated
  from the hit = 50 `strmap_get` comparator's interval upper bound and committed
  before the R ≥ 2 cells are read. **REFUTED** if the head R = 1 cycles interval
  upper bound is within 1.15× of the comparator's lower bound. `threshold_a`
  does not exist (§18.3); this prediction is registered as contingent on it
  landing in `scripts/reader_scaling_bounds.py` with a pinned reference value,
  and **is not evaluated until it does**. No substitute statistic is registered
  in its place. The comparator run itself is runnable today.
- **P18.2 — the R-growth is loaded memory latency rather than line transfer.
  Contingent on three absent instruments.** Registered on cycles:
  `cycle_activity.stalls_l3_miss ÷ read_ops` grows monotonically over R with
  non-overlapping intervals and accounts for at least 50% of the cycles growth
  by `event_cycle_ceiling` and `unexplained_cycles`; `xsnp_hitm` growth stays at
  or below 25% of it at the highest plausible per-event cost; `LLC-load-misses`
  does not rise; and the Masstree twin's R = 8 ÷ R = 1 reader cycles ratio is
  within 0.1 of Expanse's. **REFUTED** if `xsnp_fwd` + `xsnp_hitm` growth
  explains at least 50% of the cycles growth at the *lowest* plausible cost, or
  if a `PA cnt == 1`-filtered c2c report names an engine line with the sample
  floor met. Confidence: medium on the sign, low on the share. This prediction
  waits on `cycle_activity.stalls_l3_miss` and the `xsnp_{fwd,none,miss}`
  breakdown being added to the per-thread event set, and on the `_masstree`
  counter cells existing; all three are absent (§18.3). Until then it is
  registered and **not evaluated**. The per-event cost in every ceiling remains
  a stated hypothesis, not a measurement.
- **P18.3 — instructions do not grow with R. Runnable.** Single criterion: the
  head R = 8 `instructions ÷ read_ops` interval upper bound is at or below the
  R = 1 interval upper bound. **REFUTED** if it is above. The committed prior
  shows a 0.6-instruction fall with non-overlapping intervals, so a fall
  confirms.
- **P18.4 — the concurrent magnitude of the page-size asymmetry. Partly
  contingent.** Read on the single-arm counter cells, where the process-wide
  tunable reaches one engine at a time: R = 1 reader cycles per probe fall by at
  least 8%, `dTLB-load-misses` per probe fall below 0.01, instructions are
  unchanged, and the R = 8 ÷ R = 1 cycles ratio does not rise. **REFUTED** if
  R = 1 cycles fall by less than 4% with `AnonHugePages` verified non-zero. The
  cycles and instructions clauses are runnable; the `dTLB-load-misses` clause is
  **contingent** on that event being added to the per-thread set, and is not
  evaluated until it is. The single-threaded prior for the treatment is a −11.2%
  change in cycles per probe *(workloads differ: `strmap_get` single-threaded at
  `b1868813` vs `masstree_conc_str`)*, which is a prior and not a prediction for
  a concurrent cell. On the two-arm throughput sweep the tunable also reaches the
  competitor's incidental allocations, so the ratio under it is published as
  paired-process rather than as a one-arm delta.

### 18.5 Counters here are diagnostic, and are not gate inputs

§16 registered **no counter threshold** and labelled counters taken on an
evaluated head diagnostic, because the module's event ceilings convert counts to
cycles through a per-event cost that no committed artifact prices. P18.1, P18.2
and P18.4 read counters and attach thresholds to them. That is consistent with
§16 and does not amend it: **a threshold on a diagnostic prediction is not a gate
input.** Stated so no later reader mistakes one for the other —

- a **refuted P18.2 is not a failed gate**, and a confirmed P18.2 is not a met
  one. §16's gate reads per-reader cost on a different cell and reads no counter
  at all;
- no verdict in this section makes §16's gate met, unmet, harder or easier;
- every cycles ceiling quoted here carries its hypothesised per-event cost, and
  the unexplained remainder stays unexplained (AGENTS.md §8.20.4);
- the counter figures are exact per-round counts reduced to means and intervals;
  where a figure is a deterministic count it carries no interval, which is
  correct rather than missing.

### 18.6 Controls

- The integer cells at W = 0 over the same R values, both suites, with an
  R = 8 ÷ R = 1 cycles ratio at or below 1.15 — the control that separates a
  string-specific effect from a host-wide one.
- **The Masstree twin's own R-curve**, run in the same sweep. It is the free
  symmetric baseline (AGENTS.md §8.3): if the competitor's R = 8 ÷ R = 1 cycles
  ratio degrades by the same factor, the degradation is the machine and the
  differential explanation is dead. It is a control on a mechanism question, and
  it is **not** a gate condition and forms no published competitive ratio
  (§18.1).
- Every existing Callgrind arm at 0.00%: this campaign changes no engine code,
  so any movement is an instrument defect, not a result.
- The W = 1, R = 8 string throughput and health cells, reported with their
  per-round series and attached to no threshold.

### 18.7 What voids a cell

§6 applies in full. In addition:

- any verdict on P18.0 read without all three of L, P and I;
- any cell whose `load.foreign_busy_cpus_since_prev` exceeds 1.0, or a load
  shift above 2 between the halves of a pair;
- a c2c round taken from an `occ-stats` build;
- thread placement not recorded, or not one thread per physical P-core;
- a bimodal per-round series reduced to a mean;
- any threshold evaluated on a median, or on ns across R rather than cycles;
- an evaluation of a prediction §18.3 marks contingent, taken before the
  instrument it names exists.

A void cell is discarded whole and the discard is disclosed beside the result
(AGENTS.md §8.17), never silently.

### 18.8 What this design cannot answer

- **The scaling coefficient behind reader–reader sharing is not identifiable on
  an eight-P-core pin.** The clock term at R = 4 is a large share of the
  discriminating quantity, and the retrograde point of a pure-coherency model
  lies outside the pin. R ∈ {2, 4} are published as the curve and are used in no
  threshold.
- **A closed loop with no think time and R at or below the core count measures
  the saturation asymptote and locates no knee.**
- **Nothing about the fix.** Whether any change meets #730's gate is §16's
  question on §16's cell, and no measurement registered here answers it.
- **No competitive claim.** No ratio against Masstree or HOT is formed, and the
  retracted readers-only string figures remain retracted and are not restored,
  re-derived or replaced by anything in this section.
- **Not predicted:** writer-count cells, the bytes and blob wrappers, any other
  host, and any level, direction or magnitude for a future head.

### 18.9 Withdrawn, 2026-09-21

This registration is withdrawn before any cell of its sweep ran. §18.1–§18.8
above are unchanged and stand as the record of what was registered.

- **It cannot run as registered.** By §18.3 and §18.4's own labels, P18.1 and
  P18.2 are contingent on instruments that do not exist and P18.4 is partly
  contingent. The R ∈ {2, 4} cells
  cannot be dispatched without a driver change (§18.3).
- **The question behind it has changed.** #730 was opened on a competitive loss
  that is retracted and has since inverted (§16.12), and #730 is closed as
  superseded.
- A reader-count measurement of the FFI string cell, if one is wanted later, is
  registered afresh against #1061 rather than resumed from this section.

## 19. Pre-registration for #929, second gate — the `SyncExpanseStrMap` multi-writer path as a priced trade (appended and locked 2026-09-17, before any admissible run of it)

### 19.1 What §17 decided, and what this section is not

§17's gate was evaluated at `23425a75` (PR #1001) as far as its own text allows,
and it is **not met**; `README.md` §17 is the record. Its Callgrind precondition
fails on five `sync_strmap_*` mutation arms (`instruction-counts` run
[35187216693](https://github.com/orieg/expanse/actions/runs/35187216693):
+18.78% to +40.44% against a bound of +5.0%, no override admissible), so under
§17.8 no gate dispatch was taken. Two diagnostic runs (§19.2) show that §17's
W = 1 control would also have failed: §17.8 expected the single writer
"unchanged", and it reads 0.956–0.961 of the serialised build with every
interval wholly below 1.0.

AGENTS.md §8.19 leaves two moves after a falsifier trips: change the code and
re-measure against the same bound, or record the rejection. Two code changes
were measured against the same bound and do not reach it (`README.md` §17.3);
the rejection is recorded. **This section does not reopen §17.** §17's text,
bound and verdict stand as written. What follows is a different claim with a
different statistic, registered before any admissible run of it: §17 asked for
multi-writer scaling at no single-writer cost, and the evidence says that
design does not exist at this protocol; §19 asks whether the path is worth a
**stated, bounded** single-writer price.

### 19.2 What had been seen when this was written

A registration written after looking is weaker than one written before, and the
reader is owed the list (AGENTS.md §8.7, §8.19):

- the CI Callgrind table on `23425a75`, and Docker Callgrind attributions of
  the string arms (`README.md` §17.2–§17.3);
- two diagnostic throughput runs per pin at `23425a75`, default build against
  `ablation-str-serial-writers`, the driver's generic `--compare` mode, 8
  rounds, one process per cell — four artifacts,
  `results/diagnostic_929_str_serial_writers{,_pin0to15}_23425a75_run{1,2}.json`.
  They are **not** evaluations of §17 (its gate mode was never dispatched) and
  are **never** inputs to this gate.

Consequences, fixed here: every threshold below is either carried over from §17
unchanged, or derived from something those runs could not inform, or marked as
**maintainer policy set with knowledge of them**; and the gate is evaluated
only on fresh runs taken after the lock.

### 19.3 The claim this gate would license, in full

> On the reference host, at the registered pins, `SyncExpanseStrMap`'s
> per-node OLC write path delivers more insert throughput at every writer count
> W ≥ 2 than the serialised build delivers at **any** writer count, and its
> single-writer throughput is at least the stated fraction F of the serialised
> build's. *(workload: `concurrency_writer_str`)*

Nothing about reads, removals, skewed access, other hosts, or the bytes and
blob wrappers.

### 19.4 The gate

Stated verbatim, and evaluated per cell. `T_b(W, r)` is `writer_mops` of build
*b* at W writers in round *r* of one interleaved comparison run; `head` is the
default build and `serial` the same commit built with
`ablation-str-serial-writers`.

> **G1, scaling — §17.3's statistic, unchanged.** R(W, r) =
> [T_head(W, r) ÷ T_head(1, r)] ÷ [T_serial(W, r) ÷ T_serial(1, r)]. A cell
> passes iff the BCa 95% lower bound over the round series is strictly above
> 1.0.
>
> **G2, level.** L(W, r) = T_head(W, r) ÷ max over W′ ∈ {1, 2, 4, 8} of
> T_serial(W′, r). A cell passes iff the BCa 95% lower bound is strictly above
> 1.0.
>
> **G3, price.** P(r) = T_head(1, r) ÷ T_serial(1, r). A cell passes iff the
> BCa 95% lower bound is at least **F = 0.90** — maintainer policy, set at the
> lock with knowledge of §19.2's diagnostics.
>
> G1 and G2 cells are W ∈ {2, 4, 8} × two pins (`0-15`,
> `0,2,4,6,8,10,12,14`) × two independent runs: twelve each. G3 has one cell
> per (pin, run): four. **The gate is met at a head** when all twenty-eight
> pass. BCa 95%, 2,000 resamples, `scripts/bca_bootstrap.py`.
>
> **Callgrind, preconditions.** On the head's own `instruction-counts` job:
> every plain-tree arm, and every `sync_map_*` and `sync_set_*` arm, within
> AGENTS.md §6's +0.1% of main — the paths this change does not target. The six
> `sync_strmap_*` arms are **expected over the automated threshold**; this
> registration pre-authorises one `allow-regression:` line naming exactly those
> arms and citing that job's run, and nothing else. **Defect tripwire,
> deterministic:** the `occ-stats` replay of the five `sync_strmap_*` mutation
> arms records zero `lock_restarts`, and fallbacks within §17.2.3's bounds.

**Why G2 exists.** R divides by each build's own single-writer cell
(AGENTS.md §8.20.2), which is right for separating scaling from baseline speed
and wrong as the only statistic when the head's single writer is *slower*: a
slower W = 1 raises C_head(W) by itself. L cannot be passed that way — it
compares absolute throughput with the best the serialised build does anywhere.

**Why the Callgrind clause changed shape.** §17.3 bounded the string arms
against the serialised wrapper at +5.0%. On the same job the map wrapper's OLC
path costs +87% over the plain map per insert, and the string OLC wrapper +81%
over the plain string map (48,860,697 ÷ 26,190,927 and 81,239,845 ÷ 44,913,726,
run 35187216693): the string path pays what the protocol already costs where it
shipped, and a bound near zero against a mutex wrapper asked for OLC at mutex
cost. Instruction count is kept where it is decisive — untargeted paths, which
wall clock cannot protect at 0.1% — and the single-writer price is gated on the
quantity a user pays, G3. The tripwire is the instrument that actually caught
this path's one defect (`README.md` §17.2: 64 restarts per operation, invisible
to every test).

**On F.** No measurement fixes what single-writer price is acceptable; it is
policy: one writer may be at most 10% slower than under the serialised
protocol. It was set with knowledge that the diagnostics read 0.956–0.961, and
is declared as such. Independently of them: it sits below 1.0, or §19 would be
§17 again, and far enough from the expected level that the instrument resolves
the difference (§19.5). It does not move for any head (§8.19).

### 19.5 Math-first audit

`reader_scaling_bounds.mde_from_rounds` (the function §17.6 used; reference
value pinned in `SyntheticTests.test_mde_hand_value`), applied to the four
diagnostic round series at 8 rounds:

| statistic | relative MDE, range over (pin, run) |
|---|--:|
| P | 0.47%–1.21% |
| L(2) | 0.94%–1.62% |
| L(4) | 2.87%–4.70% |
| L(8) | 3.02%–8.34% |

A floor of 0.90 is resolvable against a level near 0.96 (gap 0.06, MDE at most
0.012), and so is any floor down to about 0.95. L's observed levels (§19.2's
artifacts: 1.57–1.59, 2.84–2.87, 5.06–5.21) are far outside its MDE. §17.6's
table stands for R. Not established: that a future head's spread resembles
these series; the round count does not change for it (§8.19).

### 19.6 Rounds, pins, runs, isolation, voids

§17.7 and §17.10 apply unchanged: 8 rounds, both pins, two runs per pin, one
process per cell, populations 1,048,576, BCa, no `occ-stats` timings, the four
runs of one evaluation identical in `crates/` and in the driver. An artifact
naming §17's gate mode is not an evaluation of this section.

**Instrument prerequisite (AGENTS.md §8.20.7).** `writer_scaling.py
--gate-929-str` computes G1 and §17's control only. G2 and G3 are added as a
second gate mode with its own artifact block and self-test assertions — that a
missing round, a wrong pin or an absent W′ cell yields `NOT_EVALUABLE`, never a
pass — before any run. The G1 code path is not edited.

### 19.7 Expected losses

| cell or condition | expectation at lock | consequence of a loss |
|---|---|---|
| G3, every cell | **about 0.96**, both diagnostics, both pins | lower bound under F: the gate is not met; F does not move (§8.19) |
| G2, W = 2 | the closest level cell, about 1.57 | `INCONCLUSIVE` or `REFUTED`: not met |
| G1, W = 8, one thread per core | very large (about 44), because the serialised build collapses to about 0.49 M ops/s under that pin (AGENTS.md §8.20.5 step 0); it says more about the mutex arm's pin sensitivity than about the head, which is why G2 is beside it | none expected |
| G1, W = 8, `0-15` | widest interval of the set (diagnostics: [9.6, 18.2] and [10.6, 23.3]) | `INCONCLUSIVE` possible at a real gain; reported, the pin is not dropped |
| structural fallback rate | §17.2.3's bounds, unchanged | `REFUTED` on that prediction, published regardless |
| `sync_strmap_*` Callgrind arms | over the automated threshold, as at `23425a75` | none — reported with the override this section pre-authorises |
| untargeted Callgrind arms | within +0.1% | a precondition failure; no throughput run is taken |
| removal and short-key workloads | **not measured by this gate**: `concurrency_writer_str` inserts fresh route-shaped keys; the short-key Callgrind arms are the costliest (+25.6%, +40.4%) and no throughput cell exercises them | stated in the promotion text; a follow-up cell, not a condition here |

### 19.8 Verdicts

§17.9's vocabulary, unchanged: `PASS`, `REFUTED` (interval wholly on the wrong
side of the cell's threshold), `INCONCLUSIVE` (threshold inside the interval),
`INTERMEDIATE` (anything differing from this registration), `NOT_EVALUABLE`.
Met only when all twenty-eight cells read `PASS`. A further run added to decide
an `INCONCLUSIVE` relabels the evaluation `INTERMEDIATE`.

### 19.9 If it is met: what the promotion must say

The default build's single writer is slower than the serialised build by the
measured P, stated with its interval in `docs/ARCHITECTURE.md` §4 and the
wrapper's rustdoc, beside the `ablation-str-serial-writers` feature that
restores the serialised protocol (AGENTS.md §2.7). Not claimed: reads, removes,
short keys, skew (#1006), other hosts, the bytes and blob wrappers.

### 19.10 Correction of two drafting errors (2026-09-17, after the lock and before any admissible run)

No threshold, statistic, pin, round count, cell or verdict rule changes. §19.1–§19.9
are left as locked; where they disagree with this subsection, this subsection is
what was meant, and the disagreement is stated rather than edited away
(AGENTS.md §8.7). Found while checking the preconditions at the evaluated head
`1abfb7ff`, before the first dispatch.

**1. What the gate workload is.** §19.7's last row and §19.9 describe
`concurrency_writer_str` as inserting route-shaped keys and the short-key
workloads as unmeasured. That is backwards. §17.2.3 registers the workload: a
2^20-key prefill, then 2^20 fresh **8–16 byte alphanumeric** keys inserted by W
writers over disjoint slices — one or two hops, essentially every insert a T2
into the root node's tree-state sub-map. So this gate **does** measure
short-key inserts, the shape of the `sync_strmap_insert_short` Callgrind arm,
and does **not** measure multi-hop route-shaped keys (the `routes` arms'
shape), removals or churn on any key shape, or access skew (#1006). A promotion
text says so (§19.9).

**2. Where §17.2.3's fallback bounds are read.** §19.4's tripwire sentence asks
the `occ-stats` replay of the Callgrind arms for "zero `lock_restarts`, and
fallbacks within §17.2.3's bounds". §17.2.3 derives and defines those bounds on
`concurrency_writer_str`, per W, over the harness's `write_ops` (§17.3's input
table), and that is where they are read: the driver's `fallback_prediction`
block in each gate artifact, a published prediction that is `REFUTED` when
exceeded, as §17.8 and §19.7 already state. The replay half of the tripwire is
**zero `lock_restarts` on the five mutation arms**, the signature of the one
defect this path has had. The bounds were never derived for the Callgrind arms'
workloads, and one of them does not meet the number: `sync_strmap_remove/routes`
takes 284 `branch_split` fallbacks and one `root_growth` in 50,285 operations,
a structural rate of 5.7 × 10^-3 — the engine's removal path falls back on a
branch split by design (§17.2.3's `FallbackCause` set). That figure was
published before the lock (`README.md` §17.2) and the clause was written over
it; it is recorded here, not excused. Replay at `1abfb7ff`: zero restarts on
all five arms; fallbacks 44 of 50,044 operations on `insert/routes` (43
`branch_split`, 1 `root_growth`), 285 of 50,285 on `remove/routes`, 1 of 50,001
on `insert_short`, 0 on `churn/routes` and `churn_short`.

## 20. Pre-registration for #1006 — a concurrent YCSB suite for `SyncExpanseMap`: Zipfian, read-latest and read-modify-write (appended and locked 2026-09-17, before any harness code or any run of it)

AGENTS.md §8.8 commit 2. Commit 1 is `scripts/ycsb_concurrent_bounds.py`; no
harness exists, no cell has run, and nothing in this section is a result.
§1–§19 are not edited. **Locked 2026-09-17**, with every policy value of
§20.11 set by the maintainer on that date. From here this section is never
rewritten in place (AGENTS.md §8.7): outcomes go to `README.md`, and a
correction is appended as a dated subsection, as §19.10 was.

### 20.1 What is registered, and what is not

Every timed concurrent cell this suite has draws keys uniformly or from
disjoint streams, so the optimistic-lock-coupling results for `SyncExpanseMap`
describe the regime in which two threads rarely aim at one node. This section
registers four YCSB-shaped cell families in which they do — A, B, D and F — on
one key type (`u64` keys, 8-byte values), against one α = 1 control and three
competitor arms, with the statistics, thresholds, rounds, pins and void rules
fixed before a harness is written. Beside them sit ungated anchor cells
(§20.4), two diagnostic passes that are never gate inputs (§20.14) and a
correctness oracle that is not a timing (§20.15).

**Workload E is not registered here, and the reason #1006 gives for that is out
of date.** #1006 defers scans to "the ordered-OCC work tracked in #900". #900
was closed as completed on 2026-09-17: `MapReader` carries `first`, `last`,
`next_at_or_after`, `next_after`, `prev_at_or_before` and `prev_before` as
validated optimistic reads (`crates/expanse/src/sync.rs`, the `MapReader`
ordered block; §12). What landed is *operations, not a cursor*: a 10–100 item
scan over the concurrent surface is 10–100 independent validated `next_after`
calls with no snapshot across them, while `RwLock<BTreeMap>::range` under a
read guard **is** a snapshot and `SkipMap::range` is neither. Registering E
therefore needs a decision this section does not make — what a scan means per
arm, and whether the arms are comparable at all under AGENTS.md §8.3 — and it
is left to a registration of its own (§20.11, D9). Nothing here waits on #900.

**`run_concurrent_ycsb` is not the instrument.** It now lives at
`crates/expanse/benches/ycsb_common/mod.rs` (moved from `benches/ycsb.rs` by
#1011; #1006 cites the old location) and is still called only from
`crates/expanse/tests/test_ycsb.rs`. #1006's description of it holds at the new
location: D and E are the B mix on Zipfian keys, F is `reader.get` followed by
`map.insert` with nothing excluding a second thread between them, the window is
`thread::sleep`, and rates divide by the nominal duration. It stays a smoke
test.

### 20.2 What had been seen when this was written

AGENTS.md §8.7, §8.19 — the reader is owed the list:

- `README.md` §15's uniform-stream writer baselines at `170a4bc3` (`map` C(8)
  2.233–2.280, `set` 1.990–2.031 *(workloads: `concurrency_writer_map_64bit`,
  `concurrency_writer_set_63bit`; intervals in README table 15.2)*), and §19's
  text, including its report that on `instruction-counts` run 35187216693 the
  map wrapper's OLC insert retires 87% more instructions than the plain map's.
- No concurrent Zipfian, read-latest or read-modify-write cell of any arm, on
  any host. No single-threaded result of the #1005 YCSB re-measure was read
  while writing this.
- The outputs of `scripts/ycsb_concurrent_bounds.py`, quoted in §20.7.
- `README.md` §18.2's cells of the second #929 gate at `1abfb7ff`, read for the
  level identity of §20.6 and for nothing else *(workload:
  `concurrency_writer_str`)*.
- **A synthetic four-lens review of this section as first written**, read
  before it was revised and before the lock. It is LLM-generated adversarial
  brainstorming: it found real defects (an unspecified update idiom, a
  head-to-head gate that measured a missing primitive, a price floor
  inconsistent with the level gate, a coincidence model at the wrong
  granularity), they were fixed before the lock, and it is **not** external
  validation of anything here. No reviewer outside the project has read this
  section. Where it supplied a number, the number
  was recomputed by the bounds module or dropped; one claim it made — that a
  USL fit of the published `str` and `map` curves has a negative coherency
  term — was reproduced (`usl_unclamped_beta`, §20.7 (e)) before being
  repeated.

No threshold below is derived from a measurement of the quantity it gates: no
harness exists, so no cell of this suite had been measured by anyone when the
values were set. Those that are policy are collected in §20.11, which says for
each what measured data was known when it was set.

### 20.3 The claims these gates would license, in full

Each is licensed only by its own gate, on the reference host, at the
registered pins, population and thread counts, and says nothing beyond them.

> **C1 (G1 + G2), per family f ∈ {A, B, D, F}.** Under workload f,
> `SyncExpanseMap` scales further with thread count than the same tree behind
> one mutex, **and** delivers more throughput at every T ≥ 2 than that mutex
> build delivers at any thread count.
>
> **C2 (G3), per family f ∈ {A, B, F}.** Under θ = 0.99, `SyncExpanseMap`
> keeps at least the stated fraction of the throughput it has under uniform key
> choice at the same T, mix and population.
>
> **C3 (G4), per family f ∈ {A, B, D} — not F.** Under workload f,
> `SyncExpanseMap` delivers at least the stated fraction of the throughput of
> `crossbeam_skiplist::SkipMap` — **an** ordered concurrent competitor (a skip
> list), not the class of them — at every T.
>
> **C4 (G5), family F only.** Read-modify-write through the registered
> per-arm primitive loses no update in any round of any arm.
>
> **C5 (G6), per family.** `SyncExpanseMap`'s throughput under workload f is
> still rising between four and eight threads.

Against `DashMap` no claim is registered: it is unordered, and the comparison
is published with a direction label and no gate (D7).

### 20.4 Workloads, precisely

**Common to every family.** `u64` keys and 8-byte values in every arm
(AGENTS.md §8.16, payload symmetry): `u64` in `olc` and `mutex`, `AtomicU64`
in `skip`, `dash` and `rwbtree`, for the reason §20.5 gives. Population N = 2^20 = 1,048,576 keys
drawn from the suite's XorShift64 over the full 64-bit space, distinct, present
before the window opens — the population of `concurrency_writer_map_64bit`, at
which a 2-byte-prefix expanse holds 16 keys on average and
`density_poisson.cascade_key_share(16)` puts 0.028% of keys in cascaded
expanses. Prefill order is **sorted ascending in every arm**, the suite's
convention (`writer_scaling.rs` shape table); `RwLock<BTreeMap>` and `SkipMap`
are order-sensitive at build time, no cell inserts into the prefilled range in
A, B or F, and D's inserts are appends in every arm, so one order is declared
and `insertion_order` reads `sorted` (§8.12.4; D10 records the limitation). Initial value of key k is 0 in family F and in every other family a
written value encodes its origin, (t + 1) in the top 8 bits and the index of
the write within thread t's stream in the low 56, with the prefill written as
thread 0, sequence 0 — which is what §20.15's oracle reads.

**Threads.** T ∈ {1, 2, 4, 8}. As in YCSB, every thread executes the family's
whole mix; there is no writer/reader role split. Each thread runs a
pre-generated stream of 2^20 operations built before the barrier from its own
seed (suite seed XOR (t + 1)·φ64, the construction `writer_scaling.rs` uses
for its reader streams), so a cell executes T · 2^20 operations. All arms of a
family consume byte-identical streams (§8.3). Throughput X is total operations
÷ the window, in M ops/s.
**Fixed work per thread joined on the last thread gives an uneven effective
T**: a thread that finishes early leaves the rest running at T − 1. So every
round records each thread's own `thread_elapsed_s`, the artifact publishes
`reader_scaling_bounds.max_over_mean_bias` over them beside every cell, and a
cell whose slowest thread ran far beyond the mean is visible as such rather
than averaged away. No threshold is set on it.

**Ungated extra load points.** On the one-thread-per-core pin only, `olc` and
`mutex` also run T ∈ {3, 6} in families A and B. They enter no gate and no
paired statistic; they exist so the reported scalability fit of §20.6 has six
load points and not four.

**Key choice.** θ = 0.99 (`ycsb_common::ZIPFIAN_THETA`; YCSB's
`ZIPFIAN_CONSTANT`), ranks from `ycsb_common::ZipfianGenerator`, rank r mapped
to the r-th key of the population **in generator draw order**, which is
`ycsb_common`'s rule. Draw order is uniform over the key space, so popular keys
are scattered — what YCSB gets by hashing the generator's output (Cooper et
al. 2010, §5.3). The generator is Gray et al.'s closed form, not an exact
sampler: its share of draws on its k lowest ranks is
`gray_top_k_share(k, N, θ)`, above the exact law's `top_k_share` by at most
0.011 at this N (§20.7). The harness's rank-histogram unit test is held to
`gray_top_k_share`, the law its generator follows.

| family | mix per thread | key choice | what a write is | gated |
|---|---|---|---|---|
| **A** | 50% read, 50% update | Zipfian over the population | a blind overwrite of a present key's 8-byte value, **in place and without allocation**, by the idiom §20.5 fixes per arm | yes |
| **B** | 95% read, 5% update | as A | as A | yes |
| **D** | 95% read, 5% insert | reads: Zipfian over recency, below; inserts: fresh keys | the arm's `insert` of an absent key | yes |
| **F** | 50% read, 50% read-modify-write | as A | value ← value + 1 on a present key, atomically (§20.5) | G1, G2, G5, G6 — **not G4** |
| **A0, B0, F0** | as A, B, F | **uniform** over the population (θ = 0), same seed schedule | as A, B, F | G3's denominator; `olc` only |
| **C, C0** | 100% read | Zipfian; uniform (`olc` only) | none | **no** — anchor |
| **Ac, Fc** | as A, F | Zipfian, **contiguous ranks**: rank r is the r-th smallest key | as A, F | **no** — labels only |
| **A-dram, C-dram** | as A, C | Zipfian over N = 2^24 = 16,777,216 keys | as A, C | **no** — anchor; `olc`, `skip`, `dash`; T ∈ {1, 8} |

Reads are point `get`s; every read in every family names a present key, so
`hit_rate` is 100% and `miss_gen_method` is `n/a` (§8.6) — the miss path is not
measured and not claimed.

**What the ungated cells are for.** *C* has no writes at all, so whatever
separates C from C0 is read-path skew — a hot set that fits in cache, or does
not — and whatever separates A's retention from C's is what writes add. Without
it G3 cannot tell the two apart. *Ac and Fc* are the layout §20.7 (b)
identifies as a trie's structural exposure: popular keys as neighbours under
one leaf and one covering word. *The `-dram` cells* run the same law over a
population sixteen times larger, chosen so the structure does not fit in the
reference host's last-level cache; at N = 2^24 the hottest key takes 5.35% of
the stream and the generator's 256 lowest ranks 34.5% (§20.7). They are
published with intervals and direction labels, never gated, and never pooled
with the 2^20 cells.

**D, in full.** Thread t's j-th insert is key 2^63 + 1 + j·T + t: T interleaved
arithmetic slices of one counter, pre-assigned so no shared counter sits inside
the window. A read draws recency rank ρ from the generator and names thread
t's own (ρ + 1)-th most recent insert, or, once ρ exceeds what t has inserted,
the population key ρ − (t's insert count) places from the **end of the
generator draw order** — `ycsb_common`'s rule with "this stream" read as "this
thread". Two departures from YCSB, both declared: insert keys are monotonic
(Cooper et al. §4.1 note that the latest item "may not be inserted at the end
of the key space"; monotonic keys are what makes D an append workload here),
and recency is per thread and not global, which keeps every read a hit without
a shared high-water mark inside the window. Threads still share leaves: the
slices interleave at stride T. **The append path is not the tree's rightmost
path.** About half of a uniform 64-bit population lies above 2^63, so D's
inserts share one path in the middle of the key space (top byte `0x80`, then
zeros); the chance that a population key falls among the first 2^24 counter
values is about 1e-6 (derivation (c) in the bounds module). #1006's "rightmost
path" is read as "one shared append path" throughout.

**Value dereference and sinks (§8.6).** Every read folds its value into a
per-thread accumulator passed to `black_box` after the window; every write's
return value is folded likewise. In A, B and D a post-window check asserts
every population key present and the population count equal to N plus the
inserts made.

**The window.** Barrier release to the join of the last thread, timed by its
own elapsed time. Prefill, stream generation, Zipfian table construction,
thread spawn, reader-handle registration, the post-run checks and `Drop` are
outside it.

### 20.5 Read-modify-write semantics, per arm

`SyncExpanseMap` has **no atomic per-key update** on current main. Its public
surface is `insert`, `remove`, `clear`, `get`, `len`, `is_empty`, `mem_used`,
`with_locked` (which hands out `&ExpanseMap`, read-only), and the three reader
constructors (the one `impl SyncExpanseMap` block in
`crates/expanse/src/sync.rs`, and `.github/public-api/expanse-trie.txt`); there is no
compare-and-swap, `update`, `entry` or `fetch_add`, and no `with_locked_mut` —
the string and bytes wrappers have one, the map wrapper does not. `get` then
`insert` is two linearizable operations and loses updates. So F needs either
an external lock or a new engine primitive; D1 fixes the external lock.

**One update idiom, fixed per family, applied to every arm that can express
it.** This is not a detail: for
`SkipMap<u64, _>` an `insert` on a present key removes the old entry and links
a newly allocated node, the removed one being reclaimed through
`crossbeam-epoch`, while a `store` into an atomic value cell allocates nothing
(`crossbeam-skiplist-0.1.3/src/map.rs`, `insert`: "If there is an existing
entry with this key, it will be removed before inserting the new one"). The
registered idiom for A, B and F is the
**value-cell idiom**: look the key up through the arm's shared-access read
path and update its 8-byte value in place. Three arms can express it and do;
two cannot, and say so:

| arm | update in A and B | read-modify-write in F | what it excludes | lock symmetry (§8.16) |
|---|---|---|---|---|
| `skip` — `SkipMap<u64, AtomicU64>` | `get(&k)` then `value().store(v, Relaxed)`; no node is replaced | `get(&k)` then `value().fetch_add(1, Relaxed)` | nothing; lock-free | no lock. 0.1.3's `compare_insert` is not used: it returns an entry and not a success flag, so it is not a retry-loop primitive |
| `dash` — `DashMap<u64, AtomicU64>` | `get(&k)`, which holds the shard's **read** lock, then `store` | `get(&k)` then `fetch_add` | writers of the shard's table, not other updaters of the key | its own sharded locks, in every family. Shard count fixed at 64 by `with_shard_amount`: the default is 4 × `available_parallelism` rounded up to a power of two (`dashmap-6.2.1/src/lib.rs`), which follows the affinity mask and would differ between the two pins |
| `rwbtree` — `RwLock<BTreeMap<u64, AtomicU64>>` | **read** guard, `get`, `store` | read guard, `get`, `fetch_add` | D's inserts, which take the write guard | reads and value updates share the read guard; only D's inserts serialise |
| `olc` — `SyncExpanseMap` | **cannot express the idiom**: the wrapper returns values by copy and exposes no value cell (`ExpanseMap::get_slot_ptr` exists on the plain tree only). Uses `insert(k, v)`, which on a present key stores into the slot in place, allocating nothing, inside the covering version word's bracket (`mutate_map.rs`, the present-key branches) | **no atomic per-key update exists**; an external striped lock: S cache-line-padded `Mutex<()>`, stripe = hash(key) mod S, held across `get` + `insert`. Reads do not take it | two RMWs on keys of one stripe | external lock, **disclosed**; point reads and the `insert` inside it stay optimistic |
| `mutex` — `Mutex<ExpanseMap>` | cannot express it without the lock it already holds; `insert(k, v)` under the mutex — the same tree operation `olc` runs, which is what makes it `olc`'s control | the mutex, held across `get` + `insert` | everything | the α = 1 control; every operation of every family takes this lock |

D's write is the arm's own `insert` of an absent key in every arm
(`AtomicU64::new(v)` as the value where the type is atomic); for `rwbtree` that
is the write guard. No arm allocates on an A, B or F write.

**F is not a lock-symmetric comparison across arms, by construction, and no
head-to-head on it is gated.** Three arms do one atomic add; `olc` takes an
external lock and does a `get` and an `insert`. That cell measures a primitive
`SyncExpanseMap` does not have, not the index under it, so F keeps G1 and G2
against the α = 1 control — both hold a lock across the pair, one striped and
one global — G5 and G6, and its competitor comparisons are published with a
direction label and no floor (§20.6). A `compare_exchange`-shaped primitive on
`SyncExpanseMap` is being evaluated separately; it does not exist, nothing here
depends on it, and **if it lands F is re-registered with it as a second `olc`
arm**, in a section of its own, beside the striped-lock arm and not in place of
it.

**The stripe lock does not dilute with S.** The stripe holding the hottest key
takes `hottest_stripe_share(N, θ, S)` = p₁ + (1 − p₁)/S of all RMWs: 0.065654
at S = 1,024 against p₁ = 0.064740, and never below p₁ for any S. Two RMWs meet
on one stripe with probability 0.007944 per pair, of which 0.006975 is the same
key. (C(8, 2)/S ≈ 2.7% is the figure for draws uniform over stripes; it does
not describe a Zipfian stream and is not used.) S therefore
buys almost nothing beyond a few dozen stripes, and the lock's contended
acquisitions are counted (§20.14).

**The invariant, G5.** After the window, outside it: the sum of all values over
the population equals the number of RMW operations the streams contained,
exactly; and every per-key value equals that key's RMW count, which the harness
tallies from the streams before the window. A cell failing either is `VOID_LOST_UPDATE`:
its throughput is not published as a result, and the run is not an
evaluation. **Negative control, a precondition:** an `olc` variant that skips
the striped lock must fail the invariant at T = 8 in the harness's own test,
asserted on the diagnostic string and not the exit code (AGENTS.md §5); the
fail-then-pass is recorded in the harness PR.

### 20.6 The gates

Verbatim, per cell. X_a(f, T, r) is total M ops/s of arm *a*, family *f*, T
threads, round *r* of one interleaved run. Every interval is BCa 95% over the
round series, 2,000 resamples, `scripts/bca_bootstrap.py`, with its `ci_method`
label recorded.

> **G1, scaling against the α = 1 control.** R(f, T, r) =
> [X_olc(f, T, r) ÷ X_olc(f, 1, r)] ÷ [X_mutex(f, T, r) ÷ X_mutex(f, 1, r)].
> Passes iff the lower bound is strictly above 1.0. (§17.3's statistic;
> AGENTS.md §8.20.2.)
>
> **G2, level against the control's best.** L(f, T, r) = X_olc(f, T, r) ÷
> max over T′ ∈ {1, 2, 4, 8} of X_mutex(f, T′, r). Passes iff the lower bound
> is strictly above 1.0. (§19.4's G2.)
>
> **G3, skew retention, f ∈ {A, B, F}.** K(f, T, r) = X_olc(f, T, r) ÷
> X_olc(f0, T, r), the Zipfian cell over its uniform twin at the same T in the
> same round. Passes iff the lower bound is at least **ρ = 0.50** (D2).
> Reported beside it and not gated: the scaling retention
> [X_olc(f, T) ÷ X_olc(f, 1)] ÷ [X_olc(f0, T) ÷ X_olc(f0, 1)].
>
> **G4, an ordered competitor (a skip list), f ∈ {A, B, D} only.**
> Q(f, T, r) = X_olc(f, T, r) ÷ X_skip(f, T, r). Passes iff the lower bound is
> at least **q = 1.0**, that is, strictly above 1.0 (D3). Family F has no G4
> cell (§20.5).
>
> **G6, the peak cell.** K₈₄(f, r) = X_olc(f, 8, r) ÷ X_olc(f, 4, r), one cell
> per (family, pin, run). Passes iff the lower bound is strictly above 1.0.
> Model-free: it asks only whether throughput is still rising at the last
> doubling. Computed and published for every arm; gated for `olc` only.
>
> **G5, no lost update.** §20.5's invariant holds in every round of every arm
> of F. Deterministic; no interval.
>
> **Control cells, T = 1.** P(f, r) = X_olc(f, 1, r) ÷ X_mutex(f, 1, r), one
> per (family, pin, run). Fails iff the interval lies wholly below **F₁ =
> 0.50** (D4), a collapse guard; the binding price gate is G2 at T = 2.
>
> Gate cells are T ∈ {2, 4, 8} × two pins × two runs: twelve per family per
> gate, and four per family for G6. **A family's claim is licensed at a head** when all its cells pass and
> no control cell of that family fails. Families are judged separately; no
> claim is made for "the suite".

**Why G3 gates the level and only reports the scaling ratio.** §19's lesson
generalises: a ratio of scaling factors divides by each cell's own T = 1, and
under Zipfian choice a single thread works on a small hot set, so X(f, 1) may
well exceed X(f0, 1). That alone lowers C(f, T) against C(f0, T) with nothing
lost at T = 8 — and a slower single thread would raise it with nothing gained.
K compares what a user gets at the same T. The scaling ratio is printed beside
it so the two readings cannot be confused.

**Why G1 needs G2.** As §19.4: R can be passed by a slower T = 1 cell; L
cannot.

**The level identity, and what it does to the T = 1 control.** Write P =
X_olc(1) ÷ X_mutex(1) and C(T) = X_olc(T) ÷ X_olc(1). Where the control's best
cell is its T = 1 cell,

> L(T) = P · C(T), exactly and round by round; and L(T) ≤ P · C(T) always,

because a control that peaks elsewhere only has a larger best cell
(`level_from_price_and_scaling`). It holds to rounding in all 96 published
round-cells of the second #929 gate, where the serialised build's best cell is
W = 1 in every round (`level_identity_residual` below 5 × 10^-5 on the four
`1abfb7ff` artifacts; README §18.2's first row is 0.9518 × 6.294 ÷ 3.789 =
1.581 *(workload: `concurrency_writer_str`)*). So **G2 at T = 2 is the binding
single-thread price gate**: L(2) > 1 needs P > 1 ÷ C(2)
(`price_floor_implied_by_level`). A separate floor on P below that value binds
nothing before G2 does, which is why F₁ = 0.50 is a collapse guard and not a
price (§20.7 (f), D4).

**Beside every cell, not gated:** the per-point efficiency E(T) = C(T) ÷ T
with its interval, for every arm, so a reader sees at once how far each load
point is from linear.

**The two pins are a control for the blocking arms, not a second concurrency
axis.** Thread count is the axis. `0-15` and one-thread-per-core differ in
where a thread that is *not running* parks, which moves arms that block
(`mutex`, `rwbtree`, `dash` under a contended shard) and is expected to leave a
non-blocking arm nearly where it was (AGENTS.md §8.20.5 step 0; §17.4). A gate
that passes under one pin and not the other is reported as exactly that.

**No scalability-law fit is gated.** Four load points cannot support a
three-parameter fit: on every committed curve tried — the four uniform `map`
sweeps at `170a4bc3` and the #929 head's four `str` sweeps at `1abfb7ff` — the
unclamped coherency term of Gunther's linearised fit is **negative**
(−0.017 to −0.019 for `map`), so `fit_usl.fit_usl_ols` returns β = 0 because
its β ≥ 0 clamp binds, not because coherency was measured to be absent
(`usl_unclamped_beta`; pinned as a test) *(workloads differ:
`concurrency_writer_map_64bit` vs `concurrency_writer_str`; one fit per curve,
none compared)*. A fit over the six per-core load points of A and B
(T ∈ {1, 2, 3, 4, 6, 8}) is **reported**, with BCa intervals on α and β and a
flag saying whether the β ≥ 0 constraint binds, and decides nothing.

**`dash`, `rwbtree`, and `skip` in family F.** Published per cell as X with
its interval and as the paired ratio X_olc ÷ X_arm with its interval, labelled
`AHEAD` (interval wholly above 1.0), `BEHIND` (wholly below) or
`INCONCLUSIVE`. No floor. `dash` is not gated (D7), and appears in A, B, C, D
and F: all are point operations. The same labels, and no
gate, apply to every cell of C, C0, Ac, Fc and the `-dram` anchors.

**Every input, and the artifact field it is read from.** The harness and
driver do not exist; these are the names they are obliged to use, fixed here so
the artifact cannot be shaped after the fact (paths relative to
`docs/benchmarks/concurrency/`).

| gate input | field |
|---|---|
| the cells | elements of `throughput` with `family` ∈ {`A`,`B`,`D`,`F`,`A0`,`B0`,`F0`,`C`,`C0`,`Ac`,`Fc`,`A-dram`,`C-dram`}, `arm` ∈ {`olc`,`mutex`,`skip`,`dash`,`rwbtree`}, `threads` ∈ {1,2,4,8} (and 3, 6 where §20.4 adds them), `gated` true or false, and `workload_id` one of the ids the harness's shape table `emits` |
| the per-round series | each cell's `rounds_raw[*].total_mops`, matched by `round`; with `elapsed_s`, `ops`, `read_ops`, `write_ops` beside it |
| effective T | `rounds_raw[*].thread_elapsed_s`, one entry per thread, and the cell's `max_over_mean_bias` |
| update idiom | each cell's `update_idiom` ∈ {`value_cell_store`, `map_insert_in_place`}, `value_type` ∈ {`u64`, `atomic_u64`} |
| G6, efficiency | `peak_8_over_4` with its interval per (family, arm); `efficiency` = C(T) ÷ T with its interval per cell |
| the reported fit | `usl_fit` per (family, arm) on the per-core pin: `alpha`, `beta`, their intervals, `load_points`, `estimator`, `beta_constraint_binds` |
| G5 | `rounds_raw[*].rmw_ops`, `rounds_raw[*].value_sum`, `rounds_raw[*].per_key_mismatches` (must be 0) |
| θ, N, stream length, seeds | `provenance.theta`, each cell's `population`, `ops_per_thread`, `seed` |
| the generator check | `provenance.rank_histogram`: observed share on the 1, 2, 16, 256 and 4,096 lowest ranks of one stream, beside `gray_top_k_share` of each |
| RMW provider | each F cell's `rmw_provider`, and `rmw_stripes` for `olc` |
| `dash` shards | each `dash` cell's `shard_amount` (must read 64) |
| intervals | `<stat>_ci_lower`, `<stat>_ci_upper`, `<stat>_ci_method` for every statistic above |
| pin, isolation, commits, load | `provenance.core_pin` and each cell's `cpu_pin`; `provenance.cell_isolation` = `process`; `provenance.commit`; each cell's `load` and `provenance.loads`, with the busy-CPU delta (AGENTS.md §8.17) |
| which registration | `provenance.preregistration` naming this section; an artifact that does not is a baseline and carries no verdict |
| counters, diagnostic only | from a separate `occ-stats` build: `lock_fallbacks`, `fallback_causes_total`, `lock_restarts`, read-validation failures per operation. Never timed, never gated (AGENTS.md §8.9) |

### 20.7 Math-first audit

Computed by `scripts/ycsb_concurrent_bounds.py` (25 pinned tests, run by
`scripts/gate.sh` and CI's `lint` job); the rows below are its `--table`
output at N = 1,048,576, θ = 0.99, not retyped arithmetic.

**(a) Where the stream lands.** H(N, θ) = 15.446323.

| k lowest ranks | exact law, `top_k_share` | what the generator emits, `gray_top_k_share` |
|--:|--:|--:|
| 1 | 0.064740 | 0.064740 |
| 2 | 0.097336 | 0.097336 |
| 16 | 0.221390 | 0.232078 |
| 256 | 0.406592 | 0.416150 |
| 4,096 | 0.598855 | 0.605396 |
| 65,536 | 0.796643 | 0.799963 |
| 10,485 (1% of N) | 0.665291 | 0.670753 |

The two agree exactly at k = 1, 2 and N (Gray et al.'s construction) and
differ by at most 0.010951, at k = 30, the generator always the heavier. One
key in a million takes 6.5% of every thread's operations; 256 keys take four
tenths.

**(b) How often threads aim at one place.** W threads each holding one
independently drawn target — independence is the assumption, and it holds for
per-thread seeded streams. "Any two" is exact (Newton's identities over the
power sums; checked against enumeration and the 23-in-365 birthday figure).

| W | any two on one key, θ = 0.99 | same, uniform | any two in one leaf, scattered ranks (upper bound) | contiguous ranks, 16 per leaf | contiguous, 256 per leaf |
|--:|--:|--:|--:|--:|--:|
| 2 | 0.006975 | 9.5e-07 | 0.006990 | 0.053307 | 0.169980 |
| 4 | 0.039194 | 5.7e-06 | 0.041939 | 0.237922 | 0.558301 |
| 8 | 0.157279 | 2.7e-05 | 0.195716 | 0.619047 | 0.923400 |

Four things follow, and they are statements about targets, not about cost:

1. At the **key**, skew multiplies the instantaneous pair probability by
   N·Σp² ≈ 7,313 over uniform choice.
2. **The key is not the unit threads contend on.** A linear leaf carries no version word; a value
   store into it is bracketed by its *parent's* word (`mutate_map.rs`, the
   present-key branches). At this population the top two key bytes saturate,
   so the covering word of a 16-key leaf belongs to the branch over the second
   byte — one per top-byte value, `COVER_BINS_64` = 256, each covering about
   4,096 keys. That is the model's input, read from the code and not from a
   tree census. With scattered ranks two threads aim under one cover with
   probability 0.010854, against **0.003907 under uniform choice**: uniform
   cells do not sit at parts per million at this granularity, and skew raises
   cover coincidence by a factor of 2.8, not 7,313.
3. With scattered ranks, *leaf* sharing adds almost nothing to key sharing
   (0.006990 against 0.006975 over 65,536 leaves). The neighbourhood regime is
   the contiguous layout — 0.62 at W = 8 and 16 keys per leaf — which is why Ac
   and Fc are in (§20.4).
4. W is the number of threads *inside a write at once*, which is at most T.
   `write_pair_fraction(w)` = w² of pairs have both threads writing — 0.25 in A
   and F, 0.0025 in B and D — and `write_involving_pair_fraction(w)` =
   1 − (1 − w)² have at least one: 0.75 and 0.0975. Both count operations as
   if a write took as long as a read; where writes are slower the true
   fractions are higher.

**(b′) A prediction, where the bounds support one.** If the
only thing skew costs `olc` is threads stalling on a coinciding target — a
writer waiting for a version lock, or a reader retrying because a write closed
the bracket it read under — then the throughput share lost at T threads is at
most `coincidence_loss_bound` = (T − 1)/2 · (pair fraction) · (pair
probability) · h, with h ≤ 1 the fraction of an operation spent holding. At
h = 1:

| family | T | writer–writer, same key: (T−1)/2 · w² · Σp² | writer–writer, same cover | write-involving, same cover | same, uniform twin |
|---|--:|--:|--:|--:|--:|
| A, F | 2 | 0.000872 | 0.001357 | 0.004070 | 0.001465 |
| A, F | 4 | 0.002616 | 0.004070 | 0.012210 | 0.004396 |
| A, F | 8 | 0.006103 | 0.009497 | 0.028491 | 0.010256 |
| B, D | 8 | 0.000061 | 0.000095 | 0.003704 | 0.001333 |

> **P-A, registered.** Under the stall-only hypothesis, family A's *scaling
> retention* at T = 8 — [X_olc(A, 8) ÷ X_olc(A, 1)] ÷ [X_olc(A0, 8) ÷
> X_olc(A0, 1)], the statistic G3 reports beside its level — is at least
> 1 − 0.028491 = **0.9715**. The scaling form is used because a hot set's
> cache benefit to a single thread is in both its numerator and denominator.
> **Refuter:** the BCa 95% interval of that statistic lying wholly below
> 0.9715, in both runs of a pin. Then hot-key lock waits and validation
> retries are **not** what skew costs, by more than an order of magnitude of
> headroom, and the remaining candidate is cache-line ownership transfer on
> the hot cover — every write to a hot covering word moves its line between
> cores whether or not anyone waits, a cost this bound does not contain. That
> candidate is unmeasured until §20.14's HITM counter reads it; the refutation
> of P-A names it as a candidate and credits it with nothing (AGENTS.md §8.9).
> First order: the bound ignores a stalled thread lingering and raising the
> coincidence rate, which is why it is trusted only while small.

P-A is a prediction of a mechanism's *ceiling*, not of the cell. No level is
predicted for K, and P-A failing is the expected outcome (§20.9).

**(c) Workload D.** Applied in counter order, every insert lands in the expanse
holding the greatest key inserted so far, or opens the one after it: the
fraction is 1 by construction (derivation in the module; no function, since
one returning 1.0 would test nothing). With T interleaved slices an insert can
land one leaf behind the append leaf; how often is not derivable and is an
empirical residual.

**(d) Detectability at 8 rounds.** `mde_per_unit_sigma(8)` = 1.40079 through
`reader_scaling_bounds.mde_from_rounds` (Cohen 1988, ch. 2): 8 rounds per arm
resolve a relative effect e only if the per-round coefficient of variation is
at most e ÷ 1.40079 — 0.0357 for 5%, 0.0714 for 10%, 0.1785 for 25%. The only
measured stand-in for a cell that does not exist is the uniform `map` writer
sweep at `170a4bc3`: over its twelve C(W) cells `baseline_scaling_mde` gives a
relative MDE of 1.14%–3.26% *(workload: `concurrency_writer_map_64bit`; the
four `baseline_writer_scaling_170a4bc3_*` artifacts)*. The reducer reproduces
§17.6's published `str` row from the same artifact, which is pinned as a test.

**(e) The peak cell and the fit.** On the four uniform `map` sweeps
X(8) ÷ X(4) is 1.289–1.313 with a relative MDE of 2.84%–4.86% (`peak_series`
through `mde_from_rounds`) *(workload: `concurrency_writer_map_64bit`)*: a cell
that is still rising by 25% is resolvable against 1.0 at 8 rounds, one
rising by under 5% is not, and G6 would then read `INCONCLUSIVE`. The
unclamped USL coherency term on those four curves is −0.0189, −0.0168, −0.0191
and −0.0169 (`usl_unclamped_beta`), which is why §20.6 gates no fit.

**(f) The price floor the level gate already implies.** With the uniform `map`
C(2) of 1.2994–1.3136, `price_floor_implied_by_level` gives 0.7612–0.7696: a
single thread below about 0.77 of the control cannot pass G2 at T = 2 if
C(2) under a YCSB mix resembled the insert-only C(2) *(different workload:
an orientation, not an input)*. At P = 0.50, L(2) would be about 0.65. A mix
that is 95% reads may well have C(2) near 2, where the implied floor falls to
0.5; that is why the floor is left to G2, which reads the family's own C(2),
and D4 is a collapse guard only.

**(g) The stripe lock.** §20.5: the hottest of 1,024 stripes takes 0.065654 of
RMWs, p₁ is 0.064740, and no S goes below p₁.

**Audit verdict: `PROCEED`, conditionally.** G1 and G2 look for a multiple
against a mutex build, far outside a 3% MDE if Zipfian spread resembles uniform
spread. G3, G4 and the control are resolvable only if the cells land at least
one MDE away from the locked ρ = 0.50, q = 1.0 and F₁ = 0.50, and where they
land is unknown; a cell that lands inside that margin reads `INCONCLUSIVE`, and
the values do not move for it. **Empirical residuals, which only measurement supplies:** how long
an operation holds a node's version lock; restart, validation-failure and
fallback rates under coincidence; whether per-round spread under skew
resembles spread under uniform choice; single-thread speed on a hot set;
whether `COVER_BINS_64` = 256 describes the built tree (the oracle pass
records the node census, §20.15); everything about the competitor arms. Not
established: that any gate is met.

### 20.8 Rounds, pins, runs, isolation, load, voids

- **8 rounds per cell**, fixed. Adding rounds to decide an `INCONCLUSIVE` cell
  relabels the evaluation `INTERMEDIATE` (AGENTS.md §8.19).
- **Pins `0-15` and `0,2,4,6,8,10,12,14`, never pooled**, named on every
  figure (AGENTS.md §8.20.5 step 0). The `mutex` and `rwbtree` arms serialise,
  and §17.4 records a serialising arm moving four-fold between these pins at
  W = 8; every paired statistic is formed within one pin.
- **Two independent runs per pin**, each a fresh dispatch, all four at one
  head. A cross-run statement is made only for cells both runs move the same
  way (`docs/BENCHMARKING.md` rule 18).
- **One harness process per timed cell** (§15); `provenance.cell_isolation`
  reads `process`.
- **Interleaved within rounds.** Each round runs every (arm, T) cell of a
  family block — the family and, for `olc`, its uniform twin — in the order of
  that round's row of a Williams design over the block's cells. A block has up
  to 28 cells (five arms × four T, the uniform twin's four, and the four
  per-core T ∈ {3, 6} cells) and there are 8 rounds, so position and first-order carryover are
  **not** fully balanced, unlike §15.1's 8-cell comparison; with one process
  per cell what can carry over is host state, not process state. Declared, not
  corrected.
- **Load snapshots** with the busy-CPU delta before the first cell, between
  family blocks and mid-block (AGENTS.md §8.17).
- **Voids: §17.10 by reference**, with its fields read as this suite's —
  wrong or unrecorded pin, a population other than 1,048,576 or
  `ops_per_thread` other than 1,048,576, a round count other than 8, a method
  other than `bca`, isolation other than `process`, timings from an
  `occ-stats` build, the four runs differing in `crates/` or in the driver, an
  artifact naming another registration — and §6's load rules. Added here: θ
  other than 0.99 (0 in a uniform twin); `shard_amount` other than 64;
  `rank_histogram` outside its binomial tolerance of `gray_top_k_share`; an
  `update_idiom` or `value_type` other than §20.5's for the arm;
  `VOID_LOST_UPDATE`; and `VOID_ORACLE` (§20.15). A void run is discarded whole, replaced at the same head,
  and the discard disclosed.

**When a run is taken.** After the #929 Callgrind-bound decision is recorded
(#1006, *Sequencing*; AGENTS.md §8.20.6 — a verdict describes the engine it
measured), after the lock of this section, and after §20.12's prerequisites
have landed. Every evaluation is appended to `README.md` whatever it reads.

### 20.9 Expected losses

Registered before any run, so an unwelcome cell is a recorded expectation.
**No magnitude is predicted anywhere in this table**: nothing in §20.7 prices
an operation.

| cell or condition | expectation at lock | consequence of a loss |
|---|---|---|
| G3, A and F, T = 8 | **`olc` may fall below its uniform-stream scaling and level.** Two threads aim under one covering word with probability 0.0109 per pair against 0.0039 uniform, and at one key with 0.0070 against 9.5e-07 (§20.7 (b)). Direction expected; size not predicted beyond P-A's ceiling | lower bound under ρ: C2 is not licensed for that family; the cell is published |
| G3, B | a loss is less expected than in A: 5% writes. Not predicted | as above |
| `olc` against `dash`, every family, high T | **`olc` may be `BEHIND` a sharded hash map on point operations.** A hash map pays no key-ordered descent and its 64 shards spread the hot keys. Expected for A and F; not predicted for B and D | published with its label. No gate (D7) |
| G1/G2, D | **read-latest may serialise on the append path.** Every insert from every thread lands in one expanse or the next (§20.7 (c)), and 95% of reads chase the same keys. C1 may fail for D at every T ≥ 2 | C1 is not licensed for D; published |
| D, `rwbtree` and `skip` | appends are the rightmost-append best case for a B-tree and a cheap tail insert for a skip list (AGENTS.md §8.12.4). `olc` may be `BEHIND` both | published; G4 fails for D if it is `skip` |
| `olc` against `skip`, `dash`, `rwbtree`, F | each does one atomic add where `olc` takes a striped lock and does a `get` and an `insert` (§20.5). `BEHIND` is expected in every cell | no gate (§20.6): published with its label and the asymmetry restated beside it |
| G4, A and B | the value-cell idiom gives `skip` an update that allocates nothing and takes no lock. `olc` may be `BEHIND`; not predicted | C3 is not licensed for that family |
| P-A | **expected to be refuted.** The stall-only ceiling is 2.8% at T = 8; a hot covering word written from eight cores is expected to cost more than that through line transfer alone. Direction expected, size not predicted | P-A reads `REFUTED`; §20.14's counters are then what is read, and no mechanism is named without them |
| G6, A and F | whether throughput still rises from 4 to 8 threads under skew is **not predicted**. On the uniform insert-only stream it rises by 1.29–1.31 (§20.7 (e)) | C5 is not licensed for that family; E(T) is published either way |
| G6, `mutex` and `rwbtree` in D | expected below 1.0: blocking arms past their peak | published; these arms carry no G6 gate |
| Ac and Fc against A and F | **expected lower for `olc`** and not for `skip` or `dash`: 0.62 of instants at W = 8 put two threads in one 16-key leaf. Size not predicted | published with labels; no gate |
| C against C0 | not predicted in either direction: a hot set that fits in cache favours C, and no write is present to cost anything | published; it is what A's retention is read against |
| the `-dram` anchors | not predicted | published, never pooled with the 2^20 cells |
| control P, write-heavy families | **`olc` at T = 1 is expected below `mutex` at T = 1**: an uncontended mutex is cheap, and the OLC insert retires 87% more instructions than the plain map's (§19.4, run 35187216693). Size in wall clock not predicted | below F₁: that family's claim is not licensed, whatever T = 8 reads |
| G2, T = 2 | the closest level cell, as in §19.7: two threads must beat the control's best at any T | `INCONCLUSIVE` or `REFUTED`: C1 not licensed for that family |
| G5 | passes in every arm. The negative control fails | a failing arm is a harness or engine defect: the run stops, nothing is published as a result |
| pin against pin | the serialising arms are expected to read lower one-thread-per-core at T = 8 (§17.4); which pin flatters `olc`'s ratios is not predicted | both published; neither dropped |
| the two runs of a pin | expected to agree | a cell they disagree on is reported direction-only |
| fallback and restart counters under skew | expected above their uniform-stream values; not predicted further, and never a gate | reported as diagnostics |

### 20.10 Verdicts

§17.9's vocabulary, per cell: `PASS`, `REFUTED` (interval wholly on the wrong
side of the cell's threshold), `INCONCLUSIVE` (threshold inside the interval),
`INTERMEDIATE` (anything differing from this registration), `NOT_EVALUABLE`
(an input absent; never a pass, never 0). Added: `VOID_LOST_UPDATE` (§20.5),
`VOID_ORACLE` (§20.15), and the direction labels `AHEAD` / `BEHIND` for ungated
comparisons. P-A reads `REFUTED`, `NOT_REFUTED` or `NOT_EVALUABLE`. A family's
claim is licensed only when all its gate cells read `PASS` and no control
fails. No joint error rate is claimed over cells that share a head and a host,
and every evaluation is recorded so a later pass cannot be reported as the
first.

### 20.11 The policy values, as locked

Each is a choice no measurement makes. **Every value in this table is
maintainer policy, set on 2026-09-17, and does not move for any head**
(AGENTS.md §8.19): changing one after a result is seen relabels that
evaluation `INTERMEDIATE` and needs fresh runs. The bounds module carries the
numeric ones as constants (`LOCKED_*` in `scripts/ycsb_concurrent_bounds.py`)
and pins them in a test, so a silent edit turns the suite red. P-A's 0.9715 is
**not** policy: it is derived (§20.7 (b′)), and moves only if the bounds module
does.

**What was known when they were set.** No cell of this suite, of any arm: no
harness exists. Known and read: the uniform insert-only `map` writer cells of
`README.md` §15 at `170a4bc3` (their C(2) of 1.2994–1.3136 and their per-round
spread), and `README.md` §18.2's `str` cells at `1abfb7ff` (for the level
identity) *(workloads differ: `concurrency_writer_map_64bit` vs
`concurrency_writer_str`; neither is a YCSB mix, and neither is an input to
any gate here)*. The last column says which value that knowledge touched.

| id | what it is | locked value | what it is a ratio of, in plain words | set with knowledge of measured data? |
|---|---|---|---|---|
| D1 | F's RMW provider for `olc` | an **external striped lock, S = 1,024** | not a ratio: how `SyncExpanseMap` makes value ← value + 1 atomic, given that it has no per-key update. The hottest stripe carries p₁ + (1 − p₁)/S = 0.0657 of RMWs at 1,024 and never less than p₁ = 0.0647 at any S (§20.7 (g)). If a `compare_exchange`-shaped primitive lands, F gains a second `olc` arm by a new registration; this one is not edited | no — derived from the rank law |
| D2 | ρ, G3's floor | **0.50** | Zipfian throughput ÷ uniform throughput, same arm, same T, same mix: `olc` must keep at least half of its uniform-stream throughput under θ = 0.99 | only its resolvability: the §15 cells' spread says 8 rounds resolve it unless a cell lands within about 3% of it. The level it gates was not known |
| D3 | q, G4's floor, families A, B and D | **1.0**, the lower bound strictly above | `olc` throughput ÷ `SkipMap` throughput, same family, same T: not slower than one ordered concurrent structure, a lock-free skip list using its cheapest update. It says nothing about ART with optimistic lock coupling, ROWEX or Masstree (§20.13) | no |
| D4 | F₁, the T = 1 collapse guard | **0.50, a collapse guard only**; the measured P is published beside every verdict | `olc` single-thread throughput ÷ `Mutex<ExpanseMap>` single-thread throughput. **The binding price gate is G2 at T = 2**, which needs P > 1 ÷ C(2) by identity (§20.6): about 0.77 if C(2) resembled the uniform `map` value, down to 0.5 if a read-heavy mix scales near 2. F₁ never binds before G2 does; it is kept so a collapse is reported as a failed control and not only as a failed level cell | **yes**: §15's `map` C(2) and §18.2's identity cells, which are why it is a guard and not a price |
| D5 | which families carry gates | **A, B and D on G1–G4 and G6; F on G1, G2, G5 and G6** | not a ratio. D is the family most likely to fail C1, which is the reason it is gated | no |
| D6 | competitor set | **`skip`, `dash`, `rwbtree`, and `mutex` as the control. HOT/ROWEX and Masstree out** | not a ratio. `crossbeam-skiplist` 0.1 and `dashmap` 6 are already dev-dependencies of `expanse-trie`; nothing new is added. HOT/ROWEX and Masstree expose `insert` and `get` only, with no update or RMW entry point (`crates/expanse-hot-bench/src/rowex.rs`, `masstree.rs`), `RowexMap::insert`'s doc comment does not say whether it overwrites, and they live in a crate with C++ submodules this harness does not link | no |
| D7 | whether `dash` is gated | **direction labels only, no floor** | `olc` throughput ÷ `DashMap` throughput is published with its interval and a label. An unordered sharded hash map is a different capability; the loss is expected (§20.9), and publishing it is the requirement | no |
| D8 | the contiguous-rank cells | **Ac and Fc in, ungated, labels only** | not a ratio: rank r mapped to the r-th smallest key, the layout under which leaf sharing is 0.62 at W = 8 and not 0.0070 | no — derived |
| D9 | workload E | **out of this section; a registration of its own** | not a ratio. §20.1: scan semantics differ per arm in a way §8.3 has to be argued for, not assumed | no |
| D10 | prefill order | **`sorted` only** | not a ratio. No registered cell inserts into the prefilled range, so build order changes the built shape of `rwbtree` and `skip` only. **Stated limitation:** a strict reading of AGENTS.md §8.12.4 would give `both` for those two order-sensitive arms; this registration measures one order and says so | no |
| D11 | θ and N | **θ = 0.99; N = 2^20 gated; N = 2^24 as the ungated anchor** | not a ratio. θ is YCSB's constant; 2^20 matches every writer cell in this suite; whether the 2^24 tree exceeds the reference host's last-level cache is recorded by `mem_used` in the artifact, not assumed | the §15 cells' population, which is why the uniform twins are comparable with them in kind |
| D12 | the update idiom | **the value-cell idiom wherever the arm can express it** | not a ratio. It gives each competitor its cheapest documented update; each structure's own map-level `insert` would charge `SkipMap` a node allocation and an epoch deferral per update and flatter `olc`. `olc` and `mutex` cannot express it and are the only arms whose update is a map operation | no |
| D13 | the extra load points | **T ∈ {3, 6} on the per-core pin, `olc` and `mutex`, A and B, in and ungated** | not a ratio. Six load points make the reported fit less degenerate than four; nothing is gated on it | **yes**: the negative unclamped coherency term on the §15 and §18.2 curves (§20.6) |

### 20.12 Instrument prerequisites, before any run counts

1. **A new harness file**, `crates/expanse/examples/ycsb_concurrent.rs`, with
   its own `# Workload shape` table (AGENTS.md §8.12, §8.15): one `workload_id`
   and an `emits` row naming a unique id per cell family, as
   `writer_scaling.rs` does; `insertion_order` `sorted`; `hit_rate` 100%.
   Throughput and `occ-stats` counters from two builds, each refusing the
   other's role. It reuses `ycsb_common`'s generator and θ and does not define
   a second one.
2. **A rank-histogram unit test** holding one stream's shares to
   `gray_top_k_share` within a stated binomial tolerance, and the G5 negative
   control (§20.5), both run by CI.
3. **A driver** modelled on `scripts/writer_scaling.py`: one process per cell,
   the Williams rows of §20.8, both pins through `bench_pin.apply(`, load
   snapshots, paired BCa with `ci_method`, the schedule check that refuses an
   artifact disagreeing with what was asked for, and every gate statistic of
   §20.6 computed by committed code before the run it judges.
4. **The driver's self-test asserts the artifact's shape** (AGENTS.md
   §8.20.7): every §20.6 field present; a missing round, a wrong pin, an
   absent T′ cell, an absent uniform twin or a non-zero `per_key_mismatches`
   yields `NOT_EVALUABLE` or `VOID_LOST_UPDATE`, never a pass. It is
   **mutation-tested**: each of those assertions is shown to turn red when the
   production line it guards is removed, and the demonstration is recorded in
   the PR (AGENTS.md §5).
5. **Registration**: `.github/bench-suites.json`, the dispatch `case`, flag
   spelling and upload list in `bench_baremetal.yml`,
   `scripts/check_bench_suites.py --write`, `DIRECT_HARNESSES` in
   `scripts/check_bench_pin.py`, a CI self-test job on a path filter as
   `writer-scaling-selftest` has, and `check_bench_provenance.py` passing with
   no grandfather entry.
6. **`run_concurrent_ycsb`'s doc comment** says it is a smoke test and not a
   measurement (#1006, *Done when*).
7. **The latency build and the PMU pass of §20.14**, each refusing to emit a
   throughput figure, with their artifact fields asserted by the driver's
   self-test as item 4 requires.
8. **The oracle of §20.15**: the value encoding, the post-window final-value
   check in every timed cell, the untimed monotonicity pass, and a Zipfian
   history test added to `crates/expanse/tests/linearizability.rs` beside
   `test_sync_map_linearizability`, with the count it ran stated (never 0).

### 20.13 Explicitly not claimed

- **No mechanism.** No cell's level or movement is attributed to version-lock
  retries, validation failures, cache-line transfer, allocator behaviour or
  frequency. §20.7's probabilities describe where threads aim, not what it
  costs; a mechanism named for any loss needs its counter or an ablation
  (AGENTS.md §8.9, §8.20.3). Cause unknown until then.
- **No magnitude, for any cell.** Directions are given where §20.7 supports
  one; "not predicted" is the entry everywhere else.
- **Nothing about scans, removals, the miss path, the set, string, bytes or
  blob wrappers, 32-bit targets, other hosts, other θ or other populations.**
- **No claim against ART with optimistic lock coupling, ROWEX or Masstree.**
  G4's competitor is one skip list. The HOT/ROWEX and Masstree arms this
  repository can build expose `insert` and `get` only through
  `crates/expanse-hot-bench` (D6), so none of A, B or F can be run on them, and
  no ratio is formed against any figure in another suite *(different
  workload)*. "An ordered concurrent competitor" never reads as "ordered
  concurrent indexes".
- **No head-to-head claim on read-modify-write.** F's competitor cells measure
  a missing primitive (§20.5).
- **No scalability-law parameter is claimed.** The reported fit is a
  description with its intervals and its constraint flag (§20.6).
- **No latency claim.** §20.14's percentiles are closed-loop service times
  from a separate build and are diagnostic.
- **No comparison with README §15's numbers.** They were taken at `170a4bc3`
  on an insert-only stream; G3's denominator is measured in the same round at
  the evaluated head.
- **F is not a lock-symmetric comparison across arms** (§20.5), and no sentence
  built on it may read as one.
- **D is not YCSB's workload D** in two declared respects (§20.4), and the
  generator is Gray's approximation and not an exact Zipfian sampler (§20.7).
- **No claim that the registered competitor configurations are tuned.**
  `DashMap` at 64 shards and `SkipMap` at its defaults are stated, not
  optimised; a different shard count is a different cell.

### 20.14 Two diagnostic passes, declared now and never gate inputs

Registered so their shape cannot be chosen after a throughput cell
disappoints. Neither produces a number that enters §20.6, neither is timed for
throughput, and a mechanism named from either is a finding of that pass with
its counter beside it, not a verdict of this gate (AGENTS.md §8.9, §8.20.3).

**(a) Latency, from a separate build.** A third build role, `latency`, which
like `occ-stats` refuses to emit `total_mops`.

- Per thread, per operation type, a **non-allocating log-bucketed histogram**:
  fixed arrays sized before the barrier, power-of-two buckets with 16 linear
  sub-buckets each, no allocation and no shared state inside the window.
- Timestamps are `rdtsc` reads, converted with the **measured** `tsc_hz`
  (`occ_stats::cycles_hz`), never with a core clock frequency (AGENTS.md
  §8.20.1). x86-64 only; the pass is `NOT_INSTRUMENTED` elsewhere.
- **These are service times of a closed loop.** Each thread issues its next
  operation when the last returns, so there is no arrival process, no queueing
  delay and no correction for coordinated omission: p99.9 here is "how long the
  slowest operations took", not "what a client at a fixed rate would see".
  Stated beside every table.
- Cells: every gated (family, arm) at T ∈ {1, 8}, both pins, 8 rounds,
  histograms merged across threads and rounds before percentiles are read.
- Artifact, fixed now: elements of `latency` with `family`, `arm`, `threads`,
  `op` ∈ {`read`, `update`, `insert`, `rmw`}, `samples`, `p50_ns`, `p99_ns`,
  `p999_ns`, `max_ns`, `tsc_hz`, `bucket_scheme` = `log2x16`, `clock` =
  `rdtsc_over_tsc_hz`, `model` = `closed_loop_service_time`, and
  `bracket_overhead_ns`, the calibrated cost of one timestamp pair.

**(b) PMU and `perf c2c`, one pass per family at T = 8.**

- `olc` arm, one-thread-per-core pin, every event **prefixed with the pinned
  core class** (`cpu_core/…/`; a bare event name on the hybrid host opens both
  PMUs): `cpu_core/mem_load_l3_hit_retired.xsnp_fwd/` — the forwarded-snoop
  count the driver already opens — `cpu_core/cycles/` and
  `cpu_core/ref-cycles/`, per operation, per thread, 8 rounds so a frequency
  ratio carries an interval (AGENTS.md §8.20.2).
- One `perf c2c` recording per family on the same cell, ranked by
  `scripts/c2c_ranking.py`, symbols resolved from `perf report` and not from
  the c2c column (AGENTS.md §8.20.5 steps 4–5). HITM load samples per line are
  what P-A's refutation points at; they locate, they do not attribute.
- **Every arm, every gated cell:** voluntary context switches per operation,
  from `getrusage(RUSAGE_THREAD)` read by each thread outside the window —
  the direct reading of whether an arm's threads block, which is the thing the
  two pins are a control for.
- **The D1 stripe lock:** a contended-acquisition counter — `try_lock` first,
  count the failures, then `lock` — in the `occ-stats` build only, reported as
  contended acquisitions per RMW beside `hottest_stripe_share`'s 0.0657.
- Artifact, fixed now: elements of `pmu` with `family`, `arm`, `threads`,
  `event`, `per_op`, its interval and `ci_method`, `pmu_prefix`; `c2c` with
  `family`, `recording`, `ranking`; per-cell `nvcsw_per_op`; per F cell
  `stripe_contended_per_rmw`.

### 20.15 The correctness oracle, beyond family F

G5 covers F. A lost or torn update in A, B or D would inflate a cell just as
silently, so:

- **Values say who wrote them.** §20.4's encoding: thread in the top 8 bits,
  that thread's write sequence in the low 56.
- **In every timed cell of A, B and D, after the window:** each population
  key's final value is one of at most T candidates — for each thread, the
  **last** write that thread's stream made to that key, or the prefill value
  if no stream wrote it — tallied from the streams before the window. Every D
  insert is present with exactly its own value, since one thread owns each
  inserted key. A cell failing either is `VOID_ORACLE`, handled as
  `VOID_LOST_UPDATE` is.
- **An untimed oracle pass**, T = 8, same streams, each family and arm once
  per run: every reader keeps, per key among the 4,096 lowest ranks and per
  writer thread, the last sequence number it saw, in arrays sized before the
  start. A sequence number going **backwards** for one (reader, key, writer)
  fails the pass: a single writer's values to one key must be seen in order by
  any one reader. It times nothing, and it records the built tree's node
  census so §20.7's `COVER_BINS_64` can be contradicted.
- **The existing `linearizability` target** gains a history test on a small
  Zipfian sample (θ = 0.99 over a few hundred keys, so histories collide),
  through the same per-key checker. It runs in CI, outside any timed cell, and
  its result is a precondition of an evaluation, not a statistic.

What this does not establish: linearizability of the timed cells themselves —
recording histories inside the window would change what is timed — or anything
about the competitor arms beyond the final-value check they share.

### 20.16 Readings and interpretations implemented in harness and driver (appended 2026-09-18)

*Disclosure: This subsection was written on 2026-09-18, AFTER the four evaluation runs of PR #1033 were executed, to record twelve interpretations and implementation choices of §20 that the original text left open. No threshold, cell list, gated cell definition, or verdict rule was changed by these readings; all gated cells remain as locked in §20.6 and §20.11.*

1. **Arms and thread counts for contiguous-rank families Ac and Fc** (`docs/benchmarks/concurrency/scripts/ycsb_concurrent.py:219-223`): §20.4 specified "Ac, Fc | as A, F | Zipfian, contiguous ranks ... | no — labels only" without enumerating arm and thread sets. Implemented as running `ALL_ARMS` (`olc`, `mutex`, `skip`, `dash`, `rwbtree`) across `PRIMARY_THREADS` ($`T \in \{1, 2, 4, 8\}`$) matching families A and F.
2. **Arms and thread counts for read-only anchor families C and C0** (`docs/benchmarks/concurrency/scripts/ycsb_concurrent.py:215-217`): §20.4 registered "C, C0 | 100% read | Zipfian; uniform (`olc` only) | none | no — anchor". Implemented as `ALL_ARMS` across `PRIMARY_THREADS` for C, and `("olc",)` across `PRIMARY_THREADS` for C0.
3. **`usl_fit` evaluation scope** (`docs/benchmarks/concurrency/scripts/ycsb_concurrent.py:815-828`): §20.6 specified "`usl_fit` per (family, arm) on the per-core pin". Implemented as computing Universal Scalability Law fits for every (family, arm) pair present in cells on the `per_core_layout` pin ($`T \in \{1, 2, 4, 8\}`$, plus $`T \in \{3, 6\}`$ where evaluated), producing an empty list on the 0-15 pin, and labeling all fit entries `gated: False`.
4. **`max_over_mean_bias` per-cell aggregation** (`docs/benchmarks/concurrency/scripts/ycsb_concurrent.py:856-863`): §20.4 required publishing `reader_scaling_bounds.max_over_mean_bias` over thread elapsed times beside every cell without setting a threshold. Implemented as emitting a structured dictionary `{"by_round": biases, "mean": sum(biases)/len(biases), "max": max(biases)}` recording per-round and aggregate values.
5. **Prefill value encoding** (`crates/expanse/benches/ycsb_concurrent_common/mod.rs:432, 656, 768, 830, 889, 947`): Keys prefilled prior to the timed window are populated with value `0` across all competitor arms and families.
6. **"Index of the write" encoding** (`crates/expanse/benches/ycsb_concurrent_common/mod.rs:434-436`): §20.4 specified encoding the thread ID and "index of the write" into payload words. Implemented as 1-indexed operation index: `((thread_id as u64 + 1) << 56) | ((op_idx as u64 + 1) & 0x00FF_FFFF_FFFF_FFFF)`, ensuring stream writes are strictly non-zero and distinguishable from prefill `0`.
7. **Rank histogram binomial tolerance threshold** (`docs/benchmarks/concurrency/scripts/ycsb_concurrent.py:108-109`): §20.8 voids a rank histogram "outside its binomial tolerance" and §20.12 item 2 asked for a stated threshold. Implemented as $4.5\sigma$ (`RANK_TOLERANCE_SIGMAS = 4.5`) around expected `gray_top_k_share` values.
8. **Anchor C vs C0 comparison labeling** (`docs/benchmarks/concurrency/scripts/ycsb_concurrent.py:786-794`): §20.4 anchor comparison between Zipfian read-only C and uniform read-only C0 labeled as `numerator: "olc under C"`, `denominator: "olc under C0"`, under `family: "C"`, with `gated: False`.
9. **G5 lost-update gate evaluation scope** (`docs/benchmarks/concurrency/scripts/ycsb_concurrent.py:697-705`): In accordance with §20.3 C4 ("family F only"), gate G5 (`G5_no_lost_update`) is evaluated and reported specifically under family F, checking that 0 lost updates occurred across all F rounds, while whole-tree correctness assertions (§20.15) run as preconditions across all families and arms.
10. **SkipMap was-present insert semantics** (`crates/expanse/benches/ycsb_concurrent_common/mod.rs:856-861`): Crossbeam's `SkipMap::insert` returns an `Entry` rather than a previous-value boolean; the post-window oracle count check verifies key presence and final entry count rather than an in-loop return flag.
11. **RMW provider string representations** (`crates/expanse/benches/ycsb_concurrent_common/mod.rs:113-119`, `docs/benchmarks/concurrency/scripts/ycsb_concurrent.py:92-103`): Artifact rows emit `rmw_provider: "striped_lock"` for `olc`, `"global_mutex"` for `mutex`, and `"atomic_fetch_add"` for `skip`, `dash`, `rwbtree`.
12. **Uniform PRNG draw sequence via unconditional read/write roll** (`crates/expanse/benches/ycsb_concurrent_common/mod.rs:481`): In `generate_thread_stream`, `let roll = rng.next_u64() % 100` is drawn once per operation across all families, including 100% read family C, guaranteeing that identical stream generator seeds produce identical rank/key PRNG draw sequences across families.

## 21. Pre-registration for #929 — the `SyncExpanseBlobMap` multi-writer path as a priced trade (appended and locked 2026-09-18, before any admissible run of it)

### 21.1 Context and relation to previous gates

Issue #929 evaluates multi-writer optimistic concurrency across the compound wrappers
(`SyncExpanseStrMap`, `SyncExpanseBlobMap`, `SyncExpanseBytesMap`).

§17 evaluated `SyncExpanseStrMap` under a zero-overhead expectation ($W = 1$ unchanged,
$W \ge 2$ scaling), which was falsified: optimistic version-lock coupling on digital
trie descent carries an inescapable instruction-count penalty (+81% on `insert` over the
plain map) and a wall-clock throughput reduction (~4% to 5%) relative to a mutex-protected
wrapper executing flat, bracket-free mutations.

§19 established the priced-trade model for compound wrappers: accepting a stated, bounded
single-writer price floor $F = 0.90$ in exchange for multi-writer scaling at $W \ge 2$
that exceeds the serialized build at *any* writer count.

This section extends the priced-trade model to `SyncExpanseBlobMap` under the
`concurrency_writer_blob_64bit` workload. Unlike string maps, blob maps combine digital
trie index navigation with chunk-arena payload allocation and epoch-based garbage
collection. This pre-registration is committed and locked before any multi-writer throughput
run of `SyncExpanseBlobMap` is executed.

### 21.2 What had been seen when this was written

The reader is owed the full accounting of prior observations (AGENTS.md §8.7, §8.19):

1. **CI Callgrind baselines**: The `instruction-counts` job on `main` at `f25189fd`
   measuring the plain `blobmap_insert` arm (incorporating the double-descent removal from #1021)
   and the serial mutex wrapper `sync_blobmap_insert` arms (Refs #1018).
2. **Prerequisite audits**:
   - **Counter visibility audit (AGENTS.md §8.22.1)**: `ExpanseBlobMap::len` is an
     exported public API (`crates/expanse/src/blobmap.rs:1107@a80ce8a5`), as are `BlobArena::live_bytes`
     (`crates/expanse/src/blobmap.rs:1013@a80ce8a5`), `BlobArena::chunks` and `chunks_count`
     (`crates/expanse/src/blobmap.rs:1027@a80ce8a5, 1020@a80ce8a5`), and `BlobArena::mem_used`
     (`crates/expanse/src/blobmap.rs:1006@a80ce8a5`). Public API signatures cannot be modified or
     reduced to test-only under cargo semver without breaking `scripts/check_public_api.py`.
     Under multi-writer execution, `len` reads from the sharded pop counter (`tree_pop.sum()`).
     `BlobArena`'s internal counters (`live_bytes`, `total_allocated`) are maintained as plain
     `usize` under the writer mutex during `prepare_slot` and are never sharded.
   - **Arena-section hold-time measurement (AGENTS.md §8.22)**: Direct instrumentation of the
     `arena_write` critical section (`prepare_slot`) on the reference host with 32-byte payloads
     measures hold time at **17.41 ns/alloc**, representing **12.62%** of total insert latency
     (137.95 ns/op in release mode). Under Amdahl's law, the serialization ceiling is
     $1 / (17.41 \times 10^{-9}) \approx 57.4 \text{ M ops/s}$, which strictly dominates the
     $W = 8$ scaling floor ($F \ge 20 \text{ M ops/s}$).
   - **Compaction hazard & invariant analysis**: Evaluated `crates/expanse/src/blobmap.rs:902@a80ce8a5`.
     Chunk header generation checks invalidate retired chunks, but cannot track concurrent
     arena reallocations across uncoordinated threads. The writer gate must strictly enclose
     arena allocation before index insertion; pointer-valued epoch pins must span read through
     publish. Active compaction remains stop-the-world behind the writer lock with the tree-level
     version word bracketed unconditionally (`write_quiesced`).
   - **Single-block bucket layout analysis**: Evaluated `crates/expanse/src/bytesmap.rs:78-98@a80ce8a5`
     for layout consolidation and indirection overhead bounds.
3. **No multi-writer throughput run**: No execution of `concurrency_writer_blob_64bit`
   under multi-writer OLC or `--compare` has been performed.

Consequences: All thresholds below are either carried over from §19 unchanged, derived from
first principles, or set as maintainer policy prior to execution; evaluation is conducted
solely on fresh runs executed after the lock.

### 21.3 The claim this gate would license, in full

> On the reference host, at the registered pins, `SyncExpanseBlobMap`'s
> per-node OLC write path delivers more insert throughput at every writer count
> W ≥ 2 than the serialised build delivers at **any** writer count, and its
> single-writer throughput is at least the stated fraction F = 0.90 of the serialised
> build's. *(workload: `concurrency_writer_blob_64bit`)*

Nothing is claimed regarding concurrent compaction, reads, removals, key churn,
32-bit targets, alternate hosts, or the string and bytes map wrappers.

### 21.4 The gate

Evaluated per cell. `T_b(W, r)` is `writer_mops` of build *b* at W writers in round *r*
of an interleaved comparison run; `head` is the default build and `serial` is the same
commit compiled with `ablation-blob-serial-writers`.

> **G1, scaling.** R(W, r) = [T_head(W, r) ÷ T_head(1, r)] ÷ [T_serial(W, r) ÷ T_serial(1, r)].
> A cell passes iff the BCa 95% lower bound over the round series is strictly above 1.0.
>
> **G2, level.** L(W, r) = T_head(W, r) ÷ max over W′ ∈ {1, 2, 4, 8} of T_serial(W′, r).
> A cell passes iff the BCa 95% lower bound is strictly above 1.0.
>
> **G3, price.** P(r) = T_head(1, r) ÷ T_serial(1, r).
> A cell passes iff the BCa 95% lower bound is at least **F = 0.90** — maintainer policy,
> matching §19.4.
>
> **G4, overwrite under skew (Decision 6).** K_skew(W, r) = T_head_skew(W, r) ÷ T_head_uniform(W, r)
> under Zipfian key skew (θ = 0.99). Overwriting existing 32-byte blob entries under high
> contention exercises slot compare_exchange and dead-slot retirement without arena growth.
> A cell passes iff the BCa 95% lower bound is at least **ρ = 0.50** (matching §20.3 D2).
>
> **Peak ratio.** X(8)/X(4) = T_head(8) ÷ T_head(4) is computed and reported for both pins
> to diagnose scaling saturation.
>
> G1, G2, and G4 cells are W ∈ {2, 4, 8} × two pins (`0-15`, `0,2,4,6,8,10,12,14`) × two
> independent runs: twelve each (36 cells). G3 has one cell per (pin, run): four cells.
> Peak ratio X(8)/X(4) is reported for both pins (four cells).
> **The gate is met at a head** when all forty cells pass.
> BCa 95%, 2,000 resamples, computed via `scripts/bca_bootstrap.py`.
>
> **Callgrind, preconditions.** On the head's own `instruction-counts` job:
> every plain-tree arm, and every `sync_map_*`, `sync_set_*`, and `sync_strmap_*` arm,
> within AGENTS.md §6's +0.1% of main. The `sync_blobmap_*` arms are expected over the
> automated threshold; this registration pre-authorises one `allow-regression:` line
> naming exactly those arms and citing that job's run.
>
> **Defect tripwire, deterministic:** The `occ-stats` replay of the `sync_blobmap_*`
> mutation arms records zero `lock_restarts`, and zero unbracketed mutations.

### 21.5 Math-first audit

`reader_scaling_bounds.mde_from_rounds` (`scripts/reader_scaling_bounds.py:227`, Cohen 1988, ch. 2),
applied to round series of 8 rounds:

Given typical per-round coefficient of variation $\sigma / \mu \le 0.015$ on quiet runs on
the reference host, the relative MDE for $N = 8$ rounds at $\alpha = 0.05, \beta = 0.20$ is:

```math
\text{MDE}_{\text{rel}} = (z_{\alpha/2} + z_{\beta}) \cdot \frac{\sigma}{\mu} \cdot \sqrt{\frac{2}{N}} \approx (1.960 + 0.842) \cdot 0.015 \cdot \sqrt{\frac{2}{8}} \approx 2.802 \cdot 0.015 \cdot 0.5 \approx 0.021 \ (2.1\%)
```

A floor of $F = 0.90$ is resolvable against an expected level $P \approx 0.95$ (gap $0.05 > 0.021$).
Level ratios $L(W) \ge 1.4$ for $W \ge 2$ sit far outside the detectable margin.

### 21.6 Rounds, pins, runs, isolation, voids

- **Rounds**: 8 rounds per cell, interleaved Latin square ordering (`williams_order`).
- **Pins**: Both `0-15` (hyperthread pairs) and `0,2,4,6,8,10,12,14` (one thread per physical core).
- **Runs**: Two independent runs per pin.
- **Process isolation**: One process per cell invocation.
- **Workload parameters**:
  - Fresh inserts: $N_0 = 0$ prefill, $M = 2^{20} = 1,048,576$ fresh keys, payload size 32 bytes (`BLOB_PAYLOAD_LEN`).
  - Overwrite under skew: $N_0 = 2^{20}$ prefill, $M = 2^{20}$ overwrites under Zipfian key skew ($\theta = 0.99$).
- **Instrument prerequisite (AGENTS.md §8.20.7)**: `scripts/writer_scaling.py --gate-929-blob`
  computes G1, G2, G3, G4, and $X(8)/X(4)$ with fail-closed self-test assertions before any
  production run is evaluated.

### 21.7 Expected losses

| cell or condition | expectation at lock | consequence of a loss |
|---|---|---|
| G3, single-writer price | about 0.94–0.96 | lower bound under 0.90: gate is not met; F does not move |
| G2, W = 2 | closest level cell, expected > 1.40 | `INCONCLUSIVE` or `REFUTED`: gate not met |
| G1, W = 8, per-core pin | large (> 10), reflecting serial mutex contention collapse | none expected |
| G1, W = 8, `0-15` pin | wider interval due to SMT thread contention | `INCONCLUSIVE` possible if variance spikes; pin is retained |
| G4, overwrite under skew | expected > 0.60 | `INCONCLUSIVE` or `REFUTED`: gate not met |
| Peak ratio X(8)/X(4) | expected > 1.0 on per-core pin; may saturate on `0-15` | diagnostic indicator; does not fail gate |
| `sync_blobmap_*` Callgrind arms | over the automated threshold (+40% to +80%) | reported under pre-authorised `allow-regression:` |
| Untargeted Callgrind arms | within +0.1% | precondition failure: no throughput run is taken |
| Deterministic tripwire | zero `lock_restarts` | non-zero trips defect investigation |

### 21.8 Verdicts

Standard vocabulary: `PASS`, `REFUTED`, `INCONCLUSIVE`, `INTERMEDIATE`, `NOT_EVALUABLE`.
The gate is met only when all forty cells evaluate to `PASS`.

### 21.9 If it is met: what the promotion must say

The default build's single writer is slower than the serialised build by the measured P,
stated with its interval in `docs/ARCHITECTURE.md` §4 and rustdoc, beside the
`ablation-blob-serial-writers` feature that restores the serialised protocol (AGENTS.md §2.7).
Explicitly stated limitations: fresh inserts only, payload size 32 bytes; no compaction during
active concurrent writes; no scans, churn, or removals; 64-bit targets only.

### 21.10 Reader-side stale chunk table hazard resolution (2026-09-18, Refs #929)

During concurrent churn (inserts, relocations, and compactions), two subtle reader invariants must hold:
1. **Compaction and clear whole-tree version bracketing**: `compact`, `clear`, and fallback `insert`
   execute through `Shared::write_quiesced`, holding the tree-level version word odd across the entire
   rebuilding/re-indexing operation. If these operations were routed to `write_root_covered_exact`,
   the tree version word would be left unbracketed whenever `root_is_tree()` is true. An overlapping
   reader would then validate against the unincremented tree version word, observing rewritten slot
   locators referencing a newly constructed generation while dereferencing chunk offsets in an old,
   superseded chunk table, resulting in torn or mismatched payload reads.
2. **Reader table reload on dangling locator resolution**: When an active writer appends a chunk
   to the arena and publishes an updated table pointer, a concurrent reader operating under an earlier
   snapshot may encounter a freshly inserted slot locator whose chunk index exceeds the length of the
   reader's sampled table. If the locator fails resolution (`None`), the reader must reload
   `reader_table()`. If the table pointer changed during the lookup, the read retries from the root
   under the new table rather than returning a false negative (`None`) for a present key.

### 21.11 Correction: G4 was registered before the instrument could run it (2026-09-18, after the lock and before any admissible run)

No threshold, statistic, pin, round count, cell count or verdict rule changes.
§21.1–§21.10 are left as locked; where they disagree with this subsection, this
subsection is what was meant, and the disagreement is stated rather than edited
away (AGENTS.md §8.7).

**1. The order of events.** §21.6 names as a prerequisite that
`writer_scaling.py --gate-929-blob` "computes G1, G2, G3, G4, and X(8)/X(4)".
When §21 was locked it computed G1, G2, G3 and the peak ratio, and
`writer_scaling.rs` had no overwrite cell at all, so G4 could not be run and a
report without it could read as all cells passing. No admissible run of §21
preceded this note, and none is taken on an instrument older than the change
that carries it. From that change on, a run without G4 rows is void and its
three G4 cells read `NOT_EVALUABLE`; the driver's self-test carries one negative
control per clause (rows absent, a lower bound of 0.49, a row reporting another
skew or prefill) beside the passing case.

**2. What `T_head_uniform` is.** §21.4 defines K_skew(W, r) =
T_head_skew(W, r) ÷ T_head_uniform(W, r) and describes only the numerator. Both
terms are the **same overwrite workload**: a prefill of N0 = 2^20 keys with
32-byte payloads, then M = 2^20 overwrites of existing keys with new 32-byte
payloads, split across W writers (⌊M/W⌋ each, the last writer taking the
remainder). They differ in one thing, how an overwrite picks its key: Zipfian
for the numerator, uniform over the prefill for the denominator. Both run on
the head build only, in the same rounds, one process per cell; the serialised
build has no G4 cell. The denominator is not the fresh-insert cell of G1–G3.

**3. The generator and the rank → key mapping, as implemented.**
- The prefill is the fresh-insert cell's: `WriterWorkload::generate(2^20, _, 64)`,
  ascending, payload `blob_payload(k)`, metadata `blob_meta(k)`.
- `rank_keys` is a Fisher–Yates permutation of that ascending prefill: for `i`
  from N0 − 1 down to 1, swap `rank_keys[i]` with `rank_keys[x % (i + 1)]`,
  `x` the next draw of the harness XorShift seeded `SEED_BLOB_RANK`. **Rank `r`
  is the key `rank_keys[r]`**, under both distributions. The hot ranks are
  therefore spread over the keyspace; with the sorted prefill as the table they
  would share the lowest leaves, and the cell would measure that adjacency.
- Writer `w` in round `r` draws from its own `ycsb_common::XorShift64`, seeded
  `SEED_BLOB_OVERWRITE ^ (w + 1)·0x9E37_79B9_7F4A_7C15 ^ (r + 1)·0xD1B5_4A32_D192_ED03`.
  Zipfian: rank = `ycsb_common::ZipfianGenerator::new(2^20, ZIPFIAN_THETA).next(rng.next_f64())`,
  with `ZIPFIAN_THETA` = 0.99 and rank 0 the most probable
  (`crates/expanse/benches/ycsb_common/mod.rs`, the generator §20 uses; no
  second implementation exists). Uniform: rank = `rng.next_u64() % 2^20`.
- Every rank is drawn before the barrier. The timed loop is the same code under
  both distributions: `rank_keys[rank]`, the payload, `insert`. The generator's
  zeta sum, the draws, the prefill and the read-back check are outside the
  timed region (AGENTS.md §8.6).
- An overwrite's payload is eight bytes of a per-op counter
  (`(w + 1) << 40 | (i + 1)`) and three words derived from the key and that
  counter; its metadata word is derived from the same pair. After the window
  the cell requires the population to be 2^20, and reads back the 64 hottest
  ranks and every 1,000th rank after them: an untargeted key must hold its
  prefill payload, a targeted key must hold exactly one overwrite whose counter
  names a stream entry for that key. A failure exits non-zero and no row is
  printed.
- Rows carry `workload_id` `concurrency_writer_blob_overwrite_64bit`, `blob_op`,
  `key_dist`, `theta`, `prefill`, `overwrites` and `payload_bytes`. The driver
  checks each against θ = 0.99 (0 on the uniform rows), 2^20, 2^20 and 32, and
  voids the run on any disagreement.

**4. Schedule and cell count.** The forty cells are unchanged: per (pin, run),
G1, G2 and G4 at W ∈ {2, 4, 8} and one G3 cell — ten — over two pins and two
runs. The peak ratio's four values are reported beside them and are not among
the forty; §21.4's "(four cells)" for it names reported values, as §21.7's row
for it already says. The eight (build, W) fresh-insert cells keep their 8 × 8
Williams square. The six (key_dist, W) overwrite cells of a round form a second
block in the same construction; which block runs first alternates by round.
Six treatments over eight rounds repeat rows 0 and 1 of their square, so
position balance inside the G4 block is incomplete, which a paired within-round
ratio tolerates and which is stated here rather than discovered later.

**5. §21.2's hold-time figures are not a reference-host measurement.** §21.2
gives the `arena_write` hold time as measured "on the reference host". No
bare-metal run, CI run or committed artifact backs those figures. The only
instrument in the tree that prints them is the unit test
`sync::tests::test_arena_section_hold_time` (`crates/expanse/src/sync.rs`), an
un-pinned wall-clock loop on whatever machine runs the test suite. They are a
development diagnostic, they are not an input to any cell of this gate, and the
serialisation ceiling §21.2 derives from them is unmeasured (AGENTS.md §8.7,
§8.9).

**6. "Without arena growth" is not what the cell does.** §21.4 describes G4 as
exercising "dead-slot retirement without arena growth". The index population
does not grow. The arena does: every overwrite of a 32-byte payload
bump-allocates a new record (`BlobArena::alloc_blob`) and the record it replaces
is only subtracted from `live_bytes` (`record_deleted`); nothing compacts during
the cell. Read G4 as overwrites at a fixed key population.

**7. The fresh-insert cell's prefill.** §21.6 gives the fresh-insert cell as
"N0 = 0 prefill". That is a drafting error. The cell G1–G3 are read on is the
instrument's `concurrency_writer_blob_64bit` cell as it has been since it was
added for the `README.md` §15 baselines: `run_blob_cell` over
`WriterWorkload::generate(N_PREFILL, M_FRESH, 64)` with
`N_PREFILL = 1 << 20` and `M_FRESH = 1 << 20` — a prefill of 2^20 keys outside
the timed window, then 2^20 fresh keys inserted by W writers over disjoint
slices, 32-byte payloads (`BLOB_PAYLOAD_LEN`). It is the shape every
writer-sweep arm of `writer_scaling.rs` has and the shape §19 gated the string
arm on (§17.2.3, §19.10 item 1). M and the payload size are as §21.6 states
them; only N0 was misdrafted. §21.9's limitation reads accordingly: fresh
inserts into a tree that already holds 2^20 keys.

**8. The tripwire is the single-threaded reading §21.4 registers.** §21.4's
tripwire is zero `lock_restarts` in the `occ-stats` replay of the
`sync_blobmap_*` mutation arms, a single-threaded replay: with one writer a
restart can only be that writer meeting its own lock, which is the defect the
tripwire names. As first written, `gate_929_blob_report` summed `lock_restarts`
over every W of the multi-writer counters pass and voided the run on any, which
the registered text does not say and which §19's report never did. At W ≥ 2 a
restart is two writers meeting on one node, which optimistic lock coupling does
by design. The driver now implements the registered reading: the tripwire reads
the head build's single-writer (W = 1) counters rows only, a run whose W = 1
counters rows are absent is void because the tripwire was not read, and
`lock_restarts` at W ≥ 2 are reported per cell under `fallback_prediction` —
and, for the overwrite cells, under `g4_overwrite_skew` — and void nothing. The
driver's self-test pins the pair: restarts at W = 1 void the run; restarts at
W = 8 with none at W = 1 do not. The replay of the Callgrind arms stays the
precondition §21.4 states, read on the head's own CI run.

## 22. Pre-registration for #929 — the `SyncExpanseBytesMap` multi-writer path as a priced trade (appended and locked 2026-09-19, before any admissible run of it)

### 22.1 Context and relation to previous gates

Issue #929 evaluates multi-writer optimistic concurrency across the three compound wrappers
(`SyncExpanseStrMap`, `SyncExpanseBlobMap`, and `SyncExpanseBytesMap`).

§17 and §19 evaluated `SyncExpanseStrMap`, establishing the priced-trade model for compound
wrappers: accepting a stated, bounded single-writer price floor $F = 0.90$ in exchange for
multi-writer scaling at $W \ge 2$ that strictly exceeds the serialised build at *any* writer count.
§21 extended this model to `SyncExpanseBlobMap`, incorporating G4 (overwrite under Zipfian skew)
to evaluate slot replacement and GC under contention.

This section extends the priced-trade model to `SyncExpanseBytesMap` (Task C of #929).
`SyncExpanseBytesMap` maps arbitrary byte slices (`&[u8]`) to machine words (`u64`). It pairs
a 64-bit optimistic digital trie (`SyncExpanseMap`) indexed by SipHash-2-4 64-bit key hashes with
singly-linked heap bucket chains resolving 64-bit hash collisions.

Under the baseline architecture, all mutation operations (`insert`, `remove`) acquire the wrapper's
exclusive writer mutex, serialising mutations across all threads. This pre-registration defines the
gate for the lock-free/optimistic bucket-replacement multi-writer architecture. This pre-registration
is committed and locked before any admissible multi-writer throughput run of `SyncExpanseBytesMap` is
executed.

### 22.2 What had been seen when this was written

The reader is owed the full accounting of prior observations (AGENTS.md §8.7, §8.19):

1. **Serial baseline**: The existing `SyncExpanseBytesMap` implementation serialises every insert
   on the writer mutex (`MUTEX_WRITER_ARMS`). In `writer_scaling.py`, its counters pass reports
   zero lock fallbacks and zero lock restarts by construction.
2. **CI Callgrind baselines**: The `instruction-counts` job on `main` measuring the plain
   `bytesmap_insert` arm and the mutex wrapper `sync_bytesmap_insert` arms.
3. **Approved Task C architecture**:
   - **Immutable replacement buckets**: Overwrites and bucket-chain inserts allocate a new replacement
     bucket or bucket copy; modifications are never performed in-place on shared bucket memory.
   - **CAS publication via `SyncExpanseMap::compare_exchange`**: Buckets are published into the underlying
     digital trie using atomic machine-word CAS on the bucket pointer (`*mut Bucket`).
   - **Zero shared mutex on bucket allocation**: Thread-local or system allocation is used for bucket
     creation without taking a shared mutex.
   - **Reclamation via EBR**: Displaced or superseded buckets are retired into the epoch collector
     (`retire`), with epoch pins spanning lookup through publication without nested pins.
4. **No multi-writer throughput run**: Zero executions of `concurrency_writer_bytes` or
   `concurrency_writer_bytes_overwrite` under multi-writer OLC or `--compare` have been performed.

Consequences: All thresholds below are carried over from §19 and §21 unchanged or set as maintainer
policy prior to execution; evaluation is conducted solely on fresh runs executed after this lock.

### 22.3 The claim this gate would license, in full

> On the reference host, at the registered pins, `SyncExpanseBytesMap`'s
> per-node OLC write path delivers more insert throughput at every writer count
> W ≥ 2 than the serialised build delivers at **any** writer count, its
> single-writer throughput is at least the stated fraction F = 0.90 of the serialised
> build's, and its overwrite throughput under Zipfian skew (θ = 0.99) is at least
> ρ = 0.50 of its uniform overwrite throughput. *(workloads: `concurrency_writer_bytes` and `concurrency_writer_bytes_overwrite`)*

Nothing is claimed regarding concurrent removals, key churn outside the registered distributions,
alternate hash algorithms, 32-bit targets, alternate hosts, or the string and blob map wrappers.

### 22.4 The gate

Evaluated per cell. $T_b(W, r)$ is `writer_mops` of build *b* at W writers in round *r*
of an interleaved comparison run; `head` is the default build and `serial` is the same
commit compiled with `ablation-bytes-serial-writers`.

> **G1, scaling.** $R(W, r) = [T_{\text{head}}(W, r) / T_{\text{head}}(1, r)] / [T_{\text{serial}}(W, r) / T_{\text{serial}}(1, r)]$.
> A cell passes iff the BCa 95% lower bound over the round series is strictly above 1.0.
>
> **G2, level.** $`L(W, r) = T_{\text{head}}(W, r) / \max_{W' \in \{1, 2, 4, 8\}} T_{\text{serial}}(W', r)`$.
> A cell passes iff the BCa 95% lower bound is strictly above 1.0.
>
> **G3, price.** $P(r) = T_{\text{head}}(1, r) / T_{\text{serial}}(1, r)$.
> A cell passes iff the BCa 95% lower bound is at least **F = 0.90** — maintainer policy,
> matching §19.4 and §21.4.
>
> **G4, overwrite under skew.** $`K_{\text{skew}}(W, r) = T_{\text{head\_skew}}(W, r) / T_{\text{head\_uniform}}(W, r)`$
> under Zipfian key skew ($\theta = 0.99$) versus uniform key choice over the prefill.
> A cell passes iff the BCa 95% lower bound is at least **ρ = 0.50** (matching §21.4).
>
> **Peak ratio.** $X(8)/X(4) = T_{\text{head}}(8) / T_{\text{head}}(4)$ is computed and reported for both pins
> to diagnose scaling saturation (diagnostic indicator; does not gate).
>
> G1, G2, and G4 cells are W ∈ {2, 4, 8} × two pins (`0-15`, `0,2,4,6,8,10,12,14`) × two
> independent runs: twelve each (36 cells). G3 has one cell per (pin, run): four cells.
> Peak ratio $X(8)/X(4)$ is reported for both pins (four diagnostic cells).
> **The gate is met at a head** when all forty cells pass.
> BCa 95%, 2,000 resamples, computed via `scripts/bca_bootstrap.py`.
>
> **Callgrind, preconditions.** On the head's own `instruction-counts` job:
> every plain-tree arm, and every `sync_map_*`, `sync_set_*`, `sync_strmap_*`, and `sync_blobmap_*` arm,
> within AGENTS.md §6's +0.1% of main. The `sync_bytesmap_*` arms are expected over the
> automated threshold; this registration pre-authorises one `allow-regression:` line
> naming exactly those arms and citing that job's run.
>
> **Defect tripwire, deterministic:** The `occ-stats` replay of the `sync_bytesmap_*`
> single-writer ($W = 1$) counters rows records zero `lock_restarts`.

### 22.5 Math-first audit

`reader_scaling_bounds.mde_from_rounds` (`scripts/reader_scaling_bounds.py:227`, Cohen 1988, ch. 2),
applied to round series of 8 rounds:

Given typical per-round coefficient of variation $\sigma / \mu \le 0.015$ on quiet runs on
the reference host, the relative MDE for $N = 8$ rounds at $\alpha = 0.05, \beta = 0.20$ is:

```math
\text{MDE}_{\text{rel}} = (z_{\alpha/2} + z_{\beta}) \cdot \frac{\sigma}{\mu} \cdot \sqrt{\frac{2}{N}} \approx (1.960 + 0.842) \cdot 0.015 \cdot \sqrt{\frac{2}{8}} \approx 2.802 \cdot 0.015 \cdot 0.5 \approx 0.021 \ (2.1\%)
```

A floor of $F = 0.90$ is resolvable against an expected level $P \approx 0.94-0.96$ (gap $0.04-0.06 > 0.021$).
Level ratios $L(W) \ge 1.4$ for $W \ge 2$ sit far outside the detectable margin.

### 22.6 Rounds, pins, runs, isolation, voids

- **Rounds**: 8 rounds per cell, interleaved Williams Latin square ordering (`williams_positions`).
- **Pins**: Both `0-15` (hyperthread pairs) and `0,2,4,6,8,10,12,14` (one thread per physical core).
- **Runs**: Two independent runs per pin.
- **Process isolation**: One process per cell invocation (`CELL_ISOLATION = "process"`).
- **Workload parameters**:
  - Fresh inserts: $N_0 = 2^{20}$ prefill, $M = 2^{20} = 1,048,576$ fresh keys, 8–16 byte alphanumeric keys (`dist = short`), value = single `u64` word (`concurrency_writer_bytes`).
  - Overwrites under skew: $N_0 = 2^{20}$ prefill, $M = 2^{20}$ overwrites under Zipfian key skew ($\theta = 0.99$) versus uniform key choice over the prefill (`concurrency_writer_bytes_overwrite`).
- **Instrument prerequisite (AGENTS.md §8.20.7)**: `docs/benchmarks/concurrency/scripts/writer_scaling.py --gate-929-bytes`
  computes G1, G2, G3, G4, and $X(8)/X(4)$ with fail-closed self-test assertions before any
  production run is evaluated.

### 22.7 Expected losses

| cell or condition | expectation at lock | consequence of a loss |
|---|---|---|
| G3, single-writer price | about 0.94–0.96 | lower bound under 0.90: gate is not met; F does not move |
| G2, W = 2 | closest level cell, expected > 1.40 | `INCONCLUSIVE` or `REFUTED`: gate not met |
| G1, W = 8, per-core pin | large (> 8), reflecting serial mutex contention collapse | none expected |
| G1, W = 8, `0-15` pin | wider interval due to SMT thread contention | `INCONCLUSIVE` possible if variance spikes; pin is retained |
| G4, overwrite under skew | expected > 0.60 | `INCONCLUSIVE` or `REFUTED`: gate not met |
| Peak ratio X(8)/X(4) | expected > 1.0 on per-core pin; may saturate on `0-15` | diagnostic indicator; does not fail gate |
| `sync_bytesmap_*` Callgrind arms | over the automated threshold (+40% to +80%) | reported under pre-authorised `allow-regression:` |
| Untargeted Callgrind arms | within +0.1% | precondition failure: no throughput run is taken |
| Deterministic tripwire | zero single-writer `lock_restarts` | non-zero trips defect investigation |

### 22.8 Verdicts

Standard vocabulary: `PASS`, `REFUTED`, `INCONCLUSIVE`, `INTERMEDIATE`, `NOT_EVALUABLE`.
The gate is met only when all forty cells evaluate to `PASS`.

### 22.9 If it is met: what the promotion must say

The default build's single writer is slower than the serialised build by the measured P,
stated with its interval in `docs/ARCHITECTURE.md` §4 and rustdoc, beside the
`ablation-bytes-serial-writers` feature that restores the serialised protocol (AGENTS.md §2.7).
Explicitly stated limitations: 8–16 byte alphanumeric keys; SipHash-2-4 hash keys;
no concurrent removals; 64-bit targets only.

### 22.10 Concurrency invariants & bucket replacement semantics

1. **Immutable Bucket Chains & CAS Update Protocol**:
   Bucket records store `(len: u16, hash: u64, key: [u8; len], val: u64)` and a `next: *mut Bucket` pointer.
   Writers never mutate existing bucket fields in-place. An update or insert allocates a new bucket
   or replicates the chain prefix up to the modified node, and publishes the new head via
   `SyncExpanseMap::compare_exchange(hash, old_head, new_head)`. If another writer updates the head
   concurrently, CAS fails, and the writer retries from the current head.
2. **Reclamation & Epoch Pinning**:
   Old bucket versions are retired via `Collector::retire` and freed only when all concurrent readers
   and writers exit the protecting epoch. An epoch pin covers bucket chain traversal through
   CAS publication; no nested `with_writer_pin` calls are permitted.
3. **Collision Resistance**:
   SipHash-2-4 provides 64-bit pseudorandom dispersion over arbitrary byte slices, bounding collision
   chain lengths to $O(1)$ under non-adversarial keys. Bucket chain traversal is $O(1)$ expected time.


## 23. Pre-registration for #1144 — the cost of a concurrent count under a writer (appended and locked 2026-09-25, before any admissible run of it)

### 23.1 The gate

#1144's first gate, verbatim: *the cost of a count under a writer is recorded, deterministically and in wall clock, on uniform and one-top-byte key sets.* No magnitude is predicted, and the blast radius is benches and examples only. Its falsifier is an unrecorded cell: the gate is met when every cell of §23.3 is in two non-void committed runs (§23.4), not by any value those cells take.

The deterministic half is the Callgrind arms merged in #1155: `map_count_below` and `set_count_below` on plain trees, and `sync_map_count_locked`, `sync_map_write_twin` and `sync_map_count_after_write` over one probe stream and one stream of absent keys (`crates/expanse/benches/instructions.rs`), whose combined arm minus its two twins is the fold's instruction cost. This section fixes the wall-clock half.

### 23.2 What is measured, and the statistic

- **Writer cost of a counting reader.** For each probe mode and W ∈ {1, 4}: `writer_mops(W, R = 1) / writer_mops(W, R = 0)`, one ratio per round, the two cells run in the same round of the same build, with a BCa 95% interval of the mean over rounds (`count_writer_ratio`, `docs/benchmarks/concurrency/scripts/writer_scaling.py`). Reported, not gated: #1144 asks for the cost to be recorded, and no floor was set before this section.
- **Per cell:** writer Mops/s where W ≥ 1 and reader Mops/s where R ≥ 1, each the mean over rounds with a BCa 95% interval; the counters pass's `locked_reads`, `quiesce_calls`, `lock_fallbacks` and fallback causes, summed over rounds.
- **The no-fold control:** W = 0, R = 1. With no writer, every count after the first finds the dirty mask clean, so its reader throughput is the lock, quiesce and count alone.
- **Uniform against one top byte** is read from the two probe modes' intervals. A difference between them is claimed only when both runs show it in the same direction with non-overlapping intervals (`docs/BENCHMARKING.md` rule 18); a single run's interval does not bound between-run spread.

### 23.3 Instruments and cells

- Harness: `crates/expanse/examples/writer_scaling.rs` reader mode, `--arm map --read-op count_locked`: each reader calls `count_below(k)` under `SyncExpanseMap::with_locked` for its own Fisher–Yates permutation of the prefill, cycled until the writers join. `--readers 0` with `count_locked` is the writers-only control over the same prefill and fresh stream.
- Probe modes:
  - `uniform`: the writer sweep's 2^20-key 64-bit prefill plus reader mode's 256 hotspot keys; writers insert the 2^20 uniform fresh keys.
  - `one_top_byte`: 2^20 keys under the top byte `0xA5` (the writer sweep's draws shifted right one byte), nothing else prefilled; writers insert 2^20 fresh keys of the same shape. Every write marks the one top digit, so every count after a write refolds the whole tree.
- Cells: probe ∈ {`uniform`, `one_top_byte`} × (W, R) ∈ {(0, 1), (1, 0), (1, 1), (4, 0), (4, 1)}: ten cells.
- Driver: `writer_scaling.py --count-cells`, 8 rounds. Each round runs both probe blocks, the first alternating by round; within a block the five cells follow that round's row of the Williams construction (five is odd, so positions are not exactly balanced over 8 rounds; every row is a permutation). One harness process per cell (§15). The throughput build runs every round before the `occ-stats` counters build.
- Pin: `0,2,4,6,8,10,12,14`, one thread per physical P-core on the reference host, as §12.4. A cell holds at most five threads, so every thread has a core of its own, including a writer parked while a count quiesces the writers. `AGENTS.md` §8.20.5 step 0 warns against this pin where blocked threads need CPUs to park on; with five threads on eight pinned cores none shares one.
- Load: snapshots before, between and after the passes, with the busy-CPU delta (AGENTS.md §8.17), recorded in the artifact.
- Artifact: `docs/benchmarks/concurrency/results/count_cells_writer_scaling.json`, then a second run to `count_cells_writer_scaling_run2.json`, at one commit.

### 23.4 What voids a cell or a run

- An applied pin other than §23.3's, or a row recording another (the driver refuses to start).
- `--quick`, which is a smoke run of the instrument, not these cells.
- Host contention as AGENTS.md §8.17 defines it: a non-target process above about 100% CPU, a load average above cores / 2, or a load shift above 2 between passes. A contaminated run is discarded and the discard disclosed in the suite README, never reinterpreted.
- A broken counters identity (`quiesce_calls ≠ lock_fallbacks + locked_reads`, `locked_reads ≠ reader_ops`, causes not summing to `lock_fallbacks`): the driver refuses the run.
- The runs being at different commits.

### 23.5 What had been seen when this was written

- The Callgrind arms of §23.1 had run once, in #1155's own CI; their counts are read from `main`'s `instruction-counts` job, not quoted here.
- A native release run on an arm64 laptop, used only to size those arms (not published, not an input to anything here), took about 20 s for 50,000 `one_top_byte` insert–count–remove steps, against well under 1 s for `random` and `sequential`.
- A `--quick` smoke of the new cells on the same laptop (4,096 prefill keys, one round, no pin, no interval) showed the `one_top_byte` W = 1 writer far slower beside a counting reader than without one. That is one uncontrolled sample on a non-reference host, not a measurement; it fixed nothing in this section, which states no magnitude.

### 23.6 Not predicted, and out of scope

- No magnitude, direction or ordering between cells is predicted, and no threshold is set; a later design choice among #1144's options may set one, in its own pre-registration.
- W = 8, R > 1 and ordered reads are outside this gate. So are the string map, `sync32`, and any change to the write path, the fold or `nav`.

## 24. Pre-registration for #1208 — a shared `BranchU` crossing its demotion floor, in wall clock (appended and locked 2026-09-27, before any admissible run of it)

### 24.1 The gate

#1208 asks for wall-clock cells that drive one shared `BranchU` back and forth across its demotion floor while other writers and readers run, at W ∈ {1, 2, 4, 8} with and without readers, beside a control of the same operation count far from the floor. Its gate, as fixed here: *the band excess per crossing is recorded, with a BCa 95% interval, for every cell of §24.3 in two non-void runs at one commit (§24.4).* One direction is predicted (§24.2, P24.1); no magnitude is. The blast radius is benches, examples, the driver, and one diagnostic feature that no default build compiles.

The deterministic half already exists: the Callgrind arm `sync_map_branchu_band` (§24.5). Instruction counts cannot see the writer gate closing, the drain of the optimistic writers, or a reader's retries, and those are what each crossing costs a concurrent tree.

### 24.2 What is measured, and the statistic

- **The band excess per crossing.** For each build, W and R, and each round r: `Δ_r = (t_band,r − t_twin,r) / X_r`, where `t` is the owner writer's elapsed time for its cycles (barrier release to its join, `owner_elapsed_s`), `band` is the duty-1 cell and `twin` the duty-0 cell of the same (build, W, R) in the same round, and `X_r` is the crossings the band cell's owner counted (`crossings` = 2 × `owner_cycles`). Reported as the mean over rounds with a BCa 95% interval (`bca_bootstrap_ci_with_method`, 2,000 resamples), in nanoseconds per crossing.
- **The paired ratio** `t_band,r / t_twin,r`, mean over rounds with a BCa 95% interval, beside it.
- **P24.1 (directional, gated).** At W = 1, R = 0, default build, the ratio's BCa lower bound is above 1.0 in both runs. `scripts/branchu_band_bounds.py` sizes it (§24.5): the projection is far above the resolution of 8 rounds, so a run in which this fails is a finding about the instrument or the engine, and is reported as REFUTED, not re-run.
- **Everything else is reported, not gated:** Δ at W ≥ 2 and R = 1; how Δ moves with W; the default build against `ablation-one-digit-band`; reader Mops/s of each R = 1 cell with its interval; and, from the counters pass, `gate_wait_cycles` and `quiesce_drain_cycles` per crossing and per crossing × W, with the fallbacks by cause and by branch-split kind per owner cycle.
- **A difference between builds, between W, or between R = 0 and R = 1** is claimed only when both runs show it in the same direction with non-overlapping intervals (`docs/BENCHMARKING.md` rule 18).
- **What Δ contains.** The band cycle and the twin cycle make the same owner operations, but not the same work: between its two crossings the band cycle's insertions land on a `BranchB`, and the twin's never leave a `BranchU`. Δ is the whole band cycle's excess over the twin, charged to its crossings, not the cost of an exclusive section alone. The counters pass's `branch_split_*` and `cap_expansion_*` counts per owner cycle say which fallbacks the band cycle adds; no share of Δ is attributed to them (AGENTS.md §8.20.4).

### 24.3 Instruments and cells

- **Harness:** `crates/expanse/examples/writer_scaling.rs --band-cells --duty <0|1> --writers W --readers R`, one `SyncExpanseMap` per cell, workload id `concurrency_branchu_band_map_64bit`.
  - The boundary branch holds one key per second-byte digit under the top byte `0x01`: `band_key(d) = 0x01 << 56 | d << 48 | 0x42`.
  - Its layout is derived from the engine's constants, so it follows the build. With `F` = `BRANCHU_TO_B_DOWN` and `U` = `BITMAP_TO_UNCOMPRESSED_THRESHOLD` (192), digits `0..F` are fixed; the band is `F..=U`, `S = U + 1 − F` digits (33 by default, 2 under the ablation).
  - **Duty 1:** the branch starts at `U + 1` (193) digits. One owner cycle removes the band's digits in descending order, the last removal falling back as `DemoteU` to the exclusive remove that demotes the branch at `F` digits, then reinserts them in ascending order, the last falling back as `Upgrade`. Two crossings per cycle, by construction; the harness self-test checks the node form after every step of one cycle of each duty.
  - **Duty 0 (the twin):** digits `0..U + 2` are always present, and the owner cycles `S` digits from `U + 2` (194) upward, so the branch moves between 194 and `194 + S` digits (227 by default) and never reaches either threshold. Same operation count per cycle.
  - **One owner, so the crossings are deterministic.** The owner writer makes every band removal and insertion, `8192` cycles per cell, and counts its own cycles. The other W − 1 writers overwrite the values of fixed-digit keys (an insert of a present key, which never changes the digit count), each in its own Fisher–Yates order, at least once and then until the owner joins. The R readers `get` the fixed-digit keys on reader handles, each in its own Fisher–Yates order, at least once and then until the owner joins. Every answer is checked; any disagreement refuses the cell.
  - **The pre-lock scan is not symmetric near the floor, and is disclosed.** An optimistic removal from a `BranchU` counts the branch's non-null slots from slot 0, stopping at `F + 2` (`sync.rs`, `null_branch_u_slot`). Both duties keep slots `0..F + 2` populated whenever that cap is reachable, so every removal stops at slot `F + 1`, except the one duty-1 removal per cycle that finds `F + 1` digits: it reads all 256 slots and falls back. That scan is part of the crossing it detects.
- **Builds:** the default build and `--features ablation-one-digit-band`, which restores `BRANCHU_TO_B_DOWN = BITMAP_TO_UNCOMPRESSED_THRESHOLD − 1`, the one-digit band #1221 widened. #1221 promoted the wider band without an inverse; this is that inverse (AGENTS.md §2.7). Each build also has its `occ-stats` counters build. All four come from one commit.
- **Cells:** build ∈ {default, `ablation-one-digit-band`} × duty ∈ {1, 0} × (W, R) ∈ {(1, 0), (1, 1), (2, 0), (2, 1), (4, 0), (4, 1), (8, 0)}: 28 per round. W = 8 with R = 1 is not a cell: the pin has eight cores, and a ninth thread would share one (AGENTS.md §8.20.5 step 0).
- **Driver:** `docs/benchmarks/concurrency/scripts/writer_scaling.py --band-cells`, 8 rounds. Within round r the 28 cells follow row r of the Williams construction over them; 8 rounds do not balance 28 positions, and every row is a permutation. One harness process per cell (§15), with a load window (`begin_cell` / `end_cell`) around each timed cell. The throughput pass runs every round before the counters pass, which runs the same schedule.
- **Pin:** `0,2,4,6,8,10,12,14`, one thread per physical P-core on the reference host, as §12.4 and §23.3, which the driver sets when unset and refuses to replace.
- **Artifacts:** `docs/benchmarks/concurrency/results/branchu_band_writer_scaling.json`, then `branchu_band_writer_scaling_run2.json`, at one commit.

### 24.4 What voids a cell or a run

- An applied pin other than §24.3's, or a row recording another; `--quick`; a `--cycles` other than 8192. The driver refuses the first and records the others as void.
- Host contention as AGENTS.md §8.17 defines it: a non-target process above about 100% CPU, a load average above cores / 2, or a load shift above 2 between passes. A contaminated run is discarded and the discard disclosed in the suite README, never reinterpreted.
- A broken counters identity: `Stat::Inserts` ≠ owner insertions + overwrites; `Stat::WriteOps` ≠ inserts + owner removals + `lock_fallbacks` (the removal identity, band rows only); `quiesce_calls` ≠ `lock_fallbacks` + `locked_reads`; causes not summing to `lock_fallbacks`; a branch-split partition that does not sum. The harness and the driver both refuse the run.
- **The counters must confirm the schedule.** At W = 1, duty 1: `branch_split_demote_u` = `branch_split_upgrade` = `owner_cycles` exactly, every round. At duty 0, any W: both are 0 exactly. A mismatch voids the run: the cells did not run the workload registered here. At W ≥ 2, duty 1, a crossing can instead fall back for another cause (the writer gate already closed, or the retry budget exhausted), so each is at most `owner_cycles`, and over the cell's rounds `(demote_u + upgrade) / (2 × owner_cycles)` must be at least 0.95. Below that, the cell's Δ is still reported, marked `crossings_unattributed`, and is not read against another cell.
- A row whose `one_digit_band`, `owner_cycles` or crossing count disagrees with the build and schedule that asked for it; a cell whose tree does not hold exactly one `BranchU` before and after it. The harness or the driver refuses both.
- The runs being at different commits.

### 24.5 What had been seen when this was written

- **Callgrind, deterministic, x86-64 CI runner.** `sync_map_branchu_band/top` (100 cycles, 193 ↔ 160, 200 crossings): 10,116,194 instructions at the default target and 8,977,550 at `-C target-cpu=x86-64-v3` (measured: CI run https://github.com/orieg/expanse/actions/runs/36326465116, `5fdb77f3`). `sync_map_branchu_thrash/top` on #1204's one-digit band (100 cycles, 191 ↔ 193, 200 crossings): 3,278,033 and 2,747,239, 16,390 and 13,736 per crossing including the cycle's four operations (measured: CI run https://github.com/orieg/expanse/actions/runs/36256715324, `e7d57aa3`).
- **`scripts/branchu_band_bounds.py`** turns those into a crossing share of a band cycle's instructions at W = 1 of 0.3240 (default target) and 0.3060 (x86-64-v3). If time per instruction were the same on the band and twin cycles, `t_band / t_twin` at W = 1 would be about 1.48 and 1.44; that is a hypothesis carrying two unmeasured assumptions (the thrash arm's per-crossing cost transfers to the widened band, and the twin cycle is the band cycle less its crossings), and it is not a prediction. The largest per-round CV of `writer_mops` in the committed `170a4bc3` per-core map cells, 0.0236, borrowed from another workload, gives a minimum detectable ratio departure of 0.0331 at 8 rounds (two-sided 5%, 80% power), which is what P24.1's direction rests on.
- The instrument was smoke-run with `--quick` on a non-reference Linux development host, in the default, `ablation-one-digit-band` and `occ-stats` builds, to confirm that the cells run, that their identities hold, and that the counters see one `DemoteU` and one `Upgrade` per duty-1 cycle at W = 1 and none at duty 0. No timing from it is an input to this section.

### 24.6 Not predicted, and out of scope

- No magnitude of Δ is predicted at any cell, nor its direction outside P24.1, nor any ordering across W, R or builds. Whether the 32-digit band or the one-digit band costs more per crossing is what the ablation measures, not something this section expects.
- Out of scope: `SyncExpanseSet`, `sync32`, the string, bytes and blob wrappers; any change to the band, the write path or the fallback; W = 8 with R ≥ 1.

### 24.7 Why the base-engine control is not a cell

#1208 asked for a second control on the base engine without the fix, `0489deec` (main before #1204). It is replaced by the twin and the ablation build, both on a correct engine:

- On `0489deec` an optimistic removal could leave a shared `BranchU` at or below its floor (#1079). The duty-1 workload drives exactly that path, so the base engine would be timed doing what #1204 made incorrect, and `validate` rejects a `BranchU` at or below its floor (`crates/expanse/src/validate.rs`). A throughput against it compares against a defect, not against an alternative design.
- It also carries the one-digit band, so it differs from the head in two ways at once; and there a shared removal never demotes the branch, so it has no downward exclusive section to measure (#1204's description records its thrash arm at 715 instructions per crossing on the base engine and 16,196 with the fix, from a base-versus-head profile that is not a committed artifact; cited for the mechanism, not as a figure this section uses).
- The harness cannot build against it: its counters read `Stat::BranchSplitDemoteU`, which does not exist there.

What the base control was meant to answer, the cost of a crossing that falls back beside one that does not, is the twin (the same build, no crossings); what the old band costs on a correct engine is the ablation build.

### 24.8 Amendment: a crossing can fall back as contention at W = 1 with a reader (appended 2026-09-27, after two voided runs and before any admissible one)

§24.1–§24.7 are unchanged above. This subsection corrects one rule of §24.4, which was wrong about the engine.

**What §24.4 required, and what happened.** §24.4 voided a run unless, at W = 1 and duty 1, `branch_split_demote_u` = `branch_split_upgrade` = `owner_cycles` exactly. Both reference-host runs of `writer_scaling_band` at `3a1ef859` were voided by it, each in a W = 1, R = 1 counters cell:

- Run 1, `ablation-one-digit-band` `band_d1_w1_r1`, round 1: DemoteU 8,192 and Upgrade 8,191 over 8,192 cycles (measured: https://github.com/orieg/expanse/actions/runs/36371158780).
- Run 2, the default build, `band_d1_w1_r1`, round 2: DemoteU 8,192 and Upgrade 8,191 over 8,192 cycles (measured: https://github.com/orieg/expanse/actions/runs/36371755132).

The driver refuses before it writes an artifact, so neither run's other counters were kept, and no throughput from either was read.

**Cause, reproduced.** The same cell was re-run with the harness's counters build on a non-reference Linux development host, 8,192 cycles per run and two threads pinned to two cores. The counts, not the timings, are what this section uses.

| build, readers | runs | runs with a missing crossing | missing | counted instead as |
|---|---:|---:|---|---|
| `ablation-one-digit-band`, R = 1 | 800 | 2 | one Upgrade; one DemoteU | 1 contention fallback each, `contention_gate_closed` 1, with 1 and 6 locked reads |
| default, R = 1 | 400 | 1 | one Upgrade | 2 contention fallbacks, both `contention_gate_closed`, with 14 locked reads |
| `ablation-one-digit-band`, R = 0 | 300 | 0 | — | no contention fallback |
| default, R = 0 | 300 | 0 | — | no contention fallback |

In every run with a missing crossing, `lock_fallbacks` is unchanged at 2 × cycles or above, the branch-split fallbacks are one short, and a contention fallback stands in for the missing one. The row's other identities hold, and the tree is a `BranchU` before and after the cell.

This is the engine working as designed, not a harness fault. A reader whose optimistic `get` exhausts its retries takes `read_locked`, which closes the writer gate (`Shared::quiesce_writers`, `crates/expanse/src/sync.rs`). The owner may have entered as a writer just before the gate closed. Its optimistic attempt then finds the gate closed at the top of its retry loop and falls back as `FallbackCause::Contention`, before it reaches the branch whose form it would change. The exclusive section it falls back to crosses the band: `insert_shared` promotes the branch and `remove_shared` demotes it. The crossing happens and the owner counts it; only the counter that names its cause differs. With R = 0 no other thread can close the gate. §24.4 assumed that at W = 1 the owner is the only thread that falls back, and a reader can close the gate too.

**The corrected rule, which replaces §24.4's W = 1 clause for every run after this subsection.**

- **W = 1, R = 0, duty 1:** `branch_split_demote_u` = `branch_split_upgrade` = `owner_cycles` exactly. This is unchanged, and it is P24.1's cell.
- **Every duty-1 cell:** DemoteU ≤ `owner_cycles` and Upgrade ≤ `owner_cycles`. The shortfall `2 × owner_cycles − DemoteU − Upgrade` must not exceed the cell's contention fallbacks: every crossing the branch-split counters miss must be a contention fallback. At W = 1, the contention fallbacks must not exceed `locked_reads`, because with one writer only a reader's locked read closes the gate.
- **Duty 0:** DemoteU = Upgrade = 0 exactly, unchanged.
- A row that breaks any of these voids the run.
- **The 0.95 attribution floor** of §24.4 now covers W = 1, R = 1 as well as W ≥ 2. Below it, a cell is marked `crossings_unattributed` and reported.

`check_band_counters_row` in `docs/benchmarks/concurrency/scripts/writer_scaling.py` enforces this rule. Its self-test holds three cases: the voided pattern, which is now admitted at R = 1 and refused at R = 0; a contention fallback at W = 1 with no locked read, which is refused; and a missing crossing that no contention fallback covers, which is refused. Every one of the 1,800 reproduced rows passes it. Three of them failed the rule it replaces.

**The two runs.** Runs 36371158780 and 36371755132 are void and stay void. Their artifacts were never written, and nothing from them is re-read under this rule. The two runs §24.3 asks for are fresh dispatches after this subsection lands.

**Label (AGENTS.md §8.19).** This changes a void rule after seeing its outcome on two runs, so what it can move is stated. It does not move:

- the statistic: Δ and `t_band / t_twin` read the owner's own crossing count, which the harness checks structurally, not the counters;
- the cells, rounds, cycles, pin or P24.1's threshold;
- P24.1's cell, whose exact rule stands.

What it changes is which runs are admissible. At W = 1, R = 1 it now admits a run in which a reader closed the gate across a crossing, and such a run was admissible at W ≥ 2 already. The results of later runs are therefore not relabelled INTERMEDIATE. Any cell whose crossings the counters under-attribute carries its shortfall and, below 0.95, the `crossings_unattributed` mark. The gate of §24.1 is reported as met "under §24.8".

## 25. Pre-registration for #1270 — the host guard's on-pin void boundary, from injected load (appended and locked 2026-09-28, before any run of it)

### 25.1 The gate

`bench_host_guard.py summarize` voids a wall-clock run when any 2 s sample reads on-pin foreign load above `RUN_ON_PIN_VOID` = 0.25 core-equivalents (`docs/BENCHMARKING.md` rule 8). That value was set by assumption. #1270 asks for it to be set from a measured sensitivity instead. The gate, as fixed here: *`RUN_ON_PIN_VOID` becomes the largest injected level at which no gated cell of §25.3 is harmed by §25.4's rule, in one admissible run of this protocol on the reference host.* The blast radius is `RUN_ON_PIN_VOID` and `START_ON_PIN_MAX` in `scripts/bench_provenance.py`, and the prose that quotes them.

The reading those constants are compared with changed first. `bench_host_guard.py` now reads the own tree immediately after `/proc/stat` (`docs/BENCHMARKING.md` rule 8, "The two operands are read together"). Every void of #1270's runs that no process explained came from the old reading order, so this experiment measures the boundary with the new guard. It does not measure whether the old one was right.

### 25.2 What is measured, and the statistic

- **The window throughput** `T = (read_ops + write_ops) / elapsed_s` of every window in `mixed_concurrency.py`'s `rounds_raw`, the same quantity its cells report.
- **The effect of a level ℓ on a cell c:** `r(ℓ, c) = mean T(ℓ) / mean T(control)`, with each arm's windows pooled over the four blocks. It carries a two-sample BCa interval (`bca_bootstrap_ratio_ci_with_method`, 2,000 resamples) at per-cell confidence 1 − 0.05 / 11, a Bonferroni bound over the 11 gated cells. The per-block point ratio is reported beside it.
- **The injection is continuous for the whole arm.** A void reads single 2 s samples. A single sample at level ℓ overlaps a few of a cell's 18 windows, and continuous load overlaps all of them. The continuous effect is therefore the upper end of what a sample at that level can do, and a boundary set from it errs toward voiding.
- **The guard's readings.** For every arm: the new guard's samples, and beside them the previous guard's (`scripts/bench_host_guard.py` at `d0f91918`), both watching the same suite process. Each sample's on-pin reading, its attribution, and `summarize`'s exit status and output are recorded.
- **P25.1 (reported, not gated).** No control-arm sample of the new guard reads above 0.25 on the pinned CPUs. The old guard's control-arm samples include negative readings, as in #1270's runs.
- **P25.2 (reported, not gated).** `scripts/host_guard_bounds.py`'s `fair_share_loss` gives the loss if throughput tracks CPU share and the scheduler puts the load on an idle pinned CPU when one exists: 0 at W = 1 and 4 for every level, and 0.00625, 0.015625 and 0.03125 at W = 16 for 0.10, 0.25 and 0.50. That is a hypothesis carrying both of those assumptions, not a prediction of magnitude.
- **Calibration (reported).** Each injected arm's mean new-guard on-pin reading, beside the load the injector achieved.

### 25.3 Instruments and cells

- **Suite:** `docs/benchmarks/concurrency/scripts/mixed_concurrency.py --engines map,set --workloads 100,50 --threads 1,4,16 --rounds 18`. This is the workflow's `concurrency` invocation restricted to `map` and `set`, with one process per arm and block.
- **Cells:** `map` and `set` × 100 % and 50 % reads × W ∈ {1, 4, 16}, 12 per arm. **Gated:** the 11 whose per-window CV was at most 0.05 in both committed runs, `results/baseline_concurrent_mixed.json` and `results/baseline_concurrent_mixed_run2.json` (provenance commit `0a3ed07a`). `map` 50 % at W = 16 read 0.1053 and 0.1048 there, and is reported, not gated. At n = 72 windows per arm, `host_guard_bounds.py` puts the minimum detectable |r − 1| (80 % power) at 0.00613, 0.01226 and 0.01840 for a CV of 0.01, 0.02 and 0.03.
- **Arms:** control (no injection), 0.10, 0.25 and 0.50 CPUs.
- **Injection:** `host_guard_sensitivity.py inject`, started with `setsid -f`, so it is not a child of the driver and no `RUSAGE_CHILDREN` counts it. Its affinity is the pin set, it runs at nice 0 under the name `guard-inject`, and it burns ℓ × 10 ms of its own thread CPU clock every 10 ms. It records the load it achieved (CPU seconds / wall seconds). It is started from the driver's session, so the guard classes it `runner` (the run's cgroup, outside its tree).
- **Schedule:** 4 blocks. Block b runs the arms in row b of the Williams square ((0,1,3,2), (1,2,0,3), (2,3,1,0), (3,0,2,1)) over (control, 0.10, 0.25, 0.50). Every arm is once in every position and follows every other arm once. That gives 72 windows per arm per cell.
- **Idle arm, first:** both guards run `watch --interval 1` over a `sleep`, for 60 windows. This is the 1 s noise floor the start gate reads.
- **Negative control, last:** one arm at 1.00 CPU.
- **Driver:** `docs/benchmarks/concurrency/scripts/host_guard_sensitivity.py run`, under `scripts/bench_lock.py`. The decision is recomputed from the artifact alone by `host_guard_sensitivity.py evaluate`.
- **Host:** the i9-12900F reference host, pin `0-15` (the workflow's). The governor is the host's own, because setting `performance` needs the root-owned helper that only the runner's service account uses. It is recorded, and it must be the same in every sample.
- **Artifact:** `docs/benchmarks/concurrency/results/gate_host_guard_sensitivity.json`.

### 25.4 The decision rule

- **Harmful.** A level ℓ harms gated cell c when the upper bound of r(ℓ, c)'s interval is below 1, and the per-block ratio is below 1 in at least 3 of the 4 blocks (`docs/BENCHMARKING.md` rule 18). A level is harmful when it harms any gated cell.
- **A/A.** The control's blocks {0, 1} are compared with its blocks {2, 3}, per gated cell, at the same confidence. If any interval excludes 1, the run is `INCONCLUSIVE`: the between-block drift exceeds what the test resolves, and `RUN_ON_PIN_VOID` stays 0.25.
- **The boundary** B is the largest level ℓ such that neither ℓ nor any smaller level is harmful. If 0.10 is harmful, B = 0.10 (`DECIDED_BELOW_RESOLUTION`). A level below the smallest one measured is not set by reasoning from the ones above it, and the finding stays open on #1270.
- **The start gate** stays stricter than the void boundary by the ratio it has now: `START_ON_PIN_MAX = min(0.1, 0.4 × B)`.
- **The noise floor is reported, never cleared.** If a new-guard control-arm sample reads above B, the count is reported. B is not raised past it: that would be moving the bound to the observation (AGENTS.md §8.19).
- **Fail closed.** In the 1.00 arm, the new guard's `summarize` must exit 1 and print `the host was disturbed during the run`, and at least one sample must read above B. The check matches that string, not the exit status alone (AGENTS.md §5). A failure here voids the run, whatever the other arms show.

### 25.5 What voids a run

- A `--blocks` or `--rounds` other than §25.3's. The driver marks the artifact not admissible.
- More than one governor across the samples.
- A `mixed_concurrency.py` process that exits non-zero, or an injector that does not record its achieved load. The driver refuses both.
- Host contention from outside the experiment, under AGENTS.md §8.17. The pre-flight checks before the run are live `top`, `docker ps`, no `kvbench` or smoke process, load1 below 1.5, and no bench job in progress. During the run, an arm whose new-guard samples attribute more than 0.10 CPU on average to `user`-class tasks, or any one sample more than 1.0, voids the run (`evaluate` checks both). The discard is disclosed.

### 25.6 What had been seen when this was written

- The seven `concurrency` runs of 2026-09-28 named in `docs/BENCHMARKING.md` rule 8 and #1270, with their `host-activity.jsonl` records. Those records show the pattern and the reading-order mechanism the attribution fix addresses. No injected-load measurement exists.
- `scripts/host_guard_bounds.py`, whose table is quoted above.

### 25.7 Not predicted, and out of scope

- No magnitude of r is predicted in any cell. Whether W = 4 cells, which leave pinned CPUs idle, are affected by SMT sharing is measured, not assumed.
- Out of scope: `RUN_FOREIGN_VOID`, `RUN_PROCESS_VOID_PCT`, the start gate's other thresholds, and any sustained-versus-single-sample void rule. Changing to a sustained rule would loosen the gate without this measurement's support.

### 25.8 Amendment: the injector's period is jittered (appended 2026-09-28, before any run on the reference host)

§25.1–§25.7 are unchanged above. This subsection changes one detail of §25.3's injection.

On a kernel without `CONFIG_VIRT_CPU_ACCOUNTING_GEN`, `/proc/stat` charges a CPU's user and system time by sampling at the scheduler tick, while a task's `utime + stime` is scaled to its measured runtime. In a smoke run on a non-reference Linux development host (`CONFIG_HZ=250`), the per-CPU counters under-read an injector with a fixed 10 ms period. With each period drawn from 5–15 ms, the counters read about what the injector's own CPU clock reported. The guard's task-time attribution read the injector correctly in both cases. The cause is unmeasured; a burst staying in phase with the tick is the hypothesis. That host also carried unrelated container load, so the smoke settled only the direction of the change. It was not committed, and none of its readings is an input to §25.4. §25.2's calibration measures the jittered injector on the reference host.

**The change.** The injector draws each period's length uniformly from 0.5–1.5 × 10 ms, from a PRNG seeded with its PID, and burns ℓ of each period on its own thread CPU clock (`inject --jitter`, the default). Nothing else in §25 changes. This amendment lands before any run of §25, so no result is relabelled.

## 26. Pre-registration for #1280 — the optimistic `SyncExpanseBlobMap` removal, on the repaired mixed instrument (appended and locked 2026-09-29, before any run of the repaired instrument or of the builds below)

### 26.1 Context

Every `SyncExpanseBlobMap::remove` took `Shared::remove_root_covered`, present key or not: the fallback mutex, a writer quiesce that drains every allocated writer slot, the writer mutex, a population re-sync and `fold_writer_arenas`, with the tree version word held for the section (`ExpanseBlobMap` publishes its root), so concurrent readers restart across it. `docs/ARCHITECTURE.md` §4 records blob removals as serialised; #929's blob gate (§21, README §19) measured fresh inserts only and states that removals were not measured.

The published `core_concurrency` blob cells could not say what that costs: both Expanse blob arms timed an insert that failed once the arena reached its 1 GiB cap, and the key-parity write selector left every removal a miss (#1280). `benches/concurrency.rs` is repaired on `fix/1280-mixed-harness` (`859214cb`, `d1c3ca2a`): stationary occupancy at half the keyspace, 0.25 of writes removing a present key (checked per window by `mixed_concurrency.py`), in-window compaction on one trigger for both Expanse blob arms, one worker pool per cell. This section registers what is measured on that instrument and how it is read.

### 26.2 Builds

| build | what it is | ref |
|---|---|---|
| B | the repaired harness; blob removal serialised as on `main` | `fix/1280-mixed-harness` head |
| M | B plus a remove-miss short-circuit: an absent key is answered by a validated read (`contains_key`), a present one takes B's serialised removal. Diagnostic, never merged | `exp/1280-m-miss-shortcut` |
| C | B plus the optimistic removal: `olc_remove_map` over the blob host, the unlinked record charged through `charge_dead` under the writer-gate guard, B's serialised path for root-leaf state and fallbacks | `perf/1280-blob-optimistic-remove` head |
| A | C plus `has_writer_deltas` stored only when it reads `false`. Diagnostic unless its gate below passes | `exp/1280-a-deltas-flag` |

The exact commit of each build is the one its dispatch records in `provenance.commit`; a build whose commit differs between its two runs voids both.

### 26.3 Instrument, runs and order

- The `concurrency` suite of `bench_baremetal.yml`, unchanged: `mixed_concurrency.py --threads 1,4,16 --workloads 100,50 --engines all`, 18 rounds per cell, pin `0-15`, host guard as on `main`.
- Two runs per build, dispatched in the order B, M, C, A, A, C, M, B. Run 1 of a build is its first dispatch; comparisons pair run k with run k.
- B's two runs are also the baseline the harness PR publishes (§26.8).

### 26.4 Gates

All on `SyncExpanseBlobMap`, 50 % read, total ops/s. X ÷ Y is the ratio of mean total ops/s over a cell's 18 windows, two-sample BCa 95 % (`bca_bootstrap_ratio_ci_with_method`), computed by `docs/benchmarks/concurrency/scripts/blob_remove_gate.py`, which holds these floors as constants.

| id | comparison | threads | floor | reading |
|---|---|--:|--:|---|
| G1 | C ÷ B | 16 | 1.0 | `PASS` when the lower bound exceeds the floor in both runs, `REFUTED` when the upper bound is below it in both, `INCONCLUSIVE` otherwise |
| G2 | C ÷ B | 4 | 1.0 | as G1 |
| G3 | C ÷ B | 1 | 0.90 | the single-thread price, as G1 (§21's floor) |
| G4 | C's own paired C(16), `scaling_c_n` of the artifact | 16 | 1.0 | as G1: `PASS` means C's 16-thread cell is not below its 1-thread cell |
| R1 | M ÷ B | 16, 4 | — | reported with intervals: the share of the change the miss path alone delivers |
| R2 | A ÷ C | 16, 4 | — | reported; A is proposed for the PR only if its lower bound exceeds 1.0 at 16 threads in both runs and its Callgrind table meets §26.6 |

C merges as the default when G1 and G3 pass. G2 and G4 are read and published whatever they show. No gate is evaluated on M or A beyond R1 and R2.

### 26.5 Controls, and what voids a comparison

- Cells the change should not move: `blob` 100 % read at 1 and 16 threads, `map` 50 % and 100 % read at 16 threads, read as C ÷ B. A control whose interval excludes [0.95, 1.05] in the same direction in both runs marks the comparison `DRIFT` (`docs/BENCHMARKING.md` rule 18) and G1–G4 are not read from that pair of runs.
- A run the host guard voids, a window `mixed_concurrency.py` refuses (write mix drift, unbalanced design, a failed `.expect`), or a build whose commit differs between runs, voids the run; it is re-dispatched in the same position and the discard is disclosed.
- A blob 50 % cell with zero compactions across its windows at 16 threads voids the run: the trigger did not fire and the arena bound of §26.7 was not exercised.

### 26.6 Expected losses, before measuring

Single-threaded instructions (the PR's own `instruction-counts` run). The price of an optimistic removal is known from its twin on the same body: on `main` at `d7b1df7c`, `sync_map_remove/random` is 65,649,541 Ir and `sync_blobmap_remove/random` 52,711,517 Ir over 50,000 removals each, 1,313.0 and 1,054.2 Ir per removal, a ratio of 1.245 (CI run [36597494345](https://github.com/orieg/expanse/actions/runs/36597494345), the `instruction-counts` report's comparison against `main`). `sync_blobmap_remove_miss/random` there is 22,191,055 Ir, 443.8 per absent-key removal. The blob removal adds the dead-byte charge and a slot `try_own` on each hit. Ceilings, set as policy; an arm over its ceiling is a stop-and-ask, not an override:

| arm | ceiling |
|---|--:|
| `sync_blobmap_remove/random` | +40 % |
| `sync_blobmap_churn/random` | +25 % |
| `sync_blobmap_insert`, `sync_blobmap_overwrite`, `sync_blobmap_get` | +3 % (inlining only; no source change on their paths) |
| `sync_strmap_*` | +3 % (a new instantiation of the generic `olc_remove_map` can change inlining crate-wide, `sync.rs` `OlcHost::edge_tag` note) |
| `blobmap_remove/random` (plain map) | 0.0 %, a control |
| every other arm | 0.1 % (AGENTS.md §6) |

`sync_blobmap_remove_miss` (#1280's Callgrind arm) is expected to fall; it has no ceiling.

### 26.7 Math-first inputs (`scripts/blob_mixed_bounds.py`, §8.8 commit 1)

- The harness's arena ceiling under its trigger is 191,817,728 B, 5.60× under the cap, above the registered 2× margin.
- At 50 % read the serialised share of operations is 0.25 on B, 0.125 on M and 0 by construction on C, whose fallbacks are measured, not derived.
- Under the utilization law B's throughput at any thread count is at most 1 / (0.25 · s), s the serialised section's duration. s is not derivable and is not registered; no magnitude of G1 is predicted from it, and #1055's 10.97× on the bytes map is not an expectation here.

### 26.8 What each outcome licenses

- G1 and G3 `PASS`: "the optimistic removal raises the blob map's 50/50 throughput at 16 threads by [interval]", with R1 stating how much of it the miss path alone delivers. "Removal was the cause of the retrograde curve" is claimed only if G4 also passes; otherwise the remaining deficit is reported as unexplained (§8.20.4).
- G1 `INCONCLUSIVE` or `REFUTED`: C does not merge as a performance change. The result, the intervals and R1 are recorded on #1280.
- Independently of every gate, B's two runs are the repaired instrument's first measurement and are published as such in README §12, replacing the withheld blob cells.

### 26.9 Not predicted, and out of scope

- No magnitude for any ratio. No USL fit: three thread counts leave no residual for a two-parameter model.
- Out of scope: a `writer_scaling` remove or churn cell (the repaired mixed cell removes a present key on 0.25 of writes, checked per window), compaction under load as its own measurement, the `ablation-blob-*` builds, and a Miri UB-site workload for concurrent blob removals (a follow-up on #1280).

## 27. Pre-registration for #1280 — does in-window compaction set the blob map's four-thread peak? (appended and locked 2026-09-30, before any run of the builds below)

### 27.1 Context

With the optimistic removal (§26, README §26) `SyncExpanseBlobMap` at 50 % read runs 11.9 M total ops/s at one thread, 20.7 M at four and 13.4 M at sixteen: it peaks at four and falls from there, cause unmeasured. Both Expanse blob arms of the `core_concurrency` harness compact their arena inside the window: worker 0 compacts once its own inserts times the thread count reach `BLOB_COMPACT_APPENDS` (`BLOB_POP`, 200k), and a compaction excludes every other writer — the OCC arm's `compact` runs under `write_quiesced`, the `Mutex` arm's under the lock. At the §26 rates that is 18–26 compactions a second. This section tests whether that exclusion is a measurable part of the sixteen-thread cell, by one intervention and one direct measurement.

### 27.2 Builds

| build | what it is | ref |
|---|---|---|
| D | `main` plus the `compact_ns` instrument: worker 0 times each in-window `compact()` and the driver reports `compaction_time_share`, the summed compaction time over the summed window time | the PR head that carries this section |
| K | D with `BLOB_COMPACT_APPENDS = 4 * BLOB_POP`: the same trigger, firing once for every four of D's compactions | `exp/1280-k-compact-4x`, D plus that one constant |

K changes only the two Expanse blob arms. Its arena ceiling is 278,217,728 B, 3.86× under the cap, above the §26 2× margin (`scripts/blob_mixed_bounds.py`, `ABLATION_COMPACT_APPENDS`). Each build's commit is the one its dispatch records; a build whose commit differs between its two runs voids both.

### 27.3 Instrument, runs and order

The `concurrency` suite of `bench_baremetal.yml`, unchanged (`--threads 1,4,16 --workloads 100,50 --engines all`, 18 rounds per cell, pin `0-15`, host guard as on `main`). Two runs per build, dispatched in the order D, K, K, D; run k of each build pairs with run k of the other.

### 27.4 The gate, and what is reported

K ÷ D is the ratio of mean total ops/s over a cell's 18 windows, two-sample BCa 95 %, computed by `docs/benchmarks/concurrency/scripts/blob_compaction_gate.py`, which holds these cells as constants.

| id | comparison | cell | floor | reading |
|---|---|---|--:|---|
| H1 | K ÷ D | `SyncExpanseBlobMap`, 50 % read, 16 threads | 1.0 | `PASS` when the lower bound exceeds the floor in both runs, `REFUTED` when the upper bound is below it in both, `INCONCLUSIVE` otherwise |

Reported with their intervals, not gated: K ÷ D for the blob arm at 4 and 1 threads and for `Mutex<ExpanseBlobMap>` at 16 and 4 threads; and `compaction_time_share` of both builds for the blob arm at 1, 4 and 16 threads and for `Mutex<ExpanseBlobMap>` at 16.

### 27.5 Controls

Cells the trigger does not reach, read as K ÷ D: `blob` 100 % read at 16 threads, `map` 50 % read at 16, `str` 50 % read at 16. A control whose interval excludes [0.95, 1.05] in the same direction in both runs marks the comparison `DRIFT` (`docs/BENCHMARKING.md` rule 18) and H1 is not read.

### 27.6 What voids a run

A run the host guard voids, a window `mixed_concurrency.py` refuses, a build whose commit differs between its runs, or a blob 50 % cell at 16 threads with no compaction in any window. A voided run is re-dispatched in the same position and the discard is disclosed.

### 27.7 What each outcome licenses

- H1 `PASS`: "in-window compaction is a measurable part of the blob map's sixteen-thread cell on this harness", with the ratio and D's `compaction_time_share` beside it. It licenses nothing about the engine without compaction; how much of the four-to-sixteen fall remains in K is reported and, where it remains, called unexplained.
- H1 `REFUTED` or `INCONCLUSIVE`: compaction at this trigger is not shown to set the sixteen-thread cell; the fall stays unexplained, and #1290's priority is not raised by this measurement.
- Either way the harness keeps its registered trigger (`BLOB_COMPACT_APPENDS = BLOB_POP`); K is a diagnostic build and never merges. The `compact_ns` instrument merges with this section.

### 27.8 Not predicted, and out of scope

- No magnitude for any ratio or share. The share is a wall-clock sum over one worker's compactions; it bounds the time no write could proceed only where every other writer was excluded for it, which both arms' compaction does by construction (above).
- Out of scope: the engine's own reclamation (#1290), the `enter_writer` hot/cold split, other payload sizes, and changing the published harness's trigger.

## 28. Pre-registration for #1290 — the blob arena reclaims itself at the cap (appended and locked 2026-09-30, before any code or run of the builds below)

### 28.1 Context

An `ExpanseBlobMap` or `SyncExpanseBlobMap` arena never rewrites a record in place, and only the caller's `compact()` reclaims dead records. Once the chunk bytes reach `max_capacity`, every insert that needs a new chunk fails with `ArenaError::OffsetOverflow`, however much of the arena is dead (#1290). The #1280 harness hit this and timed failing inserts until #1288 gave it an in-window compaction policy.

### 28.2 The change

On a refused growth, an insert compacts the arena once and retries once. It does so only if the live bytes, which the compaction would copy, are at most `RECLAIM_COPY_PER_GROWTH` (1) times the bytes freeable since the previous compaction: the chunk bytes grown plus the net drop in live bytes. Otherwise it fails as today. `docs/design/large-values.md` §6.3.1 states the rule, and `scripts/blob_reclaim_bounds.py` bounds it:

- an overwrite-forever workload of 128-byte payloads is sustained at the default cap up to 3,830,069 live records, 0.514 of the records the arena can hold;
- no workload triggers two compactions without inserting or removing in between.

The branch sits on the insert's error path. The `core_concurrency` blob arms never reach it: their arena high-water mark is 191,817,728 B, 5.60× under the cap (`scripts/blob_mixed_bounds.py`).

### 28.3 G1 — correctness, deterministic

Tests in the change, each shown red by a mutation that disables the rule or its removal term:

| id | map | workload (chunk 4,096 B, cap 64 KiB, `hot_meta = 1` so every payload is an arena record) | expected |
|---|---|---|---|
| G1.1 | `ExpanseBlobMap` | 224 live 128 B records, 1,792 overwrites | 0 errors, 7 compactions |
| G1.2 | `ExpanseBlobMap` | 225 live 128 B records, 1,792 overwrites | 1,346 errors, 1 compaction |
| G1.3 | `ExpanseBlobMap` | 1,280 live 9 B records, 8,192 overwrites | 0 errors, 10 compactions |
| G1.4 | `ExpanseBlobMap` | 1,281 live 9 B records, 8,192 overwrites | 6,658 errors, 1 compaction |
| G1.5 | both | distinct 128 B keys until an insert fails, one more insert, remove half, insert | the fill stops at 448 keys after 1 compaction; the next insert fails with no compaction; the insert after the removals succeeds after a second |
| G1.6 | `SyncExpanseBlobMap`, one thread | 210 live 128 B and 1,204 live 9 B records, overwrites as G1.1 and G1.3 | 0 errors |
| G1.7 | `SyncExpanseBlobMap`, four writers, cap 256 KiB | 400 live 128 B records, 100 per writer, each writer overwriting its own keys 2,000 times | 0 errors; every key reads its last payload; the live bytes equal the sum over the index |

- G1.1–G1.5's expected counts are `simulate` and `simulate_fill_then_remove` in `scripts/blob_reclaim_bounds.py`. The engine must match the model exactly.
- G1.6's live sets are `max_sustained_live_records` with the cap reduced by one chunk. After a compaction, `SyncExpanseBlobMap`'s writer takes a fresh private chunk instead of filling the tail of the last compacted one, so each cycle can lose up to one chunk.
- G1.7's live set is under half of `max_sustained_live_records(128, 4096, 256 KiB − 8 chunks)` = 812, which covers each writer holding and abandoning a private chunk per cycle.
- G1.6 also runs under Miri in the Tier-1 filter.

### 28.4 G2 — single-threaded cost, deterministic

Every arm of `instruction-counts` and `callgrind-smoke` within the §6 review threshold (0.1 %) of `main`, the `sync_blobmap_*` and `blobmap_*` arms included. No arm reaches the cap, so the branch never executes; a change in any arm is inlining or layout. An arm over the threshold is a stop-and-ask, not an `allow-regression:` line.

### 28.5 G3 — the published blob cells, wall clock

The change touches the insert path that §12, §26 and §27 measured, so those figures are stale for the merged code until re-measured (AGENTS.md §8.7).

- **Builds and order:** B, the change's merge base, and R, the change's head. The `concurrency` suite of `bench_baremetal.yml`, unchanged from §27.3, runs twice per build in the order B, R, R, B; run k of each build pairs with run k of the other.
- **Evaluator:** R ÷ B is the ratio of mean total ops/s over a cell's 18 windows, two-sample BCa 95 %. The evaluator is `docs/benchmarks/concurrency/scripts/blob_reclaim_gate.py`, committed with the data. It reuses `blob_remove_gate.py`'s ratio, verdict and drift rules.

| id | comparison | cell | floor | reading |
|---|---|---|--:|---|
| G3 | R ÷ B | `SyncExpanseBlobMap`, 50 % read, 16 threads | 0.95 | `PASS` (non-inferior) when the lower bound exceeds the floor in both runs, `REFUTED` when the upper bound is below it in both, `INCONCLUSIVE` otherwise |

- **Reported with intervals, not gated:** R ÷ B for the blob arm at 50 % read with 4 and 1 threads and at 100 % read with 16; and for `Mutex<ExpanseBlobMap>`, whose plain `insert` also changes, at 50 % read with 16 and 4 threads.
- **Controls:** `map` 50 % read at 16 threads and `str` 50 % read at 16 threads, read as in §27.5. A control outside [0.95, 1.05] in the same direction in both runs marks the comparison `DRIFT`, and G3 is not read.
- **Voids:** as §27.6, without the compaction clause.

### 28.6 What each outcome licenses

- **G1 and G2 met:** the change is eligible to merge after G3.
- **G3 `PASS`:** the published blob cells stand for the merged code, with R's runs beside them as the re-measurement. It licenses "not slower by more than 5 % on these cells", never "faster".
- **G3 `REFUTED` or `INCONCLUSIVE`, or any G2 arm over its threshold:** stop and ask, and do not merge on this pre-registration.

### 28.7 Not predicted, and out of scope

- No ratio is predicted beyond the non-inferiority floor.
- The latency of an insert that triggers a compaction is not measured. Its copy is bounded by the live bytes (`stall_copy_bytes`: 25.9 MiB for the #1280 harness's live set, at most 512 MiB at the default cap), and during it `SyncExpanseBlobMap` excludes every writer, as a caller's `compact()` does.
- Out of scope:
  - incremental or per-chunk compaction;
  - an error that reports the dead share;
  - `ExpanseBlobMap32` (a fixed 4,096-slot arena);
  - any change to the harness's own trigger;
  - a different `RECLAIM_COPY_PER_GROWTH`.

## 28a. Amendment to §28 — the waste guard, a fast refusal, and corrected bounds (appended and locked 2026-09-30, before any code of the amended build; §28 above is not edited)

### 28a.1 Why §28 is amended

§28's gates were met (README §28). A review of the change with four synthetic reviewers, read as adversarial brainstorming rather than as peer review, raised the points below. Each was then checked against the code at `3d4b46a6`.

- **§28.7's stall bound is wrong.**
  - It says an automatic compaction copies "at most 512 MiB at the default cap". That holds only in the steady overwrite cycle.
  - After construction or `clear` the rule's baseline is zero, so the first refusal compacts whatever the dead share is. The copy is then bounded only by the live bytes: about 1 GiB at the cap, and 967 MiB for 128 B records filled to the cap (`stall_copy_bytes(7,456,256, 128)`).
  - G1.5 pinned that first compaction, and it frees nothing (`simulate_fill_then_remove`, `compactions_at_fill: 1`).
- **New reads wait for the compaction.**
  - On `SyncExpanseBlobMap`, a compaction runs under `write_quiesced`. That opens the tree `SeqVersion` bracket for the whole closure (`write_root_covered_with::<true, true>`, sync.rs:2777 and 2790), and `SeqVersion::sample` spins while the bracket is odd (occ.rs:132–157).
  - Only borrows taken before the bracket opened keep reading the retired chunks.
  - §28.2 says only that every writer waits, so it is incomplete. This holds for a caller's `compact()` on `main` too.
- **A refusal the rule declines now quiesces the map.** Every `OffsetOverflow` from `SyncExpanseBlobMap::insert_once` enters `write_quiesced` before the rule is read. On a full map, callers that retry turn each refused insert into a writer drain plus a reader bracket; before §28 the refusal returned without either.
- **The memory peak is not bounded by the cap.**
  - `compact_with_index` holds the old and new chunk sets at once, plus two vectors of 16 B per arena record. `live_entries` grows by doubling, with no size hint.
  - A failed vector allocation aborts the process inside an `insert`.
  - On `SyncExpanseBlobMap` the old set stays allocated until the epoch collector frees it.
- **Documentation contradicts the behaviour.**
  - `with_chunk_size_and_max_capacity` on `BlobArena`, `ExpanseBlobMap` and `SyncExpanseBlobMap` (blobmap.rs:792, 1403; sync.rs:9851) says an insert at the ceiling "fails atomically and leaves … completely unmodified". Under the rule, an insert can compact and still fail.
  - A compaction that cannot allocate makes the insert return `AllocationFailed`.
  - `expanse.h`, the man page and COMPAT.md do not say that an insert can relocate and free every payload.

### 28a.2 The amended build (R′), one change per item

- **C1 — the waste guard.**
  - `BlobArena::reclaim_allowed` also requires `2 * live_bytes < total_allocated`.
  - An arena filled with live records never passes it, so a bulk load that reaches the cap fails with no copy.
  - It bounds every automatic copy below half the cap (`max_stall_bytes`: 512 MiB − 1 B at the default cap).
  - The budget (A) is unchanged. `scripts/blob_reclaim_bounds.py` models both conditions, and the sustained live sets and every G1.1–G1.4, G1.6 and G1.7 count are unchanged: (A) binds in each, as the script's self-test pins.
- **C2 — a fast refusal on the concurrent map.**
  - When the quiesced decision declines, `SyncExpanseBlobMap` records the population at that point.
  - A later refused insert fails with `OffsetOverflow` without quiescing, while the population is at least the recorded one. Every `RECLAIM_RECHECK_EVERY` (64) such refusals it re-checks under quiescence instead, so a live-byte drop that removes no key (an overwrite with a shorter payload) is seen.
  - `compact`, `clear`, a successful reclaim and the policy setter (C5) clear the record.
  - The hot path is unchanged: the record is read only after a refusal.
- **C3 — one home for the wrapper's compaction invariant.** "Fold the writers' deltas before a compaction, reset their private chunks after it" becomes one private function, which `SyncExpanseBlobMap::compact` and the insert's reclaim both call.
- **C4 — a bounded, fallible relocation list.**
  - `compact_with_index` collects into one vector, reserved once with `try_reserve_exact` for the index's entry count, and rewrites it in place. A failed reservation returns `AllocationFailed` before anything is changed.
  - The transient peak becomes the old chunk set, plus the new chunk set, plus 16 B per index entry (`compaction_peak_bytes`).
- **C5 — a Rust opt-out.**
  - `ExpanseBlobMap::set_reclaim_at_cap(bool)` and `SyncExpanseBlobMap::set_reclaim_at_cap(bool)`, with getters. The default is on.
  - Off restores the pre-#1290 behaviour at the cap: `OffsetOverflow` with no compaction.
  - The C API and the bindings keep the default. Exposing the switch and the cap there is out of scope (28a.6).
- **C6 — documentation stating what is true.**
  - The three `with_chunk_size_and_max_capacity` docs and `insert`'s docs: the map's contents are unchanged on failure, its layout may not be, and `AllocationFailed` can be returned.
  - `MAX_ARENA_CAPACITY`: it bounds allocated chunk bytes, not peak memory.
  - `docs/design/large-values.md` §6.3.1: the rule with (B); the stall is under half the cap; new reads wait for the compaction; the peak; the fast refusal; the opt-out.
  - `include/expanse.h`, `man/man3/expanse_blob_map.3` and `docs/COMPAT.md`: an insert may relocate and free every arena payload.

### 28a.3 G1a — correctness, deterministic

§28.3's G1.1–G1.4, G1.6 and G1.7 keep their workloads and expected counts. The amended model gives the same numbers. Replaced or added:

| id | map | workload (as §28.3 where not stated) | expected |
|---|---|---|---|
| G1.5a | both | distinct 128 B keys until an insert fails, one more insert, remove half, insert | the fill stops at 448 keys with **no** compaction; the next insert fails with no compaction; the insert after the removals succeeds after **one** compaction (`simulate_fill_then_remove`) |
| G1.8 | both | G1.1's workload with `set_reclaim_at_cap(false)` | refusals begin at the first refused chunk; the arena generation never changes |
| G1.9 | `SyncExpanseBlobMap`, four writers | a full 64 KiB arena (the G1.5a fill); each writer inserts a fresh key 1,000 times | every insert returns `OffsetOverflow`; the number of quiesced reclaim decisions, counted by a test-only counter, is at most 4,000 / 64 + 4; after one writer removes half the keys, the next insert succeeds |
| G1.10 | `ExpanseBlobMap` | a map with 1,000 inline keys and 10 arena keys, compacted | the relocation list is reserved for 1,010 entries and the compaction succeeds; the counts are read through a test-only accessor |

Each new or changed test is shown red by a mutation of the clause it pins: C1's guard removed, C2's record ignored, C2's periodic re-check removed, C5's flag ignored.

### 28a.4 G2a — single-threaded cost, deterministic

As §28.4: every arm of `instruction-counts` and `callgrind-smoke` within 0.1 % of `main`, and a stop-and-ask above.

### 28a.5 G3 — carried over, or re-run

- **Carried over if:** on the CI Callgrind run of R′, every `blobmap_*` and `sync_blobmap_*` arm is within 0.1 % of its count on R (`d50f4f05`, [run 36681492675](https://github.com/orieg/expanse/actions/runs/36681492675)). Then §28's G3 reading (README §28) stands for R′: the amendment changed only paths those cells never execute.
- **Otherwise:** G3 is re-run as §28.5 with the same B (`79184f25`) and R′, in the order B, R′, R′, B, against the same floor.

### 28a.6 Not predicted, and out of scope

- No latency figure.
  - The stall's worst case is bounded by construction (C1) but unmeasured.
  - An estimate derived from §27's committed runs, about 11 ms per compaction of the harness's 200,000 records, is an extrapolation, not a measurement, and is not published as one.
- Follow-ups, each needing its own registration:
  - latency histograms of inserts and reads across a triggered reclaim, and peak memory, on the reference host;
  - running phase 1 of a concurrent compaction outside the reader bracket;
  - a distinct error, or error context, separating "full" from "fragmented";
  - exposing the cap and the switch in `expanse.h` and the bindings, after splitting the cap's image-validation role from its runtime budget;
  - incremental compaction. It needs per-chunk live counters and a key in the record header, which is a format change under AGENTS.md §2.3.

## 29. Pre-registration for #1300 item 1 — the stall and the memory peak of reclamation at the cap (appended and locked 2026-10-01, before any code or run of the harness below)

### 29.1 Context

#1296 made an insert that the arena's capacity cap refuses a chunk compact the arena under the reclaim rule (`docs/design/large-values.md` §6.3.1). The rule bounds the copy by construction: it stays under half the cap. The copy's duration has not been measured, and neither has the memory a compaction holds. §28 and §28a measured cells that never reach the cap.

The §27 runs give a per-record cost: 53.07–58.80 ns per live record across 12 cells (`s27_ns_per_record` in `scripts/blob_reclaim_bounds.py`). Linear in the live records, that predicts a stall of 10.61–11.76 ms at 200,000 records and 191.1–211.7 ms at 3,600,000 (`predicted_stall_ns`). A review of #1296 expected large live sets to cost more per record, because the copy and the index re-descent leave the last-level cache. This section measures both.

### 29.2 The harness

`crates/expanse/benches/reclaim_stall.rs`, run by `docs/benchmarks/concurrency/scripts/reclaim_stall.py` as the `reclaim_stall` suite of `bench_baremetal.yml`.

- **One process per (cell, round).** Cells run in an order rotated per round. Each process prints one JSON row.
- **Payloads:** 128 B with `hot_meta = 1`, so every payload is an arena record. The arena uses the default 2 MiB chunks and 1 GiB cap.
- **Keys:** the live set's keys are `0..L`.
- **Timing:** per operation, with `Instant::now()` around the call, recorded in an HdrHistogram (3 significant figures).
- **Memory:** a counting global allocator records the process's live heap and its peak.

**Overwrite cells.** The map is prefilled with `L` live records. Then `W` writers overwrite uniformly drawn live keys until the arena has completed 2.2 append cycles. A cycle is `max_chunks × records_per_chunk − L` appends: 7,256,256 at `L = 200,000` and 3,856,256 at 3,600,000. That is enough for two automatic compactions. The compactions are counted once, at the end of the window, from the arena generation, so no observer runs inside it. On `SyncExpanseBlobMap`, `R = 2` reader threads read uniformly drawn live keys through their own reader handles for the whole window.

| map | W | R | L |
|---|---|---|---|
| `ExpanseBlobMap` | 1 | 0 | 200,000 and 3,600,000 |
| `SyncExpanseBlobMap` | 1, 4, 12 | 2 | 200,000 and 3,600,000 |

**Full-map cells.** A `SyncExpanseBlobMap` is filled with distinct keys until an insert is refused. The arena is then more than half live, so the waste guard declines every compaction. `W ∈ {4, 12}` writers then insert fresh keys for 3 s while `R = 2` readers read. Every insert is expected to be refused.

**Recorded per row:**
- the insert-latency and read-latency percentiles (p50, p99, p99.9, max);
- the 8 largest insert latencies;
- the compactions in the window;
- the index's `mem_used`, the arena's `total_allocated`, and the peak and final live heap;
- in full-map cells, the refused-insert count and rate.

**Rounds and host.** 5 rounds per cell, on the reference host, with the bare-metal suite's pin (`0-15`), host guard and load windows. An artifact carries the §8.17 provenance that `check_bench_provenance.py` requires.

### 29.3 Predictions

The stall of a window is its largest insert latency. A window holds two automatic compactions, each paid by the insert that triggered it, and no other source of a stall that size is expected.

| id | quantity | prediction | read |
|---|---|---|---|
| P1 | the stall, per overwrite cell, median over rounds | within the linear range of 29.1: 10.61–11.76 ms at 200,000, 191.1–211.7 ms at 3,600,000 | reported against the range, with the BCa interval over rounds |
| P2 | the per-record stall ratio, (stall ÷ L at 3,600,000) ÷ (stall ÷ L at 200,000), per map and W | linear: 1.0; the review's cache-bound alternative: above 1.25 | `LINEAR` when the BCa interval lies in [0.8, 1.25], `SUPERLINEAR` when its lower bound exceeds 1.25, `INCONCLUSIVE` otherwise |
| P3 | on `SyncExpanseBlobMap` overwrite cells, the largest read latency ÷ the stall | readers wait for the whole compaction (the tree bracket is open): at least 0.5 | `HOLDS` when it is at least 0.5 in every round, `FAILS` when it is below 0.5 in every round |
| P4 | on `ExpanseBlobMap` overwrite cells, the peak live heap ÷ (index `mem_used` + `compaction_peak_bytes`) | within [0.9, 1.1] | per round; `SyncExpanseBlobMap` cells reported, since retired chunks wait for the epoch collector |
| P5 | on full-map cells, the refused-insert p99 latency | at most 100 µs at W = 4 and 12 (the fast refusal, §28a C2) | `HOLDS` when it holds in every round |

### 29.4 What each outcome licenses

- **P1 at 3,600,000 — a median stall of at least 100 ms in any cell:** the stall is material at sizes the rule sustains. #1300's reader-bracket split and incremental compaction move ahead of its other items. Below 100 ms in every cell, they stay behind them.
- **P2 `SUPERLINEAR`:** the linear prediction understates large live sets. The docs' stall statement gains the measured per-record cost.
- **P3 `FAILS`:** the docs' statement that new reads wait for the compaction is wrong and is corrected.
- **P4 outside [0.9, 1.1]:** `compaction_peak_bytes` misstates the peak and is corrected, with the measured term.
- **P5 not `HOLDS`:** the fast refusal does not hold under load, and #1300 records it.

No prediction is a merge gate: the harness changes no engine code.

### 29.5 Not predicted, and out of scope

- **Not predicted:**
  - the latency of reads that started before the compaction (they keep reading retired chunks);
  - the frequency droop a long copy may cause;
  - payload sizes other than 128 B.
- **Out of scope:** any engine change. A measurement that motivates one gets its own pre-registration.

## 30. Pre-registration for #1300 item 2 — new reads proceed during a concurrent compaction (appended and locked 2026-10-01, before any engine code of the change below)

### 30.1 Context

README §29.1 timed each phase of a compaction on `SyncExpanseBlobMap`. The tree bracket is open for the whole exclusive section of `write_quiesced`, so every read that starts in it waits. The collect (`index.iter()` into the rewrite list) and the copy (phase 1) are 89.4–91.0 % of that section. Neither writes anything a reader loads: phase 1 copies into a new arena no reader can reach. Only the index rewrite (phase 2) and `republish_table` change what readers validate against.

`reader_wait_ratio_bound` (`scripts/blob_reclaim_bounds.py`) computes the wait left if the bracket covers only those two steps:
- 9.93–10.45 % of the section at 200,000 live records (1.48–1.59 ms);
- 9.00–9.14 % at 3,600,000 (25.6–26.5 ms).

### 30.2 The change

- **Split `BlobArena::compact_with_index` (`crates/expanse/src/blobmap.rs`) in two.**
  - A prepare step takes `&self` and `&ExpanseMap`. It runs the collect and phase 1 into a private arena.
  - An apply step takes `&mut self` and `&mut ExpanseMap`. It runs phase 2, installs the chunk set, republishes the table and disposes of the old chunks.
  - `compact_with_index` calls the two in sequence, so `ExpanseBlobMap` does the same work in the same order.
- **Add a serialised section to `Shared` (`crates/expanse/src/sync.rs`) that runs a prepare closure before opening the tree bracket.** It takes the same exclusion as `write_root_covered_with`: the fallback mutex, the writer gate drained and the writer mutex. It runs the prepare closure on a shared reference to the engine, then opens the bracket for the apply closure.
- **Route both compaction paths of the default build through it:**
  - `SyncExpanseBlobMap::compact`;
  - the reclaim in `insert_after_reclaim`. There, the reclaim decision and the prepare run before the bracket; the apply and the retried insert run inside it.
- **Unchanged:**
  - the writers' arena fold before the compaction and the private-chunk reset after it (`with_folded_writers`);
  - the `ablation-blob-*` paths;
  - the record format;
  - every non-compaction path.

**Hazards, each with its check:**

| id | hazard | check |
|---|---|---|
| H1 | the prepare writes a word a reader loads | the prepare's signature is `&self` / `&ExpanseMap` (compile time), and a Tier-1 Miri test runs a reader through a compaction |
| H2 | a read that starts during the prepare returns a wrong payload or reports a present key absent | a test reads every live key concurrently with repeated compactions and asserts each payload |
| H3 | a failed prepare leaves the map changed, or the bracket open | the existing all-or-nothing tests, run against the split, and a failure-injection test through the concurrent path |
| H4 | the single-threaded map pays for the split | the Callgrind arms `blobmap_compact` and `sync_blobmap_compact`, landed before the change |

No tree mutation entry point is added. Phase 2 stores slots through the existing atomic store, so §2.1 invariant 5's `by_mode!` twin does not apply.

### 30.3 Instrument and protocol

- **Suite:** `reclaim_stall` (§29) on the reference host, unchanged.
- **Builds:** B, the change's merge base, and H, its head.
- **Runs:** dispatched in the order B, H, H, B, 5 rounds per cell each.
- **Admissibility:** each run's host guard reads quiet, and every round completes two compactions with no refused insert.
- **Callgrind:** the PR's `instruction-counts` job.

### 30.4 Predictions and gate

| id | quantity | prediction | verdict |
|---|---|---|---|
| G1 | at 3,600,000 live, per `SyncExpanseBlobMap` overwrite cell (W = 1, 4, 12), the largest read latency | H's BCa 95 % upper bound ≤ 0.15 × B's BCa 95 % lower bound | `PASS` when it holds in all three cells in both run pairs; `FAIL` when it fails in any cell in both; `INCONCLUSIVE` otherwise |
| P2 | at 200,000 live, the same quantity | H ≤ 0.25 × B, by the same interval rule | reported |
| P3 | the stall (largest insert latency) at 3,600,000 | H ÷ B medians within [0.9, 1.1]: the writers' wait is not the target | reported |
| P4 | `blobmap_compact` instructions | at most +0.1 % against B | review bound (§6) |
| P5 | every other Callgrind arm | within the §6 review bound | review bound |

**Why the thresholds sit above the bound.** G1's 0.15 is the bound's 0.0914 plus slack for a read already in flight when the bracket opens, and for scheduler noise. P2's 0.25 is wider because at 200,000 live the bound is 1.5 ms, near the size of other read tails the base run has not attributed.

**Gate.**
- G1 `PASS`, with the test suite, Tier-1 Miri and the H2–H3 tests green: the change merges.
- G1 `FAIL`: it is reverted, and the measured outcome is recorded on #1300.
- G1 `INCONCLUSIVE`: one more run pair, under the same rule; no threshold moves (§8.19).

### 30.5 Not predicted, and out of scope

- **Not predicted:**
  - the read latency of pinned views opened before the compaction;
  - the collector's free of the old chunk set, which lands on a later writer (README §29.1).
- **Out of scope:**
  - the writers' wait, which is item 5;
  - the free's placement, which gets its own record on #1300.

## 31. Pre-registration for #1320 — incremental blob arena compaction and bounded per-insert evacuation (appended 2026-10-03, locked before any engine code of the change below; next free section, may be renumbered at merge)

### 31.1 Context

Compaction at the arena cap is currently $O(\text{live records})$ executed synchronously inside a single triggering insert. On `SyncExpanseBlobMap` (W = 1, 4, 12) and `ExpanseBlobMap`, the largest insert latency measured at 3,600,000 live 128 B records is 302.5–320.1 ms (README §29–§30, `results/gate_1300_split_h_run{1,2}.json`).

*Appended 2026-10-20; the text of §31 is not rewritten.* The W = 4 and 12 part of this range, here and in §31.2 and §31.5, is withheld pending re-measurement (#1320): writer pairs on identical key streams, README §29's † note; the W = 1 and `ExpanseBlobMap` part is unaffected.

Where the time goes (README §29.1, `results/step0_1300_phases/probe.log`):
- payload copy (phase 1): 217.0–231.3 ms (57.3–64.2 ns per live record);
- index collect: 34.2–36.4 ms;
- slot rewrite: 24.8–26.5 ms.

While #1318 split the reader bracket so readers no longer wait for phase 1, writers still wait for all of it. To eliminate this stall, compaction transitions from a whole-arena bulk copy to bounded incremental evacuation (log-structured segment cleaning; Rosenblum & Ousterhout 1992 §4; Baker 1978; Cormen et al. §17.4):
1. Per-chunk live counters, to select candidate victim chunks by highest dead share.
2. An 8-byte key stored directly in each record header (`BlobRecordHeader { key: u64, len: u32, generation: u32 }`). When records in a victim chunk are relocated, the key enables compactor reverse lookup from evacuated record to index slot and direct rewrite without scanning the entire index ($O(1)$ per evacuated record vs $O(\text{index})$ whole-index traversal).
3. A partial evacuation reclaim rule budgeting bounded copies per insert.

### 31.2 Mathematical derivations and bounds (`scripts/blob_reclaim_bounds.py`)

All quantities and thresholds below are computed by pure functions in `scripts/blob_reclaim_bounds.py` (Rule 12 / §8.8 commit 1; no hand arithmetic):

1. **Header layout and record needed:**
   - Baseline: 8-byte header (`len: u32`, `generation: u32`). `record_needed(128)` = 136 B.
   - Keyed header: 16-byte header (`key: u64`, `len: u32`, `generation: u32`; `KEYED_RECORD_HEADER_BYTES = 16`). `keyed_record_needed(128)` = 144 B.
   - Stride: records are aligned to 16 bytes (`ARENA_ALIGN = 16`). `keyed_record_stride(128)` = 144 B (identical to baseline stride of 144 B).

2. **Chunk density and records per chunk:**
   - At default chunk size (2 MiB, 2,097,152 B):
     - `records_per_chunk(128, 2 MiB)` = 14,563.
     - `keyed_records_per_chunk(128, 2 MiB)` = 14,563.
     - Because 128 B payloads with 8 B headers required 136 B and padded to 144 B, the additional 8-byte key fits within the existing 8-byte alignment tail. Records per chunk is strictly preserved for 128 B payloads.

3. **Sustained-live bound:**
   - With 8 B headers: `max_sustained_live_records(128, 2 MiB, 1 GiB)` = 3,830,069 records (fills 263 chunks).
   - With 16 B headers: `keyed_max_sustained_live_records(128, 2 MiB, 1 GiB)` = 3,728,128 records (fills exactly 256 chunks: $3,728,128 \times 144 = 536,850,432 \le 256 \times 2\text{ MiB} = 536,870,912$).
   - Condition (B) waste guard requires $2 \times L \times 144 < 2^{30} \implies L \le 3,728,270$, so condition (A) binds at 3,728,128.

4. **Density cost:**
   - At 128 B payload, 2 MiB chunks and 1 GiB cap:
     - `density_cost_records(128, 2 MiB, 1 GiB)` = 101,941 records.
     - `density_cost_ratio(128, 2 MiB, 1 GiB)` = 2.66% ($101,941 / 3,830,069 \approx 0.026616$).
   - For small unpadded payloads (e.g. 8 B), where records packed without padding (16 B vs 32 B stride), density loss is 42.97% (`density_cost_ratio(8, 2 MiB, 1 GiB)` = 0.4297).

5. **Per-insert copy bound (chunk-wise victim evacuation on refusal):**
   - Under the waste guard (`blobmap.rs:1463`: `self.live_bytes.saturating_mul(2) < self.total_allocated`), the arena's overall live fraction is strictly below 0.5 ($u_{\text{mean}} < 0.5$).
   - Because candidate victim chunks are selected by lowest live utilisation (highest dead share), the victim chunk's live fraction satisfies $u_{\text{victim}} \le u_{\text{mean}} < 0.5$.
   - A single refusal therefore evacuates at most:
     `victim_chunk_max_live_records(128, 2 MiB)` = $\lfloor 14,563 / 2 \rfloor = 7,281$ records.
     `victim_chunk_max_live_bytes(128, 2 MiB)` = $7,281 \times 144 = 1,048,464$ B ($\le 1\text{ MiB}$; `per_insert_copy_bound(128, 2 MiB, 1 GiB)` = 1,048,464 B).
   - The continuous 1-record / 144 B scheme is out of scope for this design; chunk-wise evacuation on refusal is the single operational rule.

6. **Writer-stall projection and target gate ceiling:**
   - Evacuating at most 7,281 live records:
     - Payload copy stall: `predicted_incremental_stall_ns(128, 2 MiB)` = 467,757 ns $\approx 0.468\text{ ms}$ `(projected)` (derived via `step0_max_phase1_ns_per_record()` = 64.244 ns/record from committed artifact `STEP0_PROBE_LOG`, `docs/benchmarks/concurrency/results/step0_1300_phases/probe.log`; cf. README §29.1, `docs/benchmarks/concurrency/README.md:5674–5675`).
     - Slot rewrite component: each evacuated record's slot is rewritten via an $O(k)$ trie descent ($k \le 8$ levels) using its header key (eliminating the $O(\text{index population})$ full-index scan). At $\sim 50\text{ ns}$ per descent, rewriting 7,281 slots adds $\sim 364\text{ µs}$.
     - Combined projected stall: $\approx 0.832\text{ ms}$ `(projected)`.
   - Gate ceiling: largest insert latency $\le 5.0\text{ ms}$ `(target)` — maintainer policy providing a $>60\times$ reduction from the 302.5–320.1 ms whole-arena baseline and a $>6\times$ operational margin over the 0.832 ms projected stall to absorb tail dispersion across writer counts.

### 31.3 The change

- **Format change (`crates/expanse/src/blobmap.rs`):**
  - Extend `BlobRecordHeader` from 8 bytes to 16 bytes:
    ```rust
    #[repr(C)]
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct BlobRecordHeader {
        pub key: u64,
        pub len: u32,
        pub generation: u32,
    }
    ```
    Under `#[repr(C)]`, `size_of::<BlobRecordHeader>()` is exactly 16 bytes and `align_of` is 8 bytes with zero interior or trailing padding. Because every record allocation is aligned to `ARENA_ALIGN = 16` (`blobmap.rs:518`, `ArenaChunk::alloc` at `:406`), every record offset is a multiple of 16, guaranteeing aligned pointer dereferences for `base.cast::<BlobRecordHeader>()` without `read_unaligned` or packed UB.
  - Update `read_record` to validate 16-byte header bounds and offset payload pointers by 16 bytes.
- **Binary image format version bump:**
  - `EXPANSE_FORMAT_VERSION` will bump from `2` to `3`. Older version 2 images will be rejected explicitly with `ArenaError::UnsupportedFormatVersion { found: 2, supported: 3 }` on load (`from_bytes_slice` at `blobmap.rs:2356`, `load_from_file` at `:2530`).
- **Per-chunk live accounting:**
  - Add per-chunk live byte/record counters to `BlobChunk` / `BlobArena` to identify candidate victim chunks by highest dead share.
- **Partial evacuation path:**
  - On refused allocation at the cap, select the victim chunk with highest dead share ($u < 0.5$), copy its live records into the active chunk, update each relocated record's index slot directly via $O(k)$ trie descent on its header key, and retire the victim chunk to EBR (or reuse in plain mode).
- **Instruments:**
  - Callgrind arms `blobmap_insert_reclaiming` and `sync_blobmap_insert_reclaiming` (N = 1,000 ops, exactly 10 reclaims asserted inside each arm) added to `benches/instructions.rs` and registered in `scripts/perf_report.py`.

**Hazards, each with its check:**

| id | hazard | check |
|---|---|---|
| H1 | Header format change corrupts OCC readers | `BlobReadGuard::get` (struct at `sync.rs:11249`) validates seqlock bracket via `shared.version().validate(snap)` (`:11327`); `read_record` validates 16-byte header bounds and `header.generation != generation` (`blobmap.rs:95`); generation mismatch or failed validation triggers retry loop / fallback, never returning `None` for a present key; old chunks remain EBR-live and immutable; Miri test runs concurrent reads through partial evacuation |
| H2 | Older binary image format loaded without validation | `load_from_file` / `from_bytes_slice` explicitly returns `ArenaError::UnsupportedFormatVersion { found, supported: 3 }` |
| H3 | Stale index slot points to reclaimed chunk | Atomic slot store updates locator to new chunk before victim chunk is retired |
| H4 | Non-compaction blob operations pay for header or counters | CI Callgrind arms checked against pre-data derived ceilings (G2) |
| H5 | Corrupted metadata during partial evacuation | `hot_meta` is preserved in slot writeback; `scan_filtered` differential test passes |

### 31.4 Instrument and protocol

- **Suite:** `reclaim_stall` (§29) on the reference host, 5 rounds per cell, W ∈ {1, 4, 12}, 3,600,000 live 128 B records.
- **Builds:** B (merge base `7ca19dc7`) and H (head with incremental compaction).
- **Runs:** dispatched in order B, H, H, B, 5 rounds per cell each.
- **Callgrind:** CI `instruction-counts` job on PR, evaluating `instructions.rs`.

### 31.5 Predictions and gate

| id | quantity | prediction | verdict |
|---|---|---|---|
| G1 | at 3,600,000 live, per `SyncExpanseBlobMap` (W = 1, 4, 12) and `ExpanseBlobMap` overwrite cell, largest insert latency | H's BCa 95 % upper bound $\le 5.0\text{ ms}$ `(target)` (predicted stall 0.832 ms `(projected)`: 0.468 ms copy + 0.364 ms index descent; vs B's 302.5–320.1 ms) | `PASS` when it holds in all cells in both run pairs; `FAIL` if $> 5.0\text{ ms}$; `INCONCLUSIVE` otherwise |
| G2 | non-compaction Callgrind arms: <br>• unmodified paths (`blobmap_get`, `sync_blobmap_get`, `blobmap_insert_inline`, non-blob arms) <br>• plain mutations gaining header write / counters (`blobmap_insert`, `blobmap_overwrite`, `blobmap_remove`) <br>• concurrent mutations gaining counters (`sync_blobmap_insert`, `sync_blobmap_overwrite`, `sync_blobmap_remove`, `sync_blobmap_churn`) | <br>within +0.1 % of B <br>within +1.5 % of B (pre-data derived ceiling, +3–6 Ir/op) <br>within +1.0 % of B (pre-data derived ceiling, +3–6 Ir/op) | review bounds (§6) |
| G3 | sustained live records under overwrite workload at 128 B, 2 MiB chunks, 1 GiB cap | sustains at least 3,728,128 live records without error | `PASS` / `FAIL` |
| P4 | density loss at 128 B payload | matches `density_cost_ratio` (2.66 %, 101,941 records) | reported |

**Gate.**
- G1 `PASS`, G2 review bound met, G3 `PASS`, with all tests and §2.3 five-subsystem audit satisfied: the change merges.
- G1 `FAIL`: reverted, and measured outcome recorded on #1320.
- G1 `INCONCLUSIVE`: one more run pair under the same rule.

### 31.6 Not predicted, and out of scope

- **Not predicted:**
  - read latency of pinned views opened before compaction;
  - epoch reclamation scheduling of retired chunks.
- **Out of scope:**
  - engine implementation code (deferred per plan until pre-registration lands);
  - 32-bit embedded `ExpanseBlobMap32` (fixed 4,096-slot arena);
  - variable-length or non-integer key headers.

### 31.7 Verdict note — Stage 3B G2 review outcome and engine deferral (appended 2026-10-04)

The incremental compaction engine spike (PR #1344, branch `feat/1320-incremental-blob-compaction`, commit `1ce0974b53ab35ba3222d31a7f7c23826075fdcd`) was evaluated under CI `Perf / Callgrind Deterministic Instructions` run [37169426109](https://github.com/orieg/expanse/actions/runs/37169426109) against the `main` baseline run [37158441818](https://github.com/orieg/expanse/actions/runs/37158441818) at commit `9427093c1493f7c77c3b12ce99f42685efd00a15`.

**Verdict: §31 G2 FAIL on 10 of 13 blob arms.**

| Benchmark Arm | Main Baseline (Ir) | Spike Branch (Ir) | Delta (Ir) | Delta (%) | Locked §31 G2 Ceiling | Verdict |
|---|---:|---:|---:|---:|---|:---:|
| `cost::blobmap_get::random` | 10,692,296 | 10,942,296 | +250,000 | +2.3381% | +0.1% | **FAIL** |
| `cost::sync_blobmap_get::random` | 19,682,875 | 19,782,875 | +100,000 | +0.5081% | +0.1% | **FAIL** |
| `cost::blobmap_insert_inline::random` | 28,902,578 | 28,802,802 | -99,776 | -0.3452% | +0.1% | **PASS** |
| `cost::blobmap32_scan::ipv4_routes` | 343,888 | 343,888 | +0 | +0.0000% | +0.1% | **PASS** |
| `cost::sync_blobmap_remove_miss::random` | 19,221,107 | 19,269,545 | +48,438 | +0.2520% | +0.1% | **FAIL** |
| `cost::blobmap_insert::random` | 31,353,427 | 31,603,651 | +250,224 | +0.7981% | +1.5% | **PASS** |
| `cost::blobmap_overwrite::random` | 19,537,355 | 20,787,389 | +1,250,034 | +6.3982% | +1.5% | **FAIL** |
| `cost::blobmap_remove::random` | 25,873,477 | 26,823,477 | +950,000 | +3.6717% | +1.5% | **FAIL** |
| `cost::blobmap_churn::random` | 74,458,987 | 76,909,061 | +2,450,074 | +3.2905% | +1.5% | **FAIL** |
| `cost::sync_blobmap_insert::random` | 58,997,033 | 59,901,165 | +904,132 | +1.5325% | +1.0% | **FAIL** |
| `cost::sync_blobmap_overwrite::random` | 34,411,875 | 37,462,869 | +3,050,994 | +8.8661% | +1.0% | **FAIL** |
| `cost::sync_blobmap_remove::random` | 67,503,439 | 72,618,157 | +5,114,718 | +7.5770% | +1.0% | **FAIL** |
| `cost::sync_blobmap_churn::random` | 143,334,856 | 156,724,505 | +13,389,649 | +9.3415% | +1.0% | **FAIL** |
| `cost::blobmap_compact::random` | 19,436,154 | 20,335,943 | +899,789 | +4.6295% | N/A (partial evac) | REPORTED |
| `cost::sync_blobmap_compact::random` | 19,436,930 | 20,388,976 | +952,046 | +4.8981% | N/A (partial evac) | REPORTED |
| `cost::blobmap_insert_reclaiming::random` | *(new)* | 540,425 | — | — | NEW | REPORTED |
| `cost::sync_blobmap_insert_reclaiming::random` | *(new)* | 933,171 | — | — | NEW | REPORTED |

**Suite integrity (non-blob arms):** All 172 non-blob arms across the Callgrind suite passed within 0.1% (maximum $|\Delta| = 0.001507$%, worst arm `cost::strmap_get_short::short` at +308 Ir on 20,437,539 Ir baseline).

**Root cause analysis:**
1. Point lookup paths (`blobmap_get`, `sync_blobmap_get`): `read_record` reads and validates the widened 16-byte `BlobRecordHeader` (with embedded `key: u64` for $O(1)$ reverse lookup during relocation) instead of the former 8-byte header, adding $+5.0\text{ Ir/op}$ (+2.34%) on the plain path and $+2.0\text{ Ir/op}$ (+0.51%) on the concurrent path against the locked +0.1% ceiling.
2. Mutation paths (`overwrite`, `remove`, `churn`, `sync_*`): Stamping keys into headers, maintaining per-chunk live bytes/records, and atomic synchronization (`ChunkCounters` `fetch_sub`/`fetch_add`) added $+3$ to $+20\text{ Ir/op}$ across plain and concurrent mutations, exceeding the pre-registered +1.0% / +1.5% target ceilings.

**Resolution (maintainer decision):**
Per rule B-12 and AGENTS §1.6 (no post-hoc threshold redesigns to force a pass), the engine changes cannot merge as-is. The engine implementation is deferred to v0.12 under a new pre-registration with redesigned thresholds and/or an optimized record format. This evaluation PR lands the committed mathematical derivations (`scripts/blob_reclaim_bounds.py`), the §31 pre-registration, the new reclaiming Callgrind benchmark arms, and this recorded verdict.

## 32. Pre-registration for #1142 — validated batch cursor on concurrent map readers (appended 2026-10-03, locked before any batch-cursor engine code)

> **Section numbering note:** Section 31 is allocated to PR #1344 (A5, incremental blob arena compaction). This section uses §32 and will be renumbered if needed upon merge per work plan §0 rule 11.

### 32.1 The problem and architectural shape

`SyncExpanseMap` reader handles (`MapReader`, `OwnedMapReader`, `DetachedMapReader`) expose single-key ordered operations (`first`, `last`, `next_at_or_after`, `next_after`, `prev_at_or_before`, `prev_before`) via `map_reader_ordered_reads!` (`crates/expanse/src/sync.rs`). These entry points are public and tracked in `.github/public-api/expanse-trie.txt`, completing prerequisite (a) of #1142.

However, each single-key ordered operation pins the epoch, samples the tree version word, and descends from the root through `sync_nav::next_validated`. A $K$-entry scan using `next_after` therefore pays $K$ epoch pins and $K$ root descents.

The validated batch cursor architecture:
- **Pinning:** pins the epoch once per batch.
- **Terminal buffer:** copies one terminal's `(key: u64, val: u64)` entries using raw atomic loads into a cursor-owned buffer.
- **Buffer sizing derivation (`scripts/batch_cursor_bounds.py`):**
  - In Expanse's digital trie, the widest terminal is a level-1 bitmap leaf (`LeafB1`), which represents an 8-bit byte expanse and holds at most $2^8 = 256$ entries (`MAX_LEAFB1_POP = 256`, `crates/expanse/src/leaf.rs`).
  - At 16 bytes per entry, a 4,096-byte (4 KiB) cursor-owned buffer strictly accommodates any terminal in the trie.
  - Sizing decision: a 4 KiB buffer is chosen over mid-leaf resumption. A 4 KiB buffer guarantees that every terminal is copied in a single batch (0 extra descents across all valid populations $P \in [0, 256]$) and preserves terminal-level atomicity under the parent bracket. A narrower buffer (e.g. 64 entries) incurs $\lceil P / B \rceil - 1$ extra root descents (up to 3 extra descents, 42 extra version loads, and 21 extra fences per leaf under depth-7 point descent), breaks terminal atomicity across batches, and provides no memory advantage since 4 KiB resides entirely within L1 data cache and requires zero heap allocation during scanning.
- **Terminal atomicity and covering function:** in Expanse, terminal leaves (`Leaf1`..`Leaf7`, `LeafB1`, immediates) carry no version word of their own (`docs/ARCHITECTURE.md` §4.1 line 589; `crates/expanse/src/sync_nav.rs:26-29`). Their stores are bracketed by their covering parent branch's version word (`Holder::Node(vp, snap)`, `crates/expanse/src/sync_nav.rs:101-114`), which brackets every in-place store (`crates/expanse/src/mutate_map.rs:1339, 1485, 1500, 1684`). Draining a terminal leaf under its covering parent's version word validates strictly 1 branch node (`terminal_drain_read_set_branches() = 1`), paying 2 version loads and 1 acquire fence (`olc_bounds.version_word_cost(1, retained=False) = (2, 1)`: `occ::node_sample` at line 44, `occ::node_validate` at line 60).
- **Sibling-step resume and skipped empty siblings:** within a batch (while pinned), the cursor steps to the next sibling under the direct parent's retained version word. Under Expanse's lazy census rollup (`docs/ARCHITECTURE.md` §4.1 line 591), child mutations do not modify ancestor version words. However, filling an empty slot in a branch rewrites the branch's digit and edge arrays inside that branch's own version bracket (`crates/expanse/src/mutate_map.rs:1868` for `BranchL3`, line 1904 for `BranchL7`, line 2000 for `BranchB`, line 2140 for `BranchU`). Therefore, the parent branch's version word covers the absence of any skipped empty sibling. If navigation enters a non-empty child subtree, that child has its own version word and may mutate without moving the parent; hence any entered branch is retained in the read set (`crates/expanse/src/sync_nav.rs:407, 442, 490`), bounded by `ordered_read_set_branches(l) <= 13` (`scripts/olc_bounds.py`), within `READ_SET_CAP = 16` (`sync_nav.rs:46`), and re-validated at the end (`sync_nav.rs:209`).
- **Across-batch resume:** across batches, the cursor unpins and re-descends from the root by key (`next_at_or_after`). Once unpinned, epoch-based reclamation (EBR) permits node memory to be retired and recycled through freelists; retaining raw node pointers across an unpin would introduce a use-after-free hazard.

### 32.2 Scan semantics

The batch cursor provides a **weakly consistent, strictly ascending scan**:
1. **Strictly ascending:** keys are yielded in strictly increasing order ($k_1 < k_2 < \dots < k_m$).
2. **Atomic terminal snapshots:** each copied terminal is read and validated atomically under its parent's OCC version bracket.
3. **Present throughout:** every key that exists in the map continuously from the start of the scan until the scan passes its key value is guaranteed to be emitted exactly once.
4. **Not a snapshot (#1103):** keys inserted or removed concurrently during the scan may or may not be observed; keys inserted in regions already passed are not seen; keys inserted ahead of the cursor before it reaches that terminal may be seen. No key is ever emitted more than once.

### 32.3 Soundness gates, before any measurement

No performance claim is evaluated until every gate below passes:
- **G32.1 — Deterministic park-point tests:** thread-armed `cfg(test)` park points at key transitions with negative controls (insertion into just-copied terminal before validation, insertion into skipped sibling, removal from next leaf, obsolete/split parent, leaf demotion). Each negative control must fail by name; the positive tests must pass (AGENTS.md §2.3).
- **G32.2 — Loom model:** model verifying the retained-path resume and parent validation safety under concurrent preemptions.
- **G32.3 — Scan linearizability checker:** whole-scan checker in `tests/linearizability.rs` verifying scan semantics over concurrent execution histories.
- **G32.4 — Miri UB-site workload:** concurrent reader/writer batch cursor scan workload registered in `.github/miri-ub-sites.json`.
- **G32.5 — Differential testing:** differential testing against `MapCursor` and `std::collections::BTreeMap` on quiescent trees across all key distributions.
- **G32.6 — Boundary tests:** verification of empty map, empty range ($lo > hi$), single-key present/absent ($lo == hi$), bounds between existing keys, and extremal keys ($0$ and `u64::MAX`).

### 32.4 Predictions, each with its refuter

- **P32.1 — Single-threaded scalar paths do not move (AGENTS.md §2.1.5):**
  - In each PR that adds batch-cursor code, against that PR's base, `map_cursor_scan/*`, `map_nav/*`, `map_prev/*`, `map_get/*`, and `map_insert/*` change by at most 0.1%, the §6 review threshold.
  - **REFUTED** on any single-threaded arm regressing by > 0.1%.
- **P32.2 — Callgrind instruction reduction against `sync_map_next_after_scan`:**
  - Pre-registered target ceilings (target) registered in `scripts/batch_cursor_bounds.py::TARGET_CALLGRIND_RATIOS`:
    - `sync_map_scan/sequential`: instruction count ratio $\le 0.30$ vs `sync_map_next_after_scan/sequential` (target) (>= 70% instruction reduction target).
    - `sync_map_scan/clustered`: instruction count ratio $\le 0.40$ vs `sync_map_next_after_scan/clustered` (target) (>= 60% instruction reduction target).
    - `sync_map_scan/random`: instruction count ratio $\le 0.50$ vs `sync_map_next_after_scan/random` (target) (>= 50% instruction reduction target).
  - Evaluated on CI `instruction-counts` job.
  - **REFUTED** if any ratio exceeds its pre-registered target ceiling.
- **P32.3 — Concurrent wall-clock throughput scaling:**
  - Evaluated on the reference host via `writer_scaling --read-op scan` (specified in §32.5 below; not run in this stage).
  - Reader scan throughput with batch cursor exceeds unbatched `next_after` scan throughput by $\ge 3\times$ at W = 0 (target) and $\ge 2\times$ at W = 1, R = 4 (target).
  - **REFUTED** if BCa 95% confidence interval lower bound falls below these thresholds across two independent runs.

### 32.5 Instruments and cells specification

- **Harness:** `crates/expanse/examples/writer_scaling.rs` gains `--read-op scan` for `--arm map`.
  - Readers traverse the map via the batch cursor in batches of up to 256 entries.
  - With W $\ge$ 1, readers scan from the writers' start barrier until writers complete. With W = 0, readers scan until completing a fixed probe quota.
  - Rows record `readers`, `read_op`, `probe`, `reader_ops`, `reader_elapsed_s`, and counters `read_ops`, `read_attempts`, `read_fallbacks`.
- **Probes:**
  - `uniform`: 64-bit uniform random prefill keys.
  - `hotspot`: clustered keys where writes land within active scan subtrees.
- **Cells:** $`(W, R) \in \{(0, 1), (0, 4), (1, 4), (4, 4)\} \times \text{probe} \in \{\text{uniform}, \text{hotspot}\}`$: 8 cells total.
- **Driver:** `writer_scaling.py --scan-cells`, 8 rounds per cell, Williams construction permutation ordering.
- **Pin:** `0,2,4,6,8,10,12,14`, one thread per physical P-core on the reference host.
- **Protocol:** Two independent runs (`results/scan_cells_writer_scaling.json` and `scan_cells_writer_scaling_run2.json`), BCa 95% confidence intervals, pre/post system load snapshots.
- **Execution status:** Specified in this pre-registration; execution deferred to reference host evaluation.

### 32.6 What voids a cell or a run

- An applied pin other than `0,2,4,6,8,10,12,14`.
- Host contention per AGENTS.md §8.17 (non-target CPU > 100%, load average > cores / 2, or load shift > 2).
- Inconsistent counters or uncompleted rounds.
- Runs compared across different commits.

## 33. Pre-registration for #1143 — validated ordered reads and batch cursor on StrReader (appended 2026-10-04, locked before any engine code; next free section, may be renumbered at merge per work plan §0 rule 11)

> **Section numbering note:** Next free section on `main` following §32 (#1142). Will be renumbered if needed upon merge per work plan §0 rule 11.

### 33.1 The problem and architectural shape

`StrReader` exposes only point lookup `get` and `contains` (`crates/expanse/src/sync.rs:13030-13095`). An ordered string scan on a shared map today must go through `SyncExpanseStrMap::with_locked` (`sync.rs:12808`) — every ordered operation on `ExpanseStrMap` takes `&self` on the single-threaded map (`cursor` `strmap.rs:2498`, `cursor_at_or_after` `:2509`, `next_*` / `prev_*` `:2560-2610`), but on a shared map `with_locked` acquires the writer mutex and holds the tree word for the whole closure, quiescing optimistic writers and blocking concurrent readers that fall back. A scan therefore serialises against writers and concurrent readers.

The scope of #1143:
1. Validated ordered point operations on `StrReader` (`first`, `last`, `next_at_or_after`, `next_after`, `prev_at_or_before`, `prev_before`), the string twin of #900.
2. A validated concurrent batch cursor `SyncStrMapCursor` on `StrReader` with prefix and `[lo, hi)` bounds, following the u64 cursor's design (#1142).

What differs from the u64 map (`scripts/str_cursor_bounds.py`):
- **Holder generalization:** `sync_nav` assumes a `SeqVersion` tree word (`Holder::Tree(&'a SeqVersion, u64)`); a `StrNode`'s cover is a 32-bit OCC version word (`cover: u32`) standing where the tree word stands (`docs/ARCHITECTURE.md` §4.2; `crates/expanse/src/strmap.rs:387`), addressed through `version_cell` and bracketed by `version_begin` / `version_end`. `Holder` must generalise to cover node-level words standing as the root cover of each sub-trie.
- **Real trie descent & depth bounds:** Keys are partitioned into 8-byte chunks (`CHUNK_BYTES = 8`, `crates/expanse/src/strmap.rs:117-122`). A key of length $K$ spans $C(K) = \lfloor K / 8 \rfloor + 1$ chunk stages (`chunk_at`, `strmap.rs:591-598`). Each chunk is looked up in a `StrNode`'s `MapCore` trie (`strmap.rs:384-392, 2309-2327`), descending through up to 7 branch levels (levels 8 down to 2; level 1 is leaf) plus the `StrNode.cover`. Total versioned node depth from root to leaf is bounded by $D(K) \le 8 C(K) = 8 (\lfloor K / 8 \rfloor + 1)$. For $K \in [0, 7]$, $D \le 8$; for $K = 8$, $D \le 16$; for $K = 64$, $D \le 72$.
- **Read set scaling & reconciliation with `READ_SET_CAP = 16` (`sync_cursor.rs:54`):**
  - Within a single `StrNode`'s `MapCore`, an ordered step or backtrack retains at most 13 branch nodes (`scripts/olc_bounds.py`) plus the 1 `StrNode` cover, totalling at most 14 versions. This strictly fits within `READ_SET_CAP = 16` ($14 \le 16$).
  - Across multiple `StrNode` levels ($K \ge 8, C(K) \ge 2$), unconstrained backtracking would require $14 + 8(C(K) - 1) \ge 22$ versions, exceeding `READ_SET_CAP = 16`.
  - **Scoped terminal drains & resume-by-key policy:** Batches drain terminal chunks/leaves within the active `StrNode` under its cover. Across `StrNode`s and across batches, the cursor validates the retained versions, unpins, and resumes from root by key (`next_at_or_after`), ensuring no raw node pointers are held across unpins and memory stays strictly $O(1)$. If any descent path ever exceeds `READ_SET_CAP = 16`, `ReadSet::sample` returns `Err(Retry)` (`sync_cursor.rs:80-82`), triggering validation, unpin, and root re-descent by key.
- **Variable-length key buffering & byte budget:**
  - `BATCH_CAP = 256` entries (matching `sync_cursor.rs:51` and `LeafB1` maximum population).
  - `KEY_BUFFER_BUDGET_BYTES = 4096` bytes (4 KiB arena for variable-length key bytes).
  - `ENTRY_DESCRIPTOR_BYTES = 16` (8 bytes value + 4 bytes offset + 4 bytes length).
  - Total cursor buffer footprint: $4096 + 256 \times 16 = 8192$ bytes (8 KiB, residing entirely within L1 data cache).
  - **Oversized key policy ($K > 4096$ bytes):** If the buffer is non-empty, close and validate the current batch. When the buffer is empty and the next key exceeds 4096 bytes, allocate a dedicated spill buffer sized to $K$, validate under the node cover, and emit as a 1-item batch ($N = 1$). Subsequent batches resume from root by key. Zero heap allocations occur for all keys $\le 4096$ bytes during cursor scanning.
- **Sub-map in leaf state:** covered by the `cover` word used as a lock (transitions T1/T7).
- **Suffix byte copying and atomicity:** suffix bytes in `StrSuffix` are write-once upon publication, but an unlinked suffix can be retired by an optimistic writer (T4 split, T8 removal). Suffix bytes must be copied into the cursor buffer under the node's `cover` word and then validated, never borrowed across brackets or unpins.
- **Keys:** keys are `NulFreeStr` (issue #808), byte-lexicographically ordered with NUL padding for terminal chunks.

### 33.2 Scan semantics

The string batch cursor provides a **weakly consistent, strictly ascending scan** (identical to #1142 / §32.2 contract):
1. **Strictly ascending:** keys are yielded in strictly increasing byte-lexicographic order ($k_1 < k_2 < \dots < k_m$).
2. **Atomic terminal chunks:** each copied terminal chunk or suffix leaf is read and validated atomically under its parent `StrNode`'s OCC version bracket.
3. **Present throughout:** every key that exists in the map continuously from the start of the scan until the scan passes its key value is guaranteed to be emitted exactly once.
4. **Not a snapshot (#1103):** keys inserted or removed concurrently during the scan may or may not be observed; keys inserted in regions already passed are not seen; keys inserted ahead of the cursor before it reaches that node may be seen. No key is ever emitted more than once.

### 33.3 Soundness gates, before any measurement

No performance claim is evaluated until every gate below passes:
- **G33.1 — Deterministic park-point tests with negative controls:** thread-armed `cfg(test)` park points at key string transitions with negative controls (AGENTS.md §2.3):
  - `test_park_insert_into_just_copied_terminal_before_validation`: Writer inserts into terminal chunk/leaf after cursor copies entries but before `cover` validation; negative control skips `cover` validation and must fail; positive test detects version change, retries and passes.
  - `test_park_insert_into_skipped_sibling`: Writer inserts into empty branch slot in `MapCore` skipped by cursor; negative control skips parent version validation and must fail; positive test detects parent version bump, retries and passes.
  - `test_park_remove_from_next_leaf`: Writer removes/condenses next leaf while cursor steps; negative control yields stale/torn entry and must fail; positive test retries and passes.
  - `test_park_split_obsolete_parent`: Writer splits `StrSuffix` (transition T4) or parent branch while cursor holds snapshot; negative control uses retired pointer and must fail; positive test catches obsolete/dirty mark, retries, and passes.
  - `test_park_node_demote_prune`: Writer demotes/prunes `StrNode` (transitions T8/T9); negative control reads unlinked node and must fail; positive test detects cover lock/version mismatch, retries and passes.
- **G33.2 — Loom model:** model in `crates/expanse/src/sync.rs` / `occ.rs` verifying `Holder` generalization with 32-bit `cover` roots and suffix copy under concurrent writer preemptions.
- **G33.3 — Scan linearizability checker:** whole-scan checker in `tests/linearizability.rs` verifying string scan semantics over concurrent execution histories.
- **G33.4 — Miri UB-site workload:** concurrent reader/writer string batch cursor scan workload registered in `.github/miri-ub-sites.json`.
- **G33.5 — Differential testing:** differential testing against single-threaded `StrCursor` on quiescent maps across `paths` and `routes` key distributions.
- **G33.6 — String boundary tests:** verification of empty prefix, a prefix equal to a whole key, a prefix with no keys, prefix lengths 7, 8 and 9 at the 8-byte chunk edge, and NUL-terminal ordering against a continuation byte.

### 33.4 Predictions, each with its refuter

- **P33.1 — Single-threaded scalar paths do not move (AGENTS.md §2.1.5):**
  - In each PR that adds string batch-cursor code, against that PR's base, `strmap_cursor_scan/*`, `strmap_prefix_scan/*`, `strmap_get/*`, and `strmap_insert/*` change by at most 0.1%, the §6 review threshold.
  - **REFUTED** on any single-threaded plain string arm regressing by > 0.1%.
- **P33.2 — Callgrind instruction ratio against `sync_strmap_scan_locked`:**
  - Evaluated on CI `instruction-counts` job when `sync_strmap_scan` lands in Stage 2.
  - Pre-registered target ceilings derived in `scripts/str_cursor_bounds.py::TARGET_CALLGRIND_RATIOS` from measured baseline `sync_strmap_scan_locked` (184.0 ins/key on `paths`, 273.2 ins/key on `paths_dense`, CI run 37249211829, commit dce4447834374b562fc8923e19d60a091a5d923b) + amortized sync/buffering overhead (~4.4 ins/key):
    - `sync_strmap_scan/paths`: instruction count ratio $\le 1.15$ vs `sync_strmap_scan_locked/paths` (target; predicted ~1.024).
    - `sync_strmap_scan/paths_dense`: instruction count ratio $\le 1.15$ vs `sync_strmap_scan_locked/paths_dense` (target; predicted ~1.016).
  - **REFUTED** if any ratio exceeds its pre-registered target ceiling 1.15.
- **P33.3 — Concurrent wall-clock throughput scaling:**
  - Evaluated on the reference host via `writer_scaling --read-op scan` on string maps.
  - Harness prerequisites: `crates/expanse/examples/writer_scaling.rs` currently supports only `--writers 0 --read-op get` on `--arm str` (`:3413-3426`). It must be extended to support `--arm str` with `--read-op scan` and $W \ge 1$ concurrent writers (using `WriterStrWorkload`).
  - Cells: $`(W, R) \in \{(0, 1), (0, 4), (1, 4), (4, 4)\} \times \text{probe} \in \{\text{paths}, \text{paths\_dense}\}`$ (8 cells total).
  - Driver: `writer_scaling.py --scan-cells --arm str`, 8 rounds per cell, Williams permutation ordering.
  - Pin: `0,2,4,6,8,10,12,14`, one thread per physical P-core on the reference host.
  - Protocol: Two independent runs, BCa 95% bootstrap confidence intervals, pre/post system load snapshots.
  - Floors judged on BCa 95% CI lower bound:
    - At $(W=0, R=4)$: Reader scan throughput scaling floor CI lower bound $\ge 2.5\times$ over single reader ($W=0, R=1$).
    - At $(W=1, R=4)$: Throughput ratio of `SyncStrMapCursor` over `with_locked` baseline has CI lower bound $\ge 3.0\times$ (target).
      *Derivation:* Under `with_locked`, 4 readers and 1 writer serialize on a single mutex, causing an $O(R)$ collapse in reader throughput ($\approx 0.25\times$ single-reader throughput). Optimistic lock coupling allows readers to scan in parallel without acquiring the writer mutex, paying only OLC validation retries when writers mutate in-place, achieving $> 3.0\times$ speedup.
    - At $(W=4, R=4)$: Throughput ratio over `with_locked` CI lower bound $\ge 2.0\times$ (target).
  - **REFUTED** if BCa 95% confidence interval lower bound falls below these floors across two independent runs.

### 33.5 Instruments and cells specification

- **Arms:**
  - `sync_strmap_scan_locked`: `paths` and `paths_dense` (50,000 ops), measuring the `with_locked` single-threaded cursor walk baseline. Registered and measured in Stage 1 (9,199,722 ins / 184.0 ins/key on `paths`; 13,661,632 ins / 273.2 ins/key on `paths_dense`; CI run 37249211829, commit dce444783437).
  - `sync_strmap_scan`: `paths` and `paths_dense` (50,000 ops), measuring the concurrent validated batch cursor walk. Added in Stage 2 together with engine implementation.
- **Harness:** `crates/expanse/examples/writer_scaling.rs` extended to support `--arm str --read-op scan` with $W \ge 1$.
- **Probes:** `paths` and `paths_dense` key generators (`path_keys`).
- **Cells:** $`(W, R) \in \{(0, 1), (0, 4), (1, 4), (4, 4)\} \times \text{probe} \in \{\text{paths}, \text{paths\_dense}\}`$: 8 cells total.
- **Driver:** `writer_scaling.py --scan-cells --arm str`, 8 rounds per cell.
- **Pin:** `0,2,4,6,8,10,12,14`, one thread per physical P-core on the reference host.
- **Protocol:** Two independent runs, BCa 95% confidence intervals, pre/post system load snapshots.
- **Execution status:** Specified in this pre-registration; execution deferred to reference host evaluation.

### 33.6 What voids a cell or a run

- An applied pin other than `0,2,4,6,8,10,12,14`.
- Host contention per AGENTS.md §8.17 (non-target CPU > 100%, load average > cores / 2, or load shift > 2).
- Inconsistent counters or uncompleted rounds.

### 33.7 Amendment A1 to §33 — mechanism, read-set bound, projected constants, and units (appended 2026-10-08, before any engine code of #1143; §33.1 to §33.6 above are not edited)

A review of §33 against `origin/main` at `d5c5289a1` found statements of fact in §33.1 that the code contradicts, a read-set bound that undercounts, and constants presented as derived that no run measured. This amendment is errata plus a re-derivation. It changes no P33 threshold, no cell, no void condition and no gate G33.1 to G33.6. Where it contradicts §33.1, this section is the statement that stands. Refs #1143.

#### 33.7.1 The `with_locked` mechanism

§33.1 says `with_locked` "holds the tree word for the whole closure ... blocking concurrent readers that fall back". The code does not do that.

- `SyncExpanseStrMap::with_locked` (`crates/expanse/src/sync.rs:12980`) calls `Shared::with_locked` (`sync.rs:3032-3074`), which takes `fallback_mutex`, quiesces the writer gate and takes the writer mutex, then runs the closure.
- It brackets the tree word only around the dirty-digit fold republish (`sync.rs:3054-3061`), and only when `T::PUBLISHES_ROOT` holds and the fold finds a top edge. `SharedTree for ExpanseStrMap` (`sync.rs:797-835`) does not override `root_top_ptr`, whose default is null (`sync.rs:684-686`), so for the string map the closure runs with the tree word even. Optimistic `get` readers are not made to retry by it.
- What a scan under `with_locked` serialises against is other `with_locked` callers (the mutex) and writers (the gate), and a reader that has fallen back to `read_locked` (the same mutex).

This changes the mechanism sentence in the P33.3 derivation (the `(W=1, R=4)` "single mutex" collapse applies to scanners and to fallback readers, not to optimistic point readers) and nothing else. The floors are unchanged, and the 4-reader collapse figure in §33.4 stays an unmeasured expectation (unsourced; the cell decides).

Line references that moved: `StrReader` is at `sync.rs:13207` with `get` at `:13235` (§33.1 cites `:13030-13095`); `SyncExpanseStrMap::with_locked` is at `sync.rs:12980` (§33.1 cites `:12808`). The `writer_scaling.rs:3413-3426` cite is still current.

#### 33.7.2 Depth is shared prefix chunks, not key length

§33.1 indexes depth by key length, $C(K) = \lfloor K / 8 \rfloor + 1$, and bounds $D(K) \le 8 C(K)$. Tail collapse (`strmap.rs` module doc, `TAG_SUFFIX`) moves a key's remainder into a `StrSuffix` once it is unique, so the number of `StrNode` levels on a path is the number of shared whole 8-byte prefix chunks plus the level where the keys diverge: $\lfloor P / 8 \rfloor + 1$ for $P$ shared prefix bytes (`str_strnode_depth`). $C(K)$ remains a ceiling only for keys with no shared prefix. The `path_keys` generator builds 47-byte keys with a 35-byte shared prefix (`crates/expanse/benches/instructions.rs:1945`, `path_keys`), so its depth is 5, where key length would give 6. `paths_dense` can be deeper where chunk-4 bytes collide.

#### 33.7.3 The retained read-set bound, derived by enumeration

§33.1 gives $14 + 8(C(K) - 1)$ for a multi-level search and "14 <= 16" for the cursor. Both are wrong for the code to be ported. Three reviews of the scoping computed the point-op bound three ways; each is reproduced by one seek-count assumption of the enumeration `enumerate_point_read_sets` in `scripts/str_cursor_bounds.py`, which walks the shapes of the ported `StrNode::next_at_or_after` and takes the maximum:

- Inputs, per `StrNode` level: the cover (1); a descent of 0 to 7 branches (0 for a sub-map in leaf state, at most `ROOT_LEAF_CAP = 31` entries); a sub-map seek that answers after a backtrack at level $\ell \in [2, 8]$, retaining `olc_bounds.ordered_read_set_branches(ℓ)` branches, at most 13.
- `ReadSet::sample` pushes every sample and `backtrack` shrinks the set only under `cfg(test)` as a negative control (`crates/expanse/src/sync_nav.rs:69-79`, `:758-771`), so the paths of two seeks in one sub-map are both retained.
- The single-threaded walk it mirrors seeks `next_at_or_after(target)` at `strmap.rs:957`, then `next_after(target)` when a suffix compares below the remainder (`strmap.rs:977`) or `next_after(parent_target)` on unwinding from a failed child (`strmap.rs:1001`). That is two seeks per descended level, so the two-seek row is the one that matches the code.

| Maximum versions retained, probe depth $D$, answer depth $D - 1$ (derived: `scripts/str_cursor_bounds.py::str_point_read_set_bound`) | D=1 | 2 | 3 | 4 | 5 | Form |
|---|---|---|---|---|---|---|
| `two_seek` (matches the code) | 21 | 44 | 67 | 90 | 113 | $15d + 6 + 8a$ |
| `one_seek` (the resume reuses the first seek's path) | 14 | 30 | 46 | 62 | 78 | $8d + 6 + 8a$, i.e. $16D - 2$ |
| `ordered_step_per_failed_level` (scoping document; a failed level is charged a full ordered step, which a level that found nothing cannot retain) | 14 | 36 | 58 | 80 | 102 | $14d + 8a$, i.e. $22D - 8$ |

The maximum is the answer taken at level 0 (each lower answer level removes more from the descent term than it adds). Of the two figures that are not the code's, only `one_seek` is reachable, and only if the port shares a path between its two seeks, which `ReadSet` does not do today. The enumeration is cross-checked against the closed forms over $d \in [1, 5]$, $a \in [0, 4]$ in the script's self-test.

Cursor attempt, ancestor covers included (derived: `str_cursor_attempt_read_set_bound`). An attempt descends $d - 1$ ancestor levels (cover plus descent path each) and works in the final level; a failed child restarts by key, so failed levels are not retained across attempts.

| | d=1 | 2 | 3 | 4 | 5 |
|---|---|---|---|---|---|
| `two_seek`, worst case | 21 | 29 | 37 | 45 | 53 |
| `one_seek`, worst case | 14 | 22 | 30 | 38 | 46 |
| `one_seek`, ancestors in leaf state (cover only) | 14 | 15 | 16 | 17 | 18 |
| `two_seek`, ancestors in leaf state (cover only) | 21 | 22 | 23 | 24 | 25 |

Corrections to §33.1 that follow:

- "14 <= 16" holds only at depth 1 and only under one seek per level. With ancestor covers a `paths` attempt (depth 5, ancestors in leaf state) reaches 18 under one seek and 25 under the code's two seeks, and the worst case reaches 46 and 53. Under two seeks even depth 1 (21) exceeds `READ_SET_CAP = 16`.
- The leaf-state ancestor rows rest on the shape of `path_keys` (levels 0 to 3 are single-entry chains) and are (projected): no read-set-length census on a built map has been taken, and none is claimed.
- The $14 + 8(C(K) - 1)$ form is withdrawn. It charged 8 per extra level and indexed levels by key length.
- This amendment does not set a read-set capacity for the point ops. Whether to size the set at a fixed capacity, share the seeks' paths or de-duplicate is a decision for the PR that adds the walk, taken against this table.

#### 33.7.4 Overflow policy

§33.1 says an overflow of `READ_SET_CAP` makes `ReadSet::sample` return `Err(Retry)` and triggers root re-descent. `sample` does that today (`sync_nav.rs:72-73`, `crates/expanse/src/sync_cursor.rs:82-83`), and for the u64 maps it is sound because there an overflow implies a moving tree. For strings a depth overflow is deterministic, so every retry overflows again and the read spends `MAX_RETRIES = 64` attempts (`sync.rs:165`) before `read_locked`; `Shared::optimistic_read` has no outcome that falls back at once (`sync.rs:3146-3180`).

The policy for the string walk is: an overflow is classified by `len == READ_SET_CAP` after `validate_all`. A failed validation is a moving tree and retries; a passed validation with a full set is a depth overflow and goes to `read_locked` at once, counted by its own `occ_stats` stat. `classify_read_set_overflow` pins this. No Rust changes in this amendment.

#### 33.7.5 Open modelling item: an emptied child that stays linked

The enumeration assumes that a sibling descent which starts always answers, as the comment block above `BRANCH_TOP_LEVEL` in `scripts/olc_bounds.py` states for a non-empty node. Transition T9 can leave a node empty and still linked: "The emptied node then stays linked and empty" (`strmap.rs:3610-3620`). The single-threaded `extreme_entry` expects a non-empty node (`strmap.rs:882-892`, `expect("non-empty node")`). A validated port must instead treat an empty linked child as a node that answers nothing and continue to the next sibling, and each such child retains its cover and path. The bound in §33.7.3 does not cover that case: it is open, unmodelled, and the table is a lower bound on the worst case until it is closed. Tracked in #1143.

#### 33.7.6 Constants that are projections

§33.5 and `scripts/str_cursor_bounds.py` present the following as derived. None was measured, and each is (projected) from here on:

- the batch overhead of $40 + 15 + 35 = 90$ instructions per batch (projected);
- the assumed batch of 64 keys (projected), which makes the amortized sync cost $90 / 64 \approx 1.4$ instructions per key (projected);
- the 3.0 instructions per key of buffering (projected);
- the resulting ~4.4 instructions per key (projected), and the ratios "predicted ~1.024" and "~1.016" in §33.4 (projected).

The baseline instructions per key, 184.0 on `paths` and 273.2 on `paths_dense`, stay (measured: CI run 37249211829, commit `dce4447834374b562fc8923e19d60a091a5d923b`). The ceiling 1.15 stays (target). The 90-instruction term was not re-derived for the larger read set of §33.7.3, so the prediction does not constrain P33.2; only the `instruction-counts` job decides it.

#### 33.7.7 Unit of P33.3, and arm identities

- **Unit.** P33.3 throughput is scan keys per second (elements returned), not scans per second. Each cell records its scan length (keys per scan) beside the rate, and the floors are ratios of keys per second between cells of equal scan length. A comparison between cells of different scan length is not a P33.3 comparison.
- **Arm identities.** `sync_strmap_scan` stays the batch cursor's arm, as §33.5 registers it. The arm that scans through the validated point ops (`first` then `next_after`) is named `sync_strmap_scan_pointops`, and no result of it is filed under `sync_strmap_scan`. `sync_strmap_scan_locked` stays the baseline both are measured against.

#### 33.7.8 What is not changed

P33.1, P33.2 and P33.3 thresholds, the cells and the void conditions of §33.6, gates G33.1 to G33.6, and the instruments of §33.5 other than the added arm name. Reproduction of every figure in this amendment: `python3 scripts/str_cursor_bounds.py` (the table) and `python3 scripts/str_cursor_bounds.py --self-test` (the pinned values).

### 33.8 Pre-registration P33.4 — the value claim for the validated point ops (appended 2026-10-09, before any engine code of the point ops and before any data; §33.1 to §33.7 above are not edited)

The point ops (`StrReader::{first, last, next_at_or_after, next_after, prev_at_or_before, prev_before}`) are item 1 of §33.1. P33.3 registers the batch cursor's value claim; nothing registered one for the point ops. This section does, before the code that adds them and before any measurement of it. Refs #1143.

#### 33.8.1 Read-set capacity the claim assumes

The walk the point ops port is the two-seek walk of §33.7.3, whose bound is $15d + 6 + 8a$ with $a = D - 1$ at probe depth $d = D$: 21, 44, 67, 90, 113 for $D = 1 \dots 5$ (derived: `scripts/str_cursor_bounds.py::str_point_read_set_bound`). `path_keys` has depth 5 (§33.7.2), so a capacity that covers the measured workloads must hold 113 entries. The point ops use a read set of 128 entries, which covers every depth $D \le 5$; at $D = 6$ the bound is 136 and exceeds it. A full set is classified as §33.7.4 says: `validate_all` first, a failure retries, a success with the set full falls back to `read_locked` at once and is counted on its own stat. The emptied-but-linked child of §33.7.5 adds one cover and one parent seek per such child and is not in the bound; it reaches the overflow path, never a wrong answer.

A consequence registered here: on `paths` and `paths_dense` at depth 5 or less, the overflow count is 0. A non-zero overflow count on either probe falsifies the depth claim of §33.7.2 for that probe, and the cell is reported with it.

#### 33.8.2 The claim

- **Cell.** $(W, R) = (1, 4)$ on `paths` and on `paths_dense`: one writer running `WriterStrWorkload`, four readers, each reader repeating a full ascending scan made of `first` then `next_after` on its own `StrReader` (the `sync_strmap_scan_pointops` route). The cells are those of §33.5 at $(1, 4)$; no other cell is part of this claim.
- **Comparator.** The same cell with each reader scanning through `SyncExpanseStrMap::with_locked` and `StrCursor` (the `sync_strmap_scan_locked` route), under the same pin `0,2,4,6,8,10,12,14`, same key set, same writer.
- **Metric.** Aggregate reader throughput in scan keys per second (elements returned, §33.7.7), with the mean scan length (keys per scan) recorded beside it in every round. A pair of cells whose mean scan lengths differ by more than 1% is not a P33.4 comparison and is reported, not judged.
- **Threshold (proposed for maintainer confirmation).** The BCa 95% lower bound of the paired ratio (point ops over comparator, keys per second) is at least 1.0, on both probes, in both of two independent runs. Derivation: under `with_locked` the four scanners and the writer's serialised fallbacks share one mutex (§33.7.1), so the comparator's aggregate rate is at most one scanner's rate; the point ops take no lock on the optimistic path, so their aggregate rate is up to $R = 4$ times one point-op scanner's rate less retries. The ratio therefore clears 1.0 when one point-op step costs less than $R = 4$ cursor steps, that is when the instruction ratio of `sync_strmap_scan_pointops` to `sync_strmap_scan_locked` is below 4 (derived; the instruction ratio is unmeasured until the arm runs). Validation adds at most 113 version loads per step at depth 5 (§33.7.3), against a baseline of 184.0 instructions per key on `paths` (measured: CI run 37249211829, commit `dce4447834374b562fc8923e19d60a091a5d923b`); whether the whole step stays under four cursor steps is what the arm measures, not what this section asserts.
- **Falsifier.** The lower bound is below 1.0 on either probe in either run (REFUTED). A lower bound at or above 1.0 in one run and below it in the other is `INTERMEDIATE`. A cell whose optimistic read fallbacks exceed 1% of its read ops is reported with that share and does not count toward a PASS, since it measures the locked route.

#### 33.8.3 Which instrument decides what

- CI Callgrind decides **only** the no-regression gate of the pull request that adds the point ops: the §6 review threshold (0.1%) on the u64 ordered arms (`sync_map_get`, `sync_map_next_after_scan`, `sync_map_prev`, `sync_map_prev_locked`, `sync_map_insert`, `sync_map_churn`) and on every `strmap_*` arm, measured against that pull request's base. It records the new arm `sync_strmap_scan_pointops/{paths,paths_dense}` (50,000 ops, the `sync_strmap_scan_locked` setup and key set) as a figure, not as evidence for or against P33.4, because Callgrind cannot see contention.
- The reference host decides P33.4, with the §33.5 protocol: two independent runs, BCa 95% intervals, §8.17 load snapshots, pin `0,2,4,6,8,10,12,14`. The laptop decides nothing here.
- Execution status: specified; no run has been taken. The harness route (`--read-op scan` with a point-op variant on `--arm str`) does not exist yet and is a prerequisite of the run, not of the pull request that adds the point ops.

### 33.9 Amendment A1 to §33.8 — derivation scope, frozen threshold, missing parameters, and one correction (appended 2026-10-09, before any run of P33.4; §33.1 to §33.8 above are not edited)

A review of §33.8 found a derivation stated more strongly than it holds, parameters a run needs that §33.8 does not fix, a one-sided void rule, and a sentence the code contradicted. Where this section and §33.8 differ, this section stands. The proposed threshold value is not changed. Refs #1143.

#### 33.9.1 The derivation is a necessary condition

§33.8.2 derives that the paired ratio "clears 1.0 when" one point-op step costs less than R = 4 cursor steps. Read it as: P33.4 **can clear 1.0 only if** the instruction ratio of `sync_strmap_scan_pointops` to `sync_strmap_scan_locked` is below R = 4. It is an upper bound on what the point ops can achieve, not a prediction that they achieve it: an instruction ratio is not a time ratio (cache misses per step, the writer's effect on both routes and the mutex's hand-off cost are not instructions), and a ratio below 4 is necessary, not sufficient.

#### 33.9.2 The threshold freezes at confirmation

The threshold of §33.8.2 is proposed. It freezes at the moment the maintainer confirms it, in the issue or in a review of the pull request that adds the point ops, and that confirmation is recorded beside this section before any P33.4 run. A change after that moment relabels any result taken under the old value `INTERMEDIATE` and requires fresh runs (AGENTS.md §8.19).

#### 33.9.3 Parameters a run needs

- **Rounds and pairing.** 8 rounds per run. Each round runs the point-op cell and the comparator cell once each, back to back, in the order the §33.5 driver's Williams permutation gives that round. The paired statistic is the per-round ratio of point-op keys/s to comparator keys/s, both taken in the same round.
- **Interval.** BCa 95% over the 8 per-round ratios of a run, with 10,000 bootstrap resamples (at least the 1,000 AGENTS.md §8.4 requires).
- **Scan length.** A round whose two cells differ in mean scan length by more than 1% contributes no ratio. A run with fewer than 6 contributing rounds on a probe is `NOT_EVALUABLE` on that probe and counts as not taken; P33.4 is then evaluated only when two runs that are not `NOT_EVALUABLE` exist.
- **Verdicts.** `PASS` needs the lower bound at or above the threshold on both probes in both runs; `REFUTED` follows from a lower bound below it on either probe in either run; one run each way is `INTERMEDIATE`, as §33.8.2 says.

#### 33.9.4 The fallback share

A cell whose optimistic read fallbacks exceed 1% of its read ops counts toward `REFUTED` and not toward `PASS`. The asymmetry is deliberate: the shipped point ops include their fallback, so a loss measured through it is a loss of the shipped route; a win is attributed to the optimistic route this section registers only when that route served at least 99% of the reads. A cell over 1% whose lower bound clears the threshold is `INTERMEDIATE`, and the share is reported with it.

#### 33.9.5 A pre-run expectation, not evidence

The new arm's first counts put the point-op scan at 3,823 instructions per key on `paths` and 4,234 on `paths_dense`, against 184.0 and 273.2 for `sync_strmap_scan_locked`: ratios of about 20.8 and 15.5 (measured: CI run 37906645775 at commit `5b8fdac19`, an earlier head of the pull request that adds the point ops; the per-key figures are derived from the arm totals over 50,000 ops). By §33.9.1 a ratio above 4 means P33.4 cannot clear 1.0 at $(W, R) = (1, 4)$ unless time per instruction differs between the routes by more than the ratio's excess. This is recorded as a pre-run expectation only. It is not evidence for or against P33.4, which only the reference host decides (§33.8.3), and the threshold is not adjusted for it.

#### 33.9.6 Correction to §33.8.1

§33.8.1 says an emptied but linked child "reaches the overflow path, never a wrong answer". At the time it was written the overflow path's fallback, `read_locked` running the single-threaded ordered reads, panicked on such a child (`StrNode::extreme_entry`'s `expect("non-empty node")`), as `with_locked` did on `main`. The single-threaded reads pass over the child since the fix that precedes the point ops (Refs #1143), and the sentence holds from that commit on. `StrCursor` under `with_locked` still stops early at such a child, without a panic; that is outside P33.4.

### 33.10 Amendment A2 to §33 — the batch cursor's attempt shape and its read-set bound (appended 2026-10-31, before any engine code of the batch cursor; §33.1 to §33.9 above are not edited)

Item 2 of §33.1 is the batch cursor. §33.7.3 left open which walk the cursor would port; this section fixes it, and derives its bound, before the code that adds it. Where it differs from §33.1, this section stands. No threshold, cell, void condition or gate of §33.3 to §33.6 changes. Refs #1143.

#### 33.10.1 One drained level per attempt, restart by key

A batch is one attempt. It descends the probe's exact continuations (at each level a sub-map drain from the probe's chunk that finds the chunk first and stops at the child it maps to), then drains one `StrNode` sub-map with the u64 batch drain of §32 (`sync_cursor.rs`, `drain_batch_in`, including the end-bound stop of #1382 and #1385) and copies each entry's key into the batch. The attempt never unwinds. Three outcomes end it with nothing copied, and each is validated (every retained version and the tree word) before the next attempt starts by key with an empty read set:

- the level holds nothing at or after the probe: restart at the smallest key above every key under the level's prefix (the prefix with its trailing `0xFF` bytes dropped and its last byte incremented); at the root this ends the scan;
- the only entry drained is a suffix that sorts below the probe: restart at the smallest key whose chunk at that level is the next one;
- the first entry to copy is a child: restart at the child's prefix, which the next attempt descends exactly.

A child met after the batch has entries ends the batch, which resumes from the last key followed by `0x01`, as the point ops' `next_after` does. An emptied but linked child (§33.7.5) is a level that holds nothing, so it is passed over by the first rule: it costs an attempt, not a wrong answer and not an overflow.

#### 33.10.2 The bound

Per attempt: $8d + 6$ versions at path depth $d$, $d + 13$ when every ancestor is a sub-map in leaf state (derived: `scripts/str_cursor_bounds.py::str_cursor_drain_attempt_read_set_bound`). A sub-map drain retains at most 13 branches (derived by enumeration: `submap_drain_read_set_branches`, which equals `olc_bounds.max_ordered_read_set_branches()`), and an exact descent at most 7. The figures equal §33.7.3's `one_seek` attempt row (14, 22, 30, 38, 46 for $d = 1 \dots 5$), which the self-test pins. With the point ops' read set of 128 entries every path of depth 15 or less fits (126 at $d = 15$, 134 at $d = 16$, `str_cursor_max_depth_within_cap`); a deeper attempt that overflows a set that still validates fills that batch under the writer lock at once (§33.7.4). On `paths` (depth 5) the worst case is 46 and the overflow count is expected to be 0.

#### 33.10.3 Bounds and buffers

`[lo, hi)` takes a NUL-free `lo` inclusive and `hi` exclusive; a prefix scan is `[p, p')` with $p'$ the prefix incremented as above, unbounded when $p$ is all `0xFF`. The batch holds at most `BATCH_CAP` = 256 entries and 4,096 key bytes (§33.1), plus a 256-entry scratch for the drained `(chunk, value)` pairs, which §33.1's 8 KiB does not count. A key longer than the arena is copied into a spill buffer as a batch of one. P33.2 and its ceiling of 1.15 (target) are unchanged; the constants of §33.7.6 stay (projected), and only the `instruction-counts` job decides P33.2.

## 34. Pre-registration for #1280 Item 1 — cause of the four-thread peak in `SyncExpanseBlobMap` (appended 2026-10-04, locked before any run of the builds or instruments below)

> **Section numbering note:** Section 33 was claimed by PR #1373 (A6, thread-local freelist allocation stripe) which merged. This pre-registration uses §34 (peak) and §35 (split).

### 34.1 Context and problem statement

With optimistic removal landed (#1289, METHODOLOGY §26, README §26), `SyncExpanseBlobMap` at 50% read on the stationary mixed harness (`benches/concurrency.rs`) runs:
- 11.9 M total ops/s at 1 thread ($C(1) = 1.00$)
- 20.7 M total ops/s at 4 threads ($C(4) \approx 1.74$)
- 13.4 M total ops/s at 16 threads ($C(16) \approx 1.13$)

The curve exhibits a retrograde turnover at 4 threads, dropping by 35% at 16 threads ($`(20.7 - 13.4)/20.7 = 35.26\% \approx 35\%`$, observed on the reference host in #1288/#1293: README §26 line 5379 and README §27). In #1293 (METHODOLOGY §27), build K tested whether in-window compaction (which quiesces all writers) caused this peak by firing 4× less frequently (`BLOB_COMPACT_APPENDS = 4 * BLOB_POP`); while K raised throughput by 1.19× at 16 threads and 1.27× at 4 threads, K still fell from 4 to 16 threads (26.4 M at 4 threads vs 15.9 M at 16 threads). In-window compaction was refuted as the cause of the four-thread peak, and the remaining deficit remains unmeasured.

This pre-registration locks the disciplined investigation sequence per AGENTS.md §8.20.5: running the cheap diagnostic checks first, evaluating Gunther's USL model in committed Python, profiling PMU event shapes, analyzing cache line layouts via `perf c2c` and `perf report`, and testing named candidate hypotheses.

### 34.2 Diagnostic sequence per AGENTS.md §8.20.5

The investigation proceeds strictly through Steps 0–6 in order:

#### Step 0 & Step 1: Exclude the host core topology first
- **Problem & Topology:**
  The reference benchmark host (Intel Core i9-12900F) features an asymmetric hybrid architecture:
  - 8 Golden Cove Performance cores (P-cores) with Hyper-Threading / SMT, presenting CPUs 0–15 (2 virtual threads per physical P-core).
  - 8 Gracemont Efficient cores (E-cores) without SMT, presenting CPUs 16–23 (1 thread per core).
  Total host capacity is 24 hardware threads.

  The original 4-vs-16-thread figures in #1288 and #1293 were recorded with pin `0-15`:
  *(measured: reference host — Intel Core i9-12900F, pin `0-15`, 18 rounds per cell; runs D 36658309874 / 36661955960, K 36659602446 / 36660784014; README §27 line 5431, README §26 line 5312)*.

  What `cpu_pin=0-15` isolates: it pins execution exclusively to the 8 P-cores (16 hardware threads across 8 physical cores), completely excluding the 8 E-cores (16–23). This successfully eliminates E-core frequency asymmetry and heterogeneous scheduler migrations. However, within `0-15`, for thread counts $T \le 8$, the OS scheduler can co-schedule worker threads onto SMT sibling threads of already active physical cores while other physical cores remain idle.

  What dedicated physical core pin `cpu_pin=0,2,4,6,8,10,12,14` isolates: it pins each worker thread to an independent physical P-core (even logical CPU numbers 0, 2, 4, 6, 8, 10, 12, 14), structurally denying SMT sibling co-scheduling and completely eliminating hyperthreading crosstalk.

- **Instrument & Cells:**
  - Dedicated physical core pin: `cpu_pin=0,2,4,6,8,10,12,14` (one thread per physical P-core).
  - Swept physical thread counts: $`N \in \{1, 2, 3, 4, 5, 6, 7, 8\}`$ (all 8 physical P-cores, providing 8 points for USL fitting).
  - SMT baseline pin (`cpu_pin=0-15`): swept across $`N \in \{1, 2, 4, 8, 16\}`$, reproducing the original $T=16$ condition of #1288/#1293 to verify whether the peak and retrograde turnover survive.
  - Workloads: 50% read (primary) and 100% read (control).
  - Engines: `blob` (primary), `blob_mutex` (control), `map` (control), `str` (control).
- **Decision rule:** Compare the physical core sweep ($N=1..8$) against the SMT baseline ($N=1..16$). If the peak remains at $N=4$ and throughput degrades at $N=8$ under dedicated physical core placement, host SMT co-scheduling is **refuted** as the cause of the peak.

#### Step 2: Fit Gunther's Universal Scalability Law (USL) and read $\alpha$ against $\beta$
- **Model:** Gunther's USL defines modeled throughput $X(N)$ as:

  $`X(N) = \frac{\gamma N}{1 + \alpha (N - 1) + \beta N (N - 1)}`$

  with retrograde peak concurrency $`N_{\max} = \sqrt{\frac{1 - \alpha}{\beta}}`$.
- **Committed implementation:**
  - Executed via committed Python module `scripts/blob_peak_bounds.py` (and `scripts/fit_usl.py`), carrying unit tests pinning reference values.
  - Data points: 8 points ($`N \in \{1, 2, 3, 4, 5, 6, 7, 8\}`$) on the physical-core pin `cpu_pin=0,2,4,6,8,10,12,14`, with 18 Williams-ordered replicate rounds per cell (144 total measurements).
  - Degrees of freedom: with 8 data points, fitting the 3-parameter model provides 5 degrees of freedom (avoiding trivial over-fitting).
  - Parameter estimation: Gunther's linear transformation (OLS) and non-linear least squares (NLLS), with BCa 95% bootstrap confidence intervals for $\alpha$ and $\beta$.
- **Decision reading & classification rules:**
  - Goodness-of-fit thresholds: $`R^2 \ge 0.95`$ (target), $`\text{NRMSE} \le 5.0\%`$ (target), confirming the model accurately captures the empirical curvature.
  - Report $\alpha$ and $\beta$ with their BCa 95% bootstrap confidence intervals. Classification requires the interval to exclude 0:
    - If $\beta$'s 95% BCa CI excludes 0 (lower bound $> 0$) and modeled $N_{\max} \in [3, 5]$: contention is classified as **pairwise coherency crosstalk** ($\beta$, scaling as $N(N-1)$). The retrograde turnover is driven by cache-line invalidation traffic.
    - If $\beta$'s 95% BCa CI overlaps 0, while $\alpha$'s 95% BCa CI excludes 0 (lower bound $> 0$): contention is classified as **serial** ($\alpha$, e.g. writer gate, lock serialization). Cache-line coherence remedies will not move it.
    - If both intervals overlap zero or fit fails: classification is **inconclusive**.

#### Step 3: PMU event profiling for shape, not just magnitude
- **Events captured per operation:**
  - `cycles`, `instructions`: IPC and instruction count scaling across $`N \in \{1, 2, 3, 4, 5, 6, 7, 8\}`$ (physical pin) and $`N \in \{1, 2, 4, 8, 16\}`$ (SMT pin).
  - `ref-cycles`: frequency droop computation ($`f(N) / f(1) = (\text{cycles}/\text{ref-cycles})_N / (\text{cycles}/\text{ref-cycles})_1`$). Must be verified against the 35% throughput deficit ($`(20.7 - 13.4)/20.7 = 35.26\%`$); a frequency droop of 2–5% cannot explain a 35% fall, refuting clock throttling.
  - `LLC-load-misses`, `LLC-loads`: cache hierarchy eviction rate.
  - `page-faults`: kernel minor page fault rate.
  - `dTLB-load-misses`: data translation lookaside buffer miss rate.
- **Shape classification:**
  - $\alpha$-shaped: cost steps at $N=2$ writers and remains flat or sublinear per operation.
  - $\beta$-shaped: cost grows quadratically ($N^2$) per operation.

#### Step 4 & Step 5: `perf c2c` layout analysis and symbol resolution
- **Layout analysis:** Contended cache lines are classified before symbol attribution:
  - Compact 64-byte-aligned run inside a sub-page span ($< 4$ KiB): identifies a padded per-slot array or wrapper struct (e.g. `BlobWriterArenas`). Indicates false or true sharing on struct fields.
  - Dispersed lines across heap pages: identifies per-node or per-record heap allocations (e.g. tree version words, record headers).
- **Symbol resolution:** `perf c2c report` truncates mangled Rust v0 symbols in fixed-width columns. Full symbol resolution is performed using `perf report --stdio --sort symbol,dso` over the identical `perf.data` recording.

#### Step 6: Evaluation of named candidate hypotheses
All named candidates are currently unmeasured; causality requires interventional verification:
1. **Candidate A — `has_writer_deltas` line sharing:**
   - In `BlobWriterArenas` (`crates/expanse/src/sync.rs:10280`), `has_writer_deltas: AtomicBool` is unconditionally release-stored by every writer on every insert (`sync.rs:10582`) and removal (`sync.rs:10647`).
   - Line sharing: `has_writer_deltas` resides on the same 64-byte cache line as `unowned_live_delta` and `arena_epoch`.
   - Coherency impact: $W$ writers continuously invalidate this line across all cores. Modeled by `candidate_a_line_sharing_traffic` in `scripts/blob_peak_bounds.py`.
2. **Candidate B — Page faults on fresh 2 MiB chunks:**
   - Private chunks allocated via `grant_private_chunk_raw` (`crates/expanse/src/blobmap.rs:1724`) use 2 MiB anonymous virtual memory chunks.
   - Touching uncommitted virtual memory triggers 512 minor page faults per chunk (under 4 KiB pages).
   - Under 16 concurrent writers rapidly appending, concurrent page faults serialize on kernel memory management structures (`mmap_lock` / page table locks). Modeled by `candidate_b_chunk_page_faults` in `scripts/blob_peak_bounds.py`.
3. **Candidate C — `charge_dead` header reads:**
   - `charge_dead` (`crates/expanse/src/sync.rs:10615`) resolves overwritten or removed record lengths via `resolve_meta_in_table` (`sync.rs:10631`), reading the 8-byte record header.
   - For records distributed across older chunks, header dereferencing incurs LLC misses and memory bus contention. Modeled by `candidate_c_header_reads` in `scripts/blob_peak_bounds.py`.
4. **No residual-by-subtraction (AGENTS.md §8.20.4):**
   - Never attribute an unexplained remainder to a hypothesis by subtracting measured shares from 100%.
   - If Candidates A, B, and C fail to account for the measured scaling deficit under interventional testing, the remaining deficit MUST be explicitly reported as **unexplained**.

### 34.3 Instruments, cells, and dispatch specification

- **Harness:** `benches/concurrency.rs` executed via `docs/benchmarks/concurrency/scripts/mixed_concurrency.py`.
- **Workload:** `core_concurrency`, 18 rounds per cell, Williams construction order, 500 ms window duration.
- **Cells:**
  - Primary physical sweep: `engine=blob`, `workload=50% read`, $`N \in \{1, 2, 3, 4, 5, 6, 7, 8\}`$, `cpu_pin=0,2,4,6,8,10,12,14`.
  - Primary SMT baseline sweep: `engine=blob`, `workload=50% read`, $`N \in \{1, 2, 4, 8, 16\}`$, `cpu_pin=0-15`.
  - Control cells: `engine=blob_mutex`, `workload=50% read`, $`N \in \{1, 4, 16\}`$; `engine=blob`, `workload=100% read`, $`N \in \{1, 16\}`$; `engine=map`, `workload=50% read`, $`N \in \{16\}`$; `engine=str`, `workload=50% read`, $`N \in \{16\}`$.
- **Exact workflow dispatch:**
  Dispatched on the reference host via `.github/workflows/bench_baremetal.yml`:
  1. Physical core exclusion run:
     ```bash
     gh workflow run bench_baremetal.yml \
       -f ref=<commit> \
       -f benchmark_suite=concurrency \
       -f cpu_pin="0,2,4,6,8,10,12,14"
     ```
  2. Standard SMT baseline run:
     ```bash
     gh workflow run bench_baremetal.yml \
       -f ref=<commit> \
       -f benchmark_suite=concurrency \
       -f cpu_pin="0-15"
     ```

### 34.4 What voids a run or comparison

- An applied CPU pin differing from the declared dispatch specification.
- Co-resident host contention per AGENTS.md §8.17 (foreign CPU $`> 100\%`$, load average $`> \text{cores} / 2`$, or load shift $> 2$).
- Control cells moving outside $`\pm 5\%`$ in the same direction across runs (`DRIFT` per BENCHMARKING rule 18).
- Zero in-window compactions across 16-thread blob windows (verifying compaction trigger executed).

---

## 34a. Amendment A1 to §34 — the dispatch suite, the round counts and the controls on the physical-core pin (appended and locked 2026-10-04, before any §34 run produced data; §34 above is not edited)

### 34a.1 Why §34.3 cannot run as written

Three statements of §34.3 cannot hold together on the instrument it names:

1. **The dispatch.** §34.3 dispatches `benchmark_suite=concurrency`. That suite runs a fixed `--threads 1,4,16 --engines all --workloads 100,50`, so it cannot produce the §34.3 cells, and on the eight-CPU pin it asks for 16 threads. Two dispatches were made under it (runs 37261662824 and 37261672017). Run 37261672017 was cancelled before any step ran. Run 37261662824 was cancelled 61 s into its benchmark step: its uploaded bundle holds an empty `concurrency.txt` and the host guard files, and no results artifact. No throughput figure was produced or read.
2. **The round count.** §34.2 and §34.3 fix 18 rounds per cell in Williams order. A Williams design over $n$ thread counts is balanced only over whole cycles of $n$ rounds ($2n$ for odd $n$), and 18 is a whole number of cycles for neither the eight-level sweep (cycle 8) nor the five-level sweep (cycle 10).
3. **The controls on the physical-core pin.** §34.3 lists controls at 16 threads without naming a pin. The pin `0,2,4,6,8,10,12,14` has eight CPUs, and `benches/concurrency.rs` drops any thread count above the CPUs it may use.

### 34a.2 What changes

No hypothesis, threshold, decision rule or void condition of §34 changes. The three corrections are:

1. **Suite.** Both runs dispatch `benchmark_suite=concurrency_blob_peak`, which runs `docs/benchmarks/concurrency/scripts/mixed_concurrency.py --blob-peak`. The driver maps the applied pin to the cells below and refuses any other pin. The `cpu_pin` input is required.
2. **Rounds.** Each group runs the smallest whole number of Williams cycles that is not below 18. No cell has fewer windows than §34.3 fixed.
3. **Controls on the physical-core pin.** Every §34.3 control at 16 threads runs at 8, the pin's CPU count. The controls on pin `0-15` are those of §34.3.

| Pin | Group | Thread counts | Rounds per cell | Windows |
|---|---|---|--:|--:|
| `0,2,4,6,8,10,12,14` | `blob`, 50% read | 1, 2, 3, 4, 5, 6, 7, 8 | 24 | 192 |
| | `blob_mutex`, 50% read | 1, 4, 8 | 18 | 54 |
| | `blob`, 100% read | 1, 8 | 18 | 36 |
| | `map`, 50% read | 8 | 18 | 18 |
| | `str`, 50% read | 8 | 18 | 18 |
| `0-15` | `blob`, 50% read | 1, 2, 4, 8, 16 | 20 | 100 |
| | `blob_mutex`, 50% read | 1, 4, 16 | 18 | 54 |
| | `blob`, 100% read | 1, 16 | 18 | 36 |
| | `map`, 50% read | 16 | 18 | 18 |
| | `str`, 50% read | 16 | 18 | 18 |

The USL fit of §34.2 Step 2 therefore reads $8 \times 24 = 192$ windows, where §34.2 stated 144. The model, its estimators, the goodness-of-fit thresholds and the classification rules are unchanged.

Artifacts: `docs/benchmarks/concurrency/results/baseline_concurrency_blob_peak_percore.json` for the physical-core pin and `baseline_concurrency_blob_peak_pin0-15.json` for pin `0-15`. Each records its round count per group under `provenance.rounds`.

### 34a.3 Runs and dispatch

Two runs per pin, the second dispatched after the first completes, so that the `DRIFT` rule of §34.4 and `docs/BENCHMARKING.md` rule 18 can be evaluated:

```bash
gh workflow run bench_baremetal.yml \
  -f ref=<commit> \
  -f benchmark_suite=concurrency_blob_peak \
  -f cpu_pin="0,2,4,6,8,10,12,14"

gh workflow run bench_baremetal.yml \
  -f ref=<commit> \
  -f benchmark_suite=concurrency_blob_peak \
  -f cpu_pin="0-15"
```

The second run of a pin writes the same path, so each run's artifact is committed under a run-suffixed name.

### 34a.4 The §34.4 compaction condition is checked by the driver

§34.4 voids a run whose 16-thread `blob` windows at 50% read saw no in-window compaction. The driver sums `compactions` over those windows, records `provenance.verdict` (`ADMISSIBLE` or `VOID`) with the reason under `provenance.voids`, and exits non-zero on `VOID` after writing the artifact. The condition names 16 threads, so it applies to pin `0-15` only. The other three §34.4 conditions (the applied pin, host contention, control drift across runs) are read from the artifact's `core_pin`, its load snapshots and the pair of runs.

### 34a.5 Sizing against the job limit

The timed windows are 500 ms each: 318 windows (159.0 s) on the physical-core pin and 226 windows (113.0 s) on pin `0-15` (derived from the table above). Build and population time is outside the windows and is not measured here. The job limit is `timeout-minutes: 180` (`.github/workflows/bench_baremetal.yml`), that is 10,800 s.

---

## 35. Pre-registration for #1280 Item 2 — `Shared::enter_writer` hot/cold split and Callgrind ceilings (appended 2026-10-04, locked before any engine code; renumbers on rebase if #1373 merges first)

### 35.1 Context and problem identification

In PR #1289 (`1813cc1c4`), `SyncExpanseBlobMap::remove` was converted to optimistic lock coupling (`olc_remove_map`). Prior to #1289, `SyncExpanseBlobMap::insert` was the sole caller of `Shared::enter_writer_blocking` in `SyncExpanseBlobMap`. The compiler had inlined `enter_writer` directly into `insert`.

Adding `remove` introduced a second caller within the generic instantiation `Shared<ExpanseBlobMap>`. Because `enter_writer` (`crates/expanse/src/sync.rs:2372`) contains both the fast thread-local cache check and the cold slot allocation fallback (`CACHED_SLOTS.with(...)`, vector iteration, slot allocation), the compiler outlined `Shared<ExpanseBlobMap>::enter_writer` as a non-inlined function call.

Attribution measured on `rust:1.98` / Valgrind 3.24.0 (README §26):
- `Shared<ExpanseBlobMap>::enter_writer` (now out of line): +2,350,000 Ir
- `SyncExpanseBlobMap::insert`: −1,399,996 Ir
- Net delta on `sync_blobmap_overwrite/random`: +950,004 Ir (+2.84%).

**Consequence for Build A:**
Build A (`has_writer_deltas` stored only when reading `false`) delivers a measured 1.23–1.25× wall-clock throughput improvement at 16 threads (README §26.4). However, because `sync_blobmap_overwrite/random` had already absorbed +950,004 Ir from the outlining of `enter_writer`, Build A's additional flag check added 149,996 Ir, pushing `sync_blobmap_overwrite` to +3.28% against the locked +3% ceiling in §26.6 (CI run 36639227281). Build A was consequently rejected from landing in #1289.

**Multiplying call sites:**
Subsequent PRs #1355 (`compare_exchange`) and #1361 (`update`) added additional call sites to `enter_writer_blocking` across all five `Sync*` wrappers (`SyncExpanseSet`, `SyncExpanseMap`, `SyncExpanseStrMap`, `SyncExpanseBytesMap`, `SyncExpanseBlobMap`). Resolving this unforced overhead requires an architectural hot/cold split of `enter_writer`.

### 35.2 Architectural design of the hot/cold split

1. **Inlined fast path (`enter_writer`):**
   Marked `#[inline(always)]`. Performs only the thread-local cache check:
   ```rust
   #[inline(always)]
   pub(crate) fn enter_writer(&self) -> Option<crate::occ::WriterGuard<'_>> {
       let key = self.gate.id();
       let slot_id = if LAST_KEY.get() == key && LAST_SLOT.get() != usize::MAX {
           LAST_SLOT.get()
       } else {
           self.enter_writer_slow(key)?
       };
       self.gate.enter_writer(&self.writers.slots[slot_id], slot_id)
   }
   ```
2. **Outlined cold path (`enter_writer_slow`):**
   Marked `#[cold] #[inline(never)]`. Handles cache misses, slot allocation, and vector maintenance:
   ```rust
   #[cold]
   #[inline(never)]
   fn enter_writer_slow(&self, key: u64) -> Option<usize> {
       // CACHED_SLOTS lookup, allocate_slot(), updates LAST_KEY and LAST_SLOT
   }
   ```
3. **Concurrency discipline (AGENTS.md §2.1 invariant 5):**
   - Concurrency-only code stays out of shared inlined paths. Non-concurrent plain-tree mutation paths (`map_insert`, `set_insert`, `blobmap_insert`, etc.) never touch `enter_writer` and must gain 0 thread-local reads (`%fs:`) and 0 branch instructions.
   - Hot struct layout: if any struct definition is touched, it must use explicit `#[repr(C)]` with cold fields last, pinned by `core::mem::offset_of!` in a layout test.

### 35.3 Baseline instruction counts for context (CI run 37218971129)

Reference baseline: CI run **37218971129** (commit `5ee1e8e399b78ed5fb42d62547f3529eee331b3f`, push to main for PR #1361).
Measured on GitHub Actions runner (`ubuntu-24.04`, Valgrind 3.27.1).

All 63 `sync_*` arms recorded on main at commit `5ee1e8e399`:

| Benchmark Arm | Parameter | Measured Instructions (Ir) |
|---|---|--:|
| `sync_blobmap_churn` | `random:built_sync_blobmap("random")` | 143,334,832 |
| `sync_blobmap_compact` | `random:built_sync_blobmap_garbage("random")` | 19,436,930 |
| `sync_blobmap_compare_exchange` | `random:built_sync_blobmap("random")` | 52,661,355 |
| `sync_blobmap_get` | `random:built_sync_blobmap("random")` | 19,682,875 |
| `sync_blobmap_insert` | `random:keys("random")` | 58,997,021 |
| `sync_blobmap_insert_reclaiming` | `random:built_sync_blobmap_reclaiming("random")` | 812,952 |
| `sync_blobmap_overwrite` | `random:built_sync_blobmap("random")` | 34,411,863 |
| `sync_blobmap_remove` | `random:built_sync_blobmap("random")` | 67,503,310 |
| `sync_blobmap_remove_miss` | `random:built_sync_blobmap_misses("random")` | 19,221,107 |
| `sync_bytesmap_churn` | `routes:built_sync_bytesmap("routes")` | 304,005,440 |
| `sync_bytesmap_compare_exchange` | `routes:built_sync_bytesmap_rmw("routes")` | 130,533,586 |
| `sync_bytesmap_get` | `routes:built_sync_bytesmap("routes")` | 32,270,096 |
| `sync_bytesmap_insert` | `routes:str_keys("routes")` | 114,150,712 |
| `sync_bytesmap_overwrite` | `routes:built_sync_bytesmap("routes")` | 58,672,690 |
| `sync_bytesmap_remove` | `routes:built_sync_bytesmap("routes")` | 185,224,369 |
| `sync_bytesmap_update` | `routes:built_sync_bytesmap_rmw("routes")` | 102,419,586 |
| `sync_map_branchu_band` | `top:built_sync_map_band("top")` | 10,065,997 |
| `sync_map_churn` | `leaf:built_sync_map_leaf("leaf")` | 124,997,852 |
| `sync_map_churn` | `random:built_sync_map("random")` | 123,607,195 |
| `sync_map_compact_baseline` | `random60:drained_partial_sync_map("random60")` | 46,094,675 |
| `sync_map_compact_drained` | `random60:drained_partial_sync_map("random60")` | 20,549,879 |
| `sync_map_compare_exchange` | `random:built_sync_map("random")` | 41,764,777 |
| `sync_map_count_after_write` | `one_top_byte:built_sync_map_count("one_top_byte")` | 1,709,974,464 |
| `sync_map_count_after_write` | `random:built_sync_map_count("random")` | 32,139,090 |
| `sync_map_count_after_write` | `sequential:built_sync_map_count("sequential")` | 14,501,811 |
| `sync_map_count_locked` | `one_top_byte:built_sync_map_count("one_top_byte")` | 11,110,655 |
| `sync_map_count_locked` | `random:built_sync_map_count("random")` | 11,050,125 |
| `sync_map_count_locked` | `sequential:built_sync_map_count("sequential")` | 4,426,901 |
| `sync_map_drain_floor` | `top:built_sync_map_floor("top")` | 248,231 |
| `sync_map_get` | `leaf:built_sync_map_leaf("leaf")` | 7,554,521 |
| `sync_map_get` | `random:built_sync_map("random")` | 14,970,295 |
| `sync_map_insert` | `random:keys("random")` | 51,129,429 |
| `sync_map_next_after_scan` | `clustered:built_sync_map("clustered")` | 26,221,848 |
| `sync_map_next_after_scan` | `random:built_sync_map("random")` | 33,051,471 |
| `sync_map_next_after_scan` | `sequential:built_sync_map("sequential")` | 35,051,479 |
| `sync_map_prev` | `random:built_sync_map("random")` | 34,114,722 |
| `sync_map_prev_locked` | `random:built_sync_map("random")` | 57,226,942 |
| `sync_map_remove` | `random:built_sync_map("random")` | 64,379,580 |
| `sync_map_scan` | `clustered:built_sync_map("clustered")` | 4,595,253 |
| `sync_map_scan` | `random:built_sync_map("random")` | 10,331,730 |
| `sync_map_scan` | `sequential:built_sync_map("sequential")` | 4,593,400 |
| `sync_map_update` | `random:built_sync_map("random")` | 41,764,777 |
| `sync_map_write_twin` | `one_top_byte:built_sync_map_count("one_top_byte")` | 2,163,979 |
| `sync_map_write_twin` | `random:built_sync_map_count("random")` | 1,926,727 |
| `sync_map_write_twin` | `sequential:built_sync_map_count("sequential")` | 2,179,054 |
| `sync_set_churn` | `leaf:built_sync_set_leaf("leaf")` | 97,839,488 |
| `sync_set_churn` | `random:built_sync_set("random")` | 98,495,626 |
| `sync_set_compact_baseline` | `random60:drained_partial_sync_set("random60")` | 22,884,629 |
| `sync_set_compact_drained` | `random60:drained_partial_sync_set("random60")` | 19,101,885 |
| `sync_set_contains` | `leaf:built_sync_set_leaf("leaf")` | 7,104,526 |
| `sync_set_contains` | `random:built_sync_set("random")` | 14,804,794 |
| `sync_set_drain_floor` | `top:built_sync_set_floor("top")` | 251,841 |
| `sync_set_insert` | `random:keys("random")` | 47,271,375 |
| `sync_set_remove` | `random:built_sync_set("random")` | 55,982,089 |
| `sync_strmap_churn` | `routes:built_sync_strmap("routes")` | 192,580,541 |
| `sync_strmap_churn_short` | `short:built_sync_strmap_short("short")` | 237,502,750 |
| `sync_strmap_compare_exchange` | `routes:built_sync_strmap("routes")` | 123,741,610 |
| `sync_strmap_get_short` | `short:built_sync_strmap_short("short")` | 29,832,322 |
| `sync_strmap_insert` | `routes:str_keys("routes")` | 79,106,223 |
| `sync_strmap_insert_short` | `short:short_keys("short")` | 99,490,464 |
| `sync_strmap_insert_sorted` | `uuid:uuid_keys_sorted("uuid")` | 46,912,663 |
| `sync_strmap_remove` | `routes:built_sync_strmap("routes")` | 89,281,391 |
| `sync_strmap_update` | `routes:built_sync_strmap("routes")` | 74,308,922 |

### 35.4 Pre-registered Callgrind ceilings against PR merge base

Per AGENTS.md §6 and review discipline, all ceilings are expressed as **Δ% against the merge base** in the stage-2 PR's own `instruction-counts` CI run (the dual-pass comparison already run by CI), keeping the baseline figures above as historical context:

| Arm | Context Ir (`37218971129`) | Pre-Registered Ceiling vs Merge Base | Expected Delta | Rationale |
|---|--:|---|---|---|
| `sync_blobmap_overwrite/random` | 34,411,863 | **≤ +0.0%** | −6.8% (projected) (−2.35M Ir context) | Hot path inlining eliminates out-of-line call overhead |
| `sync_blobmap_insert/random` | 58,997,021 | **≤ +0.0%** | ≤ 0.0% | Fast path inlined; no regression permitted |
| `sync_blobmap_remove/random` | 67,503,310 | **≤ +0.0%** | ≤ 0.0% | Fast path inlined; no regression permitted |
| `sync_blobmap_compare_exchange/random` | 52,661,355 | **≤ +0.0%** | ≤ 0.0% | Fast path inlined; no regression permitted |
| All other 59 `sync_*` arms | — | **≤ +0.1%** | ≈ 0.0% | Review threshold per AGENTS.md §6 |
| Single-threaded plain-tree arms | — | **≤ +0.1%** | 0.0% | Control; non-concurrent paths must not move (AGENTS.md §2.1 invariant 5) |

**Falsifier:** Any regression exceeding +0.0% vs merge base on `sync_blobmap_overwrite`, `sync_blobmap_insert`, `sync_blobmap_remove`, or `sync_blobmap_compare_exchange`, or exceeding +0.1% on any plain-tree arm, marks the split **REFUTED** (no `allow-regression:` overrides permitted per work plan §0 rule 4). The expected −6.8% on overwrite is a (projected) figure, not an acceptance floor.

### 35.5 Soundness gates before measurement

1. **Gate G35.1 — Unwind safety:** `tests/test_serialised_section_unwind.rs` passes.
2. **Gate G35.2 — Loom race verification:** `loom_shared_enter_writer_quiescence` in `crates/expanse/src/sync.rs` passes under `cargo test --test ... --features loom`.
3. **Gate G35.3 — Concurrency isolation (AGENTS.md §2.1.5):** Plain-tree disassembly check in CI x86 job verifies strictly 0 `%fs:` thread-local reads in `mutate::insert_with_path_flat` and `mutate_map::insert_with_path_flat`.

### 35.6 What is measured if Build A returns

In README §26 (line 5400), Build A added a branch checking `has_writer_deltas` only when clear (`has_writer_deltas.load(Relaxed) == false`), adding 149,996 Ir (+0.44%) to `sync_blobmap_overwrite`. Build A was rejected in #1289 because the out-of-line `enter_writer` call had already pushed overwrite by +2.84%, causing Build A (+3.28%) to breach the +3% ceiling.

Once the hot/cold split inlines `enter_writer` and recovers the 2,350,000 Ir overhead, Build A may be evaluated under a follow-on pre-registration:
1. **Callgrind instruction counts:** With `enter_writer` inlined, Build A's +149,996 Ir is offset by the −2,350,000 Ir recovery, yielding a projected net delta of −2,200,004 Ir (≈ −6.4% (projected)) vs pre-#1289 baseline, easily satisfying the ≤ +0.0% ceiling.
2. **Wall-clock mixed scaling:** Build A is evaluated against Build C on the named cell: `SyncExpanseBlobMap`, 50% read mixed workload (`core_concurrency`), 16 threads, pin `0-15`, two independent runs (18 rounds per cell).
   - **Metric & Interval:** Ratio of mean total ops/s ($\text{Build A} \div \text{Build C}$), computed with a two-sample BCa 95% confidence interval (`bca_bootstrap_ratio_ci_with_method`) over the 18 paired window iterations in each run.
   - **Pre-registered Floor:** BCa 95% CI lower bound $\ge 1.15$ `(target)`.
     - *Derivation & context:* README §26 R2 (quoted as historical context only) measured $\text{A} \div \text{C}$ at $1.232$ [1.217, 1.247] in run 1 and $1.246$ [1.234, 1.260] in run 2 on this cell. Per BENCHMARKING rule 18, within-run intervals do not bound between-run spread, so the exact level ($1.23\times - 1.25\times$) cannot be assumed to replicate. The floor of $1.15$ `(target)` is derived by applying an operational dispersion margin of $0.067$ below the historical lowest BCa lower bound ($1.217 - 0.067 = 1.150$), requiring a statistically bounded win $`> 15\%`$ that sits $> 3\times$ above the $`\pm 5\%`$ control drift threshold (`scripts/blob_peak_bounds.py::derive_build_a_speedup_floor`).
   - **Pass Rule:**
     - `PASS`: the BCa 95% CI lower bound exceeds $1.15$ `(target)` in **both** run 1 and run 2 ($L_1 \ge 1.15$ and $L_2 \ge 1.15$).
     - `REFUTED`: the BCa 95% CI upper bound is below $1.15$ in both runs ($U_1 < 1.15$ and $U_2 < 1.15$).
     - `INCONCLUSIVE`: either run has its CI spanning the floor ($L_k < 1.15 \le U_k$), or the two runs disagree.
   - **Cross-Run Rule (`docs/BENCHMARKING.md` rule 18):**
     - A speedup claim is licensed **only when both runs move the same way** (both runs show positive effect with BCa lower bounds $L_k \ge 1.15$ in the same direction). If one run passes while the other is flat or inverted, or if any control cell (`blob` 100% read at $T=16$, `map` 50% read at $T=16$) drifts outside $`\pm 5\%`$ in both runs, no speedup is claimed and the outcome is marked `INCONCLUSIVE` or `DRIFT`.

---

## Errata to locked sections (appended 2026-10-05; the sections above are not edited)

Each row corrects a reference or a statement of fact in a locked section. None changes a hypothesis, a threshold, a cell, a decision rule or a void condition.

| Section | As written | Correction |
|---|---|---|
| §31.2 | "Rule 12 / §8.8 commit 1" | `AGENTS.md` §8.8, commit 1. There is no "Rule 12" in this repository. |
| §31.7 | "rule B-12 and AGENTS §1.6 (no post-hoc threshold redesigns to force a pass)" | `AGENTS.md` §8.19. Neither "B-12" nor an `AGENTS.md` §1.6 exists. |
| §31 header | "locked before any engine code of the change below" | The G2 ceilings of §31.5 were amended once after the first lock and before any engine commit: commit `5f2776ac2` on the branch of #1344 replaced "within 0.1%" on four arms with tiered ceilings. The disclosure was in the PR body only. |
| §32 header | "locked before any batch-cursor engine code" | §32 was first written in commit `ab5cba7ef` and amended in `9e161661f` (the buffer sizing, the scan-semantics wording and the prediction text, reconciled with `scripts/olc_bounds.py`), both before any engine code. The disclosure was in the body of #1351 only. |
| §32, §33 numbering notes | "per work plan §0 rule 11" | Section numbers were assigned at merge. The work plan is not a document of this repository. |
| §34 numbering note | "#1373 (A6, thread-local freelist allocation stripe)" | #1373 pre-registered validated ordered reads and a batch cursor on `StrReader` (§33). It has nothing to do with freelists. |
| §35.5, G35.2 | "passes under `cargo test --test ... --features loom`" | There is no `loom` feature. The model runs under `RUSTFLAGS="--cfg loom" cargo test -p expanse-trie --release loom_`, the `Loom Concurrency Race Model` CI job. |
| §35.4 | "All other 59 `sync_*` arms" against a falsifier naming only the four blob arms and the plain-tree arms | The table and the falsifier sentence disagree on whether another `sync_*` arm above +0.1% refutes the split. #1378 was held to the table: it merged with no arm above +0.1% (CI run 37281909211). |

## 34b. Hold on §34 — what the registered design cannot decide (appended 2026-10-07, before any §34 verdict; §34 and §34a are not edited)

A review of the §34 design, made after four dispatches and before any evaluation, found that the registered cells cannot support the causal reading §34.2 asks of them. No §34 verdict is recorded, and no further §34 run is taken, until an amendment registers a design that can. `concurrency_blob_peak` is declared unavailable in `.github/bench-suites.json` for that reason.

### 34b.1 The runs taken

| Dispatch | Pin | Commit | Outcome |
|---|---|---|---|
| 37477330005 | `0,2,4,6,8,10,12,14` | `052b0a2a0` | completed; host guard quiet; artifact `ADMISSIBLE`, not evaluated |
| 37478793335 | `0,2,4,6,8,10,12,14` | `052b0a2a0` | completed; host guard quiet; artifact `ADMISSIBLE`, not evaluated |
| 37480277219 | `0-15` | `052b0a2a0` | discarded by the host guard as it then was (§8.17); the guard's rule has since changed |
| 37481727076 | `0-15` | `052b0a2a0` | discarded, as above |

None of the four artifacts is committed. The two discarded runs stay discarded: the guard's rule was changed after them, not applied to them. The two completed runs are throughput measurements under an uncontrolled placement (34b.2) and are not evidence for or against any §34.2 hypothesis.

### 34b.2 The gaps

1. **Thread placement is neither controlled nor recorded.** The pin is the process's affinity mask (`scripts/bench_pin.py`); the harness spawns plain threads and sets no per-thread affinity, and placement is captured for the `sync32` arm only. §34.2 says the physical pin "pins each worker thread to an independent physical P-core". It does not: it prevents two threads from sharing a core and lets them migrate among the eight. On `0-15`, whether two of the four workers of the four-thread cell share a core varies by window and is not in the artifact. The comparison's independent variable is therefore unobserved.
2. **No cell separates "more than eight writers" from "two threads on one core".** Sixteen threads exist only with SMT and eight only without it. If throughput rises to eight threads on the physical pin and falls at sixteen on `0-15`, the two explanations coincide and the design cannot tell them apart.
3. **Steps 3 and 4 have no instrument in the dispatched suite.** `mixed_concurrency.py --blob-peak` runs no `perf` and the harness opens no counters, so frequency droop, memory bandwidth and coherence traffic cannot be read from these runs.
4. **Compaction is a serial term the decision rules do not carry.** One worker compacts in-window and quiesces every writer. §34.1 shows compaction is not the only cause; its share of each window is recorded (`compaction_time_share`) and no rule uses it, so a serial fraction fitted to these cells includes the harness's own reclamation.
5. **The §34.4 void rule tests one cell total.** It requires at least one compaction over all windows of the sixteen-thread cell on `0-15`, and nothing on the physical pin, where the four-against-eight comparison depends on compaction in the same way.

### 34b.3 What an amendment must register before another run

- each worker pinned to one CPU, and the placement recorded per window for the `blob` arms;
- cells that separate thread count from core sharing (for example eight threads on four cores against eight on eight);
- the counters §34.2's Steps 3 and 4 name, attached to the same windows, in groups the P-core PMU can co-schedule;
- compaction as a budget line: the fit reported on raw throughput and on throughput over the window less compaction time;
- a compaction condition on every `blob` 50% cell of both pins.

The thresholds of §34.2 are not changed by this note. Runs taken under the amendment use fresh seeds and are evaluated only against it.


## 36. Pre-registration for #1194 — validated ordered reads on the shared set (appended 2026-10-12, locked before any engine code of the change below; next free section, may be renumbered at merge)

The question (#1194, *Ordered reads on the set and string map*; design record #900): can `SyncExpanseSet`'s reader handles answer `first`, `last`, `next_at_or_after`, `next_after`, `prev_at_or_before` and `prev_before` by the validated walk the map uses, instead of through `with_locked`, and do its readers gain from it? §12.6 excludes the set from the map's registration, so nothing in §12 decides it. This section is commit 2 of the cadence (AGENTS.md §8.8): it holds no engine code and no new data. Every code reference is to `origin/main` at `b4e153475`. Refs #1194, #900.

### 36.1 Scope and sequence

**In scope.** Six methods, each returning `Option<u64>` as the single-threaded set does (`crates/expanse/src/set.rs:1785-1837`), on the three handle forms the set has (`crates/expanse/src/sync.rs:6349`, `:6403`, `:6428`):

| Type | Methods | Note |
|---|---|---|
| `SetReader<'_>` | `first`, `last`, `next_at_or_after(key)`, `next_after(key)`, `prev_at_or_before(key)`, `prev_before(key)` | borrows the set |
| `OwnedSetReader` | the same six | holds the `Arc` |
| `DetachedSetReader` | the same six, each taking `&SyncExpanseSet` first | the per-call collector identity check `SyncExpanseSet::registered` (`sync.rs:6163`) runs before the first pin, as it does for `contains` |

`SyncExpanseSet` itself gains no one-shot ordered method; `SyncExpanseMap` has none either: `.github/public-api/expanse-trie.txt` lists the map's six on `MapReader`, `OwnedMapReader` and `DetachedMapReader` only.

**Sequence, in order, one gate each.**

1. **PR M — exact-size node allocation under Miri.** Under `cfg(miri)`, `NodeAlloc` serves every block from the system allocator at exactly the requested size, zeroed only where the engine asked for zeroed memory, in place of slab pages. It applies to every Miri lane, not to a set-only feature. *Depends on:* nothing. *Gate:* every Tier-1 shard, every nightly Tier-3 shard and the `miri-ub-sites` census stay green; or each report the mode surfaces is listed in the PR and handled. A report that shows a defect already on `main` is a finding for the maintainer, with its reproduction, and is never suppressed or ignored to get a lane green. Per-node allocations run slower under Miri, so the shard budgets may need re-balancing (`scripts/check_miri_shards.py`; `docs/CI.md` §5, Miri tiering); that cost is read from the CI runs of PR M and of a dispatched nightly, never from a local run. **Whether the mode fits inside `cfg(miri)` alone is unverified until this PR.** If it does not, PR M says what else it had to touch, and P36.1 applies to it. (superseded by §36.11)
2. **PR T — the tiny-set Callgrind arms** on the locked route (§36.5), registered in `crates/expanse/benches/instructions.rs`, in the ops maps of `scripts/perf_report.py`, and in the arm inventory by `python3 scripts/check_bench_shapes.py --write`. Bench only. *Depends on:* nothing; may land beside PR M. *Gate:* `check_bench_shapes.py` green and the arms' own result-count assertions.
3. **PR A — the §36.3 ledger turned into tests.** It may touch `sync_nav.rs` and `sync.rs` under `cfg(test)`, `debug_assert!` and `occ-stats`-gated counter bumps only, plus test modules and test targets. No public API and no non-test instantiation of the set form. (superseded by §36.13) *Depends on:* PR M (G36.2's third part and G36.6 need its allocation mode) and PR T (its arms are in P36.1's base). (superseded by §36.11) *Gate:* G36.2 to G36.6 and G36.9, with P36.1.
4. **PR C1 — suite wiring** that the dispatch reads from `main`: the `.github/bench-suites.json` entry, the dispatch `case`, flag spelling and upload paths in `bench_baremetal.yml`, and the driver sweep. No Rust that names the new methods. *Depends on:* nothing in code; lands before the runs. *Gate:* `check_bench_suites.py` and the driver's self-tests.
5. **PR B — the engine.** The eighteen public methods, on all three handle forms (borrowed, owned, detached), through one macro (§36.5); the optimistic Callgrind arms; the histories of G36.1; the concurrent Miri workloads; G36.7; the eight foreign-reader tests of G36.8; the public-API snapshot; `docs/ARCHITECTURE.md` §4.1 and `docs/COMPAT.md`; and the harness's reader mode for `--arm set` (§36.6). **Held unmerged** until the verdict. *Depends on:* PR A, PR T and PR C1 on `main`. *Gate:* every G36 gate, with P36.1 and P36.2.
6. **The runs and the verdict.** Dispatches of the suite on `main` with `ref` set to PR B's full 40-character head SHA; the outcomes are appended here as a new subsection. *Depends on:* PR B's gates met on that SHA. *Gate:* P36.3, P36.4, P36.5 and §36.8.

**Named exclusions.** Each needs its own registration and is not decided here:

- a batch cursor on the set (`SyncSetCursor`; the map's is §32);
- the 32-bit set (`sync32.rs`, `SyncExpanseSet32`), whose readers return `Busy` instead of falling back (§12 decisions) and whose wall clock is measured on-device;
- the C ABI and the Python, Node and WebAssembly surfaces, which are registered separately later (`include/expanse.h` has `expanse_sync_set_reader_contains` and no ordered entry point for the set);
- rank and select on the shared set, and any change to a write path.

### 36.2 What is already true, verified at lock time

- **The walk is shared and generic.** `sync_nav::next_validated` and `prev_validated` take `const MAP: bool` (`crates/expanse/src/sync_nav.rs:271`, `:307`), as do `root_leaf` (`:386`), `next_in` (`:433`), `prev_in` (`:678`) and `bitmap_leaf` (`:991`).
- **The set form has run, on one shape.** `MAP = false` is instantiated in exactly one place, a unit test (`sync.rs:26016-26036`, `set_branch_sample`), which drives the two entry points over the #1466 fixture: a chain of single-digit `BranchL3`s above a level-2 `BranchL3` with two 17-key `Leaf1` children (`sync_nav.rs:1060-1068`), plus the one-key immediate the armed insert creates. The only non-test callers pass `true` (`sync.rs:10418`, `:10429`), so no build other than the test build contains the set form. What has never run is the set form over a root leaf, a multi-key set immediate on the answer path, a `Leaf2`..`Leaf7`, a `LeafBitmap1`, a `FullExpanse`, a `BranchB` or a `BranchU`, and any concurrent history.
- **The retry protocol is shared.** `Shared::optimistic_read` (`sync.rs:3144-3179`) counts the operation, makes up to `MAX_RETRIES` = 64 attempts (`sync.rs:165`) and then answers under `read_locked`. The set publishes its root (`PUBLISHES_ROOT = true`, `sync.rs:777`), so a reader takes the root from the tree head (`sync.rs:3158-3159`).
- **The read-set bound carries over, and overflow is a retry.** `ReadSet::sample` is called in the three branch arms only (`sync_nav.rs:544`, `:589`, `:638` and `:791`, `:840`, `:893`), and the set uses the same four branch forms as the map, so the bound of 13 retained versions (`scripts/olc_bounds.py::ordered_read_set_branches`, §12.1) and `READ_SET_CAP` = 16 (`sync_nav.rs:46`) apply unchanged (derived from those call sites). A seventeenth sample returns `Err(Retry)` (`sync_nav.rs:103-105`); 64 such attempts end in the locked fallback. On a consistent tree that cannot happen while the bound holds, and G36.3 requires a fixture that reaches 13.
- **No emptied child stays linked.** An optimistic removal that empties a child nulls a `BranchU` slot under that branch's lock (`sync.rs:1276-1311`), and a child emptied under a linear or bitmap branch falls back to the exclusive removal (`sync.rs:3474-3483`). §33.7.5's open case for the string map has no counterpart here.
- **How a shared set is built.** `SyncExpanseSet::new` is the only constructor (`sync.rs:4146`; the public-API snapshot lists no `From`), and `compact` rebuilds through `ExpanseSet::from_sorted_keys_into` (`sync.rs:6250-6262`).
- **Node memory is slab-carved.** A node of at most 256 bytes is cut from a 4 KiB page obtained with `alloc_zeroed` (`crates/expanse/src/alloc.rs:181`, `:197-199`, `:1121-1129`), and `alloc.rs` has no `cfg(miri)` path. A read past a small node therefore stays inside one initialised allocation, and neither Miri nor ASan reports it. G36.2 is built on that fact. (superseded by §36.11)
- **The comparator exists.** The Callgrind arms of #1468 measure the locked route, and nothing else, today:

  | Arm | What it measures now |
  |---|---|
  | `set_nav/{random,sequential,clustered}` | `ExpanseSet::next_at_or_after` from each present key, single-threaded, no wrapper |
  | `set_prev/{random,sequential,clustered}` | `ExpanseSet::prev_before` from each present key, single-threaded |
  | `sync_set_prev_locked/random` | `SyncExpanseSet::with_locked(\|s\| s.prev_before(k))` per present key (`crates/expanse/benches/instructions.rs:3125`) |
  | `sync_set_next_after_scan_locked/{random,sequential,clustered}` | a full ascending scan, `first` then `next_after`, one `with_locked` per call (`instructions.rs:3154`) |

  For scale, `sync_set_prev_locked/random` counted 1,161.2 instructions per operation and `sync_set_next_after_scan_locked/random` 1,144.4 *(measured: GitHub-hosted ubuntu x86_64 runner, `8d27a968c`; CI run 38276056419; workload: `core_instructions`)*. The names `sync_set_prev` and `sync_set_next_after_scan` are unused and reserved for the optimistic reads.
- **The #1466 order holds in the shared arm.** A linear branch's version is reached by field projection (`sync_nav.rs:197-207`), sampled (`:544`, `:791`), and only then is its header loaded (`:551`, `:798`). `sync::ordered_read_tests::ordered_read_samples_a_linear_branch_before_its_header` (`sync.rs:26067`) and its negative control (`sync.rs:26098`) cover the set form by the direct call above.

### 36.3 The site ledger

How it was built: every occurrence of `MAP` in `sync_nav.rs` was read (31 lines), then each arm of `next_in` and `prev_in` was compared with the matching arm of the single-threaded `nav::next` / `nav::prev` (`crates/expanse/src/nav.rs:128`, `:354`) and of the lookup walk `walk_validated_body!` (`sync.rs:231-610`), then the wrapper path of the map's ordered reads (`sync.rs:10412-10482`) was read for what a set wrapper must write differently, then the set's writer entry points were read for operations that replace what a reader holds. Result: **12 flavour conditionals, 12 flavour-selection sites, 7 shared-code sites that carry a set-specific assumption, 5 wrapper sites and 5 writer-side operations — 41 entries**.

**Class A — the `if MAP` conditionals** (`verified` at each line). "Inverted" means the map arm running on a set node. The last column is what an observer sees; the set wrapper drops the walk's value word, so a wrong value alone changes no public answer.

| # | `sync_nav.rs` | Set node form | If inverted | Visible as |
|---|---|---|---|---|
| A1 | `:412` (`root_leaf`) | root leaf: sorted `u64` keys only (`set.rs:1322`) | loads a value word at `map::leaf_values_offset(pop)`, past the key array | a non-zero value word, or nothing: inside a slab page the read is not reported (superseded by §36.11) |
| A2, A7 | `:451`, `:696` | set immediate: up to 15 payload bytes across word 0 and the aux bytes (`node.rs:214`) | decodes keys from the 7 aux bytes only (`mutate.rs:307`) | a bounds-check panic when count × key bytes exceeds 7, wrong keys otherwise |
| A3, A8 | `:460`, `:704` | set immediate: word 0 is key bytes | one key: returns word 0 as the value. Two or more keys: treats word 0 as a value-array pointer and loads through it (`:1056`). That load is reached only when 2 ≤ count × key bytes ≤ 7; above 7 the A2/A7 panic comes first if it is inverted too | a non-zero value word; or a wild dereference |
| A4, A9 | `:490`, `:734` | set linear leaf: keys start at the allocation base | searches keys at `base + leaf::map_keys_offset(pop)` | wrong keys; out of bounds only where the node is its own allocation (superseded by §36.11) |
| A5, A10 | `:503`, `:750` | set linear leaf: no value area | loads word `slot` at the base, which is key bytes for a low slot and past the key area (`kb * cap_class(pop)` bytes, `leaf.rs:804`) for a high one | a non-zero value word, or nothing |
| A6, A11 | `:525`, `:772` | `FullExpanse`, set flavour only | answers `Err(Retry)` on every attempt | 64 attempts and a fallback, on a tree nothing is changing |
| A12 | `:997` (`bitmap_leaf`) | `LeafBitmap1`: 64 bytes, a 32-byte bitmap, then `version` and padding (`node.rs:905-911`) | reads it as `LeafBitmapL`: loads `values[d >> 5]` at offset 32 + 8·(d >> 5). For `d < 128` that word is `version` or padding inside the node; for `d >= 128` it is past the node. A null word returns `Err(Retry)` (`:1021-1023`); any other is dereferenced (`:1028`) | a retry on a quiescent tree, or a wild dereference |

Eight conditionals read memory the set node does not have if inverted (A1, A3, A4, A5, A8, A9, A10, A12), over four node forms: the root leaf, the immediate, the linear leaf and the bitmap leaf. For three of them (A1, A5, A10) no sanitizer and no key-only differential sees the read; G36.2 exists for those. (superseded by §36.11)

**Class B — where the flavour is selected** (`verified`).

| # | Site | What must hold |
|---|---|---|
| B1–B4 | `sync_nav.rs:285`, `:288`, `:320`, `:323` | the entry points pass their own `MAP` to `root_leaf` and to `next_in` / `prev_in` |
| B5, B6 | `sync_nav.rs:520`, `:767` | the `LeafB1` arm passes `MAP` to `bitmap_leaf` |
| B7–B12 | `sync_nav.rs:575`, `:613`, `:662`, `:826`, `:868`, `:917` | every recursion passes `MAP` through |
| — | `sync_nav.rs:364`, `:370`, `:372` (`seek_submap`) | hard-codes `true`; it is the string map's entry, and no set path may reach it |
| — | the wrapper call PR B adds | `::<false>`, and nowhere else outside tests; the map's stay `::<true>` (`sync.rs:10418`, `:10429`) |

**Class C — shared code with no `MAP` branch** (`verified`), which set trees reach for the first time or under an assumption the walk does not check.

| # | `sync_nav.rs` | Set-specific point | Required of PR A |
|---|---|---|---|
| C1 | `:448`, `:693` | an immediate's key width must equal the level; set immediates hold up to 15 one-byte keys where the map holds 7 (`docs/ARCHITECTURE.md` §10.4) | a test with a full 15-key set immediate on the answer path, both directions |
| C2 | `:478-488`, `:722-732` | leaf width, skipped-prefix compare and `edge.pop0(kb)` are flavour-independent | a narrow-pointer set leaf (key bytes below the slot level) in the differential (superseded by §36.13) |
| C3 | `:497`, `:502`, `:743`, `:749` | `leaf::shared_keys::lower_bound` and `read` take a key area of `kb * cap_class(pop)` bytes at an 8-aligned base (`leaf.rs:801-806`, `:862-880`); for a set leaf that base is the allocation itself. `lower_bound` and `find` are both `seek` (`leaf.rs:1028-1043`), and `contains` already runs `find` on set leaves (`sync.rs:542`), so the search is exercised; the `read` of the slot it returns is what this walk adds | set leaves of every key width 1..=7 at populations 1, a class boundary and the cap, under G36.2's allocation mode (superseded by §36.13) |
| C4 | `:512-522`, `:759-769` | a `LeafB1` behind a narrow pointer | a skipping set bitmap leaf in the differential |
| C5 | `:1031-1040` | the set bitmap arm loads up to four bitmap words separately, with no holder check between them and no park point; only the final validation covers it | a park point between word loads; a writer armed there must force a retry (superseded by §36.13) |
| C6 | `:524-528`, `:771-775` | the arm answers from the suffix alone and has no skipped-prefix compare. Its arithmetic is right at any level; it is wrong for a full expanse behind a narrow pointer. On a shared set the form is created at level 1 without a skip (`mutate.rs:1056`, `:1897`; `algebra_build.rs:87-88`). Other producers exist on plain sets: the warm insert path (`set.rs:815-850`, which asserts it is never armed on a shared tree) and the algebra builders, which make full expanses at any level (`algebra_build.rs:40`, `:1443`, `:1559-1570`); `SyncExpanseSet::new` being the only constructor keeps those out today | tests that read at, below and above a level-1 full expanse and across two adjacent ones; a `debug_assert!` that the edge's population field equals the full count of the level it is read at; a note in the constructor's rustdoc that sharing a pre-built set would bring multi-level full expanses under this walk |
| C7 | `:386-426` | the root-leaf search has no park point in this walk (the lookup walk has `Site::RootLeafProbe`, `sync.rs:265`) | a park point, and a test in which a writer changes the root leaf at it and the read retries |

**Class D — the wrapper** (`verified` against the map's wrapper; the set's does not exist yet).

| # | Map reference | What the set differs in |
|---|---|---|
| D1 | `sync.rs:10412-10431` | the answer is `Option<u64>`: the walk's `(key, 0)` is projected to the key |
| D2 | `sync.rs:10419`, `:10430` | the locked fallback calls `ExpanseSet::next_at_or_after` / `prev_at_or_before` (`set.rs:1797`, `:1819`) under `read_locked` (`sync.rs:3000`) |
| D3 | `sync.rs:10467`, `:10479` | `next_after(u64::MAX)` and `prev_before(0)` answer `None` before `optimistic_read`, so they count no read operation |
| D4 | `sync.rs:6163` | every `DetachedSetReader` ordered read runs `registered` before its first pin, and before D3's early return |
| D5 | `sync.rs:777`, `:3158-3159` | the root comes from the published head, never from the engine |

**Class W — writer-side operations that replace what a reader holds** (`verified`). No code changes here; each must appear in a G36.1 history.

| # | Operation | What it does to a reader in flight |
|---|---|---|
| W1 | `compact` (`sync.rs:6250`) | builds a second tree and publishes its root under the tree word (`write_quiesced_staged`, `sync.rs:2907`) |
| W2 | `clear` (`sync.rs:6112`) | empties the root under the tree word (`write_root_covered_exact`, `sync.rs:2788`) |
| W3 | root leaf to tree, on insert (`sync.rs:4164-4168`) | the root state changes form under the tree word |
| W4 | tree to root leaf or empty, on remove (`sync.rs:4238-4242`) | as W3, in the other direction |
| W5 | exclusive removal from a full expanse (`mutate.rs:2725-2740`) | materialises a `LeafBitmap1` of 256 bits and rewrites the slot under the parent's word |

### 36.4 Correctness gates

No cell of §36.6 is read until every gate below passes on the head measured. Each test named here is new unless a line number says otherwise; a name may change in review, its content may not. Every test command quoted in PR A or PR B states its non-zero test count (AGENTS.md §8.11.8).

- **G36.1 — differential against `BTreeSet` under concurrent writers.**
  - *Harness.* The whole-map checker of `crates/expanse/tests/linearizability.rs` (`check_ordered_linearizability`, `:1057`; its own negative control `:1121`). The set plugs in through an adapter, not a second checker: a set history is recorded as `OrdEvent`s with every value 0, `insert` returning `true` as `OrdRet::Insert(None)` and `false` as `OrdRet::Insert(Some(0))`, `remove` likewise, and an ordered answer `k` as `OrdRet::Found(Some((k, 0)))`. The model is a `BTreeMap<u64, u64>` whose values are all 0, which is a `BTreeSet<u64>`. The checker gains two operations, `Clear` (the model becomes empty) and `Compact` (the model is unchanged), each with a rejecting case beside the existing negative control.
  - *Histories,* each a twin of `optimistic_ordered_history` (`:1247`) on a `SyncExpanseSet`, 24 rounds, at most 64 events: the tree-rooted and hot-spot shapes of G12.2 (`:1310`, `:1333`); a root-leaf history that crosses `ROOT_LEAF_CAP` in both directions (W3, W4) (superseded by §36.14); a bitmap history (one terminal byte prefilled above `LEAF1_CAP`, written across `LEAFB1_DOWN`) (superseded by §36.14); a full-expanse history (one terminal byte prefilled with 250 keys, the writers inserting and removing the last six, so the leaf converts to `FullExpanse` and back through W5); and a history in which one thread interleaves `compact` and `clear` with the others' reads (W1, W2).
  - *Forms visited.* G36.3's visit counters, read as deltas over the whole history in a test binary of its own, must show each form the history is registered for visited at least once: both `LeafBitmap1` and `FullExpanse` for the full-expanse history, which changes form on purpose and so cannot be checked by tree state before and after.
  - *What these histories can and cannot show.* They are live and probabilistic. Their discriminating power is not assumed: PR B runs each of them once with `set_drop_child_snapshots` on (`sync.rs:23758`) and once with `set_header_before_sample` on (`sync.rs:23800`) in the reader threads, and records how many of the 24 rounds the checker rejects. That count is reported and not gated. (superseded by §36.14) The deterministic gates G36.2 and G36.4 are what must fail on a defect.
  - **Falsifier:** any round the checker rejects on the unmodified walk, or a form never visited.
- **G36.2 — every flavour conditional has a mutation that fails.** The mechanism has three parts, because no single one sees all twelve.
  - (i) *The value word is checked.* Every PR A test calls `next_validated::<false>` / `prev_validated::<false>` directly and asserts the value word is 0, on fixtures whose keys have a non-zero low word and a non-zero byte in every position, so that key bytes read as a value are non-zero. The set macro of PR B carries `debug_assert_eq!(v, 0)` before it drops the word, so every debug-build test through the public methods keeps the check.
  - (ii) *A quiescent read never retries.* The direct calls assert `Ok` on the first attempt, and G36.3's counters assert it through the wrappers.
  - (iii) *Exact-size nodes under Miri* (PR M, §36.1). Each node is then its own allocation, and Miri reports a read past it or of bytes never written. If PR M could not deliver the mode, A1's row falls back to part (i) alone and is recorded as review-enforced. (superseded by §36.11)
  - *Chosen over making the value loads untypable for the set.* Moving the flavour from a const generic to a trait with per-flavour node views would change the source the map's instantiation compiles from, which puts P36.1 at risk on the map's arms, and a wrong body can still be written inside the set's implementation. The mutation table is needed either way.
  - *The table.* For each row PR A inverts that one conditional for the set instantiation, records the test that fails and where, and restores it (AGENTS.md §5, one negative control per clause). The demonstrations are recorded in the PR; they are not kept as running tests, because nine of them execute a read the language does not define.

    | Conditional | Fixture | Must fail by |
    |---|---|---|
    | A1 `:412` | root leaf at populations 1, 8 and 31 | (iii) a Miri report; (i) where the word read is non-zero |
    | A2 `:451`, A7 `:696` | immediates of 1, 2 and 15 one-byte keys; 2 seven-byte keys | wrong key against the model, or the panic |
    | A3 `:460`, A8 `:704` | a one-key immediate; a 7-key one-byte immediate | (i) for one key; a Miri or ASan report for seven |
    | A4 `:490`, A9 `:734` | `Leaf1`..`Leaf7` at populations 1, a class boundary and the cap (superseded by §36.13) | wrong key against the model; (iii) |
    | A5 `:503`, A10 `:750` | the same leaves, answers at slot 0 and at the last slot | (i) at slot 0; (iii) at the last slot |
    | A6 `:525`, A11 `:772` | one full expanse, probes inside it | (ii) |
    | A12 `:997` | a `LeafBitmap1`, answers at digits below and above 128 | (ii) below 128 where the word is null; a Miri or ASan report otherwise |

  - **Falsifier:** an inverted conditional under which every PR A test still passes.
- **G36.3 — quiescent differential with the optimistic path proven, and the forms the walk visited.**
  - *Visit counters.* The walk bumps one `occ_stats` counter per terminal arm it answers from or passes over: root leaf; immediate, and immediate at its capacity; linear leaf by key width 1..=7, and a leaf behind a narrow pointer; bitmap leaf, and one behind a narrow pointer; full expanse; plus a high-water mark of the read set's length. `occ_stats::bump` is a no-op without the `occ-stats` feature (`crates/expanse/src/occ_stats.rs:351-357`), so the default build is unchanged. The counters record what the walk visited, which `ExpanseSet::stats().node_counts` cannot: that census has no per-width leaf count, no narrow-pointer count and no immediate population, and describes the tree, not the read.
  - *The test.* A new integration target gated on `occ-stats`, holding one `#[test]`, on the pattern and for the reasons of `crates/expanse/tests/test_occ_stats_readers.rs:1-27`; (superseded by §36.13) `scripts/test_occ_stats.sh` discovers it and fails a binary that runs zero tests. On quiescent sets (root leaf; tree over each `keys` distribution; a dense set holding bitmap leaves and full expanses; a set left by removing most of a larger one through `SyncExpanseSet::remove`; a set after `compact`), every one of the eighteen methods is compared with `BTreeSet` over present keys, absent keys, 0, `u64::MAX`, `next_after(u64::MAX)` and `prev_before(0)`.
  - *Counters.* Over a block of calls of which `w` reach the walk (all but `next_after(u64::MAX)` and `prev_before(0)`, D3), the deltas of `occ_stats::snapshot()` must be `Stat::ReadOps` = `w`, `Stat::ReadAttempts` = `w` and `Stat::ReadFallbacks` = 0. `ReadFallbacks` is bumped at one place on this path, immediately before `read_locked` (`sync.rs:3177`; defined `occ_stats.rs:57`), `ReadOps` and `ReadAttempts` at `sync.rs:3151` and `:3153`. Every visit counter of the list above must be non-zero over the whole test.
  - *The bound is reached.* One fixture, with branches at every level on both the probe's path and its sibling's, backtracks at level 8; the read set's high-water mark must read exactly 13 and the read must answer on its first attempt.
  - **Falsifier:** a mismatch with the model; a non-zero `ReadFallbacks`; `ReadAttempts` above `ReadOps`; `ReadOps` other than `w`; a visit counter at zero; a high-water mark other than 13 on the bound fixture. Without the counter clauses this test passes on a build whose every answer comes from the lock.
- **G36.4 — the #1466 order, per node form, each with a mutation that must fail.** The rule: the version that covers a load is sampled before that load. A mutation that loads a whole node early yields a stale but consistent view, which is linearizable and cannot fail; each mutation below moves one load only, so that it pairs with loads taken after the sample.

  | Form | Order today | Mutation | Status |
  |---|---|---|---|
  | `BranchL3`, `BranchL7` | sample `:544` / `:791`, header `:551` / `:798` | the `set_header_before_sample` switch (`sync.rs:23800`): digits from before, edges from after | exists for the set by direct call (`sync.rs:26098`); PR B re-points it at `SetReader` |
  | `BranchB` | sample `:589` / `:840`, digit search `:605` / `:856`, child `:610` / `:865` | take the digit (`next_set` / `prev_set`) before the sample and leave `branch_b_child`, which computes the rank and loads the subarray (`:953-981`), after it. The writer removes that digit's key and inserts under a neighbouring digit in place; the old digit then composes with the neighbour's child (superseded by §36.13) | new |
  | `BranchU` | sample `:638` / `:893`, edge `:643` / `:898` | load the probe digit's child edge before the sample. The writer inserts below that child and then below its sibling; the walk finds nothing in the stale child, backtracks, and answers from the sibling's new key | new |
  | root leaf, top edge | tree version `sync.rs:3155`, root copy `sync.rs:3158-3165` | a source scanner over `optimistic_read`: the sample precedes the root load. Nothing pins this order today. The scanner is shown red with the two statements swapped | new |
  | immediates, linear leaves, `LeafBitmap1`, `FullExpanse` | carry no version; covered by the holder, which is sampled before the edge that names them is loaded | covered by the three branch rows and by C5's park point | — |

  Each mutation is run on the set form in both directions, on a fixture that asserts its own shape as `branch_sample_fixture` does (`sync_nav.rs:1088-1124`), and must make the read return a key the model never held. The fail-then-pass run is recorded in the PR with the panic site.
  - **Falsifier:** a mutated walk that still returns the model's answer. A row may be recorded `not discriminable` only after PR A has tried the mutation exactly as worded above and recorded the attempt; a source scanner over that arm (sample before the first load, with its own mutation demonstration, AGENTS.md §5 item 2 d) then enforces it.
  - Also in this gate: the park-point tests of C5 and C7; the set twins of the backtrack reproducer G12.1 (`sync.rs:25797-25866`) in an insert and a remove variant; and `tests/test_validated_answers.rs` staying green with its entry-point scan (`:185`) and its one-`node_sample` clause.
- **G36.5 — Loom, and what it covers.** Models in `sync_nav::loom_tests`, built as the existing ones are (`sync_nav.rs:1143-1326`): the real `ReadSet::sample` and `validate_all` over model version cells, with the search itself written by hand over atomics.
  - (a) *Bitmap terminal.* A holder branch and a set bitmap leaf of two words. A successor search finds word 0 empty and takes word 1's minimum; the writer, under the holder's bracket, sets a bit in word 0 and then a lower bit in word 1. Negative control: the same search without the final validation returns the bit that was never the successor.
  - (b) *Terminal replaced.* The holder's slot is rewritten from a bitmap-leaf edge to `FullExpanse` and back under the holder's bracket while the search holds the old edge copy. Negative control: validating the holder only before the terminal's loads. (superseded by §36.13)
  - (c) The existing models stay green and unedited (`crates/expanse/src/occ.rs:4203`, `:4163`; `sync_nav.rs:1314`, `:1323`).
  - *Covered:* the validation rule for a multi-word terminal and for a replaced terminal, on the real read-set type. *Not covered:* `bitmap_leaf::<false>`, `next_in` and `prev_in` themselves. A model stays green if that code is wrong; G36.2, G36.3 and G36.4 are what run it.
  - **Falsifier:** a positive model that fails, or a negative control that does not panic with its expected message.
- **G36.6 — Miri. Run in CI only, never on a development machine.**
  - *Tier 1, per pull request.* `sync::miri_tests::set_ordered_reads_under_miri`: single-threaded, every method over every fixture of G36.2's table, in PR M's allocation mode. (superseded by §36.11 and §36.13) It is selected by the existing filter word `sync::miri_tests::` (shard `1 of 3`, `.github/workflows/ci.yml:1722`), so no filter word is added and the three lists `scripts/check_miri_shards.py` compares stay equal.
  - *What it reports.* With exact-size nodes: a read past a node, a read of unwritten bytes, and a wild dereference. Without that mode it would report the wild dereferences only (§36.2). (superseded by §36.11)
  - *Nightly census.* By module, every test under `sync::` is in shard `lib-sync` and every test under `sync_nav::` in `lib-rest` (`scripts/check_miri_shards.py`, `LIB_SHARDS`).
  - *Concurrent sites.* `sync::miri_ub_sites::set_ordered_leaf_reader_writer`, `set_ordered_tree_reader_writer` and `set_ordered_bitmap_reader_writer`, twins of the map's (`sync.rs:16884`, `:16909`), each entered in `.github/miri-ub-sites.json` under the three checks. `nightly.yml` accepts `workflow_dispatch`, and it is dispatched on PR B's branch for this gate, since that branch is not on `main`.
  - *Outside Miri, by declaration.* `sync::ordered_read_tests` is `cfg(all(test, not(miri)))` (`sync.rs:25701`); the `occ-stats` targets carry a file-level `#![cfg(not(miri))]`; the G36.1 histories carry the `cfg_attr(miri, ignore)` their map twins do (`linearizability.rs:1306-1309`); Loom tests exist only under `--cfg loom`.
  - **Falsifier:** any Miri report in the Tier 1 test; a census failure; a `miri-ub-sites` entry whose observed state differs from the manifest.
- **G36.7 — the set form is instantiated in one place.** A source test with two clauses, each shown red by its own mutation (AGENTS.md §5): outside test modules, (1) `next_validated::<false>` and `prev_validated::<false>` occur only inside the set's macro; (2) `::<true>` occurs only at the map's two sites and in `seek_submap`. **Falsifier:** either clause green under its mutation (the set macro's constant flipped; a `::<false>` added to a map path).
- **G36.8 — a detached reader is checked on every ordered entry.** One `should_panic` test per `DetachedSetReader` method, eight in all with `next_after(u64::MAX)` and `prev_before(0)`, on the pattern of `foreign_reader_rejected!` (`sync.rs:18911-18935`): a reader registered on set `a` and called with set `b` panics with the foreign-collector message before any node of `b` is read. The early-return pair pins that the check precedes D3. Fail-then-pass with `registered` removed, as #1465 recorded for `contains` (`sync.rs:19094`, `:19104`). **Falsifier:** a method that returns.
- **G36.9 — no write path is touched.** The change adds readers. It stores nothing into tree memory, so it adds no `assert_bracketed()` obligation and no `by_mode!` entry point (AGENTS.md §2.1 invariant 5). **Falsifier:** a hunk outside `cfg(test)` and `occ-stats` bumps in `mutate.rs`, `mutate_map.rs`, `map.rs`, `set.rs`, `alloc.rs` or the `olc_*` bodies of `sync.rs`; or any change in `set_insert/*`, `set_remove/random`, `sync_set_insert/random`, `sync_set_remove/random` or `sync_set_churn/*`. A park point a test needs on the writer side is armed from the reader's hooks, as `arm_branch_sample` runs the writer's call on the reader's thread (`sync.rs:25955-25961`).

G36.4, G36.5 and the reproducer are re-run after any change to the obsolete-marking or cover rules, as §12.2 requires of G12.1.

### 36.5 Performance gates: instruction counts

Fixed before any measurement. The instrument is the `instruction-counts` job on each pull request, against that pull request's base. It is single-threaded: it shows the work one call does, the locked route's mutex and quiesce operations included, and nothing about waiting.

**How the walk is shared.** The three handle forms expand **one macro**, `set_ordered_walk!`, the way `set_contains_walk!` does (`sync.rs:6365-6380`). It is not a function and not an `#[inline(always)]` function: on `sync_set_contains` a plain function measured +1.90% on `random`, and `#[inline(always)]` +7.50% on `random` and +9.15% on `leaf` (recorded at `sync.rs:6357-6364`, from the `instruction-counts` job of #1465). Direction is a const generic or a macro argument, never a runtime flag in the loop, as the string walk settled (#1448). The method list may be a second macro, as `map_reader_ordered_reads!` is (`sync.rs:10444`).

**Debug assertions.** A `debug_assert!` in the shared walk is acceptable. It compiles out of release builds, and P36.1 is the check that it did.

**The tiny-set arms (PR T, before any engine code).** Two shapes no existing ordered arm reads, each on the locked route first:

| Arm | Setup | Ops |
|---|---|---|
| `sync_set_prev_locked/leaf` | `built_sync_set_leaf` (`instructions.rs:2961`): a set held in the root leaf | 50,000 `prev_before` calls, cycling its present keys |
| `sync_set_prev_locked/immed` | a tree-rooted set whose every terminal is an immediate, asserted from `stats().node_counts` in setup | 50,000 `prev_before` calls over present keys |

Their optimistic twins, `sync_set_prev/leaf` and `sync_set_prev/immed`, land with PR B on the same setups and counts.

- **P36.1 — nothing that exists moves.**
  - The claim: in PR A and in PR B, every Callgrind arm that exists on the base changes by at most 0.1% (AGENTS.md §6 review threshold). The bound has no direction, as §12.7 read P12.2.
  - Most at risk, and named in the PR body with their figures whatever they are:
    - `sync_set_contains/{random,leaf}`: `SetReader::contains` sits in the `impl` blocks the new methods join and shares `optimistic_read`; this is the arm that moved in §12.7 and under the two rejected sharing forms of #1465.
    - `sync_map_prev/random`, `sync_map_next_after_scan/*`, `sync_map_scan/*` and `sync_strmap_scan_pointops/*`: PR B adds the first non-test `MAP = false` instantiation of the generic source whose `MAP = true` instantiation they run, and PR A adds debug assertions and feature-gated bumps to that source.
    - `sync_set_insert/random`, `sync_set_remove/random`, `sync_set_churn/*`, `sync_map_get/*`: same file, no shared source line changed.
    - the plain arms `set_nav/*`, `set_prev/*`, `set_contains/random`, `set_insert/*`, `set_remove/random`, `map_nav/*`, `map_prev/*`.
  - *Disassembly.* `objdump -d -C` of the bench binary at base and head, with the count of `%fs:` reads in `nav::next::<false>`, `nav::prev::<false>`, `ExpanseSet::contains` and `mutate::insert_with_path_flat` stated for both: equal, and zero where it is zero today (AGENTS.md §2.1 invariant 5).
  - **REFUTED** on any arm above 0.1% in either direction, or on a changed `%fs:` count. A refuted arm is attributed from a base-against-head `callgrind_annotate` profile, and from the disassembly where its source did not change, before any fix (AGENTS.md §6).
- **P36.2 — an optimistic ordered read counts fewer instructions than the locked one.**
  - Six cells, each with the keys, probe stream and ops count of its locked twin: `sync_set_prev/{random,leaf,immed}` and `sync_set_next_after_scan/{random,sequential,clustered}`.
  - The claim, and the bound: each cell counts fewer instructions per operation than its `_locked` twin on the same head.
  - A projection, not a gate (unmeasured): each lands between its single-threaded reference and its locked twin, with the `leaf` and `immed` cells showing the smallest margin, since no branch is visited there and validation removes no work. No magnitude is predicted.
  - **REFUTED** for a cell at or above its locked twin.

Callgrind cannot see contention, so P36.2 says nothing about P36.4.

### 36.6 Performance gates: wall clock

Reference host only: the host §12.9 describes (Intel Core i9-12900F, 8P+8E, 24 threads). A development machine decides nothing here. Where a construct is §12.4's it is referenced, not restated.

**Workload.**

| Field | Value |
|---|---|
| workload ID | `concurrency_ordered_readers_set_63bit`, declared in the harness's shape table by PR B |
| harness | `crates/expanse/examples/writer_scaling.rs`, `--arm set` in reader mode. Today that mode is the readers-only cell and refuses writers, any `--read-op` but `get` and any `--probe` but `uniform` (`writer_scaling.rs:3293`, `:3414-3430`); PR B extends it |
| read operations | `prev_locked`: `with_locked(\|s\| s.prev_before(k))`. `prev`: `SetReader::prev_before(k)`, one handle per reader thread |
| prefill | the set arm's writer-sweep prefill, 2^20 keys drawn over 63 bits (`writer_scaling.rs:174`), plus §12.4's 256 hot-spot keys in every cell; inserted in ascending order |
| writers | `uniform`: 2^20 fresh keys of the same draw, split across W. `hotspot`: the expanse's other 65,280 keys, shuffled, split across W |
| probes | every probe is a present key, each reader cycling its own permutation; there is no absent-key probe and no miss shape to declare. A `prev_before` of the minimum is the one probe that answers `None` |
| measured region | from the barrier release to the join of the last reader (`writer_scaling.rs:73-75`); prefill, workload generation, reader-handle registration, the answer check and teardown are outside it |
| statistic's unit | aggregate reader throughput of the cell: the readers' summed calls over that region |
| node forms, `uniform` cells | linear leaves, immediates and branches. 2^20 keys over 63 bits put about 32 keys under each two-byte prefix, at the level-2..7 leaf cap (`LEAF_CAP` = 32), so no terminal byte gathers the 26 keys a `LeafBitmap1` needs and no full expanse forms (derived from the constants; unverified by a census). **The `uniform` gate cells therefore do not exercise the set-only forms; coverage of those rests on the correctness gates and on the `hotspot` cells** |
| node forms, `hotspot` cells | the writers fill every 256-key block, which converts each block's `LeafBitmap1` to `FullExpanse` on the serialised path (`CapExpansionKind::BitmapNearFull`, `sync.rs:3524`, `:4867-4869`; conversion `mutate.rs:1897`). A set hot-spot window contains writer fallbacks a map's does not. §12.4's branch argument cites the map's leaf cap (`writer_scaling.rs:95-100`); the set's is `mutate.rs:1267`, and the harness asserts the shape before it times anything (superseded by §36.14) |
| recorded per row | the node counts at the end of the window. 256 full expanses are expected in a hot-spot cell with writers (derived, unverified by a run); such a cell ending with none is void for any statement about form coverage |

**Cells and schedule,** unchanged from §12.4: (W, R) ∈ {(0, 1), (0, 4), (1, 4), (4, 4)} × `read_op` ∈ {`prev_locked`, `prev`} × `probe` ∈ {`uniform`, `hotspot`}; 8 rounds per cell, one harness process per cell and round, the block interleaved within each round by the driver's Williams rows; throughput and counters from separate binaries. The comparator is the locked route of the same commit and binary, over the identical probe stream.

**Pin.** `0,2,4,6,8,10,12,14`, one thread per physical P-core, W + R ≤ 8. AGENTS.md §8.20.5 step 0 applies to the `prev_locked` cells, whose readers serialise on a mutex: with at most 8 threads on 8 cores each blocked thread has a core of its own to park on, so the pin does not make the locked route compete with itself. Both routes of a cell run under the same pin.

**Why `prev` alone.** The wall-clock gate reads one operation, `prev_before`, as §12 did, so the set's cells can be laid beside the map's and the 8-cell block keeps its Williams design. Forward reads are not measured in wall clock; their instruction counts are P36.2's three scan cells, and their correctness is every gate of §36.4.

- **P36.3 — fallbacks stay rare.** In the counters build, `read_fallbacks ÷ read_ops` of the `prev` cells is below 0.1% in every cell, both probes. The ceiling is §12.3's choice for P12.4, kept. Attempts per operation are reported, not gated.
- **P36.4 — readers gain over `with_locked` while writers run.**
  - Statistic: ρ = aggregate reader throughput of `prev` ÷ that of `prev_locked`, paired within each round.
  - Gate cells, three: (W, R) = (1, 4) `uniform`, (4, 4) `uniform` and (4, 4) `hotspot`. Floor: the BCa 95% lower bound of ρ is at least 1.0, at each.
  - *Why (4, 4) for the hot-spot gate.* The principle: among the cells with writers, gate the one §36.7 names as most at risk. That is (4, 4) `hotspot`: it has the most writers filling the subtrees every probe backtracks through, the most `FullExpanse` conversions on the serialised path per window, and on the map it was the cell with the most attempts per read and the largest fallback count (§12.9). A registration that gated only cells expected to pass would test nothing.
  - Reported, not gated: both W = 0 cells of each probe and (1, 4) `hotspot`.
- **P36.5 — the gain is a concurrency gain** (AGENTS.md §8.20.2).
  - Statistic: κ = ρ at (1, 4), `uniform` ÷ ρ at (0, 1), `uniform`, both taken in the same round. The (0, 1) cell is the control: one reader and no writer, where a validated read can only be cheaper per call, not less blocked.
  - Floor: the BCa 95% lower bound of κ is above 1.0.
- **Interval.** BCa 95% over the 8 per-round values of a run, 2,000 resamples, seed 42: the defaults of `scripts/bca_bootstrap.py:141-142` and `:216-217`. Each interval is recorded with its `ci_method`.
- **Runs.** Two independent dispatches form the pair. The `prev_locked` throughput at (0, 1), `uniform`, a cell this change does not touch, is reported for both runs beside the verdict so a reader can see whether the host moved between them. It is not a gate: this suite has no committed measure of its between-run spread to set a band from.
- **What voids a run.** §12.5 in full, with this section's gates in place of §12.2's; a row whose `read_ops` does not equal the readers' call count, which the harness refuses (`writer_scaling.rs:455-501`); and host contention, stated as **foreign** busy CPU: a cell whose recorded `foreign_busy_cpus` reaches 1.0 core-equivalent (`RUN_FOREIGN_VOID`, `scripts/bench_provenance.py:127`), or a single foreign process above 100% of a CPU (`RUN_PROCESS_VOID_PCT`, `:128`). `foreign_busy_cpus` is host busy time less the run's own process tree (`bench_provenance.py:250`, `:324-331`), so the harness's eight pinned threads do not count against it. No load-average bound is registered: eight busy threads of the run itself put the load average at 8, and a bound below that could never be met.
- **Replacement runs, for void runs only.** A void run is replaced. At most three replacement runs are taken per pair; every void run is kept and disclosed. A pair that is still not valid after them has the outcome `INTERMEDIATE`. A valid run is never replaced, whatever it shows.

**What the design can and cannot detect.**

- *Resolution.* With 8 rounds, the map's gate cell had an interval of about 6% of its ratio either side: 91.81 [85.52, 96.74] and 89.47 [84.24, 93.75] (measured: §12.9, `results/ordered_readers_writer_scaling.json` and `_run2.json`; workload: `concurrency_ordered_readers_map_64bit`). If the set's cells are as wide, a true ρ within roughly 6% above 1.0 lands `INCONCLUSIVE`, not `PASS`.
- *The two `uniform` cells are easy to clear.* The locked comparator stops every writer for each read, and the map's gate ratio stood far above 1 for that reason (same figures). They would be missed only by a gross defect, and they do not touch the set-only node forms.
- *The `hotspot` cell is the one that can fail.* It is where §36.7 expects the set to be weakest, and it is gated at the same floor. On the map that cell read 11.38 [10.07, 12.47] and 12.38 [11.14, 15.19] (measured: §12.9, same artifacts; workload: `concurrency_ordered_readers_map_64bit`), an interval about 10 to 20% of the ratio either side, wider than the `uniform` cells'. The set's window adds serialised writer sections the map's does not have, so the map's margin is not evidence for the set. **This registration expects a real chance of `REJECTED`, and `REJECTED` means the methods do not ship.**

**Silent constants,** all fixed by this section: `MAX_RETRIES` 64; `READ_SET_CAP` 16; the 0.1% Callgrind bound; the 0.1% fallback ceiling; floors 1.0 on ρ and κ; the three gate cells; 8 rounds; 2,000 resamples, seed 42; the pin; the foreign-load boundaries 1.0 and 100%; three replacement runs per pair; one second pair; 2^20 prefill keys and probes per reader at W = 0 (`writer_scaling.rs:72-73`); the 256-block hot-spot expanse; 24 rounds and at most 64 events per G36.1 history; 50,000 operations per Callgrind arm. Changing any of them after a run relabels that run `INTERMEDIATE` and requires fresh runs (AGENTS.md §8.19).

### 36.7 Expected losses

Recorded before any measurement and never reconciled with the outcome (AGENTS.md §8.7). Every reason is a hypothesis; none is measured on the set.

| Cell or use | Expected | Reason (hypothesis) |
|---|---|---|
| Any validated read against the plain `ExpanseSet` call (`sync_set_prev` against `set_prev`, `sync_set_next_after_scan` against `set_nav`) | loses, by design | a pin, a version sample and a final validation per call that the single-threaded call does not make. Not a comparison this section gates |
| A whole scan by `first` + `next_after` against one `with_locked` that iterates the set inside the lock | loses per element | one pin, one sample and one root descent per element against an iterator step; no arm measures this pair, and the remedy is the excluded batch cursor |
| (0, 1), either probe | smallest ρ of the sweep | nothing waits on anything, so only the per-call cost of the locked route is saved; the map's control read lowest too (§12.9) |
| `sync_set_prev/leaf` and `/immed` against their locked twins | narrowest P36.2 margins; a loss here refutes P36.2 for that cell | no branch is visited, so validation removes no work, and the root leaf's keys are loaded one atomic word at a time |
| `hotspot` with writers, most at (4, 4), which is a gate cell | ρ far below the `uniform` cells and possibly below 1.0; fallback share nearest the ceiling | every probe backtracks through subtrees the writers are filling, so attempts per read rise as they did on the map (§12.9), and each block's conversion to `FullExpanse` adds a serialised writer section the map's window does not have |
| `sync_set_next_after_scan/sequential` and `/clustered` against their locked twins | narrow P36.2 margins | dense terminals make the locked walk short, so the fixed per-call cost is the larger share |
| Writer throughput in the `prev` cells | not gated; reported per cell | writers are no longer the only running threads |

### 36.8 Verdicts, and what each means for shipping

**Correctness** is binary. With any G36 gate unmet on PR B's head, no cell is read.

**Instruction counts** are verdicts on a pull request: `HOLDS` or `REFUTED` for P36.1 and P36.2. A refuted P36.1 blocks that pull request until the moved arm is attributed and either restored or accepted as a stated trade-off under AGENTS.md §6. A refuted P36.2 is published and decides nothing about shipping.

**Wall clock.** The verdict is a function of the valid pair, defined for every outcome.

1. *Per run, per statistic* (ρ at (1, 4) `uniform`, ρ at (4, 4) `uniform`, ρ at (4, 4) `hotspot`, κ): `MET` when the BCa 95% lower bound clears its floor, `MISSED` when the upper bound is below it, `OPEN` otherwise.
2. *Per run, P36.3:* `HOLDS` when every `prev` cell is below the ceiling, `REFUTED` otherwise.
3. *Per run:* `RUN_PASS` when all four statistics are `MET` and P36.3 `HOLDS`; `RUN_REJECT` when any statistic is `MISSED` or P36.3 is `REFUTED`; `RUN_OPEN` in every other case.
4. *The pair:*

| Run 1, run 2 (either order) | Verdict |
|---|---|
| `RUN_PASS`, `RUN_PASS` | `PASS` |
| `RUN_REJECT`, `RUN_REJECT` | `REJECTED`, whether or not the same statistic missed in both |
| every other combination, and a pair not valid after its replacement runs | `INTERMEDIATE` |

So two gate cells met and the third missed is `RUN_REJECT`; an `OPEN` statistic with nothing missed is `RUN_OPEN`; P36.3 refuted in one run only makes the pair `INTERMEDIATE`.

**What ships.** The methods are released only on `PASS`.

- *Enforcement.* PR B stays unmerged while the runs are taken. `bench_baremetal.yml` resolves its `ref` input to one commit and checks that commit out (`.github/workflows/bench_baremetal.yml:5-9`, `:253-266`, `:623-625`), and reads the workflow and the suite manifest from the branch it is dispatched on, so the suite is dispatched on `main`, where PR C1 put its wiring, with `ref` set to PR B's full head SHA. Nothing of PR B is on `main`, in the public-API snapshot or in a release until the verdict. The artifacts name that SHA, which stays fetchable as the pull request's head; PR B is rebased on `main` before the runs, and a later push to it voids them.
- `PASS`: PR B merges with its outcome subsection, and the documentation may state that set readers gain over `with_locked`, with the intervals and §36.7's losing cells beside them.
- `REJECTED`: PR B is closed unmerged, and the outcome subsection and the artifacts are committed to `main` on their own. The methods do not ship. PR T, PR A and PR C1 stay: they add arms, tests and wiring, and no surface. The rejection is not reopened by moving a floor to the measurement (AGENTS.md §8.19).
- `INTERMEDIATE` on the first pair: exactly one more pair of fresh runs is taken, on the same SHA, with every constant of this section unchanged. Its verdict is read by the table above and is **final**: `PASS` ships; `REJECTED` or `INTERMEDIATE` closes PR B unmerged, as `REJECTED` does, and the methods do not ship. The first pair's runs are kept and published; they are not pooled with the second pair's.
- *How the two limits compose.* The replacement cap belongs to a pair and concerns void runs only; the second pair is granted once and concerns an inconclusive outcome only. So the most this registration allows is two pairs, each of two valid runs with up to three void runs replaced: ten dispatches. Nothing beyond that is taken without an amendment appended here first (AGENTS.md §8.19).

In every case all cells are published, the losing ones included.

### 36.9 Explicitly not predicted

- Magnitudes for P36.2, P36.4 and P36.5.
- `first`, `last`, `next_at_or_after`, `next_after` and `prev_at_or_before` in wall clock.
- The owned and detached handles in wall clock: they expand the same macro, and G36.3 covers their answers and counters.
- Reader cells at W ≥ 2 other than (4, 4).
- Any transfer of the map's figures to the set. §12.9's intervals describe the map.
- Everything under *Named exclusions* in §36.1.

### 36.10 Open questions

None. The section number, 36, is the next free one at lock time and may move at merge.

### 36.11 Correction — which trees a node over-read is visible in (appended 2026-10-13; §36.1 to §36.10 are not rewritten)

§36.2 says node memory is slab-carved and that neither Miri nor ASan reports a read past a small node. That is true of a plain tree and false of a shared one, and every tree this section's gates read is shared. The sentences marked "(superseded by §36.11)" above are replaced by this subsection. **No threshold, cell, statistic, constant, verdict rule or shipping rule of §36 changes.** Two gates' detection claims change, G36.2 part (iii) and G36.6, and one dependency edge of §36.1; they are listed below. The error came from a finding of the synthetic review panel that was accepted after reading the allocator's slab path and without reading its deferred path. It was reported from the work on PR M (#1491). Code references are to `origin/main` at `ac001baf4`. Refs #1194.

**What the allocator does** (`verified`, `crates/expanse/src/alloc.rs`).

- A tree in deferred mode never uses a slab page. `alloc_raw` leaves through the deferred branch before any slab or per-tree freelist code: it pops the collector's freelist for the class, or calls `alloc_system` (`:1065-1078`). `defer_to` asserts that the allocator has never carved a slab page (`:1608-1621`), and every `Sync*` wrapper defers before its first node (`SyncExpanseSet::new`, `crates/expanse/src/sync.rs:4146`).
- `alloc_system` is one `alloc_zeroed` of `Layout::from_size_align(bytes, align)` with the bytes the engine requested (`:1035-1041`, `:1171-1179`). `accounted_size` rounds only the figure added to the usage counters (`:368-370`).
- A size class is one exact `(bytes, align)` pair (`class_for`, `:345-357`), so a block reused from the collector's freelist (`crates/expanse/src/occ.rs:1914`) has exactly the layout it was allocated with, and is zeroed over `bytes` before it is returned (`alloc.rs:1070-1071`).
- Slab pages serve plain trees only: the slab branch is below the deferred return (`:1121-1129`).

So each node of a shared set is its own allocation of the requested size, on `main` today, in every build. `with_locked` on a shared set reads that same tree, so the locked route's fixtures read exact-size nodes too.

**Statements corrected.**

| Where | As merged | What is true |
|---|---|---|
| §36.2, *Node memory is slab-carved* | a read past a small node stays inside one initialised allocation; neither Miri nor ASan reports it | true of a plain tree only. On a shared tree a read past a node leaves its allocation, and Miri reports it. ASan (the `test-asan` job, which runs the crate's unit tests and integration targets with `-Zsanitizer=address`) reports it when the read lands in the redzone beside the block; a read far enough past the block to land in another live allocation is not reported by ASan |
| §36.3, row A1 and the sentence after the class A table | "inside a slab page the read is not reported"; "no sanitizer ... sees the read" for A1, A5, A10 | on a shared set an inverted A1 reads at or past the end of the root leaf's allocation (whether the offset always reaches the end is unverified: it depends on the set root leaf's request size against `map::leaf_values_offset`). An inverted A5 or A10 at a high slot reads past the leaf and is reported by Miri; at a low slot it reads the leaf's own key bytes, which no sanitizer reports. That low-slot case is what G36.2 part (i) exists for |
| §36.3, row A4/A9 | "out of bounds only where the node is its own allocation" | on a shared set the node always is its own allocation |
| §36.4, G36.2 part (iii) | exact-size nodes come from PR M; without it A1 falls back to part (i) | part (iii) holds on `main` without PR M for every fixture built on a `SyncExpanseSet`, which is every fixture G36.2 registers. No fallback applies |
| §36.4, G36.6 | the Tier 1 test runs "in PR M's allocation mode"; "without that mode it would report the wild dereferences only" | `sync::miri_tests::set_ordered_reads_under_miri` reads a shared set, so it reports a read past a node, a read of unwritten bytes and a wild dereference with or without PR M |
| §36.1, item 1 (PR M) | the mode that makes G36.2 and G36.6 able to see over-reads | PR M changes what Miri sees on plain trees: a read past a node of an `ExpanseSet`, `ExpanseMap` or any other single-threaded container, and a use of a node after the plain path freed it, which today stays inside a slab page or a per-tree freelist block. It changes nothing on a shared tree. Its own gate, as registered, stands |
| §36.1, item 3 (PR A) | depends on PR M and PR T | depends on PR T only. PR M strengthens PR A only where a PR A test reads a plain tree, and none registered here does |

**Which Miri runs execute the set walk.** The threaded tests of `sync::tests` and `sync::ordered_read_tests` are compiled out under Miri (`sync.rs:18201`, `:25701`), so nothing registered there reaches a Miri lane. What does: the single-threaded Tier 1 test G36.6 registers, in the nightly `lib-sync` shard as well; and the three `sync::miri_ub_sites` workloads G36.6 registers, run by the nightly census under the race detector and the two aliasing models. All four read a `SyncExpanseSet`. The per-pull-request census step runs a fixed list of five workloads, none of them a set workload (`.github/workflows/ci.yml`, job `miri-ub-sites-pr`), so the concurrent set workloads are seen nightly and on a dispatched nightly, as G36.6 already says for PR B's branch. The per-conditional detectors of G36.2's table therefore read: Miri for a read past a node, on the Tier 1 test; the value-word check for a read inside the node; first-attempt answers for the retry rows. ASan adds the unit tests Miri does not compile.

**A blind spot neither tool covers: a retired block.** A node a writer retires goes to a collector bin and then, for a size class, to the collector's freelist for reuse (`occ.rs:2052`, `:1914`). It is not deallocated in between, so a reader that reads it without a valid pin (a pin that ended, or a pin on another collector) reads live memory, and neither Miri nor ASan reports it. PR M does not change this. No §36 gate relies on a sanitizer for it:

- a reader pinned on the wrong collector is rejected by the identity check before it reads, and G36.8 tests that by its panic, not by a sanitizer;
- the reads this section adds pin through `Shared::optimistic_read`, which they share with `contains` and add nothing to; the pin and epoch protocol is covered where it was before, by the Loom models and the handle types `docs/ARCHITECTURE.md` §4.1 lists under S4 and S4a;
- a walk that used a stale edge copy under a valid pin is the case the version validation exists for, and G36.4, G36.5 and G36.1 test it by the answer returned.

What is not detected by anything registered here: a defect that let a set reader hold a node past its pin without changing any answer. This section adds no code on that path, and makes no claim about it.

**Unverified.** Whether an inverted A1 always reads at or past the end of the root leaf's allocation; and ASan's redzone width on this toolchain, which decides how far past a block a read is still reported.

### 36.12 Clarifications before data (appended 2026-10-13; §36.1 to §36.11 are not rewritten)

Wiring the suite (#1495, PR C1 of §36.1) showed four places where §36 can be read two ways. Each is settled here by what the map's suite already does (§12, and the driver `docs/benchmarks/concurrency/scripts/writer_scaling.py`). No run of the suite exists and no artifact has been written. **No threshold, cell, statistic, constant, verdict or shipping rule of §36 changes.** One void condition the workflow already applies to every suite is disclosed in item 4. Code references are to `origin/main` at `4eb0904f7` and to #1495's head as read on this date. Refs #1194.

1. **P36.3 is computed on counters summed over a cell's rounds.**
   - For each `prev` cell, `read_fallbacks` and `read_ops` are each summed over the cell's 8 counters-role rounds of a run, and the cell holds when the summed ratio is below the ceiling. It is not a per-round test.
   - The comparison is in integers, `read_fallbacks × 1000 < read_ops`, so a cell at exactly 0.1% does not hold, and P36.3 is `REFUTED` for the run when any of the eight `prev` cells does not hold.
   - This is the map's P12.4 as implemented (`writer_scaling.py:2766-2790`, ceiling `:1523`) and as §12.9 reports it. #1495's `set_fallback_verdict` does the same.
2. **`MET`, `MISSED` and `OPEN` (§36.8 item 1), per statistic.** With `[lo, hi]` the BCa 95% interval of a run:

   | Statistic | Floor | `MET` | `MISSED` | `OPEN` |
   |---|---|---|---|---|
   | ρ at each of the three gate cells | 1.0 | `lo >= 1.0` | `hi < 1.0` | every other interval |
   | κ | 1.0 | `lo > 1.0` | `hi < 1.0` | every other interval |

   - `MET` is tested first, so the three labels are disjoint and cover every interval with `lo <= hi`.
   - κ's floor is excluded from `MET`, and it is excluded from `MISSED` too: an interval whose upper bound equals 1.0 is `OPEN`, and so is the degenerate interval [1.0, 1.0]. For ρ that degenerate interval is `MET`.
   - An interval that is not a pair of numbers, or has `lo > hi`, is refused, not labelled (AGENTS.md §8.1).
   - #1495's `set_stat_status` implements exactly this; no mismatch was found.
3. **Node counts per row, and the form-coverage void (§36.6).**
   - *Field.* Every throughput and counters row of this suite carries `node_counts_end`: an object with the fields of `ExpanseSet::stats().node_counts` (`null`, `immed`, `leaf_linear`, `leaf_bitmap`, `branch_l3`, `branch_l7`, `branch_b`, `branch_u`, `full_expanse`; `crates/expanse/src/validate.rs:124-143`), read under `with_locked` after the readers have joined and outside the timed region. The field arrives with the harness's set reader mode, in PR B.
   - *Until then.* #1495 neither requires the field nor reads it: the mode that would emit it does not exist on `main`.
   - *Obligations of PR B.* (a) The harness emits the field on every row of this suite. (b) The driver's `--set-ordered-readers` mode requires it, and refuses a row without it by name. (c) For a `hotspot` cell with W ≥ 1 whose rows end with `full_expanse` = 0, the driver records `form_coverage: "void"` on that cell with the reason. (superseded by §36.14) That marks the cell's form-coverage statement void. It does not void the cell's ρ, the run or the pair, and it does not enter the §36.8 verdict function. (d) The outcome subsection says, per `hotspot` cell with writers, whether its form-coverage statement stands.
   - *Also for PR B.* The harness's refusals of writers, of `--read-op` other than `get` and of `--probe` other than `uniform` on `--arm set` (`crates/expanse/examples/writer_scaling.rs:3417`, `:3423`, `:3430`) are pinned by the suite's self-tests, and #1495's driver probes for the mode by that refusal. Those assertions change in the same pull request as the mode.
4. **The host-contention void is decided by the workflow's host guard.**
   - The driver cannot see other processes. It records each cell's `foreign_busy_cpus` (`scripts/bench_provenance.py:250`, `:324-331`). The single-process clause of §36.6 (a foreign process above 100% of a CPU) is computed by `scripts/bench_host_guard.py`, which `bench_baremetal.yml` runs for every suite: `check` before the benchmarks (`.github/workflows/bench_baremetal.yml:959-961`), `watch` during them (`:993-994`) and `summarize` after (`:1854-1855`). A contaminated record fails the job with `fail_reason=host_contaminated` (`:1864-1867`).
   - *Rule.* A run whose job the host guard failed is void under §36.6 and is replaced under §36.6's cap. A run is admissible on this clause only if its job passed the guard.
   - *Disclosure.* The guard judges three boundaries, not two: foreign load of 1.0 core-equivalent, a foreign process above 100%, and foreign load on the pinned CPUs of 0.25 core-equivalent (`RUN_ON_PIN_VOID`, `bench_provenance.py:135`). §36.6 named the first two. The third is applied by the workflow to every suite and so to this one; it is recorded here, not added by this section.
   - *Where the record lives.* The workflow writes the guard's record into artifact provenance for one suite only, `patricia_d2_paging` (`bench_baremetal.yml:1875-1882`); #1495 does not extend that to this suite. Until it is extended, admissibility on this clause is read from the dispatch's job conclusion and its `host-activity` summary in the run log and uploaded files, and the outcome subsection cites the run for it. Attaching the record to this suite's artifact, so the artifact alone decides, is a follow-up to land on `main` before the first run, since the dispatch reads the workflow from `main`. (superseded by §36.14)

### 36.13 Amendment from PR A (appended 2026-10-13; §36.1 to §36.12 are not rewritten)

Implementing PR A (#1497 and #1498, which split it along the gate boundaries of §36.4) found eight places where §36 is wrong against the code or cannot be implemented as worded. Each is recorded here with what replaces it and with whether the gate concerned detects more, the same or less than §36 registered. The sentences marked "(superseded by §36.13)" above are replaced by this subsection. **No wall-clock or Callgrind threshold, cell, statistic, constant, verdict or shipping rule changes, and no run of the suite exists.** This subsection covers these eight items only; a review of #1497 and #1498 is in progress, and anything it adds is appended separately. Code references are to `origin/main` at `79ea9b1e3`; test names are those of the two pull requests as read on this date. Refs #1194.

1. **A set leaf of population 1 does not exist on a shared set.**
   - *§36 said:* fixtures with set leaves "at populations 1, a class boundary and the cap" (C3; G36.2's table, rows A4/A9 and A5/A10).
   - *The code:* an immediate becomes a leaf one key above the level's immediate capacity, and the optimistic removal keeps a leaf only while `pop - 1 > immed_max`, in place (`crates/expanse/src/sync.rs:5874-5877`) or rebuilt (`:5899`); below that it returns to an immediate.
   - *Registered instead:* per key width 1 to 7, the smallest leaf is one key above `ImmedType::max_count` (16, 8, 6, 4, 4, 3, 3 keys), then a capacity-class boundary, then the cap. A smaller leaf, three keys, exists only behind a narrow pointer (item 8) and is in the table.
   - *Also:* G36.2 part (i) asks for a non-zero byte in every position of every key. A full expanse holds all 256 low bytes, so its fixture cannot meet that; its arm loads no memory, and its detector is part (ii).
   - *Detection:* **same.** The populations dropped cannot occur, so no reachable state lost a test.
2. **G36.3's integration target moves to PR B.**
   - *§36 said:* a new integration target, one `#[test]`, over the eighteen public methods, in PR A.
   - *The code:* the methods are PR B's, and an integration target cannot call the crate-private walk.
   - *Registered instead:* PR A carries the lib-level equivalents: `sync::set_ordered_stats_tests::set_ordered_reads_answer_optimistically_and_visit_every_form` (under `occ-stats`, in the single-threaded lib pass of `scripts/test_occ_stats.sh`), which drives `Shared::optimistic_read` over the set form for the six reads on one borrowed handle and asserts the model's answers, `ReadOps` = `ReadAttempts` = `w`, `ReadFallbacks` = 0 and every visit counter non-zero; and `sync::ordered_read_tests::set_ordered_read_set_reaches_its_bound_of_13`. PR B adds the integration target as registered, over all eighteen methods and the three handle forms, with the `keys` distributions. (superseded by §36.14)
   - *Detection:* **same for the walk and the retry protocol; deferred for the wrappers.** Ledger class D (the projection, the fallback closure, the early returns, the detached check) is first tested in PR B, which is where that code first exists.
3. **PR A changes one public surface: the `occ_stats::Stat` variants.**
   - *§36 said:* "No public API" in PR A (§36.1 item 3), and visit counters bumped through `occ_stats` (G36.3).
   - *The code:* `occ_stats::Stat` is a public enum, and `NAMES` and `snapshot()` are public with the stat count in their types (`.github/public-api/expanse-trie.txt:1522`, `:1538`). `Stat` carries `#[non_exhaustive]` today (`crates/expanse/src/occ_stats.rs:44-50`); AGENTS.md §2.3 item 6 requires that attribute of `SlotTag` and is silent on `Stat`.
   - *Registered instead:* the 15 visit-counter variants, and the two array lengths that follow from them, are the one permitted surface change of PR A, with the snapshot regenerated by `scripts/check_public_api.py --write`. Adding a variant to a `#[non_exhaustive]` enum is not breaking. The change of `[_; 52]` to `[_; 67]` in two public signatures is a signature change, which under the Cargo 0.x rule needs a minor version: it ships in 0.13.0 and in no 0.12.x patch. PR A still adds no method and no non-test instantiation of the set form.
   - *Detection:* **same.**
4. **The Tier-1 Miri test runs a reduced table; the full table runs under Miri nightly.**
   - *§36 said:* the Tier-1 test runs "every method over every fixture of G36.2's table" (G36.6).
   - *The implementation:* the full table has 19 rows, and #1497 reports that it does not fit Tier-1 shard `1 of 3` under Miri (not re-measured for this amendment). `sync::miri_tests::set_ordered_reads_under_miri` runs 8 rows per pull request; `sync::set_ordered_table_tests::set_ordered_reads_over_the_full_table` runs all 19 natively per pull request (the `test` job, and the `test-asan` job) and under Miri in the nightly `lib-sync` shard.
   - *Forms under Miri per pull request:* root leaves (three populations); immediates; one linear leaf of every key width 1 to 7 at the population that fills its allocation exactly; one narrow one-byte leaf; a bitmap leaf and a narrow bitmap leaf.
   - *Forms under Miri only nightly:* linear leaves at the class boundary and at the cap; the second narrow leaf; full expanses; a walk through a bitmap branch and through an uncompressed branch.
   - *What that means.* A regression that only Miri can see on a nightly-only form is found by the next nightly run, after the pull request that caused it has merged, not on that pull request. A regression that changes an answer, a value word or an attempt count is still found on the pull request, natively. In particular an inverted A6 or A11 passes the Tier-1 Miri test, since that table has no full expanse; its registered detector, part (ii), fires in the full table natively.
   - *Detection:* **weaker per pull request, same nightly.**
5. **G36.4's `BranchB` mutation cannot fail, and the registered fallback applies.**
   - *§36 said:* take the digit before the sample; the writer removes that digit's key and inserts under a neighbour; the stale digit composes with the neighbour's child.
   - *The code:* a removal that empties a child under a bitmap branch is not made optimistically. It falls back to the exclusive removal (`sync.rs:3474-3483`), which moves the tree version, and the walk's final check of the tree version sends the mutated walk back. `set_ordered_read_past_a_removed_bitmap_branch_digit_restarts` (#1498) runs the mutation as worded in both directions and asserts that the tree version moved and the read retried; if that removal ever stops moving the tree version, the test fails there. An insert alone cannot expose the mutation, because the rank is computed after the sample for a digit that is still present.
   - *Registered instead:* the row is `not discriminable`, by the rule §36.4 set for it, and is enforced by the source scanner `bitmap_branch_arms_sample_before_they_load` in `crates/expanse/tests/test_validated_answers.rs`. It reads the two `BranchB` arms of `sync_nav.rs` without their `cfg(test)` statements and has two clauses, each with its own negative control: exactly one `rs.sample(vp)`; and no load from the branch above it.
   - *What the scanner cannot detect:* it reads source order, not behaviour. It does not see a load reached through a call placed above the sample unless its patterns name that call, a reordering the compiler is not allowed to make but a macro expansion could hide, or a wrong order in a third arm it does not scan. It says nothing about whether a sample taken in the right place is the right version.
   - *Detection:* **weaker.** The `BranchL` and `BranchU` rows are enforced by a wrong answer; this row is enforced by the position of statements.
6. **C5's park point sits before the first word load.**
   - *§36 said:* "a park point between word loads; a writer armed there must force a retry."
   - *The code:* the set bitmap arm's word loads are inside `bits::shared_bitmap::next_set` and `prev_set` (`crates/expanse/src/bits.rs:1204-1250`), outside the files PR A may touch.
   - *Registered instead:* `Site::OrderedSetBitmapLeaf` parks after the walk has copied the leaf's edge and before its first word load. `set_ordered_bitmap_leaf_answer_is_validated` arms a writer there that inserts the probe's true neighbour into the leaf without moving the tree version, and requires a retry; it is shown red with the read-set clause of the final validation removed.
   - *What it covers:* that an answer read from a bitmap leaf changed after the edge copy is rejected by the final validation. *What it does not:* a write that lands between two word loads of one search on the real code. That interleaving is covered only by Loom model (a), whose search is written by hand.
   - *Detection:* **weaker** than registered, on that one interleaving.
7. **G36.5 model (b)'s negative control is not counted.**
   - *§36 said:* model (b), a terminal replaced under its holder, with the negative control "validating the holder only before the terminal's loads".
   - *The code:* the model keeps the leaf's words at one address across the rewrite to a full expanse and back. On a shared set the rewrite back builds a new node (`crates/expanse/src/mutate.rs:2725-2740`), and a node a pinned search holds is not reused while the pin lasts. The schedule on which the control fails, a search reading the same words after two rewrites, cannot occur in the engine.
   - *Registered instead:* the positive model stays, as a check of the validation rule against an adversary stronger than the engine. Its negative control is **not counted as a negative control**: it shows the model can fail, not that the rule is necessary on any schedule the engine can produce. Row (b) therefore has none. The necessity of validating after a terminal's loads is carried by model (a)'s control and by the running negative controls of G36.4.
   - *Detection:* **weaker by one control.** No property lost its only test.
8. **Narrow linear leaves on a shared set are one byte wide.**
   - *§36 said:* "a narrow-pointer set leaf (key bytes below the slot level)" (C2), with no width named, and a visit counter for "a leaf behind a narrow pointer"; read broadly, leaves of two to six bytes behind a narrow pointer were expected.
   - *The code:* an insert never produces one. A leaf of two or more key bytes that overflows becomes a narrow bitmap leaf when its keys diverge only at level 1 (`mutate.rs:1788-1808`), and otherwise a branch at the divergence level, narrow when that is below the slot (`:1809-1817`); it is never rebuilt as a narrower linear leaf. The one source of a narrow linear leaf is the demotion of a narrow bitmap leaf below `LEAFB1_DOWN`, which yields a one-byte leaf that keeps the skipped prefix (`mutate.rs:2682-2700`).
   - *Registered instead:* the form list reads "a one-byte linear leaf behind a narrow pointer" (rows `narrow Leaf1 of 3` and `of 20`), and the counter `OrderedVisitLeafNarrow` counts that form. Narrow linear leaves of two to six bytes are removed from the list for trees built by inserts and removals.
   - *Unverified:* whether `compact`, which rebuilds through `ExpanseSet::from_sorted_keys_into`, can produce a wider narrow leaf. It kept full-width leaves in every fixture #1497 tried; the builder was not read for this amendment.
   - *Detection:* **same** for reachable trees, with the `compact` case open.

### 36.14 Amendment from PR B (appended 2026-10-13; §36.1 to §36.13 are not rewritten)

The engine pull request (#1504, PR B of §36.1, held unmerged) and two reviews of it found ten places where §36 and the code differ or where §36 left something unregistered. Each is recorded here with what replaces it and with whether the gate concerned detects more, the same or less. The sentences marked "(superseded by §36.14)" above are replaced by this subsection. **No wall-clock or Callgrind threshold, gate cell, statistic, constant, verdict or shipping rule changes. No data exists: the suite has not been dispatched, and no artifact of it has been written.** Code references are to #1504's head `c9fe2a1f0` unless a line says `main`; that branch is not on `main`, and its line numbers move with it. The reviews were made by LLM reviewers and are not an external review. Refs #1194.

1. **G36.1's discrimination counts, and what the verdict text must say without them.**
   - *§36 said:* PR B runs each live history once under `set_drop_child_snapshots` and once under `set_header_before_sample` and records the rejected rounds, "reported and not gated".
   - *The code:* both switches are items of `sync::test_hooks`, which is `#[cfg(test)]` (`crates/expanse/src/sync.rs:24724-24725`, `:24933`, `:24975`). The checker and the histories are an integration target (`crates/expanse/tests/linearizability.rs`), which is compiled against the crate without `cfg(test)` and cannot reach them.
   - *Registered instead, for either outcome:*
     - If PR B makes the counts takeable without adding code to a release path (P36.1 and G36.9 decide that), the counts are taken and reported as §36.4 registered, and G36.1 is recorded **met**.
     - Otherwise G36.1 is recorded **met in part**: its histories are shown to reject a wrong answer of the kind PR B demonstrated (a read answering for the wrong bound fails all six, as #1504 records), and they are not shown to reject a race-class defect. The outcome subsection, and any `PASS` text built on it, then states in these words that detection of race-class defects in the set walk rests on the deterministic gates G36.2 and G36.4 and on the Loom models of G36.5, and not on the live histories.
   - Either way the wall-clock gates are unaffected, and "met in part" does not block the dispatch: §36.4's precondition is read with this item.
   - *Detection:* **same if the counts are taken; weaker if not**, by the unshown discriminating power of six probabilistic tests.
2. **W4: only the exclusive removal condenses the root.**
   - *§36 said:* a root-leaf history that crosses `ROOT_LEAF_CAP` in both directions.
   - *The code:* a tree returns to a root leaf in `ExpanseSet`'s exclusive removal (`crates/expanse/src/set.rs:1628-1634`), reached from the optimistic removal's fallback (`sync.rs:4301`). A removal that succeeds optimistically never condenses the root, so a history whose removals all succeed crosses the cap upwards only.
   - *Registered instead:* the root-leaf history writes keys that are each alone under a digit of the top branch, so that removing one empties a child and takes the exclusive path (`sync.rs:3474-3483` on `main`). The forms test requires root-leaf visits and tree visits in every round.
   - *Detection:* **stronger**: the history as first worded would not have exercised W4 at all.
3. **The bitmap and full-expanse histories need neighbours.**
   - *§36 said:* one terminal byte prefilled above `LEAF1_CAP`.
   - *The code:* 26 keys in one block, with only a few keys elsewhere, fit one wider linear leaf (`LEAF_CAP` = 32) and never form a `LeafBitmap1` in a level-1 slot.
   - *Registered instead:* both histories prefill 36 further keys in the next three blocks, which keeps a level-2 branch above the block; the prefill's shape is asserted before any thread starts.
   - *Detection:* **stronger**, for the same reason as item 2.
4. **Forms per history, now registered.** §36.4 named the forms of the full-expanse history only. The others are:

   | History | Forms its scripted reads must visit |
   |---|---|
   | tree-rooted, and hot-spot (the G12.2 twins) | immediates |
   | root leaf | the root leaf, and a tree terminal |
   | bitmap | the bitmap leaf, and the one-byte linear leaf it demotes to |
   | full expanse | the bitmap leaf and the full expanse (§36.4) |
   | `compact` and `clear` | any of root leaf, immediate or one-byte leaf |

   The four phased histories use barriers between phases so the scripted reads meet their form deterministically. *Detection:* **same**; a statement that was missing is now fixed text.
5. **§36.12 item 3 (c): any row at zero voids the statement.** A `hotspot` cell with writers has `form_coverage: "void"` when **any** of its rows, in either role and any round, ends with `full_expanse` = 0, and the rows are named in the reason. The other reading, every row at zero, would let one covered round stand for the cell. It still voids that statement only. *Detection:* **stronger** than the looser reading.
6. **The host-guard record is required on this suite's artifact, from its own pull request.**
   - *§36.12 said:* attaching the record is a follow-up; until then admissibility is read from the run.
   - *Now:* the workflow hunk leaves PR B, which is held and whose copy of the workflow a dispatch never reads, and lands on `main` by itself (branch `bench/1194-set-suite-host-guard-record`). **It merges before the first dispatch.** From then on this suite's artifact must carry `provenance.host_guard`, and a run whose artifact lacks it is not admissible on the host-contention clause; the run log no longer decides it.
   - *Detection:* **stronger**: the artifact alone decides.
7. **What the hot-spot shape assertion asserts.** §36.6 said the harness asserts the shape before timing and did not say what. `check_set_hotspot_shape` (`crates/expanse/examples/writer_scaling.rs:2714-2753`) asserts, on the prefilled set and before the window opens:
   - the §12.4 backtrack geometry: every probe's search target has no prefill key at or below it in its own terminal byte, and one in the byte below;
   - a set holding the hot-spot prefill alone is 256 one-key immediates under a branch, with no leaf;
   - on the cell's own prefilled set, `prev_before` of every hot-spot probe answers the prefill key of the block below, on the optimistic route and on the locked route;
   - that set holds no full expanse, so the full expanses a window ends with are that window's.

   A failed assertion stops the cell before any timing. *Detection:* **same**; the content is now registered.
8. **G36.3's `keys` distributions run at 6,000 keys.**
   - *Registered:* the bench `keys` distributions, which the Callgrind arms draw at 50,000 keys (§36.13 item 2).
   - *The code:* the integration target draws `random`, `sequential` and `clustered` from the same generator and seed at 6,000 keys (`crates/expanse/tests/test_occ_stats_set_ordered_reads.rs:243`). A registered parameter changes.
   - *What is lost:* population-scale depth on the `random` and `clustered` shapes. A tree of 6,000 keys has fewer branch levels and smaller terminals than one of 50,000, so a defect that needs that depth or those populations on those two shapes is not met by this test.
   - *What still holds:* every form is asserted from per-handle visit counters, not inferred from the population; the dense set, the bitmap-branch and uncompressed-branch sets and the bound-of-13 fixture are separate and unchanged; and the Callgrind arms run the public methods at 50,000 keys with their result-count assertions.
   - *Detection:* **weaker**, by that depth.
9. **(review) The fallback route is tested for every method.**
   - *Found:* G36.3 asserts that a quiescent read does not fall back, and ledger row D2 was pinned for direction only. No test checked that the locked answer of each method has that method's bound: inclusive for `next_at_or_after` and `prev_at_or_before`, exclusive for `next_after` and `prev_before`.
   - *Registered:* a forced-fallback test, part of G36.3's fallback clause and of PR B. With every optimistic attempt made to retry, all six methods on all three handle forms are compared with the model at present keys and at the boundary keys 0 and `u64::MAX`, and `ReadFallbacks` must equal the number of calls that reach the walk.
   - *Detection:* **stronger.**
10. **(review) The `FullExpanse` arm in release builds.**
    - *Found:* the arm's answer is right only for a full expanse at the level it is read at, without a skipped prefix; that is checked by a `debug_assert!` (C6), which is absent from a release build.
    - *Why it is unreachable today:* on a shared set the form is produced at level 1 only, and `SyncExpanseSet::new` is the sole constructor (§36.2, C6).
    - *Registered:* PR B may add a release guard in the set arm that answers `Retry` when the invariant does not hold, which sends the read to the locked route. It is permitted under P36.1 as it stands: no existing arm may move by more than 0.1%, and the new arms are judged by P36.2. If the guard is not added, the reachability argument is recorded in the rustdoc of `SyncExpanseSet::new` and of the arm. Either way no gate changes.
    - *Detection:* **same.** With the guard, a future violation becomes a fallback that `ReadFallbacks` shows.

**Recorded for P36.2.** The optimistic arms call `set.reader()` inside the measured function (`crates/expanse/benches/instructions.rs:3270`, `:3302`), once per arm against 50,000 operations, as the map's optimistic arms do. The locked twins register no reader. The pair is therefore not the same work to the instruction: the optimistic count includes one reader registration and its release. P36.2's claim and bound are unchanged.

**Also noted.** The direction of the walk is a literal macro argument in `set_ordered_walk!` (`sync.rs:6407`), which §36.5 allows.

**Unverified here.** Which outcome item 1 takes; whether the guard of item 10 is added; and that the host-guard pull request of item 6 has merged. Each is settled by its own pull request before the first dispatch, and the outcome subsection states it.

### 36.15 Outcome — the wall-clock verdict is PASS (appended 2026-10-14; §36.1 to §36.14 are not rewritten)

*(measured: reference host — Intel Core i9-12900F, 8P+8E / 24 threads, 0f88b715; pin `0,2,4,6,8,10,12,14`; 8 rounds per cell; admissible runs [38595664300](https://github.com/orieg/expanse/actions/runs/38595664300) and [38622944190](https://github.com/orieg/expanse/actions/runs/38622944190); `results/ordered_readers_set_writer_scaling.json` and `results/ordered_readers_set_writer_scaling_run2.json`; void runs `results/ordered_readers_set_writer_scaling_void1.json` and `_void2.json`; workload: `concurrency_ordered_readers_set_63bit`)*

Every dispatch measured PR B (#1504) at its head `0f88b715f0b0917adeaaf6fda13bafe8cf892144`, dispatched on `main` with that SHA as `ref` (§36.8, *Enforcement*). The tables below are generated by `docs/benchmarks/concurrency/scripts/tables.py` from the four artifacts and appear in `docs/benchmarks/concurrency/README.md` §33. Refs #1194, #900.

**Correctness gates on the measured head.** G36.2 to G36.5 and G36.7 to G36.9 are met for the parts assigned to PR A and PR B (#1497, #1498, #1504). Of G36.6, the Tier 1 test passed on the measured head's CI run [38590262139](https://github.com/orieg/expanse/actions/runs/38590262139); a census of the three `sync::miri_ub_sites::set_ordered_*` workloads at the manifest's range is not cited here. G36.1 is **met in part**, as §36.14 item 1 provides: the counts of rounds rejected under `set_drop_child_snapshots` and `set_header_before_sample` were not taken, because both switches are `#[cfg(test)]` and the histories are an integration target. The six live histories are shown to reject a wrong answer of one kind (a read answering for the wrong bound fails all six), and are not shown to reject a race-class defect. **Detection of race-class defects in the set walk rests on the deterministic gates G36.2 and G36.4**, with the Loom models of G36.5 for the validation rule. The full-expanse arm carries the release guard of §36.14 item 10.

**Dispatches.** Admissibility on §36.6's host clause is read from each artifact's `provenance.host_guard` (§36.12 item 4, §36.14 item 6). Four dispatches, two void, both replaced within §36.6's cap of three:

| dispatch | run | host guard, run verdict | worst foreign load on the pinned CPUs | admissible | run verdict |
|--:|---|---|--:|---|---|
| 1 | [38595664300](https://github.com/orieg/expanse/actions/runs/38595664300) | `undisturbed` | 0.216 | yes (run 1) | `RUN_PASS` |
| 2 | [38601123152](https://github.com/orieg/expanse/actions/runs/38601123152) | `disturbed`, one void window | 0.310 | no: above the 0.25 on-pin boundary | void |
| 3 | [38609023513](https://github.com/orieg/expanse/actions/runs/38609023513) | `disturbed`, one void window | 0.290 | no: same boundary | void |
| 4 | [38622944190](https://github.com/orieg/expanse/actions/runs/38622944190) | `undisturbed` | 0.069 | yes (run 2) | `RUN_PASS` |

The driver's own section of the two void artifacts reads `RUN_PASS`: the driver cannot see per-process load (§36.12 item 4), and those statistics are not read. The cause of both voids: a scheduled ssh/rsync session from another machine logs in to the reference host at the same minute of every hour. Dispatches 2 and 3 each contained one such session, which the guard counted as foreign load on the pinned CPUs; dispatch 1 contained one that stayed below the boundary; dispatch 4 was timed to finish before the next. It is host load the guard exists to count, and not a guard defect (established from the guard's code and the host's ssh log; no counter of this suite measures it).

**Gated statistics**, BCa 95% over the 8 per-round values, 2,000 resamples, seed 42 (workload: `concurrency_ordered_readers_set_63bit`):

| statistic | floor | run 1 | status | run 2 | status |
|---|--:|--:|---|--:|---|
| rho, `uniform`, W = 1, R = 4 | 1.0 | 1,329.18 [1,287.36, 1,371.70], `bca` | `MET` | 1,427.64 [1,350.11, 1,519.31], `bca` | `MET` |
| rho, `uniform`, W = 4, R = 4 | 1.0 | 1,705.83 [1,507.56, 1,828.83], `bca` | `MET` | 1,790.53 [1,620.24, 1,888.29], `bca` | `MET` |
| rho, `hotspot`, W = 4, R = 4 | 1.0 | 7.52 [6.63, 8.06], `bca` | `MET` | 6.87 [6.31, 7.67], `bca` | `MET` |
| kappa | 1.0, strictly above | 638.80 [620.79, 659.25], `bca` | `MET` | 688.56 [650.82, 735.57], `bca` | `MET` |

**P36.3 — HOLDS in both runs.** `read_fallbacks × 1000 < read_ops` in all eight `prev` cells, counters summed over the cell's rounds (§36.12 item 1). The largest share is `hotspot`, W = 1, R = 4: 247 of 1,281,605 reads in run 1 and 250 of 1,324,806 in run 2. Attempts per operation, reported and not gated, are 3.287 and 3.241 in that cell and 2.972 and 2.980 at W = 4; every `uniform` cell is at most 1.002. Five reads fell back in `uniform`, W = 4 in run 2, and none in any other `uniform` cell. Every cell is in `README.md` §33.4.

**Reported, not gated** (rho, BCa 95%, run 1 / run 2): `uniform` W = 0, R = 1, kappa's control, 2.08 [2.07, 2.09] / 2.07 [2.06, 2.08]; `uniform` W = 0, R = 4, 11.96 [11.83, 12.08] / 11.95 [11.82, 12.06]; `hotspot` W = 0, R = 1, 3.40 [3.23, 3.46] / 3.45 [3.45, 3.45]; `hotspot` W = 0, R = 4, 18.57 [18.24, 18.79] / 18.65 [18.46, 18.98]; `hotspot` W = 1, R = 4, 5.98 [5.23, 6.36] / 6.70 [6.29, 7.91] (workload: `concurrency_ordered_readers_set_63bit`).

**Form coverage.** The four `hotspot` cells with writers ended every row of both runs with 256 full expanses: their form-coverage statements stand (§36.12 item 3, §36.14 item 5). No `uniform` cell ended with one, as §36.6 derived.

**Host comparability.** The `prev_locked` control at `uniform`, W = 0, R = 1, which this change does not touch, read 5.944 [5.914, 5.956] Mops/s in run 1 and 5.947 [5.937, 5.952] in run 2. Not a gate.

**The pair verdict: PASS.** Both admissible runs are `RUN_PASS`: every statistic `MET` and P36.3 `HOLDS` (§36.8 items 1 to 3), so the pair is `PASS` by §36.8 item 4. `writer_scaling.py --set-ordered-readers-verdict` on the two admissible artifacts returns `{"pair": {"state": "COMPLETE", "verdict": "PASS"}}`. That command reads each artifact's `run_verdict` and not its `host_guard`, so the two void dispatches are excluded by the guard's record before it runs, as this subsection's table shows; the command alone would not exclude them.

**What ships, and what does not follow.**

- Per §36.8, PR B merges with this subsection. The measured code is what merges: `git diff 0f88b715f0b0917adeaaf6fda13bafe8cf892144 HEAD -- crates/ include/ bindings/` is empty on the merged branch.
- The losing cells of §36.7, beside the gain: the gate ratios at W ≥ 1 are those of a locked route whose readers wait for writers, and are not a per-call cost. Per call, a validated read counts more instructions than the single-threaded `ExpanseSet` read: `sync_set_prev/random` 664.0 per operation against `set_prev/random` 454.0 (measured: GitHub-hosted ubuntu x86_64 runner, `0f88b715`; CI run [38590262139](https://github.com/orieg/expanse/actions/runs/38590262139); workload: `core_instructions`). The smallest ratio of the sweep is the W = 0, R = 1 control, about 2. The `hotspot` cells with writers stay far below the `uniform` ones, 6.87 to 7.52 at the gate cell. A whole scan by `first` and `next_after` against one `with_locked` iteration is not measured.
- Not decided here: the forward reads in wall clock, the owned and detached handles in wall clock, other W, the 32-bit set, a set cursor, and the C ABI and binding surfaces (§36.9, §36.1).
- G36.1 met in part stays the record: any statement built on this verdict says that race-class detection in the set walk rests on G36.2 and G36.4.

## 37. Pre-registration for #1479 — do the writer-side wait loops gain from yielding? (appended 2026-10-13, locked before any code or run of the builds below; next free section, may be renumbered at merge)

Three wait loops in the concurrent engine spin on `core::hint::spin_loop()` with no yield, no park and no iteration bound in a non-Loom build (#1479). The decision on that issue is to measure first and to change code only if the measurement shows a gain. This section registers that measurement. It holds no engine code and no data. The runs are taken on the reference host after the verdict runs of §36. Code references are to `origin/main` at `7c5c6f2f3`. Refs #1479.

### 37.1 The question, and what it is not

**The question.** When a writer waits at a closed gate, or a serialised section waits for in-flight writers to drain, does yielding the CPU after a short spin shorten the holder's section or raise throughput, in the one situation where a spinning waiter can take time from the thread it waits for: more runnable threads than CPUs?

**What it is not.**

- Not whether the protocol should block. It blocks by design (AGENTS.md §2.2): writers serialise, a sample waits out an open bracket, readers fall back to a lock. Nothing here proposes lock-freedom, a park queue or a condition variable.
- Not #1222. That starvation was of threads waiting on `fallback_mutex`, a parking mutex, behind an unpaced compaction loop; it did not involve these loops (#1479, *Consequence*).
- Not a diagnosable bound on a stuck gate. §37.8 says where that is decided.

**The loops** (`verified`).

| Loop | Where | Waits for | Reached from | Holder |
|---|---|---|---|---|
| `Shared::enter_writer_blocking` | `crates/expanse/src/sync.rs:2516-2549`; as a call, `:2554` | the gate to reopen | every optimistic mutation of every 64-bit wrapper, 15 call sites (`sync.rs:4178` to `:14777`), and the diagnostic probe (`:2632`) | whichever thread closed the gate in `quiesce_writers` and has not dropped its `QuiesceGuard` |
| `WriterGate::wait_drained` | `crates/expanse/src/occ.rs:1106-1112` | one writer slot's in-flight count to reach zero | `Shared::quiesce_writers` (`sync.rs:2451-2474`), once per allocated slot, from every serialised section: `write_root_covered_with` (`:2820`), `write_quiesced_staged` (`:2919`), `read_locked` (`:3009`), `with_locked` (`:3035`), `with_locked_pre` (`:3092`) | an optimistic writer inside the gate. The waiter already holds `fallback_mutex`, and in `read_locked` and `with_locked` goes on to take the writer mutex |
| `SeqVersion::sample` | `occ.rs:132-158` | the tree word to be even | every optimistic read and write attempt, through `Shared::optimistic_read` and its siblings in `sync.rs`, `strmap.rs` and `sync_cursor.rs` | a thread inside a tree-level bracket (a root-state change, a root publish) |

Per-node versions do not spin: `node_sample` answers `None` on an odd word and the caller restarts (`occ.rs:1155-1158`).

### 37.2 Scope: the two writer-side waits; `SeqVersion::sample` is measured, not changed

**In the variant:** `enter_writer_blocking` and `wait_drained`.

**`SeqVersion::sample` is excluded from the variant,** for three reasons, and included in what is measured.

- It is inlined into every read and write attempt. A slow path added to it changes the code of every caller, so every `sync_*` Callgrind arm is at risk under AGENTS.md §2.1 invariant 5; the writer-side waits are reached once per mutation and once per serialised section.
- It compiles without `std` (`occ.rs` has no `std` gate on it), where there is no thread to yield to; the two writer-side waits exist only with `std` (`sync.rs:2450`, `:2514`).
- Its holder's section is a tree-level bracket, which since #568 PR 3 is open only while one frame stores, and since §30 no longer spans a compaction's copy. A wait there is long only if the holder loses its CPU inside the bracket. Whether that happens often enough to matter is unmeasured.

So the counters pass of every cell reports `sample_spins` and `sample_spin_cycles` beside `gate_wait_cycles` and `quiesce_drain_cycles` (`crates/expanse/src/occ_stats.rs:67`, `:95`, `:170`, `:174`), each as a share of the window's thread time. No verdict reads them. A change to `sample` needs a registration of its own, made against those figures.

### 37.3 The builds

Cargo features, by the conventions of AGENTS.md §2.7. The default stays what ships.

| Build | Feature | The two writer-side waits |
|---|---|---|
| default | none | `spin_loop` until the condition holds, as on `main` |
| variant | `ablation-gate-spin-then-yield` | the same first check; on its failure, an out-of-line slow path that calls `spin_loop` for the next `N` failed checks and `std::thread::yield_now()` once after every failed check from then on |

- **N = 64.** It is the spin budget the engine already uses: the mutation loops' backoff is capped at 64 `spin_loop` calls (`sync.rs:2654-2659`) and `MAX_RETRIES` is 64 (`sync.rs:165`). It was chosen to add no new magnitude, and it was not tuned. Another `N` is another variant and needs its own registration; a sweep over `N` after seeing a result is the tuning AGENTS.md §8.19 prices.
- **The slow path is out of line.** The first check stays where it is. Everything after its failure is in a `#[cold]`, `#[inline(never)]` function, in the variant build. In the default build the feature's code does not exist.
- **Under Loom** both builds keep the `loom::thread::yield_now()` the loops have today.
- **On `PASS` the variant becomes the default,** under §2.7 in full. Two names are fixed now. Before promotion the variant is `ablation-gate-spin-then-yield`. After promotion that name would describe the default, so it is retired to a `compile_error!`, and the inverse feature `ablation-gate-spin-only` restores today's spin-only loops, so the new default stays re-testable. Every consumer (CI jobs, `scripts/gate.sh`, the drivers' feature maps, `.github/bench-suites.json`, the dispatch `case`, the docs) migrates in the same pull request.

**Gates on the pull request that adds the feature,** before any wall-clock run:

- *P37.1, default build.* Every Callgrind arm that exists on the base changes by at most 0.1% (AGENTS.md §6), and the bench binary's disassembly differs from the base's in nothing but anonymous symbol hashes.
- *P37.2, variant build.* A Callgrind pass of the `instructions` bench built with the feature, against the default build of the same commit: every `sync_*` arm and every plain-tree arm within 0.1%. The benches are single-threaded, so the slow path is never entered; what this checks is that the out-of-line call did not move the callers.
  - *How it is taken.* No job runs that bench under a feature list today: every `cargo bench --bench instructions` in the workflows is the default build (`.github/workflows/bench_baremetal.yml:1054`, `:1174-1189`, `:1508-1510`), and no `--features` line of `ci.yml` names a bench. The implementation pull request therefore adds a temporary step to its own `instruction-counts` job, on the same runner: the default build saved as a baseline, then `cargo bench --bench instructions -p expanse-trie --features ablation-gate-spin-then-yield` compared against it. That step's log is the cited evidence. The step is removed before merge, with the `allow-gate-weakening: ci-integrity` line the removal of a verification step needs, unless the verdict promotes the variant, in which case the default pass covers it.
- *Disassembly.* `objdump -d -C` of both builds: the count of `%fs:` reads in `nav::next`, `nav::prev` and `mutate::insert_with_path_flat` is unchanged, and the body of each caller of `enter_writer_blocking` differs only by the call into the slow path.
- *Loom.* The Loom lane passes with the feature on.
- **Falsifier:** any arm above 0.1% in either pass. The experiment does not run on a build that fails these.

### 37.4 Instrument and cells

**Primary instrument: `reclaim_stall`** (§29.2), the `SyncExpanseBlobMap` overwrite cells. It is the instrument #1479 names, and it has a holder of known length: an insert the cap refuses compacts the arena inside `write_quiesced_staged` (`sync.rs:11118`), with the gate closed, and that insert's latency is the window's stall. At 200,000 live records the stall read 31.9 to 33.5 ms across W at `d71e65bd` (measured: `results/baseline_reclaim_stall.json`, §29 of the README; workload: `reclaim_stall`). *(Appended 2026-10-20: the W = 4 and 12 part of that range is withheld pending re-measurement (#1320): writer pairs on identical key streams, README §29's † note; the W = 1 figure, 31.9 ms, is unaffected.)* Every other writer of the cell waits in `enter_writer_blocking` for that long, twice per window.

**How oversubscription is produced: by the pin and the thread count, with no load generator.** The driver's pin is set to the eight physical P-cores, `0,2,4,6,8,10,12,14`, one logical CPU each. A cell's threads are its W writers and R readers. A cell with more threads than 8 is oversubscribed by its own threads; one with at most 8 is not. Nothing outside the harness's process tree is started.

| Cell | Role | W | R | Threads on 8 CPUs | Live records |
|---|---|---|---|---|---|
| **O** | primary: oversubscribed | 12 | 2 | 14 | 200,000 |
| **C** | control: not oversubscribed | 4 | 2 | 6 | 200,000 |
| O-large | reported | 12 | 2 | 14 | 3,600,000 |
| S | reported: the §29 pin `0-15`, 14 threads on 16 logical CPUs of 8 cores | 12 | 2 | 14 | 200,000 |

- Cell C is where a yield should change nothing: every thread has a CPU, so a waiter that yields has nobody to yield to and the holder was never waiting for a CPU.
- Cell S is reported because there a spinning waiter shares a physical core with the holder without displacing it. No direction is predicted for it.
- AGENTS.md §8.20.5 step 0: both builds of a cell run under the same pin, and every published figure names it. The comparison is between builds, never between pins.

**Second instrument, for harm: `writer_scaling`'s count cell** (§23). (W, R) = (4, 1), `--read-op count_locked`, `--probe uniform`, pin `0,2,4,6,8,10,12,14`: four writers and one reader that calls `count_below` through `with_locked` in a loop. Every count closes the gate for a short section. This is the regime where a yield can only cost: the wait is shorter than a system call. It is cell **H**.

**Harness prerequisites,** each a change to a driver or a workflow, none to the engine, all merged to `main` before the dispatch as §36.1 item 4 was:

- *Builds.* `reclaim_stall.py` builds one binary, with no feature list (`docs/benchmarks/concurrency/scripts/reclaim_stall.py:70-73`). It gains a build list (default, and the feature), runs both builds of a cell in each round, and labels every row with its build.
- *Pin.* The driver pins itself once through `bench_pin.apply` (`reclaim_stall.py:178`), which honours `EXPANSE_BENCH_PIN=<list>` (`scripts/bench_pin.py:28`); the suite's dispatch `case` sets none, so today's runs take the host's P-core list, `0-15` (`bench_baremetal.yml:1502-1504`). Cells O, C and O-large need `0,2,4,6,8,10,12,14` and cell S needs `0-15`, in one run. The driver gains a pin per cell, applied to the child process, recorded in each row and refused if the applied list differs.
- *Cells.* Its cell table is fixed (`reclaim_stall.py:38-45`) and holds §29's cells. This experiment is a mode of its own with cells O, C, O-large and S, a suite name of its own and an artifact of its own; §29's cells and baseline artifact are not touched.
- *The harm cell.* `writer_scaling.py` already builds a feature list against the default for its ablation sweeps (`docs/benchmarks/concurrency/scripts/writer_scaling.py:321-331`) and gains this one cell pair.

### 37.5 Statistics, fixed now

- **Rounds and processes.** 8 rounds per run. One process per (build, cell, round). Within a round the two builds of a cell run back to back, and which goes first alternates by round; cell order rotates by round as §29.2 does.
- **Per-round values.**
  - *Stall* `S`: the window's largest insert latency (§29.3).
  - *Throughput* `T`: the window's overwrites divided by its duration, all writers together.
  - *Cell H:* writer throughput in Mops/s.
- **Paired statistics,** each a per-round ratio of the two builds of one cell, so above 1.0 favours the variant:
  - `g_stall` = `S` default ÷ `S` variant, in cells O and C;
  - `g_thr` = `T` variant ÷ `T` default, in cells O and C;
  - `h` = writer throughput variant ÷ default, in cell H.
- **The primary statistic is `g_stall` in cell O:** the stall, the longest pause of the window. It is the holder's section, which is the thing a yield is hypothesised to shorten. Total throughput, `g_thr`, is reported beside it and decides nothing.
- **One oversubscription level.** Cell O, 12 writers and 2 readers on 8 pinned CPUs, is the only oversubscribed gate cell. No second level is run.
- **Intervals.** BCa 95% over the 8 per-round ratios, 2,000 resamples, seed 42 (`scripts/bca_bootstrap.py`), recorded with `ci_method`.
- **The floor for "a gain"** (AGENTS.md §8.20.2: a gain under contention that the uncontended control does not share). In one run: the lower bound of `g_stall` in cell O is above 1.0, **and** the interval of `g_stall` in cell C contains 1.0.
- **The floor for "no harm".** In one run: the upper bound of `h` is not below 1.0.
- **What latency the suite can resolve.** A window at 200,000 live records makes about 16 million overwrites (2.2 cycles of 7,256,256 appends, §29.2) and holds two compactions. At most `2 × (W − 1)` operations per window wait at the gate for a compaction: 22 of 16 million at W = 12. They sit above the 99.9999th percentile, so p99 and p99.9 do not contain them and are not a statistic of this registration. They are reported, as §29 reports them. The gate wait is read from the stall and from the eight largest latencies each row already records.
- **Runs.** Two independent dispatches form a pair. A claim needs the same per-run verdict in both (`docs/BENCHMARKING.md` rule 18).
- **What voids a run.** A build that failed §37.3's gates; a pin other than the registered one; a round of an overwrite cell that did not complete exactly two compactions or refused an insert (§29's admissibility); a job the workflow's host guard failed (`scripts/bench_host_guard.py`: foreign load of 1.0 core-equivalent, 0.25 on the pinned CPUs, or a foreign process above 100%; `scripts/bench_provenance.py:127-135`). Foreign means outside the run's own process tree, so cell O's fourteen threads on eight CPUs do not count against it. No load-average bound is registered: cell O's own threads put it above any such bound by design.
- **Replacements and a second pair,** as §36.6 and §36.8 set them: at most three replacement runs per pair, for void runs only; and after an `INTERMEDIATE` first pair, exactly one more pair of fresh runs, whose verdict is final. At most ten dispatches.

### 37.6 Verdict

1. *Per run, cell O:* `GAIN` when the lower bound of `g_stall` is above 1.0; `LOSS` when its upper bound is below 1.0; `OPEN` otherwise.
2. *Per run, cell C:* `FLAT` when the interval of `g_stall` contains 1.0; `MOVED` otherwise.
3. *Per run, cell H:* `HARM` when the upper bound of `h` is below 1.0; `NO_HARM` otherwise.
4. *Per run,* three conditions, all required for promotion: `RUN_PASS` when O is `GAIN` **and** C is `FLAT` **and** H is `NO_HARM`. `RUN_REJECT` when O is `LOSS` or H is `HARM`, whatever the other cells show. `RUN_OPEN` in every other case, which includes a `GAIN` beside a control that `MOVED`.
   - The harm cell blocks promotion: a run in which O is `GAIN` and H is `HARM` is `RUN_REJECT`. Its gain is still published, with its interval, beside the harm cell's.
5. *The pair:* `PASS` for two `RUN_PASS`; `REJECTED` for two `RUN_REJECT`; `INTERMEDIATE` for every other combination and for a pair not valid after its replacement runs.

| Verdict | What it means |
|---|---|
| `PASS` | the variant becomes the default, in its own pull request, under AGENTS.md §2.7 and §37.3's gates and names |
| `REJECTED` | the variant is not promoted and the loops stay as they are, including when the primary cell gained and the harm cell lost. The outcome is recorded here and on #1479, with every cell, and the feature is removed or kept only as a diagnostic |
| `INTERMEDIATE`, after the second pair | not promoted. Recorded as inconclusive, with the intervals; the loops stay |

A verdict describes the build it measured (AGENTS.md §8.20.6). The stall this experiment rests on is the compaction's copy; a change that shortens or splits that copy (#1320) removes the long holder, and the verdict then expires and is not carried to the new engine.

### 37.7 Expected losses, and what is not predicted

Every reason is a hypothesis.

| Cell | Expected | Reason (hypothesis) |
|---|---|---|
| C | no change | no thread lacks a CPU |
| H | a loss is possible | each gate closure is short; a waiter that passes 64 spins pays a system call for a wait that was about to end, and comes back to a colder cache |
| O, readers | read latency may rise | a yielding writer gives its CPU to any runnable thread, readers included, which lengthens nothing for the holder but changes who runs |
| O | the gain may be absent | `sched_yield` moves the caller behind other runnable threads of its CPU; it does not hand the CPU to the holder, which may be queued on another CPU |
| S | unknown | the waiter and the holder may share a core's two logical CPUs; `spin_loop` already relaxes the sibling |

**Not predicted:** any magnitude; `g_thr`; the share of time in `sample`; behaviour on hosts with other core counts, other schedulers or other operating systems (the run is one Linux host); a park-based wait, which is not a variant here.

**Silent constants,** all fixed here: N = 64; a yield after every failed check beyond N; the pin; W, R and live records of each cell; 8 rounds; 2,000 resamples, seed 42; floors 1.0; the 0.1% Callgrind bound; the host guard's three boundaries; three replacement runs per pair; one second pair. Changing any of them after a run relabels that run `INTERMEDIATE` and requires fresh runs (AGENTS.md §8.19).

### 37.8 The bound on a stuck gate is a separate question

#1479 also notes that the two writer-side waits have no bound and no diagnostic. A bound that panics with a named message after a very large wait is not a performance question, and this registration does not decide it. No measurement here bears on it: a correct gate always reopens (`QuiesceGuard`, pinned by `tests/test_serialised_section_unwind.rs`), so the bound never fires in any cell above.

It is decided on #1479 as its own item, by review, on three points this section only lists: what the bound counts (spins or elapsed time: a 300 ms compaction is already millions of spins, so a spin count risks a false panic); whether a panic inside a writer is an acceptable failure for a library; and its gate, which would be P37.1 as worded above plus a test that a gate deliberately left closed produces the named message.

### 37.9 Open questions

None. The section number, 37, is the next free one at lock time and may move at merge.

### 37.10 Outcome (appended 2026-10-15; §37.1 to §37.9 are not rewritten)

The variant build of §37.3 was implemented in #1540 and failed P37.2: across the 37 `sync_*` Callgrind arms the largest change was +1.84%, against the 0.1% bound ([CI run](https://github.com/orieg/expanse/actions/runs/38724709931)). Per §37.3 the experiment does not run on a build that fails its cost check, so no §37 cell ran and no verdict was evaluated: the registered question is neither PASS nor REJECTED. #1540 was closed unmerged, and the writer-side waits stay as on `main`. The question remains with #1479.

The premise it tested, that the two writer-side waits and `SeqVersion::sample` take a material share of time, is measured on `main` without the variant: `reclaim_stall.py --wait-counters` records each sync stall window's `gate_wait_cycles`, `quiesce_drain_cycles`, `sample_spins` and `sample_spin_cycles` from an `occ-stats` build, with each share taken against the window's summed per-thread CPU time (`CLOCK_THREAD_CPUTIME_ID`) and not wall time times threads. The counters are TSC deltas and include time a waiting thread was descheduled, so a share can exceed the waits' CPU cost. No verdict reads them.

---

## 38. Pre-registration for 32-bit set ordered reads on `SyncExpanseSet32` (#1194)

**Status:** Stage 1 pre-registration (`AGENTS.md` §8.19). Plan only; no production code in `crates/expanse/src/sync32.rs` or `crates/expanse/src/trie32.rs` is changed by this section. All numerical thresholds below are `(target)` gates locked before implementation (`AGENTS.md` §1.6, §8.7). Refs #1194, #900.

Section 36.1 (`docs/benchmarks/concurrency/METHODOLOGY.md:5390`) explicitly carved out the 32-bit concurrent set (`crates/expanse/src/sync32.rs`, `SyncExpanseSet32`) from the 64-bit `SyncExpanseSet` ordered-read pre-registration because its concurrency protocol, node layout, failure contract, and target hardware differ fundamentally from the 64-bit engine. This section pre-registers the design, site-by-site validation ledger, substitution twin, correctness gates, deterministic Callgrind arms, and host/on-device evaluation protocol for ordered reads on `SyncExpanseSet32`.

### 38.0 Pre-condition gate (G38.0) and architectural relationship to §12 and §36

1. **Sequencing pre-condition (G38.0).** Stage 2 implementation of §38 does not start until the 64-bit `SyncExpanseSet` ordered-read rollout (§36, #1497 / #1498 / #1504) has completed its Stage 2/3 gates and recorded its verdict.
2. **Why the 64-bit walk (`sync_nav.rs`) does not port structurally to 32-bit (`trie32.rs`).**
   - **64-bit `SyncExpanseSet` (`crates/expanse/src/sync.rs` + `crates/expanse/src/sync_nav.rs`):** Multi-writer / multi-reader optimistic concurrency control with per-node `SeqVersion` words, a 13-slot `ReadSet` (`ORDERED_READ_SET_CAP = 13`, §36.2) that tracks branch versions across backtracking, an internal retry loop (`MAX_RETRIES = 64`), and a mutex fallback (`read_locked`) that guarantees an infallible `Option<u64>` return. Its trie carries root leaves (`Header` with `ROOT_LEAF_CAP`), narrow pointers encoding skipped prefix bytes in the edge, and `ImmedType::FullExpanse` 256-bit terminals.
   - **32-bit `SyncExpanseSet32` (`crates/expanse/src/sync32.rs:314`, `Sync32<ExpanseSet32>`):** Single-writer / many-reader wrapper split via `Sync32::split(&mut self)` (`crates/expanse/src/sync32.rs:370-385`) into one non-clonable `Writer32<'_, ExpanseSet32>` (`sync32.rs:451, 815`) and a `ReaderPool32<'_, ExpanseSet32>` (`sync32.rs:390`) handing out `Reader32<'_, ExpanseSet32>` handles (`sync32.rs:414, 945`). It uses **zero atomic read-modify-write instructions** (`sync32.rs:4-17`) so that it compiles and runs natively on `riscv32imc` (`ESP32-C2`/`C3`) and ARMv6-M/v7-M targets without CAS or mutex primitives.
   - **Tree-level seqlock (`SeqVersion32`) and single-attempt `Busy` contract:** `Sync32<T>` carries a single tree-level version word `version: PaddedVersion(SeqVersion32)` (`crates/expanse/src/sync32.rs:199, 260`; `crates/expanse/src/occ32.rs:14-60`). Nodes in `trie32` carry no per-node version words (`docs/design/32-bit-embedded.md:34, 214, 225`). Every `Reader32` query is **single-attempt and interrupt-safe** (`sync32.rs:39-51`): inside `Reader32::pinned` (`sync32.rs:423-448`), the reader marks its own cache-line-padded `ReaderSlot::walk: AtomicU32` (`sync32.rs:190-194`) odd to hold deferred reclamation (`Arena::pending`, `trie32.rs:1469-1490`, drained by `Writer32::try_reclaim` at `sync32.rs:502-559`), samples `owner.version.0.try_sample()`, resolves node handles through the published slot table `PubTable` (`trie32.rs:1186-1189, 1320-1341`, `PubSlot { kind: AtomicU32, ptr: AtomicPtr<u8>, len: AtomicU32 }` at `trie32.rs:1130-1134`), and returns `Err(Busy)` immediately if `version` is odd on entry or changes before the final `seal`.
   - **Structural template:** The direct architectural sibling of `SyncExpanseSet32` ordered reads is **`SyncExpanseMap32`'s validated ordered-seek path** (`trie32::map_seek_validated` at `crates/expanse/src/trie32.rs:5069-5086`, `Seek32` at `trie32.rs:5052-5057`, `seek_children` at `trie32.rs:5199-5320`, and `seek_first` / `seek_last` / `seek_after` / `seek_before` at `trie32.rs:5322-5497`), which landed under §12 (#900) and is stress-tested in G12.5 (`crates/expanse/tests/sync32_stress.rs:188-358`). What transfers from §36 (#1194) is the verification methodology: the exhaustive site ledger (§38.2), pause-point negative controls (§38.4), and twin/baseline winning-regime evaluation (§38.3, §38.6).

### 38.1 Scope, API surface, and named exclusions

1. **Existing 32-bit ordered-read surface (verified by `READ`).**
   - Single-threaded `ExpanseSet32` (`crates/expanse/src/set32.rs`) already provides `first` (`set32.rs:261`), `last` (`set32.rs:267`), `next` (`set32.rs:274`), `prev` (`set32.rs:281`), `next_at_or_after` (`set32.rs:389`), `next_after` (`set32.rs:396`), `prev_at_or_before` (`set32.rs:403`), and `prev_before` (`set32.rs:410`), backed by `trie32::first` (`crates/expanse/src/trie32.rs:5938`), `trie32::last` (`trie32.rs:6317`), `trie32::next` (`trie32.rs:6354`), and `trie32::prev` (`trie32.rs:6424`), plus `iter` (`set32.rs:440`), `range` (`set32.rs:451`), `iter_rev` (`set32.rs:466`), and `range_rev` (`set32.rs:477`) over `RawIter32` (`trie32.rs:5542`).
   - Concurrent `SyncExpanseMap32` (`crates/expanse/src/sync32.rs`) already provides the six infallible ordered queries on `Writer32<'_, ExpanseMap32>` (`sync32.rs:770-813`) and the six validated `try_*` ordered queries on `Reader32<'_, ExpanseMap32>` (`sync32.rs:887-943`, currently `#[doc(hidden)]`).
   - Concurrent `SyncExpanseSet32` (`Writer32<'_, ExpanseSet32>` at `sync32.rs:815-853` and `Reader32<'_, ExpanseSet32>` at `sync32.rs:945-977`) currently exposes only point membership (`contains` / `try_contains`), cardinality (`len` / `try_len`, `is_empty` / `try_is_empty`), and writer mutations (`try_insert`, `try_remove`, `clear`).
2. **In-scope methods on `Writer32<'_, ExpanseSet32>` (`crates/expanse/src/sync32.rs:815-853`).**
   Because the single `Writer32` performs all mutations and holds an exclusive borrow of the underlying `ExpanseSet32` whenever no write bracket is open, its six ordered queries are exact, infallible, and delegate directly to `self.inner()` (mirroring `Writer32<'_, ExpanseMap32>` at `sync32.rs:770-813`):
   - `pub fn first(&self) -> Option<Key32>` $\to$ `self.inner().first()`
   - `pub fn last(&self) -> Option<Key32>` $\to$ `self.inner().last()`
   - `pub fn next_at_or_after(&self, bound: Key32) -> Option<Key32>` $\to$ `self.inner().next_at_or_after(bound)`
   - `pub fn next_after(&self, bound: Key32) -> Option<Key32>` $\to$ `self.inner().next_after(bound)`
   - `pub fn prev_at_or_before(&self, bound: Key32) -> Option<Key32>` $\to$ `self.inner().prev_at_or_before(bound)`
   - `pub fn prev_before(&self, bound: Key32) -> Option<Key32>` $\to$ `self.inner().prev_before(bound)`
3. **In-scope methods on `Reader32<'_, ExpanseSet32>` (`crates/expanse/src/sync32.rs:945-977`).**
   Each method performs a single pinned, seqlock-validated attempt and returns `Result<Option<Key32>, Busy>` (mirroring `Reader32<'_, ExpanseMap32>` at `sync32.rs:887-943`):
   - `pub fn try_first(&mut self) -> Result<Option<Key32>, Busy>` $\to$ `self.try_seek(Seek32::First)`
   - `pub fn try_last(&mut self) -> Result<Option<Key32>, Busy>` $\to$ `self.try_seek(Seek32::Last)`
   - `pub fn try_next_at_or_after(&mut self, bound: Key32) -> Result<Option<Key32>, Busy>` $\to$ if `bound == 0`, `self.try_first()`, else `self.try_seek(Seek32::After(bound - 1))`
   - `pub fn try_next_after(&mut self, bound: Key32) -> Result<Option<Key32>, Busy>` $\to$ if `bound == u32::MAX`, enters `self.pinned(|owner| { owner.version.0.try_sample().ok_or(Busy)?; Ok(None) })`, else `self.try_seek(Seek32::After(bound))`
   - `pub fn try_prev_at_or_before(&mut self, bound: Key32) -> Result<Option<Key32>, Busy>` $\to$ if `bound == u32::MAX`, `self.try_last()`, else `self.try_seek(Seek32::Before(bound + 1))`
   - `pub fn try_prev_before(&mut self, bound: Key32) -> Result<Option<Key32>, Busy>` $\to$ if `bound == 0`, enters `self.pinned(|owner| { owner.version.0.try_sample().ok_or(Busy)?; Ok(None) })`, else `self.try_seek(Seek32::Before(bound))`
   All twelve methods land `#[doc(hidden)]` in Stage 2 (matching `SyncExpanseMap32`'s ordered methods at `sync32.rs:771-937`) and become public only after Stage 3 gates G38.7 and G38.8 pass.
4. **Named exclusions (out of scope for §38).**
   - **Stateful cross-call cursors or heap-allocating batch iterators on `Reader32`:** `SyncExpanseSet32` pre-allocates its entire node and pending-free capacity in `SyncExpanseSet32::with_capacity(node_cap, max_readers)` (`sync32.rs:330-346`; `docs/design/32-bit-embedded.md:14-48`). Holding a reader pin across calls would stall `Writer32::try_reclaim` and exhaust `MUTATION_HEADROOM` (`128`, `sync32.rs:167`), while allocating an internal scratch buffer would violate the zero-post-init-allocation guarantee. Multi-element scans use caller-driven `try_next_after` stepping (or, if needed later, a separately pre-registered zero-allocation `try_range_into(range, out: &mut [Key32]) -> Result<usize, Busy>`).
   - **C ABI (`expanse32.h` / `ffi32.rs`):** `crates/expanse/src/ffi32.rs` wraps single-threaded `ExpanseMap32` and `ExpanseSet32`, not `Sync32`.
   - **Concurrent `try_count_range` / `rank` / `select` on `Reader32`:** Excluded here so §38 is strictly scoped to the six ordered neighbour queries.

### 38.2 Structural invariants and site-by-site validation ledger (`trie32::set_seek_validated`)

Before enumerating the validation sites, three structural properties of `crates/expanse/src/trie32.rs` distinguish `set_seek_validated` from the 64-bit `sync_nav.rs` ledger in §36.2:

1. **One tree-level `SeqVersion32` covers backtracking without a `ReadSet`.** In `sync32`, every mutation on `Writer32` opens the single tree-level seqlock bracket on `owner.version` (`Writer32::write` at `sync32.rs:475-497`). When `seek_after` or `seek_before` descends into digit `da` / `db`, finds no qualifying key in that child (`Ok(None)`), and backtracks into `seek_children` to visit the next or previous populated sibling digit (`trie32.rs:5421-5431, 5482-5492`), both subtrees are governed by the single version snapshot `snap: u32` taken at the start of `try_seek`. No per-branch `ReadSet` (§36.2 Classes B and C) is required; the final `seal(still_valid, found)` at the exit of `set_seek_validated` proves that the entire root-to-terminal walk and every subtree skipped as empty coexisted in commit `snap`.
2. **Fixed 1-byte-per-level radix (`kb` from `4` down to `1`), no root leaf, no narrow pointers.** `ExpanseSet32`'s root is an `Edge32` at level `kb = 4` (`crates/expanse/src/set32.rs:34`). Every branch transition consumes exactly one key byte (`kb -= 1`, `trie32.rs:4034-4059`). Neither root leaves nor narrow pointers (skipped prefix bytes inside an edge) exist in `trie32`.
3. **No `FullExpanse` kind.** `trie32::Kind` (`crates/expanse/src/trie32.rs:3880-3889`) has eleven variants (`Null`, `SetImmed`, `MapImmed`, `SetLeaf`, `MapLeaf`, `Bitmap`, `MapBitmap`, `BranchL2`, `BranchL6`, `BranchB`, `BranchU`). A 256-key 1-byte expanse is stored as a `Kind::Bitmap` (`LeafBitmap1_32` with `pop0 = 255`, `trie32.rs:647-662`).

#### Site ledger for `trie32::set_seek_validated`

| Site ID | `trie32` construct | Difference vs `map_seek_validated` (`trie32.rs:5069-5497`) | Concurrent writer hazard | Validation rule before address/index formation & before return |
|---|---|---|---|---|
| **S0** | `Reader32::<ExpanseSet32>::try_seek` & `trie32::set_seek_validated` | Returns `Result<Option<Key32>, Torn>` instead of `Result<Option<(Key32, Value32)>, Torn>`. | Writer opens or advances `owner.version` during `PubEdge::load` of `owner.root` or during the walk. | Enter `self.pinned(...)` (`sync32.rs:423-448`); sample `snap = owner.version.0.try_sample().ok_or(Busy)?`; load `root = owner.root.load()`; check `still_valid()` before dispatching on `root`; check `seal(still_valid, found)` before returning `Ok(found)` (unless `#[cfg(all(test, feature = "std"))] seek_hooks::skips_seal()` is active). |
| **S1** | `Kind::Null` | Identical (`Ok(None)`). | Torn edge tag from an unvalidated edge load. | Reached only after the edge (`root` at `kb = 4` or child `c` in `seek_children` at `trie32.rs:5224, 5238, 5292, 5309`) has been validated by `still_valid()`. |
| **S2** | `Kind::SetImmed { kb: _, count }` (`trie32.rs:3481-3554, 4989-4995`) | `Kind::MapImmed` holds 1 key in `e.w[0]` and 1 value in `e.w[1]` (`trie32.rs:3470-3479`). `Kind::SetImmed` packs up to `set_immed_cap(kb) = 7 / kb` sorted `kb`-byte remainders across `e.w[0]` (bytes `1..4`) and `e.w[1]` (`aux`, bytes `0..4`). | A 2-word `Edge32` load (`PubEdge::load` or `word::load_edge`) can observe a torn `(w[0], w[1])` pair where `count` in the tag byte of `w[0]` exceeds `set_immed_cap(kb)` before `still_valid()` runs, or `count == 0`. | Edge `e` is validated by `still_valid()` before dispatch. Clamp `let n = (count as usize).min(set_immed_cap(kb)) as u8;` (matching `set_contains_validated` at `trie32.rs:4992`); if `n == 0`, return `Err(Torn)`. Decode `(keys, n) = set_immed_keys(e, kb, n)` (`keys[..n]` is sorted ascending, `trie32.rs:3500-3526`): `First` $\to$ `keys[0]`; `Last` $\to$ `keys[n - 1]`; `After(k)` $\to$ `keys[..n].iter().copied().find(\|&r\| r > k)`; `Before(k)` $\to$ `keys[..n].iter().rev().copied().find(\|&r\| r < k)`. |
| **S3** | `Kind::SetLeaf(_)` (`trie32.rs:4996-5012`) | `Kind::MapLeaf(_)` (`seek_leaf` at `trie32.rs:5092-5116`) prefixes the key array with `keys_off = 4 * cap_class(pop)` value bytes and reads `v.val(i)`. `Kind::SetLeaf(_)` stores packed `kb`-byte remainders at byte offset `0` (`keys_off = 0`) with no value region. | Writer edits the leaf in place (`set_leaf_insert` / `set_leaf_remove_span`, `trie32.rs:3608-3695`) via `word` stores, or reallocates/demotes the leaf and retires the old handle to `Arena::pending`, or edge `pop` disagrees with `PubSlot::len`. | In `seek_set_leaf(a, e, kb, still_valid)`: (1) read `pop = edge_pop(e)`; if `pop == 0`, return `Err(Torn)`; (2) resolve `node = a.try_node(edge_handle(e))?` and `v = node.leaf_view()?` (`PubKind::Leaf`); (3) check `if !still_valid() { return Err(Torn); }`; (4) call `#[cfg(all(test, feature = "std"))] seek_hooks::before_leaf_keys();`; (5) check `pop.checked_mul(kb as usize).ok_or(Torn)? <= v.len`; (6) read remainders via `v.rem(0, i, kb as usize)` (`word::load_rem` at `trie32.rs:1096-1122`, atomic `u32` word loads) and binary-search via `v.lower_bound(0, pop, kb as usize, target)`. |
| **S4** | `Kind::Bitmap` (`LeafBitmap1_32`, `trie32.rs:647-662, 5013-5029`) | `Kind::MapBitmap` (`LeafBitmapL32Data`, `PubKind::MapBitmap`, `seek_bitmap` + `seek_bitmap_entry` at `trie32.rs:5123-5169`) has 8 value subarrays `[PubSub; 8]` and must validate and index `sub_vals_ptr`. Set `Kind::Bitmap` (`LeafBitmap1_32`, `PubKind::Bitmap`) is a 256-bit `[u64; 4]` bitmap with **no value subarrays**. | Writer sets/clears bits in place via `word::store` (`trie32.rs:4024-4032, 4127-4138`) or demotes the bitmap leaf to `SetLeaf`/`SetImmed` below `SET_BITMAP_LEAVE` (`16`, `trie32.rs:645`). | In `seek_set_bitmap(a, e, kb, still_valid)`: (1) if `kb != 1`, return `Err(Torn)`; (2) resolve `node = a.try_node(edge_handle(e))?` and `b = node.ptr_as::<LeafBitmap1_32>(PubKind::Bitmap)?`; (3) check `if !still_valid() { return Err(Torn); }`; (4) call `#[cfg(all(test, feature = "std"))] seek_hooks::before_leaf_keys();`; (5) load `bitmap: [u64; 4]` via `word::load_u64(&raw const (*b).bitmap[w])` (`trie32.rs:1066-1074`); (6) check `if !still_valid() { return Err(Torn); }`; (7) select digit via `bitmap_first_ge_raw(&bitmap, 0)` (`First`), `bitmap_last_le_raw(&bitmap, 255)` (`Last`), `bitmap_first_ge_raw(&bitmap, after as u16 + 1)` when `after < 255` (`After`), or `bitmap_last_le_raw(&bitmap, before as i32 - 1)` when `before > 0` (`Before`). |
| **S5** | `Kind::BranchL2`, `Kind::BranchL6`, `Kind::BranchB`, `Kind::BranchU` (`seek_children`, `trie32.rs:5199-5320`) | **Shared verbatim.** `seek_children<F, R, V>` (`trie32.rs:5199-5320`) is already generic over the answer type `R` (`R = (u32, u32)` for map, `R = u32` for set). | Writer mutates branch digits/edges in place (`lin_load_racy`, `BranchB32Data` subarrays, `BranchU32` edges) or promotes/demotes the branch kind. | `seek_children` checks `kb >= 2` (else `Err(Torn)`), validates `PubKind::L2` / `L6` / `B` / `U` and `still_valid()` before reading branch words, invokes `seek_hooks::before_branch_edges()`, bounds-checks `BranchB` subarray `rank < len`, and validates `still_valid()` after loading each child `Edge32` before visiting it. |
| **S6** | Map-flavoured kinds inside a set walk (`Kind::MapImmed`, `Kind::MapLeaf`, `Kind::MapBitmap`) | Mirror of `map_seek_validated` rejecting set-flavoured kinds (`trie32.rs:5353, 5384, 5434, 5495`). | Torn edge tag transiently decoding to a map kind before `still_valid()` catches the version change. | Return `Err(Torn)` immediately on any map-flavoured `Kind` arm. |

### 38.3 Substitution twin, external baseline, and winning regimes (`AGENTS.md` §1.2, §8.3, §8.20)

Per `AGENTS.md` §1.2, §8.3, and §8.20, the optimistic `Reader32<'_, ExpanseSet32>` ordered-read construct is evaluated alongside its natural substitution twin and an external baseline, with falsifiable regimes where each wins:

1. **Construct under test (`sync32_set_ord`):** `SyncExpanseSet32` split into one `Writer32<'_, ExpanseSet32>` and $`R`$ `Reader32<'_, ExpanseSet32>` handles executing `try_first`, `try_last`, `try_next_at_or_after`, `try_next_after`, `try_prev_at_or_before`, and `try_prev_before`.
2. **Natural substitution twin (`mutex_set32_ord`):** `Mutex<ExpanseSet32>` wrapping the identical single-threaded `ExpanseSet32` (`crates/expanse/src/set32.rs`) at the same keyspace, prefill, PRNG algorithm and seed (`XorShift`), and writer pacing rate. Readers acquire the mutex and call `ExpanseSet32::first` / `last` / `next_at_or_after` / `next_after` / `prev_at_or_before` / `prev_before` directly; the writer acquires the same mutex for `insert` / `remove`.
3. **External reference baseline (`rwlock_btreeset32_ord`):** ` std::sync::RwLock<std::collections::BTreeSet<u32>>` at identical prefill, PRNG seed, and writer pacing rate, executing the equivalent six `BTreeSet<u32>` ordered queries under `read()` and `write()` locks.
4. **Explicit winning regimes (non-definitional comparison):**
   - **Regime where `SyncExpanseSet32` (`Reader32`) wins:**
     - *Quiescent and paced-writer multi-reader regimes* (0 writes/s, $`10\text{k}`$ writes/s, $`100\text{k}`$ writes/s, and $`1\text{M}`$ writes/s across $`R \in \{2, 4, 8\}`$ readers): `Reader32` touches only its own cache-line-padded `ReaderSlot::walk` word (`sync32.rs:190-194`) and reads shared tree nodes without writing to any shared cache line, whereas `mutex_set32_ord` serializes every reader behind one lock word and `rwlock_btreeset32_ord` bounces the `RwLock` reader-count cache line across cores on every query.
     - *Non-blocking interrupt-service-routine (ISR) regime:* On single-core MCUs (`riscv32imc` / Cortex-M) where an ISR preempts the main loop, a lock-based reader (`mutex_set32_ord`) cannot block when the writer holds the lock without deadlocking the system, whereas `Reader32::try_*` returns `Err(Busy)` in constant instructions.
   - **Regimes where the substitution twin (`mutex_set32_ord`) wins:**
     - *Single-threaded deterministic instruction count:* Plain `ExpanseSet32::next_at_or_after` and `prev_before` (`set32_nav` / `set32_prev`, and `Writer32<'_, ExpanseSet32>`) walk direct slice references (`Arena::leaf`, `Arena::bitmap`) without `ReaderSlot::walk` pin/unpin stores and `SeqCst` fence (`sync32.rs:437`), without `PubTable::try_node` slot lookups, and without per-level `still_valid()` version checks. Single-threaded `ExpanseSet32` therefore retires strictly fewer instructions per probe than `Reader32::try_*`.
     - *Unpaced full-duty writer saturation (`write_rate = None`):* Because `Sync32` uses a single **tree-level** `SeqVersion32` word covering the entire trie and `Reader32` makes a single attempt without spinning, a writer mutating continuously in a tight loop invalidates `version` on nearly every overlapping multi-level walk. Under `write_rate = None`, `Reader32::try_*` returns `Err(Busy)` on the majority of attempts (yielding lower completed `ok` reads/s), whereas `mutex_set32_ord` and `rwlock_btreeset32_ord` serialize the reader and complete every query on the first pass. Measuring both unpaced (`write_rate = None`) and paced ($`10\text{k}`$/s, $`100\text{k}`$/s, $`1\text{M}`$/s, and quiescent) regimes exposes this exact crossover rather than hiding it (`AGENTS.md` §1.2, §8.20).

### 38.4 Correctness, linearizability, and safety gates (Stage 2)

Every correctness gate below must pass in Stage 2 before any performance claim is evaluated, and each behavioural gate carries a required negative control (`AGENTS.md` §2.3, §8.12):

1. **G38.1 — Quiescent model-equivalence gate across all reachable `trie32` set node forms.**
   - *Test:* A deterministic fixture suite testing all six `Writer32<'_, ExpanseSet32>` methods and all six `Reader32<'_, ExpanseSet32>` `try_*` methods against `std::collections::BTreeSet<u32>` and `ExpanseSet32` across:
     - (a) empty set (`Kind::Null`);
     - (b) `Kind::SetImmed` at `kb = 3` (counts 1..=2), `kb = 2` (counts 1..=3), and `kb = 1` (counts 1..=7, spanning both `w[0]` and `w[1]` payload words);
     - (c) `Kind::SetLeaf` at `kb \in \{1, 2, 3, 4\}` across capacity-class boundaries (`cap_class`: 4, 8, 16, 24, 32), explicitly including `kb = 2` and `kb = 3` leaves whose packed keys straddle 32-bit word boundaries inside `word::load_rem` (`crates/expanse/src/trie32.rs:1096-1122`);
     - (d) `Kind::Bitmap` (`LeafBitmap1_32`) at populations 25, 64, 128, 255, and 256 (single-word, cross-word `bitmap[0..4]`, and full 256-bit leaf), plus a bitmap leaf demoted below `SET_BITMAP_LEAVE` (`16`, `trie32.rs:645`) back to `SetLeaf` and `SetImmed`;
     - (e) `Kind::BranchL2`, `Kind::BranchL6`, `Kind::BranchB`, and `Kind::BranchU` at `kb \in \{2, 3, 4\}` where `try_next_after` and `try_prev_before` must backtrack out of an exhausted child subtree (`Ok(None)`) and descend into the next or previous populated sibling digit;
     - (f) boundary probes `0`, `1`, `u32::MAX - 1`, and `u32::MAX`.
   - *Pass criterion:* Exact agreement with `BTreeSet<u32>` on every probe (`Ok(expected)` on `Reader32`, `expected` on `Writer32`).
   - *Negative control:* Flip the strict/inclusive offset in `try_next_at_or_after` (`bound` vs `bound - 1`) or drop the multi-word straddle branch in `word::load_rem` and verify G38.1 fails.
2. **G38.2 — Open write-bracket `Busy` gate (including boundary short-circuits).**
   - *Test:* With the writer's `SeqVersion32` bracket held open (odd version word), invoke all six `Reader32<'_, ExpanseSet32>` methods — including the boundary short-circuits `try_next_after(u32::MAX)` and `try_prev_before(0)` — and verify that every call returns `Err(Busy)` and restores `ReaderSlot::walk` to an even value on exit (`w.try_reclaim()` returns `true`).
   - *Negative control:* Move the `bound == u32::MAX` early return in `try_next_after` or the `bound == 0` early return in `try_prev_before` above `self.pinned(|owner| { owner.version.0.try_sample().ok_or(Busy)?; ... })` and verify G38.2 fails.
3. **G38.3 — Deterministic pause-point (`seek_hooks`) mid-walk mutation gate.**
   - *Test:* Using `trie32::seek_hooks` (`crates/expanse/src/trie32.rs:5081-5084, 5108, 5219, 5233, 5285, 5306`, extended to `seek_set_leaf` and `seek_set_bitmap`), pause a `Reader32<'_, ExpanseSet32>` ordered walk mid-flight while the writer mutates:
     - (a) a `SetLeaf` in place (inserting/removing a key that shifts remainders after `before_leaf_keys`);
     - (b) a `SetLeaf` across a promotion to `Bitmap` or demotion to `SetImmed` (retiring the leaf handle to `Arena::pending` while the reader's pin keeps the slot allocated);
     - (c) a `Bitmap` (`LeafBitmap1_32`) in place or across demotion below `SET_BITMAP_LEAVE`;
     - (d) a `BranchL2`, `BranchL6`, `BranchB`, or `BranchU` child edge at `before_branch_edges`.
   - *Pass criterion:* Every paused read returns `Err(Busy)`, and after the reader exits its pin, `w.try_reclaim()` drains `Arena::pending` to `0`.
   - *Negative control:* Enable `seek_hooks::skips_seal()` (`trie32.rs:5082`) and remove the post-load `still_valid()` check in `seek_set_leaf` / `seek_set_bitmap`; verify the paused reader returns a stale/torn key instead of `Err(Busy)` and fails the test.
4. **G38.4 — Multi-threaded linearizability stress gate (`set_ordered_reads_return_a_committed_neighbour_under_writer_churn`).**
   - *Test:* Add the set twin of G12.5 (`map_ordered_reads_return_a_committed_neighbour_under_writer_churn`, `crates/expanse/tests/sync32_stress.rs:188-358`) in `crates/expanse/tests/sync32_stress.rs`:
     - Prefill multiples of `STRIDE = 16` up to `SPAN = 4_096` into `SyncExpanseSet32::with_capacity(16_384, 3)`.
     - One writer churns inserts and removals on non-multiples of `STRIDE` for `MIN_ROUNDS = 40_000` (up to `MAX_ROUNDS = 400_000`) until `READERS = 3` concurrent reader threads record at least `MIN_OVERLAP = 1_000` overlapping `Ok` answers and `MIN_CHURN_ANSWERS = 100` churn-key answers across all six `Ord32` query variants (`First`, `Last`, `NextAtOrAfter`, `NextAfter`, `PrevAtOrBefore`, `PrevBefore`).
   - *Pass criterion:* Every `Ok(answer)` bracketed by writer commit counters `[t0, t1]` matches the exact neighbour of `q` in at least one replayed `BTreeSet<u32>` state in `t0..=t1 + 1`, and post-join `w.try_reclaim()` succeeds with `w.pending_len() == 0`.
   - *Negative control:* Inject an unvalidated answer (via `seek_hooks::skips_seal()` or by XORing `1` into a churn-key answer in the harness check) and verify the replay oracle fails.
5. **G38.5 — Miri UB-site census gate (`scripts/check_miri_coverage.py`).**
   - *Test:* Extend `sync32::tests::set_reader_writer` (`crates/expanse/src/sync32.rs:1076`) and `sync32::tests::set_bitmap_reader_writer` (`sync32.rs:1110`) to call all six `Writer32<'_, ExpanseSet32>` ordered methods and all six `Reader32<'_, ExpanseSet32>` `try_*` ordered methods across `SetImmed`, `SetLeaf` (1-, 2-, and 3-byte remainders), `Bitmap`, `BranchL2`, `BranchL6`, `BranchB`, and `BranchU`.
   - *Pass criterion:* `cargo +nightly miri test --lib sync32::tests::` and `python3 scripts/check_miri_coverage.py` pass with zero errors and 100% registered `unsafe`-block coverage.
6. **G38.6 — Zero arena and node-layout footprint growth.**
   - *Pass criterion:* `size_of::<Edge32>() == 8`, `size_of::<LeafBitmap1_32>() == 40`, `size_of::<PubSlot>() == 16`, and `SyncExpanseSet32::with_capacity(node_cap, max_readers)` memory usage change by **0 bytes**.

### 38.5 Deterministic instruction-count gates (`crates/expanse/benches/instructions.rs`)

1. **New single-threaded baseline arms (`set32_nav` and `set32_prev`, Stage 1).**
   - `crates/expanse/benches/instructions.rs` currently registers `map32_nav` (`instructions.rs:1566-1580`) and `map32_prev` (`instructions.rs:1583-1598`), plus `set32_iterate` and `set32_range` (`instructions.rs:1654-1680`), but has no point-ordered navigation arm for `ExpanseSet32`.
   - Stage 1 adds `set32_nav` (`ExpanseSet32::next_at_or_after`) and `set32_prev` (`ExpanseSet32::prev_before`) over `built_set32_remove` across `sequential`, `clustered`, and `random` distributions (`POP = 50_000` shuffled probes, matching `map32_nav` and `map32_prev`).
2. **New validated concurrent reader arms (`sync32_set_nav` and `sync32_set_prev`, Stage 2).**
   - Mirroring `sync32_map_get` (`crates/expanse/benches/instructions.rs:1882-1900`, `S32R_OPS = 4_000` probes, 50% hits / 50% misses drawn from the population generator and rejected on membership per `AGENTS.md` §8.6 across `bitmap`, `linear`, and `linear_wide` shapes), Stage 2 adds:
     - `sync32_set_nav`: `Reader32::<ExpanseSet32>::try_next_at_or_after` over `bitmap` (`8,192` keyspace), `linear` (`1 << 16` keyspace), and `linear_wide` (`1 << 24` keyspace);
     - `sync32_set_prev`: `Reader32::<ExpanseSet32>::try_prev_before` over `bitmap`, `linear`, and `linear_wide`;
     - `sync32_map_nav` and `sync32_map_prev`: the `Reader32::<ExpanseMap32>::try_next_at_or_after` and `try_prev_before` twins on the identical `built_sync32_read` shapes so set vs map validated seek costs are compared on the same probe stream.
3. **Deterministic instruction-count gate criteria (`AGENTS.md` §1.1, §8.5).**
   - **Non-regression on existing arms:** `set32_insert`, `set32_remove`, `set32_iterate`, `set32_range`, `map32_get`, `map32_nav`, `map32_prev`, `sync32_map_write`, and `sync32_map_get` must change by $`\le 0.5\%`$ `(target)` in retired instructions (`Ir`). Out-of-line `#[inline(never)]` on `seek_set_leaf` (matching `map_leaf_get_validated` at `crates/expanse/src/trie32.rs:4943`, #1274) is pre-registered if inlining moves `sync32_map_get` or `set_contains_validated`.
   - **Set vs Map validated seek bound:** On each of `bitmap`, `linear`, and `linear_wide`, `sync32_set_nav` and `sync32_set_prev` must retire $`\le 1.00\times`$ `(target)` the instructions of `sync32_map_nav` and `sync32_map_prev` (because `SetLeaf` omits the `4 * cap_class(pop)` value offset and value load, and `Bitmap` omits the `LeafBitmapL32Data` value-subarray lookup).

### 38.6 Multi-threaded concurrency and 32-bit target evaluation protocol (Stage 3)

1. **Host multi-threaded concurrency matrix (`crates/expanse/benches/concurrency.rs`).**
   - Extend the `sync32` benchmark driver (`bench_sync32_map` at `crates/expanse/benches/concurrency.rs:1004-1187`) with three symmetric ordered-read arms over the same keyspace (`S32_STABLE = 4_096` prefill draws, `S32_CHURN = 4_096` writer churn range, `S32_NODE_CAP = 16_384`), identical `XorShift` seeds, and Williams Latin-square thread-count ordering (`williams_order`, `concurrency.rs:1023`):
     - `sync32_set_ord`: `SyncExpanseSet32`, 1 writer thread (`try_insert` / `try_remove` in a 2:1 ratio on the churn range) + $`R \in \{1, 2, 4, 8\}`$ reader threads cycling the ordered queries (`try_next_at_or_after`, `try_prev_before`, `try_first`, `try_last`);
     - `mutex_set32_ord` (substitution twin): `std::sync::Mutex<ExpanseSet32>` with 1 writer thread + $`R \in \{1, 2, 4, 8\}`$ reader threads executing the same query and mutation streams under `lock()`;
     - `rwlock_btreeset32_ord` (external baseline): `std::sync::RwLock<std::collections::BTreeSet<u32>>` with 1 writer thread + $`R \in \{1, 2, 4, 8\}`$ reader threads under `read()` / `write()`.
   - **Key distributions:** `bitmap` (`8,192` keyspace), `linear` (`1 << 16` keyspace), and `linear_wide` (`1 << 24` keyspace).
   - **Writer pacing regimes:** Quiescent (`0` writes/s), deadline-paced writer at $`10\text{k}`$, $`100\text{k}`$, and $`1\text{M}`$ mutations/s (`concurrency.rs:1086-1107`), and unpaced full-duty writer (`write_rate = None`).
   - **Statistical protocol and host hygiene (`AGENTS.md` §1.1, §1.9, §8.4, §8.8):**
     - $`n \ge 30`$ rounds per cell on an isolated reference host (`load avg < cores / 2`, verified before and after the sweep per `AGENTS.md` §1.9).
     - Report macro-mean validated reader throughput (`ok` ops/s), `Busy` fraction ($`\text{busy} / (\text{ok} + \text{busy})`$ ), writer throughput (`write_ops`/s), writer refusals (`refused`), CPU/core placement (`placement()`, `concurrency.rs:969-995`), and **95% BCa bootstrap confidence intervals** ($`\ge 1,000`$ resamples) on both absolute throughput and the `sync32_set_ord / mutex_set32_ord` and `sync32_set_ord / rwlock_btreeset32_ord` throughput ratios.
   - **Host shipping gate (G38.7):**
     - In the quiescent (`0` writes/s) and paced-writer ($`10\text{k}`$/s and $`100\text{k}`$/s) regimes at $`R \ge 2`$ reader threads, the **95% BCa CI lower bound** of the validated reader throughput ratio `sync32_set_ord / mutex_set32_ord` must be $`\ge 1.25\times`$ `(target)` across `bitmap`, `linear`, and `linear_wide` (`workload: sync32_set_ord`), with `refused == 0` after reader quiescence.
     - In the unpaced full-duty writer regime (`write_rate = None`), both validated `ok` throughput and `busy` fraction are published alongside `mutex_set32_ord` and `rwlock_btreeset32_ord` to document the single-seqlock saturation regime (§38.3).
2. **On-device / 32-bit target verification gate (G38.8).**
   - **Zero atomic RMW disassembly check:** Compile `expanse-trie` for `riscv32imc-unknown-none-elf` and verify via `nm` / `objdump` that `trie32::set_seek_validated` and `Reader32::<ExpanseSet32>::try_*` emit zero atomic RMW instructions (`amo*`, `lr.w`, `sc.w`) and zero `__atomic_*` software-lock libcalls (`crates/expanse/src/sync32.rs:4-17`).
   - **On-device cycle-counter gate (`docs/design/32-bit-embedded.md` §7):** On the 32-bit target harness (hardware cycle counter `DWT->CYCCNT` on Cortex-M7 or `mcycle` on `ESP32-C3` `riscv32imc`, $`n \ge 30`$ rounds), the **95% BCa CI upper bound** of the uncontended per-probe cycle ratio `Reader32::<ExpanseSet32>::try_next_at_or_after / ExpanseSet32::next_at_or_after` and `try_prev_before / prev_before` across `bitmap` and `linear` shapes must be $`\le 1.35\times`$ `(target)` (`workload: embedded32_set_ord`).
3. **Three-stage rollout and amendment rule (`AGENTS.md` §1.6, §8.19).**
   - **Stage 1:** Land this §38 pre-registration in `docs/benchmarks/concurrency/METHODOLOGY.md` (and `set32_nav` / `set32_prev` baseline arms in `crates/expanse/benches/instructions.rs`).
   - **Stage 2:** Land `trie32::set_seek_validated`, the six `Writer32<'_, ExpanseSet32>` ordered methods, and the six `#[doc(hidden)]` `Reader32<'_, ExpanseSet32>` `try_*` ordered methods, passing G38.0–G38.6 and the Stage 2 Callgrind gates (§38.5).
   - **Stage 3:** Execute G38.7 and G38.8. Remove `#[doc(hidden)]` from the `SyncExpanseSet32` ordered-read methods iff both G38.7 and G38.8 pass against the locked thresholds above; any threshold change requires an explicit amendment subsection (`§38.7`) and a fresh-seed re-run.


---

## 39. Pre-registration for #1417 — does per-call reader registration in `update` cost throughput under contention? (appended 2026-10-17, locked before any run of the cells below; next free section, may be renumbered at merge)

`SyncExpanseMap::update`, `SyncExpanseStrMap::update` and `SyncExpanseBytesMap::update` read the current value through `self.get(key)` before their exchange loop, and `get` registers a throwaway epoch reader and drops it. This section registers the measurement #1417 asks for before any engine change: a contended wall-clock arm for `update`, several threads on one map. It holds no engine code and no data. The harness cells and the driver mode arrive in the same pull request as this section; the runs are taken on the reference host afterwards. Code references are to `origin/main` at `f19935181`. Refs #1417.

### 39.1 The question, and what the arms isolate

**The question.** With W threads updating present keys of one shared map, is `update` slower than the same read-then-exchange loop run on a reader each thread registered once, and does the difference grow with W?

**What `update` does per call** (`verified`):

- `update` sets `expected = self.get(key)`, then loops `compare_exchange(key, expected, f(expected))`, on `Err(seen)` setting `expected = seen` and calling `core::hint::spin_loop()` (`crates/expanse/src/sync.rs:6958-6970` map, `:14012-14028` str, `:15556-15568` bytes). `SyncExpanseOrderedBytesMap::update` delegates to the str map's (`sync.rs:16694-16696`) and is not measured separately.
- `get` is `self.reader().get(key)` (`sync.rs:7275`, `:14209`, `:16055`).
- `reader()` calls `Collector::register` (`crates/expanse/src/occ.rs:1975-1988`): one `Arc<Slot>` allocation, the collector's `readers` mutex, a `Vec` push. Dropping the handle (`occ.rs:2691-2701`) takes the same mutex again and runs `retain` over every registered slot. The mutex is one per collector, so one per map.

**The arms.** Both are written in the harness against the public API; no engine code changes.

| Arm key | Per operation | Per thread, outside the timed window |
|---|---|---|
| `<e>_update` | `m.update(k, f)` with `f = \|v\| v.map(\|x\| x.wrapping_add(1))` | registers one reader `rd`, which no update uses |
| `<e>_update_held` | `expected = rd.get(k)`, then the loop `update` runs, verbatim: `m.compare_exchange(k, expected, f(expected))`, on `Err(seen)` set `expected = seen` and call `spin_loop()` | registers one reader `rd` |

with `<e>` one of `map`, `str`, `bytes`. The held arm is the shape #1417's proposal 2 names ("a reader held for the call's retries"), with the reader held for the thread instead of the call. The two arms run the same read function on the same key, the same exchange and the same retry loop; per operation they differ by one `register` and one drop, and by nothing else. Both arms keep one registered reader per thread, so the registry holds W slots in both, plus the update arm's transient one.

**Twin check (C-b).** The held arm does strictly less work, so the update arm has no regime in which it should be faster; this is a cost measurement, not an architectural comparison. A run in which the held arm is materially slower is an instrument defect and is void (§39.5).

### 39.2 Instrument and cells

**Harness:** `crates/expanse/benches/concurrency.rs`, the six arm keys above. They are opt-in: an unset `EXPANSE_BENCH_ENGINES` runs the arms it ran before, so the nightly `bench-report` job is unchanged.

- **Keys.** Each window prefills the engine's usual population (`map`: 1,000,000 distinct draws over 2,000,000 keys; `str`, `bytes`: 100,000 of the 200,000 route keys) and every operation updates a key drawn uniformly from the prefill, so every update replaces a present value and no operation inserts or removes. Each worker's stream is the suite XorShift seeded per window and per worker (`Window::seed`); both arms of a cell draw the identical stream.
- **Window.** 500 ms from a barrier after every worker's setup (reader registration included) to the stop flag; the rate is updates over the window's own elapsed time.
- **Driver:** `docs/benchmarks/concurrency/scripts/mixed_concurrency.py --update-cells`, one bench process per window (`EXPANSE_BENCH_ROUNDS=1`, `EXPANSE_BENCH_WORKLOADS=0`), so every window starts from its own prefill and its own process state (§15).
- **Cells.** Engine `e` in {map, str, bytes} x W in {1, 2, 4, 8} x arm in {update, held}: 24 windows per round.
- **Pin.** `0,2,4,6,8,10,12,14`, one logical CPU per physical P-core, so W = 8 is one worker per core and no cell shares a core (AGENTS.md §8.20.5 step 0). The driver refuses any other pin outside `--quick`.
- **Order within a round.** The engines run in the order `(map, str, bytes)` rotated left by `round mod 3`; within an engine, W follows row `round` of the four-level Williams design; within an (e, W) pair the two arms run back to back, update first in even rounds and held first in odd ones. Every quantity in this order cycles in 12 rounds.
- **Suite:** `concurrency_update_cells` (`.github/bench-suites.json`); it writes `results/baseline_concurrent_update_cells.json`. A second run is a second dispatch, committed as `results/baseline_concurrent_update_cells_run2.json`.

### 39.3 Statistics, fixed now

- **Rounds:** 36 per run, three whole order cycles. Sized in `scripts/update_registration_bounds.py` (§8.8 commit 1): from the largest per-round CV of a paired, one-process-per-window ratio on this host, borrowed from the #568 Step 2 map cells (0.1015 contended, 0.0084 at one thread; `results/baseline_concurrent_step2_gate*.json`), 36 rounds give a minimum detectable departure of 0.0474 for `rho` at W ≥ 2 and 0.0476 for `kappa`, both below the margin, and about an 84% chance of reaching the falsifier when the true ratio is 1. The spread is a different workload's; the run's own intervals are what decide.
- **Per-round values.** `T(e, W, arm)` = updates per second in that round's window. `rho(e, W)` = `T(e, W, held) / T(e, W, update)`, paired within the round. `kappa(e, W)` = `rho(e, W) / rho(e, 1)` of the same round, for W in {2, 4, 8}.
- **Intervals.** BCa 95% over the 36 per-round values, 2,000 resamples, seed 42 (`scripts/bca_bootstrap.py`), recorded with `ci_method`. Each cell also carries both arms' mean, median and interval of `T`, and every window under `rounds_raw`.
- **Margin.** `MARGIN = 0.05`: a registration cost below 5% of an update's throughput at W = 8 is not worth an engine change. The driver and the bounds module carry the same constant, and the driver's self-test asserts they agree.
- **Primary cell:** W = 8, per engine. W = 2 and 4 are reported and decide nothing.

### 39.4 Verdict

Per run, per engine, at W = 8:

1. `COST` when the lower bound of `rho(e, 8)` is at least `1 + MARGIN`: registration costs at least 5% of `update`'s throughput at eight threads.
2. `NO_COST` when the upper bound of `rho(e, 8)` is below `1 + MARGIN`: the contended arm shows no cost of 5% or more attributable to registration. This is #1417's falsifier.
3. `OPEN` otherwise.

Reported beside it, deciding nothing on its own, the contention label from `kappa(e, 8)`: `GROWS` when its lower bound is above 1.0 (the cost per update is larger at eight threads than at one, the contention-specific form of AGENTS.md §8.20.2), `SHRINKS` when its upper bound is below 1.0, `FLAT` otherwise.

**The pair.** Two independent dispatches. Per engine, the pair's verdict is the run verdict when both runs give the same one, and `OPEN` otherwise (`docs/BENCHMARKING.md` rule 18). #1417's falsifier is met when all three engines read `NO_COST` in the pair.

| Pair verdict for an engine | What follows |
|---|---|
| `COST` | proposal 2 may go ahead for that engine, under its own pre-registration (§39.7) |
| `NO_COST` | no engine change for that engine on this evidence; the result is recorded here and on #1417 |
| `OPEN` | no engine change; one further pair of fresh runs may be taken, whose verdict is final |

### 39.5 What voids a run

- A pin other than `0,2,4,6,8,10,12,14`. A `--quick` run is never verdict-bearing.
- A window process that exits non-zero, or writes anything but exactly one row naming the requested arm, thread count, read percentage 0, round 0 and position 0, or a window with no updates. The run is discarded and nothing is written.
- A round whose `foreign_busy_cpus` (the per-round load snapshot, `scripts/bench_provenance.py` `begin_cell` / `end_cell`) is missing or at least `RUN_FOREIGN_VOID` (1.0 core-equivalent), and a job the workflow's host guard failed (`scripts/bench_host_guard.py`).
- An instrument defect: the upper bound of `rho(e, W)` below `1 - MARGIN` in any cell, the arm that does less work measuring materially slower (§39.1).

A void run is written with `verdict: VOID` and its reasons, and is not one of the pair. At most three replacement runs per pair.

### 39.6 Expected outcomes, and what is not predicted

Every reason is a hypothesis.

| Cell | Expected | Reason (hypothesis) |
|---|---|---|
| W = 1, all engines | `rho > 1` | the registration's allocation, two mutex acquisitions and `retain`. `update / cas` from #1411's instruction counts bounds `rho(1)` from above at 2.116 (map), 1.815 (str) and 1.719 (bytes) if time is proportional to instructions; the held arm's read sits inside that difference, so `rho(1)` lies between 1 and those bounds |
| W = 8, all engines | `kappa > 1` possible | eight threads take the one `readers` mutex twice per update, and the drop's `retain` walks at least eight slots; the exchange itself can also contend, on the tree word and on the writers' own slots, which both arms share |
| W = 8, str and bytes | smaller `rho` than map | a longer read and exchange per update dilutes a fixed registration cost |

**Not predicted:** any magnitude of `rho` or `kappa` at W ≥ 2; the W at which a cost appears; behaviour on other hosts, core counts or allocators (the registration allocates through the system allocator); `update` on absent keys, where `compare_exchange(None, None)` registers a second reader (`sync.rs:6935-6941`); the other wrappers' `update`.

**Silent constants,** all fixed here: the three engines and their prefills; uniform keys over the prefill; `f = v + 1`; W in {1, 2, 4, 8}; the pin; the 500 ms window; 36 rounds; the order of §39.2; 2,000 resamples, seed 42; `MARGIN = 0.05`; the void bound `1 - MARGIN`; `RUN_FOREIGN_VOID`; three replacement runs and one further pair. Changing any of them after a run relabels that run `INTERMEDIATE` and requires fresh runs (AGENTS.md §8.19).

### 39.7 What a later engine change must show

A pull request that changes `update` for an engine this section found `COST` (proposal 2: read without a per-call registration, through a reader held across the call's retries or a read under the writer's own pin) carries its own pre-registration, and at least:

- **Contended gain.** These cells re-run on the change, two runs: the update arm's `T(e, 8)` against the base build's, paired within the round, with the lower bound of the ratio above 1.0; and `rho(e, 8)` on the change with its upper bound below `1 + MARGIN`, i.e. `update` no slower than the held loop.
- **No instruction regression.** No arm of `instructions` or `rmw_instructions` regresses beyond the 0.1% review threshold (AGENTS.md §6); the `sync_*_update_rmw` arms are expected to fall.
- **The plain paths.** Concurrency-only code stays out of shared inlined paths (AGENTS.md §2.1 invariant 5).
- **Expiry.** The verdict here describes the engine at the commit it measured (AGENTS.md §8.20.6). A change to `Collector::register`, to the reader registry, or to `compare_exchange`'s optimistic path expires it, and these cells are re-run before an engine change is evaluated against them.

### 39.8 Open questions

None. The section number, 39, is the next free one at lock time and may move at merge.

### 39.9 Outcome (appended 2026-10-17; §39.1 to §39.8 are not rewritten)

Two dispatches of the `concurrency_update_cells` suite at `72c733c8` (the commit of #1564, which holds the cells and this section): [run 1](https://github.com/orieg/expanse/actions/runs/39078599689) and [run 2](https://github.com/orieg/expanse/actions/runs/39082333529). Both are `ADMISSIBLE` under §39.5: pin `0,2,4,6,8,10,12,14`, 36 rounds, `voids: []`, not `--quick`. Artifacts: `results/baseline_concurrent_update_cells.json` (run 1) and `results/baseline_concurrent_update_cells_run2.json` (run 2). The host-guard and perf-counter files the workflow also uploads are not committed. (measured: i9-12900F 8 P-cores pinned 0,2,..,14, Linux 6.8, commit 72c733c8, runs 39078599689 / 39082333529).

**Pair verdict (§39.4).** Per engine at W = 8, `rho(e, 8)` is the held-reader loop's throughput over `update`'s, paired within the round, BCa 95% over 36 rounds.

| engine | run 1 verdict | run 2 verdict | pair verdict | `rho(e, 8)`, run 1 | `rho(e, 8)`, run 2 | contention label, runs 1 and 2 |
|---|---|---|---|---|---|---|
| `map` | `COST` | `COST` | **`COST`** | 22.034 [21.906, 22.186] | 22.106 [22.015, 22.199] | `GROWS`, `GROWS` |
| `str` | `COST` | `COST` | **`COST`** | 10.192 [10.140, 10.256] | 10.147 [10.041, 10.214] | `GROWS`, `GROWS` |
| `bytes` | `COST` | `COST` | **`COST`** | 14.210 [14.111, 14.299] | 14.288 [14.204, 14.367] | `GROWS`, `GROWS` |

**The falsifier is not met.** #1417's falsifier needs all three engines to read `NO_COST` in the pair; none does. Every lower bound of `rho(e, 8)` is far above `1 + MARGIN` = 1.05 (the smallest is 10.041, `str` in run 2), and the two runs' intervals overlap for each engine. The §39.5 instrument-defect check does not trip: no upper bound of `rho(e, W)` is below `1 - MARGIN`, and the held arm is faster in every cell.

**`rho(e, W)` per thread count** (held / update throughput; W < 8 decide nothing, §39.3):

| engine | W | run 1 | run 2 |
|---|--:|---|---|
| map | 1 | 1.330 [1.327, 1.333] | 1.327 [1.324, 1.330] |
| map | 2 | 4.063 [4.000, 4.115] | 4.091 [4.001, 4.145] |
| map | 4 | 10.442 [10.351, 10.544] | 10.382 [10.303, 10.461] |
| map | 8 | 22.034 [21.906, 22.186] | 22.106 [22.015, 22.199] |
| str | 1 | 1.061 [1.059, 1.062] | 1.060 [1.058, 1.062] |
| str | 2 | 1.548 [1.541, 1.556] | 1.548 [1.541, 1.555] |
| str | 4 | 3.867 [3.834, 3.897] | 3.841 [3.809, 3.873] |
| str | 8 | 10.192 [10.140, 10.256] | 10.147 [10.041, 10.214] |
| bytes | 1 | 1.131 [1.129, 1.134] | 1.134 [1.131, 1.138] |
| bytes | 2 | 1.961 [1.941, 1.976] | 1.965 [1.952, 1.977] |
| bytes | 4 | 5.650 [5.594, 5.705] | 5.612 [5.559, 5.662] |
| bytes | 8 | 14.210 [14.111, 14.299] | 14.288 [14.204, 14.367] |

**`kappa(e, W)`** (`rho(e, W) / rho(e, 1)` of the same round; `GROWS` at W = 8 means the cost per update is larger at eight threads than at one, AGENTS.md §8.20.2):

| engine | W | run 1 | run 2 |
|---|--:|---|---|
| map | 2 | 3.055 [3.006, 3.092] | 3.082 [3.014, 3.121] |
| map | 4 | 7.852 [7.782, 7.928] | 7.822 [7.765, 7.885] |
| map | 8 | 16.570 [16.461, 16.686] | 16.657 [16.562, 16.742] |
| str | 2 | 1.460 [1.453, 1.467] | 1.460 [1.454, 1.468] |
| str | 4 | 3.645 [3.616, 3.674] | 3.624 [3.592, 3.657] |
| str | 8 | 9.608 [9.558, 9.675] | 9.574 [9.468, 9.645] |
| bytes | 2 | 1.733 [1.716, 1.747] | 1.732 [1.719, 1.743] |
| bytes | 4 | 4.994 [4.942, 5.045] | 4.948 [4.902, 4.991] |
| bytes | 8 | 12.560 [12.470, 12.643] | 12.596 [12.516, 12.669] |

At W = 8 the mean updates per second over the 36 windows were, update arm / held arm: `map` 2.32 M / 51.1 M (run 1) and 2.32 M / 51.2 M (run 2); `str` 2.18 M / 22.2 M and 2.18 M / 22.2 M; `bytes` 2.14 M / 30.5 M and 2.14 M / 30.5 M, each from the artifacts' `throughput` cells (the per-cell intervals are there).

**What the result says, and what it does not.**

- Registering a reader per call costs far more than the 5% margin at W = 8 on all three engines (`rho` of about 10 to 22), reproduced across two runs. At W = 1 the ratio lies between 1 and the §39.6 instruction-count bounds (2.116, 1.815, 1.719) for all three engines, as §39.6 expected. The `kappa(e, 8)` intervals lie far above 1.0, so the cost per update is contention-specific and not a fixed per-call overhead alone.
- The mechanism is not measured. §39.6 hypothesised the collector's one `readers` mutex taken twice per update and the drop's `retain`; this experiment varies only whether a reader is registered per call, and no counter on that mutex was read. The attribution stays a hypothesis until a lock or PMU instrument reads it (AGENTS.md §8.9).
- The held arm is an upper bound on what an engine change can recover, not a prediction of it: it keeps a reader registered for the thread's whole life, which proposal 2 may or may not match.
- Not measured, as §39.6 already stated: `update` on absent keys, other hosts, core counts or allocators, and the other wrappers' `update`.

**What §39.7 now requires.** Proposal 2 of #1417 may go ahead for each of `SyncExpanseMap`, `SyncExpanseStrMap` and `SyncExpanseBytesMap` under its own pre-registration, which carries §39.7's four items: the contended gain (these cells re-run on the change, two runs, the update arm's `T(e, 8)` paired against the base build with a lower bound above 1.0, and `rho(e, 8)` with its upper bound below 1.05), no `instructions` or `rmw_instructions` arm regressing beyond 0.1%, the plain paths kept free of concurrency-only code, and the expiry rule. The base build for that comparison is the pair above, and this verdict expires on any change to `Collector::register`, the reader registry, or `compare_exchange`'s optimistic path. `SyncExpanseOrderedBytesMap::update` delegates to the str map's and is covered by the str cells' change, not by a cell of its own. This outcome ships no engine change.

## 40. Pre-registration for #1417 proposal 2 — `update` reads through the thread's cached writer reader (appended 2026-10-17, locked before any run of the change build; next free section, may be renumbered at merge)

§39.9 found `COST` for all three engines in the pair. This section registers the engine change §39.7 allows and what it must show, before the change build is measured. Code references are to `origin/main` at `204ee8e0`, whose `crates/expanse` is identical to the §39.9 base at `72c733c8` (`git diff --stat 72c733c8 204ee8e0 -- crates/expanse` is empty). Refs #1417.

### 40.1 The change, and why this one

**The change.** `SyncExpanseMap::update`, `SyncExpanseStrMap::update` and `SyncExpanseBytesMap::update` take their first read of the current value through a reader the calling thread already holds, instead of through `get`. The reader is the one `Shared::with_writer_pin` pins for the thread's writes: a per-thread cache of at most 16 readers, one per collector, keyed by the collector's address and evicted least-recently-used (`crates/expanse/src/sync.rs:2570-2601`). A new `Shared::with_thread_reader` lends that reader unpinned; the read is the same validated read `MapReader::get`, `StrReader::get` and `BytesReader::get` run (it pins per attempt), behind the same collector-identity check the detached readers use (`registered`, #1194). `compare_exchange`, its optimistic write path, the retry loop and every public signature are unchanged; no new mutation entry point exists, so no `by_mode!` twin is owed (AGENTS.md §2.1 invariant 5).

**Why it is sound.**

- The cache already exists and already holds this reader: every exchange on a tree-state map takes it through `compare_exchange_observed` (`sync.rs:7039`, `:14093`, `:15667`), so on a tree the change registers nothing new and keeps nothing alive that the exchange did not already keep. On a root-leaf map the exchange takes the root path without the pin, so there the change adds the cache entry.
- A cached reader holds an `Arc` on its collector (`occ.rs` `Reader::collector`), so no other collector can take its key while the entry lives, and the identity check panics before any node is read if that ever failed.
- Pins from one reader must not overlap (`occ.rs` `Reader::pin`). The read's pins end before `compare_exchange` takes the writer pin on the same reader, and the cache's `RefCell` is borrowed for the whole read, so a nested use panics rather than overlapping. The caller's closure runs outside that borrow.

**Candidates rejected.**

- *(a) One reader per call, reused across retries.* `update` reads once per call; a retry takes the value the failed exchange observed (`sync.rs:6958-6970`). Holding a reader for the call's retries therefore removes nothing: the per-call registration stays, and §39.9 measured it at one thread too (`rho(e, 1)` 1.06 to 1.33).
- *(c) Read inside the write path.* The optimistic exchange needs `expected` before it descends. A read-modify-write in the write path would call the caller's closure under a per-node version lock, where a panic or a slow closure holds the bracket, and it is a new mutation entry point with its OCC twin (AGENTS.md §2.1 invariant 5): more machinery than (b), for no registration (b) leaves.
- *`get` through the cache.* It would remove the same cost from every one-shot `get`, but it changes `get`'s documented contract (a throwaway reader), puts a thread-local access on every one-shot read, and keeps a collector alive per thread for threads that only read. No cell measures it; it is out of scope.

**Not changed, with the reason.**

- `SyncExpanseBlobMap::update` has the same shape (`sync.rs:12670`), but no Callgrind arm covers it (`rmw_instructions.rs` builds no blob `update` arm), and AGENTS.md §6 requires one before an engine path is changed. It is left for a change that lands the arm first.
- An `update` on an absent key whose closure returns `None` calls `compare_exchange(None, None)`, which still reads through `get` (`sync.rs:6935-6941`). No arm or cell exercises it (§39.6 did not predict it); it is unchanged.
- `SyncExpanseOrderedBytesMap::update` delegates to the string map's and is covered by it.

### 40.2 Instrument

- **Cells.** §39's, unchanged: suite `concurrency_update_cells`, `mixed_concurrency.py --update-cells`, the arms, prefills, pin `0,2,4,6,8,10,12,14`, order, 36 rounds and void rules of §39.2, §39.3 and §39.5.
- **Change runs.** Two dispatches of that suite at the pull request's head commit, committed as `results/baseline_concurrent_update_cells_s40.json` (run 1) and `results/baseline_concurrent_update_cells_s40_run2.json` (run 2).
- **Base.** The §39.9 pair (`results/baseline_concurrent_update_cells.json` and `..._run2.json`, `72c733c8`), not re-run: its engine is the change's base engine (above). Change run *i* is read against base run *i*.
- **Evaluation.** `mixed_concurrency.py --update-fix RUN1,RUN2` reads the two change artifacts against the base pair and prints every quantity below with its interval and construction; it refuses a void or quick run, an artifact of another mode, and two builds at one commit (self-test: `update_fix_self_test`).
- **Sizing.** `scripts/update_registration_bounds.py` (§8.8 commit 1, its §40 table): over the base pair's W = 8 cells the worst per-window CV is 0.0120 / 0.0218 / 0.0180 (map / str / bytes), the two-sample MDE of a change-over-base ratio at 36 windows a side 0.0079 / 0.0144 / 0.0119, and the base pair's own between-run drift at most 0.4% on either arm (measured: i9-12900F 8 P-cores pinned 0,2,..,14, Linux 6.8, commit 72c733c8, runs 39078599689 / 39082333529).

**A deviation from §39.7, stated before any change-build run.** §39.7 asks for the contended gain "paired within the round". The driver has no two-build mode for these cells, so this section compares the change runs with the base runs between runs, with a two-sample interval and a drift control on the arm the change does not touch. What §39.7 pairs within the round, `update` against the held loop, is G1 below and is kept as §39.7 states it. The gain G2 has an expected size of the base's `rho` (10 to 22) against an MDE of at most 0.0144 and a measured between-run drift of at most 0.4%, so the between-run design can decide it; it cannot attribute a gain of a few percent, and this section does not try to.

### 40.3 Gates

Per engine `e` in {map, str, bytes}, per change run, at W = 8:

- **G1, match.** The §39.4 verdict of the change run's `rho(e, 8)` is `NO_COST`: the upper bound of `T_held / T_update`, paired within the round, is below `1 + MARGIN` = 1.05.
- **G2, gain.** `gain(e)` = mean `T_update` of the change run's 36 windows over the base run's, with the two-sample BCa 95% interval (`bca_bootstrap_ratio_ci_with_method`, 2,000 resamples, seed 42). `GAIN` when its lower bound is above 1.0, `NO_GAIN` when its upper bound is at most 1.0, `OPEN` otherwise.
- **Drift control.** `drift(e)` = the same ratio on the held arm, whose code the change does not touch. The engine is `VOID` for that run when its interval lies wholly outside `[1 - MARGIN, 1 + MARGIN]`.
- **Run verdict.** `PASS` when G1 is `NO_COST` and G2 is `GAIN`; `FAIL` when G1 is `COST` or G2 is `NO_GAIN`; `VOID` per the drift control or §39.5; `OPEN` otherwise.
- **Pair verdict.** The run verdict when both runs agree, `OPEN` otherwise. A `VOID` run is replaced, at most three times per pair (§39.5).

Deterministic, on the pull request:

- **G3, instructions.** `sync_map_update_rmw`, `sync_strmap_update_rmw` and `sync_bytesmap_update_rmw` (`rmw_instructions`) fall. A rise on any of them falsifies the claim that the change removes work on that engine. No other arm of `instructions` or `rmw_instructions` moves beyond the 0.1% review threshold (AGENTS.md §6); one that does is attributed from a base-versus-head `callgrind_annotate` diff, and from the disassembly where its source did not change, before the change is titled a performance change.
- **G4, plain paths.** In an x86_64 object of `expanse-trie` built at base and at head, no function outside the `Sync*` wrappers changes its count of `%fs:`-relative instructions, and the plain-tree insert and remove paths keep zero (AGENTS.md §2.1 invariant 5). The census is recorded in the pull request.

### 40.4 Falsifier, and what follows from each outcome

The falsifier is an engine whose pair reads `FAIL`: on the change build `update` is still at least 5% slower than the held loop at eight threads, or it is not faster than the base build's `update`. Either refutes "the per-call registration is what §39.9 measured, and this change removes it" for that engine.

| Pair verdict for an engine | What follows |
|---|---|
| `PASS` | the change stands for that engine |
| `FAIL` | that engine's `update` returns to reading through `get` in the same pull request; the result is recorded here and on #1417 |
| `OPEN` | one further pair of fresh change-build runs, whose verdict is final, as §39.4 |

The engines are decided separately: one engine passing and another failing ships the change for the first only. The pull request merges after the pair is read, with G3 and G4 met; it is not merged ahead of the runs.

### 40.5 Expected outcomes, and what is not predicted

Every reason is a hypothesis.

| Quantity | Expected | Reason (hypothesis) |
|---|---|---|
| G1, `rho(e, 8)` on the change | near 1, below 1.05 | the two arms then differ by one cache lookup per update (a thread-local access and a short scan), and nothing in it is shared between threads |
| G2, `gain(e)` | far above 1, and at most about the base `rho(e, 8)` | the held arm's rate is what an update without registration reaches; the base pair measured `rho(e, 8)` near 22.0 / 10.2 / 14.2 |
| drift control | inside the band | the held arm's code is unchanged; the base pair drifted at most 0.4% between its runs |
| `kappa(e, 8)` on the change | no longer far above 1 | reported, decides nothing |
| G3, the `update_rmw` arms | fall, and stay above the `_cas` arms | the registration and drop leave; a read remains |

**Expected losses.** None on the gated cells. Possible elsewhere, each named in the pull request if it appears: an instruction change on other `sync_*` arms from moving the writer cache's thread-local out of `with_writer_pin` (the lookup becomes a shared `#[inline(always)]` function); a thread cycling over more than 16 maps re-registers on each eviction, as before; a thread that only updates a root-leaf map now keeps that map's collector alive in its cache until the entry is evicted or the thread exits, as its writes on a tree already did.

**Not predicted:** any magnitude of G1 or G2 beyond the bounds above; `rho(e, W)` at W < 8; other hosts, core counts or allocators; `update` on absent keys; the blob map.

**Silent constants,** fixed here: §39's (§39.6), plus the base pair as the comparator, run *i* against run *i*, the two-sample estimator and its seed, the drift band `[1 - MARGIN, 1 + MARGIN]`, and the 0.1% instruction threshold for arms other than the three `update_rmw` arms. Changing any of them after a change-build run relabels that run `INTERMEDIATE` and requires fresh runs (AGENTS.md §8.19).

### 40.6 Expiry

As §39.7: a change to `Collector::register`, the reader registry, `compare_exchange`'s optimistic path, or the writer reader cache expires the verdict, and the cells are re-run before a later change is evaluated against it.

### 40.7 Open questions

None. The section number, 40, is the next free one at lock time and may move at merge.

### 40.8 Amendment after change-build run 1 (appended 2026-10-18; written after observing run [39139933415](https://github.com/orieg/expanse/actions/runs/39139933415); §40.1 to §40.7 are not rewritten)

**What was observed.** The first change-build dispatch of `concurrency_update_cells` at `ad3e1475` ran on a quiet host (largest per-round `foreign_busy_cpus` 0.03) with pin `0,2,4,6,8,10,12,14` and 36 rounds, and the driver wrote it `VOID` under §39.5's instrument clause, which §40.2 inherits: the upper bound of `rho(e, W)` was below `1 - MARGIN` = 0.95 in three cells. Its artifact is `results/baseline_concurrent_update_cells_s40_void1.json`. It is not one of the pair and is not counted toward it. Its numbers, disclosed here and decided on by nothing (BCa 95%, 2,000 resamples, seed 42):

| cell | `rho(e, W)` = held / update |
|---|---|
| map, W = 2 | 0.9363 [0.9332, 0.9397] |
| map, W = 8 | 0.9172 [0.9134, 0.9212] |
| bytes, W = 8 | 0.9351 [0.9313, 0.9387] |
| str, W = 8 | 0.9740 [0.9689, 0.9792] |

Against §39.9 base run 1 (`results/baseline_concurrent_update_cells.json`), mean updates/s of the `update` arm at W = 8 went from 2.32 M to 55.56 M (map, ratio 23.95 [23.85, 24.07]), 2.18 M to 22.85 M (str, 10.48 [10.42, 10.55]) and 2.14 M to 32.58 M (bytes, 15.19 [15.12, 15.26]). The held arm's change-over-base ratio at W = 1 and W = 8 lay between 0.991 and 1.002 for every engine, inside the §40.3 drift band (two-sample BCa 95%, 2,000 resamples, seed 42, computed with `bca_bootstrap_ratio_ci_with_method` over the two artifacts' windows). (measured: i9-12900F 8 P-cores pinned 0,2,..,14, commit ad3e1475, run 39139933415; base commit 72c733c8, run 39078599689)

**Why §39.5's clause does not apply to the change build.** The clause voids a run when the held arm measures materially slower than `update`. It rests on §39.1's twin check: on the base build the held arm does strictly less work than `update` (no per-call registration), so it can never lose by more than noise. On the change build that premise is gone: `update` reads through the thread's already-registered writer reader, the same one its exchange pins, while the held arm reads through a second registered reader and pins it per attempt. Neither arm does strictly less work, and `update` can legitimately be the faster one. The clause tests a property only the base build has. Replacement runs under it would void the same way, and §40 has no reading of a void run.

**What changes, for change-build runs only.**

- The §39.5 instrument clause (`rho(e, W)` upper bound below `1 - MARGIN`) is withdrawn. Change-build runs are dispatched through the suite `concurrency_update_cells_fix`, which runs `mixed_concurrency.py --update-cells --change-build`; the artifact records `change_build: true` and the withdrawn clause in its gate block, and is committed as `results/baseline_concurrent_update_cells_s40.json` (run 1 of the pair) and `..._s40_run2.json` (run 2).
- The held-arm drift control of §40.3 is the instrument check for these runs: the held arm's code is the same in both builds, so a held arm that moved between the builds' runs is what an instrument or host defect would show.
- `--update-fix` reads change-build artifacts, refuses a base-mode run that §39.5 voided, refuses a change-build artifact as the base, and labels its result `INTERMEDIATE`.

**What does not change.** G1 (the §39.4 verdict of the change run's `rho(e, 8)` is `NO_COST`, upper bound below 1.05), G2 (two-sample BCa lower bound of `T_update(change) / T_update(base)` above 1.0), G3, G4, the drift band `[1 - MARGIN, 1 + MARGIN]`, the base pair, the pin, the 36 rounds and their order, every other §39.5 void (pin, a round's foreign load, a window process that fails or writes the wrong row, `--quick`), the pair rule and the cap of three replacement runs. The §39 suite `concurrency_update_cells` and its void clause are unchanged for base-mode runs.

**Label.** This amendment was written after run 1 was observed, so under AGENTS.md §8.19 the §40 pair outcome is labelled `INTERMEDIATE`, whatever it reads. The outcome subsection becomes §40.9.

### 40.9 Outcome (appended 2026-10-18; §40.1 to §40.8 are not rewritten)

The change-build pair ran as suite `concurrency_update_cells_fix` at `462247b8` (engine code identical to `ad3e1475`): run 1 [39162778714](https://github.com/orieg/expanse/actions/runs/39162778714) and run 2 [39167296656](https://github.com/orieg/expanse/actions/runs/39167296656), both on a quiet host (load average 0.00 at dispatch), pin `0,2,4,6,8,10,12,14`, 36 rounds, gate `ADMISSIBLE` with no voids. Artifacts: `results/baseline_concurrent_update_cells_s40.json` (run 1) and `results/baseline_concurrent_update_cells_s40_run2.json` (run 2). Read with `mixed_concurrency.py --update-fix` against the §39.9 base pair (`results/baseline_concurrent_update_cells.json`, `results/baseline_concurrent_update_cells_run2.json`, both at `72c733c8`); intervals are BCa 95%.

| engine | run | G1 `rho(e, 8)` | G2 gain `T_update(change) / T_update(base)` at W = 8 | held drift | run verdict |
|---|---|---|---|---|---|
| map | 1 | 0.9203 [0.9168, 0.9233] `NO_COST` | 23.93 [23.84, 24.05] `GAIN` | 0.9999 [0.9946, 1.0048] | PASS |
| map | 2 | 0.9219 [0.9181, 0.9252] `NO_COST` | 23.94 [23.85, 24.04] `GAIN` | 0.9986 [0.9943, 1.0031] | PASS |
| str | 1 | 0.9719 [0.9673, 0.9767] `NO_COST` | 10.51 [10.45, 10.57] `GAIN` | 1.0019 [0.9971, 1.0068] | PASS |
| str | 2 | 0.9740 [0.9691, 0.9798] `NO_COST` | 10.48 [10.42, 10.52] `GAIN` | 1.0056 [0.9998, 1.0162] | PASS |
| bytes | 1 | 0.9262 [0.9197, 0.9341] `NO_COST` | 15.27 [15.13, 15.36] `GAIN` | 0.9953 [0.9856, 1.0040] | PASS |
| bytes | 2 | 0.9285 [0.9230, 0.9341] `NO_COST` | 15.25 [15.17, 15.34] `GAIN` | 0.9910 [0.9844, 0.9972] | PASS |

**Pair verdict:** `PASS` for map, str and bytes (both runs agree), labelled **`INTERMEDIATE`** under §40.8 and AGENTS.md §8.19, because the §39.5 instrument clause was withdrawn for change-build runs after run 1 was observed. Every held-drift interval lies inside `[0.95, 1.05]`. G3 was met on the engine head `ad3e1475` (CI run [39128941741](https://github.com/orieg/expanse/actions/runs/39128941741): `instructions` 270/270 No change; `rmw_instructions` `sync_map_update_rmw` −21.37%, `sync_strmap_update_rmw` −10.77%, `sync_bytesmap_update_rmw` −12.47%, every other arm No change). G4 (no `%fs:` change outside the `Sync*` wrappers) is recorded in the pull request body.

The falsifier of §40.4 is not met for any engine, so no engine's `update` reverts to reading through `get`. Not measured here: contention below W = 8 is reported in the artifacts but decides nothing; `SyncExpanseBlobMap::update` was not changed and has its own Callgrind arm since #1571.

## 41. Pre-registration for #1575 — the blob map's `update` reads through the thread's cached writer reader (appended 2026-10-18, locked before any run of the blob cells and before any engine code of the change; next free section, may be renumbered at merge)

`SyncExpanseBlobMap::update` still takes its first read through `get`, which registers a throwaway epoch reader and drops it on every call. §40 changed the three other wrappers' `update` and left this one. This section registers both halves for the blob map before either is measured: the cost measurement §39 made for the other engines, and the change §40 made, with the gates each must meet. It holds no data. The harness cells and the driver mode are the previous commit of the same pull request; the engine change is the next one. Code references are to `origin/main` at `78cc0386d`, whose `crates/expanse/src` is the base build of every base run below. Refs #1575, #1417.

### 41.1 The question, and why §39 does not carry over

**What `update` does per call** (`verified`): `update` sets `current = self.get(key)` (`crates/expanse/src/sync.rs:12756`), then loops: `f(current)`, `compare_exchange(key, cur, offered)`, on `Mismatch(seen)` setting `current = seen` and calling `spin_loop()`, stopping on `Unstorable`. `get` is `self.reader().get(key)` (`sync.rs:12905`): one `Collector::register` and one drop around the owned-copy read `blob_copy_get!` (`sync.rs:13373`), the same registration §39.1 describes. `compare_exchange` on a tree-state map runs inside `Shared::with_writer_pin` (`sync.rs:12389`), so the thread's cached writer reader for this map already exists after its first exchange.

**Why §39.9's result is not this engine's.** A blob value is a payload with metadata, read into an owned buffer: the first read copies the payload into a fresh `Vec`, the caller's closure returns another, and the exchange appends an arena record for every stored payload above the inline limit, which a compaction later reclaims. Each of those is work the map, string and bytes maps' `update` does not do, so the registration's share of a blob `update`, at one thread and under contention, is not the share §39.9 measured, and no magnitude transfers. This section measures it.

**The question.** With W threads updating present keys of one shared blob map, is `update` slower than the same read-then-exchange loop on a reader each thread registered once, by at least the §39.3 margin at W = 8 (B0 below); and if it is, does reading through the thread's cached writer reader remove that cost (G1, G2)?

### 41.2 Instrument and cells

- **Arms.** `blob_update` and `blob_update_held` (`crates/expanse/benches/concurrency.rs`, `bench_blob_update`). `blob_update` calls `m.update(k, blob_bump)`; `blob_update_held` runs `update`'s loop verbatim on `rd.get(k)`, `rd` a `BlobReader` the thread registered once, outside the window. Both register that one reader per thread.
- **Workload.** The blob arms' prefill: 200,000 distinct draws over 400,000 keys, each a 128-byte payload with 24-bit metadata. Every operation updates a key drawn uniformly from the prefill. `blob_bump` returns the present payload copied and its metadata plus one (masked to 24 bits), so every store appends an arena record. Worker 0 compacts the arena on the blob arms' trigger (`blob_compaction_due`, one append counted per update), in both arms; each window publishes its compactions, their wall time and the arena bytes. Workload id `core_concurrency_update`.
- **Driver.** `mixed_concurrency.py --update-cells --blob`: §39.2's design on the blob pair alone. One engine, so a round is W in {1, 2, 4, 8} in row `round` of the four-level Williams design, the two arms back to back, update first in even rounds and held first in odd ones; the order cycles in 4 rounds, and the 36 rounds of §39.3 are nine cycles. Pin `0,2,4,6,8,10,12,14`, 500 ms windows, one bench process per window, 0% read.
- **Base runs.** Two dispatches of suite `concurrency_update_cells_blob` at a commit of this pull request before the engine change, whose `crates/expanse/src` equals `78cc0386d`'s. Committed as `results/baseline_concurrent_update_cells_blob.json` (run 1) and `..._blob_run2.json` (run 2).
- **Change runs.** Two dispatches of suite `concurrency_update_cells_blob_fix` (`--update-cells --blob --change-build`) at the pull request's head. Committed as `results/baseline_concurrent_update_cells_blob_s41.json` (run 1) and `..._blob_s41_run2.json` (run 2).
- **Evaluation.** `mixed_concurrency.py --update-blob-fix BASE1,BASE2,CHANGE1,CHANGE2` prints every quantity below with its interval and construction and the §41.5 decision; it refuses a void or quick run, an artifact of another mode, a change-build base, a change run not dispatched as a change build, and two builds at one commit (self-test: `update_blob_fix_self_test`, one negative control per clause).
- **Statistics.** §39.3's, unchanged: per-round `rho(blob, W)` = `T_held / T_update` paired within the round, `kappa(blob, W)` = `rho(blob, W) / rho(blob, 1)`, BCa 95% over the 36 per-round values, 2,000 resamples, seed 42, recorded with `ci_method`; `MARGIN = 0.05`; primary cell W = 8. The gain and drift ratios are §40.3's two-sample BCa (`bca_bootstrap_ratio_ci_with_method`, 2,000 resamples, seed 42) of mean `T` over the 36 windows of change run *i* against base run *i*.
- **Sizing.** No blob window has been measured, so no spread of this workload exists to size from. The 36 rounds are §39.3's, sized there from another workload's spread; the runs' own intervals decide, and a pair that cannot decide reads `OPEN` (§41.5), never a verdict.

### 41.3 Gates

**B0, the cost (base pair).** Per base run, the §39.4 verdict of `rho(blob, 8)`: `COST` when its lower bound is at least `1 + MARGIN` = 1.05, `NO_COST` when its upper bound is below 1.05, `OPEN` otherwise. The pair's verdict is the run verdict when both agree, `OPEN` otherwise. The base runs keep every §39.5 void, the instrument clause included: on the base build the held arm does strictly less work than `update`.

**Change pair**, read only when B0 is `COST`. Per change run *i* against base run *i*, at W = 8, §40.3's gates unchanged:

- **G1, match.** The §39.4 verdict of the change run's `rho(blob, 8)` is `NO_COST` (upper bound below 1.05).
- **G2, gain.** `gain` = mean `T_update(change) / T_update(base)`: `GAIN` when its lower bound is above 1.0, `NO_GAIN` when its upper bound is at most 1.0, `OPEN` otherwise.
- **Drift control.** The same ratio on the held arm; the run is `VOID` when its interval lies wholly outside `[0.95, 1.05]`.
- **Run verdict.** `PASS` when G1 is `NO_COST` and G2 is `GAIN`; `FAIL` when G1 is `COST` or G2 is `NO_GAIN`; `VOID` per the drift control or §39.5; `OPEN` otherwise. **Pair verdict:** the run verdict when both agree, `OPEN` otherwise; a `VOID` run is replaced.

**Deterministic, on the pull request:**

- **G3, instructions.** `sync_blobmap_update_rmw/random` (`rmw_instructions`) falls. A rise falsifies the claim that the change removes work. Every other arm of `instructions` and `rmw_instructions` reads `No change` in `perf_report.py`; one that moves beyond 0.1% is attributed from a base-versus-head `callgrind_annotate` diff, and from the disassembly where its source did not change, before the change is titled a performance change.
- **G4, plain paths.** In an x86_64 object of `expanse-trie` built at base and at head, no function outside the `Sync*` wrappers changes its count of `%fs:`-relative instructions, and the plain-tree insert and remove paths keep zero (AGENTS.md §2.1 invariant 5). The census is recorded in the pull request.

### 41.4 The change-build instrument rule, stated before any run

§40.8 found, after observing a run, that §39.5's instrument clause (a `rho` upper bound below `1 - MARGIN` voids the run) rests on the base build's twin check and does not hold on a change build, where `update` reads through the thread's already-registered writer reader and the held arm through a second reader. The same holds here. The clause therefore applies to the base runs and is withdrawn for the change runs from the start: they are dispatched through `concurrency_update_cells_blob_fix`, whose artifact records `change_build: true`, and the held-arm drift control is their instrument check. Every other §39.5 void applies to both builds: a pin other than `0,2,4,6,8,10,12,14`, a round's `foreign_busy_cpus` missing or at least 1.0 core-equivalent, a window process that fails or writes anything but its one requested row, a job the host guard failed, `--quick`. Because this rule is fixed before any run, the outcome carries no `INTERMEDIATE` label unless a later amendment adds one.

### 41.5 Falsifier, and what follows from each outcome

| Base pair (B0) | Change pair | What follows |
|---|---|---|
| `NO_COST` | not read | Per-call registration is not material for the blob map at W = 8. The engine change leaves the pull request (`update` keeps reading through `get`) and the result is recorded here and on #1575 |
| `OPEN` | not read | No engine change on this evidence; one further base pair, whose verdict is final |
| `COST` | `PASS` | The change stands |
| `COST` | `FAIL` | `update` returns to reading through `get` in the same pull request; recorded here and on #1575 |
| `COST` | `OPEN` | One further change pair, whose verdict is final |
| `COST` | `VOID` | The void run is replaced, at most three times per pair (§39.5) |

The falsifier of the change is a `FAIL` change pair: `update` still at least 5% slower than the held loop at eight threads, or not faster than the base build's `update`. The pull request is not merged before the base pair and, where B0 is `COST`, the change pair are read, with G3 and G4 met.

### 41.6 Expected outcomes, and what is not predicted

Every reason is a hypothesis.

| Quantity | Expected | Reason (hypothesis) |
|---|---|---|
| `rho(blob, 1)` | above 1, below the map engine's `rho(map, 1)` of 1.33 (§39.9) | one registration per call, diluted by the payload copies, the closure's allocation, the arena append and the compactions |
| B0, `rho(blob, 8)` | possibly `COST` | §39.9 found the per-call registration contention-specific on three engines (`kappa(e, 8)` 9.57 to 16.66 over both runs of all three engines); the blob update's longer critical path may dilute it, and the compactions quiesce every writer in both arms |
| G1 on the change | near 1 | the arms then differ by one cache lookup per update and by which registered reader the read pins |
| G2 | above 1, at most about the base `rho(blob, 8)` | the held arm's rate is what an update without registration reaches |
| G3 | `sync_blobmap_update_rmw` falls, other arms unchanged | the registration and the drop leave a single-threaded update; every other path is unchanged in source |

**Expected losses.** None on the gated cells. Possible elsewhere, each named in the pull request if it appears: an instruction change on `sync_blobmap_*` arms whose callers inline `get` differently once `update` no longer calls it (the bytes map's `compare_exchange` did, §40, `absent_exchange_read`); the blob map is not generic over a hasher, so whether this applies is read from the disassembly, not assumed.

**Not predicted:** any magnitude of `rho`, `kappa`, the gain or the arena's share; behaviour on other hosts, core counts or allocators; `update` on absent keys, whose `compare_exchange(None, None)` reads through `get` (`sync.rs:12348`) and is unchanged; inline-sized payloads, which take no arena record.

**Silent constants,** fixed here: §39's (§39.6) and §40's (§40.5) where they apply, plus the blob prefill (200,000 over 400,000 keys, 128-byte payloads, metadata `k & 0xFF_FFFF`), `blob_bump`, the compaction trigger `BLOB_COMPACT_APPENDS` = 200,000 counted by worker 0 times the window's thread count, the one-engine order and its 4-round period, the artifact names above, the base build (`crates/expanse/src` of `78cc0386d`) and the change runs' head. Changing any of them after a run relabels that run `INTERMEDIATE` and requires fresh runs (AGENTS.md §8.19).

### 41.7 Expiry

As §39.7 and §40.6: a change to `Collector::register`, the reader registry, the writer reader cache, `SyncExpanseBlobMap::compare_exchange`'s optimistic path, the blob arena's append or compaction expires the verdicts, and the cells are re-run before a later change is evaluated against them.

### 41.8 Open questions

None. The section number, 41, is the next free one at lock time and may move at merge.

### 41.9 Outcome (appended 2026-10-19; §41.1 to §41.8 are not rewritten)

The base pair ran as suite `concurrency_update_cells_blob` at `5cbbba94` (engine code identical to `78cc0386d`): run 1 [39194202044](https://github.com/orieg/expanse/actions/runs/39194202044) and run 2 [39198876226](https://github.com/orieg/expanse/actions/runs/39198876226). The change pair ran as suite `concurrency_update_cells_blob_fix` at `a2f0cf0e`: run 1 [39215246494](https://github.com/orieg/expanse/actions/runs/39215246494) and run 2 [39221931547](https://github.com/orieg/expanse/actions/runs/39221931547). All four ran on a quiet host (load average at most 0.12 at dispatch, worst foreign activity during a run 0.09 core-equivalents), pin `0,2,4,6,8,10,12,14`, 36 rounds, gate `ADMISSIBLE` with no voids. Artifacts: `results/baseline_concurrent_update_cells_blob.json`, `results/baseline_concurrent_update_cells_blob_run2.json` (base), `results/baseline_concurrent_update_cells_blob_s41.json`, `results/baseline_concurrent_update_cells_blob_s41_run2.json` (change). Read with `mixed_concurrency.py --update-blob-fix`; intervals are BCa 95%.

| run | build | `rho(blob, 8)` | verdict | G2 gain `T_update(change) / T_update(base)` at W = 8 | held drift | run verdict |
|---|---|---|---|---|---|---|
| 1 | base | 3.7232 [3.6945, 3.7614] | `COST` | — | — | — |
| 2 | base | 3.7424 [3.7084, 3.7826] | `COST` | — | — | — |
| 1 | change | 0.9782 [0.9694, 0.9852] | `NO_COST` (G1) | 3.7684 [3.7421, 3.8056] `GAIN` | 0.9898 [0.9799, 0.9983] | PASS |
| 2 | change | 0.9886 [0.9807, 0.9950] | `NO_COST` (G1) | 3.7248 [3.6991, 3.7568] `GAIN` | 0.9838 [0.9718, 0.9936] | PASS |

**Verdicts.** B0 is `COST` (both base runs agree). The change pair is `PASS` (both runs agree); §41.5's decision is `CHANGE_STANDS`. Both held-drift intervals lie inside `[0.95, 1.05]`. No threshold, method or sample size changed after any run, and §41.4's change-build rule was fixed before the first run, so the outcome carries no `INTERMEDIATE` label.

**G3** was met on the engine head `a2f0cf0e` (CI run [39187395954](https://github.com/orieg/expanse/actions/runs/39187395954), head-versus-`main` passes): `instructions` 270/270 and `search_instructions` 50/50 No change; `rmw_instructions` `sync_blobmap_update_rmw/random` 112,445,082 → 99,525,924 (−11.49%), and every other arm reads No change in `perf_report.py`. One of those, `sync_bytesmap_cas_remove/routes`, moved by +34,946 (+0.0177%), below the 0.1% review threshold; it is disclosed in the pull request body with its unmeasured attribution. **G4** (no `%fs:` change outside the `Sync*` wrappers, plain-tree insert and remove paths at zero) is recorded in the pull request body.

Not measured here: contention below W = 8 is reported in the artifacts but decides nothing.

## 42. Pre-registration for #1481 item 1 — a bounded free of a compaction's retired chunk set (appended 2026-10-18, locked before any engine code of the change below; next free section after #1575's §41, may be renumbered at merge)

README §29.1 found that on `SyncExpanseBlobMap` the free of a compaction's old chunk set lands on a later plain insert: 29.2–32.7 ms at 200,000 live, the largest insert of the window, above the two compactions. `crates/expanse/tests/test_blob_chunk_free_attribution.rs` (#1578) attributes it deterministically: the whole set (512–513 chunks and the chunk table) is taken and freed in one `Collector::try_advance` call, reached from a plain insert's advance countdown within 2 × `ADVANCE_EVERY` operations. Code references are to `origin/main` at `e76bb2ef`. Refs #1481.

### 42.1 The change

`Collector::try_advance` (`crates/expanse/src/occ.rs:2096`) frees the blocks no size class serves (`free_raw`, `occ.rs:2557`: arena chunks, chunk tables) up to a byte budget per call, `UNCLASSED_FREE_BUDGET`, and always at least one block per call. Classed blocks still go to their freelists as today; the budget does not apply to them.

- A call frees the next unclassed block when it has freed nothing yet, or when the block fits what remains of the budget. The first unclassed block that does not fit, and every unclassed block after it in the bin it took, move to a **carry list**: blocks already past their grace period, owned by the collector.
- The carry list is touched only by the holder of the existing `advancing` try-lock (`occ.rs:2099`). No lock is added, and no thread-local buffer exists (AGENTS.md §2.6 item 1): there is one list per collector.
- A later call frees from the carry list first, before it checks readers, since a carried block is past its grace period whatever the readers do. It then advances as today, if it can, with what remains of the budget.
- `drain` (drop) takes the same try-lock and frees whatever the carry list still holds.
- `retained_bytes()` counts a carried block until it is freed, so the figure falls at the actual free, and `CollectorCensus::unclassed_grace` counts carried blocks too, so `grace_bytes() == retained_bytes()` keeps holding on a quiesced collector. Both are read through two counters the advancer writes (blocks and bytes carried).
- Under `collector-census`, `UnclassedCounters` gains `freed_blocks` and `freed_bytes`, counted at the free site. `released_*` keep their meaning, blocks taken from a bin by an advance, which a bounded free makes different from freed.
- Under `collector-census` only, the budget is a per-collector value a test can lower (a hidden setter on `SyncExpanseBlobMap`), so a deterministic test can drive the carry on a 4 KiB-chunk arena. A default build reads the constant.

**Allocation behaviour.** The tail of the bin's list is compacted in place (`Vec::retain`) and moved into the carry list with one `Vec::append`, a bulk move of the remaining elements, not a push per element. The carry list's capacity is kept, never shrunk. `try_advance` therefore allocates only when a carry is larger than every earlier one on that collector: the **first carry does allocate** (the list starts empty), as does a later, larger one. In the steady state of a workload whose compactions retire sets of one size, it does not allocate. The bin's own list is taken and dropped as today.

**Not changed.** The epoch protocol, the grace period, the reader registry, the classed freelists, `retire`, `ADVANCE_EVERY` and `tick_advance`. No mutation entry point is added (AGENTS.md §2.1 invariant 5).

### 42.2 The budget, and its rationale

`UNCLASSED_FREE_BUDGET` = **64 MiB** (67,108,864 B): 32 chunks of the default 2 MiB.

- **Bounds** (`scripts/blob_reclaim_bounds.py`, §8.8 commit 1). The worst measured per-chunk free in README §29.1's Step 0 log is 59.53 µs (`step0_max_dispose_ns_per_chunk`: 30,481,160 ns over 512 chunks, a first compaction's dispose phase on `ExpanseBlobMap`, where each chunk is an `munmap`). At that cost a 64 MiB call frees 32 chunks in **1.905 ms — a projection, not a measurement** (`projected_free_ns_per_call`). The default set at the cap, 512 chunks and the table, drains in 17 calls (`bounded_free_calls`, matched by `bounded_free_calls_bound`), within 608 single-writer operations with no reader pinned (`drain_ops_bound`: (17 + 2) × 32).
- **Why 64 MiB and not smaller.** G2 below needs the projected per-call free well under 0.25 × the base stall (29.2 ms × 0.25 = 7.3 ms at §29.1's lower figure); 1.905 ms leaves 3.8× margin for a free cost above Step 0's worst. A smaller budget lengthens the drain, which holds freed-in-principle memory longer (G3, G4) and adds calls, for a stall that is already projected far below the gate.
- **Why not larger.** At 128 MiB the projection is 3.81 ms, half the margin; the drain shortens from 17 calls to 9, which no gate rewards.

### 42.3 Gates

Deterministic, on the pull request:

- **G1, per-call free and drain.** A `collector-census` test (`crates/expanse/tests/test_blob_chunk_free_attribution.rs`, run by `scripts/test_collector_census.sh`) on a 4 KiB-chunk arena of more than 512 chunks, budget lowered to 16 chunks (64 KiB), one writer, no reader:
  - (a) on every operation after the compaction, `freed_bytes` grows by at most the budget (every block of the set is at most the budget in this geometry, which the test asserts);
  - (b) the whole set is freed within `drain_ops_bound(bounded_free_calls_bound(set_bytes, set_blocks, max_block, budget))` operations of the compaction, the bound computed in the test by the same formula as the Python;
  - (c) at least `ceil(set_bytes / budget)` operations free part of the set, so the carry is exercised and the set is not freed on one operation;
  - each clause shown to fail on its own (AGENTS.md §5, one negative control per clause), and the existing attribution tests re-read against `freed_*` at the default budget, where the 4 KiB set (about 2 MiB) still frees in one call.
- **G4, retained bytes.** In the same test, `collector_census().unclassed_grace.bytes` is above its pre-compaction value while any of the set is carried and returns to its pre-compaction value within G1's operation bound. Its own negative control.
- **G5, instructions.** `instructions` arms, PR versus `main` (the `|main_base`, `|rmw_main_base` and `|search_main_base` blocks): **No change at the 0.1% review threshold (AGENTS.md §6; `perf_report.py`'s `NOISE_PCT`) on every arm except `sync_blobmap_reclaim_free`**, which may rise by **at most +0.1%**. Every arm whose count moves at all, below the threshold included, is listed in the pull request with its change. At the 64 MiB default that arm's 2 MiB set frees in one call, so it never reaches the carry path; G5 there measures only the budget bookkeeping on the free loop. The multi-call drain is measured by G1 and G4, not by Callgrind. A rise above 0.1% on that arm, or any change on another arm, falsifies "the bookkeeping is free on the paths that do not carry" and is attributed from a base-versus-head `callgrind_annotate` diff before anything else (AGENTS.md §6).
- **Plain paths (§2.1 invariant 5).** `try_advance`'s x86_64 disassembly gains no `%fs:`-relative instruction, and no plain-tree insert or remove path changes its count of them (zero). Recorded in the pull request.

Wall-clock, on the reference host — **pending the maintainer's go-ahead; not run by this pull request**:

- **G2, stall.** Suite `reclaim_stall`, `SyncExpanseBlobMap`, W = 1, R = 2, 200,000 live, 5 rounds per build, base (`main` at the pull request's merge base) and head in one dispatch. `H` and `B` are the head's and the base's largest insert latency among inserts that ran no compaction, median over rounds with the BCa 95% interval over rounds (§29.2's estimator). **`PASS` when the upper bound of `H` ≤ 0.25 × the lower bound of `B`; `FAIL` otherwise.** The 3,600,000-live cell is run and reported with its intervals and decides nothing. *Instrument requirement:* `reclaim_stall` records the 8 largest latencies of all inserts, not of inserts that ran no compaction. Before any G2 run it gains, in its own commit, the same top-8 over the inserts whose arena generation did not change across the call, the generation read outside the timed call, at W = 1 only; that change is not a gate.
- **G3, peak heap.** Same cells: `heap_peak`, median over rounds. **`PASS` when the upper bound of the head's ≤ 1.02 × the lower bound of the base's.**

### 42.4 Falsifiers, and what follows

| Outcome | What follows |
|---|---|
| G1 or G4 fails | the rule does not do what §42.1 states; the change does not merge |
| G5 fails | attributed first; a cost that cannot be removed is disclosed and the change waits for the maintainer's decision, not for an override |
| G2 `FAIL` | the budgeted free does not remove the stall it targets at 200,000 live; recorded here and on #1481, and the budget is not tuned against the measured value (AGENTS.md §8.19) |
| G3 `FAIL` | the carry holds memory past the compaction peak; recorded, and the change is reverted unless the maintainer decides otherwise |

### 42.5 Expected outcomes, and what is not predicted

Every reason is a hypothesis.

| Quantity | Expected | Reason |
|---|---|---|
| G1 calls on the 4 KiB set at a 64 KiB budget | 33 calls (`bounded_free_calls`, with an 8-byte-over-chunk table as an illustrative size), within the bound's 35 | the greedy rule frees 16 chunks a call |
| G5 `sync_blobmap_reclaim_free` | a rise of a few instructions per advance, far under 0.1% | one compare per unclassed block and a carry-length check per call |
| G2 head `H` at 200,000 | near the projection, about 2 ms | 32 chunks a call at Step 0's per-chunk cost |
| G3 | inside the band | a compaction's peak precedes the free, and the drain ends within 608 operations of millions per cycle |

**Expected losses.** None on the gated quantities. Possible elsewhere: with readers pinned, the carry still drains (carried blocks are past their grace period), but a bin taken while the carry is non-empty carries more; a workload that compacts more often than the drain completes would grow the carry, and none in the suites does.

**Not predicted:** multi-writer drain order; any wall-clock magnitude beyond §42.2's projection; other allocators or chunk sizes; the free's cost on other hosts.

**Silent constants,** fixed here: the 64 MiB budget, the at-least-one-block rule, carry-first order, the census test's 64 KiB budget and 4 KiB chunks, G2's 0.25 factor, G3's 1.02 factor, G5's 0.1% bound on the one arm, §29.2's harness parameters and 5 rounds. Changing any of them after a measurement relabels that result `INTERMEDIATE` (AGENTS.md §8.19).

### 42.6 Expiry

A change to `try_advance`, `retire`, the bins, `ADVANCE_EVERY` or the arena's chunk retirement expires G1, G4 and G5, which are re-run.

### 42.7 G5 on the first engine head, and the re-measured head (appended 2026-10-19, after both runs were observed; §42.1 to §42.6 are not rewritten)

**First head (`759e309e`): G5 `FAIL`.** Run [39210615318](https://github.com/orieg/expanse/actions/runs/39210615318), PR-vs-`main` blocks: 60 arms moved, 15 of them by more than 0.5% (13 in the `|main_base` block, 2 in `|rmw_main_base`); the worst were `sync_blobmap_reclaim_free/random` +2.01% (281,740 against 276,201 instructions) and `sync_set_churn/leaf` +1.42%.

**Attribution** (§42.4's first step; AGENTS.md §6). A probe of 50,000 `SyncExpanseMap` inserts and removes, run under Callgrind (valgrind 3.24, an aarch64 Linux container, not the CI host) at base `e76bb2ef` and at `759e309e`, read with `callgrind_annotate --inclusive=no` and `--auto=yes`:

- The probe gained 1,203,860 instructions over base, 1.02%, as CI's `sync_map_remove` did. Of these, 1,197,450 were `Collector::try_advance`'s own instructions; no other function moved by more than 6,230.
- Per line, the cost was in the classed-block loop, which every advance runs. The index loop that replaced `for g in stale` cost 178,879 instructions against 55,473, `class_for` 216,462 against 105,516, and the freelist link 251,715 against 100,686.
- That the loop lost the bin's pointer and length from registers once the vector could be passed `&mut` to the carry helper is read from the line counts and the x86_64 assembly (381 to 466 instructions in `try_advance`); it is not a counter measurement.

**Change, with the thresholds unchanged** (§8.19). Commit `df142e08` reads the stripe's retained bytes after the bin is taken. Every block in the bin was counted there under the bin's lock before its push, so the figure bounds the bin. When the call's frees plus that bound fit the budget, the bin is freed by the base loop unchanged; otherwise the budgeted loop runs out of line. G1 and G4 are unchanged in outcome: 514 blocks over 33 operations, drained at operation 1,064 against a bound of 1,344.

**Re-measured head (`df142e08`, rebased onto `83dc1fb6`): G5 `PASS`.** Run [39229197570](https://github.com/orieg/expanse/actions/runs/39229197570), PR-vs-`main` blocks (`|main_base`, `|rmw_main_base`, `|search_main_base`):

- 331 arms, of which 271 are unchanged.
- 60 moved, every one below 0.1%. The largest are `sync_set_churn/random` +0.0997%, `sync_set_churn/leaf` +0.0961% and `sync_blobmap_insert_reclaiming/random` +0.0910%.
- `sync_blobmap_reclaim_free/random` moved +0.0152% (276,243 against 276,201), inside its +0.1% bound.
- The margin on `sync_set_churn/random` is 0.0003 percentage points. It is a pass at the registered threshold and nothing more.

**The sub-claim G5 also stated is falsified.** §42.3 states that any change on an arm other than `sync_blobmap_reclaim_free` falsifies "the bookkeeping is free on the paths that do not carry". Sixty arms moved, so that claim does not hold: the re-measured head adds a cost below the review threshold on every `sync_*` path that advances. The gate passes at its registered threshold. The residual cost is not attributed by its own profile; the retained-bytes read and the budget comparison after each bin is taken are the only code `df142e08` adds to the path that does not carry, and the probe above measured +40,518 instructions over base on it (+0.034%, aarch64 container, not the CI host).

### 42.8 How G2 and G3 are read, fixed before their run (appended 2026-10-19, before any G2 or G3 dispatch; §42.1 to §42.7 are not rewritten)

The instrument §42.3 requires is commit `661e25c5`: `reclaim_stall --noncompact-top 1` records the 8 largest latencies of inserts across which the arena generation did not change, the generation read under `with_locked` outside the timed call, at W = 1 only; `reclaim_stall.py --free-gate --base-ref REF` builds base and head with the same harness file, one process per build, cell and round, 5 rounds, order rotated per round, one seed per cell and round for both builds, suite `reclaim_stall_free_gate`. Two readings §42.3 leaves implicit are fixed here, before any data:

- **G3 decides at 200,000 live only.** §42.3 gives G3 "the same cells" as G2, and G2 decides at 200,000 live and reports 3,600,000; G3 is read the same way. Its 3,600,000-live figure is reported and decides nothing.
- **A deciding cell can be `VOID`.** A 200,000-live cell in which any round of either build did not run exactly two compactions, or refused an insert, is `VOID`, reusing §29's window check, and is not read as `PASS` or `FAIL`. A `VOID` cell is re-run once at the same commits; a second `VOID` is reported as the outcome and decides nothing. This rule was not stated in §42.3 and is added here, before any run, so it carries no `INTERMEDIATE` label.

### 42.9 G2 and G3 outcome (appended 2026-10-19, after the run was observed; §42.1 to §42.8 are not rewritten)

One dispatch of suite `reclaim_stall_free_gate`, run [39284534803](https://github.com/orieg/expanse/actions/runs/39284534803): base `83dc1fb6` (the merge base on `main`) and head `b0642d03`, the same harness file in both, 5 rounds per build and cell, on a quiet host (load average 0.21 at dispatch, worst foreign activity during the run 0.14 core-equivalents). No round of either build at either cell lacked its two compactions or refused an insert, so no cell is `VOID`. Artifact: `results/gate_1481_reclaim_free.json`. Medians over rounds with their 95% interval.

| gate | live | head | base | bound | verdict |
|---|---:|---|---|---:|---|
| G2, largest non-compaction insert | 200,000 | 2,053,489 ns [2,004,873, 2,119,215] (BCa) | 28,946,338 ns [26,259,406, 29,111,323] (BCa) | 6,564,852 ns | **PASS** |
| G2 | 3,600,000 | 2,067,835 ns [2,054,479, 2,110,466] (BCa) | 28,943,834 ns [27,624,987, 29,066,561] (BCa) | — | reported |
| G3, `heap_peak` | 200,000 | 1,108,986,802 B (degenerate) | 1,108,975,170 B (degenerate) | 1,131,154,673 B | **PASS** |
| G3 | 3,600,000 | 1,683,245,043 B (degenerate) | 1,683,233,411 B (degenerate) | — | reported |

The `heap_peak` interval is degenerate because every round of a build reached the same peak; the head's is 11,632 B above the base's at both cells. With G1, G4 and G5 met (§42.7), every gate of §42.3 is met; G5's free-bookkeeping sub-claim stays falsified as §42.7 records. The head's largest non-compaction insert, about 2.05 ms, sits near §42.2's projected 1.905 ms per budgeted call; the projection was not a gate.

## 44. Pre-registration for #1586 — how long a string-map memory reading stalls writers, on current `main` (appended 2026-10-20, locked before any run; next free section)

The number is §44, not §43: §43 is cited by the closed #1588's record (its branch's §43.8 and §43.9), and reusing the number on `main` would make those citations ambiguous.

This is a measurement spike. It ships no engine change, and its outcome decides whether #1586 gets an engine change at all (candidates A, B, C in the issue). It holds no data. Code references are to `origin/main` at `9a27dd3e`.

### 44.1 The question

When a thread calls `SyncExpanseStrMap::mem_used()` periodically while W threads insert, how long do the inserts wait, and what share of writer time is lost? It is measured at the populations the issue names (2·10⁶ and 2·10⁷ keys) on the reference host. The issue's 13–16 ms is the call's own duration on a laptop, and §44 replaces it.

Mechanism (`verified`): `SyncExpanseStrMap::mem_used` (`sync.rs:14370`) calls `Shared::read_locked` (`sync.rs:3070`), which takes the fallback mutex, then `quiesce_writers()`, then the writer mutex, and walks `shell_bytes()` (`strmap.rs:1320`). New inserts spin in `enter_writer_blocking`; an optimistic attempt that sees the gate closed falls back to the writer mutex.

### 44.2 Commit 1: bounds (AGENTS.md §8.8)

`scripts/strmap_stall_bounds.py`, with tests pinning reference values:
- `stalled_fraction(walk_s, period_s)` = `walk_s / period_s`: the share of wall time during which no insert completes. Every writer waits, so W does not divide it. It reaches 1 when the walk outlasts the period, since the one poller's readings then run back to back.
- `spin_core_seconds(walk_s, writers)` = `walk_s × writers`: CPU burned spinning per reading.
- `walk_projection(ir_per_key, keys, ir_per_s)`: instructions per key from the #1587 arm (`routes` 3,884,760 / 50,000 = 77.70; `short` 4,172,139 / 50,000 = 83.44) times keys, over an instruction rate. It is used only to size the run (§44.4). It is labelled a projection and is never compared with a result as a prediction, because the walk misses cache once the tree outgrows LLC and Callgrind cannot see that.

### 44.3 Instrument

- **Harness.** A new bench binary `crates/expanse/benches/strmap_poll_stall.rs`, modelled on `reclaim_stall.rs`. It records an HdrHistogram (3 significant figures) of insert latencies (`Instant::now()` around each insert, uncalibrated), the 8 largest insert latencies, each poll's own duration and the returned value, inserts completed, and wall time.
  - The prefill is outside every measured window.
  - One process per (cell, round), 5 rounds, order rotated per round.
- **Population.** A `SyncExpanseStrMap` prefilled to N keys, then W writers inserting fresh keys from the same generator, rejected on membership (§8.6), for a fixed duration D.
  - Shapes: `routes` (the `str_keys` 37-byte generator) and `short` (8–16 random bytes), the two shapes the #1587 arms build.
  - `insertion_order`: `generator`.
- **Arms per cell, interleaved within each round:**
  - `none`: no poller.
  - `len`: a poller calling `len()` every T. This is the control: a lock-free reading at the same cadence, so any difference from `none` is the poller's own presence (thread, wakeups, cache), not the walk.
  - `mem_used`: a poller calling `mem_used()` every T.
- **Grid.**
  - N ∈ {2·10⁶, 2·10⁷}.
  - W ∈ {1, 4, 8}.
  - T ∈ {1 s, 100 ms}.
  - D = 30 s.
  - Shapes `routes` and `short`.
  - Pin: the `bench_pin.sh` P-core pin, one thread per physical core (§8.20.5.0), with the poller on its own core. W = 8 plus the poller exceeds 8 P-cores, so W = 8 uses the SMT siblings for writers. This is stated in the artifact.
- **Counters.** A second, untimed `occ-stats` pass of each `mem_used` cell records `GateBlockedEntries`, `GateWaitCycles` and `QuiesceDrainCycles` (`occ_stats.rs:168–174`), converted with `cycles_hz()` (§8.20.1). Timed figures never come from the `occ-stats` build.
- **Host.** Reference host only, one dispatch, §8.17 load snapshots before, between and during cells, discarded and disclosed if contaminated. Built from the commit at lock. The harness, driver and bench suite (`strmap_poll_stall`, wired in `.github/bench-suites.json` and `bench_baremetal.yml`) land in commit 3 and are run once with `--quick` to scratch first.

### 44.4 What is reported, and the decision rule

**Reported per cell**, with BCa 95% intervals over rounds (≥ 1,000 resamples; `ci_method` recorded):
- the median poll duration;
- insert p50, p99, p999 and max;
- insert throughput;
- `stalled_fraction` computed from the measured poll duration;
- the `len`-control deltas.

Everything is reported, and nothing is folded into an aggregate.

**Stall exists (S1, a mechanism check):** in the `mem_used` arm, the median over rounds of the largest insert latency is ≥ 0.5 × the median poll duration, at N = 2·10⁶, W = 4, T = 1 s, both shapes. A `FAIL` means a reading does not hold writers for the walk on current `main`, and #1586's premise is wrong.

**Material (M1), the gate that decides whether #1586 gets an engine change:** at N = 2·10⁷, W = 4, T = 1 s:
- the lower bound of `stalled_fraction` ≥ 0.01 (1% of wall time with no insert completing), **or**
- the lower bound of the `mem_used` arm's insert p999 ≥ 10× the upper bound of the `len` arm's insert p999.

The outcomes:
- `MATERIAL`: either clause holds.
- `NOT_MATERIAL`: both upper bounds lie below their thresholds.
- `INTERMEDIATE`: otherwise.

| Outcome | What follows |
|---|---|
| S1 `FAIL` | #1586 is re-examined; no engine change |
| M1 `NOT_MATERIAL` | #1586 closes on candidate A (documented caller-side estimator); no engine change |
| M1 `INTERMEDIATE` | reported to the maintainer; no engine change without a new decision |
| M1 `MATERIAL` | an engine design is pre-registered in a new section, with this stall as its named gain axis next to its instruction bounds (AGENTS.md §6 trade-off clause) |

Every other cell is reported and decides nothing.

### 44.5 Expected outcomes, and what is not predicted

- **Expected (hypothesis):** S1 passes, since the gate closes for the whole walk, and the poll duration grows faster than linearly from 2·10⁶ to 2·10⁷ on `short`, through cache and TLB misses on the suffix headers.
- **Not predicted:**
  - the poll duration's magnitude;
  - whether W changes the poll duration through spinner interference on SMT siblings;
  - the `len` control's own effect.

### 44.6 Silent constants, fixed here

N, W, T, D, the shapes, 5 rounds, the pin, the 0.5 factor in S1, M1's thresholds (0.01 of wall time; 10×), and the BCa resample count. Changing any of them after a run relabels that result `INTERMEDIATE` (AGENTS.md §8.19).

### 44.7 Expiry

The result describes the walk on `9a27dd3e`. An engine change to `shell_bytes`, `read_locked`, `quiesce_writers` or `enter_writer_blocking` voids it.

### 44.8 Clarifications fixed before the first run (appended 2026-10-20)

These settle points §44.1–§44.7 leave open. They precede every run, so no result exists that they could have been fitted to. They carry no `INTERMEDIATE` label, and §44.1–§44.7 are not edited.

- **The process unit is (cell, arm, round).** A cell is (shape, N, W, T). Each of its three arms runs in a process of its own, which prefills its own map, so no arm starts from keys another arm's writers inserted. Each round runs every (cell, arm) once. The cell order is rotated by a fifth of the list per round, and the arm order within a cell by one place per round. The three arms of a cell share one seed per round. This is what §44.3's "one process per (cell, round)" means for a cell with three arms.
- **S1 needs both shapes.** It is `PASS` iff the rule holds on `routes` and on `short`, and `FAIL` otherwise.
- **M1 is read per shape, then aggregated.** §44.4 names no shape for M1. Each shape gets its own `MATERIAL` / `NOT_MATERIAL` / `INTERMEDIATE` by §44.4's clauses. The run is `MATERIAL` if either shape is, `NOT_MATERIAL` if both are, and `INTERMEDIATE` otherwise.
- **`VOID`.** A verdict is `VOID` when any of the cells it reads meets one of these conditions, quoted from `docs/benchmarks/concurrency/scripts/strmap_poll_stall.py`:
  - `rounds [...] are not 0..4`;
  - `no insert completed`, in any round;
  - `no poll completed`, in a `len` or `mem_used` round;
  - `threads were not placed one per core (§44.3's pin)`;
  - `foreign busy CPU <x> above 1.0 (§8.17)`;
  - `no numeric load.foreign_busy_cpus; the window cannot be shown quiet (§8.17)`;
  - `load average <x> above <cpus/2> at the window's start (§8.17)`.

  So does a run whose `round count <n> is not the registered 5`, an M1 shape whose `M1's cells lack the rounds or polls an interval needs`, and any `--quick` run (`quick run: a reduced grid, not §44's registered cells`).
- **Population growth is recorded, not controlled.** N is the population at the window's start. The writers keep inserting for D, so the map grows during the window, and at N = 2·10⁶ with W = 8 the inserts may exceed N itself. Each reading then walks a larger tree than the one before it. Every row records `len_before`, `len_after`, `mem_used_after` and each reading's returned value, so the growth is visible per window. No verdict corrects for it.
- **`stalled_fraction` is capped at 1.** It is `walk_s / max(walk_s, period_s)`. A walk longer than T leaves the next reading due on its return, so the readings run back to back and the gate is closed for the whole window. This changes nothing below the cap, and so nothing about M1's 0.01 threshold.
- **The hourly pause.** The reference host runs an rsync at minutes :15–:20 UTC every hour. Before each process, timed or counted, the driver estimates the process's length:
  - the largest prefill seen so far for its (shape, N), times 1.5, or 5·10⁻⁶ s per key before the first prefill of that pair;
  - plus D;
  - plus 30 s.

  If the process could reach :15, or would start inside the blackout, the driver waits until :20 and checks again. No prefill or measured window therefore overlaps the blackout. Every pause is recorded under `provenance.hour_guard.pauses`. The job timeout for this suite alone is 640 minutes: 1.2 × the 31,934 s bound computed by `run_seconds_bound` in `scripts/strmap_stall_bounds.py`, which counts these pauses.

### 44.9 The first reference-host run was discarded, and the guard change made before the next (appended 2026-10-21)

This records an outcome and a tooling change. §44.1–§44.8 are not edited, and no threshold, cell, constant or decision rule of §44 changes.

- **Run [39532347063](https://github.com/orieg/expanse/actions/runs/39532347063), at `b4846424`, was discarded by the workflow's host-activity guard** (`scripts/bench_host_guard.py summarize`), which reported 20 samples over the void boundary and failed the job with `host_contaminated`. The run log prints the first five of them, in three episodes:
  - 04:17:34Z and 05:17:35Z, about 0.33 core-equivalents of foreign load on the pinned CPUs, 0.30 of it from children that foreign tasks reaped; the guard names no single process at or above 1% of a CPU. The times match the reference host's hourly rsync, which runs while the driver pauses (§44.8) and starts no measured process.
  - 06:59:22Z to 06:59:26Z, up to 0.98 core-equivalents, with `unattended-upgr` (the `apt-daily-upgrade` unit) named as the offender. It lies outside the :15–:20 blackout the driver pauses for.

  The other fifteen samples are in the run's uploaded `host-activity.jsonl` and are not restated here.
- **None of the run's figures is quoted, and it decides nothing** (AGENTS.md §8.17: a contaminated run is discarded, not reinterpreted). S1 and M1 remain unevaluated. No figure from this run appears in this document, in an artifact, or in the change below.
- **The guard change, made before any further run.** The guard judged every sample, including those taken while the driver deliberately ran nothing. It now reads the driver's own record of when it paused:
  - the driver records each pause with its `start_t` and `end_t`, and each measured process with its own interval, under `provenance.hour_guard` in the artifact (`docs/benchmarks/concurrency/scripts/strmap_poll_stall.py`, `guard_pause` and `run_recorded`);
  - a pause is taken only between processes, each of which runs through a blocking `subprocess.run`, and the driver refuses to write an artifact in which a pause overlaps a process (`check_pauses`);
  - `bench_host_guard.py summarize --pauses <artifact>` reports a sample whose whole window, widened by one second on each side, lies inside a recorded pause, labelled as taken during a pause, and does not judge it. A sample outside every recorded pause is judged as before, whatever the clock reads. A missing, unreadable or inconsistent record excludes nothing and prints a named `::notice::`;
  - the workflow passes `--pauses` for this suite only; every other suite is judged exactly as before.

  The change does not apply retroactively. Run 39532347063 stays discarded: its 06:59Z episode lies outside that blackout, and its driver at `b4846424` recorded no pause intervals, so the new guard would exclude nothing from it either.
- **`apt-daily-upgrade` is held on the reference host during runs.** The maintainer holds that unit for the duration of each run of this suite. It is host administration, not code, and nothing in this repository enforces it. The guard still voids a run that the unit, or anything else, disturbs outside a recorded pause.

### 44.10 The second reference-host run was discarded (appended 2026-10-21; §44.1 to §44.9 are not rewritten)

Run [39633666220](https://github.com/orieg/expanse/actions/runs/39633666220) at `67c2a7b9`, which carries §44.9's pause-aware guard, was discarded by the workflow's host-activity guard: "the host was disturbed during the run (9 sample(s) over the void boundary)". Per §8.17 it is discarded, not reinterpreted. None of its figures is quoted, it decides nothing, and S1 and M1 remain unevaluated.

- **The pause record worked.** The guard read 4 recorded pauses and 688 samples taken inside them, one of them over a void boundary at 19:17:55Z (0.45 core-equivalents on the pinned CPUs, during the hourly rsync). That sample was reported and not judged.
- **The 9 judged samples came from two sources outside every pause** (`host-activity.jsonl` in the run's artifacts):
  - **17:21:07Z and 18:24:01Z** (0.30 and 0.41 core-equivalents on the pinned CPUs, no single offender named): interactive ssh logins to the reference host that checked the run's progress while it was in flight.
  - **17:39:03Z to 17:39:23Z, 7 samples** (up to 1.015 core-equivalents host-wide and 0.905 on the pinned CPUs; offenders `hw-probe`, `systemd`, `hdparm`): a daily root cron job, `hw-probe -all -check -upload -monitoring`, which the host's journal shows starting at 17:39:01 UTC every day.
- **What changes for the next run.** Nothing in the instrument. The run is scheduled around the host instead:
  - it is dispatched in a :19 window from 18:19 UTC on, so that it finishes before the next 17:39 job;
  - nobody logs into the host while it runs; status is read from the workflow run only;
  - the `apt-daily` and `apt-daily-upgrade` timers stay stopped, as in §44.9.

  These are operating rules, and nothing in this repository enforces them. The guard still voids a run that any of them, or anything else, disturbs outside a recorded pause.

### 44.11 Outcome (appended 2026-10-22; §44.1 to §44.10 are not rewritten)

Run [39685648391](https://github.com/orieg/expanse/actions/runs/39685648391) at `ac7c1237` (the harness and driver as of `b4846424`, plus §44.9's guard and §44.10's text) was admitted by the host-activity guard.
- Worst foreign load on the pinned CPUs outside a recorded pause: 0.24 core-equivalents, below the 0.25 void boundary.
- Of 677 samples taken during 4 recorded pauses, 3 were over the boundary. They were the hourly rsync at 00:17:33Z, 01:17:33Z and 02:17:32Z, reported and not judged.
- Dispatched at 22:19 UTC with the `apt-daily` timers stopped and no login to the host during the run (§44.10).
- Artifact: `results/baseline_strmap_poll_stall.json`. Intervals are BCa 95% over 5 rounds unless marked.

**S1, stall exists** (N = 2·10⁶, W = 4, T = 1 s): **PASS**.

| shape | median of the largest insert latency | bound (0.5 × median poll) | median poll |
|---|---:|---:|---:|
| `routes` | 449,839,103 ns | 137,075,716 ns | 274,151,433 ns |
| `short` | 990,380,031 ns | 455,301,212 ns | 910,602,425 ns |

**M1, material** (N = 2·10⁷, W = 4, T = 1 s): **MATERIAL** on both shapes.

| shape | `stalled_fraction` | clause 1 (lower bound ≥ 0.01) | `mem_used` arm insert p999 | 10 × `len` arm p999 upper bound | clause 2 |
|---|---|---|---|---:|---|
| `routes` | [0.3619, 0.3743] | met | [4,019, 4,189] ns | 40,990 ns | not met |
| `short` | [0.9994, 1.0000] | met | [3,357, 3,521] ns | 34,014 ns | not met |

- M1 is met through clause 1 alone. On `short` at 2·10⁷ keys the walk takes about as long as the 1 s period, so `stalled_fraction` reaches §44.8's cap.
- Clause 2 shows no tail effect at p999: one walk per second stalls too few inserts to move the 99.9th percentile. The stall shows in the largest latencies and in `stalled_fraction`, not in the percentiles. That result is recorded, and it changes nothing in §44.4.
- The cells that decide nothing (N, W, T and shape beyond the two above) and the `occ-stats` counter pass are in the artifact, reported in full and not summarised here.

**What follows (§44.4).** #1586's premise holds on current `main`: a `mem_used` reading on a `SyncExpanseStrMap` holds every writer for the length of the walk, and at 2·10⁷ keys with a reading every second no insert completes for over a third of wall time (`routes`) or nearly all of it (`short`). An engine design is pre-registered in a new section. This stall is its named gain axis, beside its instruction bounds (AGENTS.md §6 trade-off clause). Candidates A, B and C are as #1586 lists them.

## 46. Pre-registration for #1586 — the string map's plain/shared dispatch moves to its call sites (appended 2026-10-22, locked before any measurement of the change build; next free section)

The number is §46, not §45: `main` ends at §44, and §45 is the B0 record on the open draft #1596 (its branch's §45, cited by its artifact `results/b0_packed_suffix_callgrind_5f8c8e64.json`). Reusing §45 here would make those citations ambiguous, as §44 explains for §43.

This is an instruction-count change on the single-threaded write paths, measured with Callgrind. It is independent of candidate B and of the §44 stall: it changes no allocation, and its gain axis is instructions, not time. Code references are to `origin/main` at `e6520a21`.

### 46.1 The change, and why

§45.2 (#1596) measured `strmap_insert/routes` at 717,202 instructions above `strmap_insert_plain/routes` in the default build: 14.34 per call over 50,000 calls. The two arms differ only in the entry point. `ExpanseStrMap::insert` (`strmap.rs:2221`) tests `self.deferred` and runs the shared twin `insert_shared` on a map switched to deferred reclamation, the plain `insert_impl` otherwise; `insert_plain` (`strmap.rs:2235`) calls `insert_impl` alone. `remove` (`strmap.rs:2992`) has the same shape. B0's disassembly (§45.4) found `insert_shared` and `remove_shared` inlined into `insert` and `remove`, so every plain insert and remove runs through a function that also carries the shared twin's code. The figure of 4.30 per call in §45.2 is the `packed-suffix` delta in `insert`'s own body; the default-build gap this section starts from is 14.34 per call.

The change (commit 3):
- `ExpanseStrMap::insert` and `remove` keep their signatures and plain-tree semantics and lose the deferred test: each is the plain body (`insert_impl`, `remove_impl`) behind a debug-only guard (§46.4).
- Two crate-private entry points, `insert_deferred` and `remove_deferred`, take the shared calls. Each body is the dispatch `insert` and `remove` carry today, kept whole, so a map that was never deferred still gets the plain path in a release build. Each is `#[inline(never)]`: base `insert` and `remove` were called out of line at every measured shared call site (§46.2).
- Every site that mutates a deferred string map calls them (§46.5). The bytes map's exclusive handle already calls its shared entry directly (`BytesExclusive::insert`, `sync.rs:16480`).

### 46.2 Baseline (measured before this lock)

Measured at `4482daac` (`e6520a21` plus the `strmap_insert_plain` arm, commit 1 of this PR), on the x86_64 heavy-jobs host in the B0 container (`rust:1.99.1@sha256:a5ab4292…`, Debian 13.7, valgrind 3.24.0 from apt, `iai-callgrind-runner` 0.16.1): every arm of `instructions`, `smoke_instructions`, `rmw_instructions`, `search_instructions`, `leaf_cap_cascaded` and the `expanse-capi` `smoke_instructions` harness. The baseline was measured before this section was written, and its numbers are the inputs below. No build containing the change has been measured.

- `strmap_insert/routes` 43,325,939; `strmap_insert_plain/routes` 42,608,730. Gap 717,209, 14.34 per call, 1.6554% of `strmap_insert` (§45.2's 717,202 reproduced to 7 instructions; `main` has moved by #1595, docs only).
- Base disassembly (`objdump -d -C --no-show-raw-insn` of the `instructions` bench binary): `ExpanseStrMap::insert` 1,612 instructions, 0 `%fs:`; `ExpanseStrMap::remove` 2,645, 0; `SyncExpanseStrMap::insert` 743, 33; `SyncExpanseStrMap::remove` 4,338, 34; `ExpanseStrMap::olc_insert` 4,361, 12; the plain body inlined into the `strmap_insert_plain` arm 1,419, 0 (all equal to §45.4). `insert_shared`, `remove_shared`, `insert_impl`, `remove_impl` and `StrExclusive`'s methods have no out-of-line symbol.
- On a plain map the deferred test in `insert` and `remove` is three executed instructions per call, the same three in both: `mov 0x238(%rdi),%eax; test %eax,%eax; je` to the shared branch, at `insert+0x1e` and `remove+0x19`. The branch target clones the `Arc` at `0x230`, so `0x238` is the `OnceLock`'s state word (inferred from the target, not from a layout dump).
- Calls into `ExpanseStrMap::insert` / `remove` per arm, from each arm's Callgrind call records:

| arm | base Ir | `insert` calls | `remove` calls |
|---|---:|---:|---:|
| `strmap_insert/routes` | 43,325,939 | 50,000 | 0 |
| `strmap_oscillate/routes` | 33,843,428 | 50,000 | 50,000 |
| `strmap_churn/routes` | 119,575,793 | 100,000 | 50,000 |
| `strmap_refill/routes` | 103,338,884 | 50,000 | 50,000 |
| `strmap_refill_small/routes` | 87,019,217 | 50,000 | 50,000 |
| `strmap_clear_refill/routes` | 56,562,168 | 50,000 | 0 |
| `strmap_transcode_insert/clean` | 50,575,381 | 50,000 | 0 |
| `strmap_transcode_insert/escaped` | 100,121,039 | 50,000 | 0 |
| `ordered_bytesmap_insert/clean` | 50,875,378 | 50,000 | 0 |
| `ordered_bytesmap_insert/escaped` | 68,267,044 | 50,000 | 0 |
| `strmap_raw_insert/clean` | 46,275,777 | 50,000 | 0 |
| `domain_dict_insert/routes` | 89,240,581 | 50,000 | 0 |
| `smoke_instructions` `strmap_insert/routes` | 8,440,946 | 10,000 | 0 |
| `smoke_instructions` `strmap_churn/routes` | 22,929,493 | 20,000 | 10,000 |
| `sync_strmap_update/routes` (through `StrExclusive`) | 73,966,529 | 50,000 | 0 |
| `sync_strmap_compare_exchange/routes` (through `StrExclusive`) | 123,149,234 | 50,000 | 0 |
| `sync_strmap_insert/routes` (serialised fallback) | 76,895,794 | 44 | 0 |
| `sync_strmap_insert_sorted/uuid` | 45,544,902 | 216 | 0 |
| `sync_strmap_insert_short/short` | 96,858,964 | 1 | 0 |
| `rmw_instructions` `sync_strmap_cas_insert/routes` | 99,147,549 | 2 | 0 |

No other arm calls either function.

### 46.3 Bounds

The derivation. On a plain map the change removes, at least, the three-instruction deferred test from each call (§46.2). Whether it removes more depends on how the shared twin's absence changes the plain body's register allocation and frame, which cannot be derived in advance, so the gates are set on the three instructions. The full 14.34 per call is a ceiling, reached only if `insert` matched the plain body inlined into the arm, call included. The change keeps `insert` out of line, so the call itself (prologue, epilogue, `call`, `ret`, argument moves) remains, and the ceiling is not expected.

**G1, gain on insert.** `strmap_insert/routes`, head against base: Δ ≤ −0.34%. Arithmetic: 3 × 50,000 = 150,000 instructions; 150,000 / 43,325,939 = 0.3462%, floored to 0.34%. Expected range [−1.66%, −0.34%].

**G2, gain on remove.** No arm calls remove alone, so G2 reads the per-function record: on `strmap_oscillate/routes`, the inclusive Ir of the calls into `ExpanseStrMap::remove` (16,087,536 in base, over 50,000 calls) falls by ≥ 150,000 (3 × 50,000; 0.93% of it). If the head inlines `remove` into the arm, so that no call record exists, G2 is read instead on the arm total as Δ ≤ −0.88% (3 × 100,000 / 33,843,428 = 0.8864%, floored), and the verdict says it was read the weaker way.

**P, plain arms.** No plain `strmap_*`, `ordered_bytesmap_*`, `domain_*` or `judysl_*` arm, in any harness, rises by more than 0.1%. Falls are reported and attributed. Predicted falls, from the call counts in §46.2 at three instructions per call (predictions, not gates):

| arm | predicted Δ, at most |
|---|---:|
| `strmap_oscillate/routes` | −0.8864% |
| `strmap_churn/routes` | −0.3763% |
| `strmap_refill/routes` | −0.2903% |
| `strmap_refill_small/routes` | −0.3448% |
| `strmap_clear_refill/routes` | −0.2652% |
| `strmap_transcode_insert/clean`, `/escaped` | −0.2966%, −0.1498% |
| `ordered_bytesmap_insert/clean`, `/escaped` | −0.2948%, −0.2197% |
| `strmap_raw_insert/clean` | −0.3241% |
| `domain_dict_insert/routes` | −0.1681% |
| `smoke_instructions` `strmap_insert/routes`, `strmap_churn/routes` | −0.3554%, −0.3925% |

Plain arms that call neither function (gets, scans, cursors, `judysl_*`) are predicted unchanged.

**S, shared arms.** Every `sync_strmap_*` arm (`instructions` and `rmw_instructions`): no change at ±0.1%. There are no `sync_ordered_bytesmap_*` Callgrind arms; the ordered-bytes wrapper reaches the map through `SyncExpanseStrMap` and `StrExclusive`, so the same bound covers its paths. Derivation: each shared call site now calls `insert_deferred` or `remove_deferred`, whose body is base `insert` or `remove` verbatim, kept out of line as base `insert` was at every measured shared call (§46.2). The executed instructions on the shared paths are therefore unchanged by construction, and what remains is layout, which B0 measured at 7 instructions on unrelated arms (§45.2).

**N, non-string arms.** No change: every arm that builds no string map is within ±0.01% (B0's layout-only differences reached 0.001%; the bound is ten times that). Any non-zero delta is listed with its count.

**C, codegen** (`objdump -d -C --no-show-raw-insn` of the `instructions` bench binary, base against head, instructions and `%fs:` operands per symbol):
- reported for `ExpanseStrMap::insert`, `remove`, `insert_deferred`, `remove_deferred`, `olc_insert`, and `SyncExpanseStrMap::insert`, `remove`, with any symbol that appears or disappears named;
- no plain function gains a `%fs:` reference: `ExpanseStrMap::insert` and `remove` stay at 0, and so do the plain arms' wrappers;
- `SyncExpanseStrMap::insert` and `remove` keep their `%fs:` counts (33, 34).

**Guard.** A plain `insert` or `remove` on a deferred map is caught: the tests `strmap::tests::plain_insert_on_a_deferred_map_is_caught` and `plain_remove_on_a_deferred_map_is_caught` pass, and fail when the guard's assertion is deleted (§46.4).

### 46.4 The guard, and why it is debug-only

The plain entry points carry `debug_assert!(self.deferred.get().is_none())`. A release build relies on this invariant: **no `&mut ExpanseStrMap` to a deferred map is reachable outside `expanse-trie`.**
- `deferred` is set only by `ExpanseStrMap::defer_to`, which is `pub(crate)` (`strmap.rs:1950`). Its one non-test caller is `SyncExpanseStrMap::from_map` (`sync.rs:13933`), which moves the map into the wrapper's private `shared` field. `SharedTree for ExpanseStrMap` does not override the trait's no-op `defer_to` (`sync.rs:706`), so `prepare_staged` cannot defer a string map, and no string-map path calls it.
- The wrapper exposes the map as `&ExpanseStrMap` only (`with_locked`, `sync.rs:14486`). `&mut` is reachable only through the crate-private `exclusive` and the `Shared` write closures; `with_exclusive` passes `StrExclusive`, which holds the map privately (`sync.rs:14541`).
- Nothing moves the map out: `SyncExpanseStrMap` has no `into_inner`, `ExpanseStrMap` is not `Clone`, and `ExpanseOrderedBytesMap::clone` and `SyncExpanseOrderedBytesMap::into_inner` build a fresh, never-deferred map or return the wrapper itself.
- `expanse-capi` and the bindings hold plain `ExpanseStrMap`s or use the public `SyncExpanseStrMap` API, never a deferred `&mut`.

The crate's own sites are pinned by a source census (§46.5), and the shared entries keep a release-mode dispatch: a never-deferred map handed to `insert_deferred` runs the plain path. The guard is debug-only rather than a `#[cold]` check because a release check would put a load and a branch back on the plain path, the cost this change removes (AGENTS.md §2.1 invariant 5).

### 46.5 What the change must also satisfy

- Every crate site that mutates a deferred string map calls `insert_deferred` or `remove_deferred`. The source census `sync::str_deferred_dispatch_tests::deferred_str_mutations_call_the_shared_twin` scans `SyncExpanseStrMap`'s and `StrExclusive`'s production code for a plain `insert`/`remove` on the map, and counts the shared-twin calls (9). Its negative controls (`deferred_dispatch_scan_fails_on_each_clause`) show each clause failing on its own; reverting one real site is shown red once, by hand, and recorded in the PR.
- `scripts/gate.sh` in full, `scripts/test_occ_stats.sh` and `scripts/test_collector_census.sh` pass, with their test counts in the PR.

### 46.6 Falsifiers, and what follows each

| Falsifier | What follows |
|---|---|
| G1 or G2 not met | `callgrind_annotate --inclusive=no` and `--auto=yes`, base against head, on the arm, then the disassembly of `insert` / `remove`. Then either the code changes and is re-measured against the same bound, or the change is recorded as rejected. |
| A P arm rises by more than 0.1% | attributed the same way before anything changes; the change does not ship over it |
| An S arm moves by more than 0.1% | attributed; the §46.3 derivation was wrong for that path, and the PR says why |
| An N arm moves by more than 0.01% | attributed; layout is not accepted as the cause without the annotate diff |
| A plain function gains a `%fs:` reference, or `SyncExpanseStrMap::insert`/`remove` change theirs | read the disassembly; a plain-path `%fs:` blocks the change |
| A guard test does not fail with its assertion deleted | the guard does not guard; fixed before the PR is marked ready |

No bound in this section moves after the first measurement of the change build.

### 46.7 Not predicted

- How much of the 14.34-per-call gap beyond the three instructions the change recovers. That is the codegen effect of removing the inlined shared twin, and only the measurement says.
- Whether LLVM now inlines the smaller `insert` or `remove` into a caller in the crate (`domain`, `ordered_bytesmap`) or across it under thin LTO. A newly inlined call removes the call overhead and can move an arm by more than predicted; it is named in the results if it happens.
- Wall-clock effect. Nothing is timed.

### 46.8 Outcome (appended 2026-10-22, after the runs; §46.1–46.7 are unchanged)

Three builds were measured against the §46.2 baseline, one change each, on the instrument of §46.2. All three, the baseline and the per-function attribution are in `results/strmap_dispatch_split_callgrind_fd6f39fc.json` (372 arms per build). The third build is the one that ships.

| build | commit | change |
|---|---|---|
| head1 | `18cc3e96` | the split as §46.1 states it: `insert` and `remove` out of line, without the deferred test |
| head2 | `f0cf28b6` | head1 plus `#[inline(always)]` on `insert` and `remove`, as `insert_plain` has |
| head3 | `fd6f39fc` | head2 plus `ExpanseOrderedBytesMap::insert` computing the encoded key in its own body |

head2 and head3 depart from §46.1's "the change keeps `insert` out of line". They were made through §46.6's first row, after head1 tripped G1 and P and was attributed; no bound moved.

**head1 tripped G1 and P, and was attributed before anything changed.** `strmap_insert/routes` rose 254,313 (+0.5870%, against G1's ≤ −0.34%), and all 14 plain arms that call `insert` or `remove` rose, by +0.1969% to +0.6511% (`strmap_raw_insert/clean`). `callgrind_annotate --inclusive=no` puts the whole rise in `ExpanseStrMap::insert`'s own body: +304,347, offset by −50,034 in `__rust_alloc_zeroed`. Per instruction (`--dump-instr=yes`, self Ir per address joined with `objdump`), `insert` executed 370,017 more `mov`s and about 100,000 fewer `test`/`jne` (the deferred test, gone as predicted). The chunk loop, executed 193,748 times, gained one register copy. The terminal-chunk block, executed 49,503 times, gained two register copies and a reload of a static's address from the GOT that base kept in a register. Data reads in `insert` fell by 422,337 while instructions rose. So the plain body compiled out of line on its own allocates registers worse than it did beside the shared twin. The same body inlined into its caller, which the `strmap_insert_plain` arm measures, is 717,209 instructions under base `insert`. The shared arms and the non-string arms were identical to base.

**head2 met G1 and tripped P on `ordered_bytesmap_insert`.** `strmap_insert/routes` reached 42,608,730, the `strmap_insert_plain` count to the instruction (−1.6554%). `ordered_bytesmap_insert/clean` rose 551,289 (+1.0836%) and `/escaped` 549,917 (+0.8055%). Attribution: `ExpanseOrderedBytesMap::insert` reaches the map through `with_encoded_key`'s closure, which has one call site per encoding branch. With `insert` inlined into it, the closure was compiled out of line (a new 1,298-instruction symbol), so it cost +250,000 in `ExpanseOrderedBytesMap::insert` for the call and +301,323 in the closure's copy of the plain body against base `insert`'s self cost.

**head3: every bound met.**

| gate | bound | head3 | verdict |
|---|---|---|---|
| G1 | `strmap_insert/routes` Δ ≤ −0.34% | 43,325,939 → 42,608,730, **−1.6554%** | met |
| G2 | `remove`'s inclusive Ir on `strmap_oscillate/routes` −150,000; read on the arm total (Δ ≤ −0.88%) if `remove` leaves no call record | `remove` is inlined and leaves no call record; arm 33,843,428 → 32,143,398, **−5.0232%** | met, read the weaker way |
| P | no plain string arm up by more than 0.1% | 14 arms fall (table below), 27 unchanged; none rises | met |
| S | every `sync_strmap_*` arm within ±0.1% | all 20 identical to base | met |
| N | every non-string arm within ±0.01% | all 311 identical to base | met |
| C | no plain function gains `%fs:`; `SyncExpanseStrMap::insert`/`remove` keep 33/34 | 0 in every plain function and arm wrapper read; 33/34 | met |
| Guard | the guard tests pass, and fail with the assertion deleted | both pass; with the assertion deleted both fail with "panic did not contain expected string" (the plain path then trips `alloc_raw`'s debug assertion, `alloc.rs:1180`) | met |

Plain arms that moved, base against each build (every other plain arm, `strmap_insert_plain/routes` included, is unchanged in all three):

| arm | base | head1 | head2 | head3 | head3 Δ |
|---|---:|---:|---:|---:|---:|
| `strmap_insert/routes` | 43,325,939 | +0.5870% | −1.6554% | 42,608,730 | −1.6554% |
| `strmap_oscillate/routes` | 33,843,428 | +0.4431% | −5.0232% | 32,143,398 | −5.0232% |
| `strmap_churn/routes` | 119,575,793 | +0.4181% | −3.2093% | 115,738,295 | −3.2093% |
| `strmap_refill/routes` | 103,338,884 | +0.1969% | −2.2997% | 100,962,425 | −2.2997% |
| `strmap_refill_small/routes` | 87,019,217 | +0.2313% | −2.5900% | 84,765,452 | −2.5900% |
| `strmap_clear_refill/routes` | 56,562,168 | +0.4537% | −1.2117% | 55,876,779 | −1.2117% |
| `strmap_raw_insert/clean` | 46,275,777 | +0.6511% | −1.4237% | 45,616,965 | −1.4237% |
| `strmap_transcode_insert/clean` | 50,575,381 | +0.5957% | −1.1999% | 49,968,515 | −1.1999% |
| `strmap_transcode_insert/escaped` | 100,121,039 | +0.3495% | −0.5991% | 99,521,198 | −0.5991% |
| `ordered_bytesmap_insert/clean` | 50,875,378 | +0.5922% | +1.0836% | 50,680,795 | −0.3825% |
| `ordered_bytesmap_insert/escaped` | 68,267,044 | +0.5126% | +0.8055% | 67,867,003 | −0.5860% |
| `domain_dict_insert/routes` | 89,240,581 | +0.2850% | −1.1815% | 88,186,207 | −1.1815% |
| `smoke_instructions` `strmap_insert/routes` | 8,440,946 | +0.2923% | −2.0059% | 8,271,632 | −2.0059% |
| `smoke_instructions` `strmap_churn/routes` | 22,929,493 | +0.3925% | −3.6525% | 22,091,995 | −3.6525% |

Each fall is larger than §46.3 predicted, because the prediction priced only the deferred test and the shipped change also inlines the call. Attribution, head3 against base: on `strmap_insert/routes`, `ExpanseStrMap::insert` (−14,770,709) moves into the arm's wrapper (+14,103,787), and `__rust_alloc_zeroed` falls 50,252; on `strmap_oscillate/routes`, `insert` (−5,601,264) and `remove` (−6,900,000) move into the wrapper (+10,801,275); on `domain_dict_insert/routes`, `insert` (−14,770,701) moves into `ExpanseDomainDict::intern`'s closure (+13,716,327); on `ordered_bytesmap_insert/clean`, `insert` (−5,362,925) moves into `ExpanseOrderedBytesMap::insert` (+5,168,376). The split of `strmap_oscillate`'s fall between its inserts and its removes is not established: both are inlined into one wrapper.

**Codegen** (`objdump -d -C --no-show-raw-insn`, instructions and `%fs:` per symbol):

| symbol | base | head1 | head2 | head3 |
|---|---:|---:|---:|---:|
| `ExpanseStrMap::insert` | 1,612, 0 | 1,298, 0 | inlined | inlined |
| `ExpanseStrMap::remove` | 2,645, 0 | 2,295, 0 | inlined | inlined |
| `ExpanseStrMap::insert_deferred` | — | 1,612, 0 | 1,612, 0 | 1,612, 0 |
| `ExpanseStrMap::remove_deferred` | — | 2,645, 0 | 2,645, 0 | 2,645, 0 |
| `ExpanseStrMap::olc_insert` | 4,361, 12 | 4,361, 12 | 4,361, 12 | 4,361, 12 |
| `SyncExpanseStrMap::insert` | 743, 33 | 743, 33 | 743, 33 | 743, 33 |
| `SyncExpanseStrMap::remove` | 4,338, 34 | 4,338, 34 | 4,338, 34 | 4,338, 34 |
| `ExpanseOrderedBytesMap::insert` | 139, 0 | 139, 0 | 136, 0 | 2,659, 0 |
| its closure | inlined | inlined | 1,298, 0 | inlined |
| `strmap_insert` arm wrapper | 85, 0 | 85, 0 | 1,419, 0 | 1,419, 0 |
| `strmap_oscillate` arm wrapper | 115, 0 | 115, 0 | 3,643, 0 | 3,643, 0 |

- The shared entries compile to base `insert` and `remove` exactly in static size, which is the §46.3 S derivation, and the shared arms bear it out to the instruction.
- In head2 and head3 the binary has no `strmap_insert_plain` wrapper symbol, and the two arms measure the same 42,608,730. The two wrappers are taken to have been folded into one; that is inferred from the absent symbol and the equal counts and not verified further.
- The shipped change grows every plain caller by the body it now inlines: `ExpanseOrderedBytesMap::insert` from 139 to 2,659 instructions, the `strmap_churn` wrapper from 119 to 4,867, and so on for each caller of `insert` or `remove`, in this crate, in `expanse-capi` and in the bindings. Callgrind counts executed instructions and does not price that size. Its wall-clock effect, if any, is unmeasured.

**What this does not establish.**
- Wall clock. Nothing was timed.
- `ExpanseOrderedBytesMap::remove`, `ExpanseStrMap::update` and every caller outside the measured arms (`expanse-capi`'s `expanse_strmap_insert`/`remove`, the bindings) now inline the plain body too. No arm measures them, so their counts are unmeasured. `ExpanseOrderedBytesMap::remove` still goes through `with_encoded_key`'s closure, the shape that cost `insert` +1.08% in head2.
- `ins_slot` and `clear` keep their deferred dispatch. They are outside this change.
- The CI `instruction-counts` job reruns the gate on its own host and toolchain build, against `main`; its table is the confirmation, and the PR body carries it.

## 47. Pre-registration for #1586 design B — shells and suffix leaves through the tree's allocator, and a `mem_used` that does not stop writers (appended 2026-10-22, locked before any measurement of a build containing design B; next free section)

The number is §47: `main` holds §44 and §46, and §45 is the B0 record on the open draft #1596 (its branch's §45, cited by its artifact), so §45 stays reserved as §46 explains. This section is the engine pre-registration §44.4 calls for after `MATERIAL`, with §44.11's stall as its named gain axis (AGENTS.md §6 trade-off clause). It holds no data of the change. Code references are to `origin/main` at `f65060d8`.

### 47.1 The change, and what it has to give the caller

**The caller.** ExpanseDB keeps one long-lived `SyncExpanseStrMap` that keeps taking inserts and reads its memory once per checkpoint, every few seconds. It wants the logical figure (`mem_used`), tolerates about ±10% error, does not need an upper bound, needs the figure to move smoothly, and needs the reading not to stall writers. §44.11 measured the reading today: with 4 writers at 2·10⁷ keys, no insert completed for a fraction [0.3619, 0.3743] (`routes`) or [0.9994, 1.0000] (`short`) of wall time.

**Design B**, chosen by review over candidates A and C of #1586, in three increments, each measured on its own (AGENTS.md §6):

- **I1: shells through `NodeAlloc`.** A `StrNode` shell (40 bytes, `size_of::<StrNode>()`) becomes a raw block of the map's one `NodeAlloc`, allocated by an out-of-line `new_shell::<SHARED>` (`alloc_bytes_dispatch::<SHARED>`) and freed by an out-of-line `free_shell`. Today it is a `Box` (`strmap.rs:2180`, `:2297`, `:2380`, `:4701`), freed in `dispose_node` (`strmap.rs:480`). The root slot `root: Option<Box<StrNode>>` (`strmap.rs:1884`) becomes `Option<RootNode>`, a `#[repr(transparent)]` owning `NonNull<StrNode>` with no `Drop`. It cannot stay a `Box`, because dropping one hands the block to the global allocator. The type keeps the nullable-pointer layout that `root_raw` (`strmap.rs:4444`) and `root_or_create_shared` (`strmap.rs:4704`) read and write through a raw cast, and a compile-time assertion pins that layout. Out of line because `alloc_raw` is `#[inline(always)]` (`alloc.rs:1177`): an inlined shell allocation would move the insert walks' codegen (AGENTS.md §2.1 invariant 5).
- **I2: `packed-suffix` becomes the default** (AGENTS.md §2.7). The inverse `ablation-unpacked-suffix` restores global suffix allocation. The name `packed-suffix` stays declared as a `compile_error!`. Every consumer `git grep packed-suffix` finds migrates in the same PR: `ci.yml` (`:1035`, `:1864`, `:1867`), the two tests that branch on it (`tests/no_heap_churn.rs:514`, `tests/test_mem_held.rs:237`), `alloc.rs:304`–`:338`, `docs/BENCHMARKING.md:2121`, `docs/CI.md:65`.
- **I3: no walk.** `StrNode::shell_bytes` (`strmap.rs:1320`) and `SUFFIX_IN_NODE_ALLOC` (`strmap.rs:228`) are deleted. `ExpanseStrMap::mem_used` becomes `self.alloc.bytes_in_use()`, and `mem_held` becomes `self.alloc.bytes_held()`. `SyncExpanseStrMap::mem_used` (`sync.rs:14409`) no longer calls `Shared::read_locked`. It reads the allocator's accounting shards through an `AcctCell` taken at construction (§47.4), as `SyncExpanseBlobMap::mem_used_estimate` reads its index (`sync.rs:13021`). The ordered-bytes delegate (`sync.rs:16852`) follows. `mem_held` keeps `read_locked`. Under `ablation-unpacked-suffix` the suffix bytes are outside the allocator again, so a suffix-only walk (`unpacked_suffix_bytes`) and the `read_locked` reading come back with that feature alone.

**The review findings and where each is answered.**
- (a) `shell_bytes` visits every entry even under `packed-suffix`: B0's `sync_strmap_mem_used` fell only 1.07% (`routes`) and 18.00% (`short`) under the feature (§45.2). Both halves are therefore in B, and I3 deletes the walk rather than shortening it.
- (b) A lock-free read must not form `&ExpanseStrMap` while optimistic writers mutate: §47.4.
- (c) `AcctCell::bytes_in_use` clamps a negative sum to 0: §47.4 says where a debug assertion is sound and where it is not.
- (d) "Exact when quiet" is stated as a happens-before contract: §47.4.
- (e) A deferred `NodeAlloc` allocates from the collector's locked striped freelists, a per-insert cost Callgrind counts in instructions but not in waiting: G-instr bounds the instructions, and G-scale is the instrument for the waiting.
- (f) §44 does not control population growth: in §44.11's M1 cell the map grew from 2·10⁷ to 1.2·10⁸ keys in 30 s (`len_after`, `routes`, `len` arm). G-stall adds a constant-population churn cell (§47.3).

### 47.2 Baseline (measured before this lock)

Measured at `f65060d8` (`origin/main`, no change), on the x86_64 heavy-jobs host in the B0 container (`rust:1.99.1@sha256:a5ab4292…`, Debian 13.7, valgrind 3.24.0 from apt, `iai-callgrind-runner` 0.16.1): every arm of `instructions`, `smoke_instructions`, `rmw_instructions`, `search_instructions`, `leaf_cap_cascaded` and the `expanse-capi` `smoke_instructions` harness, in the default build and under `--features packed-suffix`. That is 372 arms per build. Artifacts:
- `results/strmap_design_b_baseline_callgrind_f65060d8.json`: per-arm counts for both builds, and for each of the 61 string arms the call records §47.3 reads (shell allocations, frees and retirements, the `shell_bytes` walk, the `mem_used` readings), with how each was extracted;
- `results/allocator_overhead_f65060d8.txt`: the byte census, both builds, at 10⁶ and 10⁷ keys.

What it shows:
- **The default build reproduces §46.8's shipped head** to the instruction: `strmap_insert/routes` 42,608,730, `strmap_oscillate/routes` 32,143,398.
- **The 311 arms that build no string map are identical between the two builds.**
- **The suffix half on `main` after #1597.** On the plain insert arms it is now small: `strmap_insert/routes` −0.0295%, against B0's +0.5351% before #1597 removed the codegen coupling (§45.2, §46.8). On the shared insert arms it is B0's, since #1597 left the shared paths identical (§46.8 S): `sync_strmap_insert_short/short` +6.3691%, `sync_strmap_insert_sorted/uuid` +6.2901%, `sync_strmap_insert/routes` +0.9798%.
- **Shells per arm.** In the packed build every suffix is a `NodeAlloc` block, so a `malloc` left in a string insert body is a shell. `strmap_insert/routes` allocates 6,253 shells (6,252 in `split_suffix`, 1 root) at 189.9 instructions per `malloc` call. `strmap_oscillate/routes` allocates and frees 50,000 roots, at 41.0 per `malloc` and 65.0 per `free`. `sync_strmap_insert/routes` allocates 6,253 at 215.8 per call. `sync_strmap_remove/routes` retires 6,253. The other arms are in the artifact.
- **The walk.** `sync_strmap_mem_used` spends 9,813,786 instructions (`routes`) and 10,986,132 (`short`) in its four `mem_used` readings, of 15,663,913 and 16,838,489. The rest is the arm dropping its 50,000 probe keys, which is unchanged. `strmap_clear_refill/routes` spends 2,410,866 in the `shell_bytes` walk inside `clear` (packed build).
- **The byte census** (exact columns, B/key; packed minus default as in §45.3, which these reproduce):

| N | shape | `mem_held` default → packed | `+headers` default → packed | shells per key (packed census, live `(40, 8)` requests) |
|---|---|---:|---:|---:|
| 10⁶ | `prefix` | 35.73 → 48.72 | 49.06 → 49.34 | 0.0518 |
| 10⁶ | `uuid` | 68.80 → 73.57 | 89.25 → 74.21 | 0.000114 |
| 10⁷ | `prefix` | 40.40 → 46.72 | 47.21 → 49.24 | 0.2747 |
| 10⁷ | `uuid` | 66.81 → 69.71 | 86.93 → 69.96 | 0.0011575 |

### 47.3 Bounds

Every bound is a function in `scripts/strmap_stall_bounds.py` with pinned tests (commit 1 of this PR). `python3 scripts/strmap_stall_bounds.py --design-b` prints the G-instr table below from the §47.2 artifact.

#### G-stall (gain; reference host)

**Cells.** §44's M1 cell, unchanged: N = 2·10⁷, W = 4, T = 1 s, D = 30 s, both shapes, arms `none`, `len` and `mem_used` interleaved per round as §44.8 orders them, 5 rounds, one process per (cell, arm, round), §44's pin and placement. Beside it, a **churn cell** at the same coordinates. Its population holds at N, because writer `w`, at its step `k`, inserts the fresh index `N + w + k·W` and then removes index `w + k·W` (`--workload churn` in `strmap_poll_stall.rs`). Each removal finds its key: it is a prefill key while `k < N/W`, and after that the key the same writer inserted `N/W` steps earlier. Keys are indexed, so the churn cell's `short` keys are a different draw from §44's prefill. Both run on the B build; no `main` cell is part of G-stall.

**Statistic.** A lock-free reading leaves no window in which the gate is closed, so `stalled_fraction` measures nothing and is not read. G-stall reads the per-round largest insert latency (`insert.max_ns`), with BCa 95% intervals over the 5 rounds (2,000 resamples, `ci_method` recorded). Per (shape, workload):
- `PASS` iff the `mem_used` arm's upper bound ≤ k × the `len` arm's upper bound;
- `FAIL` iff the `mem_used` arm's lower bound > k × the `len` arm's upper bound;
- `INTERMEDIATE` otherwise.

The run is `PASS` iff all four are, `FAIL` iff any is, and `INTERMEDIATE` otherwise.

**k = 3.** The `len` arm is a lock-free reading at the same cadence, so it already carries the poller's presence. A `mem_used` reading that waits for nothing should match it. The per-round maximum is one sample per round and moves with scheduling: §44.11's `len` arm at the M1 cell spread over its rounds by 2.27× (`routes`, 106,239 to 241,151 ns) and 1.80× (`short`). k sits above that spread. On `main` the same rule fails by more than two orders of magnitude: §44.11's `mem_used` arm on `routes` is [517,839,257, 533,305,753] ns against 3 × 210,354 ns (pinned in the bounds module's self-test).

**Voids.** §44.8's void rules, plus, on a churn round, a population that moved by more than W between `len_before` and `len_after`, a removal that found no key, or no removal at all. A growth cell is not held to these.

**Reported, deciding nothing:** every §44.4 statistic per cell, the removal latencies of the churn cell, and the poll durations.

#### G-accuracy (deterministic tests on the B build)

1. **Exact after join.** After every writer thread is joined, `SyncExpanseStrMap::mem_used()` equals `with_locked(ExpanseStrMap::mem_used)`, equals `stats().node_bytes.total()`, and the shards' signed sum is ≥ 0. This holds after inserts, after removals, and after `clear`, at W = 4.
2. **The census still sums.** `stats().node_bytes` sums to `mem_used` on plain and shared maps (`stats_breakdown_sums_to_mem_used`), and `layout_census().bytes` equals `mem_used` (`census::tests`), with shells counted at 48 bytes, their accounted size.
3. **Band while writers run.** W = 4 writers run phases, each growing the map by at most 25%, with an exact quiet reading E_i between phases. During phase i a poller reads `mem_used` continuously. Every reading must lie in `poll_band(E_{i−1}, E_i, 0.10)`, that is [0.9 × min, 1.1 × max]. One phase is churn (an insert and a removal per step), so the band is tested at constant population too.
4. **Smooth.** Two consecutive readings within one phase differ by at most `poll_step_bound(E_{i−1}, E_i, 0.02)`: the phase's whole change, plus 2% of the map for the bytes of operations in flight at either reading.

A reading that walks, caches or skips a shard fails at least one of 1, 3 and 4, and each is shown failing once with that defect, by hand, recorded in the PR (G-safety).

#### G-instr (Callgrind; PR B head against `main`, same host and container as §47.2)

**The trade-off.** B accepts instruction losses on the shared write paths for G-stall's gain (AGENTS.md §6 trade-off clause). Every arm the bounds below expect to lose is named, with its bound, before any measurement of B.

**Derivation, per string arm** (`instr_bound` and `shell_term`):

bound = (packed − main) − walk + shell term ± slack

- **Suffix half, measured:** packed minus main, from §47.2.
- **Walk, measured:** the `shell_bytes` walk the change deletes, the packed build's inclusive count.
- **Shell term, derived:** each shell allocation moves from `malloc` (its measured cost in that arm) to `new_shell`; each plain free from `free` (measured) to `free_shell`; each shared retirement gains one discharge and an out-of-line call. The per-call costs of the new path are not measurable before B exists, so they are ranges:
  - **plain allocation, [25, 85] instructions.** The upper end is the packed build's measured mean for a plain `new_suffix` (264,767 over 3,125 calls in `strmap_insert/routes`), which also copies the key bytes, as a shell does not. The lower end is the class pop, three relaxed counters, the 40-byte zeroing and the 40-byte write.
  - **plain free, [15, 45].** The upper end is the measured `dispose_suffix` mean (250,080 over 6,252 calls); the lower end is the class push and two counters.
  - **shared allocation, [150, 340].** This is the collector freelist pop, measured at 48.0 per call (`Collector::pop_freelist`, 2,400,700 over 50,000 calls in `sync_strmap_insert_short`), plus `calloc` on a miss, measured at 98.8 to 245 per call across the shared arms, plus a writer-slot charge and the call, estimated at ≤ 40.
  - **shared retirement, [−65, +45] relative to today's `Collector::retire`.** A discharge and the call add up to 45. A retired shell later reclaims to a collector freelist class (40, 16) instead of the system (`free`, 65 per call), which saves up to 65 if the arm reclaims.
- **Slack:** 0.1% of the arm (AGENTS.md §6 review threshold) with a floor of 500, for codegen and layout.

**`sync_strmap_mem_used`** (`mem_used_fold_bound`): each of the 4 readings becomes one fold over 64 accounting shards, [64, 1,000] instructions, and nothing else in the arm changes. Bound: −(the readings' cost on main) + 4 × [64, 1,000] ± slack.

**Non-string arms:** |Δ| ≤ 0.01% of the arm. On `main` they are identical between the two builds; B0's layout-only differences reached 0.001%.

**Per increment.** I1 is read against the shell term alone (suffix half 0, walk unchanged), I2 against shell term plus suffix half, and I3 against the full bound. The full bound against `main` is the gate. A per-increment departure is attributed before the next increment (§47.5), and it decides nothing by itself.

The table (`python3 scripts/strmap_stall_bounds.py --design-b`; low and high Δ against `main`):

| harness | arm | main | kind | bound low | bound high |
|---|---|---:|---|---:|---:|
| `capi smoke_instructions` | `judysl_get/routes` | 6,984,016 | plain string | -0.1000% | +0.1000% |
| `capi smoke_instructions` | `judysl_insert/routes` | 9,001,080 | plain string | -2.9792% | -1.9433% |
| `instructions` | `domain_dict_get/routes` | 51,816,562 | plain string | -0.1002% | +0.0998% |
| `instructions` | `domain_dict_insert/routes` | 88,186,207 | plain string | -1.1955% | -0.5701% |
| `instructions` | `ordered_bytesmap_cursor_into/clean` | 18,841,688 | plain string | -0.0978% | +0.1022% |
| `instructions` | `ordered_bytesmap_cursor_into/escaped` | 29,818,423 | plain string | -0.0966% | +0.1034% |
| `instructions` | `ordered_bytesmap_cursor/clean` | 26,741,903 | plain string | -0.0992% | +0.1008% |
| `instructions` | `ordered_bytesmap_cursor/escaped` | 40,021,959 | plain string | -0.0984% | +0.1016% |
| `instructions` | `ordered_bytesmap_get/clean` | 21,948,005 | plain string | -0.0651% | +0.1349% |
| `instructions` | `ordered_bytesmap_get/escaped` | 33,353,487 | plain string | -0.1003% | +0.0997% |
| `instructions` | `ordered_bytesmap_insert/clean` | 50,680,795 | plain string | -9.1108% | -8.8051% |
| `instructions` | `ordered_bytesmap_insert/escaped` | 67,867,003 | plain string | -6.5233% | -6.3231% |
| `instructions` | `strmap_churn/routes` | 115,738,295 | plain string | -0.0136% | +0.1864% |
| `instructions` | `strmap_clear_refill/routes` | 55,876,779 | plain string | -7.9128% | -5.8665% |
| `instructions` | `strmap_cursor_scan/paths` | 9,205,128 | plain string | -0.1070% | +0.0930% |
| `instructions` | `strmap_cursor_scan/paths_dense` | 13,693,945 | plain string | -0.1000% | +0.1000% |
| `instructions` | `strmap_get_short/short` | 20,487,291 | plain string | -0.0955% | +0.1045% |
| `instructions` | `strmap_get/routes` | 35,163,713 | plain string | -0.1001% | +0.0999% |
| `instructions` | `strmap_insert_plain/routes` | 42,608,730 | plain string | -2.5490% | -1.4684% |
| `instructions` | `strmap_insert/routes` | 42,608,730 | plain string | -2.5490% | -1.4684% |
| `instructions` | `strmap_oscillate/routes` | 32,143,398 | plain string | -12.4309% | +1.7688% |
| `instructions` | `strmap_prefix_bounded/paths` | 2,636,575 | plain string | -0.6446% | -0.4446% |
| `instructions` | `strmap_prefix_bounded/paths_dense` | 3,762,462 | plain string | -0.1000% | +0.1000% |
| `instructions` | `strmap_prefix_scan/paths` | 2,821,486 | plain string | -0.6074% | -0.4074% |
| `instructions` | `strmap_prefix_scan/paths_dense` | 3,952,485 | plain string | -0.1000% | +0.1000% |
| `instructions` | `strmap_prefix_seek/paths` | 160,473 | plain string | -9.2327% | -8.6095% |
| `instructions` | `strmap_prefix_seek/paths_dense` | 160,751 | plain string | -0.3110% | +0.3110% |
| `instructions` | `strmap_raw_cursor/clean` | 14,533,670 | plain string | -0.0971% | +0.1029% |
| `instructions` | `strmap_raw_get/clean` | 17,398,005 | plain string | -0.0560% | +0.1440% |
| `instructions` | `strmap_raw_insert/clean` | 45,616,965 | plain string | -10.0247% | -9.7072% |
| `instructions` | `strmap_refill_small/routes` | 84,765,452 | plain string | -1.4103% | -0.5165% |
| `instructions` | `strmap_refill/routes` | 100,962,425 | plain string | -1.5860% | -0.4571% |
| `instructions` | `strmap_transcode_cursor/clean` | 25,733,864 | plain string | -0.0991% | +0.1009% |
| `instructions` | `strmap_transcode_cursor/escaped` | 39,159,117 | plain string | -0.1981% | +0.0019% |
| `instructions` | `strmap_transcode_get/clean` | 21,648,007 | plain string | -0.0646% | +0.1354% |
| `instructions` | `strmap_transcode_get/escaped` | 65,745,614 | plain string | -4.8072% | -4.6072% |
| `instructions` | `strmap_transcode_insert/clean` | 49,968,515 | plain string | -9.2559% | -8.9486% |
| `instructions` | `strmap_transcode_insert/escaped` | 99,521,198 | plain string | -7.1364% | -6.9363% |
| `instructions` | `sync_strmap_churn_short/short` | 229,771,255 | shared string | -0.1515% | +0.0485% |
| `instructions` | `sync_strmap_churn/routes` | 188,497,523 | shared string | +0.2159% | +0.4159% |
| `instructions` | `sync_strmap_compare_exchange/routes` | 123,149,234 | shared string | -0.2221% | -0.0221% |
| `instructions` | `sync_strmap_get_short/short` | 29,882,814 | shared string | -0.0991% | +0.1009% |
| `instructions` | `sync_strmap_insert_short/short` | 96,858,964 | shared string | +6.2692% | +6.4694% |
| `instructions` | `sync_strmap_insert_sorted/uuid` | 45,544,902 | shared string | +6.1904% | +6.3908% |
| `instructions` | `sync_strmap_insert/routes` | 76,895,787 | shared string | +0.3447% | +2.0898% |
| `instructions` | `sync_strmap_mem_used/routes` | 15,663,913 | mem_used fold | -62.7506% | -62.5267% |
| `instructions` | `sync_strmap_mem_used/short` | 16,838,489 | mem_used fold | -65.3564% | -65.1342% |
| `instructions` | `sync_strmap_next_after_scan_locked/paths` | 118,327,679 | shared string | -0.1005% | +0.0995% |
| `instructions` | `sync_strmap_next_after_scan_locked/paths_dense` | 134,221,039 | shared string | -0.1002% | +0.0998% |
| `instructions` | `sync_strmap_remove/routes` | 88,267,360 | shared string | -0.6357% | +0.3436% |
| `instructions` | `sync_strmap_scan_locked/paths` | 9,155,814 | shared string | -0.1000% | +0.1000% |
| `instructions` | `sync_strmap_scan_locked/paths_dense` | 13,644,631 | shared string | -0.1000% | +0.1000% |
| `instructions` | `sync_strmap_scan_pointops/paths` | 182,741,758 | shared string | -0.1007% | +0.0993% |
| `instructions` | `sync_strmap_scan_pointops/paths_dense` | 171,936,738 | shared string | -0.1001% | +0.0999% |
| `instructions` | `sync_strmap_update/routes` | 73,966,529 | shared string | -0.3034% | -0.1034% |
| `rmw_instructions` | `sync_strmap_cas_insert/routes` | 99,147,549 | shared string | +2.1796% | +4.7754% |
| `rmw_instructions` | `sync_strmap_cas/routes` | 72,628,900 | shared string | +1.1739% | +1.3739% |
| `rmw_instructions` | `sync_strmap_update_rmw/routes` | 121,739,255 | shared string | +0.6600% | +0.8600% |
| `smoke_instructions` | `strmap_churn/routes` | 22,091,995 | plain string | -0.1455% | +0.0545% |
| `smoke_instructions` | `strmap_get/routes` | 6,724,116 | plain string | -0.1007% | +0.0993% |
| `smoke_instructions` | `strmap_insert/routes` | 8,271,632 | plain string | -2.5100% | -1.4011% |

**Arms expected to lose** (the lower bound is above zero), each with its bound:

| arm | bound |
|---|---|
| `sync_strmap_insert_short/short` | [+6.2692%, +6.4694%] |
| `sync_strmap_insert_sorted/uuid` | [+6.1904%, +6.3908%] |
| `rmw_instructions` `sync_strmap_cas_insert/routes` | [+2.1796%, +4.7754%] |
| `rmw_instructions` `sync_strmap_cas/routes` | [+1.1739%, +1.3739%] |
| `rmw_instructions` `sync_strmap_update_rmw/routes` | [+0.6600%, +0.8600%] |
| `sync_strmap_insert/routes` | [+0.3447%, +2.0898%] |
| `sync_strmap_churn/routes` | [+0.2159%, +0.4159%] |

Arms whose interval straddles zero and can lose up to their upper bound are named in the table too: `strmap_oscillate/routes` up to +1.7688%, `sync_strmap_remove/routes` up to +0.3436%, `strmap_churn/routes` up to +0.1864%.

Two of these exceed the CI gate's 5% single-arm threshold (`scripts/perf_report.py`), so PR B carries an `allow-regression:` line. It names every arm above the noise floor and cites this section's commit and the PR's own `instruction-counts` run (AGENTS.md §6).

#### G-scale (reference host)

**Cells.** The `str` writer arm of `writer_scaling.py`: the merge base's build, carrying the head's harness, against the head's, in interleaved (build × W) rounds. W ∈ {1, 2, 4, 8}, 8 rounds (the Williams design over 8 cells), one process per cell, under the pin `0,2,4,6,8,10,12,14`. That is one thread per physical P-core, the pin §8.20.5.0 gives an arm where every thread works (`--compare-base-ref`, suite `writer_scaling_1586_str_vs_base`).

**Statistic.** The paired per-round C_head(W) / C_base(W), with BCa 95% intervals, at W ∈ {2, 4, 8} (`g47_scale_report`).

**Bound.** At every W in **both** of two independent runs:
- `MEETS_FLOOR` iff the lower bound ≥ **0.90**;
- `BELOW_FLOOR` iff the upper bound < 0.90 in either run;
- `INTERMEDIATE` otherwise.

The 0.90 is the 10% tolerance §19 fixed for G3, applied here to the scaling ratio. B moves shell and suffix allocation on a shared tree from glibc's per-thread caches to the collector's striped, locked freelists (finding (e)). A contention cost the instruction counts cannot see shows here as a scaling loss. A cross-run claim needs the second run (`docs/BENCHMARKING.md` rule 18).

#### G-density (deterministic byte census, exact columns; `allocator_overhead` example, both N)

**Ceilings** (`density_ceiling`). The packed build's figure, plus shells per key × what B adds per shell, plus 0.02 for the census's two-decimal rounding. For `mem_held` that is 9.0 B per shell: it is accounted at 48 bytes against the 40 the walk counts today, plus a 64-byte slab header per 84 blocks. For `+headers` it is 1.0 B per shell: one glibc chunk header per 4 KiB slab page instead of one per shell, and the page's 4,032 − 84 × 48 = 0 bytes of tail.

| N | shape | `mem_held` ceiling (main → ceiling) | `+headers` ceiling (main → ceiling) |
|---|---|---:|---:|
| 10⁶ | `prefix` | 35.73 → 49.21 | 49.06 → 49.41 |
| 10⁶ | `uuid` | 68.80 → 73.59 | 89.25 → 74.23 |
| 10⁷ | `prefix` | 40.40 → 49.21 | 47.21 → 49.53 |
| 10⁷ | `uuid` | 66.81 → 69.74 | 86.93 → 69.98 |

- **Rounding.** The table rounds each ceiling to two decimals; the gate compares the census's two-decimal figure with the unrounded `density_ceiling`.
- **The `ExpanseMap` shapes** (`sequential`, `timestamp`, `random`) equal `main` in every exact column at both N.
- **Expected losses, named:** `prefix` `mem_held` rises by up to 13.48 B/key at 10⁶ and 8.81 at 10⁷, and its `+headers` by up to 0.35 and 2.32. The suffix half is most of it, as `docs/BENCHMARKING.md` records for the feature: a class-allocated suffix of 19 or 40 bytes costs what glibc already charges.
- **`mem_used` changes meaning.** It counts every block at its accounted size, shells at 48. It is reported, not gated.
- **RSS is diagnostic only.** It is one reading per shape with no interval, and decides nothing.

#### G-safety

1. **CI, green on PR B's head:**
   - the per-PR Tier-1 Miri shards, whose string filters (`strmap::tests::deferred`, `strmap::tests::cursor_*`, `sync::miri_tests::`, and `expanse-capi`'s `test_sync_strmap_capi`) cover the shared shell paths. A new test, `strmap::tests::deferred_mem_used_reads_beside_writers`, reads `mem_used` from one thread while two write, and falls under the existing `strmap::tests::deferred` filter. Miri is never run locally;
   - `test-asan`, plus the string ASan step migrated from `packed-suffix` to `ablation-unpacked-suffix`;
   - `loom`, with a model of the shard fold (`loom_acct_fold_exact_after_join`): exact after join, and within the in-flight bytes while writers run.
2. **Every new allocation site has its shared twin.** `new_shell::<false>` is called once (`RootNode::new_plain`, the plain root), `new_shell::<true>` once (`root_or_create_shared`), and `new_shell::<SHARED>` once (`build_split_child`, which the plain and shared splits instantiate). No `Box::new`, `Box::into_raw` or `Box::from_raw` of a `StrNode` remains in `strmap.rs`'s production code. A source census pins both, and each clause is shown failing on its own (one negative control per clause, AGENTS.md §5).
3. **No `%fs:` added to plain paths** (`objdump -d -C --no-show-raw-insn` of the `instructions` bench binary, base against head):
   - the plain insert and remove bodies, in the `strmap_insert` and `strmap_oscillate` arm wrappers, and `new_shell::<false>`, carry 0 `%fs:` operands;
   - the out-of-line helpers reached on both paths (`free_shell`, `new_suffix`, `dispose_suffix`) may carry `%fs:` only on their deferred-tree branch, and their counts are reported;
   - `SyncExpanseStrMap::insert` and `remove` are reported.
4. **The local gates.** `scripts/gate.sh` in full, `scripts/test_occ_stats.sh` and `scripts/test_collector_census.sh` pass once at the end, with their counts in the PR.

### 47.4 The lock-free reading: soundness and contract

**What it reads.** `SyncExpanseStrMap` holds an `AcctCell`, taken from the map's allocator in `from_map` before any other thread can see the map. That is the handle `SyncExpanseBlobMap` holds for its index (`sync.rs:11176`, `:11298`). `mem_used` is `AcctCell::bytes_in_use`: one relaxed load of each of the 64 shards' `bytes_in_use`, summed. It forms no reference to `ExpanseStrMap`, to `Shared` or to the `NodeAlloc`. It dereferences only the boxed `Deferred` cell the `OnceLock` in `NodeAlloc::deferred` owns, and only its atomic counters (`alloc.rs:563`).

**Why the handle cannot dangle.**
- The cell is boxed, so it does not move with the map.
- It is published once by `NodeAlloc::defer_to` into a `OnceLock` that is never reset, and lives until the `NodeAlloc` drops.
- The `NodeAlloc` is a field of the one `ExpanseStrMap` that `SyncExpanseStrMap` owns through `Shared`, and nothing replaces that map: the string wrapper has no `compact()` tree swap (there is no `SyncExpanseStrMap::compact`); `clear` disposes of entries through `clear_shared` and keeps the allocator (`strmap.rs:4862`); there is no `into_inner`; and `from_map` is the only constructor (`sync.rs:13930`).
- `&self` on the wrapper keeps the map alive for the reading.
- A debug assertion in `SyncExpanseStrMap::mem_held`'s locked section checks that the handle still names the allocator's cell, as `SyncExpanseBlobMap::mem_used` does.

**Why no write is a data race with it, and why no borrow is invalidated.** Writers touch the shards only through atomic RMWs (`Deferred::charge` / `discharge`, `alloc.rs:447`, `:467`), through a shared reference to the cell. A writer's `&mut ExpanseStrMap`, on the exclusive path, covers the map's own bytes, the `OnceLock`'s pointer among them, but not the boxed cell behind it. No code takes `&mut Deferred`. This is the access pattern `SyncExpanseBlobMap::mem_used_estimate` already ships (#1478).

**Contract, stated as happens-before (finding (d)).**
- **Exact when quiet.** If every insert, removal and `clear` on the map happens-before the call, and none is concurrent with it, the call returns exactly `ExpanseStrMap::mem_used` of the map at that point. The writers may have been joined, or have released a lock or an atomic the caller acquired. Each shard is written only by RMWs, so by write-read coherence each relaxed load returns that shard's last value in modification order once every write to it happens-before the load.
- **Error while writers run.** The loads are not one snapshot. The result can differ from the map's bytes when the call began by at most the bytes allocated or freed by operations that overlap the call, and it can be below the true figure. It is not an upper bound.
- **Smooth.** No walk and no cache: two readings differ by what the operations between them allocated and freed.

The caller asked for ±10%, no upper bound, smooth, and no stall. G-accuracy tests the first three and G-stall the fourth.

**The clamp (finding (c)).**
- `NodeAlloc::bytes_in_use`, the exact reading every quiesced and single-threaded caller uses, gains `debug_assert!(total >= 0)`. On those paths a negative sum is an over-free (AGENTS.md §8.22.3).
- `AcctCell::bytes_in_use` keeps the clamp without the assertion. While writers run, a load can see a free on a later shard and miss its allocation on an earlier one. On a map whose live bytes are below the bytes in flight, that sum is negative with no defect present, so an assertion there would fail without a fault.
- The over-free signal is kept by G-accuracy 1: after join, it asserts the signed sum is ≥ 0 and equals the exact figure.

### 47.5 Falsifiers, and what follows each

| Falsifier | What follows |
|---|---|
| G-instr: an arm outside its bound, at any increment | `callgrind_annotate --inclusive=no` and `--auto=yes`, base against head, on the arm, and `objdump` of every function whose count moved without a source change (AGENTS.md §6). Then the code changes and is re-measured against the same bound, or the departure is recorded here as a dated note. A note that accepts a departure says the arm ships outside its pre-registered bound and why; the bound itself does not move. |
| G-accuracy: any clause fails | the change does not ship; the reading is wrong, and it is fixed before anything is measured on the reference host |
| G-density: a ceiling exceeded, or an `ExpanseMap` column moves | attributed from the census's per-request rows; a class-size change is a new design decision, not a fix |
| G-safety: Miri, ASan or loom red; a source-census clause unpinned; a plain path gains `%fs:` | fixed before PR B is marked ready |
| G-stall `FAIL` | the lock-free reading still stalls writers; the cause is found (`occ-stats` counters, as §44.3's pass) before any re-run, and #1586 stays open |
| G-stall `INTERMEDIATE` | reported to the maintainer; no re-run with a changed k |
| G-scale `BELOW_FLOOR` | the collector freelists contend; B does not merge as is, and the remedy (a freelist change, or keeping shells off the collector) is a new pre-registration |
| G-scale `INTERMEDIATE` | reported to the maintainer |

No bound in this section moves after the first measurement of a B build (AGENTS.md §8.19).

### 47.6 Silent constants, fixed here

G-stall: k = 3, the cells (N = 2·10⁷, W = 4, T = 1 s, D = 30 s, both shapes, growth and churn), 5 rounds, BCa with 2,000 resamples, §44's pin and placement, the churn void bound W. G-accuracy: the 10% band, the 2% step allowance, 25% growth per phase, W = 4. G-instr: the per-call ranges ([25, 85], [15, 45], [150, 340], [−65, +45], [64, 1,000]), the 0.1% slack with its 500 floor, the 0.01% non-string bound. G-scale: the 0.90 floor, two runs, 8 rounds, W ∈ {1, 2, 4, 8}, the pin `0,2,4,6,8,10,12,14`. G-density: 9.0 and 1.0 B per shell, 0.02 slack. Changing any of them after a measurement of B relabels that result `INTERMEDIATE` (AGENTS.md §8.19).

### 47.7 Not predicted

- Wall-clock cost on the write paths. G-scale reads scaling, not single-writer speed, and nothing here bounds T(1).
- The largest insert latency's level in the B build, beyond its ratio to the `len` control.
- Whether `new_shell`, out of line, changes register allocation in the insert walks beyond the call. The per-call ranges carry it, and only the measurement says.
- RSS.

### 47.8 Dispatch, for the maintainer

Neither reference-host gate is dispatched by the change's author; both need the maintainer's go-ahead. With `HEAD` the PR B head (40 characters) and `BASE` its merge base with `main`:

```bash
# G-stall: 60 processes; run_seconds_bound(**DISPATCH_47) = 7,122 s, inside the default job timeout.
# Operating rules of §44.10: dispatch at :19, apt-daily timers stopped, no login to the host.
gh workflow run bench_baremetal.yml -f benchmark_suite=strmap_poll_stall_g47 -f ref="$HEAD"

# G-scale: two independent runs, each its own dispatch.
gh workflow run bench_baremetal.yml -f benchmark_suite=writer_scaling_1586_str_vs_base \
  -f ref="$HEAD" -f base_ref="$BASE" -f cpu_pin=0,2,4,6,8,10,12,14
```

### 47.9 The deterministic gates on PR B (appended 2026-10-23; §47.1 to §47.8 are not rewritten, and no bound moves)

PR B measured the three increments of §47.1 on the §47.2 instrument (same host, container and toolchain), one build per increment:
- I1 at `7abe1b01`;
- I2 at `3fcbf8da`;
- I3 at `dc178ddb`. The commit after it changes only tests, docs and code compiled under `ablation-unpacked-suffix` alone.

Artifact: `results/strmap_design_b_callgrind_dc178ddb.json` (every arm of the three builds, with §47.3's bound and whether I3 lies inside it). `main` had moved to `273871a6` (#1598) by then. #1598 changed no file under `crates/expanse/src` and none of the Callgrind harnesses, so §47.2's counts are `main`'s.

**G-instr: not met.** 354 of 372 arms lie inside their bounds at I3, and 18 do not. Inside:
- every shared string arm, including all seven named losers: `sync_strmap_insert_short/short` +6.3692%, `sync_strmap_insert_sorted/uuid` +6.2904%, `sync_strmap_insert/routes` +1.9842%, `rmw_instructions` `sync_strmap_cas_insert/routes` +2.3806%, `sync_strmap_cas/routes` +1.2739%, `sync_strmap_update_rmw/routes` +0.7600%, `sync_strmap_churn/routes` +0.3159%;
- `sync_strmap_mem_used`: 15,663,913 → 5,851,763 (−62.6418%) on `routes` and 16,838,489 → 5,852,065 (−65.2459%) on `short`;
- 308 of the 311 non-string arms, which are identical to `main`.

The 18 outside, Δ against `main` at I1 / I2 / I3, with the bound, and the attribution (`callgrind_annotate`-equivalent per-function self Ir from each arm's profile, base against head; per-instruction counts and `objdump -d -C` where a function's source did not change):

| arm | I1 | I2 | I3 | bound | direction |
|---|---:|---:|---:|---|---|
| `strmap_churn/routes` | +0.5617% | +1.1665% | +1.1665% | [−0.0136%, +0.1864%] | worse |
| `smoke_instructions` `strmap_churn/routes` | +0.6797% | +0.9511% | +0.9511% | [−0.1455%, +0.0545%] | worse |
| `strmap_insert/routes`, `strmap_insert_plain/routes` | −1.1929% | −0.6113% | −0.6113% | [−2.5490%, −1.4684%] | smaller saving |
| `smoke_instructions` `strmap_insert/routes` | −1.2225% | −0.5797% | −0.5797% | [−2.5100%, −1.4011%] | smaller saving |
| `capi smoke_instructions` `judysl_insert/routes` | −1.4002% | −1.5392% | −1.5392% | [−2.9792%, −1.9433%] | smaller saving |
| `strmap_raw_insert/clean` | −0.0725% | −9.4887% | −9.4887% | [−10.0247%, −9.7072%] | smaller saving |
| `strmap_transcode_insert/clean`, `/escaped` | −0.0633%, +0.0512% | −8.8586%, −6.8860% | same as I2 | [−9.2559%, −8.9486%], [−7.1364%, −6.9363%] | smaller saving |
| `strmap_oscillate/routes` | −11.1164% | −13.2488% | −13.2488% | [−12.4309%, +1.7688%] | larger saving |
| `strmap_refill/routes` | −1.8243% | −2.1582% | −2.1582% | [−1.5860%, −0.4571%] | larger saving |
| `sync_strmap_remove/routes` | −1.1782% | −1.1743% | −1.1743% | [−0.6357%, +0.3436%] | larger saving |
| `strmap_prefix_bounded`, `_scan`, `_seek` `/paths_dense` | 0 | −0.4055%, −0.3860%, −9.4905% | same | ±0.1%, ±0.1%, ±0.3110% | larger saving |
| `smoke_instructions` `bytesmap_insert/routes`, `bytesmap_churn/routes`; `rmw_instructions` `sync_bytesmap_cas_remove/routes` | +0.1121%, +0.0370%, −0.0178% | same | same | ±0.01% | non-string |

- **The plain insert and churn paths: register allocation in the inlined bodies.**
  - On `strmap_churn/routes`, which allocates no shell, the whole +1,250,151 instructions against `main` under `packed-suffix` (the suffix half alone, +99,956) is in the arm wrapper the insert and remove bodies are inlined into: +1,250,000.
  - Per instruction, a block executed 200,000 times, the per-chunk descent, gains 4 instructions: the key pointer and length are reloaded from the stack (`mov 0x28(%rsp)`, `mov 0x20(%rsp)`) where `main` kept them in registers. A block executed once per operation gains 11.
  - On `strmap_insert/routes` the shell half itself is inside its range: at I1, `new_shell::<false>` costs 78.9 instructions per call (493,518 over 6,252 calls), inside [25, 85], against 189.9 for the `malloc` it replaces. But the wrapper body gains 464,750 against the packed build of `main`, 9.3 per insert, from the same register allocation drift.
  - `split_suffix` grew from 765 to 1,671 instructions (1,244 under `packed-suffix` on `main`).
  - The source change behind it is the root slot's type (`Option<RootNode>` for `Option<Box<StrNode>>`) and the out-of-line `new_shell` calls in the inlined bodies.
- **One remedy was tried and reverted:** creating the plain root in a `#[cold] #[inline(never)]` function. It left `strmap_churn` at +1.1665% and moved `strmap_insert` from −0.6113% to −0.5012% and `strmap_oscillate` from −13.2488% to −11.8488%, so it was reverted (AGENTS.md §6: a flat or negative result is reverted and written down).
- **The larger savings** (`strmap_oscillate`, `strmap_refill`, `sync_strmap_remove`) are the derived per-call ranges' lower ends set too high, plus a saving the derivation left out. At I1, a warm-freelist `new_shell` costs 20.1 (`strmap_oscillate`) and 10.0 (`strmap_refill`) instructions per call, under the 25 the range assumed. Taking 40-byte chunks out of glibc's heap removes `malloc_consolidate` and `_int_free_chunk` work on the arm's other allocations: −325,667 and −276,591 on `sync_strmap_remove`. The `paths_dense` prefix arms are the same glibc effect on the arms' own key buffers: −13,120 in `_int_malloc` on `strmap_prefix_seek`.
- **The non-string arms:** `ExpanseBytesMap::insert`, whose source did not change, grew from 2,091 to 2,113 instructions in the `smoke_instructions` binary, an inlining change, +2 executed per call at I1. `sync_bytesmap_cas_remove`'s −34,946 is in `Shared::<ExpanseBytesMap>`'s writer body, not further attributed.

**What follows (§47.5, first row).** The code changed once and was re-measured against the same bounds, and it did not recover. The bounds stand.
- The four plain arms that save less than bounded and the two churn arms that lose (`strmap_churn` +1.1665%, the smoke twin +0.9511%) are departures recorded here, not met.
- The arms that save more than bounded are reported, and do not by themselves block the change.
- Whether PR B ships over the two plain churn losses is the maintainer's decision. The bound does not move to meet them.

**G-accuracy: clauses 1, 3, 4 and the `layout_census` half of 2 are met; the `stats()` half of clause 2 and of clause 1 is a pre-registration defect.**
- Met, with clause 3's churn phase included:
  - `strmap::tests::deferred_mem_used_exact_after_join` (W = 4: inserts, removals and a clear; the lock-free reading equals the exact one and the shards' signed sum);
  - `strmap::tests::mem_used_band_and_step_while_writers_run` (six phases, every reading within the band and the step bound);
  - `census::tests` with `layout-census`;
  - `stats_breakdown_sums_to_mem_used`.
- §47.3 also wrote that `stats().node_bytes.total()` equals `mem_used` after concurrent writers are joined. On `main` it does not: a `SyncExpanseStrMap` filled by 4 writers with 4,000 keys each reads `stats` 439,316 against `mem_used` 732,820 at `f65060d8`, a defect of `stats()` on a map optimistic writers have written, outside design B. That half of the clause was never true on the code it was written against. The tests read the exact figure and `layout_census` instead, and the `stats()` defect is a separate issue.

**G-density: met.** At I3 (`results/allocator_overhead_dc178ddb.txt`), every exact column is within its §47.3 ceiling, and the `ExpanseMap` shapes equal `main` in every exact column:
- `mem_held`, `prefix` / `uuid`: 49.17 / 73.57 at 10⁶ and 49.12 / 69.72 at 10⁷;
- `+headers`: 49.36 / 74.21 and 49.46 / 69.97.

Against `main`, `prefix` `mem_held` rose 13.44 and 8.72 B/key and `+headers` 0.30 and 2.25, inside the named loss.

**G-safety, local part: met; the CI part is read on the PR.**
- The source census `strmap::tests::str_node_shells_are_node_alloc_blocks` passes, and each of its clauses fails on its own in the test itself.
- The loom model `occ::loom_tests::loom_acct_fold_exact_after_join` passes.
- `objdump` of the `instructions` binary at I3, instructions and `%fs:` operands:
  - the `strmap_insert`, `strmap_oscillate` and `strmap_churn` arm wrappers (the plain bodies, inlined): 1,387 / 3,615 / 4,860, each 0 `%fs:` (`main`: 1,419 / 3,643 / 4,867, 0);
  - `new_shell::<false>` 75, 0 `%fs:`;
  - `new_shell::<true>` 113, 1;
  - `free_shell` 50, 1, on its deferred branch;
  - `new_suffix` 173, 1 and `dispose_suffix` 89, 1, as `main` under `packed-suffix`;
  - `SyncExpanseStrMap::insert` 743, 33 and `remove` 4,341, 34 (`main`: 743, 33 and 4,338, 34).

### 47.10 The reference-host gates, the instrument gap and the outcome (appended 2026-10-24; §47.1 to §47.9 are not rewritten, and no bound moves)

All runs below were admitted by the host-activity guard under §44.10's rules: dispatched at :19 inside 18:19–12:19 UTC, `apt-daily` timers stopped, no login to the host during a run. Intervals are BCa 95% unless marked otherwise.

**G-stall: PASS.**
- Run [40002326686](https://github.com/orieg/expanse/actions/runs/40002326686) at `3cbe77d6`; artifact `results/gate_1586_strmap_poll_stall.json`; worst foreign load 0.15 core-equivalents.
- Results by cell:

  | cell (N = 2·10⁷, W = 4, T = 1 s) | `mem_used` arm, largest insert | bound (3 × `len` upper) | verdict |
  |---|---|---|---|
  | `routes` growth | [114,610, 165,170] ns | 4,642,480 ns | PASS |
  | `short` growth | [105,151, 145,573] ns | 417,559 ns | PASS |
  | `routes` churn | [24,897,125, 45,232,946] ns | 99,827,709 ns | PASS |
  | `short` churn | [125,750,476, 29,943,975,116] ns | 89,902,389,655 ns | PASS |

- **Only the growth cells carry evidence.** In both churn cells the `len` control itself holds the long stall described next, so the ratio rule cannot detect a `mem_used` stall there (k × control ≈ 90 s on `short`). This reading of the churn cells was not stated in §47.3; it is recorded here and changes no verdict.
- For comparison on `main`, the same cells at the base `6dc24a74` were run once (run [40057817142](https://github.com/orieg/expanse/actions/runs/40057817142); artifact `results/gate_1586_strmap_poll_stall_base_6dc24a74.json`). Reported, not gated:

  | cell | `main` `mem_used` largest insert, per round | design B |
  |---|---|---|
  | `routes` growth | 0.501–0.545 s | 0.11–0.17 ms |
  | `short` growth | 1.012–2.002 s | 0.11–0.15 ms |

**The churn stall predates design B.** The same base run shows, on `main`:
- `short` churn: largest insert 29.31–30.09 s in **all 5** rounds of the no-poller arm, with the writers' CPU share at 0.26. Design B's run showed 29.9 s in 2 of 5 rounds of that arm and a share of 0.25–0.28.
- `routes` churn: 25–37 ms on both builds.

So a single insert on a constant-population `short`-key map can wait for about the whole 30 s window with no poller at all, on both builds. Design B neither introduced nor removed it. The low CPU share points to writers parked rather than spinning. The cause is unmeasured; it is tracked separately, outside §47.

**G-scale: MEETS_FLOOR in both runs.** The statistic is C_head(W)/C_base(W):

| W | run [40039010963](https://github.com/orieg/expanse/actions/runs/40039010963) | run [40044397191](https://github.com/orieg/expanse/actions/runs/40044397191) |
|---|---|---|
| 2 | [1.0110, 1.0235] | [0.9997, 1.0231] |
| 4 | [0.9847, 1.0401] | [0.9723, 1.0646] |
| 8 | [1.0099, 1.0424] | [1.0038, 1.0470] |

- Artifacts: `results/gate_1586_str_writer_scaling_run{1,2}.json`.
- C(W) = T(W)/T(1) within each build, so a uniform single-writer loss cancels out of this gate.
- The absolute throughput ratio T_head(W)/T_base(W), paired per round, n = 8, was not pre-registered and is reported here only:

  | W | run 1 | run 2 | base → head median (M inserts/s) |
  |---|---|---|---|
  | 1 | 0.9321 [0.9284, 0.9341] | 0.9306 [0.9250, 0.9354] | 3.57 → 3.33 |
  | 2 | 0.9486 [0.9438, 0.9540] | 0.9441 [0.9344, 0.9496] | 6.02 → 5.68 |
  | 4 | 0.9427 [0.9180, 0.9705] | 0.9504 [0.9085, 0.9857] | 10.52 → 9.98 |
  | 8 | 0.9545 [0.9414, 0.9684] | 0.9564 [0.9350, 0.9742] | 19.89 → 18.67 |

- So design B costs about 7% of shared string-insert throughput at one writer and about 4.5% at eight. Both runs agree.

**G-instr: not met, on both instruments.**
- On the registered instrument (§47.9, the §47.2 container), 18 arms lie outside their bounds.
- On CI's `instruction-counts` job (ubuntu-24.04), which `perf_report.py` gates on:

  | arm | CI | registered instrument | §47.3 bound |
  |---|---:|---:|---|
  | `sync_strmap_insert_short/short` | +8.79% | +6.37% | [+6.27%, +6.47%] |
  | `sync_strmap_insert_sorted/uuid` | +5.62% | +6.29% | [+6.19%, +6.39%] |
  | `sync_strmap_insert/routes` | +1.66% | +1.98% | [+0.34%, +2.09%] |
  | `sync_strmap_cas_insert/routes` | +2.18% | +2.38% | [+2.18%, +4.78%] |
  | `sync_strmap_cas/routes` | +1.27% | +1.27% | [+1.17%, +1.37%] |
  | `sync_strmap_update_rmw/routes` | +0.76% | +0.76% | [+0.66%, +0.86%] |
  | `strmap_churn/routes` | +1.15% | +1.17% | up to +0.1864% |

- `strmap_churn` was named in §47.3 with that upper bound and exceeds it about 6×. It is a single-threaded plain-tree cost with no gain on its arm, and §47.9 attributes it to register drift in the inlined insert and remove bodies.

**The CI gap is libc, not engine code (measured).** The same base and head binaries, built at `6dc24a74` and `2fef468b` and run under valgrind 3.27.1, reproduce both instruments:
- under glibc 2.39 (`ubuntu:24.04`, CI's runner image): CI's counts to within 0.003%;
- under glibc 2.41 (`debian:trixie`): the registered instrument's percentages.

On `sync_strmap_insert_short` the engine functions move identically under both:
- `new_suffix` +3,499,750 Ir
- `Collector::pop_freelist` +2,400,088 Ir
- `olc_insert` −1,005,493 Ir
- total about +4.89M (+5.0%)

The libc share is +1.27M on 2.41 and +3.72M on 2.39. Its source is design B replacing `malloc` with zeroed `calloc` for shells and suffixes; on 2.39 that `calloc` takes a longer path through one unresolved function, identified as `_int_malloc` by size and position only. §47.3 derived its shared-allocation range on one glibc and fixed no C library among its silent constants (§47.6). That is a pre-registration defect, recorded here; the bounds do not move.

**G-accuracy:** clauses 1, 3 and 4 and the `layout_census` half of clause 2 are met. The `stats()` half is **NOT_EVALUABLE**: §47.3 assumed `stats()` sums to `mem_used` after concurrent writers, and it does not on `main` either (§47.9). It is tracked separately.

**G-density, G-safety:** met (§47.9). CI's Miri, ASan (default and `ablation-unpacked-suffix` passes), loom, Windows and ARM lanes pass.

**Outcome.**
- Every gate is met except G-instr. That includes G-stall, the gain axis, in the growth cells that can show it.
- Under §47.5's first row, a G-instr miss is attributed (above) and the change waits for the maintainer's decision. No override is entered for it.
- What the decision weighs:
  - **Gain:** a `mem_used` reading no longer holds writers. The largest insert falls from 0.5–2 s to about 0.15 ms at 2·10⁷ keys, on the long-lived growing map the caller described.
  - **Cost:**
    - about 7% (one writer) to 4.5% (eight) of shared string-insert throughput;
    - +1.15% instructions on plain `strmap_churn`;
    - the glibc-dependent `calloc` share on every shared shell and suffix allocation.
- The likely first lever on the cost is allocating shells, which are fully written, without zeroing. It is unmeasured.

### 47.11 Pre-registration for one increment on design B: shells and suffix leaves without zeroing, and one zeroing per deferred freelist hit (appended 2026-10-24, locked before any measurement of a build containing the increment; §47.1 to §47.10 are not rewritten, and no bound moves)

§47.10 names allocating shells without zeroing as the likely first lever on design B's cost, and its instrument gap is design B's zeroed `calloc` for shells and suffixes. This section pre-registers one increment on PR B's head `a37a044e`, measured against that head, not against `main`. It holds no data of the increment.

#### The change

- **Shells.** `new_shell::<SHARED>` takes its block from a non-zeroing path of `NodeAlloc` and initialises it with its one `ptr::write` of `StrNode::new()`. That write already covers every word a reader reads: the cover, the dirty flag, the root's tag and the population. An empty root's two payload words stay unwritten, as they are on any `MapCore::new()`, and every reader of them checks the tag first (`map::root_word`). `new_shell` stays out of line (AGENTS.md §2.1 invariant 5).
- **Suffix leaves**, taken only with an audit of the unwritten tail. The audit, against `a37a044e`: `new_suffix` writes the 16-byte header and the key's `len` bytes; the block is `raw_class_fit(16 + len)` bytes, so a tail of up to the class step stays unwritten. Every read of a suffix's bytes in `strmap.rs` goes through `suffix_bytes`, a slice of exactly `len` bytes, and every other read is of `value` or `len`. No other module reads a suffix leaf. So nothing reads the tail, and `new_suffix` takes the non-zeroing path too. The `StrSuffix` type docs carry the invariant.
- **One zeroing per deferred freelist hit.** `Collector::pop_freelist` no longer zeroes the class size it pops. `NodeAlloc::alloc_raw` still zeroes the `bytes` it hands out, so a hit is zeroed once instead of twice, and not at all on the non-zeroing path.
- **The zeroed contract is unchanged for every other caller.** `alloc_bytes`, `alloc_bytes_plain`, `alloc_bytes_dispatch` and the `alloc_node` family keep returning zeroed blocks. The non-zeroing path is a separate entry point (`alloc_bytes_uninit`, `alloc_bytes_uninit_dispatch`) whose only callers are `new_shell` and `new_suffix`. `pop_freelist`'s one production caller is `alloc_raw`. Slab pages stay `alloc_zeroed`.
- **Tests.**
  - `strmap::tests::shell_is_fully_written_over_a_stale_block` and `suffix_is_fully_written_over_a_stale_block` reuse a poisoned block through a plain tree's freelist. Their controls show the block reads stale before the call, and the shell test poisons each word in turn and expects it reported alone.
  - Under Miri every allocation is exact-size, so a shell or suffix block that does not come from a freelist is uninitialised, and any read of a byte the write did not cover is reported. `strmap::tests::deferred_shell_on_an_uninit_block_is_fully_written` falls under the Tier-1 `strmap::tests::deferred` filter. Miri is not run locally.

#### Instruments and base

Callgrind, two instruments that differ only in the C library, both on the x86_64 heavy-jobs host (the §47.2 host) with `--security-opt seccomp=unconfined`:
- **glibc 2.41**: `rust:1.99.1-trixie@sha256:931c2c05…`, libc6 `2.41-12+deb13u4`;
- **glibc 2.39**: `ubuntu:24.04@sha256:224a1869…` (CI's runner release), libc6 `2.39-0ubuntu8.9`, rustup 1.99.1.

Both use rustc 1.99.1 (LLVM 23.1.1), valgrind 3.27.1 built from the pinned tarball the way `.github/actions/valgrind` builds it, and `iai-callgrind-runner` 0.16.1. The harnesses are `instructions` and `rmw_instructions` in full, 286 arms per instrument. Base `a37a044e`; head the increment's commit.

#### What was measured at the base (before this lock)

Artifact: `results/strmap_zeroing_baseline_callgrind_a37a044e.json`, every arm's count on both instruments with the call records below.
- **Call records.** For each arm: `pop_freelist`'s `memset` calls and their inclusive Ir; `new_suffix`'s `memset` calls, its slab pages (`posix_memalign` calls) and its `calloc` calls; `new_shell::<true>`'s freelist pops and `calloc` calls; `new_shell::<false>` calls. A `calls=` count includes calls made outside the arm's measured region whenever the same arc also ran inside it, while the cost is the region's alone. The bound reads a count as exact only where its cost per call is plausible.
- **Disassembly of `a37a044e`** (`objdump -d -C`): `new_shell::<true>` zeroes a freelist block with 4 instructions before its write. In `new_shell::<false>` the zeroing and the write merge into 4 stores. `pop_freelist`'s zeroing is a tail call to `memset` with 4 instructions of setup.
- **One slab page's `memset`** is 4,114 Ir under 2.41 and 4,113 under 2.39 (`new_shell::<false>`'s 75 page `memset` calls in `strmap_insert/routes`). A page stays zeroed, so this share of `new_suffix`'s `memset` cost stays.
- **`calloc(40)` against `malloc(40)`** per call, 50,000 calls each, one process per (function, heap state), with the C source in the artifact:

  | heap state | 2.41 `calloc` | 2.41 `malloc` | 2.41 difference | 2.39 `calloc` | 2.39 `malloc` | 2.39 difference |
  |---|---:|---:|---:|---:|---:|---:|
  | growth (no frees) | 240.06 | 196.05 | 44.01 | 229.05 | 198.05 | 31.00 |
  | mixed (a 24-byte free per step) | 302.15 | 258.13 | 44.02 | 302.26 | 271.26 | 31.00 |
  | same (a 40-byte free per step) | 71.01 | 47.00 | 24.01 | 163.01 | 53.00 | 110.01 |

  In the arms, a suffix's `calloc` costs 98.8 (2.41) and 153.0 (2.39) per call in `sync_strmap_insert_short`, and 245.3 and 248.1 in `sync_strmap_insert_sorted`. No single heap state stands for an arm, so the bound below spans them.

#### G-instr for the increment (Callgrind; head against `a37a044e`, per arm, two-sided, on both instruments)

Every function is in `scripts/strmap_stall_bounds.py` with pinned tests; `python3 scripts/strmap_stall_bounds.py --design-b-zeroing` prints the table from the artifact.

**Derivation** (`zeroing_term`, `zeroing_bound`). The saving on an arm is the sum of five terms:
- **The `memset` calls the change removes**, exactly. These are `pop_freelist`'s, and `new_suffix`'s less its slab pages at the measured page cost. When the page arc is not fully collected, between none and as many pages as the cost holds stay.
- **Each removed call's setup**, [0, 6] Ir. There are at most the removed cost over 8 such calls. 8 Ir is below one `memset` call's cost wherever an arc's calls all fall in the measured region (the smallest such figure is 10.0, `new_suffix`'s 50 calls in `sync_strmap_insert_short` under 2.41); an arc with a smaller figure carries calls from outside the region.
- **A shared shell served from a freelist** loses its inline zeroing, [0, 6] Ir.
- **A plain shell** loses its merged store, [0, 2] Ir.
- **Each `calloc` of a shell or suffix becomes a `malloc`.** On an arc whose cost is at least 60 Ir per call, read as fully collected, it saves at least the smallest measured difference per call (24.0 under 2.41, 31.0 under 2.39). It saves at most its measured cost less the cheapest measured `malloc` per call (47.0, 53.0). On any other arc it saves between nothing and its whole cost.

The bound is minus that saving, widened on both sides by 0.1% of the arm (AGENTS.md §6 review threshold) with a floor of 500 Ir, as §47.3's slack.

**No change.** Every arm with no term (240 of 286 under each C library, including every plain arm that allocates no shell and no suffix) is bounded at ±max(0.1%, 500 Ir).

**Arms expected to improve**, both instruments (low and high Δ against `a37a044e`):

| C library | harness | arm | a37a044e | bound low | bound high |
|---|---|---|---:|---:|---:|
| glibc-2.41 | `instructions` | `domain_dict_insert/routes` | 87,543,666 | -0.2498% | +0.0286% |
| glibc-2.41 | `instructions` | `ordered_bytesmap_insert/clean` | 46,159,029 | -4.6202% | -1.0025% |
| glibc-2.41 | `instructions` | `ordered_bytesmap_insert/escaped` | 63,455,939 | -3.4239% | -0.6880% |
| glibc-2.41 | `instructions` | `strmap_clear_refill/routes` | 51,832,766 | -0.4070% | +0.0032% |
| glibc-2.41 | `instructions` | `strmap_insert/routes` | 42,348,262 | -0.4098% | -0.0476% |
| glibc-2.41 | `instructions` | `strmap_insert_plain/routes` | 42,348,262 | -0.4098% | -0.0476% |
| glibc-2.41 | `instructions` | `strmap_oscillate/routes` | 27,884,795 | -3.9214% | -1.8724% |
| glibc-2.41 | `instructions` | `strmap_raw_insert/clean` | 41,288,529 | -5.1534% | -1.1326% |
| glibc-2.41 | `instructions` | `strmap_refill/routes` | 98,783,461 | -0.2517% | +0.0492% |
| glibc-2.41 | `instructions` | `strmap_refill_small/routes` | 84,221,275 | -0.2488% | +0.0239% |
| glibc-2.41 | `instructions` | `strmap_transcode_insert/clean` | 45,542,017 | -4.6814% | -1.0175% |
| glibc-2.41 | `instructions` | `strmap_transcode_insert/escaped` | 92,668,188 | -2.3761% | -0.4396% |
| glibc-2.41 | `instructions` | `sync_blobmap_churn/random` | 135,499,840 | -0.8314% | -0.3180% |
| glibc-2.41 | `instructions` | `sync_blobmap_insert/random` | 55,988,491 | -0.4922% | -0.1241% |
| glibc-2.41 | `instructions` | `sync_blobmap_reclaim_free/random` | 256,027 | -0.3252% | +0.1211% |
| glibc-2.41 | `instructions` | `sync_blobmap_remove/random` | 65,053,263 | -0.7505% | -0.2717% |
| glibc-2.41 | `instructions` | `sync_bytesmap_churn/routes` | 297,603,609 | -0.4323% | -0.0899% |
| glibc-2.41 | `instructions` | `sync_bytesmap_insert/routes` | 111,792,788 | -0.2958% | -0.0119% |
| glibc-2.41 | `instructions` | `sync_bytesmap_remove/routes` | 182,165,612 | -0.3330% | -0.0331% |
| glibc-2.41 | `instructions` | `sync_map_branchu_band/top` | 9,607,214 | -0.2694% | +0.0032% |
| glibc-2.41 | `instructions` | `sync_map_churn/random` | 119,043,770 | -1.0664% | -0.4522% |
| glibc-2.41 | `instructions` | `sync_map_compact_drained/random60` | 20,541,618 | -0.4703% | -0.1116% |
| glibc-2.41 | `instructions` | `sync_map_count_after_write/one_top_byte` | 1,709,867,155 | -0.1007% | +0.0996% |
| glibc-2.41 | `instructions` | `sync_map_count_after_write/random` | 32,036,998 | -0.1364% | +0.0792% |
| glibc-2.41 | `instructions` | `sync_map_count_after_write/sequential` | 14,452,770 | -0.1202% | +0.0884% |
| glibc-2.41 | `instructions` | `sync_map_drain_floor/top` | 240,055 | -0.3935% | +0.1025% |
| glibc-2.41 | `instructions` | `sync_map_insert/random` | 48,913,392 | -0.5490% | -0.1566% |
| glibc-2.41 | `instructions` | `sync_map_remove/random` | 62,718,249 | -0.7747% | -0.2856% |
| glibc-2.41 | `instructions` | `sync_map_write_twin/one_top_byte` | 2,057,074 | -0.6664% | -0.2237% |
| glibc-2.41 | `instructions` | `sync_map_write_twin/random` | 1,825,039 | -0.7384% | -0.2648% |
| glibc-2.41 | `instructions` | `sync_map_write_twin/sequential` | 2,130,198 | -0.2373% | +0.0216% |
| glibc-2.41 | `instructions` | `sync_set_churn/leaf` | 86,663,509 | -2.6231% | -1.3418% |
| glibc-2.41 | `instructions` | `sync_set_churn/random` | 93,261,435 | -0.5028% | -0.1302% |
| glibc-2.41 | `instructions` | `sync_set_compact_drained/random60` | 18,985,417 | -0.8655% | -0.3375% |
| glibc-2.41 | `instructions` | `sync_set_drain_floor/top` | 239,038 | -0.4171% | +0.0904% |
| glibc-2.41 | `instructions` | `sync_set_insert/random` | 45,039,439 | -0.4774% | -0.1157% |
| glibc-2.41 | `instructions` | `sync_set_remove/random` | 53,136,483 | -0.5627% | -0.1644% |
| glibc-2.41 | `instructions` | `sync_strmap_churn_short/short` | 229,652,955 | -1.0667% | -0.4511% |
| glibc-2.41 | `instructions` | `sync_strmap_insert/routes` | 78,421,550 | -2.6313% | -0.5709% |
| glibc-2.41 | `instructions` | `sync_strmap_insert_short/short` | 103,028,140 | -2.7962% | -1.1703% |
| glibc-2.41 | `instructions` | `sync_strmap_insert_sorted/uuid` | 48,409,841 | -8.6765% | -1.1108% |
| glibc-2.41 | `instructions` | `sync_strmap_remove/routes` | 87,230,853 | -0.3303% | -0.0316% |
| glibc-2.41 | `rmw_instructions` | `sync_bytesmap_cas_insert/routes` | 107,504,183 | -0.2893% | -0.0082% |
| glibc-2.41 | `rmw_instructions` | `sync_bytesmap_cas_remove/routes` | 195,961,904 | -0.3166% | -0.0238% |
| glibc-2.41 | `rmw_instructions` | `sync_map_cas_insert/random` | 46,722,841 | -0.5341% | -0.1480% |
| glibc-2.41 | `rmw_instructions` | `sync_strmap_cas_insert/routes` | 101,507,889 | -1.5797% | -0.4454% |
| glibc-2.39 | `instructions` | `domain_dict_insert/routes` | 88,104,489 | -0.2613% | +0.0219% |
| glibc-2.39 | `instructions` | `ordered_bytesmap_insert/clean` | 46,483,897 | -4.7796% | -1.1043% |
| glibc-2.39 | `instructions` | `ordered_bytesmap_insert/escaped` | 65,398,789 | -3.4585% | -0.7410% |
| glibc-2.39 | `instructions` | `strmap_clear_refill/routes` | 52,142,720 | -0.4261% | -0.0082% |
| glibc-2.39 | `instructions` | `strmap_insert/routes` | 42,602,403 | -0.4336% | -0.0614% |
| glibc-2.39 | `instructions` | `strmap_insert_plain/routes` | 42,602,403 | -0.4336% | -0.0614% |
| glibc-2.39 | `instructions` | `strmap_oscillate/routes` | 27,985,161 | -4.2203% | -2.0440% |
| glibc-2.39 | `instructions` | `strmap_raw_insert/clean` | 41,613,234 | -5.3273% | -1.2453% |
| glibc-2.39 | `instructions` | `strmap_refill/routes` | 100,804,387 | -0.2595% | +0.0440% |
| glibc-2.39 | `instructions` | `strmap_refill_small/routes` | 86,532,659 | -0.2578% | +0.0185% |
| glibc-2.39 | `instructions` | `strmap_transcode_insert/clean` | 45,866,878 | -4.8425% | -1.1205% |
| glibc-2.39 | `instructions` | `strmap_transcode_insert/escaped` | 96,535,197 | -2.3753% | -0.4698% |
| glibc-2.39 | `instructions` | `sync_blobmap_churn/random` | 137,996,113 | -0.8825% | -0.3471% |
| glibc-2.39 | `instructions` | `sync_blobmap_insert/random` | 56,940,240 | -0.5157% | -0.1375% |
| glibc-2.39 | `instructions` | `sync_blobmap_reclaim_free/random` | 276,267 | -0.3102% | +0.1071% |
| glibc-2.39 | `instructions` | `sync_blobmap_remove/random` | 66,038,564 | -0.7881% | -0.2932% |
| glibc-2.39 | `instructions` | `sync_bytesmap_churn/routes` | 299,649,054 | -0.4596% | -0.1055% |
| glibc-2.39 | `instructions` | `sync_bytesmap_insert/routes` | 112,847,725 | -0.3091% | -0.0195% |
| glibc-2.39 | `instructions` | `sync_bytesmap_remove/routes` | 183,529,985 | -0.3483% | -0.0419% |
| glibc-2.39 | `instructions` | `sync_map_branchu_band/top` | 9,922,514 | -0.2746% | +0.0002% |
| glibc-2.39 | `instructions` | `sync_map_churn/random` | 120,453,006 | -1.1458% | -0.4976% |
| glibc-2.39 | `instructions` | `sync_map_compact_drained/random60` | 20,549,652 | -0.4852% | -0.1201% |
| glibc-2.39 | `instructions` | `sync_map_count_after_write/one_top_byte` | 1,709,934,343 | -0.1007% | +0.0996% |
| glibc-2.39 | `instructions` | `sync_map_count_after_write/random` | 32,099,111 | -0.1397% | +0.0773% |
| glibc-2.39 | `instructions` | `sync_map_count_after_write/sequential` | 14,457,345 | -0.1219% | +0.0875% |
| glibc-2.39 | `instructions` | `sync_map_drain_floor/top` | 246,042 | -0.3945% | +0.0939% |
| glibc-2.39 | `instructions` | `sync_map_insert/random` | 49,827,275 | -0.5750% | -0.1714% |
| glibc-2.39 | `instructions` | `sync_map_remove/random` | 63,735,969 | -0.8130% | -0.3074% |
| glibc-2.39 | `instructions` | `sync_map_write_twin/one_top_byte` | 2,123,666 | -0.6995% | -0.2426% |
| glibc-2.39 | `instructions` | `sync_map_write_twin/random` | 1,886,726 | -0.7748% | -0.2856% |
| glibc-2.39 | `instructions` | `sync_map_write_twin/sequential` | 2,134,636 | -0.2481% | +0.0154% |
| glibc-2.39 | `instructions` | `sync_set_churn/leaf` | 97,606,675 | -2.4298% | -1.2313% |
| glibc-2.39 | `instructions` | `sync_set_churn/random` | 93,659,973 | -0.5409% | -0.1519% |
| glibc-2.39 | `instructions` | `sync_set_compact_drained/random60` | 19,104,347 | -0.8983% | -0.3562% |
| glibc-2.39 | `instructions` | `sync_set_drain_floor/top` | 244,243 | -0.4204% | +0.0815% |
| glibc-2.39 | `instructions` | `sync_set_insert/random` | 45,669,368 | -0.4988% | -0.1279% |
| glibc-2.39 | `instructions` | `sync_set_remove/random` | 53,837,680 | -0.5858% | -0.1776% |
| glibc-2.39 | `instructions` | `sync_strmap_churn_short/short` | 234,312,270 | -1.1322% | -0.4876% |
| glibc-2.39 | `instructions` | `sync_strmap_insert/routes` | 78,836,908 | -2.6989% | -0.6602% |
| glibc-2.39 | `instructions` | `sync_strmap_insert_short/short` | 106,539,090 | -4.9824% | -1.4642% |
| glibc-2.39 | `instructions` | `sync_strmap_insert_sorted/uuid` | 48,893,629 | -8.4922% | -1.4036% |
| glibc-2.39 | `instructions` | `sync_strmap_remove/routes` | 87,976,618 | -0.3459% | -0.0405% |
| glibc-2.39 | `rmw_instructions` | `sync_bytesmap_cas_insert/routes` | 109,680,661 | -0.3039% | -0.0165% |
| glibc-2.39 | `rmw_instructions` | `sync_bytesmap_cas_remove/routes` | 197,505,800 | -0.3307% | -0.0318% |
| glibc-2.39 | `rmw_instructions` | `sync_map_cas_insert/random` | 47,441,394 | -0.5698% | -0.1685% |
| glibc-2.39 | `rmw_instructions` | `sync_strmap_cas_insert/routes` | 103,229,461 | -1.6576% | -0.5486% |

The arms §47.10 names as design B's cost are among them: `sync_strmap_insert_short/short`, `sync_strmap_insert_sorted/uuid`, `sync_strmap_insert/routes` and `rmw_instructions` `sync_strmap_cas_insert/routes`. The non-string `sync_*` arms improve through the freelist-hit term alone. `strmap_churn/routes`, §47.10's plain loser, makes no call to `new_shell` or `new_suffix` in its profile at the base, so it is bounded at No change: this increment does not address it.

#### Wall-clock (reference host)

- **The gate is §47.3's G-scale, unchanged:** C_head(W) / C_base(W) at W ∈ {2, 4, 8}, the 0.90 floor, two independent runs, 8 rounds, the pin `0,2,4,6,8,10,12,14`. Head is the increment's commit; base is `main` at `6dc24a748bc6b28ef58aa68f6ad0439b4a65ecdc`, the base of §47.10's runs.
- **Reported against an expectation, deciding no gate:** the paired T_head(W) / T_base(W), BCa 95%, n = 8 per run, at W ∈ {1, 2, 4, 8}, in both runs.
- **Pass condition, for the maintainer to weigh:** in both runs, the W = 1 ratio's lower bound is above 0.928. §47.10 measured [0.9284, 0.9341] and [0.9250, 0.9354].
- **Expectation:** upward at every W, of unpredicted size. The instruction saving above is the only derived quantity, and how instructions map to time on this path is unmeasured (§47.7).

#### Falsifiers, and what follows each

| Falsifier | What follows |
|---|---|
| An arm outside its bound on either instrument, including a No change arm moving by more than its slack | `callgrind_annotate --inclusive=no` base against head on the arm, and `objdump -d -C` of every function whose count moved without a source change (AGENTS.md §6). Then the code changes and is re-measured against the same bound, or the departure is recorded here as a dated note. The bound does not move. |
| The two instruments disagree on an arm's direction | attributed as above, per C library; the C library's share is reported separately from the engine's |
| A stale-block test, the Miri test, or CI's Tier-1 Miri, ASan or loom fails | the change does not ship as is |
| A plain path gains a `%fs:` operand (`objdump` of the `instructions` binary, base against head: the `strmap_insert`, `strmap_oscillate` and `strmap_churn` wrappers, and `new_shell::<false>`) | fixed before the push is marked ready (AGENTS.md §2.1 invariant 5) |
| G-scale `BELOW_FLOOR` or `INTERMEDIATE` | as §47.5 |
| The W = 1 pass condition not met | reported; no wall-clock gain is claimed for the increment |

#### Silent constants, fixed here

- The base `a37a044e` and the G-scale base `6dc24a74`.
- Both instruments as stated: the images and their digests, libc6 `2.41-12+deb13u4` and `2.39-0ubuntu8.9`, rustc 1.99.1, valgrind 3.27.1 built from source, `iai-callgrind-runner` 0.16.1. The harnesses are `instructions` and `rmw_instructions` only.
- The per-call ranges: [0, 6] setup per removed `memset`, the 8 Ir `memset` floor, [0, 6] per shared shell freelist hit, and [0, 2] per plain shell.
- The page `memset` cost (4,114 and 4,113), and the fully collected thresholds (60 Ir per `calloc` call, 300 per `posix_memalign` call).
- The `calloc` difference floor (24.0, 31.0) and the `malloc` floor (47.0, 53.0), with the three heap states that produced them.
- The 0.1% slack with its 500 Ir floor.
- G-scale's own constants (§47.6), and the 0.928 threshold.

Changing any of these after a measurement of the increment relabels that result `INTERMEDIATE` (AGENTS.md §8.19).

#### Dispatch, for the maintainer

With `HEAD` the increment's commit (40 characters), two independent runs, each its own dispatch:

```bash
gh workflow run bench_baremetal.yml -f benchmark_suite=writer_scaling_1586_str_vs_base \
  -f ref="$HEAD" -f base_ref=6dc24a748bc6b28ef58aa68f6ad0439b4a65ecdc -f cpu_pin=0,2,4,6,8,10,12,14
```

### 47.12 The increment's Callgrind outcome (appended 2026-10-24; §47.11 is not rewritten, and no bound moves)

Two builds of the increment were measured on both §47.11 instruments against `a37a044e`, every arm of `instructions` and `rmw_instructions`:
- `fec9aa7a`, the increment as first committed;
- `49bc2906`, the same increment with `alloc_raw`'s body stamped by a macro (below).

`75c67e0a`, between them, changed two sentences of §47.11's text before any measurement of the increment was read, and no bound, constant or table row. Artifact: `results/strmap_zeroing_callgrind_49bc2906.json`, every arm of both builds with its delta and bound.

**G-instr for the increment: met at `49bc2906`, on both instruments.** All 286 arms lie inside their bounds under glibc 2.41 and under glibc 2.39.
- The 46 arms expected to improve did, by the same counts in both builds. The main ones, measured Δ against `a37a044e`, with the bound:

  | arm | 2.41 | 2.41 bound | 2.39 | 2.39 bound |
  |---|---:|---|---:|---|
  | `sync_strmap_insert_short/short` | -1.5472% | [-2.7962%, -1.1703%] | -3.7884% | [-4.9824%, -1.4642%] |
  | `sync_strmap_insert_sorted/uuid` | -2.3642% | [-8.6765%, -1.1108%] | -1.8325% | [-8.4922%, -1.4036%] |
  | `sync_strmap_insert/routes` | -1.0390% | [-2.6313%, -0.5709%] | -0.9599% | [-2.6989%, -0.6602%] |
  | `rmw_instructions` `sync_strmap_cas_insert/routes` | -0.6838% | [-1.5797%, -0.4454%] | -0.5979% | [-1.6576%, -0.5486%] |
  | `sync_strmap_churn_short/short` | -0.7507% | [-1.0667%, -0.4511%] | -0.7600% | [-1.1322%, -0.4876%] |
  | `strmap_oscillate/routes` | -2.8689% | [-3.9214%, -1.8724%] | -3.0373% | [-4.2203%, -2.0440%] |
  | `strmap_raw_insert/clean` | -1.7222% | [-5.1534%, -1.1326%] | -1.8310% | [-5.3273%, -1.2453%] |
  | `strmap_insert/routes` | -0.2214% | [-0.4098%, -0.0476%] | -0.2348% | [-0.4336%, -0.0614%] |
  | `sync_set_churn/leaf` | -1.7302% | [-2.6231%, -1.3418%] | -1.5874% | [-2.4298%, -1.2313%] |
  | `sync_map_churn/random` | -0.8731% | [-1.0664%, -0.4522%] | -0.9147% | [-1.1458%, -0.4976%] |

- **The No change arms.** Under 2.41 every one is identical to `a37a044e`, `strmap_churn/routes` among them. Under 2.39, eight shared string read and scan arms move by at most 0.0171% (`sync_strmap_next_after_scan_locked/paths`, −20,887), inside their slack.

**The departure at `fec9aa7a`, and what changed (§47.11's first falsifier row).**
- At `fec9aa7a`, 284 of 286 arms lay inside on each instrument. Two No change arms did not, by the same counts on both: `map_remove/random` +100,042 Ir (+0.4442% under 2.41) and `map_remove_partial/random60` +275,002 (+0.3960%).
- `callgrind_annotate --inclusive=no` puts all of `map_remove`'s change in its arm wrapper, whose source did not change: +100,042 instructions and +50,001 data writes, over 50,000 operations. `objdump -d -C` of the wrapper shows its frame growing from `0x348` to `0x358` bytes and its per-key loop reloading the loop pointer from the stack.
- Bisected by building the `instructions` binary and diffing the wrapper's disassembly against `a37a044e`. `occ.rs`'s change alone leaves it identical. `alloc.rs`'s change alone reproduces it, both with `alloc_raw` wrapping a body that takes `const ZERO` and with `const ZERO` as `alloc_raw`'s own parameter.
- `49bc2906` stamps the body twice with a macro, `alloc_raw_body!`: the zeroed variant with `write_bytes` and `alloc_zeroed`, the non-zeroing one with neither and `alloc`. The `map_remove`, `map_remove_partial`, `strmap_churn`, `strmap_insert` and `set_insert` wrappers are then instruction for instruction those of `a37a044e`. The full benches were then re-measured, as above.

**Attribution on `sync_strmap_insert_short/short`** (`callgrind_annotate --inclusive=no`, `a37a044e` against `49bc2906`, per function self Ir):

| function | 2.41 | 2.39 |
|---|---:|---:|
| `calloc` | -3,334,276 | -3,696,419 |
| `malloc` | +2,060,483 | +2,260,245 |
| unnamed libc function at `0xab650` (2.39 only) | | -2,272,628 |
| `Collector::pop_freelist` | -210,176 | -210,176 |
| `memset` (unnamed: `0x163280`, `0x189780`) | -109,920 | -117,913 |
| `new_suffix` | -150 | -150 |
| `new_shell::<true>` | -1 | -1 |
| other functions | 0 | +918 |
| **arm total** | **-1,594,040** | **-4,036,124** |

- The `calloc` to `malloc` swap nets −1,273,793 Ir under 2.41 over the arm's 49,951 shell and suffix allocations, 25.5 per call, inside the per-call range §47.11's term gives this arm's suffix arc, [24.0, 51.8]. Under 2.39, counting `0xab650`, it nets −3,708,802, 74.2 per call, inside [31.0, 100.0].
- The removed `memset` cost matches the call records: −109,920 against 109,920 under 2.41 to the instruction, and −117,913 against 118,008 under 2.39.
- **A derivation miss, recorded, deciding nothing.** `pop_freelist`'s own count fell by 210,176 on both instruments, beyond the at most 82,440 (2.41) its per-hit setup term allowed. The function fell from 145 to 103 instructions (`objdump`); which of its paths lost them is not attributed per instruction here. The bound held through the width of the `calloc` term.

**`%fs:` on the plain paths** (`objdump -d -C --no-show-raw-insn` of the 2.41 `instructions` binary, `a37a044e` → `49bc2906`, instructions and `%fs:` operands):
- the `strmap_insert`, `strmap_oscillate` and `strmap_churn` wrappers: 1,387 / 3,615 / 4,860 at both, 0 `%fs:`;
- `new_shell::<false>`: 75 → 78, 0 `%fs:`;
- `new_shell::<true>`: 113 → 94; `new_suffix`: 173 → 189; `pop_freelist`: 145 → 103; each 1 `%fs:` at both.

**Not yet measured.** The wall-clock report §47.11 registers (T_head(W) / T_base(W) against `6dc24a74`, with G-scale) needs the maintainer's dispatch at the increment's head. Against `main`, the instruction-counts job on the PR head is the record.

### 47.13 The zeroing increment on the reference host, and the outcome for design B (appended 2026-10-24; §47.1 to §47.12 are not rewritten, and no bound moves)

**Wall clock, increment head `af4810f6` against `main` `6dc24a74`.** Two dispatches under §44.10's rules, both admitted: runs [40081486902](https://github.com/orieg/expanse/actions/runs/40081486902) and [40085444157](https://github.com/orieg/expanse/actions/runs/40085444157). Artifacts are `results/gate_1586_str_writer_scaling_zeroing_run{1,2}.json`.

G-scale (C_head/C_base) meets its floor in both runs:

| W | run 1 | run 2 |
|---|---|---|
| 2 | [1.0081, 1.0202] | [0.9499, 1.0191] |
| 4 | [0.9826, 1.0290] | [0.9936, 1.0496] |
| 8 | [0.9744, 1.0586] | [0.9897, 1.0547] |

The verdict is MEETS_FLOOR in both runs.

The table below gives the paired T_head/T_base, a lower figure meaning design B is slower. It is BCa 95% with n = 8 per run, reported and not gated (§47.11):

| W | run 1 | run 2 | §47.10, before the increment |
|---|---|---|---|
| 1 | 0.9486 [0.9458, 0.9507] | 0.9499 [0.9485, 0.9509] | 0.9321 / 0.9306 |
| 2 | 0.9619 [0.9573, 0.9654] | 0.9443 [0.9027, 0.9673] | 0.9486 / 0.9441 |
| 4 | 0.9560 [0.9304, 0.9768] | 0.9661 [0.9430, 0.9969] | 0.9427 / 0.9504 |
| 8 | 0.9689 [0.9271, 1.0047] | 0.9662 [0.9405, 1.0015] | 0.9545 / 0.9564 |

§47.11's reported condition holds in both runs: the W = 1 lower bound is above 0.928. Design B with the increment still costs about 5% of shared string-insert throughput at one writer and about 3–4% at eight.

**G-instr against §47.3: NOT MET.** CI run [40074862826](https://github.com/orieg/expanse/actions/runs/40074862826) measured instruction counts at `af4810f6` against `main` `a2fc24c1`. `a2fc24c1` changes nothing under `crates/` or `include/` since `6dc24a74`.

| arm | head / base Ir | Δ | §47.3 bound | position |
|---|---:|---:|---|---|
| `sync_strmap_insert_short/short` | 102,503,237 / 97,934,923 | +4.66% | [+6.27%, +6.47%] | below (favourable) |
| `sync_strmap_insert_sorted/uuid` | 47,997,972 / 46,291,751 | +3.69% | [+6.19%, +6.39%] | below (favourable) |
| `sync_strmap_cas_insert/routes` | 102,611,783 / 101,026,457 | +1.57% | [+2.18%, +4.78%] | below (favourable) |
| `sync_strmap_cas/routes` | 73,804,246 / 72,879,132 | +1.27% | [+1.17%, +1.37%] | inside |
| `sync_strmap_update_rmw/routes` | 123,164,601 / 122,239,487 | +0.76% | [+0.66%, +0.86%] | inside |
| `sync_strmap_insert/routes` | 78,082,111 / 77,553,128 | +0.68% | [+0.34%, +2.09%] | inside |
| `strmap_churn/routes` | 119,088,554 / 117,738,626 | +1.15% | up to +0.1864% | **above, about 6×** |

- `strmap_churn` is a plain-tree cost with no gain on its arm. It is register drift in the inlined insert and remove bodies (§47.9). One remedy was tried and reverted, and the increment leaves this arm unchanged at 0 Ir. It ships as a **disclosed miss**, not as a pre-registered loss.
- The composite "design B plus the increment against `main`" was never registered as a bound of its own. The increment itself met its own bounds (§47.11) on both C libraries (§47.12).

**Carried forward:**
- G-stall PASS on the growth cells (§47.10). The churn cells are uninformative, because a 30 s single-insert stall on `short` keys is present on `main` as well.
- G-accuracy is met, with its `stats()` half NOT_EVALUABLE.
- G-density and G-safety are met.

**Whether a `len()` × ratio estimate would do instead.**
- A ratio taken from an exact reading at the start of each 30 s window misestimates the end-of-window `mem_used`:
  - by 0.58–0.62% on `routes` keys;
  - by 14.4–19.1% on `short` keys, on both builds, as the map grows about 6× and bytes per key fall (54.07 → 47.0 on design B; 43.84 → 36.9 on `main`).
- These figures are derived from the committed G-stall artifacts' `mem_used_before/after` and `len_before/after`.
- So the recipe needs refreshes to stay within ±10% on short keys. On a map that is never rotated, each refresh is the 0.5–2 s stall this design removes.
- The caller's growth between readings is smaller than this window's. That is unmeasured.

**Outcome.**
- Every §47 gate is met except G-instr: the gain axis G-stall, G-scale (four runs), G-accuracy, G-density and G-safety.
- G-instr is NOT MET against §47.3. Three arms lie below their bounds, three inside, and `strmap_churn` above.
- Under §47.5's first row, the change ships on the maintainer's decision, weighing:
  - the gain: a `mem_used` reading no longer holds writers. The largest insert goes from 0.5–2 s to about 0.15 ms.
  - the named costs: about 5% of shared string-insert throughput at one writer, the CI instruction losses above, and `strmap_churn`'s disclosed miss.
- The decision to merge is recorded with the merge.

**Follow-ups, not part of this change** (Refs #1586):
- the 30 s churn-cell stall, which predates design B;
- `stats()` not summing to `mem_used` on concurrently written maps;
- the `strmap_churn` inlining recovery;
- a relaxed emptiness hint before the locked freelist pop;
- the count of lock-prefixed RMWs per allocation in `Deferred::charge`.

Each needs its own pre-registration.

## 48. Pre-registration for #1602 — an exclusive removal counts a dirty sub-map only as far as its decision needs (appended 2026-10-24, locked before any measurement of the change build; next free section)

### 48.1 The defect and the change

On a `SyncExpanseStrMap` held at constant population on `short` keys, one insert can wait for most of a 30 s window (§47.10, churn cell, both builds). A diagnostic on the x86_64 heavy-jobs host located the cause. That host is not the reference host: these are single runs of an instrumented `occ-stats` build, unpinned, with no interval, so they are a diagnosis, not a figure.
- **The fold.** A removal that falls back to the serialised path runs `StrNode::remove_shared`, which calls `resync_at` on every node it removes from or prunes under (`strmap.rs`). On a node optimistic writers left dirty, `resync_at` folds the whole sub-map (`sync::fold_branch_pop0`) with the writer gate closed. The root sub-map of a `short`-key map holds the first chunk of every key, so the fold reads every key: 225–365 ms per fold at N = 2·10⁷. Stubbing the fold took the cell from 33 to about 10⁶ operations per second, and its largest insert from 2.27 s to 4.2 ms.
- **What the fold is for.** The removal reads the folded count only to decide whether to condense the tree into a root leaf or empty it (`MapCore::remove_covered_at`, `now = tree_pop − 1` against 0 and `ROOT_LEAF_CAP`). Branch demotion reads child counts, and ancestor `pop0` updates are saturating bumps.

**The change.** `resync_for_remove_at` replaces `resync_at` at the three removal sites. On a dirty node it counts the sub-map only up to `ROOT_LEAF_CAP + 1` keys (`sync::subtree_pop_capped`).
- **At or below `ROOT_LEAF_CAP` keys:** the sub-map is folded exactly, which is cheap at that size, and its flag is cleared, as today.
- **Above it:** the node stays dirty and its stale count is raised to at least `ROOT_LEAF_CAP + 1`. Optimistic inserts leave the count below the truth, so the raise is what keeps the removal from condensing a large tree.

Design review: three synthetic reviewers (low-latency, storage-internals and decision red-team lenses), not peer review. Their conditions are the tests in §48.3.

**Not in scope.** At N = 2·10⁷, 23% of removals take a `branch_split_remove` fallback, in the stubbed and the unstubbed build alike, against about 0.02% at 2·10⁶. That limits throughput, not the stall, and needs its own issue.

### 48.2 Gates

**G48-stall (reference host).**
- **Cells:** §47.3's churn cell at its coordinates (N = 2·10⁷, W = 4, D = 30 s, both shapes, 5 rounds, one process per round, §44's pin and placement), arm `none`, driven by `strmap_poll_stall.py --plan 47` on the change build.
- **Statistic:** the per-round largest insert latency (`insert.max_ns`), with BCa 95% intervals (2,000 resamples, `ci_method` recorded).
- **Verdicts.** Let `S` be the `short` churn cell and `R` the `routes` churn cell, both measured in the same run.
  - `PASS` iff `S`'s upper bound ≤ 2 × `R`'s upper bound.
  - `FAIL` iff `S`'s lower bound > 2 × `R`'s upper bound.
  - `INTERMEDIATE` otherwise.
- **Why that comparison:** `routes` keys never stalled in §47.10. Their largest insert there was 22–40 ms. The factor 2 allows for the different key shapes, so `PASS` means `short` keys no longer behave differently from a shape that does not stall.
- **Descriptive only:** the writers' CPU share and the total inserts per cell are reported beside the verdict, and are not compared across runs (`docs/BENCHMARKING.md` rule 18).

**G48-instr (CI, deterministic).** No `instructions` arm is worse than `main` by more than 0.1% (AGENTS.md §6 review threshold). The changed path runs only on the exclusive removals of a shared string map, so the expected movement is confined to `sync_strmap_*` removal arms.

**G48-safety (CI).** The §48.3 tests pass. Each has been shown failing on the clause it guards.

### 48.3 Tests and their negative controls

The tests are `strmap::tests::`:
- `deferred_olc_removal_from_a_large_dirty_sub_map_keeps_it_a_tree`: stale-low count, exclusive removals, every key checked;
- `deferred_olc_capped_resync_decides_at_the_root_leaf_boundary`: one removal at a time from `ROOT_LEAF_CAP + 4` keys, asserting the dirty flag, the tree state and the exact count at each step;
- `deferred_olc_removal_from_a_dirty_sub_map_does_not_fold_it`: at most 64 edges visited per removal from a dirty 20,000-key sub-map;
- `removal_sites_use_the_capped_resync`: a source pin of the three sites.

Every mutation below was applied alone, and each turned a test red. Five failed through `MapCore::condensed_leaf`'s `written < n` assertion or a named assertion, and one through the edge bound:

| mutation | red test |
|---|---|
| no `max` on the raised count | large-sub-map test (condenses a large tree) |
| the large branch clears the flag | both decision tests |
| cap `ROOT_LEAF_CAP + 2` | boundary test |
| cap `ROOT_LEAF_CAP` | both decision tests |
| always fold | edge-bound test |
| one removal site reverted to `resync_at` | source pin |
| one removal site commented out | source pin |

The run waits for the maintainer's go-ahead. No bound in §48.2 moves after it.

### 48.4 The G48-stall outcome (appended 2026-10-25; §48.1 to §48.3 are not rewritten, and no bound moves)

**The run.**
- **Host:** the reference host (12th Gen Intel Core i9-12900F, 24 threads, pinned to CPUs 0–15).
- **Commit:** `f5cb17a6`, `main` with #1606 merged.
- **Suite:** `strmap_poll_stall_g47` (`strmap_poll_stall.py --plan 47`), CI run [40161439558](https://github.com/orieg/expanse/actions/runs/40161439558).
- **Artifact:** `results/gate_1602_strmap_poll_stall.json`, the driver's output committed unchanged under this name. Its provenance names §47 and #1586 because the suite is §47's driver.

**G48-stall: `PASS`.** The statistic is the per-round largest insert of the `none` arm of each churn cell, 5 rounds, with BCa 95% intervals (2,000 resamples, `bca_bootstrap.bca_bootstrap_ci_with_method`):

| cell | rounds (ns) | BCa 95% (ns) |
|---|---|---|
| `short` churn (`S`) | 614,911 · 87,423 · 125,439 · 85,695 · 134,015 | [96,050, 422,553] |
| `routes` churn (`R`) | 1,284,095 · 219,647 · 131,007 · 166,015 · 197,247 | [164,505, 858,316] |

`S`'s upper bound, 422,553 ns, is at most 2 × `R`'s upper bound, 1,716,632 ns, so the cell is `PASS` (measured: reference host, `f5cb17a6`).

**Host quiet.** The foreign busy CPU of the ten processes G48 reads was at most 0.37, against the void threshold of 1.0 (§8.17).
- The workflow concluded `failure` because §47's own G-stall verdict reads `VOID`.
- One process voided it: the `short` growth cell's `len` arm, round 0, with foreign busy CPU 1.25.
- That cell is not one G48 reads. §47's verdict on this build is not part of §48.

**Descriptive, not compared** (§48.2; `docs/BENCHMARKING.md` rule 18).
- **Writers' CPU share, `short` churn:** 0.94–1.00 per round.
- **Throughput, BCa 95% over the rounds:**

  | cell | ops/s |
  |---|---|
  | `short` churn | [1,107,524, 1,172,868] |
  | `routes` churn | [2,980,294, 3,066,920] |

- **Earlier record:** §47.10 recorded this cell on earlier builds with a largest insert of 29.31–30.09 s and a share of 0.26. Those are different builds and different runs, and this run does not form a paired comparison with them.

**Still open.** The `branch_split_remove` fallback rate at N = 2·10⁷ (§48.1) is untouched by this change and needs its own issue.
