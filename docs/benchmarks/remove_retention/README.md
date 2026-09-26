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
| 3 | Engine change behind `subtree-condense`, the new Callgrind arms, gate evaluation | evaluated; **negative**, nothing promoted (§2); the engine code is kept at tag `poc/subtree-condense`, not on `main` |

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

## 2. Phase 3 result: negative

**Verdict (METHODOLOGY §6, Promotion): neither arm meets all four gates, so
nothing is promoted and the result is recorded as a negative.** `H1` meets
G-mem and G-thrash and fails G-ins. `wide` fails G-mem, the loss METHODOLOGY
§8 predicted. The gates, arms and predictions are those locked in
[`METHODOLOGY.md`](METHODOLOGY.md) §5–§8; none was changed after results were
seen.

**The code measured is not on `main`.** It is kept at the signed tag
`poc/subtree-condense` (`a17aad16`), so every provenance commit below stays
reachable:

| Commit | What |
|---|---|
| `0c8fd639` | the engine change behind `subtree-condense` (`H1`) and `subtree-condense-wide` (`wide`), `crates/expanse/src/condense.rs` |
| `1e2b31df` | the memory instrument run on a condensing build (the G-mem artifacts' arm commit) |
| `a17aad16` | measurement only: `subtree-condense` made a default feature so that CI's Callgrind, Miri and ASan jobs build the `H1` arm |

The base for every comparison is `main` at `86adbf15`, the commit the phase 3
branch was based on. Only `H1` was built by CI; `wide` has no G-ins, G-thrash
or CI G-valid measurement, and fails G-mem regardless.

| Gate | `H1` | `wide` |
|---|---|---|
| G-mem | **met** | **not met** (`r64_to_2m` above 1.25) |
| G-ins | **not met** (target arms above +5%, non-target arms above +0.1%, no attribution) | not measured |
| G-thrash | **met** on both inputs | not measured |
| G-valid | green as run; the Tier-1 Miri filter does not select the new `condense::` tests (§2.4) | local run only |

### 2.1 G-mem

The Step 0a grid, unchanged, on `main` and on each arm (measured: Apple M1,
macOS, rustc 1.98.1; `main` at `86adbf15`, both arms at `1e2b31df`;
workload: example_remove_retention; artifacts
[`results/phase3_retention_main.json`](results/phase3_retention_main.json),
[`results/phase3_retention_h1.json`](results/phase3_retention_h1.json),
[`results/phase3_retention_wide.json`](results/phase3_retention_wide.json).
Exact byte counts with no interval. The `main` run reproduces every cell's
`used_drained` of `results/step0a_retention.json` to the byte, and every
cell's fresh build reads the same bytes in all three builds.)

Columns: R per build, and each arm's drained `mem_used()` divided by
`main`'s for the same cell (the "no cell worse than main by more than 1%"
clause). The 22 rows omitted here (`r64_range`, `r56_range` and the nine
sequential, sparse and clustered cells, both flavours) read R = 1.000 and the
same drained bytes in all three builds; they are in the artifacts.

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

- `H1`: **met.** Headline R 1.001 (set) and 1.000 (map), at most 1.10;
  highest cell R 1.222 (`r62` map, unchanged from `main`), at most 1.25; no
  cell's drained bytes above `main`'s (highest ratio 1.0000).
- `wide`: **not met.** The headline clause holds (1.046 / 1.013) and no cell
  is worse than `main`, but `r64_to_2m` reads R 1.586 (set) and 1.274 (map),
  above the 1.25 ceiling. METHODOLOGY §8 predicted this loss at 1.511 / 1.290
  (derived, `condense_bounds.py`).

Against the other §8 predictions (derived): headline `H1` 1.000 / 1.000
predicted, 1.001 / 1.000 measured; headline `wide` 1.048 / 1.013 predicted,
1.046 / 1.013 measured; `r64_to_2m` `H1` 1.068 / 1.091 predicted, 1.087 /
1.062 measured; `r62` map unchanged at 1.222, as predicted.

### 2.2 G-ins

`instruction-counts` job, head `a17aad16` (the `H1` arm on by default) against
`main_base` built at `86adbf15` (measured: GitHub-hosted `ubuntu-latest`
runner, Callgrind; workload: core_instructions; run
<https://github.com/orieg/expanse/actions/runs/36192522024>). Callgrind counts
are exact integers and carry no interval. Target set as listed in METHODOLOGY
§7.2; every other arm is non-target.

Target arms above +5% (each blocks promotion):

| Arm | `main` | `H1` | Change |
|---|---|---|---|
| `map_remove_partial/random60` | 69,718,456 | 136,448,376 | +95.71% |
| `set_remove_partial/random60` | 78,397,685 | 142,288,516 | +81.50% |
| `set_remove/random` | 23,743,408 | 30,336,593 | +27.77% |
| `map_remove/random` | 23,111,842 | 28,062,976 | +21.42% |
| `blobmap_remove/random` | 25,990,221 | 30,488,472 | +17.31% |
| `set_refill/random` | 44,410,698 | 51,030,330 | +14.91% |
| `map_refill/random` | 46,949,847 | 51,917,263 | +10.58% |
| `set_subtree_condense_control/h1` | 646,179 | 743,459 | +15.05% |
| `set_subtree_condense/h1` | 640,004 | 15,970,726 | +2,395.41% |
| `set_subtree_boundary_oscillate/band2` | 16,997,105 | 221,035,697 | +1,200.43% |
| `set_subtree_boundary_oscillate/band9` | 71,126,513 | 272,674,769 | +283.37% |
| `set_subtree_condense/wide` | 528,562 | 802,379 | +51.80% |

The last four are the G-thrash instruments of §7.3–§7.4, in the target set by
§7.2; a condense arm is expected to rise under condensing, and G-thrash, not
the +5% line, is the gate that prices it. The seven arms above them are not
instruments of that kind. Under the `H1` build the `wide` inputs of the
condense pair were already condensed during setup (their drain from
`LEAF_CAP + 1` passes 31), so `set_subtree_condense/wide` (+51.80%) and its
control (−9.11%, 554,077 → 503,578) time leaf removes against `main`'s
branch removes and isolate nothing.

Target arms between +0.1% and +5%: `bytesmap_remove/routes` +4.77%,
`sync_set_remove/random` +3.14%, `map_churn/random` +2.08%,
`sync_map_remove/random` +2.01%, `blobmap_churn/random` +1.13%,
`sync_map_churn/leaf` +0.98%, `bytesmap_churn/routes` +0.89%,
`strmap_refill/routes` +0.52%, `strmap_refill_small/routes` +0.49%,
`sync_map_churn/random` +0.25%, `sync_set_churn/leaf` +0.10% (97,255,643 →
97,355,643). `set_oscillate`, `map_oscillate`, `strmap_oscillate`,
`strmap_churn`, `sync_strmap_remove`, `sync_strmap_churn`,
`sync_bytesmap_remove` and `sync_blobmap_remove` are unchanged to the
instruction.

Non-target arms above +0.1% (each falsifies G-ins on its own):

| Arm | `main` | `H1` | Change |
|---|---|---|---|
| `sync_map_write_twin/random` | 1,860,875 | 1,882,182 | +1.14% |
| `sync_map_write_twin/one_top_byte` | 2,066,751 | 2,086,058 | +0.93% |
| `sync_map_insert/random` | 49,669,212 | 49,887,349 | +0.44% |
| `sync_map_compare_exchange/random` | 41,957,862 | 42,088,776 | +0.31% |
| `sync_blobmap_overwrite/random` | 32,010,456 | 32,057,050 | +0.15% |

Four string arms moved down: `sync_strmap_insert_short/short` −1.49%,
`sync_strmap_insert_sorted/uuid` −1.40% and `sync_strmap_insert/routes`
−0.16% (non-target), and `sync_strmap_churn_short/short` −1.20% (target).

`callgrind-smoke` in the same run: `map_churn/random` +2.01% and
`bytesmap_churn/routes` +0.91%, both target arms. The C ABI arms of the target
set (`judyl_churn_expanse`, `judyl_churn_expanse_dl`) were **not measured
under the arm**: `expanse-capi` depends on `expanse-trie` with
`default-features = false`, so it built without `subtree-condense`.

**Attribution: none.** No `callgrind_annotate` base-against-head profile and
no disassembly diff were taken, so the per-function cause of every change
above, the non-target arms and the decreases included, is **unattributed**.
G-ins's falsifier trips on that clause alone ("any target arm whose change
is not attributed").

Recorded, not part of G-ins: the `wasm-fuel` job in the same run
(workload: wasm_fuel) read `map_remove/random` +117.99%, `set_remove/random`
+117.58%, `set_remove/clustered` +44.68%, `map_remove/clustered` +24.27% and
`set_remove/sequential` +21.54%.

### 2.3 G-thrash

Same run and job (workload: core_instructions). Per-operation counts divide
by the operation counts in `scripts/perf_report.py` (32,768 for `band2`,
147,456 for `band9`, 1,024 for each isolating arm). C_split and C_condense
from `condense_bounds.isolated_cost`, the bound from
`condense_bounds.thrash_bound`:

| Quantity | Event arm | Control arm | Per event |
|---|---|---|---|
| C_split | `set_subtree_split` 11,847,744 | `set_subtree_split_control` 392,369 | 11,186.9 |
| C_condense (`H1`) | `set_subtree_condense/h1` 15,970,726 | `set_subtree_condense_control/h1` 743,459 | 14,870.4 |

| Input | H | `main` per op | `H1` per op | Bound | Verdict |
|---|---|---|---|---|---|
| `band2` | 1 | 518.7 | 6,745.5 | 7,033.0 | met |
| `band9` | 8 | 482.4 | 1,849.2 | 1,930.0 | met |

`set_subtree_split` and its control read the same count on `main` and under
`H1`: condensing does not touch the insert path.

**The evaluation check alone has a cost.** `set_subtree_condense_control/h1`
removes one key from each of 1,024 expanses at 33 keys, landing on 32, which
is not an evaluation point, so nothing condenses. It rose from 646,179 to
743,459 instructions, +15.05%: 95.0 instructions per remove with no
condense (derived: 97,280 / 1,024). Unattributed, like every other change in
§2.2. This is the one shape it was measured on; whether every remove pays
the same is not measured.

### 2.4 G-valid

Same run: the workspace tests on `ubuntu-latest`, `macos-latest` and
`windows-latest`, the ASan job and both Tier-1 Miri shards are green with
`H1` on by default. The falsifier ("any failure in any of them") did not
trip. Coverage caveat: the Tier-1 Miri filter (AGENTS.md §5) does not select
the new `condense::` unit tests, so Miri did not run them. Before CI, the
local run on the engine commit with each arm on (`cargo test -p expanse-trie
--features subtree-condense,diag-entry`, and the same with
`subtree-condense-wide`, `PROPTEST_CASES=500`) passed 646 tests and failed 0
per arm, including the proptest model suites, the validator on every drained
tree and `tests/test_subtree_condense.rs` (as recorded on the phase 3 branch at
`f9c81639`; no log is committed). Not a G-valid job: the run's
`Public Rust API Surface` job failed because the feature's five public items
are absent from the committed snapshot.

### 2.5 Recorded, not gated

- **The `worst_l3_16_1` shape grows.** The METHODOLOGY §4 model shape reads
  368 B drained under both arms against 288 B on `main` (`model_pins`,
  `worst_l3_16_1_drained`, in `results/phase3_retention_h1.json` and
  `results/phase3_retention_wide.json`; measured as §2.1). The drain passes an
  evaluation point where the packed leaf is the smaller form, condenses, and
  then drains the leaf to 17 keys, where the branch `main` keeps is smaller.
  The byte rule decides at the evaluation point only. No grid cell shows this
  (every arm-to-`main` ratio in §2.1 is at most 1.0000), but it is a shape
  where an arm holds more than `main`, and no prediction named it.
- `mem_held()` after `shrink_to_fit()` over the fresh build's `mem_used()`, on
  the headline cell: `main` 8.218 (set) / 4.440 (map), `H1` 2.132 / 1.256,
  `wide` 3.969 / 2.539 (measured as §2.1). On `r64_to_2m` it is higher under
  `H1` than on `main`: 2.839 against 2.811 (set), 2.425 against 2.196 (map).
  The cause is not measured.

### 2.6 How much a condense could cost: the feasibility bound

What per-condense cost would let eager condensing keep a partial-drain target
arm within G-ins's +5%? `condense_bounds.max_condense_cost`:

    C_max = (0.05 × base − removes × per_remove_overhead) / condenses

Inputs (derived unless marked):

- `base`: `main`'s count for the arm, 78,397,685 (`set_remove_partial`) and
  69,718,456 (`map_remove_partial`) (measured: run 36192522024, §2.2).
- `removes`: 137,500, the arm's 200,000 − 62,500
  (`crates/expanse/benches/instructions.rs`, `PARTIAL_N`, `PARTIAL_M`).
- `condenses`: **4,074**, from `condense_bounds.partial_arm_condenses`, which
  replays the arm's own keys and removal order (the harness's generator and
  seeds, checked against the source by `test_bench_sync`). Of the 4,096
  level-6 expanses, 4,074 hold more than `LEAF_CAP` keys at build; every one
  drains to at most 31, and under `H1` the byte rule accepts each at 31, so
  each condenses once, with 0 evaluations declined. No other level can
  condense on this arm: the smallest level-7 survivor count is 3,836 and the
  largest level-5 child holds 5 keys. The replay prices drained children in
  their fresh form, which can only under-count accepted evaluations; it is a
  model count, not an instrumented one.

| Arm | 5% budget | C_max, no per-remove overhead | C_max with 95.0 per remove |
|---|---|---|---|
| `set_remove_partial` | 3,919,884 | **962.2** | −2,244.1 (infeasible) |
| `map_remove_partial` | 3,485,923 | **855.7** | −2,350.7 (infeasible) |

The measured C_condense of `H1` at level 6 on the set is 14,870.4 (§2.3),
15.5 × the set's ceiling. The second column charges every remove the 95.0
instructions measured on the condense control (§2.3); that alone spends
13,062,500 instructions, over three times the budget, but whether every
remove pays it is not measured, so that column is conditional on it. The
model ignores that removes from a condensed leaf may cost more or less than
the same removes from the branch it replaced.

Arithmetic, not an attribution: 4,074 condenses × 14,870.4 = 60,581,920
instructions, against a measured rise of 63,890,831 on `set_remove_partial`,
leaving 24.1 per remove. This carries a per-call cost measured on one arm
(`set_subtree_condense/h1`) to another with the call count stated above; no
profile shows that the rise is made of condenses.

### 2.7 What follows

The next step is measure-first: an allocator census of a drained tree and a
comparison against rebuilding it, before any further engine change.
