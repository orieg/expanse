# Remove retention: subtree condensation on remove (pre-registration)

This document pre-registers condensing a drained branch subtree back into a
packed leaf on remove (AGENTS.md §8.8 commit 2). It is frozen once merged:
outcomes are reported in [`README.md`](README.md) against the gates below and
are never reconciled into this file (§8.7). A threshold, method or sample that
changes after results are seen relabels the result `INTERMEDIATE` (§8.19).

**Status.** Phases 0–2 only: the Step 0a census (§3), the bound functions
(`scripts/condense_bounds.py`, §4) and this document. No engine code changes
here. Phase 3 (the engine change behind a feature, the new Callgrind arms, the
gate evaluation) is a separate pull request.

## 1. The question

Expanse's remove paths never rebuild a branch subtree back into a leaf. A
branch steps down its own ladder (`BranchU` below 192 digits, `BranchB` below
7, `BranchL7` below 3) and is freed when it empties
(`crates/expanse/src/mutate_map.rs`, `map_remove`; `crates/expanse/src/mutate.rs`,
`remove`). Only the root condenses (`condense_to_root_leaf`,
`crates/expanse/src/map.rs` and `set.rs`). A tree that cascaded an expanse past
`LEAF_CAP` and was then drained below it keeps the branch and its edge
subarrays.

Judy's published design coalesces on delete ("decascade", with at most one
index of hysteresis: *Judy IV Shop Manual*, Silverstein 2002, glossary entry
"Cascade" and §5). Only that published idea is used here (AGENTS.md §3).

**Does condensing a drained subtree into a packed leaf, when the leaf is the
smaller of the two, bring a drained tree's `mem_used()` back to that of a fresh
build of the same keys, without costing the paths that do not condense?**

## 2. Scope

In scope: the 64-bit `ExpanseSet` and `ExpanseMap` remove walks, both modes of
the const-generic walk (`mutate_map::map_remove::<OCC, NESTED>`,
`mutate::remove::<OCC, NESTED>`), and a fold-then-condense sweep inside
`shrink_to_fit`. `ExpanseStrMap`, `ExpanseBytesMap` and `ExpanseBlobMap`
remove through `MapCore` and so inherit the map's behaviour.

Out of scope, by maintainer decision:

1. folding a one-child `BranchL3` into a narrow edge (a later change); the
   root's top edge is never condensed by this mechanism;
2. the 32-bit engine (a follow-up);
3. [#1079](https://github.com/orieg/expanse/issues/1079) (optimistic removes
   leave a `BranchU` below 192 digits), which is separate and not touched;
4. a validator `num ≥ 2` floor on branches.

## 3. Step 0a: the no-go gate, measured before this document

**Gate.** On current `main`, the headline cell's
`R = mem_used(insert N, remove down to M) / mem_used(fresh insert of the same M keys)`
must be at least 1.5, or the work stops as a documented negative
(`REJECT_UNDETECTABLE`).

**Instrument.** `crates/expanse/examples/remove_retention.rs` (workload:
`example_remove_retention`). Every figure is `mem_used()` / `mem_held()`, the
engine's exact byte accounting: deterministic, so a cell has no interval (§8.4)
and reproduces to the byte on any 64-bit host at the same commit. Each drained
and fresh tree is validated and its `NodeBytes` attribution asserted equal to
`mem_used()`. The fresh build is canonical because a fresh build's
`mem_used()` does not depend on insertion order
(`crates/expanse/tests/test_mem_used_order_invariant.rs`).

**Headline cell.** `ExpanseSet` and `ExpanseMap`, uniform random 64-bit keys,
N = 3,200,000 (λ = 48.8 keys per 2-byte expanse) removed in shuffled order to
M = 1,000,000 (λ = 15.3). It fits laptop memory (peak RSS 430 MB for the full
grid), so no substitute cell was used.

**Result** (measured: Apple M1, engine source of `main` at `463ff2d0`;
workload: example_remove_retention; artifact
[`results/step0a_retention.json`](results/step0a_retention.json)):

| Flavour | drained B/key | fresh B/key | R |
|---|---|---|---|
| `ExpanseSet` | 27.08 | 8.21 | **3.297** |
| `ExpanseMap` | 29.90 | 17.58 | **1.700** |

**Verdict: the gate is met for both flavours.** The map clears the floor by
less than the set: a map leaf carries 8 B of value per slot, so its fresh build
is larger and the retained branches are a smaller share of the drained tree.
The full grid is in [`README.md`](README.md) §1.

## 4. The model

`scripts/condense_bounds.py` (commit 1) holds the byte model. Its self-test
reads every constant, node size and re-implemented function body from
`crates/expanse/src` and fails when the engine moves; `remove_retention.rs`
builds the pinned shapes on the engine and asserts the same bytes
(`model_pins` in the artifact). Pinned values (derived, and engine-checked
where marked):

| Quantity | Value | Engine-checked |
|---|---|---|
| `Edge`, `BranchL3`, `BranchL7`, `BranchB`, `BranchU` | 16, 64, 128, 128, 4,160 B | `node.rs` const asserts |
| `Leaf6` of 32 keys, set / map | 192 / 448 B (208 B set with its edge) | `model_pins` |
| 33-key cascade: `BranchB` over 33 single-key children, set and map | 704 B (720 B with its edge) | `model_pins` |
| Drained `BranchB` at 20 keys / fresh leaf of the same keys, set | 512 / 144 B | `model_pins` |
| same, map | 512 / 336 B | `model_pins` |
| Worst packed-leaf / branch, set: `BranchL3` at level 3 over three 7-key immediates | 80 / 64 B = 1.25 (1.20 with the edge) | derived |
| Worst packed-leaf / branch, map: `BranchL3` at level 7 over a 16-key `Leaf6` and one immediate | 368 / 288 B = 1.2778 (1.2632 with the edge) | `model_pins` |

The map's worst case with the edge counted, 1.2632, is the "up to 1.26×" by
which a key-count-only rule would increase memory. **A packed leaf can be
larger than the branch it replaces**, hence the byte-aware rule (§5.1). A
second fact the engine probe established: a leaf is not narrowed below its
parent, so a branch reached through a skip edge packs at its parent's child
level, which is wider (21 keys under a 5-byte prefix below a level-7 parent
build a 144 B `Leaf6`, not an 80 B `Leaf3`; `model_pins`, `skip_edge_fresh_21`).
The byte rule prices the leaf the implementation would actually build.

`predicted_retention` models a uniform random 64-bit drain from Poisson
occupancy and binomial thinning; its approximations are listed in its
docstring. Its fit to the random @64 cells is in `README.md` §1. **It is not a
blind prediction**: the model and its pins were written after an earlier run
of the Step 0a grid. It is used here only to predict the arms (§8), which no
run has measured.

## 5. The design under test (phase 3)

### 5.1 Rule

On a remove that leaves a branch subtree with population P, where P is one of
the arm's evaluation points (§5.2), compare the subtree's accounted bytes with
the bytes of the same P keys packed as one leaf (or immediate) at the level a
fresh build would place it. **Condense only when the packed form is strictly
smaller** (`condense_saves`). P is read from the branch edge's `pop0`, which the
remove walk already maintains; no walk counts keys.

### 5.2 The two arms

Both are pre-registered now and both are measured; neither threshold changes
after results are seen. Each is derived from `LEAF_CAP`, never a literal
(§2.1 invariant 6). H is the arm's distance below `LEAF_CAP`
(`hysteresis`), and one full insert/remove cycle across the band is
2(H + 1) operations.

| Arm | Threshold T | H | Evaluation points |
|---|---|---|---|
| `H1` (Judy's one index of hysteresis) | `LEAF_CAP - 1` = 31 | 1 | 31, 24, 16, 12, 8, 4, 2, 1 |
| `wide` (the 24-slot class) | `LEAF_CAP - 8` = 24 | 8 | 24, 16, 12, 8, 4, 2, 1 |

**How "evaluate only at slot-class boundaries" is read, and why.** The
maintainer's decision lists the class sizes 32, 24, 16, 12, 8, 4, 2, 1. A
subtree cannot be evaluated at 32 under either arm (both thresholds are below
it). Read strictly (evaluate only at P with `cap_class(P) == P`, at or below
T), both arms first evaluate at 24 and at every point after it, so the two arms
would be one arm (`class_tops`, pinned). The points above therefore add the
threshold itself as the arm's first evaluation; every later point is a class
top, where a subtree the byte rule declined is retried because its packed leaf
just got cheaper. A removal lowers P by one, so a subtree that ends at or below
T has passed through T. This reading interprets the decision rather than
restating it; it was reviewed and adopted before this document was locked.

### 5.3 Shared trees

- An optimistic (OLC) remove never condenses.
- A serialized remove condenses only when its key's top digit is clean in
  `DirtyDigits` (`crates/expanse/src/sync.rs`): under a dirty digit the
  ancestor `pop0` counts are stale, and the rule reads P from them.
- `shrink_to_fit` gains a fold-then-condense sweep: fold the dirty digits'
  counts, then condense every subtree the rule accepts. No remove refolds per
  operation (#1162 recorded that pattern as quadratic).
- Every new mutation entry point carries its `by_mode!` OCC twin (§2.1
  invariant 5) or goes through an existing one.

### 5.4 Rollout

Behind a cargo feature, `subtree-condense`, off by default in phase 3, with
the arm selected by its threshold. An arm that is promoted is promoted under
AGENTS.md §2.7, with an `ablation-no-subtree-condense` inverse.

## 6. Gates

Stated verbatim with their falsifiers. Each is evaluated per arm, feature on,
against `main` at the commit the phase 3 branch is based on.

**G-mem.** Headline R ≤ 1.10; every cell ≤ 1.25; no cell worse than main by
more than 1%. Evaluated on the full Step 0a grid (`remove_retention.rs`, every
cell, both flavours).
*Falsifier:* the headline cell's R above 1.10 in either flavour; any cell's R
above 1.25; any cell whose drained `mem_used()` exceeds main's for the same
cell by more than 1%. Deterministic: one run decides.

**G-ins.** Callgrind arms outside the target set within +0.1%; each target
arm's change attributed with `callgrind_annotate`; any target arm above +5%
blocks promotion. Measured by the `instruction-counts` and `callgrind-smoke`
jobs with the feature on; attribution per §6 of AGENTS.md
(`callgrind_annotate --inclusive=no` per function and `--auto=yes` per line,
base against head).
*Falsifier:* any arm outside the target set above +0.1%; any target arm above
+5%; any target arm whose change is not attributed.

**G-thrash.** On `set_subtree_boundary_oscillate` (§7.3), for each input, the
arm's instructions per operation are at most

    baseline + (C_split + C_condense) / (2 (H + 1))

(`condense_bounds.thrash_bound`), where *baseline* is main's instructions per
operation on the same input, H is the input's band (§7.3), and C_split and
C_condense are the per-event costs from the isolating arm pairs of §7.4
(`condense_bounds.isolated_cost`), measured in the same CI run.
*Falsifier:* either input above its bound.

**G-valid.** The structural validator, the proptest model suite
(`PROPTEST_CASES=500`), the CI Tier-1 Miri filter and the ASan job are green
with the feature on, and the same with it off.
*Falsifier:* any failure in any of them.

**Promotion.** An arm is promotable only if it meets all four gates. If both
meet all four, the arm promoted is the one with the lower sum of Callgrind
instruction counts over `set_remove_partial` and `map_remove_partial` (§7.2),
measured on the same CI run; if the two sums are equal, `H1` is promoted. If
neither arm meets all four, nothing is promoted and the result is recorded as
a negative.

## 7. Instruments

### 7.1 Memory

`crates/expanse/examples/remove_retention.rs`, the Step 0a grid unchanged
(README §1), run on `main` and on each arm.

### 7.2 Instruction counts: the target set

The arms whose measured body removes keys through the 64-bit remove walks
(`crates/expanse/benches/instructions.rs` and the C ABI harness; names as in
`docs/BENCHMARKING.md` §"Arm Inventory"): `set_remove`, `map_remove`,
`set_refill`, `map_refill`, `set_oscillate`, `map_oscillate`, `map_churn`,
`strmap_refill`, `strmap_refill_small`, `strmap_oscillate`, `strmap_churn`,
`bytesmap_remove`, `bytesmap_churn`, `blobmap_remove`, `blobmap_churn`,
`sync_set_remove`, `sync_map_remove`, `sync_set_churn`, `sync_map_churn`,
`sync_strmap_remove`, `sync_strmap_churn`, `sync_strmap_churn_short`,
`sync_bytesmap_remove`, `sync_bytesmap_churn`, `sync_blobmap_remove`,
`sync_blobmap_churn`, `judyl_churn_expanse`, `judyl_churn_expanse_dl`, and the
new arms of §7.3 and §7.4. Every other arm, the 32-bit `set32_remove` and
`map32_remove` included, is outside the target set and held to +0.1%.

New arms, added in phase 3 before any engine change (AGENTS.md §6, benchmark
arm prerequisite):

- `set_remove_partial`, `map_remove_partial`: build 200,000 uniform random
  keys at a 60-bit width (λ = 48.8 per 2-byte expanse, the headline λ_N), then
  remove 137,500 in shuffled order down to 62,500 (λ = 15.3, the headline
  λ_M). Counted per remove. On `main` this drains cascaded expanses without
  condensing; under an arm it crosses each expanse's evaluation points.

### 7.3 The oscillation arm

`set_subtree_boundary_oscillate`: a set whose top two levels are `BranchU`
(every 2-byte prefix populated), with E = 1,024 level-6 expanses built to
`LEAF_CAP + 1` keys (cascaded), then insert/remove cycles in every expanse
across a band. Two inputs:

| Input | Band | H |
|---|---|---|
| `band2` | `LEAF_CAP + 1` ↔ `LEAF_CAP - 1` | 1 |
| `band9` | `LEAF_CAP + 1` ↔ `LEAF_CAP - 8` | 8 |

Counted per operation. On `band2` the `wide` arm never reaches its threshold
and is bounded by the same formula with H = 1.

### 7.4 The isolating arm pairs (C_split, C_condense)

Each pair runs the same operation count on the same tree shape; the event arm
performs one structural event per expanse and the control arm none.
`isolated_cost(event, control, E)` is the per-event cost; a control above its
event arm is a defect in the pair, not a zero.

- `set_subtree_split` / `set_subtree_split_control`: E = 1,024 level-6
  expanses at `LEAF_CAP` keys (a full 32-slot leaf) / at `LEAF_CAP - 1` keys;
  insert one key into each. The event arm cascades every expanse; the control
  inserts into the same slot class with no cascade.
- `set_subtree_condense` / `set_subtree_condense_control`: E = 1,024 drained
  level-6 expanses held as a `BranchB` of single-key children at T + 1 keys /
  at T + 2 keys, T the arm's threshold; remove one key from each. Under an arm
  the event arm condenses every expanse and the control condenses none. On
  `main` neither condenses, so C_condense is measured only with the feature on.

## 8. Expected losses and predictions

Predictions are from `condense_bounds.py` (derived) unless marked otherwise.
They are recorded before any arm exists, so an outcome cannot be reconciled
into them (§8.7).

| Arm | Prediction | Consequence for the gates |
|---|---|---|
| both | headline R: `H1` 1.000 (set) / 1.000 (map); `wide` 1.048 / 1.013 | G-mem headline met by both, if the model holds |
| `wide` | 3.2M → 2M (`r64_to_2m`): R 1.511 (set), 1.290 (map); every expanse that ends at 25–32 keys is never evaluated and keeps its branch | **predicted to fail G-mem** ("every cell ≤ 1.25") in both flavours |
| `H1` | `r64_to_2m`: R 1.068 (set), 1.091 (map) | within G-mem |
| both | `r64_2m_to_1m`, `r64_4m_to_1m`: `H1` 1.000 / 1.000; `wide` 1.048 / 1.012 (2M) and 1.048 / 1.014 (4M) | within G-mem |
| both | `r62` map (R 1.222 on main): the excess is linear-leaf bytes, not branch bytes (README §1), and its expanses keep about 61 keys, so no arm evaluates them | unchanged; within 1.25 on main already |
| both | the range, sequential, sparse and clustered cells are at R = 1.000 on main | G-mem needs them within 1% of main |
| both | the target-set arms add a population check at evaluation points and the condense itself | increases expected on the target set; their size is unmeasured, bounded only by G-ins's 5% |
| both | G-thrash: the bound admits one cascade and one condense per cycle; whether the arm's per-operation cost beyond those two events (the evaluation check, and leaf against branch operation cost inside the band) is positive or negative is unmeasured | an empirical residual; no prediction |
| both | a shared tree drained only by optimistic removes condenses nothing until `shrink_to_fit` | by design (§5.3); the Step 0a grid measures plain trees only, so G-mem does not cover shared trees |

**`mem_held()` is not gated, and condensing is not expected to fix it.** On
`main`, `mem_held()` after `shrink_to_fit()` on the headline drained set is
8.218 × the fresh build's `mem_used()`, and 3.360 × on `r64_range`, where R is
1.000 (measured: Apple M1, engine at `463ff2d0`; workload:
example_remove_retention; `results/step0a_retention.json`). Branch retention
does not explain what the drained tree holds after `shrink_to_fit()`. One
candidate is that `shrink_to_fit()` releases only slab pages with no live node
on them; that this accounts for the figures is a hypothesis, not measured here.
Condensing frees `BranchB` nodes and edge subarrays; whether that empties
whole pages is not modelled. It is reported, not gated.

## 9. What wall clock would add (secondary; reference host; later)

None of the gates above is a wall-clock gate. A later wall-clock pass on the
reference host, under §8.4 (BCa 95% intervals, load snapshots, the core pin),
would add: remove latency on the `*_remove_partial` workloads; operations per
second on the oscillation inputs, where Callgrind counts instructions but not
the allocation and cache cost of a rebuild; and point-lookup latency on a
drained tree against the same tree condensed, at a 50% hit rate with misses
drawn from the population's generator (§8.6), where a denser tree may be
faster. Its outcome would be reported, not used to change a gate above.

## 10. What was seen before this document was written

- The Step 0a artifact, every cell (§3, README §1), and an earlier run of the
  same grid at `d3c67152` on another 64-bit host, whose shared cells read the
  same bytes; that run is not published.
- The model's values of §4 and its fit to the measured cells.
- That the strict "class boundary only" reading makes the two arms identical
  (§5.2), which is why the threshold-first reading was adopted.
- No arm has been built and no condensing code exists.

## 11. Empirical residuals

What only measurement can decide: the instruction cost of the evaluation check
on the remove path; C_split and C_condense; each arm against `band2` and
`band9`; the cells the model does not cover (@62, @56, the construction-fixed
distributions, range removals) under each arm; the shared-tree paths; and
`mem_held()` after `shrink_to_fit()` under each arm.

## 12. Pre-registration: an explicit `compact()`

A separate pre-registration (AGENTS.md §8.8 commit 2), appended after phase 3
was evaluated. It does not amend §1–§11, which stay frozen as the record of
the condensing design; this section is frozen once merged in the same way.
Outcomes are reported in [`README.md`](README.md) §4, never reconciled into
this section, and a threshold, method or sample changed after results are
seen relabels the result `INTERMEDIATE` (§8.19).

### 12.1 The question

**Does an explicit `compact()` that rebuilds a tree's surviving keys into a
new allocator bring `mem_held()` after bulk deletes to within 10% of a fresh
build of the same keys, on every grid cell, at a peak and an instruction cost
no worse than the rebuild measured in README §3?**

### 12.2 What was seen before this section was written

- README §3 and its artifact [`results/census_rebuild.json`](results/census_rebuild.json)
  (measured: Apple M1, `a154bc57`; workload: example_remove_retention): after
  `shrink_to_fit()` the headline set holds 16,214 slab pages where its live
  blocks fit on 6,499, at 0.392 slab occupancy, and its rebuild (`clone()`,
  then drop of the drained tree) holds 1,782 pages at 0.979. Held ÷ held_fresh
  reads 5.318 (set) and 3.164 (map) after `shrink_to_fit()` on the headline
  cell, and 0.658–0.854 rebuilt on every uniform random cell, 0.577–1.000 on
  the construction-fixed cells. The rebuild's peak held was 1.110–1.786 ×
  `held_shrunk` on the uniform random cells.
- The rebuild arms (measured: CI `instruction-counts`, x86_64 Callgrind,
  run 36205856578 at `f9782220`): `set_rebuild_drained/random60` 16,960,355
  instructions (271.4 per surviving key), `map_rebuild_drained/random60`
  40,329,898 (645.3 per surviving key). The set's `Clone` is
  `from_sorted_iter`; the map's is `iter().collect()`, an ascending insert,
  because the map has no bulk builder.
- That artifact's `seq_range` set cell: `from_sorted_iter` builds 64,832 B
  where the fresh insert build uses 65,792 B (README §3.1). `mem_used()` order
  invariance (`tests/test_mem_used_order_invariant.rs`) covers insert-built
  trees, not the bulk builder, which may emit the more compact `FullExpanse`.
- The predictions of §12.4, computed from that artifact before any
  `compact()` code existed. No `compact()` code, arm or measurement existed
  when this section was written.

### 12.3 The design under test

**API.** `ExpanseSet::compact(&mut self)` and `ExpanseMap::compact(&mut self)`,
no return value, on the 64-bit engine. Each collects the tree's ascending
iteration into a buffer of exact capacity, builds a new tree from it into a
new allocator, swaps the new tree in, and drops the old one.

- **Set.** The buffer is a `Vec<u64>`; the build is the one
  `from_sorted_iter` already uses (`ExpanseSet::from_sorted_keys`,
  `algebra_build::build_subtree`), without its sortedness check, since the
  iteration is ascending by construction.
- **Map.** The buffer is a `Vec<(u64, u64)>`; the build is a new map bulk
  builder that emits, bottom-up and without intermediate forms, the terminal
  and branch forms the map insert path converges to: an immediate up to
  `map_immed_max(kb)` entries, a linear leaf up to `LEAF_CAP` (`LEAF1_CAP` at
  level 1), a `LeafBitmapL` at level 1 or behind a skip where the keys differ
  only in their final byte, and otherwise a branch at the divergence level in
  the form its child count implies. `Clone` and `FromIterator` are not changed
  by this work; routing them through the builder is a follow-up that must hold
  their own arms (`map_clone`) to the review threshold.

**Contract** (rustdoc on both methods). Every node moves: every value pointer
(`get_slot_ptr`, `get_value_slot`, `ins_slot`) and every pointer derived from
the tree's nodes is invalidated, and no iterator or cursor can be live across
the call (it takes `&mut self`). The cost is O(n) in the population. The call
holds the old tree, the new tree and the key buffer at once: a transient peak
of about old + new held bytes, plus 8 B per key (set) or 16 B per entry (map).
`shrink_to_fit()` is the non-moving alternative and keeps its contract that
nothing moves. `compact()` pays off after deleting most of a tree's keys.

**Shared trees.** On a tree whose allocator is deferred to a collector (shared
through a `Sync*` wrapper) `compact()` does nothing, as `shrink_to_fit()`
returns 0 there. No public API hands out `&mut ExpanseSet` or
`&mut ExpanseMap` for a shared tree, so the guard is defensive. `compact()` is
not a tree mutation walk: it never writes a node of the old tree, and it
writes only a private allocator that no reader can reach until the swap on
`&mut self`, so there is no in-place write for an OCC twin to bracket
(AGENTS.md §2.1 invariant 5). Compacting a shared tree needs writer quiesce
and a collector-aware swap; it is out of scope.

**Out of scope, as follow-ups.** The `Sync*` wrappers; the 32-bit engine;
`ExpanseStrMap`, `ExpanseBytesMap` and `ExpanseBlobMap`; the C ABI (no
`expanse_*_compact` symbol until a consumer asks).

### 12.4 Predictions (`scripts/compact_bounds.py`, derived)

`no_free_held` is the `mem_held()` of an allocator that has only allocated: a
slab class carves a page only when its freelist is empty, so each class holds
ceil(live blocks ÷ blocks per page) pages, and the system-served classes hold
their live bytes. The set builder never frees (its rebuild equals the formula
on all 21 set cells of the artifact, a pinned test); the map builder of §12.3
is written to allocate only. With the fresh build's per-class live blocks the
predicted held ÷ held_fresh is at most 1 on every cell
(`predicted_compact_over_fresh`; derived from `results/census_rebuild.json`,
workload: example_remove_retention):

| Cell | set | map |
|---|---|---|
| `headline` | 0.6595 | 0.7180 |
| `r64_range` | 0.8100 | 0.8394 |
| `seq_range` | 0.5770 | 0.9924 |
| `sparse_shuffled` | 0.9978 | 0.9982 |
| highest over the 21 cells | 0.9978 | 0.9982 |

**These are not blind.** The set's figures are the `from_sorted_iter` rebuild
already measured in README §3; the map's assume the bulk builder emits the
same per-class live blocks as the insert path, which the ascending-insert
rebuild did on all 21 map cells of the artifact.

**Peak.** `peak_no_free`: the old tree is only read until the new one is
complete, and the new allocator never frees during the build, so the peak held
by the two tree allocators is `held_before + held_after`. With the prediction
above it is at most `held_before + held_fresh`, inside G-peak's ceiling.

### 12.5 Gates

Stated verbatim with their falsifiers. Each is evaluated once on the PR head.
`held_fresh` is the fresh build's `mem_held()` without `shrink_to_fit()`, the
denominator of README §3.

**G-held.** After `compact()`, `mem_held()` ÷ held_fresh ≤ 1.10 in every grid
cell, set and map. The map's `mem_used()` after `compact()` equals the fresh
build's in every cell (order invariance). The set's `mem_used()` after
`compact()` equals the `from_sorted_iter` rebuild's (`used_rebuilt`, same run)
in every cell and is at most the fresh build's.
*Falsifier:* any cell above 1.10 in either flavour; any map cell whose
`mem_used()` after `compact()` differs from the fresh build's; any set cell
whose `mem_used()` after `compact()` differs from `used_rebuilt` or exceeds
the fresh build's. Deterministic: one run decides.

**G-peak.** The peak held during `compact()` ≤ held_before + held_fresh ×
1.10, where held_before is `mem_held()` of the tree when `compact()` is called
(the drained tree, without `shrink_to_fit()`). Measured with the allocator
counters: the peak is `held_before + held_after`, which holds when the
compacted tree's allocator made no free during the build (its live allocation
count equals its total allocation count, read from the census).
*Falsifier:* any cell where `held_before + held_after` exceeds the ceiling, or
any cell whose compacted allocator's live allocation count differs from its
total allocation count (the identity above then does not bound the peak, and
the cell is recorded not met).

**G-cost.** The Callgrind `set_compact_drained` and `map_compact_drained` arms
are at or below the matching `set_rebuild_drained` and `map_rebuild_drained`
arms measured in the same CI run (x86_64 `instruction-counts`). A map bulk
builder is allowed and expected to lower the map figure; `Clone` and
`FromIterator` may use it only if their existing arms do not regress, and this
work does not route them through it.
*Falsifier:* either compact arm above its rebuild arm in the same run.

**G-ins.** Every existing Callgrind arm unchanged within the 0.1% review
threshold (AGENTS.md §6), in every Callgrind job of the run
(`instruction-counts`, `callgrind-smoke`); no hot-path change.
*Falsifier:* any existing arm whose count moves by more than 0.1% against
`main` in either direction.

**G-valid.** The structural validator after `compact()` (unit tests and every
instrument cell); a proptest of random operations interleaved with
`compact()`, checked against a model for equality (`PROPTEST_CASES=500` in
CI); unit tests of `compact()` that the CI Tier-1 Miri filter selects, and
that the nightly Miri shard census assigns; and the ASan job. All green.
*Falsifier:* any failure in any of them, or a Tier-1 Miri shard that selects
no `compact` test.

**Verdict.** `compact()` ships only if all five gates are met. A failing gate
is recorded as failed, never re-thresholded.

### 12.6 Expected losses

| Loss | Size | Consequence |
|---|---|---|
| Transient peak | old + new held bytes, plus the key buffer: 8 B per key (set), 16 B per entry (map), `sort_buffer_bytes` | bounded by G-peak for the tree allocators; the buffer is outside both allocators, so G-peak does not count it; it is reported per cell from a counting global allocator and not gated |
| O(n) cost | per surviving key, at most the rebuild arm (G-cost) | a call on a tree that lost few keys pays the whole cost for little return; the rustdoc says when to call it |
| Moved nodes | every value pointer and node-derived pointer | the contract of §12.3; `shrink_to_fit()` stays the non-moving alternative |

No prediction is made for the instruction counts: the set path drops a
sortedness check and a growing buffer from the rebuild, and the map path
replaces one insert per entry with direct emission, but how many instructions
that saves is unmeasured.

### 12.7 Instruments

- **Memory** (G-held, G-peak): `crates/expanse/examples/remove_retention.rs`,
  the Step 0a grid unchanged, with a compact arm per cell: the drained tree is
  built a second time with the same keys and removal order (its `mem_used()`
  and `mem_held()` asserted equal to the first build's), `compact()` is called
  without `shrink_to_fit()`, and the instrument records `held_before`,
  `mem_used()` and `mem_held()` after the call, a census of the compacted
  allocator (with its live and total allocation counts), the validator, and
  the process heap's peak during the call from the counting global allocator.
  Deterministic byte counts: one run on any 64-bit host decides.
- **Instructions** (G-cost, G-ins): `set_compact_drained` and
  `map_compact_drained` in `crates/expanse/benches/instructions.rs`, the
  `*_rebuild_drained` setup unchanged (200,000 random 60-bit keys drained to
  62,500), `compact()` inside the measured region, the compacted tree leaked,
  counted per surviving key (62,500), registered in `scripts/perf_report.py`.

### 12.8 Empirical residuals

What only measurement decides: whether the map bulk builder emits the insert
path's per-class node census on every cell (a unit test checks its total
`mem_used()` against an insert build; the instrument reads the per-class
census); the builder's own scratch on the process heap; every instruction
count; and whether any existing arm moves, which a new function in the same
crate can cause through inlining alone.
