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
3. **Contention health cells ($W \ge 2$)**: The two-commit sweep script (`run_all.py`) is updated to record health rows for $W \in \{1, 2, 4, 8, 16\}, R = 0$ in addition to the $W = 1, R = 8$ mixed-reader row. Previously committed health cells were restricted to $W = 1$, where `Stat::LockRestarts` and `Stat::LockSpins` were 0 by construction. Measuring health across all $W$ provides the empirical restart counts required for P5.3 evaluation against the instantiated restart ceilings, and directly measures the fallback share and spin time under concurrent write pressure.
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
  - **Base exits.** Every walk exit stores its answer in `%rax` and branches to one shared block, reached by seven branches. That block converts the answer to a bool (`test %rax,%rax; setne`), writes `INACTIVE` to the reader's slot (the pin's drop, `occ.rs:1932`) and jumps to the epilogue. The bitmap exit reaches it through `bt`/`setb`.
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
