# Remove retention: what a drained tree keeps

What a tree's `mem_used()` is after removals, against a fresh build of the keys
that remain, and whether condensing drained branch subtrees back into packed
leaves closes the gap. The pre-registration (question, arms, gates, expected
losses) is [`METHODOLOGY.md`](METHODOLOGY.md).

| Phase | What | State |
|---|---|---|
| 0 | Step 0a census on `main`, the no-go gate | measured; gate met (§1) |
| 1 | Bound functions, `scripts/condense_bounds.py` | committed; self-test in the `lint` job and `scripts/gate.sh` |
| 2 | Pre-registration, `METHODOLOGY.md` | committed; frozen once merged |
| 3 | Engine change behind `subtree-condense`, the new Callgrind arms, gate evaluation | arms in `crates/expanse/benches/instructions.rs`; engine change behind the feature (off by default); G-mem measured (§2); G-ins, G-thrash and CI's half of G-valid not yet measured |

**Reproduce.** `EXPANSE_COMMIT=<sha> EXPANSE_RUSTC="$(rustc -V)" cargo run --release -p expanse-trie --example remove_retention -- --json docs/benchmarks/remove_retention/results/step0a_retention.json`.
Single-threaded and deterministic: any 64-bit host reproduces every byte at the
same commit. `--quick` runs the grid at 1/32 of the populations as a smoke run
and refuses `--json`. Every run first builds the fixed shapes that
`scripts/condense_bounds.py` pins and panics if the engine reads different
bytes (`model_pins` in the artifact). Peak RSS of the full grid was 430 MB
(measured: Apple M1, `/usr/bin/time -l`), so the headline cell at
N = 3.2M runs on a laptop and no substitute cell was needed.

## 1. Step 0a: retention on `main`

`R = mem_used(insert N, remove down to M) / mem_used(fresh insert of the same M keys)`.
A fresh build's `mem_used()` does not depend on insertion order
(`crates/expanse/tests/test_mem_used_order_invariant.rs`), so it is the
canonical denominator. Removal orders: `shuffled` (a Fisher–Yates
permutation), `sorted` (the same removed set, ascending), `range` (the N − M
smallest keys, ascending). The last two columns are `mem_held()` after
`shrink_to_fit()` on the drained tree, divided by the fresh build's
`mem_used()`.

(measured: Apple M1, macOS, rustc 1.98.1, engine source of `main` at
`463ff2d0`; workload: example_remove_retention; artifact
[`results/step0a_retention.json`](results/step0a_retention.json). Exact byte
counts with no interval: `mem_used()` is deterministic accounting. Re-run on
the same host at `407f73b2`, after the remove-path demotion thresholds were
rewritten as derived constants with unchanged values: all 42 cells
byte-identical.)

| Cell | Keys | N → M | Removal | set R | map R | set B/key drained / fresh | map B/key drained / fresh | set held after shrink ÷ fresh used | map held after shrink ÷ fresh used |
|---|---|---|---|---|---|---|---|---|---|
| `headline` | random@64 | 3,200,000 → 1,000,000 | shuffled | **3.297** | **1.700** | 27.08 / 8.21 | 29.90 / 17.58 | 8.218 | 4.440 |
| `r64_sorted` | random@64 | 3,200,000 → 1,000,000 | sorted | 3.297 | 1.700 | 27.08 / 8.21 | 29.90 / 17.58 | 7.696 | 4.181 |
| `r64_range` | random@64 | 3,200,000 → 1,000,000 | range | 1.000 | 1.000 | 20.99 / 20.99 | 23.89 / 23.89 | 3.360 | 3.339 |
| `r64_2m_to_1m` | random@64 | 2,000,000 → 1,000,000 | shuffled | 1.915 | 1.251 | 15.72 / 8.21 | 22.00 / 17.58 | 4.231 | 2.210 |
| `r64_4m_to_1m` | random@64 | 4,000,000 → 1,000,000 | shuffled | 3.309 | 1.739 | 27.17 / 8.21 | 30.57 / 17.58 | 8.223 | 4.794 |
| `r64_to_2m` | random@64 | 3,200,000 → 2,000,000 | shuffled | 1.706 | 1.328 | 23.42 / 13.73 | 26.26 / 19.78 | 2.811 | 2.196 |
| `r64_to_320k` | random@64 | 3,200,000 → 320,000 | shuffled | 2.991 | 1.695 | 33.43 / 11.18 | 36.25 / 21.39 | 8.909 | 5.972 |
| `r64_1m_to_312k` | random@64 | 1,000,000 → 312,500 | shuffled | 1.029 | 1.006 | 11.58 / 11.25 | 21.61 / 21.47 | 1.926 | 1.298 |
| `r62` | random@62 | 3,200,000 → 1,000,000 | shuffled | 1.005 | 1.222 | 19.78 / 19.68 | 28.33 / 23.18 | 1.603 | 2.821 |
| `r56` | random@56 | 3,200,000 → 1,000,000 | shuffled | 3.769 | 1.806 | 27.07 / 7.18 | 29.89 / 16.56 | 9.351 | 4.691 |
| `r56_sorted` | random@56 | 3,200,000 → 1,000,000 | sorted | 3.769 | 1.806 | 27.07 / 7.18 | 29.89 / 16.56 | 8.759 | 4.418 |
| `r56_range` | random@56 | 3,200,000 → 1,000,000 | range | 1.000 | 1.000 | 21.00 / 21.00 | 23.81 / 23.81 | 3.382 | 3.339 |
| `seq_shuffled` | sequential | 3,200,000 → 1,000,000 | shuffled | 1.000 | 1.000 | 1.00 / 1.00 | 11.03 / 11.03 | 1.019 | 1.730 |
| `seq_sorted` | sequential | 3,200,000 → 1,000,000 | sorted | 1.000 | 1.000 | 1.00 / 1.00 | 11.03 / 11.03 | 1.019 | 1.023 |
| `seq_range` | sequential | 3,200,000 → 1,000,000 | range | 1.000 | 1.000 | 0.07 / 0.07 | 8.56 / 8.56 | 1.104 | 1.004 |
| `sparse_shuffled` | sparse | 3,200,000 → 1,000,000 | shuffled | 1.000 | 1.000 | 20.25 / 20.25 | 20.25 / 20.25 | 1.024 | 1.024 |
| `sparse_sorted` | sparse | 3,200,000 → 1,000,000 | sorted | 1.000 | 1.000 | 20.25 / 20.25 | 20.25 / 20.25 | 1.017 | 1.017 |
| `sparse_range` | sparse | 3,200,000 → 1,000,000 | range | 1.000 | 1.000 | 16.31 / 16.31 | 16.31 / 16.31 | 1.000 | 1.000 |
| `clust_shuffled` | clustered | 3,200,000 → 1,000,000 | shuffled | 1.000 | 1.000 | 1.13 / 1.13 | 11.15 / 11.15 | 1.073 | 1.726 |
| `clust_sorted` | clustered | 3,200,000 → 1,000,000 | sorted | 1.000 | 1.000 | 1.13 / 1.13 | 11.15 / 11.15 | 1.073 | 1.024 |
| `clust_range` | clustered | 3,200,000 → 1,000,000 | range | 1.000 | 1.000 | 0.35 / 0.35 | 8.60 / 8.60 | 3.428 | 1.168 |

**Gate verdict: met.** The no-go floor was headline R ≥ 1.5. The set reads
3.297 and the map 1.700 (same artifact), so the work may proceed to phase 3
under the pre-registration.

What the table shows, and does not:

- Retention appears where expanses cascaded at N and hold fewer than
  `LEAF_CAP` keys at M: the uniform random cells whose N puts λ above 32 per
  2-byte expanse (@64 at 2M, 3.2M and 4M) or per 3-byte expanse (@56 at 3.2M).
  Where λ stays under 32 at N (1M @64, λ = 15.3), set R is 1.029 and map R
  1.006. Where the survivors still exceed `LEAF_CAP` (@62, λ_M = 61), set R is
  1.005; the @62 map is the exception below.
- The headline set's drained bytes are mostly bitmap branches: 25,919,232 of
  27,076,928 B are in 64,649 `BranchB` nodes and their edge subarrays, where
  the fresh build of the same keys holds 65,533 linear leaves and 2 `BranchB`
  (artifact, `headline` cell, `drained` / `fresh` census).
- Removal order does not move `mem_used()` on these cells: `shuffled` and
  `sorted` remove the same set and read the same bytes. `mem_held()` after
  `shrink_to_fit()` does differ between them.
- Range removal leaves R at 1.000 on every distribution: the survivors are a
  dense key range, and a fresh build of it cascades the same expanses.
- The `r62` map (R 1.222, set 1.005) keeps its excess in linear leaves, not
  branches: drained and fresh `BranchB` bytes differ by 704 B, and linear
  leaves hold 8,656,112 B drained against 3,514,816 B fresh (423,273 leaves
  against 101,916). That is consistent with the map remove path keeping a
  leaf a leaf down to its last key (`crates/expanse/src/mutate_map.rs`,
  `map_remove`), where a fresh build stores a lone five-byte key as an
  immediate (`map_immed_max(5)` = 1): 787,799 immediates fresh against
  466,470 drained.
  Branch condensation is not predicted to change it (METHODOLOGY §8).
- `mem_held()` after `shrink_to_fit()` exceeds the fresh build's `mem_used()`
  on cells where R is 1.000 (`r64_range` set 3.360), so branch retention does
  not explain it. Its cause is not measured here; it is reported and not
  gated (METHODOLOGY §8).
- The model in `scripts/condense_bounds.py` against these cells: its values
  were fixed after an earlier run of this grid, so agreement is not a blind
  test (METHODOLOGY §4).

| Cell | λ_N → λ_M | set R model | set R measured | map R model | map R measured |
|---|---|---|---|---|---|
| `headline` | 48.8 → 15.3 | 3.291 | 3.297 | 1.697 | 1.700 |
| `r64_2m_to_1m` | 30.5 → 15.3 | 1.749 | 1.915 | 1.160 | 1.251 |
| `r64_4m_to_1m` | 61.0 → 15.3 | 3.309 | 3.309 | 1.739 | 1.739 |
| `r64_to_2m` | 48.8 → 30.5 | 1.584 | 1.706 | 1.316 | 1.328 |
| `r64_to_320k` | 48.8 → 4.9 | 2.987 | 2.991 | 1.693 | 1.695 |
| `r64_1m_to_312k` | 15.3 → 4.8 | 1.000 | 1.029 | 1.000 | 1.006 |

## 2. Phase 3 results

The gates, arms and predictions are fixed in [`METHODOLOGY.md`](METHODOLOGY.md)
§5–§8 and are not restated here. Nothing is promoted; the feature stays off by
default.

**Build under test.** `subtree-condense` (the `H1` arm, threshold
`LEAF_CAP - 1`) and `subtree-condense-wide` (the `wide` arm, `LEAF_CAP - 8`),
`crates/expanse/src/condense.rs`, against `main` at the commit the phase 3
branch is based on.

### 2.1 G-mem

The Step 0a grid, unchanged, on `main` and on each arm (measured: Apple M1,
macOS, rustc 1.98.1; `main` at `86adbf15`, both arms at `1e2b31df`;
workload: example_remove_retention; artifacts
[`results/phase3_retention_main.json`](results/phase3_retention_main.json),
[`results/phase3_retention_h1.json`](results/phase3_retention_h1.json),
[`results/phase3_retention_wide.json`](results/phase3_retention_wide.json).
Exact byte counts with no interval. `main` was built from the tree of #1192's head, identical to `86adbf15`'s. The `main` run reproduces every cell's
`used_drained` of `results/step0a_retention.json` to the byte, and every
cell's fresh build reads the same bytes in all three builds.)

Columns: R per build, and each arm's drained `mem_used()` divided by
`main`'s for the same cell (the "no cell worse than main by more than 1%"
clause). Cells where all three builds read R = 1.000 and the same drained bytes
are the 22 rows of `r64_range`, `r56_range` and the nine sequential, sparse and
clustered cells, in both flavours; they are in the artifacts and omitted
here.

| Cell | Flavour | `main` R | `H1` R | `H1` ÷ `main` | `wide` R | `wide` ÷ `main` |
|---|---|---|---|---|---|---|
| `headline` | set | 3.297 | **1.001** | 0.3035 | **1.046** | 0.3174 |
| `headline` | map | 1.700 | **1.000** | 0.5883 | **1.013** | 0.5958 |
| `r64_sorted` | set | 3.297 | 1.001 | 0.3035 | 1.046 | 0.3174 |
| `r64_sorted` | map | 1.700 | 1.000 | 0.5883 | 1.013 | 0.5958 |
| `r64_2m_to_1m` | set | 1.915 | 1.000 | 0.5223 | 1.045 | 0.5457 |
| `r64_2m_to_1m` | map | 1.251 | 1.000 | 0.7992 | 1.011 | 0.8082 |
| `r64_4m_to_1m` | set | 3.309 | 1.000 | 0.3023 | 1.048 | 0.3166 |
| `r64_4m_to_1m` | map | 1.739 | 1.000 | 0.5750 | 1.014 | 0.5830 |
| `r64_to_2m` | set | 1.706 | 1.087 | 0.6372 | **1.586** | 0.9297 |
| `r64_to_2m` | map | 1.328 | 1.062 | 0.8000 | **1.274** | 0.9597 |
| `r64_to_320k` | set | 2.991 | 1.027 | 0.3433 | 1.027 | 0.3433 |
| `r64_to_320k` | map | 1.695 | 1.006 | 0.5932 | 1.006 | 0.5932 |
| `r64_1m_to_312k` | set | 1.029 | 1.029 | 0.9995 | 1.029 | 0.9995 |
| `r64_1m_to_312k` | map | 1.006 | 1.006 | 0.9998 | 1.006 | 0.9998 |
| `r62` | set | 1.005 | 1.005 | 1.0000 | 1.005 | 1.0000 |
| `r62` | map | 1.222 | 1.222 | 1.0000 | 1.222 | 1.0000 |
| `r56` | set | 3.769 | 1.000 | 0.2654 | 1.056 | 0.2802 |
| `r56` | map | 1.806 | 1.000 | 0.5538 | 1.015 | 0.5623 |
| `r56_sorted` | set | 3.769 | 1.000 | 0.2654 | 1.056 | 0.2802 |
| `r56_sorted` | map | 1.806 | 1.000 | 0.5538 | 1.015 | 0.5623 |

**Verdicts, against the locked falsifier (one run decides):**

- `H1`: **G-mem met.** Headline R 1.001 (set) and 1.000 (map), at most 1.10;
  highest cell R 1.222 (`r62` map, unchanged from `main`), at most 1.25; no
  cell's drained bytes above `main`'s (highest ratio 1.0000).
- `wide`: **G-mem not met.** The headline clause holds (1.046 / 1.013), and
  no cell is worse than `main`, but `r64_to_2m` reads R 1.586 (set) and
  1.274 (map), above the 1.25 ceiling. This is the loss §8 predicted (1.511 /
  1.290). `wide` is therefore not promotable, whatever the other gates read.

Against the §8 predictions (derived, `condense_bounds.py`): headline `H1`
1.000 / 1.000 predicted, 1.001 / 1.000 measured; `wide` 1.048 / 1.013
predicted, 1.046 / 1.013 measured; `r64_to_2m` `H1` 1.068 / 1.091 predicted,
1.087 / 1.062 measured. `r62` map is unchanged at 1.222, as predicted.

**Recorded, not gated.** The `worst_l3_16_1` model shape (METHODOLOGY §4)
reads 368 B drained under both arms against 288 B on `main`: the drain passes
an evaluation point where the packed leaf is the smaller form, and the leaf it
builds is then drained as a leaf to 17 keys, where the branch `main` keeps is
smaller. The byte rule decides at the evaluation point only. No grid cell
shows this (every arm-to-`main` ratio is at most 1.0000), but it is a shape
where an arm holds more than `main`.

`mem_held()` after `shrink_to_fit()` over the fresh build's `mem_used()`, on
the headline cell (reported, not gated, METHODOLOGY §8): `main` 8.218 (set) /
4.440 (map), `H1` 2.132 / 1.256, `wide` 3.969 / 2.539. On `r64_to_2m` it is
higher under `H1` than on `main` (2.839 against 2.811 for the set, 2.425
against 2.196 for the map); the cause is not measured.

### 2.2 G-ins, G-thrash, G-valid

- **G-ins, G-thrash: not measured.** Both are measured by the
  `instruction-counts` and `callgrind-smoke` jobs with the feature on. Those
  jobs build the benches with default features, and a draft pull request runs
  only the fast lane, so no CI run has measured either arm yet. The base-side
  counts for the new arms come from the arms' own pull request.
- **G-valid: partly evaluated.** Local, with the feature on (`PROPTEST_CASES=500`):
  `cargo test -p expanse-trie --features subtree-condense,diag-entry` and the
  same with `subtree-condense-wide`, 646 passed and 0 failed each, including
  the proptest model suites, the validator on every drained tree, and
  `tests/test_subtree_condense.rs`. The CI Tier-1 Miri filter and the ASan job
  run with default features only, so their feature-on half has not run.
  Verdict withheld until it does.

**Promotion:** none. `wide` fails G-mem; `H1` has three gates outstanding.
