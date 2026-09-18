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
  are pending re-measurement (#730), no competitor arm is dispatched by this
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
(`chunk_at`, `crates/expanse/src/strmap.rs:321`). A chunk with fewer than eight
bytes remaining is *terminal* and its entry holds the user value directly; a
non-terminal chunk's entry is a tagged word — tag 0 a `*mut StrNode` child
(`pack_child`/`unpack_child`, `strmap.rs:171`, `:177`), tag 1 a `*mut StrSuffix`
leaf (`pack_suffix`/`unpack_suffix`, `strmap.rs:161`, `:166`); module doc
`strmap.rs:22-27`.

**The two node forms.**

- `StrNode { map: MapCore }` (`strmap.rs:191-193`) — a meta-trie branch is
  exactly one word-map engine core, and nothing else. Its size is pinned equal
  to `MapCore` and at most 64 bytes by `str_node_is_just_the_map_core`
  (`strmap.rs:2153-2160`).
- `StrSuffix { value, len }` with the remaining key bytes inline in the same
  allocation (`strmap.rs:78-81`, bytes at `SUFFIX_BYTES`, `strmap.rs:104`).
  Header and bytes are write-once after publication; only `value` mutates in
  place (`strmap.rs:70-76`).

**Every transition a mutation can take**, enumerated from the entry points:

| # | entry point | transition | the store, and where it lands |
|---|---|---|---|
| T1 | `insert` `strmap.rs:1217` | terminal chunk | `MapCore::insert_pathless` into this node's sub-map (`:1232`) |
| T2 | `insert` | continuation absent | `new_suffix` (`:113`) then `insert_pathless` of the tagged word (`:1240-1241`) |
| T3 | `insert` | suffix present, remainder equal | in-place replace of the value word (`:1259`) — the only in-place payload mutation |
| T4 | `insert` | suffix present, remainder diverges | `split_suffix` (`:1188-1213`): a private child `StrNode` is built and populated (`:1201`, `:1207`), published with one `insert_pathless` over the suffix entry (`:1211`), and the old suffix disposed (`:1212`) |
| T5 | `insert` | child present | descend (`:1268`) — no store |
| T6 | `ins_slot` `strmap.rs:1278` | terminal chunk | `ins_slot_pathless` (`:1297`); otherwise T2/T3/T4/T5 (`:1306`, `:1313-1331`) |
| T7 | `remove` `strmap.rs:1489` → `StrNode::remove` `:720` | terminal chunk | `remove_pathless` (`:737`) |
| T8 | `remove` | suffix match | `remove_pathless` then `dispose_suffix` (`:745-751`) |
| T9 | `remove` unwind | emptied child pruned | `remove_pathless` on the **parent** then `dispose_node` (`:771-777`) — the one transition that propagates upward |
| T10 | `remove` | meta-trie root emptied | `self.root.take()` then `dispose_node` (`strmap.rs:1497-1505`) |
| T11 | `insert`/`ins_slot` | meta-trie root absent | a root `StrNode` is created (`strmap.rs:1225-1227`, `:1286-1288`) |
| T12 | `clear` `strmap.rs:1584` | whole tree | `dispose_tree` (`:283`) |

**What covers each store today.** Every one of T1–T12 runs inside
`Shared::write` (`crates/expanse/src/sync.rs:1654`), reached from
`SyncExpanseStrMap::insert` (`:8019`), `remove` (`:8025`), `clear` (`:8030`) and
`with_locked_mut` (`:8093`). That function takes the writer mutex and holds the
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
   `Root::Leaf` and `Root::Tree` (`crates/expanse/src/map.rs:831`, `:837`,
   `:848`) are covered by the tree-level word; only branch headers carry a word
   of their own (`docs/ARCHITECTURE.md` §4.1). A `StrNode` whose sub-map is
   empty or a root leaf therefore has no word anywhere to lock.
2. **The cover word is reached through the allocator, and the string map has
   one allocator for every sub-trie.** The engine finds its word through
   `NodeAlloc::bind_tree_word` / `tree_cover_addr`
   (`crates/expanse/src/alloc.rs:922`, `:936`, `:951`), bound once per
   allocator; `ExpanseStrMap` deliberately shares a single `NodeAlloc` across
   every sub-trie (`strmap.rs:11-14`, `defer_to` `:1129`, `:1137`), which is
   what keeps a `StrNode` at one map root instead of ~700 bytes. One allocator
   is one bound word.

**The design registered, therefore:** a **per-`StrNode` cover word**, covering
that node's sub-map root state and its tagged continuation entries, with the
engine's existing per-node words covering the sub-map interior when that
sub-map is a tree; writers enter through the wrapper's writer-entry path
(`Shared::enter_writer_blocking`, `sync.rs:1579`) and couple hand-over-hand
down the chunk chain, one cover per hop. Reaching a per-node word requires the
engine to take its cover per operation rather than from the allocator binding
above — which of the two available shapes (a word passed down the descent, or
a second binding mechanism) is an implementation choice this section does not
fix, because the gate is an outcome gate and neither shape is measured here.

**Two consequences of that word, named now rather than discovered later
(AGENTS.md §2.3).** A `StrNode` gaining a version word grows past
`size_of::<MapCore>()`, so `str_node_is_just_the_map_core`
(`strmap.rs:2153-2160`) fails and is part of the change, with its ≤ 64-byte
bound re-argued rather than deleted; and `ExpanseStrMap` does not implement
`RootState` today (the only impls are `ExpanseMap`, `sync.rs:1353`, and
`ExpanseSet`, `:1360`), so `write_root_covered` (`sync.rs:1713`) and the whole
`olc_*` route are not reachable for the string wrapper at all until it does.

#### 17.2.2 What a writer must hold for the suffix, and how readers validate

**The writer.** A suffix block is write-once after publication except its value
word (`strmap.rs:70-76`). A writer that publishes one must allocate it
(`new_suffix`, `:113`), store the tagged word into the parent `StrNode`'s
sub-map under that node's cover (T2, T4), and retire the superseded block
through the epoch collector (`dispose_suffix`, `:205-224`) — never free it
inline, since a reader that validated the old tagged word may still be reading
the header and bytes under its pin. T3's in-place value write is the one store
that mutates a published block, and it stays a single word under the cover of
the node holding the entry that points at it.

**The reader, and the hazard this design creates.** `StrReader::get`
(`sync.rs:8117`) calls `ExpanseStrMap::get_validated` (`strmap.rs:1396`), which
walks one sub-map per chunk. Each hop enters `walk_validated::<true>`
(`sync.rs:192`) with `Cover::Tree(ver, snap)` (`sync.rs:158-176`, `:198`) and
moves to `Cover::Node` inside the sub-map; **between hops, and on the suffix
arm before returning, the reader re-validates the tree version**
(`strmap.rs:1440-1447`). A design in which writers no longer bump the tree word
therefore leaves those checks validating a word nobody moves, which is the
precise failure the reader half of this design must prevent: the cross-hop
check and the suffix arm's check move to the cover word of the `StrNode` whose
entry was read, sampled before the entry is loaded and re-validated after it,
as `walk_validated` already does within a sub-map. `SyncExpanseStrMap::len`
(`sync.rs:8059`) keeps the tree word, and `with_locked` / `read_locked`
(`sync.rs:1844`, `:1815`) keep closing the gate and quiescing writers.

This is the first item of the §2.3 five-subsystem audit, and it is the reason
the coverage in §17.5 is registered as a precondition rather than a follow-up.

#### 17.2.3 What stays behind a bounded fallback, and the rate registered

Registered as staying behind the blocking fallback (`write_root_covered`
behind `fallback_mutex`, `sync.rs:1732`, which quiesces writers and serialises):

- **T11**, creation of the meta-trie root, and **T10**, its removal — root-state
  transitions of the wrapper itself.
- **T12**, `clear`, which is a whole-tree disposal.
- **T9**, the upward prune of an emptied child, which mutates a node the
  descent has already left.
- Whatever the engine's own OLC path already falls back on inside a sub-map:
  the `FallbackCause` set is `CapExpansion`, `ImmediateConversion`,
  `BranchSplit`, `RootGrowth`, `Contention`, `UnknownTag` (`sync.rs:1967-1987`),
  and a sub-map in `Root::Empty` or `Root::Leaf` state raises `RootGrowth`.

**The rate registered, and its derivation.** The declared workload is
`concurrency_writer_str`: a 2^20-key prefill, then 2^20 fresh keys inserted by
W writers over contiguous disjoint slices (`writer_scaling.rs:800-867`,
generator `:600-643`). Keys are alphanumeric over a 62-symbol alphabet
(`fill_alnum`, `:543-548`) with length `8 + rng % 9`, so every key is 8 to 16
bytes.

- A key of at least 8 bytes never presents a terminal first chunk (`chunk_at`,
  `strmap.rs:321`), so **T1 cannot occur at the meta-trie root** on this
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
| the fallback prediction (§17.2.3) | `fallback_rate` and `fallback_causes_total`, over `write_ops` — which is the harness's fresh-key count, not `Stat::WriteOps` (`scripts/writer_scaling.py:604-605`, `writer_scaling.rs:2671`) |
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
`Shared::write` (`sync.rs:1654`), which bumps `Stat::WriteOps` and never
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
  `crates/expanse/src/occ.rs:2635-3308` and
  `crates/expanse/src/sync.rs:12161`, and run by the `loom` CI job
  (`.github/workflows/ci.yml:1909-1930`). At minimum: two writers publishing
  into one `StrNode`'s sub-map entry are mutually excluded; a superseded suffix
  or child is marked and retired only after the entry pointing at it is
  rewritten (the S3 property, for the tagged word); and a reader's cross-hop
  check validates the word the writer actually bumps — the hazard §17.2.2
  names, whose negative control is a model that is red when the cross-hop check
  is left on the tree word.
- **Tier-1 Miri.** A deterministic single-threaded test of every new transition,
  named so the per-PR filter selects it. That filter is a literal list
  (`.github/workflows/ci.yml:1379`, mirrored in AGENTS.md §5), so either the
  new tests sit under a prefix it already selects — `strmap::tests::deferred`,
  `occ::tests::` — or the filter and `scripts/check_miri_shards.py`'s
  module-to-shard map are updated in the same change. No Miri is run on the
  laptop; the CI jobs are the authority.
- **Linearizability.** `crates/expanse/tests/linearizability.rs` covers
  `SyncExpanseMap` and `SyncExpanseSet` only (`:8`). A `SyncExpanseStrMap`
  history test is added with the same per-key checker shape, at W ≥ 2, plus the
  disjoint-writer census check that `test_multi_writer_parallel_disjoint_and_census`
  (`:468`) performs for the two integer wrappers.
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
   in `.github/workflows/bench_baremetal.yml` (the suite list at `:64-70` and
   the case block at `:990-999`), synced with
   `python3 scripts/check_bench_suites.py --write`.
3. **The gate statistic is computed by committed code** before the run that it
   judges. The driver writes `scaling_factor_c_n_*` per build; the ratio of the
   two builds' C(W) is not a field it emits today, so the reduction §17.3 names
   is committed with the suite entry and reads `rounds_raw` of both builds.
4. **The driver's counters-pass identities must still hold on the head.** It
   refuses a row where the fallback causes do not sum to `lock_fallbacks`
   (`writer_scaling.py:615-618`), where `Stat::Inserts` differs from the
   harness's insert count (`:620-623`), or where `quiesce_calls` differs from
   `lock_fallbacks` (`:626-629`); the harness checks the same identities
   (`writer_scaling.rs:424-441`). A design whose fallback path does not quiesce
   exactly once per fallback breaks the third, and that is a change to the
   instrument's invariant which is disclosed and re-argued, never silently
   relaxed.
5. **The artifact records which pre-registration it was read against**, as the
   ordered-reader artifacts do (`writer_scaling.py:1595`). A run whose artifact
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
| `reader_scaling_bounds.per_arm_interval` | **exists** | `scripts/reader_scaling_bounds.py:193` |
| `reader_scaling_bounds.mde_from_rounds` | **exists** | `scripts/reader_scaling_bounds.py:225` |
| `reader_scaling_bounds.frequency_share` | **exists** | `scripts/reader_scaling_bounds.py:239` |
| `reader_scaling_bounds.unexplained_cycles` | **exists** | `scripts/reader_scaling_bounds.py:277` |
| `reader_scaling_bounds.max_over_mean_bias` | **exists** | `scripts/reader_scaling_bounds.py:295` |
| `reader_scaling_bounds.hugepage_ceiling` | **exists** | `scripts/reader_scaling_bounds.py:306` |
| `reader_scaling_bounds.round_outliers` | **exists** | `scripts/reader_scaling_bounds.py:206` |
| `reader_scaling_bounds.threshold_a` | **absent** | no definition anywhere under `scripts/`, `crates/` or `docs/` |
| `hitm_cycle_ceiling` / `rfo_cycle_ceiling` | **absent under those names** | the generic `event_cycle_ceiling(per_probe_r1, per_probe_rk, cost_cycles)` exists at `scripts/reader_scaling_bounds.py:266` and takes the per-event cost as an argument |
| `instructions`, `cycles`, `ref-cycles`, `task-clock`, `LLC-load-misses`, `l2_rqsts.rfo_miss`, `mem_load_l3_hit_retired.xsnp_hitm`, `context-switches` per thread | **exist** | `scripts/bench_counters.py:147-156`; the eight `reader/*` keys of `results/counters_masstree_conc_str_w0_r{1,8}.json` |
| `cycle_activity.stalls_l3_miss` | **absent from the per-thread set** | not in `THREAD_EVENTS`, `scripts/bench_counters.py:147-156`; absent from both committed cells' `events` |
| `dTLB-load-misses` per thread | **absent from the per-thread set** | present in `BASE_EVENTS` (process mode) at `scripts/bench_counters.py:132`, not in `THREAD_EVENTS`; absent from both committed cells' `events` |
| `mem_load_l3_hit_retired.xsnp_{fwd,none,miss}` | **absent** | only `xsnp_hitm` is registered, `scripts/bench_counters.py:139` |
| `machine_clears.memory_ordering` | **absent** | not in any registered event set |
| per-cell extra events | **mechanism exists, unused by #730** | `Cell.extra_events` and `Cell.events()` at `scripts/bench_counters.py:238-242`; the precedent is `OPTIMISTIC_EXTRA_EVENTS` at `:169`. No #730 cell requests any extra event today |
| `_masstree` counter cells (`masstree_conc_str_w0_r{1,8}_masstree`) | **absent** | `_conc` hardcodes `arm="expanse"`, `scripts/bench_counters.py:248`; the registry holds only `masstree_conc_str_w0_r1` and `_r8` at `:287`, `:290` |
| harness `--arm <expanse\|masstree>` | **exists** | `crates/expanse-hot-bench/src/bin/masstree_concurrent.rs:540`, documented at `:42` |
| `masstree_conc_str_w0_r{2,4}` counter cells | **absent** | not in the registry |
| W = 0 at R ∈ {2, 4} in the throughput driver | **absent** | `CONCURRENT_MIXED_READERS = 8` is a scalar, `docs/benchmarks/masstree_comparison/scripts/run_all.py:66`; the C2 grid is W ∈ {0, 1, 2, 4, 8} × R = 8, so R = 1, 2 and 4 at W = 0 are not driver cells. R = 1 exists only as a `bench_counters.py` cell |
| `run_all.py --cells` | **absent** | `main()` at `docs/benchmarks/masstree_comparison/scripts/run_all.py:681` selects with `--quick`, `--concurrent`, `--only-concurrent`, `--ab-base-bin`, `--ab-base-commit` and `--self-test` only |
| `scripts/perf_counters.py` with `--arms strmap_get --pops --hit-pcts --runs` | **exists** | flags at `scripts/perf_counters.py:813-817`; `strmap_get` is a documented `EXPANSE_PERF_ARM` value at `crates/expanse/examples/perf_point_lookup.rs:39`, read at `:242`, unknown arms refused at `:341` |
| `GLIBC_TUNABLES=glibc.malloc.hugetlb=1`, verified by `AnonHugePages` | **exists as committed practice** | `docs/benchmarks/masstree_comparison/README.md:597-600`, verified before interpretation |
| `load.foreign_busy_cpus_since_prev` | **exists** | `scripts/check_bench_provenance.py:924`, `:927`; per-cell `load.foreign_busy_cpus` required at `:514-541` |
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
     exported public API (`crates/expanse/src/blobmap.rs:1107`), as are `BlobArena::live_bytes`
     (`crates/expanse/src/blobmap.rs:1013`), `BlobArena::chunks` and `chunks_count`
     (`crates/expanse/src/blobmap.rs:1027, 1020`), and `BlobArena::mem_used`
     (`crates/expanse/src/blobmap.rs:1006`). Public API signatures cannot be modified or
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
   - **Compaction hazard & invariant analysis**: Evaluated `crates/expanse/src/blobmap.rs:902`.
     Chunk header generation checks invalidate retired chunks, but cannot track concurrent
     arena reallocations across uncoordinated threads. The writer gate must strictly enclose
     arena allocation before index insertion; pointer-valued epoch pins must span read through
     publish. Active compaction remains stop-the-world behind the writer lock with the tree-level
     version word bracketed unconditionally (`write_quiesced`).
   - **Single-block bucket layout analysis**: Evaluated `crates/expanse/src/bytesmap.rs:78-98`
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
$$\text{MDE}_{\text{rel}} = (z_{\alpha/2} + z_{\beta}) \cdot \frac{\sigma}{\mu} \cdot \sqrt{\frac{2}{N}} \approx (1.960 + 0.842) \cdot 0.015 \cdot \sqrt{\frac{2}{8}} \approx 2.802 \cdot 0.015 \cdot 0.5 \approx 0.021 \ (2.1\%)$$

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

